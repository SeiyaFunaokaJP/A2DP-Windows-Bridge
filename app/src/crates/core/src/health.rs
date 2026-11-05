//! The stream health log: what each part of the stream path did, every few
//! seconds and once more when the stream ends.
//!
//! Issue #84: after a Disconnect / Connect a stream stuttered until the app
//! was restarted, and the log of the two sessions read the same. These lines
//! put numbers on each step, so two sessions can be compared from a log:
//!
//! - capture: audio delivered against the clock, silence the fill added
//!   (no packet for two periods), the longest wait between packets, WASAPI
//!   discontinuities and the audio lost in them, frames the encoder fell
//!   behind on
//! - encode: the longest wait between rounds, the send queue's fill and
//!   drops
//! - send: packets handed to L2CAP and the longest gap between them, the
//!   longest wait for BTstack to allow one (controller buffers full)
//! - air: the longest gap between Number Of Completed Packets events (the
//!   controller got nothing out), the most ACL packets the controller held,
//!   and the longest USB write
//! - radio: RSSI, the AFH channels in use, link quality and failed contacts
//! - process: CPU, threads (a thread left from an earlier stream shows as
//!   more threads and CPU), the system timer resolution
//!
//! The first minute of a stream logs every interval, later ones only every
//! minute or when an interval looks wrong (drops, filled silence while audio
//! plays, an air or send gap over [`GAP_WARN`]).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};

use bt_host::LinkStats;
use bt_host::stats::MEDIA_QUEUE_PACKETS;

/// One interval
const INTERVAL: Duration = Duration::from_secs(10);
/// Every interval is logged this long after a stream starts
const VERBOSE_FOR: Duration = Duration::from_secs(60);
/// Later, every this many intervals (and the unusual ones)
const QUIET_EVERY: u32 = 6;
/// A gap this long in what goes out is audible on most sinks
const GAP_WARN: Duration = Duration::from_millis(150);

/// Developer mode: every interval is logged for the whole stream
static EVERY_INTERVAL: AtomicBool = AtomicBool::new(false);

/// Log every interval of a stream (developer mode), not only its first
/// minute and the unusual ones
pub fn set_every_interval(on: bool) {
    EVERY_INTERVAL.store(on, Ordering::Relaxed);
}

/// Streams this process ran (the log says which one a line belongs to)
static STREAMS: AtomicU32 = AtomicU32::new(0);

/// What the capture says since the stream started (the caller reads its
/// [`audio_capture::Running`])
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct CaptureReading {
    pub filled_frames: u64,
    /// The longest wait between packets since the last reading
    pub packet_gap: Duration,
}

/// Counters at the start of an interval
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Counters {
    packets: u64,
    bytes: u64,
    drops: u64,
    send_errors: u64,
    capture_frames: u64,
    capture_dropped: u64,
    silence_frames: u64,
    filled_frames: u64,
    discontinuities: u64,
    lost_frames: u64,
    completed_events: u64,
    cpu: Option<Duration>,
}

/// The longest waits of an interval (or of the whole stream)
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Peaks {
    capture_gap: Duration,
    encode_gap: Duration,
    queue_depth: u64,
    send_gap: Duration,
    can_send_wait: Duration,
    completed_gap: Duration,
    usb_write: Duration,
    usb_in_flight: usize,
    controller_backlog: u64,
}

impl Peaks {
    fn max(&mut self, o: &Peaks) {
        self.capture_gap = self.capture_gap.max(o.capture_gap);
        self.encode_gap = self.encode_gap.max(o.encode_gap);
        self.queue_depth = self.queue_depth.max(o.queue_depth);
        self.send_gap = self.send_gap.max(o.send_gap);
        self.can_send_wait = self.can_send_wait.max(o.can_send_wait);
        self.completed_gap = self.completed_gap.max(o.completed_gap);
        self.usb_write = self.usb_write.max(o.usb_write);
        self.usb_in_flight = self.usb_in_flight.max(o.usb_in_flight);
        self.controller_backlog = self.controller_backlog.max(o.controller_backlog);
    }
}

/// The process side of an interval
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Process {
    /// CPU of all threads over the interval, in % of one core
    cpu_percent: Option<f64>,
    threads: Option<u32>,
    timer: Option<Duration>,
}

fn us(v: u64) -> Duration {
    Duration::from_micros(v)
}

