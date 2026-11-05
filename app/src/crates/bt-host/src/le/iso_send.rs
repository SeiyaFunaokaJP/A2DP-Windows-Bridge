//! ISO SDU sending on the BTstack thread (v1 Stream's sending half,
//! le_audio_stream.cpp).
//!
//! On HCI_EVENT_CIS_CAN_SEND_NOW the sender takes the next SDU set from the
//! shared jitter buffer ([`le_audio::sdu_queue::SduQueue`], silence on an
//! underrun) and sends each CIS its SDU as one HCI ISO data packet:
//! complete SDU (never fragmented: plan_stream only takes SDUs that fit),
//! sequence number counting up per CIS, no time stamp (the controller
//! places SDUs by sequence number). Two SDUs queued per CIS
//! (hci_set_num_iso_packets_to_queue; queue x CIS within the controller's
//! ISO buffers), and the [`le_audio::sdu_queue::SendGuard`] keeps the sent
//! sets at most queue + 2 intervals ahead of real time. A round starts once
//! every CIS still up got its SDU, so one CIS going down does not stall the
//! others. Logs "LE Audio: no audio for SDU set N" and, at the end, "LE
//! Audio: stopped sending: N SDU sets (...), S silence, U underruns, ...".
//!
//! A broadcast (BIG) is sent the same way on HCI_EVENT_BIS_CAN_SEND_NOW,
//! which BTstack gives for every BIS of the BIG in BIS order once all of
//! them have room (the last one completes the round).
//!
//! No allocation per round: a sent set goes back to the queue's pool
//! ([`le_audio::sdu_queue::SduQueue::recycle`]) when the next round starts,
//! and the silence of an underrun is copied into a spare set.

use std::sync::{Arc, Mutex};

use btstack_sys::consts::ERROR_CODE_SUCCESS;
use btstack_sys::events;
use btstack_sys::{HighTimerResolution, Stack};
use le_audio::sdu_queue::{QueueStats, SduSet, SendCounters, SendGuard};

use super::LeStreamStats;
use super::core::SharedSduQueue;

/// SDUs queued per CIS in the controller (at most)
pub const QUEUE_DEPTH: u8 = 2;

/// The run loop timer of the send guard
pub(crate) const TIMER_ISO: u32 = 4;

/// SDUs BTstack queues per CIS: [`QUEUE_DEPTH`], fewer when the controller
/// has fewer ISO buffers than that for every CIS (queue x CIS must stay
/// within Total_Num_ISO_Data_Packets, which Number Of Completed Packets
/// frees again); 0: not even one per CIS
pub(crate) fn queue_depth(iso_packets: u8, cis: usize) -> u8 {
    if cis == 0 {
        return 0;
    }
    (iso_packets as usize / cis).min(QUEUE_DEPTH as usize) as u8
}

/// What the sender needs of BTstack (the Stack; a fake in the tests)
pub(crate) trait IsoOut {
    fn time_ms(&self) -> u32;
    fn set_queue(&self, packets: u8);
    /// CIS can-send-now for every CIS of the CIG of `cis`
    fn request_can_send(&self, cis: u16);
    /// BIS can-send-now for every BIS of the BIG
    fn request_bis_can_send(&self, _big: u8) {}
    fn send(&self, cis: u16, sequence_number: u16, sdu: &[u8]) -> u8;
    fn timer_start(&self, ms: u32);
    fn timer_stop(&self);
}

impl IsoOut for Stack {
    fn time_ms(&self) -> u32 {
        Stack::time_ms(self)
    }

    fn set_queue(&self, packets: u8) {
        self.set_num_iso_packets_to_queue(packets);
    }

    fn request_can_send(&self, cis: u16) {
        self.request_cis_can_send_now(cis);
    }

    fn request_bis_can_send(&self, big: u8) {
        self.request_bis_can_send_now(big);
    }

    fn send(&self, cis: u16, sequence_number: u16, sdu: &[u8]) -> u8 {
        self.iso_send(cis, sequence_number, sdu)
    }

    fn timer_start(&self, ms: u32) {
        Stack::timer_start(self, TIMER_ISO, ms);
    }

