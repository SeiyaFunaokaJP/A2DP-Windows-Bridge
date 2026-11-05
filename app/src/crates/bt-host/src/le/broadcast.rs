//! Broadcast source (Auracast, developer mode) on the BTstack thread: one
//! extended + periodic advertising set announcing the broadcast
//! (`le_audio::broadcast`: Broadcast Audio / Public Broadcast Announcement,
//! Broadcast Name, the BASE) and one BIG with a BIS per channel, fed by the
//! same jitter buffer and [`super::iso_send::IsoSender`] as a unicast
//! stream.
//!
//! start: advertising, LE Create BIG -> GAP_SUBEVENT_BIG_CREATED (BTstack
//! has set up the ISO data paths) -> sending on the BIS, `done(true)`.
//! stop: stop sending, LE Terminate BIG -> GAP_SUBEVENT_BIG_TERMINATED ->
//! advertising removed ("Auracast: stopped"). A BIG the controller ends by
//! itself counts as a lost stream.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use btstack_sys::BigParams;
use btstack_sys::consts::ERROR_CODE_SUCCESS;
use btstack_sys::events;
use le_audio::broadcast::{
    ADV_INTERVAL, ADV_SID, BIG_HANDLE, BroadcastConfig, PA_INTERVAL, adv_data, base_ad, describe,
};
use le_audio::plan::StreamPlan;

use super::LeStreamStatus;
use super::core::{LeCtx, LeDone, LeTimerId, SharedSduQueue};
use super::iso_send::queue_depth;

/// A stop that gets no LE Terminate BIG Complete ends after this anyway
const TERMINATE_TIMEOUT_MS: u32 = 3000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// LE Create BIG sent
    Creating,
    Streaming,
    /// LE Terminate BIG sent
    Terminating,
}

/// The running broadcast (LeCore::broadcast)
pub(crate) struct Broadcast {
    state: State,
    plan: StreamPlan,
    /// The start waiting for the BIG
    started: Option<(LeDone, Arc<Mutex<String>>)>,
    /// Stops waiting for the BIG to end
    stopped: Vec<LeDone>,
    terminate_timer: Option<LeTimerId>,
}

impl Broadcast {
    /// Bluetooth stopped (stack deinit): no BTstack calls; the waits end
    pub(crate) fn abandon(mut self) {
        if let Some((start, why)) = self.started.take() {
            set_why(&why, "Bluetooth stopped");
            start(false);
        }
        for d in self.stopped.drain(..) {
            d(true);
        }
    }
}

fn set_why(why: &Arc<Mutex<String>>, s: &str) {
    *why.lock().unwrap_or_else(|e| e.into_inner()) = s.into();
}

