//! How much audio Windows lost at a data discontinuity.
//!
//! WASAPI flags a packet as discontinuous when the engine could not hand
//! over what came before it (the capture thread was held back and the
//! buffer overran, or the audio engine itself glitched). Each packet carries
//! the device position of its first frame and the QPC time of that position,
//! so the frames lost are the position's jump past the end of the previous
//! packet. The position counts in the device's own frames (at its mix rate
//! when Windows converts), so how far it moves per captured frame is learnt
//! from the packets that follow on. The jump is trusted only when the QPC
//! time moved as far; otherwise it is not known and only the event counts.
//! Process loopback reports no position at all (always 0): its losses are
//! counted, never measured.

/// QPC time is in 100 ns units
const QPC_PER_SECOND: f64 = 10_000_000.0;
/// A jump longer than this is not a glitch the capture can tell (the system
/// slept, or the device was silent and the stream restarted)
const MAX_JUMP_SECONDS: f64 = 2.0;
/// The position and the QPC time may disagree by this much (about two
/// engine periods) plus 5 % of the time between the two packets
const TOLERANCE_SECONDS: f64 = 0.02;
const TOLERANCE_SHARE: f64 = 0.05;

#[derive(Debug, Clone, Copy)]
struct Packet {
    index: u64,
    qpc: u64,
    frames: u64,
}

/// Follows the device position from packet to packet
#[derive(Debug)]
pub(crate) struct GapCount {
    rate: u32,
    /// The last packet with a valid timestamp; None at the start and after
    /// a silent stretch (positions across it tell nothing about loss)
    prev: Option<Packet>,
    /// Device position units per captured frame, from the last two packets
    /// that followed on (1 unless Windows converts the rate)
    scale: Option<f64>,
}

impl GapCount {
    pub fn new(rate: u32) -> GapCount {
        GapCount { rate, prev: None, scale: None }
    }

    /// A packet of `frames` frames at device position `index` and QPC time
    /// `qpc`. For a discontinuous one: the captured frames lost before it,
    /// when the positions tell (None: not known, the event still counts).
    pub fn packet(
        &mut self,
        frames: u64,
        index: u64,
        qpc: u64,
        timestamp_error: bool,
        discontinuity: bool,
    ) -> Option<u64> {
        let this = Packet { index, qpc, frames };
        let prev = if timestamp_error { self.prev.take() } else { self.prev.replace(this) };
        if timestamp_error {
            return None;
        }
        let prev = prev?;
        let moved = index.checked_sub(prev.index).filter(|&d| d > 0)? as f64;
        if !discontinuity {
            // Followed on: how far the position moves per captured frame
            if prev.frames > 0 {
                self.scale = Some(moved / prev.frames as f64);
            }
            return None;
        }
        let scale = self.scale?;
        let rate = f64::from(self.rate);
        let seconds = qpc.checked_sub(prev.qpc)? as f64 / QPC_PER_SECOND;
        if rate <= 0.0 || seconds <= 0.0 {
            return None;
        }
        // Both in captured frames since the previous packet started
        let by_position = moved / scale;
        let by_qpc = seconds * rate;
        if (by_position - by_qpc).abs() > rate * TOLERANCE_SECONDS + by_qpc * TOLERANCE_SHARE {
            return None;
        }
        let lost = by_position - prev.frames as f64;
        if lost < -rate * TOLERANCE_SECONDS || lost > rate * MAX_JUMP_SECONDS {
            return None;
        }
        Some(lost.max(0.0).round() as u64)
    }

    /// The silence fill ran: the next packet's position jump is the quiet
    /// stretch, not lost audio
    pub fn reset(&mut self) {
        self.prev = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;
    /// 10 ms in captured frames and in QPC units
    const P: u64 = 480;
    const T: u64 = 100_000;

    /// Packets of 10 ms in step with the QPC clock, starting at `n` periods,
    /// the position moving `scale` per frame
    fn steady(g: &mut GapCount, from: u64, count: u64, scale: u64) {
        for n in from..from + count {
            assert_eq!(g.packet(P, n * P * scale, n * T, false, false), None);
        }
    }

    #[test]
    fn a_position_jump_is_the_frames_lost() {
        let mut g = GapCount::new(RATE);
        steady(&mut g, 0, 5, 1);
        // 30 ms lost: the next packet starts 3 periods later than expected
        assert_eq!(g.packet(P, 8 * P, 8 * T, false, true), Some(3 * P));
        // In step again
        steady(&mut g, 9, 3, 1);
        // Flagged, but the position follows on: nothing lost
        assert_eq!(g.packet(P, 12 * P, 12 * T, false, true), Some(0));
    }

    #[test]
    fn a_converted_rate_is_measured_in_captured_frames() {
        // The device mixes at 96 kHz, the capture gets 48 kHz: the position
        // moves two per captured frame
        let mut g = GapCount::new(RATE);
        steady(&mut g, 0, 5, 2);
        assert_eq!(g.packet(P, 8 * P * 2, 8 * T, false, true), Some(3 * P));
        // 44.1 kHz captured from a 48 kHz device
        let mut g = GapCount::new(44_100);
        for n in 0..5 {
            g.packet(441, n * 480, n * T, false, false);
        }
        assert_eq!(g.packet(441, 7 * 480, 7 * T, false, true), Some(2 * 441));
    }

    #[test]
    fn an_unreliable_position_is_not_counted_as_lost() {
        let mut g = GapCount::new(RATE);
        // Nothing to compare with
        assert_eq!(g.packet(P, 7 * P, 7 * T, false, true), None);
        steady(&mut g, 8, 2, 1);
        // The QPC time says 20 ms, the position 60 ms
        assert_eq!(g.packet(P, 15 * P, 11 * T, false, true), None);
        steady(&mut g, 16, 2, 1);
        // A timestamp error: neither this packet nor the next is measured
        assert_eq!(g.packet(P, 30 * P, 30 * T, true, true), None);
        assert_eq!(g.packet(P, 31 * P, 31 * T, false, true), None);
        // The position went back
        assert_eq!(g.packet(P, 20 * P, 32 * T, false, true), None);
        // No QPC time at all
        let mut g = GapCount::new(RATE);
        steady(&mut g, 0, 2, 1);
        assert_eq!(g.packet(P, 4 * P, T, false, true), None);
    }

    #[test]
    fn process_loopback_has_no_position() {
        // Its packets all say position 0
        let mut g = GapCount::new(RATE);
        for n in 0..5 {
            assert_eq!(g.packet(P, 0, n * T, false, false), None);
        }
        assert_eq!(g.packet(P, 0, 8 * T, false, true), None);
    }

    #[test]
    fn a_long_jump_or_a_silent_stretch_is_not_loss() {
        let mut g = GapCount::new(RATE);
        steady(&mut g, 0, 2, 1);
        // 3 s later: the system slept, or the stream started again
        let n = 1 + 300;
        assert_eq!(g.packet(P, n * P, n * T, false, true), None);
        // The silence fill ran between these two
        steady(&mut g, n + 1, 2, 1);
        g.reset();
        assert_eq!(g.packet(P, (n + 50) * P, (n + 50) * T, false, true), None);
        // Measured again from then on
        assert_eq!(g.packet(P, (n + 52) * P, (n + 52) * T, false, true), Some(P));
    }

    #[test]
    fn small_timing_noise_is_tolerated() {
        let mut g = GapCount::new(RATE);
        steady(&mut g, 0, 3, 1);
        // 20 ms lost, the QPC time 3 ms off
        assert_eq!(g.packet(P, 5 * P, 5 * T + 30_000, false, true), Some(2 * P));
    }
}
