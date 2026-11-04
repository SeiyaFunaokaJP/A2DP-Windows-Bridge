//! Link statistics: cumulative counters written by the audio threads and the
//! BTstack thread without locks, read a few times a second by core (which
//! works with deltas, so nothing is ever reset).

use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

/// Capacity of the media send queue, in packets (as v1)
pub const MEDIA_QUEUE_PACKETS: usize = 64;
/// Longest a media packet waits in the send queue: what the 64 packets hold
/// at LDAC's 187 packets a second, so a stall the headphones' own buffer
/// rides out stays one. Audio older than this comes too late to play.
/// Developer mode may set another ([`LinkStats::queue_max_wait_ms`]).
pub const MEDIA_QUEUE_MAX_WAIT: Duration = Duration::from_millis(350);

#[derive(Debug, Default)]
pub struct LinkStats {
    /// Media packets handed to L2CAP and their L2CAP payload bytes (RTP
    /// header, media payload header and codec data)
    pub packets_sent: AtomicU64,
    pub bytes_sent: AtomicU64,
    /// Packets lost before the radio: waited too long in the send queue or
    /// found it full / L2CAP refused
    pub queue_drops: AtomicU64,
    pub send_errors: AtomicU64,
    /// PCM frames captured from Windows (real audio, not silence fill), and
    /// those dropped because the encoder fell behind
    pub capture_frames: AtomicU64,
    pub capture_dropped_frames: AtomicU64,
    /// Silence frames inserted while nothing played
    pub silence_frames: AtomicU64,
    /// Instantaneous send queue fill (packets)
    pub queue_depth: AtomicU32,
    /// How long the oldest packet in the send queue has waited (ms)
    pub queue_wait_ms: AtomicU32,
    /// Longest a packet may wait in the send queue (ms; 0 =
    /// [`MEDIA_QUEUE_MAX_WAIT`]): core sets it when a stream starts
    pub queue_max_wait_ms: AtomicU32,
    /// The playout delay the sink last reported (AVDTP Delay Report), in
    /// 100 us units plus one; 0 = none reported for this stream
    pub sink_delay: AtomicU32,
    /// The counters above when the current stream started; stream_starts
    /// changes with every stream (written after them)
    pub start_packets: AtomicU64,
    pub start_bytes: AtomicU64,
    pub start_dropped: AtomicU64,
    pub start_capture_frames: AtomicU64,
    pub start_capture_dropped: AtomicU64,
    pub stream_starts: AtomicU32,
    /// Radio readings of the link (BTstack thread, every few seconds)
    pub radio: Mutex<LinkRadio>,
    /// Longest waits along the stream path (core's stream health log)
    pub timing: StreamTiming,
    /// What the encoder runs at now, as the encode thread last saw it under
    /// ABR ([`LiveCodec::pack`]; 0 without ABR or an encode thread)
    pub live_codec: AtomicU64,
}

/// What an ABR encoder runs at now: published by the encode thread, put
/// into the stream info by core while it changes
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LiveCodec {
    pub bitrate_kbps: u32,
    /// SBC / SBC XQ: the bitpool of the frames encoded now (0 = not SBC)
    pub bitpool: u8,
    /// Bytes of the codec frames encoded now, when they all have the same
    /// length (0 = not)
    pub frame_bytes: u16,
}

impl LiveCodec {
    /// One word, so that a reader never sees half an update; 0 = nothing
    /// (a bit rate of 0 is not reported)
    pub fn pack(self) -> u64 {
        u64::from(self.bitrate_kbps.min(0xFF_FFFF)) << 24 | u64::from(self.bitpool) << 16 | u64::from(self.frame_bytes)
    }

    pub fn unpack(v: u64) -> Option<LiveCodec> {
        let c =
            LiveCodec { bitrate_kbps: (v >> 24) as u32 & 0xFF_FFFF, bitpool: (v >> 16) as u8, frame_bytes: v as u16 };
        (c.bitrate_kbps > 0).then_some(c)
    }
}

/// The largest value recorded since the last [`MaxGauge::take`]
#[derive(Debug, Default)]
pub struct MaxGauge(AtomicU64);

impl MaxGauge {
    pub fn record(&self, value: u64) {
        self.0.fetch_max(value, Ordering::Relaxed);
    }

    pub fn record_duration(&self, d: std::time::Duration) {
        self.record(d.as_micros() as u64);
    }

    /// The maximum since the last take; starts over at 0
    pub fn take(&self) -> u64 {
        self.0.swap(0, Ordering::Relaxed)
    }
}