impl LeCtx<'_> {
    /// le_start_broadcast's work: advertising and LE Create BIG; `done(true)`
    /// once the BIS are sending
    pub(crate) fn start_broadcast(
        &mut self,
        config: BroadcastConfig,
        plan: StreamPlan,
        queue: SharedSduQueue,
        status: LeStreamStatus,
        why: Arc<Mutex<String>>,
        done: LeDone,
    ) {
        if self.core.broadcast.is_some() {
            set_why(&why, "a broadcast is still being stopped");
            done(false);
            return;
        }
        if self.core.status.is_some() {
            set_why(&why, "an LE Audio stream is running");
            done(false);
            return;
        }
        let adv = adv_data(&config);
        let pa = base_ad(&plan);
        if let Err(status) = self.stack.broadcast_advertise(ADV_SID, ADV_INTERVAL, PA_INTERVAL, &adv, &pa) {
            tracing::warn!("Auracast: advertising failed (0x{status:02X})");
            set_why(&why, &format!("the adapter cannot advertise a broadcast (0x{status:02X})"));
            done(false);
            return;
        }
        let p = &plan.preset;
        let params = BigParams {
            big_handle: BIG_HANDLE,
            num_bis: plan.cis.len() as u8,
            sdu_interval_us: p.sdu_interval_us(),
            max_sdu: p.octets,
            max_transport_latency_ms: p.max_transport_latency_ms,
            rtn: p.rtn,
            phy: plan.phy,
            framing: p.framing,
            code: config.code,
        };
        let status_code = self.stack.big_create(&params);
        if status_code != ERROR_CODE_SUCCESS {
            self.stack.broadcast_advertise_stop();
            tracing::warn!("Auracast: LE Create BIG refused (0x{status_code:02X})");
            set_why(&why, &format!("LE Create BIG failed (0x{status_code:02X})"));
            done(false);
            return;
        }
        tracing::debug!("Auracast: advertising, creating the BIG: {}", describe(&plan));
        self.core.queue = Some(queue);
        self.core.status = Some(status);
        self.core.broadcast = Some(Broadcast {
            state: State::Creating,
            plan,
            started: Some((done, why)),
            stopped: Vec::new(),
            terminate_timer: None,
        });
    }

    /// le_stop_broadcast's work: stop sending, LE Terminate BIG, then the
    /// advertising goes; `done(true)` at the end
    pub(crate) fn stop_broadcast(&mut self, done: LeDone) {
        self.core.iso.stop(self.stack);
        let Some(b) = self.core.broadcast.as_mut() else {
            done(true);
            return;
        };
        if let Some((start, why)) = b.started.take() {
            set_why(&why, "stopped");
            start(false);
        }
        b.stopped.push(done);
        if b.state == State::Terminating {
            return;
        }
        b.state = State::Terminating;
        let status = self.stack.big_terminate(BIG_HANDLE);
        tracing::debug!("Auracast: terminating the BIG: 0x{status:02X}");
        if status != ERROR_CODE_SUCCESS {
            self.finish_broadcast();
            return;
        }
        let timer = self.run_after(
            TERMINATE_TIMEOUT_MS,
            Box::new(|ctx| {
                if let Some(b) = ctx.core.broadcast.as_mut() {
                    b.terminate_timer = None;
                }
                if ctx.core.broadcast.as_ref().is_some_and(|b| b.state == State::Terminating) {
                    tracing::warn!("Auracast: no answer to LE Terminate BIG, removing the broadcast");
                    ctx.finish_broadcast();
                }
            }),
        );
        if let Some(b) = self.core.broadcast.as_mut() {
            b.terminate_timer = Some(timer);
        }
    }

    /// GAP_SUBEVENT_BIG_CREATED / _TERMINATED
    pub(crate) fn on_big_event(&mut self, p: &[u8]) {
        if let Some(c) = events::big_created(p) {
            self.on_big_created(c);
        } else if let Some(handle) = events::big_terminated(p) {
            tracing::debug!("Auracast: BIG {handle} terminated");
            match self.core.broadcast.as_ref().map(|b| b.state) {
                Some(State::Terminating) => self.finish_broadcast(),
                Some(_) => {
                    // The controller ended it (or BTstack after a failed
                    // ISO path)
                    self.core.iso.stop(self.stack);
                    if !self.core.shared.stream_lost.swap(true, Ordering::AcqRel) {
                        tracing::warn!("LE Audio: stream lost: the BIG ended");
                    }
                    self.finish_broadcast();
                }
                None => {}
            }
        }
    }

    fn on_big_created(&mut self, c: events::BigCreated) {
        let Some(b) = self.core.broadcast.as_mut() else { return };
        tracing::debug!(
            "Auracast: BIG {} created: status 0x{:02X}, BIS {:04X?}",
            c.big_handle,
            c.status,
            c.bis_handles
        );
        match b.state {
            State::Terminating => {
                if c.status == ERROR_CODE_SUCCESS {
                    // Created after the stop asked for its end
                    self.stack.big_terminate(BIG_HANDLE);
                } else {
                    self.finish_broadcast();
                }
            }
            State::Streaming => {}
            State::Creating => {
                if c.status != ERROR_CODE_SUCCESS || c.bis_handles.len() != b.plan.cis.len() {
                    let why = format!("LE Create BIG failed (0x{:02X})", c.status);
                    tracing::warn!("Auracast: {why}");
                    if let Some((start, w)) = b.started.take() {
                        set_why(&w, &why);
                        start(false);
                    }
                    self.finish_broadcast();
                    return;
                }
                for (bis, &h) in b.plan.cis.iter_mut().zip(&c.bis_handles) {
                    bis.cis_handle = h;
                }
                b.state = State::Streaming;
                let frame_us = b.plan.preset.sdu_interval_us();
                let plan = b.plan.clone();
                let start = b.started.take();
                let depth = queue_depth(self.core.shared.iso_buffers().1, c.bis_handles.len()).max(1);
                if let Some(s) = self.core.status.as_mut() {
                    s.active = true;
                    s.plan = plan;
                }
                let queue = self.core.queue.clone().unwrap_or_default();
                self.core.iso.start_big(self.stack, BIG_HANDLE, &c.bis_handles, queue, frame_us, depth);
                if let Some((start, _)) = start {
                    start(true);
                }
            }
        }
    }

    /// The broadcast is over: advertising off, state gone, stops completed
    fn finish_broadcast(&mut self) {
        self.core.iso.stop(self.stack);
        self.stack.broadcast_advertise_stop();
        let Some(mut b) = self.core.broadcast.take() else { return };
        if let Some(t) = b.terminate_timer.take() {
            self.cancel_timer(t);
        }
        if let Some((start, why)) = b.started.take() {
            set_why(&why, "the broadcast ended");
            start(false);
        }
        self.core.status = None;
        self.core.queue = None;
        tracing::info!("Auracast: stopped");
        for d in b.stopped.drain(..) {
            d(true);
        }
    }

    /// The running broadcast's plan (BIS handles filled in)
    pub(crate) fn broadcast_plan(&self) -> Option<StreamPlan> {
        self.core.broadcast.as_ref().map(|b| b.plan.clone())
    }
}