    fn timer_stop(&self) {
        Stack::timer_stop(self, TIMER_ISO);
    }
}

/// The sender of the running stream
#[derive(Default)]
pub struct IsoSender {
    sending: bool,
    /// 1 ms system timer resolution while sending (as v1), so that the
    /// guard timer and time_ms() don't move in 15.6 ms steps
    timer_resolution: Option<HighTimerResolution>,
    /// Sending to a BIG (broadcast): its handle; None: a CIG
    big: Option<u8>,
    /// The CIS (or BIS) handles in plan order
    cis_handles: Vec<u16>,
    seq: Vec<u16>,
    /// Per CIS: not reported down
    alive: Vec<bool>,
    /// Per CIS: has its SDU of the current round
    served: Vec<bool>,
    round_open: bool,
    current: SduSet,
    queue: Option<SharedSduQueue>,
    guard: SendGuard,
    frame_us: u32,
    /// Run loop time of the first round
    t0_ms: Option<u32>,
    /// sets_sent, sdus_sent, send_errors (the rest is the queue's)
    counters: SendCounters,
    queue_stats: QueueStats,
    /// The jitter buffer's prefill (sets)
    prefill: usize,
    /// Send guard waits: how many, the run loop time the current one is
    /// due, and how late the timer fired at most (ms)
    guard_waits: u64,
    guard_due_ms: Option<u32>,
    guard_late_ms: u32,
    /// Published for BtHost::le_stream_stats
    stats_out: Arc<Mutex<LeStreamStats>>,
}

impl IsoSender {
    pub(crate) fn new(stats_out: Arc<Mutex<LeStreamStats>>) -> IsoSender {
        IsoSender { stats_out, ..IsoSender::default() }
    }

    /// Starts sending on these CIS (plan order) from `queue` (configured
    /// for `queue_depth` already), one set per `frame_us`
    pub(crate) fn start(
        &mut self,
        out: &dyn IsoOut,
        cis_handles: &[u16],
        queue: SharedSduQueue,
        frame_us: u32,
        queue_depth: u8,
    ) {
        self.big = None;
        self.begin(out, cis_handles, queue, frame_us, queue_depth);
    }

    /// As [`IsoSender::start`] for the BIS of a BIG (in BIS order)
    pub(crate) fn start_big(
        &mut self,
        out: &dyn IsoOut,
        big: u8,
        bis_handles: &[u16],
        queue: SharedSduQueue,
        frame_us: u32,
        queue_depth: u8,
    ) {
        self.big = Some(big);
        self.begin(out, bis_handles, queue, frame_us, queue_depth);
    }

    fn begin(&mut self, out: &dyn IsoOut, cis_handles: &[u16], queue: SharedSduQueue, frame_us: u32, queue_depth: u8) {
        self.cis_handles = cis_handles.to_vec();
        self.seq = vec![0; cis_handles.len()];
        self.alive = vec![true; cis_handles.len()];
        self.served = vec![false; cis_handles.len()];
        self.round_open = false;
        self.current = SduSet::new();
        self.queue = Some(queue);
        self.frame_us = frame_us.max(1);
        self.guard = SendGuard::new(self.frame_us, u64::from(queue_depth) + 2);
        self.t0_ms = None;
        self.counters = SendCounters::default();
        self.queue_stats = QueueStats::default();
        self.prefill = self.queue.as_ref().map_or(0, |q| q.lock().unwrap_or_else(|e| e.into_inner()).prefill());
        self.guard_waits = 0;
        self.guard_due_ms = None;
        self.guard_late_ms = 0;
        self.publish();
        out.set_queue(queue_depth);
        self.timer_resolution = Some(HighTimerResolution::new());
        self.sending = true;
        let kind = if self.big.is_some() { "BIS" } else { "CIS" };
        tracing::info!("LE Audio: sending ISO SDUs on {} {kind} (queue {queue_depth})", cis_handles.len());
        tracing::debug!(
            "LE Audio: ISO sender: {kind} {cis_handles:04X?}, frame {} us, prefill {} set(s), guard lead {} set(s)",
            self.frame_us,
            self.prefill,
            u64::from(queue_depth) + 2
        );
        self.request_next(out);
    }

