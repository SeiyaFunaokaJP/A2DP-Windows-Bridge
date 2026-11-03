//! The media packet queue between the encode thread (producer) and the
//! BTstack thread (consumer): a lock-free SPSC ring of fixed-size slots, so
//! neither side allocates or waits.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use btstack_sys::Remote;

use crate::stats::{LinkStats, MEDIA_QUEUE_MAX_WAIT, MEDIA_QUEUE_PACKETS};

/// RTP media header size (BTstack AVDTP_MEDIA_PAYLOAD_HEADER_SIZE)
pub const RTP_HEADER_SIZE: usize = 12;
/// Largest L2CAP payload with HCI_ACL_PAYLOAD_SIZE (1691 + 4-byte L2CAP header)
pub const MAX_MEDIA_PACKET_SIZE: usize = 1691;
/// Media packet size limit (advanced setting): libldac needs at least 679
/// (one 2-DH5); 1679 is the largest payload the queue can send with an RTP
/// header; 1023 fits one 3-DH5 baseband packet (larger packets span several,
/// so a lost baseband packet loses more audio, for 1-2 % less overhead)
pub const MEDIA_PAYLOAD_LIMIT_MIN: u16 = 679;
pub const MEDIA_PAYLOAD_LIMIT_MAX: u16 = 1679;
pub const MEDIA_PAYLOAD_LIMIT_DEFAULT: u16 = 1023;

pub fn clamp_media_payload_limit(limit: u32) -> u16 {
    limit.clamp(MEDIA_PAYLOAD_LIMIT_MIN as u32, MEDIA_PAYLOAD_LIMIT_MAX as u32) as u16
}

/// One media packet: the media payload (media payload header + codec
/// frames); BTstack adds the RTP header unless `rtp` is false
#[derive(Clone, Copy)]
pub struct MediaPacket {
    pub data: [u8; MAX_MEDIA_PACKET_SIZE],
    pub len: u16,
    pub timestamp: u32,
    pub rtp: bool,
    /// RTP marker bit
    pub marker: bool,
    /// When the packet entered the send queue
    pub queued_at: Option<Instant>,
}

impl Default for MediaPacket {
    fn default() -> Self {
        MediaPacket {
            data: [0; MAX_MEDIA_PACKET_SIZE],
            len: 0,
            timestamp: 0,
            rtp: true,
            marker: false,
            queued_at: None,
        }
    }
}

impl MediaPacket {
    pub fn payload(&self) -> &[u8] {
        &self.data[..self.len as usize]
    }
}

/// Whether the stream is started (AVDTP Start acknowledged, not suspended,
/// released or lost): the BTstack thread sets it, the encode thread reads it
/// before queueing (v1 send_media dropped packets while not streaming)
pub(crate) type StreamingFlag = Arc<AtomicBool>;

pub(crate) fn queue(stats: Arc<LinkStats>, remote: Remote) -> (MediaSink, rtrb::Consumer<MediaPacket>, StreamingFlag) {
    let (producer, consumer) = rtrb::RingBuffer::new(MEDIA_QUEUE_PACKETS);
    let streaming = StreamingFlag::default();
    (MediaSink { producer, remote, stats, streaming: streaming.clone() }, consumer, streaming)
}

/// The BTstack thread's side of the time limit: drops the packets that
/// waited [`MEDIA_QUEUE_MAX_WAIT`], and the oldest of a full queue so the
/// newest audio finds room, and records how long the next packet has waited.
/// Runs on every packet queued, so also while the radio stands still.
/// Returns the packets dropped.
pub(crate) fn drop_stale(media: &mut rtrb::Consumer<MediaPacket>, stats: &LinkStats, now: Instant) -> u32 {
    let mut dropped = 0;
    let mut waited = Duration::ZERO;
    while let Ok(packet) = media.peek() {
        waited = packet.queued_at.map_or(Duration::ZERO, |t| now.saturating_duration_since(t));
        if waited < MEDIA_QUEUE_MAX_WAIT && media.slots() < MEDIA_QUEUE_PACKETS {
            break;
        }
        let _ = media.pop();
        dropped += 1;
        waited = Duration::ZERO;
    }
    stats.queue_wait_ms.store(waited.as_millis() as u32, Ordering::Relaxed);
    if dropped > 0 {
        stats.queue_drops.fetch_add(dropped.into(), Ordering::Relaxed);
        stats.queue_depth.store(media.slots() as u32, Ordering::Relaxed);
    }
    dropped
}

/// The encode thread's end of the queue
pub struct MediaSink {
    producer: rtrb::Producer<MediaPacket>,
    remote: Remote,
    stats: Arc<LinkStats>,
    streaming: StreamingFlag,
}

