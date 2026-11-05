//! The LE Audio worker API on [`BtHost`] (v1 le_audio::Core's worker
//! functions): blocking calls for core's LE session and the CLI that run
//! their work on the BTstack thread and wait for it with a timeout. A user
//! stop ([`crate::Canceller`], `cancel_pending_waits`) ends the waits, as
//! for A2DP; the teardown calls (`le_stop_stream`, `le_disconnect`,
//! `le_disconnect_all`) are not cancellable, so a stop still tears down.
//! `le_scan` and `le_find` start a user operation: they clear a cancel left
//! from an earlier stop (as `BtHost::connect` / `scan` do). Never call these
//! from the BTstack thread (a hook, a GATT callback): they would deadlock
//! until their timeout.
//!
//! With `HostConfig::le_audio` None every call fails at once (`le_ready`
//! false, empty results, errors).

use std::sync::atomic::Ordering;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use le_audio::plan::StreamPlan;
use le_audio::sdu_queue::{SduQueue, SduSet};

use super::core::{ConnectResult, LeCtx, LeDone};
use super::iso_send::queue_depth;
use super::{
    LeAddr, LeDeviceId, LeDeviceInfo, LeScanResult, LeSduSink, LeStreamRequest, LeStreamStarted, LeStreamStats,
    LeStreamStatus,
};
use crate::BtHost;

/// v1 Core::run's default
const RUN_TIMEOUT: Duration = Duration::from_secs(3);
/// The whole BAP set-up (v1)
const START_TIMEOUT: Duration = Duration::from_secs(45);
const STOP_TIMEOUT: Duration = Duration::from_secs(8);
const DISCONNECT_TIMEOUT: Duration = Duration::from_secs(3);
/// Waits look at the cancel this often
const POLL: Duration = Duration::from_millis(50);
const FIND_POLL: Duration = Duration::from_millis(100);

/// A completion the worker waits for (the first `set` counts)
#[derive(Default)]
struct Signal {
    value: Mutex<Option<bool>>,
    cv: Condvar,
}

impl Signal {
    fn set(&self, ok: bool) {
        let mut v = self.value.lock().unwrap_or_else(|e| e.into_inner());
        if v.is_none() {
            *v = Some(ok);
            self.cv.notify_all();
        }
    }

    fn wait(&self, timeout: Duration) -> Option<bool> {
        let v = self.value.lock().unwrap_or_else(|e| e.into_inner());
        let (v, _) = self.cv.wait_timeout_while(v, timeout, |v| v.is_none()).unwrap_or_else(|e| e.into_inner());
        *v
    }
}

impl BtHost {
    /// LE Audio usable: configured, the LE core initialized and HCI working
    /// (v1 Core::ready)
    pub fn le_ready(&self) -> bool {
        self.le_on && self.le.initialized.load(Ordering::Acquire) && self.le.hci_working.load(Ordering::Acquire)
    }

    /// ISO buffers of the controller (LE Read Buffer Size v2):
    /// (ISO_Data_Packet_Length, Total_Num_ISO_Data_Packets). (0, 0): no ISO
    /// support, `le_start_stream` refuses (BTstack's ISO send would loop for
    /// ever).
    pub fn le_iso_buffers(&self) -> (u16, u8) {
        self.le.iso_buffers()
    }

    /// LE scan (extended scanning when the controller has it, else legacy)
    /// for `duration`: all advertisers, `le_audio` marks LE Audio devices
    pub fn le_scan(&self, duration: Duration) -> Vec<LeScanResult> {
        if !self.le_ready() {
            return Vec::new();
        }
        self.le_fresh_operation();
        tracing::info!("LE Audio: scanning for {} ms", duration.as_millis());
        self.le_run(RUN_TIMEOUT, |ctx| ctx.start_scan());
        let end = Instant::now() + duration;
        while Instant::now() < end && !self.le_cancelled() {
            std::thread::sleep(POLL.min(end.saturating_duration_since(Instant::now())));
        }
        self.le_run(RUN_TIMEOUT, |ctx| ctx.stop_scan()).unwrap_or_default()
    }