fn ms(d: Duration) -> String {
    format!("{:.0}", d.as_secs_f64() * 1000.0)
}

pub(crate) struct StreamHealth {
    stats: Arc<LinkStats>,
    sample_rate: u32,
    stream: u32,
    started: Instant,
    interval_start: Instant,
    intervals: u32,
    first: Counters,
    prev: Counters,
    session: Peaks,
    worst_cpu: f64,
    first_threads: Option<u32>,
}

impl StreamHealth {
    /// At the start of the stream (after the capture started): what came
    /// before is not counted
    pub fn start(stats: Arc<LinkStats>, sample_rate: u32, capture: CaptureReading) -> StreamHealth {
        let stream = STREAMS.fetch_add(1, Ordering::Relaxed) + 1;
        take_peaks(&stats, capture);
        let now = Instant::now();
        let c = counters(&stats, capture);
        let p = Process {
            cpu_percent: None,
            threads: win_sys::perf::process_threads(),
            timer: win_sys::perf::timer_resolution(),
        };
        tracing::info!(
            "stream health: stream {stream} of this process starts{}",
            process_text(&p).map(|t| format!(" ({t})")).unwrap_or_default()
        );
        StreamHealth {
            stats,
            sample_rate,
            stream,
            started: now,
            interval_start: now,
            intervals: 0,
            first: c,
            prev: c,
            session: Peaks::default(),
            worst_cpu: 0.0,
            first_threads: p.threads,
        }
    }

    /// Call often (the session loop's tick); logs when an interval is over.
    /// `capture` is read only then (it takes the capture's packet gap).
    pub fn tick(&mut self, capture: impl FnOnce() -> CaptureReading) {
        let now = Instant::now();
        let elapsed = now - self.interval_start;
        if elapsed < INTERVAL {
            return;
        }
        let capture = capture();
        let peaks = take_peaks(&self.stats, capture);
        let c = counters(&self.stats, capture);
        tracing::trace!("health: counters {c:?}, peaks {peaks:?}, capture {capture:?}");
        let process = Process {
            cpu_percent: cpu_percent(self.prev.cpu, c.cpu, elapsed),
            threads: win_sys::perf::process_threads(),
            timer: win_sys::perf::timer_resolution(),
        };
        self.session.max(&peaks);
        self.worst_cpu = self.worst_cpu.max(process.cpu_percent.unwrap_or(0.0));
        self.intervals += 1;
        let d = Delta::between(&self.prev, &c);
        let unusual = d.unusual(&peaks);
        let verbose = now - self.started <= VERBOSE_FOR + INTERVAL / 2
            || self.intervals.is_multiple_of(QUIET_EVERY)
            || EVERY_INTERVAL.load(Ordering::Relaxed);
        if unusual || verbose {
            let line = self.line(elapsed, &d, &peaks, Some(&process));
            if unusual {
                tracing::warn!("stream health {}: {line}", self.stream);
            } else {
                tracing::info!("stream health {}: {line}", self.stream);
            }
        } else {
            tracing::debug!(
                "health interval {} of stream {}: {}",
                self.intervals,
                self.stream,
                self.line(elapsed, &d, &peaks, Some(&process))
            );
        }
        self.prev = c;
        self.interval_start = now;
    }

    /// The stream ends: the whole of it, worst values
    pub fn finish(mut self, capture: CaptureReading) {
        let peaks = take_peaks(&self.stats, capture);
        self.session.max(&peaks);
        let c = counters(&self.stats, capture);
        let elapsed = self.started.elapsed();
        let d = Delta::between(&self.first, &c);
        let threads = win_sys::perf::process_threads();
        let line = self.line(elapsed, &d, &self.session.clone(), None);
        let mut process = Vec::new();
        if let Some(c) = cpu_percent(self.first.cpu, c.cpu, elapsed) {
            process.push(format!("CPU {c:.1}% (worst interval {:.1}%)", self.worst_cpu));
        }
        if let (Some(a), Some(b)) = (self.first_threads, threads) {
            process.push(format!("threads {a} -> {b}"));
        }
        let link = self.stats.link();
        let link = crate::link::link_state_text(&link)
            .map(|t| {
                format!(" | link: {t}, {} role change(s), {} sniff entries", link.role_changes, link.sniff_entries)
            })
            .unwrap_or_default();
        tracing::info!("stream health {} summary: {line}{link} | process: {}", self.stream, process.join(", "));
    }

