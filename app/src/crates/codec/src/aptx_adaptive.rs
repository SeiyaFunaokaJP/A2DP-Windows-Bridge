//! aptX Adaptive (Qualcomm vendor 0x000000D7, codec 0x00AD), encoded by the
//! pure-Rust encoder of AKS-Logic/aptx-adaptive-linux (crate
//! `aptx-adaptive`, MIT, vendored in `app/src/crates/aptx-adaptive`).
//!
//! The encoder covers the R1 and R2 bitstreams of a host (software) source:
//! high quality (HQ) and low latency (LL), with the reference's per-packet
//! loop, mode tables and bit rate levels (`aptx_adaptive::Stream`). A2DPWB
//! streams 48 kHz stereo, the rate every aptX Adaptive sink takes and the only
//! one an R1 sink is ever streamed at, or 96 kHz in HQ to an R2 sink that
//! lists it when the profile asks for 96 kHz (as upstream's plugin: config 3,
//! the 96 kHz table row; confirmed on real headphones, issue #86). The encoder
//! also has 44.1 kHz for R2 sinks, not used.
//!
//! Framing as upstream's spec (`a2dp-integration.md` §7D, §7F, §7H), which
//! matches a Snapdragon phone's stream: one codec packet per media packet,
//! each 16-bit word byte-swapped, behind a plain 12-byte RTP header and no
//! media payload header. The RTP header is not BTstack's, whose timestamp and
//! SSRC mean something else here: payload type 96, a sequence number +1 per
//! packet, the timestamp = the packet's samples per channel (its high 16 bits,
//! a latency adjustment in ms, 0) and the SSRC = the sink's latency mode
//! (0xA3 HQ, 0xA1 LL). So the encoder writes the whole RTP packet and the
//! packetizer sends it as is (as FastStream, without BTstack's header).
//!
//! Levels (bit rate, packet period) with an R1 sink in HQ: 2 = 318 kbps /
//! 16 ms, 3 = 364 / 14 (the reference's start), 4 = 427 / 12, all 656-byte
//! packets; R2 HQ adds level 5 (427 kbps; 4 is 340). The quality picks the
//! level: High the top one, Standard 3, Mobile 2. Under ABR the level starts
//! there and steps down (to 2) while the send queue backs up and back up (to
//! the start) once it stays empty, as PipeWire's reduce / increase_bitpool
//! drive upstream's plugin. Quality::LowLatency streams LL: 265 kbps / 10 ms
//! / 348-byte packets with an R1 sink (the HQ bitstream on the LL row), 270 /
//! 10 / 348 with an R2 sink, at every level (no ABR). At 96 kHz (R2, HQ):
//! 2 = 255 kbps / 20 ms, 3 = 340 / 15, 4 = 420 / 12, 5 = 646 / 8, all
//! 656-byte packets.
//!
//! aptX is a Qualcomm trademark. The encoder and its numeric tables were
//! reconstructed by its authors from shipped binaries and on-air captures
//! (see the crate's LEGAL.md); the codec may be covered by patents in some
//! jurisdictions (THIRD_PARTY_LICENSES.md).

use aptx_adaptive::{Config, DecoderLevel, Profile, Stream, packet_budget};

use crate::{CodecKind, Encoded, Encoder, EncoderConfig, EncoderDetails, EncoderError, Pcm, PcmFormat, Quality};

/// The RTP header the encoder writes in front of each codec packet
pub const RTP_HEADER_LEN: usize = 12;
/// RTP version 2, no padding, extension or CSRC
const RTP_V2: u8 = 0x80;
/// Payload type 96 (marker clear), as the phone's stream
const RTP_PAYLOAD_TYPE: u8 = 0x60;
/// SSRC: the sink's latency mode (§7H); LL0 is the "2.4 GHz Wi-Fi idle"
/// latency, the one a source without sideband feedback stays in
pub const SSRC_HQ: u32 = 0x0000_00A3;
pub const SSRC_LL: u32 = 0x0000_00A1;