    /// Stops sending (logs the totals)
    pub(crate) fn stop(&mut self, out: &dyn IsoOut) {
        if !self.sending {
            return;
        }
        self.sending = false;
        self.timer_resolution = None;
        out.timer_stop();
        if let Some(q) = &self.queue {
            let mut q = q.lock().unwrap_or_else(|e| e.into_inner());
            self.queue_stats = q.stats();
            self.prefill = q.prefill();
            let current = std::mem::take(&mut self.current);
            if !current.is_empty() {
                q.recycle(current);
            }
        }
        self.publish();
        tracing::info!("{}", self.send_counters().stopped_message());
        if self.guard_waits > 0 {
            // The run loop's timer follows the system timer resolution
            // (v1 raised it to 1 ms while sending)
            tracing::info!(
                "LE Audio: send guard waited {} time(s), its timer at most {} ms late",
                self.guard_waits,
                self.guard_late_ms
            );
        }
    }

    #[cfg(test)]
    pub fn sending(&self) -> bool {
        self.sending
    }

    /// HCI_EVENT_CIS_CAN_SEND_NOW
    pub(crate) fn on_cis_can_send_now(&mut self, out: &dyn IsoOut, packet: &[u8]) {
        if !self.sending || self.big.is_some() {
            return;
        }
        let Some(ev) = events::cis_can_send_now(packet) else { return };
        self.on_can_send(out, ev.cis_handle, ev.group_complete);
    }

    /// HCI_EVENT_BIS_CAN_SEND_NOW: the last BIS completes the round
    pub(crate) fn on_bis_can_send_now(&mut self, out: &dyn IsoOut, packet: &[u8]) {
        if !self.sending {
            return;
        }
        let Some(ev) = events::bis_can_send_now(packet) else { return };
        if self.big != Some(ev.big_handle) {
            return;
        }
        // By index: BTstack's event lacks the handle
        let Some(&handle) = self.cis_handles.get(usize::from(ev.bis_index)) else { return };
        let last = usize::from(ev.bis_index) + 1 == self.cis_handles.len();
        self.on_can_send(out, handle, last);
    }

    fn on_can_send(&mut self, out: &dyn IsoOut, handle: u16, group_complete: bool) {
        let Some(index) = self.cis_handles.iter().position(|&h| h == handle) else { return };
        if !self.round_open || self.served[index] {
            // First CIS of a round (whichever CIS is up): the next frame
            // interval for all CIS
            self.start_round();
        }
        if self.current.len() == self.cis_handles.len() {
            let status = out.send(handle, self.seq[index], &self.current[index]);
            tracing::trace!(
                "LE Audio: ISO SDU on 0x{:04X}: seq {}, {} bytes, status 0x{status:02X}",
                handle,
                self.seq[index],
                self.current[index].len()
            );
            self.seq[index] = self.seq[index].wrapping_add(1);
            if status == ERROR_CODE_SUCCESS {
                self.counters.sdus_sent += 1;
            } else {
                self.counters.send_errors += 1;
                let n = self.counters.send_errors;
                if n <= 10 || n.is_power_of_two() {
                    tracing::debug!(
                        "LE Audio: ISO send on 0x{:04X} failed (0x{status:02X}), {n} send error(s) so far",
                        handle
                    );
                }
            }
        } else {
            tracing::trace!(
                "LE Audio: SDU set has {} SDUs for {} CIS, nothing sent on 0x{:04X}",
                self.current.len(),
                self.cis_handles.len(),
                handle
            );
        }
        self.served[index] = true;
        // BTstack's group_complete is never set while the CIG's last CIS is
        // down: also count the CIS that are up ourselves
        if group_complete || self.round_done() {
            self.round_open = false;
            self.publish();
            self.request_next(out);
        }
    }