    fn line(&self, elapsed: Duration, d: &Delta, p: &Peaks, process: Option<&Process>) -> String {
        let secs = elapsed.as_secs_f64().max(0.001);
        let expected = self.sample_rate as f64 * secs;
        let audio_pct = d.capture_frames as f64 * 100.0 / expected;
        let silence_pct = d.silence_frames as f64 * 100.0 / expected;
        let radio = self.stats.radio();
        let mut s = format!(
            "{secs:.1} s | capture: audio {audio_pct:.1}%, silence {silence_pct:.1}% (filled {} frames), \
             packet gaps <= {} ms, discontinuities {} ({} ms lost), behind {} frames | encode: gaps <= {} ms, queue <= {}/{}, \
             drops {} | send: {} pkt ({:.1}/s, {:.0} kbps), gaps <= {} ms, can-send waits <= {} ms, errors {} | \
             air: completed gaps <= {} ms ({} events), controller backlog <= {} ACL packets, USB writes <= {} ms \
             ({} in flight)",
            d.filled_frames,
            ms(p.capture_gap),
            d.discontinuities,
            ms(Duration::from_secs_f64(d.lost_frames as f64 / f64::from(self.sample_rate.max(1)))),
            d.capture_dropped,
            ms(p.encode_gap),
            p.queue_depth,
            MEDIA_QUEUE_PACKETS,
            d.drops,
            d.packets,
            d.packets as f64 / secs,
            d.bytes as f64 * 8.0 / secs / 1000.0,
            ms(p.send_gap),
            ms(p.can_send_wait),
            d.send_errors,
            ms(p.completed_gap),
            d.completed_events,
            p.controller_backlog,
            ms(p.usb_write),
            p.usb_in_flight,
        );
        let mut radio_parts = Vec::new();
        if let Some(r) = radio.rssi {
            radio_parts.push(format!("RSSI {r}"));
        }
        if let Some(n) = radio.afh_in_use {
            radio_parts.push(format!("AFH {n}/79 in use"));
        }
        if !radio.afh_avoided.is_empty() {
            radio_parts.push(format!("avoiding Wi-Fi {}", radio.afh_avoided));
        }
        let link = self.stats.link();
        if let Some(q) = link.link_quality {
            radio_parts.push(format!("link quality {q}"));
        }
        if link.failed_contacts.is_some() {
            radio_parts.push(format!("failed contacts {} this stream", link.failed_contacts_stream));
        }
        if !radio_parts.is_empty() {
            s += &format!(" | radio: {}", radio_parts.join(", "));
        }
        if let Some(t) = process.and_then(process_text) {
            s += &format!(" | process: {t}");
        }
        s
    }
}

fn process_text(p: &Process) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(c) = p.cpu_percent {
        parts.push(format!("CPU {c:.1}%"));
    }
    if let Some(t) = p.threads {
        parts.push(format!("{t} threads"));
    }
    if let Some(t) = p.timer {
        parts.push(format!("timer {:.1} ms", t.as_secs_f64() * 1000.0));
    }
    (!parts.is_empty()).then(|| parts.join(", "))
}

fn cpu_percent(before: Option<Duration>, now: Option<Duration>, wall: Duration) -> Option<f64> {
    let (b, n) = (before?, now?);
    (wall > Duration::ZERO).then(|| n.saturating_sub(b).as_secs_f64() * 100.0 / wall.as_secs_f64())
}

fn counters(s: &LinkStats, capture: CaptureReading) -> Counters {
    let r = Ordering::Relaxed;
    Counters {
        packets: s.packets_sent.load(r),
        bytes: s.bytes_sent.load(r),
        drops: s.queue_drops.load(r),
        send_errors: s.send_errors.load(r),
        capture_frames: s.capture_frames.load(r),
        capture_dropped: s.capture_dropped_frames.load(r),
        silence_frames: s.silence_frames.load(r),
        filled_frames: capture.filled_frames,
        discontinuities: s.capture_discontinuities.load(r),
        lost_frames: s.capture_lost_frames.load(r),
        completed_events: s.timing.completed_events.load(r),
        cpu: win_sys::perf::process_cpu_time(),
    }
}

