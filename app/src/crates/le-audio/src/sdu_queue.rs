//! The jitter buffer of SDU sets and the send guard (v1 Stream in
//! le_audio_stream.cpp).
//!
//! One SDU set is one SDU per CIS for the same frame interval (10 or 7.5
//! ms), in plan order. The encode thread (pipeline) pushes sets; the
//! BTstack thread (bt-host iso_send) pops one per round on
//! HCI_EVENT_CIS_CAN_SEND_NOW. bt-host shares the queue between the two
//! behind a mutex (`LeSduSink`).
//!
//! Buffering: before audio starts and after an underrun the queue holds
//! sets back until it has `prefill` of them: queue depth + 3 at first, one
//! more after each underrun up to queue depth + 7. A full queue
//! ([`CAPACITY`]) drops the oldest set. While nothing can be popped the
//! sender sends the encoded silence set ([`SduQueue::silence`]).
//!
//! The guard keeps the sent sets at most queue depth + 2 intervals ahead
//! of real time (the controller queue takes queue-depth sets at once).
//!
//! No allocation per round: [`SduQueue::configure`] reserves the queue and
//! a pool of spare sets shaped like the silence set. The encode thread
//! takes a spare set ([`SduQueue::spare`]) to encode into, the sender gives
//! each set back once it is sent ([`SduQueue::recycle`]), and a set dropped
//! from a full queue goes back to the pool too.
//!
//! The log lines v1 printed for the queue are built here, verbatim, so the
//! emulator tests' parsing stays valid: [`Underrun`] ("LE Audio: no audio
//! for SDU set N ..."), [`SendCounters::stopped_message`] ("LE Audio:
//! stopped sending: ...") and [`SendCounters::stats_message`] ("LE Audio: N
//! SDU sets sent ...").

use std::collections::VecDeque;
use std::fmt;

/// One SDU per CIS, in plan order
pub type SduSet = Vec<Vec<u8>>;

/// Sets the queue holds at most (the oldest is dropped beyond)
pub const CAPACITY: usize = 16;

/// Underruns that are logged (v1: the first 10)
pub const LOGGED_UNDERRUNS: u64 = 10;

/// Prefill on top of the controller queue depth at the start (v1:
/// queue_depth + 3), and how far it may grow after underruns (+ 4 more)
const PREFILL_EXTRA: usize = 3;
const PREFILL_GROWTH: usize = 4;

/// Spare sets kept beyond [`CAPACITY`]: the one being encoded, the one
/// being sent and the one the sender holds back for a CIS that is late
const SPARE_EXTRA: usize = 4;

/// Counters of the queue (part of bt-host's LeStreamStats)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct QueueStats {
    /// Sets pushed (from captured audio)
    pub sets_pushed: u64,
    /// Sets popped (audio, not silence)
    pub sets_popped: u64,
    /// Pops answered with silence (buffer empty or still prefilling)
    pub silence_sets: u64,
    /// Times the buffer ran empty while audio flowed
    pub underruns: u64,
    /// Sets dropped because the buffer was full
    pub overflows: u64,
}

/// An underrun to log (the first [`LOGGED_UNDERRUNS`] of a stream). Its
/// Display is v1's warning: "LE Audio: no audio for SDU set N (underrun U),
/// sending silence; buffering P sets"
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Underrun {
    /// The set (round) that went out as silence, counted from 0
    pub set: u64,
    /// Underruns so far, this one included
    pub underruns: u64,
    /// Sets buffered from now on before audio resumes
    pub prefill: usize,
}

impl fmt::Display for Underrun {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "LE Audio: no audio for SDU set {} (underrun {}), sending silence; buffering {} sets",
            self.set, self.underruns, self.prefill
        )
    }
}

/// The sender's totals for the log (v1 Stream::Stats; bt-host's
/// LeStreamStats has the same fields)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SendCounters {
    /// SDU sets from captured audio
    pub sets_encoded: u64,
    /// SDU sets sent (audio + silence)
    pub sets_sent: u64,
    pub sdus_sent: u64,
    /// Sets sent while the buffer was empty
    pub silence_sets: u64,
    /// Times the buffer ran empty
    pub underruns: u64,
    /// Sets dropped, buffer full
    pub overflows: u64,
    pub send_errors: u64,
}

impl SendCounters {
    /// v1 Stream::stop_sending: "LE Audio: stopped sending: N SDU sets (M
    /// SDUs), S silence, U underruns, D dropped"
    pub fn stopped_message(&self) -> String {
        format!(
            "LE Audio: stopped sending: {} SDU sets ({} SDUs), {} silence, {} underruns, {} dropped",
            self.sets_sent, self.sdus_sent, self.silence_sets, self.underruns, self.overflows
        )
    }