    fn start_round(&mut self) {
        let mut underrun = None;
        if let Some(q) = &self.queue {
            let mut q = q.lock().unwrap_or_else(|e| e.into_inner());
            tracing::trace!("LE Audio: SDU round {}, {} set(s) buffered", self.counters.sets_sent + 1, q.len());
            let next = match q.pop() {
                Some(set) => set,
                None => {
                    let mut set = q.spare().unwrap_or_default();
                    let silence = q.silence();
                    let same_shape =
                        set.len() == silence.len() && set.iter().zip(silence).all(|(a, b)| a.len() == b.len());
                    if same_shape {
                        for (dst, src) in set.iter_mut().zip(silence) {
                            dst.copy_from_slice(src);
                        }
                    } else {
                        set = silence.clone();
                    }
                    set
                }
            };
            underrun = q.take_underrun();
            let old = std::mem::replace(&mut self.current, next);
            if !old.is_empty() {
                q.recycle(old);
            }
            self.queue_stats = q.stats();
            self.prefill = q.prefill();
        }
        if let Some(u) = underrun {
            tracing::warn!("{u}");
        }
        self.counters.sets_sent += 1;
        if self.counters.sets_sent.is_multiple_of(1000) {
            tracing::debug!(
                "{} (guard waits {}, timer late up to {} ms)",
                self.send_counters().stats_message(),
                self.guard_waits,
                self.guard_late_ms
            );
        }
        self.guard.round_started();
        self.round_open = true;
        self.served.fill(false);
    }

    /// A CIS of the stream disconnected: rounds no longer wait for it
    pub(crate) fn on_cis_down(&mut self, out: &dyn IsoOut, handle: u16) {
        if !self.sending {
            return;
        }
        for (i, &h) in self.cis_handles.iter().enumerate() {
            if h == handle {
                self.alive[i] = false;
            }
        }
        tracing::debug!(
            "LE Audio: ISO sender: CIS 0x{handle:04X} down, {} of {} still up",
            self.alive.iter().filter(|&&a| a).count(),
            self.alive.len()
        );
        // The round may have waited for this CIS only
        if self.alive.iter().any(|&a| a) && self.round_open && self.round_done() {
            self.round_open = false;
            self.request_next(out);
        }
    }

    /// The guard's timer fired: ask for the next round
    pub(crate) fn on_timer(&mut self, out: &dyn IsoOut) {
        if let Some(due) = self.guard_due_ms.take() {
            let late = out.time_ms().wrapping_sub(due) as i32;
            tracing::trace!("LE Audio: send guard timer, {late} ms late");
            self.guard_late_ms = self.guard_late_ms.max(late.max(0) as u32);
        }
        if self.sending {
            self.request_events(out);
        }
    }

    pub fn stats(&self) -> LeStreamStats {
        let c = self.send_counters();
        LeStreamStats {
            sets_encoded: c.sets_encoded,
            sets_sent: c.sets_sent,
            sdus_sent: c.sdus_sent,
            silence_sets: c.silence_sets,
            underruns: c.underruns,
            overflows: c.overflows,
            send_errors: c.send_errors,
            prefill: self.prefill as u32,
        }
    }

    fn send_counters(&self) -> SendCounters {
        SendCounters {
            sets_encoded: self.queue_stats.sets_pushed,
            silence_sets: self.queue_stats.silence_sets,
            underruns: self.queue_stats.underruns,
            overflows: self.queue_stats.overflows,
            ..self.counters
        }
    }

    fn publish(&self) {
        *self.stats_out.lock().unwrap_or_else(|e| e.into_inner()) = self.stats();
    }

    fn round_done(&self) -> bool {
        self.alive.iter().zip(&self.served).all(|(&alive, &served)| !alive || served)
    }

    fn request_events(&self, out: &dyn IsoOut) {
        if let Some(big) = self.big {
            out.request_bis_can_send(big);
            return;
        }
        // BTstack requests CAN_SEND_NOW for every CIS of the CIG from any of
        // its handles: take one that is still up
        if let Some(i) = self.alive.iter().position(|&a| a) {
            out.request_can_send(self.cis_handles[i]);
        }
    }