fn take_peaks(s: &LinkStats, capture: CaptureReading) -> Peaks {
    let t = &s.timing;
    let (usb_write, usb_in_flight) = hci_transport::usb::take_acl_out_timing();
    Peaks {
        capture_gap: capture.packet_gap,
        encode_gap: us(t.encode_gap_us.take()),
        queue_depth: t.queue_depth_max.take(),
        send_gap: us(t.send_gap_us.take()),
        can_send_wait: us(t.can_send_wait_us.take()),
        completed_gap: us(t.completed_gap_us.take()),
        usb_write,
        usb_in_flight,
        controller_backlog: t.controller_backlog_max.take(),
    }
}

/// What changed over an interval
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Delta {
    packets: u64,
    bytes: u64,
    drops: u64,
    send_errors: u64,
    capture_frames: u64,
    capture_dropped: u64,
    silence_frames: u64,
    filled_frames: u64,
    discontinuities: u64,
    lost_frames: u64,
    completed_events: u64,
}

impl Delta {
    fn between(a: &Counters, b: &Counters) -> Delta {
        Delta {
            packets: b.packets.saturating_sub(a.packets),
            bytes: b.bytes.saturating_sub(a.bytes),
            drops: b.drops.saturating_sub(a.drops),
            send_errors: b.send_errors.saturating_sub(a.send_errors),
            capture_frames: b.capture_frames.saturating_sub(a.capture_frames),
            capture_dropped: b.capture_dropped.saturating_sub(a.capture_dropped),
            silence_frames: b.silence_frames.saturating_sub(a.silence_frames),
            // A new capture (an application that started again) counts
            // from 0
            filled_frames: b.filled_frames.checked_sub(a.filled_frames).unwrap_or(b.filled_frames),
            discontinuities: b.discontinuities.saturating_sub(a.discontinuities),
            lost_frames: b.lost_frames.saturating_sub(a.lost_frames),
            completed_events: b.completed_events.saturating_sub(a.completed_events),
        }
    }

    /// Something a listener may have heard: audio lost before the radio,
    /// silence filled in while audio played, or a long gap in what went
    /// out while packets were sent
    fn unusual(&self, p: &Peaks) -> bool {
        let audio_playing = self.capture_frames > 0;
        self.drops > 0
            || self.send_errors > 0
            || self.capture_dropped > 0
            || (audio_playing && self.filled_frames > 0)
            || (self.packets > 0 && (p.completed_gap >= GAP_WARN || p.send_gap >= GAP_WARN))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deltas_between_counters() {
        let a = Counters { packets: 10, filled_frames: 5, discontinuities: 2, lost_frames: 480, ..Default::default() };
        let b = Counters { packets: 25, filled_frames: 9, discontinuities: 3, lost_frames: 1440, ..Default::default() };
        let d = Delta::between(&a, &b);
        assert_eq!(d.packets, 15);
        assert_eq!(d.filled_frames, 4);
        assert_eq!((d.discontinuities, d.lost_frames), (1, 960));
        // The capture started anew
        let c = Counters { filled_frames: 2, ..b };
        assert_eq!(Delta::between(&b, &c).filled_frames, 2);
    }

    #[test]
    fn what_counts_as_unusual() {
        let quiet = Peaks::default();
        let playing = Delta { capture_frames: 480_000, packets: 500, ..Default::default() };
        assert!(!playing.unusual(&quiet));
        assert!(Delta { drops: 1, ..playing }.unusual(&quiet));
        assert!(Delta { filled_frames: 480, ..playing }.unusual(&quiet));
        // Silence filled while nothing plays is what the fill is for
        assert!(!Delta { capture_frames: 0, filled_frames: 480_000, ..playing }.unusual(&quiet));
        let gap = Peaks { completed_gap: Duration::from_millis(200), ..Default::default() };
        assert!(playing.unusual(&gap));
        assert!(!Delta { packets: 0, ..playing }.unusual(&gap));
    }

    #[test]
    fn cpu_over_an_interval() {
        let s = Duration::from_secs;
        assert_eq!(cpu_percent(Some(s(1)), Some(s(2)), s(10)), Some(10.0));
        assert_eq!(cpu_percent(None, Some(s(2)), s(10)), None);
    }

    #[test]
    fn peaks_keep_the_worst() {
        let mut a = Peaks { send_gap: Duration::from_millis(20), queue_depth: 3, ..Default::default() };
        a.max(&Peaks { send_gap: Duration::from_millis(10), queue_depth: 9, ..Default::default() });
        assert_eq!((a.send_gap, a.queue_depth), (Duration::from_millis(20), 9));
    }
}