    /// Scans until `addr` is seen (to learn its address type and name).
    /// `addr` may be an identity address or an RPA of a bonded device: an
    /// advertiser whose RPA resolves to the same identity matches too, and
    /// the result's `addr` is the address it advertises now (connect with
    /// that). None on timeout / cancel.
    pub fn le_find(&self, addr: [u8; 6], timeout: Duration) -> Option<LeScanResult> {
        if !self.le_ready() {
            return None;
        }
        self.le_fresh_operation();
        let want = LeAddr { addr, addr_type: LeAddr::RANDOM };
        self.le_run(RUN_TIMEOUT, move |ctx| ctx.start_find(want));
        let end = Instant::now() + timeout;
        let mut found = None;
        tracing::debug!("LE Audio: find {want} for {} ms", timeout.as_millis());
        while found.is_none() && Instant::now() < end && !self.le_cancelled() {
            std::thread::sleep(FIND_POLL);
            found = self.le_run(RUN_TIMEOUT, move |ctx| ctx.find_match(want)).flatten();
        }
        tracing::debug!(
            "LE Audio: find {want}: {}",
            found.as_ref().map_or("not found".to_string(), |r| format!("{} (type {})", r.addr, r.addr.addr_type))
        );
        self.le_run(RUN_TIMEOUT, |ctx| {
            ctx.stop_scan();
        });
        found
    }

    /// Connects, pairs / encrypts, discovers GATT, reads PACS / ASCS, runs
    /// the modules' on_gatt_ready ("connecting with <addr>", "LE Audio:
    /// <addr> ready (device N)"). An already connected device returns at
    /// once. `addr` may be an RPA or the identity of a bonded device: the id
    /// is the same for both. Err: why, for the log.
    pub fn le_connect(&self, addr: LeAddr, name: &str, timeout: Duration) -> Result<LeDeviceId, String> {
        if !self.le_ready() {
            return Err("LE Audio is not running".into());
        }
        tracing::info!("LE Audio: connecting to {addr} (address type {})", addr.addr_type);
        let result = Arc::new(Mutex::new(ConnectResult::default()));
        let (r, name) = (result.clone(), name.to_string());
        let ok = self.le_wait(timeout, true, move |ctx, done| ctx.start_connect(addr, name, r, done));
        let (id, error) = {
            let r = result.lock().unwrap_or_else(|e| e.into_inner());
            (r.id, r.error.clone())
        };
        match (ok, id) {
            (true, Some(id)) => {
                tracing::info!("LE Audio: {addr} ready (device {id})");
                Ok(LeDeviceId(id))
            }
            _ => {
                let error = if !error.is_empty() {
                    error
                } else if self.le_cancelled() {
                    "cancelled".into()
                } else {
                    "no answer (timeout)".into()
                };
                tracing::warn!("LE Audio: connecting to {addr} failed: {error}");
                // Abandon the attempt: cancel the connect or drop the link
                // (the id is read there: a merge may have moved it)
                let r = result.clone();
                self.le_run(RUN_TIMEOUT, move |ctx| {
                    let id = r.lock().unwrap_or_else(|e| e.into_inner()).id;
                    ctx.abandon_connect(id);
                });
                Err(error)
            }
        }
    }

    /// Disconnects one device (waits up to 3 s)
    pub fn le_disconnect(&self, device: LeDeviceId) {
        if !self.le_on {
            return;
        }
        let ok = self.le_wait(DISCONNECT_TIMEOUT, false, move |ctx, done| ctx.disconnect_device(device, done));
        tracing::debug!(
            "LE Audio: disconnect device {}: {}",
            device.0,
            if ok { "done" } else { "failed or timed out" }
        );
    }

    /// Disconnects every LE device (waits up to 3 s)
    pub fn le_disconnect_all(&self) {
        for d in self.le_devices() {
            if d.connected {
                self.le_disconnect(d.id);
            }
        }
    }