    fn request_next(&mut self, out: &dyn IsoOut) {
        if !self.sending || self.cis_handles.is_empty() {
            return;
        }
        let now = out.time_ms();
        let t0 = *self.t0_ms.get_or_insert(now);
        let elapsed_ms = u64::from(now.wrapping_sub(t0));
        if self.guard.may_send(elapsed_ms * 1000) {
            self.request_events(out);
            return;
        }
        // Ahead of real time (a controller that completes SDUs early): wait
        let due_ms = self.guard.next_due_us() / 1000;
        let wait = if due_ms > elapsed_ms { due_ms - elapsed_ms } else { u64::from(self.frame_us / 1000) };
        let wait = wait.clamp(1, 1000) as u32;
        self.guard_waits += 1;
        tracing::trace!("LE Audio: send guard: {elapsed_ms} ms in, ahead of real time, waiting {wait} ms");
        self.guard_due_ms = Some(now.wrapping_add(wait));
        out.timer_start(wait);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};

    use btstack_sys::consts::{HCI_EVENT_BIS_CAN_SEND_NOW, HCI_EVENT_CIS_CAN_SEND_NOW};
    use le_audio::sdu_queue::SduQueue;

    use super::*;

    #[derive(Default)]
    struct Fake {
        now: Cell<u32>,
        log: RefCell<Vec<String>>,
    }

    impl Fake {
        fn take(&self) -> Vec<String> {
            self.log.borrow_mut().drain(..).collect()
        }
    }

    impl IsoOut for Fake {
        fn time_ms(&self) -> u32 {
            self.now.get()
        }
        fn set_queue(&self, packets: u8) {
            self.log.borrow_mut().push(format!("queue {packets}"));
        }
        fn request_can_send(&self, cis: u16) {
            self.log.borrow_mut().push(format!("request {cis:x}"));
        }
        fn request_bis_can_send(&self, big: u8) {
            self.log.borrow_mut().push(format!("request BIG {big}"));
        }
        fn send(&self, cis: u16, seq: u16, sdu: &[u8]) -> u8 {
            self.log.borrow_mut().push(format!("send {cis:x} #{seq} {:x}", sdu[0]));
            0
        }
        fn timer_start(&self, ms: u32) {
            self.log.borrow_mut().push(format!("timer {ms}"));
        }
        fn timer_stop(&self) {
            self.log.borrow_mut().push("timer stop".into());
        }
    }

    fn can_send(cis: u16, group_complete: bool) -> [u8; 8] {
        [HCI_EVENT_CIS_CAN_SEND_NOW, 6, 1, 0, cis as u8, (cis >> 8) as u8, 0, group_complete as u8]
    }

    /// As BTstack 1.8.1 emits it: without the BIS handle
    fn bis_can_send(big: u8, index: u8, _bis: u16) -> [u8; 4] {
        [HCI_EVENT_BIS_CAN_SEND_NOW, 2, big, index]
    }

    #[test]
    fn big_rounds_end_with_the_last_bis() {
        let out = Fake::default();
        let stats = Arc::new(Mutex::new(LeStreamStats::default()));
        let mut s = IsoSender::new(stats.clone());
        let q: SharedSduQueue = Arc::new(Mutex::new(SduQueue::new()));
        q.lock().unwrap().configure(2, vec![vec![0xEE; 4], vec![0xEE; 4]]);
        s.start_big(&out, 0, &[0x10, 0x11], q.clone(), 10_000, 2);
        assert_eq!(out.take(), ["queue 2", "request BIG 0"]);
        // CIS events do not reach a BIG
        s.on_cis_can_send_now(&out, &can_send(0x10, true));
        assert!(out.take().is_empty());
        // Another BIG's events neither
        s.on_bis_can_send_now(&out, &bis_can_send(1, 0, 0x10));
        assert!(out.take().is_empty());
        s.on_bis_can_send_now(&out, &bis_can_send(0, 0, 0x10));
        assert_eq!(out.take(), ["send 10 #0 ee"]);
        s.on_bis_can_send_now(&out, &bis_can_send(0, 1, 0x11));
        assert_eq!(out.take(), ["send 11 #0 ee", "request BIG 0"]);
        s.on_bis_can_send_now(&out, &bis_can_send(0, 0, 0x10));
        s.on_bis_can_send_now(&out, &bis_can_send(0, 1, 0x11));
        assert_eq!(out.take(), ["send 10 #1 ee", "send 11 #1 ee", "request BIG 0"]);
        s.stop(&out);
        assert_eq!(stats.lock().unwrap().sdus_sent, 4);
    }

