//! Silence fill timing.
//!
//! Loopback capture of a device that plays nothing delivers nothing: no
//! packets, often not even the buffer event. The A2DP stream must keep
//! flowing all the same (a sink that hears nothing may drop the link or
//! re-buffer when audio returns), so with `fill_silence` the capture thread
//! asks this clock how much silence is due whenever it wakes. Time is passed
//! in (as the offset from the stream start) so the logic is testable.

use std::time::Duration;

#[derive(Debug)]
pub(crate) struct SilenceFill {
    sample_rate: u32,
    /// No packet for this long = the device is silent
    threshold: Duration,
    /// When the last captured packet arrived (or the stream started)
    last_audio: Duration,
    /// Start of the current silent stretch; None while audio flows
    silent_since: Option<Duration>,
    /// Frames of silence delivered in the current stretch
    emitted: u64,
}

/// At most this much silence in one go. A larger gap means the thread did
/// not run at all (system sleep, debugger): delivering it in full would
/// flood the pipeline with seconds of stale silence, so the excess is
/// dropped and the clock moves on.
const MAX_BURST: Duration = Duration::from_millis(200);

/// The shortest wait before a device counts as silent. Loopback packets
/// come a period apart only on average: a missed wake-up delivers two
/// periods at once 20 ms after the last (VB-Audio Cable at 10 ms periods
/// did so every few seconds), and a fill at two periods then spliced 20 ms
/// of silence into the music, heard as crackles and skips. The pipeline
/// sends what arrives, so a later start of the fill only delays the
/// silence, not the audio.
const MIN_THRESHOLD: Duration = Duration::from_millis(50);

impl SilenceFill {
    /// `period`: the stream's device period; silence starts after two of
    /// them without a packet, and not before [`MIN_THRESHOLD`]. `now`: the
    /// stream start.
    pub fn new(sample_rate: u32, period: Duration, now: Duration) -> SilenceFill {
        let threshold = (period * 2).max(MIN_THRESHOLD);
        SilenceFill { sample_rate, threshold, last_audio: now, silent_since: None, emitted: 0 }
    }

    /// A captured packet arrived at `now`
    pub fn on_audio(&mut self, now: Duration) {
        self.last_audio = now;
        self.silent_since = None;
        self.emitted = 0;
    }

    /// Frames of silence due at `now`: nothing while packets keep arriving,
    /// then, once none has come for the threshold, the whole time since the
    /// last one (so the delivered stream keeps real-time length), and from
    /// then on whatever has elapsed since the previous call.
    pub fn poll(&mut self, now: Duration) -> usize {
        if now.saturating_sub(self.last_audio) < self.threshold {
            return 0;
        }
        let since = *self.silent_since.get_or_insert(self.last_audio);
        let due = frames(now.saturating_sub(since), self.sample_rate);
        let n = due.saturating_sub(self.emitted);
        let max = frames(MAX_BURST, self.sample_rate);
        if n > max {
            // Resynchronise: count everything up to `now` as delivered
            tracing::debug!(
                "silence fill: {} frames due at once, delivering {max} and dropping the rest (thread stalled?)",
                n
            );
            self.emitted = due;
            return max as usize;
        }
        self.emitted = due;
        n as usize
    }
}

fn frames(d: Duration, rate: u32) -> u64 {
    (d.as_nanos() * rate as u128 / 1_000_000_000) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: Duration = Duration::from_millis(1);
    const PERIOD: Duration = Duration::from_millis(10);

    #[test]
    fn nothing_while_audio_flows() {
        let mut f = SilenceFill::new(48_000, PERIOD, Duration::ZERO);
        for i in 1..100u32 {
            let now = PERIOD * i;
            f.on_audio(now);
            assert_eq!(f.poll(now + 5 * MS), 0);
        }
    }

    #[test]
    fn nothing_before_the_threshold() {
        let mut f = SilenceFill::new(48_000, PERIOD, Duration::ZERO);
        assert_eq!(f.poll(10 * MS), 0);
        assert_eq!(f.poll(49 * MS), 0);
        // Long periods: two of them
        let mut f = SilenceFill::new(48_000, 40 * MS, Duration::ZERO);
        assert_eq!(f.poll(79 * MS), 0);
        assert_eq!(f.poll(80 * MS), 3840);
    }

    #[test]
    fn late_packets_are_not_silence() {
        // 10 ms periods, packets now and then two at once 20-30 ms after the
        // previous (a missed wake-up), polled every 5 ms: never a fill
        let mut f = SilenceFill::new(48_000, PERIOD, Duration::ZERO);
        let mut now = Duration::ZERO;
        for i in 0..1000u32 {
            let gap = match i % 7 {
                3 => 20 * MS,
                5 => 30 * MS,
                _ => PERIOD,
            };
            let next = now + gap;
            while now < next {
                now += 5 * MS;
                assert_eq!(f.poll(now), 0, "at {now:?}");
            }
            f.on_audio(now);
        }
    }

    #[test]
    fn fills_elapsed_time_from_last_packet() {
        let mut f = SilenceFill::new(48_000, PERIOD, Duration::ZERO);
        f.on_audio(100 * MS);
        assert_eq!(f.poll(140 * MS), 0);
        // 50 ms after the last packet: the whole 50 ms at once
        assert_eq!(f.poll(150 * MS), 2400);
        // then whatever elapsed since the previous poll
        assert_eq!(f.poll(160 * MS), 480);
        assert_eq!(f.poll(165 * MS), 240);
        assert_eq!(f.poll(165 * MS), 0);
    }

    #[test]
    fn total_matches_real_time_without_drift() {
        // Wake-ups at odd intervals over 10 s of silence: the frames add up
        // to exactly the elapsed time (no rounding drift), less the part
        // before the threshold, which arrives in the first fill.
        let rate = 44_100;
        let mut f = SilenceFill::new(rate, PERIOD, Duration::ZERO);
        let mut total = 0u64;
        let mut now = Duration::ZERO;
        let mut step = 3u64;
        while now < Duration::from_secs(10) {
            step = step * 7 % 13 + 1; // 1..13 ms
            now += Duration::from_millis(step);
            total += f.poll(now) as u64;
        }
        assert_eq!(total, frames(now, rate));
    }

    #[test]
    fn audio_resets_the_stretch() {
        let mut f = SilenceFill::new(48_000, PERIOD, Duration::ZERO);
        assert_eq!(f.poll(60 * MS), 2880);
        f.on_audio(70 * MS);
        assert_eq!(f.poll(110 * MS), 0);
        assert_eq!(f.poll(120 * MS), 2400);
    }

    #[test]
    fn long_gap_is_capped() {
        let mut f = SilenceFill::new(48_000, PERIOD, Duration::ZERO);
        // The thread did not run for 5 s
        assert_eq!(f.poll(Duration::from_secs(5)), 9600);
        // and resumes normally afterwards
        assert_eq!(f.poll(Duration::from_secs(5) + 10 * MS), 480);
    }
}