    /// Plans and sets up the unicast stream to `devices` (connected and
    /// ready), after the modules' prepare_group / adjust_stream_request:
    /// plan_stream, the jitter buffer (with the encoded silence set
    /// `silence(&plan)` gives: one silent LC3 SDU per CIS), the BAP
    /// procedure, ISO sending. Returns once audio may flow ("Streaming LE
    /// Audio: <summary>"); push the SDU sets into the returned sink. Err:
    /// why ("adapter does not support LE Audio (no ISO buffers)", no common
    /// configuration, a BAP step that failed or timed out).
    pub fn le_start_stream(
        &self,
        mut devices: Vec<LeDeviceId>,
        mut request: LeStreamRequest,
        silence: &dyn Fn(&StreamPlan) -> SduSet,
    ) -> Result<LeStreamStarted, String> {
        if !self.le_ready() {
            return Err("LE Audio is not running".into());
        }
        let (iso_len, iso_count) = self.le_iso_buffers();
        if iso_len == 0 || iso_count == 0 {
            // BTstack's ISO send would loop for ever without ISO buffers
            let why = "adapter does not support LE Audio (no ISO buffers)";
            tracing::error!("LE Audio: cannot stream: {why}");
            return Err(why.into());
        }
        self.le.stream_lost.store(false, Ordering::Release);
        let hooks = self.le.hooks();
        let cancelled = || self.le_cancelled();
        for h in &hooks {
            tracing::debug!("LE Audio: {}: preparing the group", h.name());
            h.prepare_group(self, &mut devices, &cancelled);
        }
        tracing::debug!(
            "LE Audio: stream request {request:?} for devices {:?} (ISO {iso_count} x {iso_len})",
            devices.iter().map(|d| d.0).collect::<Vec<_>>()
        );
        for h in &hooks {
            h.adjust_stream_request(self, &devices, &mut request);
        }
        tracing::debug!(
            "LE Audio: stream request after the modules {request:?} for devices {:?}",
            devices.iter().map(|d| d.0).collect::<Vec<_>>()
        );

        let (ids, req) = (devices.clone(), request.clone());
        let plan = match self.le_run(RUN_TIMEOUT, move |ctx| ctx.plan_for(&ids, &req)) {
            Some(Ok(plan)) => plan,
            r => {
                let why = match r {
                    Some(Err(why)) if !why.is_empty() => why,
                    Some(_) => "planning failed".to_string(),
                    None => "the Bluetooth stack did not answer".to_string(),
                };
                tracing::error!("LE Audio: cannot stream: {why}");
                return Err(why);
            }
        };

        // The jitter buffer, with the silence for underruns
        let queue = Arc::new(Mutex::new(SduQueue::new()));
        tracing::debug!(
            "LE Audio: jitter buffer for {} CIS, controller queue depth {}",
            plan.cis.len(),
            queue_depth(iso_count, plan.cis.len()).max(1)
        );
        queue
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .configure(queue_depth(iso_count, plan.cis.len()).max(1), silence(&plan));
        let status = LeStreamStatus { active: false, plan: plan.clone(), devices, summary: plan.describe() };

        let why = Arc::new(Mutex::new(String::new()));
        let (q, st, w) = (queue.clone(), status.clone(), why.clone());
        let ok = self.le_wait(START_TIMEOUT, true, move |ctx, done| ctx.start_unicast(plan, q, st, w, done));
        if !ok {
            let why = why.lock().unwrap_or_else(|e| e.into_inner()).clone();
            let reason = if !why.is_empty() {
                why
            } else if self.le_cancelled() {
                "cancelled".into()
            } else {
                "timeout".into()
            };
            tracing::error!("LE Audio: stream setup failed: {reason}");
            self.le_stop_stream();
            return Err(reason);
        }
        let status = self.le_run(RUN_TIMEOUT, |ctx| ctx.stream_status()).flatten().unwrap_or(status);
        tracing::info!("LE Audio: streaming: {}", status.summary);
        Ok(LeStreamStarted { status, sink: LeSduSink::new(queue) })
    }