    /// v1 le_audio_cli print_stats: "LE Audio: N SDU sets sent (M SDUs), E
    /// encoded, S silence, U underruns, D dropped, X send errors"
    pub fn stats_message(&self) -> String {
        format!(
            "LE Audio: {} SDU sets sent ({} SDUs), {} encoded, {} silence, {} underruns, {} dropped, {} send errors",
            self.sets_sent,
            self.sdus_sent,
            self.sets_encoded,
            self.silence_sets,
            self.underruns,
            self.overflows,
            self.send_errors
        )
    }
}

/// The jitter buffer
#[derive(Debug, Clone, Default)]
pub struct SduQueue {
    sets: VecDeque<SduSet>,
    silence: SduSet,
    buffering: bool,
    prefill: usize,
    prefill_max: usize,
    stats: QueueStats,
    /// Sets to encode into (no allocation per round)
    spares: Vec<SduSet>,
    /// The last underrun not yet taken by the log
    underrun: Option<Underrun>,
}

impl SduQueue {
    pub fn new() -> SduQueue {
        SduQueue { buffering: true, ..SduQueue::default() }
    }

    /// For a new stream: empties the queue, resets the counters, sets the
    /// silence set (one encoded silent SDU per CIS) and the prefill for the
    /// controller queue depth (SDUs queued per CIS)
    pub fn configure(&mut self, queue_depth: u8, silence: SduSet) {
        self.sets.clear();
        self.sets.reserve(CAPACITY);
        self.spares.clear();
        self.spares.reserve(CAPACITY + SPARE_EXTRA);
        self.spares.extend(std::iter::repeat_n(&silence, CAPACITY + SPARE_EXTRA).cloned());
        self.silence = silence;
        self.buffering = true;
        // The controller queue takes queue_depth SDU sets at once and the
        // host may run queue_depth + 2 sets ahead of real time: buffer more
        // than that
        self.prefill = usize::from(queue_depth) + PREFILL_EXTRA;
        self.prefill_max = self.prefill + PREFILL_GROWTH;
        self.stats = QueueStats::default();
        self.underrun = None;
        tracing::debug!(
            "SDU queue: {} CIS, controller queue depth {queue_depth}, prefill {} (max {})",
            self.silence.len(),
            self.prefill,
            self.prefill_max
        );
    }

    /// Encode thread: a new set (drops the oldest when full)
    pub fn push(&mut self, set: SduSet) {
        self.stats.sets_pushed += 1;
        if self.sets.len() >= CAPACITY
            && let Some(old) = self.sets.pop_front()
        {
            self.stats.overflows += 1;
            tracing::trace!("SDU queue full: oldest set dropped ({} overflows)", self.stats.overflows);
            self.recycle(old);
        }
        self.sets.push_back(set);
    }

    /// BTstack thread: the next set, or None while buffering / empty (an
    /// underrun when audio was flowing: the prefill grows by one up to its
    /// maximum). The caller sends [`SduQueue::silence`] for None.
    pub fn pop(&mut self) -> Option<SduSet> {
        if self.buffering && self.sets.len() >= self.prefill {
            tracing::trace!("SDU queue: prefill of {} reached, sending", self.prefill);
            self.buffering = false;
        }
        if !self.buffering
            && let Some(set) = self.sets.pop_front()
        {
            self.stats.sets_popped += 1;
            return Some(set);
        }
        if !self.buffering {
            self.buffering = true;
            self.stats.underruns += 1;
            // The audio source jitters more than the buffer covers: buffer
            // one more set from now on (bounded)
            if self.prefill < self.prefill_max {
                self.prefill += 1;
            }
            tracing::trace!("SDU queue underrun {}: buffering again, prefill {}", self.stats.underruns, self.prefill);
            if self.stats.underruns <= LOGGED_UNDERRUNS {
                self.underrun = Some(Underrun {
                    set: self.stats.sets_popped + self.stats.silence_sets,
                    underruns: self.stats.underruns,
                    prefill: self.prefill,
                });
            }
        }
        self.stats.silence_sets += 1;
        None
    }

    /// The underrun the last pop ran into, for the log (Some once per
    /// underrun, for the first [`LOGGED_UNDERRUNS`]); log it with its
    /// Display as a warning
    pub fn take_underrun(&mut self) -> Option<Underrun> {
        self.underrun.take()
    }

    pub fn silence(&self) -> &SduSet {
        &self.silence
    }

    /// Encode thread: a set to encode the next frame into (same shape as
    /// the silence set; its contents are stale), None when the pool is
    /// empty (the encoder then allocates one)
    pub fn spare(&mut self) -> Option<SduSet> {
        self.spares.pop()
    }