/// The rate A2DPWB streams by default, to every sink
pub const SAMPLE_RATE: u32 = 48000;
/// HQ to an R2 sink that lists it
pub const SAMPLE_RATE_96K: u32 = 96000;

/// PCM frames per encode call: 2 ms (96 at 48 kHz, 192 at 96 kHz), a
/// divisor of every packet period (8..50 ms in whole ms, all even but 13 /
/// 15 / 19 ms rows of R2 sinks, which only shift when within a call a packet
/// goes out)
const ENCODE_MS: u32 = 2;

/// The reference's level at the start of a stream (and LL's only one)
const START_LEVEL: u8 = 3;
/// The lowest level ABR steps down to (level 1 is never used by the
/// reference; a request for it is taken as 2)
const ABR_MIN_LEVEL: u8 = 2;
/// ABR runs every 100 ms (pipeline ABR_INTERVAL) with the send queue depth
/// in packets (~10-16 ms each). A queue of this many packets lowers the
/// level by one ...
const ABR_DOWN_DEPTH: u32 = 3;
/// ... at most every 5 calls (~0.5 s, PipeWire's back-off between reductions)
const ABR_DOWN_CALLS: u32 = 5;
/// The level goes up one step after this many calls (~3 s) with at most one
/// packet queued throughout
const ABR_UP_CALLS: u32 = 30;
const ABR_UP_DEPTH: u32 = 1;

/// Encoder for [`CodecKind::AptxAdaptive`]
pub struct AptxAdaptiveEncoder {
    stream: Stream<aptx_adaptive::Encoder>,
    channels: usize,
    /// PCM frames per encode call ([`ENCODE_MS`] at the stream's rate)
    frames_per_encode: usize,
    /// 24-bit right-justified stereo input of one call (allocated once:
    /// encode() must not allocate)
    pcm: Vec<i32>,
    seq: u16,
    ssrc: u32,
    /// The level the quality asks for: fixed, or ABR's start and ceiling
    level: u8,
    abr: Option<Abr>,
}

#[derive(Debug, Default)]
struct Abr {
    /// Calls since the last step down
    since_down: u32,
    /// Calls in a row with the queue at or below ABR_UP_DEPTH
    calm: u32,
}

/// The decoder compatibility level of the sink (`caps::RemoteCaps`): R2 when
/// its codec information has the capability extension
pub fn decoder_level(r2: bool) -> DecoderLevel {
    if r2 { DecoderLevel::R2 } else { DecoderLevel::R1 }
}

/// The highest level whose bit rate differs from the one below it (HQ)
fn top_level(level: DecoderLevel) -> u8 {
    match level {
        DecoderLevel::R1 => 4,
        DecoderLevel::R2 => 5,
    }
}