    /// Disable / release the ASEs, remove the CIG (devices stay connected;
    /// waits up to 8 s)
    pub fn le_stop_stream(&self) {
        if !self.le_on || !self.le.initialized.load(Ordering::Acquire) {
            return;
        }
        let started = Instant::now();
        let ok = self.le_wait(STOP_TIMEOUT, false, |ctx, done| ctx.stop_unicast(done));
        tracing::debug!(
            "LE Audio: stream stop {} after {} ms",
            if ok { "done" } else { "timed out" },
            started.elapsed().as_millis()
        );
    }

    /// Starts a broadcast (Auracast source, developer mode): advertising with
    /// the announcements and the BASE, a BIG with one BIS per channel of
    /// `le_audio::broadcast::plan(&config)`, ISO sending. Returns once audio
    /// may flow ("Auracast: broadcasting ..."); push the SDU sets into the
    /// returned sink. Nothing needs to be connected. Err: why (no ISO
    /// buffers, the adapter cannot advertise or create the BIG, timeout).
    pub fn le_start_broadcast(
        &self,
        config: le_audio::broadcast::BroadcastConfig,
        silence: &dyn Fn(&StreamPlan) -> SduSet,
    ) -> Result<LeStreamStarted, String> {
        if !self.le_ready() {
            return Err("LE Audio is not running".into());
        }
        let (iso_len, iso_count) = self.le_iso_buffers();
        if iso_len == 0 || iso_count == 0 {
            let why = "adapter does not support LE Audio (no ISO buffers)";
            tracing::error!("Auracast: cannot broadcast: {why}");
            return Err(why.into());
        }
        let plan = le_audio::broadcast::plan(&config);
        if usize::from(plan.preset.octets) + 4 > usize::from(iso_len) {
            let why = format!("SDUs of {} octets do not fit the adapter's ISO packets", plan.preset.octets);
            tracing::error!("Auracast: cannot broadcast: {why}");
            return Err(why);
        }
        self.le.stream_lost.store(false, Ordering::Release);
        let summary = le_audio::broadcast::describe(&plan);
        let depth = queue_depth(iso_count, plan.cis.len()).max(1);
        let queue = Arc::new(Mutex::new(SduQueue::new()));
        queue.lock().unwrap_or_else(|e| e.into_inner()).configure(depth, silence(&plan));
        let status =
            LeStreamStatus { active: false, plan: plan.clone(), devices: Vec::new(), summary: summary.clone() };
        let why = Arc::new(Mutex::new(String::new()));
        let (q, st, w, cfg, pl) = (queue.clone(), status, why.clone(), config.clone(), plan.clone());
        let ok = self.le_wait(START_TIMEOUT, true, move |ctx, done| ctx.start_broadcast(cfg, pl, q, st, w, done));
        if !ok {
            let why = why.lock().unwrap_or_else(|e| e.into_inner()).clone();
            let reason = if !why.is_empty() {
                why
            } else if self.le_cancelled() {
                "cancelled".into()
            } else {
                "timeout".into()
            };
            tracing::error!("Auracast: setup failed: {reason}");
            self.le_stop_broadcast();
            return Err(reason);
        }
        let plan = self.le_run(RUN_TIMEOUT, |ctx| ctx.broadcast_plan()).flatten().unwrap_or(plan);
        tracing::info!(
            "Auracast: broadcasting \"{}\", Broadcast ID 0x{:06X}, {}: {summary}",
            config.name,
            config.broadcast_id,
            if config.code.is_some() { "encrypted" } else { "open" }
        );
        let status = LeStreamStatus { active: true, plan, devices: Vec::new(), summary };
        Ok(LeStreamStarted { status, sink: LeSduSink::new(queue) })
    }

    /// Ends the broadcast: LE Terminate BIG, advertising off (waits up to
    /// 8 s)
    pub fn le_stop_broadcast(&self) {
        if !self.le_on || !self.le.initialized.load(Ordering::Acquire) {
            return;
        }
        let started = Instant::now();
        let ok = self.le_wait(STOP_TIMEOUT, false, |ctx, done| ctx.stop_broadcast(done));
        tracing::debug!(
            "Auracast: stop {} after {} ms",
            if ok { "done" } else { "timed out" },
            started.elapsed().as_millis()
        );
    }