    /// BTstack thread: a set that has been sent goes back to the pool
    /// (dropped if the pool is full or the set has another shape, e.g. from
    /// before a reconfiguration)
    pub fn recycle(&mut self, set: SduSet) {
        let same_shape =
            set.len() == self.silence.len() && set.iter().zip(&self.silence).all(|(a, b)| a.len() == b.len());
        if same_shape && self.spares.len() < self.spares.capacity() {
            self.spares.push(set);
        }
    }

    /// Sets waiting
    pub fn len(&self) -> usize {
        self.sets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sets.is_empty()
    }

    /// Sets held back before audio (re)starts
    pub fn prefill(&self) -> usize {
        self.prefill
    }

    /// Releases everything of the stream (v1 Stream::clear): queued and
    /// spare sets, the silence set and the counters
    pub fn clear(&mut self) {
        *self = SduQueue::new();
    }

    pub fn stats(&self) -> QueueStats {
        self.stats
    }
}

/// Keeps the sending at most `lead` intervals ahead of real time (v1
/// Stream's rounds_ / lead_ / t0_ms_)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SendGuard {
    frame_us: u32,
    lead: u64,
    rounds: u64,
}

impl SendGuard {
    /// `lead` = queue depth + 2
    pub fn new(frame_us: u32, lead: u64) -> SendGuard {
        SendGuard { frame_us: frame_us.max(1), lead, rounds: 0 }
    }

    /// May the next round start `elapsed_us` after the first one?
    pub fn may_send(&self, elapsed_us: u64) -> bool {
        let elapsed_rounds = elapsed_us / u64::from(self.frame_us);
        self.rounds < elapsed_rounds + self.lead
    }

    /// When the next round may start (microseconds after the first one)
    pub fn next_due_us(&self) -> u64 {
        (self.rounds + 1).saturating_sub(self.lead) * u64::from(self.frame_us)
    }

    /// A round (one set to every CIS still up) was started
    pub fn round_started(&mut self) {
        self.rounds += 1;
    }