impl AptxAdaptiveEncoder {
    /// Takes 48 kHz (or 96 kHz in HQ to an R2 sink), 1 or 2 channels (mono
    /// is duplicated: the stream is stereo) of 32-bit MSB-aligned PCM, of
    /// which the top 24 bits are encoded. The sink's decoder level comes
    /// from [`crate::PeerParams::aptxad_r2`]; the packet size from the MTU.
    pub fn new(config: &EncoderConfig) -> Result<AptxAdaptiveEncoder, EncoderError> {
        let kind = CodecKind::AptxAdaptive;
        let unsupported = |what: String| Err(EncoderError::Unsupported { codec: kind, what });
        let level = decoder_level(config.peer.aptxad_r2);
        let profile = if config.quality == Quality::LowLatency { Profile::Ll } else { Profile::Hq };
        match config.sample_rate {
            SAMPLE_RATE => {}
            SAMPLE_RATE_96K if level == DecoderLevel::R2 && profile == Profile::Hq => {}
            SAMPLE_RATE_96K => {
                return unsupported(format!("{SAMPLE_RATE_96K} Hz in {profile:?} to an {level:?} sink (R2, HQ)"));
            }
            rate => return unsupported(format!("sample rate {rate} ({SAMPLE_RATE} or {SAMPLE_RATE_96K})")),
        }
        if config.channels != 1 && config.channels != 2 {
            return unsupported(format!("{} channels (1 or 2)", config.channels));
        }
        let sample_rate = config.sample_rate;
        let stream_config = Config { profile, sample_rate, channels: 2, level };
        let budget = packet_budget(usize::from(config.mtu));
        let mut stream = Stream::new(stream_config, budget).map_err(|e| EncoderError::Init {
            codec: kind,
            what: format!("{profile:?} at media MTU {} (packet budget {budget}): {e}", config.mtu),
        })?;
        let wanted = match (profile, config.quality) {
            (Profile::Ll, _) => START_LEVEL,
            (_, Quality::Standard) => 3,
            (_, Quality::Mobile) => 2,
            _ => top_level(level),
        };
        // The stream starts at level 3; any other level applies before the
        // first packet
        if wanted != stream.level() && stream.request_level(wanted) {
            stream.apply_level_now();
        }
        let abr = (config.abr && profile == Profile::Hq).then(Abr::default);
        let frames_per_encode = (sample_rate * ENCODE_MS / 1000) as usize;
        let encoder = AptxAdaptiveEncoder {
            ssrc: if profile == Profile::Ll { SSRC_LL } else { SSRC_HQ },
            stream,
            channels: config.channels as usize,
            frames_per_encode,
            pcm: vec![0; frames_per_encode * 2],
            seq: 0,
            level: wanted,
            abr,
        };
        let row = encoder.stream.row();
        tracing::info!(
            "aptX Adaptive: {profile:?}, sink {level:?}, {sample_rate} Hz, in_ch={} mtu={} budget={budget}: level {} = \
             {} kbps, {} ms, {}-byte packets, abr {}",
            config.channels,
            config.mtu,
            encoder.stream.level(),
            row.bitrate / 1000,
            row.period_ms,
            row.packet_bytes,
            encoder.abr.is_some()
        );
        Ok(encoder)
    }

    /// Samples per channel of the packet the next push emits (the RTP
    /// timestamp's low 16 bits)
    fn packet_frames(&self) -> u32 {
        (self.stream.samples_per_packet() / 2) as u32
    }

    fn request(&mut self, level: u8, why: std::fmt::Arguments<'_>) {
        let before = self.stream.row().bitrate;
        if self.stream.request_level(level) {
            tracing::debug!(
                "aptX Adaptive: ABR level {} -> {level} ({} kbps -> next packet), {why}",
                self.stream.level(),
                before / 1000
            );
        }
    }
}

impl Encoder for AptxAdaptiveEncoder {
    fn kind(&self) -> CodecKind {
        CodecKind::AptxAdaptive
    }

    fn pcm_format(&self) -> PcmFormat {
        PcmFormat::I32
    }

    fn pcm_frames_per_encode(&self) -> usize {
        self.frames_per_encode
    }

    /// [`Encoded::frames`] counts packets; a packet's audio depends on the
    /// level (the pipeline's own timestamp is not sent: the encoder writes
    /// the RTP header)
    fn pcm_frames_per_codec_frame(&self) -> usize {
        self.packet_frames() as usize
    }

    fn max_output_bytes(&self) -> usize {
        RTP_HEADER_LEN + aptx_adaptive::MAX_PACKET
    }