    fn set(tag: u8) -> SduSet {
        vec![vec![tag; 4], vec![tag; 4]]
    }

    #[test]
    fn rounds_sequence_numbers_silence_and_guard() {
        let out = Fake::default();
        let stats = Arc::new(Mutex::new(LeStreamStats::default()));
        let mut s = IsoSender::new(stats.clone());
        let q: SharedSduQueue = Arc::new(Mutex::new(SduQueue::new()));
        q.lock().unwrap().configure(2, vec![vec![0xEE; 4], vec![0xEE; 4]]);
        s.start(&out, &[0x60, 0x61], q.clone(), 10_000, 2);
        assert_eq!(out.take(), ["queue 2", "request 60"]);
        // Prefilling (queue + 3 = 5 sets): silence
        s.on_cis_can_send_now(&out, &can_send(0x60, false));
        s.on_cis_can_send_now(&out, &can_send(0x61, true));
        assert_eq!(out.take(), ["send 60 #0 ee", "send 61 #0 ee", "request 60"]);
        for tag in 1..=5 {
            q.lock().unwrap().push(set(tag));
        }
        s.on_cis_can_send_now(&out, &can_send(0x61, false));
        s.on_cis_can_send_now(&out, &can_send(0x60, false));
        assert_eq!(out.take(), ["send 61 #1 1", "send 60 #1 1", "request 60"]);
        // A CIS asked again before the other got its SDU starts a new round
        // (v1: served twice = next round)
        s.on_cis_can_send_now(&out, &can_send(0x60, false));
        s.on_cis_can_send_now(&out, &can_send(0x60, false));
        assert_eq!(out.take(), ["send 60 #2 2", "send 60 #3 3"]);
        s.on_cis_can_send_now(&out, &can_send(0x61, true));
        // 4 rounds within the same millisecond: the guard (lead 4) waits
        assert_eq!(out.take(), ["send 61 #2 3", "timer 10"]);
        out.now.set(10);
        s.on_timer(&out);
        assert_eq!(out.take(), ["request 60"]);
        let st = *stats.lock().unwrap();
        assert_eq!((st.sets_sent, st.sdus_sent, st.silence_sets, st.underruns, st.sets_encoded), (4, 7, 1, 0, 5));
        // The right CIS goes down: rounds go on with the left one only
        s.on_cis_down(&out, 0x61);
        out.now.set(20);
        s.on_cis_can_send_now(&out, &can_send(0x60, false));
        assert_eq!(out.take(), ["send 60 #4 4", "request 60"]);
        s.stop(&out);
        assert_eq!(out.take(), ["timer stop"]);
        assert!(!s.sending());
        let st = *stats.lock().unwrap();
        assert_eq!((st.sets_sent, st.sdus_sent), (5, 8));
        assert_eq!(queue_depth(64, 2), 2);
        assert_eq!(queue_depth(3, 2), 1);
        assert_eq!(queue_depth(1, 2), 0);
    }

    #[test]
    fn underrun_sends_silence() {
        let out = Fake::default();
        let mut s = IsoSender::new(Arc::default());
        let q: SharedSduQueue = Arc::new(Mutex::new(SduQueue::new()));
        q.lock().unwrap().configure(1, vec![vec![0xEE; 4]]);
        s.start(&out, &[0x60], q.clone(), 10_000, 1);
        for tag in 1..=4 {
            q.lock().unwrap().push(vec![vec![tag; 4]]);
        }
        for _ in 0..5 {
            out.now.set(out.now.get() + 10);
            s.on_cis_can_send_now(&out, &can_send(0x60, true));
        }
        let sends: Vec<String> = out.take().into_iter().filter(|l| l.starts_with("send")).collect();
        assert_eq!(sends, ["send 60 #0 1", "send 60 #1 2", "send 60 #2 3", "send 60 #3 4", "send 60 #4 ee"]);
        let st = s.stats();
        assert_eq!((st.underruns, st.silence_sets), (1, 1));
    }
}