impl MediaSink {
    /// Queues a packet and wakes the BTstack thread. False (and counted as a
    /// queue drop) when the queue is full, which [`drop_stale`] prevents
    /// unless the BTstack thread is held up too.
    /// While the stream is not started (suspended, reconnecting) the packet
    /// is dropped without counting: nothing would send it, and a queue
    /// filling up meanwhile would make ABR lower the quality and send stale
    /// audio on resume.
    pub fn send(&mut self, packet: &MediaPacket) -> bool {
        if !self.is_streaming() {
            return false;
        }
        let mut queued = *packet;
        queued.queued_at = Some(Instant::now());
        match self.producer.push(queued) {
            Ok(()) => {
                let depth = MEDIA_QUEUE_PACKETS - self.producer.slots();
                self.stats.queue_depth.store(depth as u32, Ordering::Relaxed);
                tracing::trace!("media queue: {} bytes, ts {}, depth {depth}", packet.len, packet.timestamp);
                self.remote.wake();
                true
            }
            Err(_) => {
                let drops = self.stats.queue_drops.fetch_add(1, Ordering::Relaxed) + 1;
                tracing::trace!("media queue full: packet ts {} dropped ({drops} drops)", packet.timestamp);
                false
            }
        }
    }

    /// The stream is started: packets sent now go out
    pub fn is_streaming(&self) -> bool {
        self.streaming.load(Ordering::Acquire)
    }

    /// Packets waiting (ABR input)
    pub fn depth(&self) -> u32 {
        (MEDIA_QUEUE_PACKETS - self.producer.slots()) as u32
    }

    pub fn stats(&self) -> &Arc<LinkStats> {
        &self.stats
    }
}

/// A queue no BTstack thread serves, for tests of the encode side: the
/// returned flag stands for the stream being started
#[doc(hidden)]
pub fn detached_queue(stats: Arc<LinkStats>) -> (MediaSink, rtrb::Consumer<MediaPacket>, Arc<AtomicBool>) {
    queue(stats, Remote::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packets_are_dropped_uncounted_while_not_streaming() {
        let stats = Arc::new(LinkStats::default());
        let (mut sink, mut consumer, streaming) = detached_queue(stats.clone());
        let packet = MediaPacket::default();
        assert!(!sink.send(&packet));
        assert_eq!((sink.depth(), stats.queue_drops.load(Ordering::Relaxed)), (0, 0));
        assert!(consumer.pop().is_err());

        streaming.store(true, Ordering::Release);
        assert!(sink.send(&packet));
        assert_eq!(sink.depth(), 1);

        // A full queue while streaming is a real drop
        while sink.depth() < MEDIA_QUEUE_PACKETS as u32 {
            assert!(sink.send(&packet));
        }
        assert!(!sink.send(&packet));
        assert_eq!(stats.queue_drops.load(Ordering::Relaxed), 1);
    }

    fn streaming_queue() -> (Arc<LinkStats>, MediaSink, rtrb::Consumer<MediaPacket>) {
        let stats = Arc::new(LinkStats::default());
        let (sink, consumer, streaming) = detached_queue(stats.clone());
        streaming.store(true, Ordering::Release);
        (stats, sink, consumer)
    }

    fn packet(timestamp: u32) -> MediaPacket {
        MediaPacket { timestamp, ..Default::default() }
    }

    #[test]
    fn packets_waiting_too_long_are_dropped_oldest_first() {
        let (stats, mut sink, mut consumer) = streaming_queue();
        assert!(sink.send(&packet(1)));
        assert!(sink.send(&packet(2)));
        let queued = consumer.peek().unwrap().queued_at.unwrap();

        assert_eq!(drop_stale(&mut consumer, &stats, queued + Duration::from_millis(100)), 0);
        assert_eq!(stats.queue_wait_ms.load(Ordering::Relaxed), 100);

        assert!(sink.send(&packet(3)));
        assert_eq!(drop_stale(&mut consumer, &stats, queued + Duration::from_secs(10)), 3);
        assert!(consumer.is_empty());
        assert_eq!(stats.queue_drops.load(Ordering::Relaxed), 3);
        assert_eq!(stats.queue_depth.load(Ordering::Relaxed), 0);
        assert_eq!(stats.queue_wait_ms.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn a_full_queue_gives_up_its_oldest_packet() {
        let (stats, mut sink, mut consumer) = streaming_queue();
        for ts in 0..MEDIA_QUEUE_PACKETS as u32 {
            assert!(sink.send(&packet(ts)));
        }
        let queued = consumer.peek().unwrap().queued_at.unwrap();
        assert_eq!(drop_stale(&mut consumer, &stats, queued), 1);
        assert_eq!(consumer.peek().unwrap().timestamp, 1);
        // The newest audio finds room
        assert!(sink.send(&packet(64)));
        assert_eq!(stats.queue_drops.load(Ordering::Relaxed), 1);
    }
}