/// Longest waits along the stream path, in microseconds (queue depth in
/// packets), each since core last took it (issue #84: streams after the
/// first stuttered and the log said nothing). Written by the encode and
/// BTstack threads while a stream runs.
#[derive(Debug, Default)]
pub struct StreamTiming {
    /// Encode thread: between two rounds that had audio to encode
    pub encode_gap_us: MaxGauge,
    /// Send queue fill the encode thread left
    pub queue_depth_max: MaxGauge,
    /// BTstack thread: between two media packets handed to L2CAP
    pub send_gap_us: MaxGauge,
    /// From asking BTstack to send a media packet until it may (the
    /// controller's ACL buffers are full meanwhile)
    pub can_send_wait_us: MaxGauge,
    /// Between two Number Of Completed Packets events while streaming: the
    /// controller got nothing through the air meanwhile
    pub completed_gap_us: MaxGauge,
    /// Number Of Completed Packets events while streaming
    pub completed_events: AtomicU64,
}

impl LinkStats {
    pub(crate) fn mark_stream_start(&self) {
        let r = Ordering::Relaxed;
        self.start_packets.store(self.packets_sent.load(r), r);
        self.start_bytes.store(self.bytes_sent.load(r), r);
        self.start_dropped.store(self.queue_drops.load(r) + self.send_errors.load(r), r);
        self.start_capture_frames.store(self.capture_frames.load(r), r);
        self.start_capture_dropped.store(self.capture_dropped_frames.load(r), r);
        self.stream_starts.fetch_add(1, Ordering::Release);
    }

    /// Longest a packet may wait in the send queue
    pub fn queue_max_wait(&self) -> Duration {
        match self.queue_max_wait_ms.load(Ordering::Relaxed) {
            0 => MEDIA_QUEUE_MAX_WAIT,
            ms => Duration::from_millis(u64::from(ms)),
        }
    }

    /// The playout delay the sink reported for this stream (None: no
    /// report; the sink does not support Delay Reporting, or not yet)
    pub fn sink_delay(&self) -> Option<Duration> {
        match self.sink_delay.load(Ordering::Relaxed) {
            0 => None,
            v => Some(Duration::from_micros(u64::from(v - 1) * 100)),
        }
    }

    pub fn set_sink_delay(&self, delay_100us: Option<u16>) {
        self.sink_delay.store(delay_100us.map_or(0, |d| u32::from(d) + 1), Ordering::Relaxed);
    }

    pub fn radio(&self) -> LinkRadio {
        self.radio.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// What an ABR encoder runs at now (None without ABR)
    pub fn live_codec(&self) -> Option<LiveCodec> {
        LiveCodec::unpack(self.live_codec.load(Ordering::Acquire))
    }

    /// The encode thread: what its ABR encoder runs at now (None: nothing
    /// to report, e.g. the thread ended)
    pub fn set_live_codec(&self, c: Option<LiveCodec>) {
        self.live_codec.store(c.map_or(0, LiveCodec::pack), Ordering::Release);
    }

    pub(crate) fn with_radio(&self, f: impl FnOnce(&mut LinkRadio)) {
        f(&mut self.radio.lock().unwrap_or_else(|e| e.into_inner()));
    }
}

/// Radio state of the current link
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkRadio {
    /// BR/EDR RSSI: relative to the golden receive range, not dBm
    pub rssi: Option<i8>,
    /// Channels the host classification leaves usable (None = none passed)
    pub afh_usable: Option<u32>,
    /// Wi-Fi channels passed as bad, "6, 11"
    pub afh_avoided: String,
    /// Channels the link hops over (Read AFH Channel Map)
    pub afh_in_use: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_codec_round_trip() {
        let s = LinkStats::default();
        assert_eq!(s.live_codec(), None);
        let c = LiveCodec { bitrate_kbps: 328, bitpool: 41, frame_bytes: 95 };
        s.set_live_codec(Some(c));
        assert_eq!(s.live_codec(), Some(c));
        let big = LiveCodec { bitrate_kbps: 1000, bitpool: 255, frame_bytes: u16::MAX };
        assert_eq!(LiveCodec::unpack(big.pack()), Some(big));
        // No bit rate is nothing
        assert_eq!(LiveCodec::unpack(LiveCodec { bitpool: 5, ..Default::default() }.pack()), None);
        s.set_live_codec(None);
        assert_eq!(s.live_codec(), None);
    }
}