    fn encode(&mut self, pcm: Pcm<'_>, out: &mut [u8]) -> Result<Encoded, EncoderError> {
        let kind = CodecKind::AptxAdaptive;
        let expected = self.frames_per_encode * self.channels;
        let Pcm::I32(samples) = pcm else {
            return Err(EncoderError::Encode { codec: kind, what: "PCM is not I32".into() });
        };
        if samples.len() != expected {
            return Err(EncoderError::InputLength { expected, got: samples.len() });
        }
        let need = RTP_HEADER_LEN + self.stream.row().packet_bytes;
        if out.len() < need {
            return Err(EncoderError::OutputTooSmall { have: out.len(), need });
        }
        // 24-bit right-justified, sign-extended (§3.4); mono to both channels
        let channels = self.channels;
        for (frame, o) in samples.chunks_exact(channels).zip(self.pcm.as_chunks_mut::<2>().0) {
            let l = frame[0] >> 8;
            *o = [l, if channels == 2 { frame[1] >> 8 } else { l }];
        }
        // The period of the packet this call may emit: a level change takes
        // effect after it
        let frames = self.packet_frames();
        let written = self
            .stream
            .push(&self.pcm, &mut out[RTP_HEADER_LEN..])
            .map_err(|e| EncoderError::Encode { codec: kind, what: e.to_string() })?;
        let Some(written) = written else {
            return Ok(Encoded::default());
        };
        let h = &mut out[..RTP_HEADER_LEN];
        h[0] = RTP_V2;
        h[1] = RTP_PAYLOAD_TYPE;
        h[2..4].copy_from_slice(&self.seq.to_be_bytes());
        // Latency adjustment 0 (the sink corrects drift itself), samples per
        // channel never 0
        h[4..8].copy_from_slice(&frames.min(u32::from(u16::MAX)).to_be_bytes());
        h[8..12].copy_from_slice(&self.ssrc.to_be_bytes());
        self.seq = self.seq.wrapping_add(1);
        Ok(Encoded { bytes: RTP_HEADER_LEN + written, frames: 1 })
    }

    fn bitrate_kbps(&self) -> u32 {
        self.stream.row().bitrate / 1000
    }

    /// One step per call (every 100 ms): down one level while the queue
    /// holds [`ABR_DOWN_DEPTH`] packets or more (at most every ~0.5 s, not
    /// below level 2), up one level after ~3 s with the queue nearly empty
    /// (not above the quality's level). A step applies from the next packet
    /// (§7L); a level whose bit rate equals the current one is skipped.
    fn abr_adjust(&mut self, queue_depth: u32) {
        let Some(abr) = self.abr.as_mut() else { return };
        abr.since_down = abr.since_down.saturating_add(1);
        abr.calm = if queue_depth <= ABR_UP_DEPTH { abr.calm + 1 } else { 0 };
        let current = self.stream.pending_level().unwrap_or(self.stream.level());
        let bitrate = |s: &Stream<_>, l: u8| s.level_row(l).map(|r| r.bitrate);
        if queue_depth >= ABR_DOWN_DEPTH && abr.since_down >= ABR_DOWN_CALLS && current > ABR_MIN_LEVEL {
            abr.since_down = 0;
            abr.calm = 0;
            let now = bitrate(&self.stream, current);
            let target = (ABR_MIN_LEVEL..current).rev().find(|&l| bitrate(&self.stream, l) != now);
            if let Some(level) = target {
                self.request(level, format_args!("queue {queue_depth}"));
            }
        } else if abr.calm >= ABR_UP_CALLS && current < self.level {
            abr.calm = 0;
            let now = bitrate(&self.stream, current);
            let target = (current + 1..=self.level).find(|&l| bitrate(&self.stream, l) != now);
            if let Some(level) = target {
                self.request(level, format_args!("queue at most {ABR_UP_DEPTH} for {ABR_UP_CALLS} rounds"));
            }
        }
    }

    fn abr_active(&self) -> bool {
        self.abr.is_some()
    }

    /// Back to the quality's level (from the next packet)
    fn abr_reset(&mut self) {
        let Some(abr) = self.abr.as_mut() else { return };
        *abr = Abr::default();
        if self.stream.level() != self.level {
            self.stream.request_level(self.level);
        }
    }