    /// True once after a device or CIS of the running stream was lost (the
    /// session stops and sets the stream up again)
    pub fn le_stream_lost(&self) -> bool {
        self.le.stream_lost.swap(false, Ordering::AcqRel)
    }

    /// Counters of the running (or last) stream
    pub fn le_stream_stats(&self) -> LeStreamStats {
        *self.le.stats.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The devices the LE core knows
    pub fn le_devices(&self) -> Vec<LeDeviceInfo> {
        self.le_run(RUN_TIMEOUT, |ctx| ctx.device_infos()).unwrap_or_default()
    }

    /// Headset volume through the modules (VCP), 0-127, None while unknown;
    /// any thread
    pub fn le_remote_volume(&self) -> Option<u8> {
        self.le.hooks().iter().find_map(|h| h.remote_volume())
    }

    /// Sets the headset volume (0-127) through the modules (VCP); false if
    /// no module handles it. Any thread.
    pub fn le_request_volume(&self, level: u8) -> bool {
        let handled = self.le.hooks().iter().any(|h| h.request_volume(level));
        tracing::debug!("LE Audio: volume request {level}: handled {handled}");
        handled
    }

    /// "Low latency (GMAP)": read when a stream is set up. Off by default;
    /// per BtHost (set it after each start).
    pub fn le_set_low_latency(&self, on: bool) {
        tracing::debug!("LE Audio: low latency (GMAP) {on}");
        self.le.low_latency.store(on, Ordering::Release);
    }

    pub fn le_low_latency(&self) -> bool {
        self.le.low_latency.load(Ordering::Acquire)
    }

    // ---- for the modules' worker hooks (v1 Core::run / call / post) ----

    /// Runs `f` on the BTstack thread and waits for its result; None if
    /// the run loop did not take it within `timeout` (it may still run
    /// later: capture owned data only)
    pub(crate) fn le_run<R: Send + 'static>(
        &self,
        timeout: Duration,
        f: impl FnOnce(&mut LeCtx<'_>) -> R + Send + 'static,
    ) -> Option<R> {
        if !self.le_on {
            return None;
        }
        self.call(timeout, move |app, stack| app.le.as_mut().filter(|le| le.initialized()).map(|le| le.run(stack, f)))
            .flatten()
    }

    /// Runs `f` on the BTstack thread; it calls `done(ok)` when finished,
    /// maybe later from a GATT / timer callback. Waits up to `timeout` or
    /// until cancelled; false on timeout / cancel / done(false).
    pub(crate) fn le_call(&self, timeout: Duration, f: impl FnOnce(&mut LeCtx<'_>, LeDone) + Send + 'static) -> bool {
        self.le_wait(timeout, true, f)
    }

    /// le_call with or without the cancel (teardown waits are not
    /// cancellable)
    fn le_wait(
        &self,
        timeout: Duration,
        cancellable: bool,
        f: impl FnOnce(&mut LeCtx<'_>, LeDone) + Send + 'static,
    ) -> bool {
        if !self.le_on {
            return false;
        }
        let signal = Arc::new(Signal::default());
        let s = signal.clone();
        let done: LeDone = Box::new(move |ok| s.set(ok));
        self.post(move |app, stack| match app.le.as_mut().filter(|le| le.initialized()) {
            Some(le) => le.run(stack, move |ctx| f(ctx, done)),
            None => done(false),
        });
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if let Some(ok) = signal.wait(left.min(POLL)) {
                return ok;
            }
            if cancellable && self.le_cancelled() {
                tracing::debug!("LE Audio: wait cancelled");
                return false;
            }
            if Instant::now() >= deadline {
                tracing::debug!("LE Audio: no completion within {} ms", timeout.as_millis());
                return false;
            }
        }
    }

    /// A user stop is pending
    fn le_cancelled(&self) -> bool {
        self.shared.lock().cancel
    }

    /// A fresh user-initiated operation: a cancel left by an earlier stop
    /// would end it at once
    fn le_fresh_operation(&self) {
        self.shared.update(|s| s.cancel = false);
    }
}