    /// Rounds started
    pub fn rounds(&self) -> u64 {
        self.rounds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(tag: u8) -> SduSet {
        vec![vec![tag; 4], vec![tag; 4]]
    }

    fn configured(queue_depth: u8) -> SduQueue {
        let mut q = SduQueue::new();
        q.configure(queue_depth, vec![vec![0xEE; 4], vec![0xEE; 4]]);
        q
    }

    #[test]
    fn audio_starts_once_queue_depth_plus_three_sets_are_queued() {
        let mut q = configured(2);
        assert_eq!(q.prefill(), 5);
        for tag in 0..4 {
            q.push(set(tag));
            assert_eq!(q.pop(), None, "still prefilling");
        }
        // Silence while prefilling is no underrun
        assert_eq!(q.stats().silence_sets, 4);
        assert_eq!(q.stats().underruns, 0);
        assert_eq!(q.take_underrun(), None);
        q.push(set(4));
        assert_eq!(q.pop(), Some(set(0)));
        // Once flowing, a non-empty queue is served without prefill
        for tag in 1..5 {
            assert_eq!(q.pop(), Some(set(tag)));
        }
        assert!(q.is_empty());
        assert_eq!(q.stats().sets_popped, 5);
        assert_eq!(q.stats().sets_pushed, 5);
    }

    #[test]
    fn underrun_sends_silence_and_grows_prefill_up_to_plus_seven() {
        let mut q = configured(2);
        let mut underruns = 0;
        for round in 0..6u64 {
            let prefill = q.prefill();
            for tag in 0..prefill {
                q.push(set(tag as u8));
            }
            for tag in 0..prefill {
                assert_eq!(q.pop(), Some(set(tag as u8)));
            }
            assert_eq!(q.pop(), None, "underrun {round}");
            underruns += 1;
            assert_eq!(q.stats().underruns, underruns);
            assert_eq!(q.prefill(), (2 + 3 + underruns as usize).min(2 + 7));
        }
        assert_eq!(q.prefill(), 9);
        // Buffering again: silence without counting another underrun
        assert_eq!(q.pop(), None);
        assert_eq!(q.stats().underruns, underruns);
        assert_eq!(q.silence(), &vec![vec![0xEE; 4], vec![0xEE; 4]]);
    }

    #[test]
    fn full_queue_drops_the_oldest() {
        let mut q = configured(2);
        for tag in 0..CAPACITY as u8 + 3 {
            q.push(set(tag));
        }
        assert_eq!(q.len(), CAPACITY);
        assert_eq!(q.stats().overflows, 3);
        assert_eq!(q.stats().sets_pushed, CAPACITY as u64 + 3);
        assert_eq!(q.pop(), Some(set(3)));
    }

    #[test]
    fn underrun_log_lines_are_v1s() {
        let mut q = configured(2);
        for tag in 0..5 {
            q.push(set(tag));
        }
        for _ in 0..5 {
            assert!(q.pop().is_some());
        }
        assert_eq!(q.pop(), None);
        let u = q.take_underrun().unwrap();
        assert_eq!(u, Underrun { set: 5, underruns: 1, prefill: 6 });
        assert_eq!(u.to_string(), "LE Audio: no audio for SDU set 5 (underrun 1), sending silence; buffering 6 sets");
        assert_eq!(q.take_underrun(), None);
        // Silence while buffering counts as rounds too
        for _ in 0..3 {
            assert_eq!(q.pop(), None);
        }
        assert_eq!(q.take_underrun(), None);
        for tag in 0..6 {
            q.push(set(tag));
        }
        for _ in 0..6 {
            assert!(q.pop().is_some());
        }
        assert_eq!(q.pop(), None);
        assert_eq!(
            q.take_underrun().unwrap().to_string(),
            "LE Audio: no audio for SDU set 15 (underrun 2), sending silence; buffering 7 sets"
        );
    }

    #[test]
    fn only_the_first_ten_underruns_are_logged() {
        let mut q = configured(2);
        let mut logged = 0;
        for _ in 0..15 {
            let prefill = q.prefill();
            for tag in 0..prefill {
                q.push(set(tag as u8));
            }
            for _ in 0..prefill {
                assert!(q.pop().is_some());
            }
            assert_eq!(q.pop(), None);
            logged += usize::from(q.take_underrun().is_some());
        }
        assert_eq!(q.stats().underruns, 15);
        assert_eq!(logged, 10);
    }

    #[test]
    fn counters_messages_are_v1s() {
        let c = SendCounters {
            sets_encoded: 990,
            sets_sent: 1000,
            sdus_sent: 2000,
            silence_sets: 12,
            underruns: 1,
            overflows: 3,
            send_errors: 4,
        };
        assert_eq!(
            c.stopped_message(),
            "LE Audio: stopped sending: 1000 SDU sets (2000 SDUs), 12 silence, 1 underruns, 3 dropped"
        );
        assert_eq!(
            c.stats_message(),
            "LE Audio: 1000 SDU sets sent (2000 SDUs), 990 encoded, 12 silence, 1 underruns, 3 dropped, 4 send errors"
        );
    }

    #[test]
    fn spares_recycle_without_growing() {
        let mut q = configured(2);
        let pool = CAPACITY + SPARE_EXTRA;
        let mut taken: Vec<SduSet> = (0..pool).map(|_| q.spare().unwrap()).collect();
        assert_eq!(q.spare(), None);
        assert_eq!(&taken[0], q.silence());
        for s in taken.drain(..) {
            q.recycle(s);
        }
        // Full pool: one more is dropped; other shapes are not taken
        q.recycle(set(1));
        q.recycle(vec![vec![0; 4]]);
        let mut n = 0;
        while q.spare().is_some() {
            n += 1;
        }
        assert_eq!(n, pool);
        // A set dropped from a full queue goes back to the pool
        for tag in 0..=CAPACITY as u8 {
            q.push(set(tag));
        }
        assert_eq!(q.spare(), Some(set(0)));
    }

    #[test]
    fn configure_and_clear_reset() {
        let mut q = configured(2);
        q.push(set(1));
        q.pop();
        q.configure(1, vec![vec![0; 2]]);
        assert_eq!((q.len(), q.prefill(), q.stats()), (0, 4, QueueStats::default()));
        q.clear();
        assert!(q.is_empty());
        assert!(q.silence().is_empty());
        assert_eq!(q.spare(), None);
    }

    #[test]
    fn guard_keeps_lead_intervals_ahead() {
        // Queue depth 2: lead 4, 10 ms frames
        let mut g = SendGuard::new(10000, 4);
        for _ in 0..4 {
            assert!(g.may_send(0));
            g.round_started();
        }
        assert_eq!(g.rounds(), 4);
        assert!(!g.may_send(0));
        assert!(!g.may_send(9999));
        assert_eq!(g.next_due_us(), 10000);
        assert!(g.may_send(10000));
        g.round_started();
        assert_eq!(g.next_due_us(), 20000);
        assert!(!g.may_send(19999));
        assert!(g.may_send(20000));
        // Behind real time: rounds may catch up at once
        assert!(g.may_send(100_000));
        let mut g = SendGuard::new(7500, 4);
        for _ in 0..5 {
            g.round_started();
        }
        assert_eq!(g.next_due_us(), 15000);
        assert!(!g.may_send(14999));
        assert!(g.may_send(15000));
        assert_eq!(SendGuard::new(10000, 4).next_due_us(), 0);
    }
}