    fn details(&self) -> EncoderDetails {
        EncoderDetails { frame_bytes: Some(self.stream.row().packet_bytes as u32), ..Default::default() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PeerParams;

    fn config(quality: Quality, r2: bool, abr: bool, channels: u32) -> EncoderConfig {
        EncoderConfig {
            sample_rate: 48000,
            channels,
            quality,
            mtu: 1011,
            bits_per_sample: 24,
            abr,
            peer: PeerParams { aptxad_r2: r2, ..Default::default() },
        }
    }

    /// A 1 kHz tone at 48 kHz, MSB-aligned like the capture's 24-bit samples
    fn tone(frames: usize, channels: usize) -> Vec<i32> {
        tone_at(48000, frames, channels)
    }

    fn tone_at(rate: u32, frames: usize, channels: usize) -> Vec<i32> {
        (0..frames)
            .flat_map(|i| {
                let v = ((i as f64 * 2.0 * std::f64::consts::PI * 1000.0 / f64::from(rate)).sin() * 0.5 * 8_388_607.0)
                    as i32;
                std::iter::repeat_n(v << 8, channels)
            })
            .collect()
    }

    /// Encodes `pcm` in encoder-sized calls; the media packets
    fn run(enc: &mut AptxAdaptiveEncoder, pcm: &[i32]) -> Vec<Vec<u8>> {
        let mut out = vec![0u8; enc.max_output_bytes()];
        let mut packets = Vec::new();
        for block in pcm.chunks_exact(enc.pcm_frames_per_encode() * enc.channels) {
            let e = enc.encode(Pcm::I32(block), &mut out).unwrap();
            if e.bytes > 0 {
                assert_eq!(e.frames, 1);
                packets.push(out[..e.bytes].to_vec());
            }
        }
        packets
    }

    fn ts(p: &[u8]) -> u32 {
        u32::from_be_bytes(p[4..8].try_into().unwrap())
    }

    #[test]
    fn r1_hq_standard_is_the_reference_stream() {
        // Level 3: 364 kbps, 14 ms (672 samples per channel), 656-byte
        // packets: 71 in one second
        let mut enc = AptxAdaptiveEncoder::new(&config(Quality::Standard, false, false, 2)).unwrap();
        assert_eq!(enc.bitrate_kbps(), 364);
        let packets = run(&mut enc, &tone(48000, 2));
        assert_eq!(packets.len(), 71);
        for (i, p) in packets.iter().enumerate() {
            assert_eq!(p.len(), 12 + 656);
            let seq = (i as u16).to_be_bytes();
            assert_eq!(p[..12], [0x80, 0x60, seq[0], seq[1], 0, 0, 0x02, 0xA0, 0, 0, 0, 0xA3]);
            // Config 0 (R1) frames start 00 81, byte-swapped on air
            assert_eq!(p[12..14], [0x81, 0x00], "packet {i}");
        }
        assert_eq!(enc.details().frame_bytes, Some(656));
    }

    #[test]
    fn r1_hq_high_is_level_4() {
        // 427 kbps, 12 ms packets (576 samples per channel) from the first
        let mut enc = AptxAdaptiveEncoder::new(&config(Quality::High, false, false, 2)).unwrap();
        assert_eq!(enc.bitrate_kbps(), 427);
        let packets = run(&mut enc, &tone(48000, 2));
        assert!((82..=84).contains(&packets.len()), "{}", packets.len());
        assert!(packets.iter().all(|p| p.len() == 12 + 656 && ts(p) == 576));
        let mobile = AptxAdaptiveEncoder::new(&config(Quality::Mobile, false, false, 2)).unwrap();
        assert_eq!(mobile.bitrate_kbps(), 318);
    }

    #[test]
    fn r2_hq_uses_the_r2_bitstream() {
        // R2 index 1, level 5: 427 kbps, 12 ms; config 3 frames start 00 83
        let mut enc = AptxAdaptiveEncoder::new(&config(Quality::High, true, false, 2)).unwrap();
        assert_eq!(enc.bitrate_kbps(), 427);
        let packets = run(&mut enc, &tone(9600, 2));
        assert!(!packets.is_empty());
        for p in &packets {
            assert_eq!(p[12..14], [0x83, 0x00]);
            assert_eq!(p[8..12], [0, 0, 0, 0xA3]);
            assert_eq!(ts(p), 576);
        }
    }

    #[test]
    fn low_latency() {
        // R1: the HQ bitstream on the LL row, 265 kbps / 10 ms / 348 bytes;
        // R2: config 4, 270 kbps
        for (r2, kbps) in [(false, 265), (true, 270)] {
            let mut enc = AptxAdaptiveEncoder::new(&config(Quality::LowLatency, r2, true, 2)).unwrap();
            assert_eq!(enc.bitrate_kbps(), kbps);
            assert!(!enc.abr_active(), "LL has no ABR");
            let packets = run(&mut enc, &tone(48000, 2));
            assert_eq!(packets.len(), 100, "r2 {r2}");
            for p in &packets {
                assert_eq!(p.len(), 12 + 348);
                assert_eq!(p[4..12], [0, 0, 0x01, 0xE0, 0, 0, 0, 0xA1]);
            }
        }
    }

    #[test]
    fn r2_hq_at_96k() {
        // R2 HQ at 96 kHz (the 96 kHz table row, config 3): 2 ms = 192
        // frames per call. High: level 5 = 646 kbps, 8 ms (768 samples per
        // channel), 656-byte packets, 125 in one second
        let at96 = |quality, abr| EncoderConfig { sample_rate: 96000, ..config(quality, true, abr, 2) };
        let mut enc = AptxAdaptiveEncoder::new(&at96(Quality::High, false)).unwrap();
        assert_eq!(enc.pcm_frames_per_encode(), 192);
        assert_eq!(enc.bitrate_kbps(), 646);
        let packets = run(&mut enc, &tone_at(96000, 96000, 2));
        assert!((124..=125).contains(&packets.len()), "{}", packets.len());
        for p in &packets {
            assert_eq!(p.len(), 12 + 656);
            assert_eq!(p[4..12], [0, 0, 0x03, 0x00, 0, 0, 0, 0xA3]);
        }
        assert_eq!(enc.pcm_frames_per_codec_frame(), 768);
        // Not the 48 kHz bitstream
        let mut at48 = AptxAdaptiveEncoder::new(&config(Quality::High, true, false, 2)).unwrap();
        assert_ne!(packets[1][12..], run(&mut at48, &tone(9600, 2))[1][12..]);
        // Standard: level 3 = 340 kbps, 15 ms (1440 samples); Mobile: 255
        let mut sq = AptxAdaptiveEncoder::new(&at96(Quality::Standard, false)).unwrap();
        assert_eq!(sq.bitrate_kbps(), 340);
        let packets = run(&mut sq, &tone_at(96000, 19200, 2));
        assert!(!packets.is_empty() && packets.iter().all(|p| ts(p) == 1440));
        assert_eq!(AptxAdaptiveEncoder::new(&at96(Quality::Mobile, false)).unwrap().bitrate_kbps(), 255);
        // ABR down from 646 kbps, mono input
        let mut abr = AptxAdaptiveEncoder::new(&EncoderConfig { channels: 1, ..at96(Quality::High, true) }).unwrap();
        assert!(abr.abr_active());
        for _ in 0..ABR_DOWN_CALLS {
            abr.abr_adjust(8);
        }
        run(&mut abr, &tone_at(96000, 4800, 1));
        assert_eq!(abr.bitrate_kbps(), 420);
    }

    #[test]
    fn mono_input_is_sent_as_stereo() {
        let mut mono = AptxAdaptiveEncoder::new(&config(Quality::Standard, false, false, 1)).unwrap();
        let mut stereo = AptxAdaptiveEncoder::new(&config(Quality::Standard, false, false, 2)).unwrap();
        assert_eq!(run(&mut mono, &tone(9600, 1)), run(&mut stereo, &tone(9600, 2)));
    }

    #[test]
    fn rejects_what_it_cannot_send() {
        let at = |rate| EncoderConfig { sample_rate: rate, ..config(Quality::High, false, false, 2) };
        assert!(AptxAdaptiveEncoder::new(&at(44100)).is_err());
        // 96 kHz: R2 HQ only
        assert!(AptxAdaptiveEncoder::new(&at(96000)).is_err());
        let r2_ll = EncoderConfig { sample_rate: 96000, ..config(Quality::LowLatency, true, false, 2) };
        assert!(AptxAdaptiveEncoder::new(&r2_ll).is_err());
        assert!(AptxAdaptiveEncoder::new(&EncoderConfig { sample_rate: 88200, ..r2_ll }).is_err());
        assert!(AptxAdaptiveEncoder::new(&EncoderConfig { channels: 6, ..at(48000) }).is_err());
        // An MTU whose packets cannot carry the lowest bit rate
        assert!(AptxAdaptiveEncoder::new(&EncoderConfig { mtu: 200, ..at(48000) }).is_err());
        let mut enc = AptxAdaptiveEncoder::new(&at(48000)).unwrap();
        let mut out = [0u8; 100];
        assert!(enc.encode(Pcm::I32(&[0; 192]), &mut out).is_err());
        let mut out = vec![0u8; enc.max_output_bytes()];
        assert!(matches!(enc.encode(Pcm::I32(&[0; 190]), &mut out), Err(EncoderError::InputLength { .. })));
        assert!(enc.encode(Pcm::I16(&[0; 192]), &mut out).is_err());
    }

    #[test]
    fn smaller_mtu_caps_the_hq_bit_rate() {
        // A 600-byte media payload: packets of 592 bytes, the bit rate from
        // the budget (§7J: 592 * 8000 / 12 - 25000)
        let mut enc =
            AptxAdaptiveEncoder::new(&EncoderConfig { mtu: 600, ..config(Quality::High, false, false, 2) }).unwrap();
        assert_eq!(enc.bitrate_kbps(), (592 * 8000 / 12 - 25_000) / 1000);
        let packets = run(&mut enc, &tone(9600, 2));
        assert!(packets.iter().all(|p| p.len() == 12 + 592));
    }

    #[test]
    fn abr_steps_down_and_back_up() {
        let mut enc = AptxAdaptiveEncoder::new(&config(Quality::High, false, true, 2)).unwrap();
        assert!(enc.abr_active());
        let pcm = tone(48000, 2);
        let level = |e: &AptxAdaptiveEncoder| e.stream.pending_level().unwrap_or(e.stream.level());
        assert_eq!(level(&enc), 4);
        // A backed-up queue: one level down, then nothing until ~0.5 s
        for _ in 0..5 {
            enc.abr_adjust(8);
        }
        assert_eq!(level(&enc), 3);
        for _ in 0..4 {
            enc.abr_adjust(8);
        }
        assert_eq!(level(&enc), 3);
        enc.abr_adjust(8);
        assert_eq!(level(&enc), 2);
        // Not below 2
        for _ in 0..20 {
            enc.abr_adjust(8);
        }
        assert_eq!(level(&enc), 2);
        // Applied at the next packet: 318 kbps, 16 ms
        let packets = run(&mut enc, &pcm[..2 * 9600]);
        assert_eq!(enc.bitrate_kbps(), 318);
        assert_eq!(ts(packets.last().unwrap()), 768);
        // ~3 s of an empty queue: one level up, up to the quality's
        for _ in 0..ABR_UP_CALLS {
            enc.abr_adjust(0);
        }
        assert_eq!(level(&enc), 3);
        for _ in 0..2 * ABR_UP_CALLS {
            enc.abr_adjust(1);
        }
        assert_eq!(level(&enc), 4);
        for _ in 0..2 * ABR_UP_CALLS {
            enc.abr_adjust(0);
        }
        assert_eq!(level(&enc), 4);
        // A reset goes back to the quality's level
        enc.abr_adjust(8);
        for _ in 0..10 {
            enc.abr_adjust(8);
        }
        assert!(level(&enc) < 4);
        run(&mut enc, &pcm[..2 * 4800]);
        enc.abr_reset();
        run(&mut enc, &pcm[..2 * 4800]);
        assert_eq!(enc.bitrate_kbps(), 427);
    }
}
