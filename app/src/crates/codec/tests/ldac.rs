//! LDAC encoder: a test tone through every supported sample rate, quality,
//! bit depth and typical MTU, checking the frames the way
//! tools/a2dp_decode does, the bit rate, the packet sizes, the RTP timestamp
//! bookkeeping and ABR.

#![cfg(feature = "ldac")]

use codec::ldac::LdacEncoder;
use codec::{CodecKind, Encoder, EncoderConfig, EncoderError, Pcm, PcmFormat, Quality};

/// 1 kHz left, 1.5 kHz right (mono: the left), amplitude 0.5, in the
/// encoder's input format (24-bit MSB-aligned in I32)
struct Tone {
    sample_rate: f64,
    channels: usize,
    format: PcmFormat,
    n: u64,
    i16s: Vec<i16>,
    i32s: Vec<i32>,
}

impl Tone {
    fn new(sample_rate: u32, channels: u32, format: PcmFormat, frames_per_call: usize) -> Tone {
        let len = frames_per_call * channels as usize;
        Tone {
            sample_rate: f64::from(sample_rate),
            channels: channels as usize,
            format,
            n: 0,
            i16s: vec![0; len],
            i32s: vec![0; len],
        }
    }

    fn next(&mut self) -> Pcm<'_> {
        let frames = self.i16s.len() / self.channels;
        for i in 0..frames {
            let t = (self.n + i as u64) as f64 / self.sample_rate;
            for ch in 0..self.channels {
                let f = if ch == 0 { 1000.0 } else { 1500.0 };
                let v = 0.5 * (2.0 * std::f64::consts::PI * f * t).sin();
                self.i16s[i * self.channels + ch] = (v * 32767.0).round() as i16;
                self.i32s[i * self.channels + ch] = ((v * 8_388_607.0).round() as i32) << 8;
            }
        }
        self.n += frames as u64;
        match self.format {
            PcmFormat::I16 => Pcm::I16(&self.i16s),
            PcmFormat::I32 => Pcm::I32(&self.i32s),
        }
    }
}
/// Media MTUs (RTP header excluded) of sinks with L2CAP MTUs 679, 1023, 1679
const MTUS: [u16; 3] = [679 - 12, 1023 - 12, 1679 - 12];

fn config(sample_rate: u32, channels: u32, quality: Quality, bits: u8, mtu: u16) -> EncoderConfig {
    EncoderConfig { sample_rate, channels, quality, mtu, bits_per_sample: bits, abr: false, peer: Default::default() }
}

/// libldac's frame size (header included) per quality, stereo
/// (tbl_ldacbt_config: 2 x 165 / 110 / 55 bytes per channel)
fn frame_bytes(quality: Quality, channels: u32) -> usize {
    let per_ch = match quality {
        Quality::High | Quality::MediumHigh | Quality::LowLatency => 165,
        Quality::Standard => 110,
        Quality::Mobile => 55,
    };
    per_ch * channels as usize
}

/// Frames libldac packs into a packet: as many as fit 679 - 18 bytes
/// (2-DH5 sizing, whatever the MTU)
fn packet_frames(quality: Quality, channels: u32) -> usize {
    (679 - 18) / frame_bytes(quality, channels)
}

/// ldacBT_frmlen_to_bitrate
fn bitrate_of(frame_bytes: usize, sample_rate: u32, frame_samples: usize) -> u32 {
    (frame_bytes as u64 * u64::from(sample_rate) / frame_samples as u64 / 125) as u32
}

/// Frame header fields as tools/a2dp_decode check_ldac reads them
struct FrameHeader {
    sr_id: u8,
    ch_id: u8,
    /// Bytes of the frame including the 3-byte header
    total: usize,
}

/// Parses one media payload ([frame count] + frames) like check_ldac;
/// panics on what it reports as an issue
fn parse_payload(p: &[u8], sr_id: u8, ch_id: u8) -> Vec<FrameHeader> {
    assert!(!p.is_empty(), "LDAC: empty media payload");
    let mut frames = Vec::new();
    let mut off = 1;
    while off < p.len() {
        assert!(p.len() - off >= 3, "LDAC: trailing bytes after the last frame");
        assert_eq!(p[off], 0xAA, "LDAC: frame sync word 0xAA not found where a frame should start");
        let h = FrameHeader {
            sr_id: p[off + 1] >> 5,
            ch_id: (p[off + 1] >> 3) & 0x03,
            total: 3 + ((usize::from(p[off + 1] & 0x07) << 6 | usize::from(p[off + 2] >> 2)) + 1),
        };
        assert!(off + h.total <= p.len(), "LDAC: frame runs past the end of the packet");
        assert_eq!(h.sr_id, sr_id, "LDAC: frame sampling rate differs from configuration");
        assert_eq!(h.ch_id, ch_id, "LDAC: frame channel config differs from configuration");
        off += h.total;
        frames.push(h);
    }
    assert_eq!(usize::from(p[0] & 0x0F), frames.len(), "LDAC: media payload header frame count");
    frames
}

struct Stats {
    frames: u64,
    frame_bytes: Vec<usize>,
    max_frames_per_packet: u32,
    /// Largest PCM consumed - timestamp
    max_lag: u64,
}

/// Encodes `seconds` of the tone and checks every packet and the timestamp
/// bookkeeping as it goes
fn run(enc: &mut LdacEncoder, cfg: &EncoderConfig, seconds: f64) -> Stats {
    let sr_id = match cfg.sample_rate {
        44100 => 0,
        48000 => 1,
        88200 => 2,
        96000 => 3,
        _ => unreachable!(),
    };
    let ch_id = if cfg.channels == 1 { 0 } else { 2 };
    let per_encode = enc.pcm_frames_per_encode();
    let per_frame = enc.pcm_frames_per_codec_frame();
    let mut tone = Tone::new(cfg.sample_rate, cfg.channels, enc.pcm_format(), per_encode);
    let mut out = vec![0u8; enc.max_output_bytes()];
    let mut payload = Vec::with_capacity(usize::from(cfg.mtu));
    let calls = (seconds * f64::from(cfg.sample_rate) / per_encode as f64) as usize;
    let mut stats = Stats { frames: 0, frame_bytes: Vec::new(), max_frames_per_packet: 0, max_lag: 0 };
    let mut consumed = 0u64;
    for _ in 0..calls {
        let r = enc.encode(tone.next(), &mut out).unwrap();
        consumed += per_encode as u64;
        assert_eq!(r.bytes == 0, r.frames == 0, "bytes {} frames {}", r.bytes, r.frames);
        if r.bytes > 0 {
            // One encoder output is one media packet: 1-byte header + frames
            let packet = 1 + r.bytes;
            assert!(packet <= usize::from(cfg.mtu), "packet of {packet} bytes over MTU {}", cfg.mtu);
            assert!(r.frames <= 15, "frame count {} does not fit 4 bits", r.frames);
            payload.clear();
            payload.push(r.frames as u8);
            payload.extend_from_slice(&out[..r.bytes]);
            let frames = parse_payload(&payload, sr_id, ch_id);
            assert_eq!(frames.len(), r.frames as usize);
            stats.frame_bytes.extend(frames.iter().map(|f| f.total));
            stats.frames += u64::from(r.frames);
            stats.max_frames_per_packet = stats.max_frames_per_packet.max(r.frames);
        }
        // The RTP timestamp advances by frames sent x samples per frame: it
        // never runs ahead of the PCM consumed, and lags by no more than the
        // frames libldac holds for the next packet
        let emitted = stats.frames * per_frame as u64;
        assert!(emitted <= consumed, "timestamp {emitted} ahead of PCM {consumed}");
        stats.max_lag = stats.max_lag.max(consumed - emitted);
    }
    let bound = (packet_frames(cfg.quality, cfg.channels) + 1) as u64 * per_frame as u64;
    assert!(stats.max_lag <= bound, "timestamp lagged PCM by {} > {bound}", stats.max_lag);
    stats
}

#[test]
fn every_rate_quality_depth_and_mtu() {
    for sample_rate in [44100, 48000, 88200, 96000] {
        for quality in [Quality::High, Quality::Standard, Quality::Mobile, Quality::MediumHigh] {
            for bits in [16, 24] {
                for mtu in MTUS {
                    let cfg = config(sample_rate, 2, quality, bits, mtu);
                    let what = format!("{cfg:?}");
                    let mut enc = LdacEncoder::new(&cfg).unwrap_or_else(|e| panic!("{what}: {e}"));
                    assert_eq!(enc.kind(), CodecKind::Ldac);
                    assert_eq!(enc.pcm_format(), if bits == 16 { PcmFormat::I16 } else { PcmFormat::I32 });
                    assert_eq!(enc.pcm_frames_per_encode(), 128);
                    let per_frame = if sample_rate > 48000 { 256 } else { 128 };
                    assert_eq!(enc.pcm_frames_per_codec_frame(), per_frame);
                    assert!(!enc.abr_enabled());

                    // 990 / 660 / 330 kbps at 48 / 96 kHz, 909 / 606 / 303 at
                    // 44.1 / 88.2 kHz
                    let fb = frame_bytes(quality, 2);
                    let expected = bitrate_of(fb, sample_rate, per_frame);
                    let nominal = match quality {
                        Quality::High | Quality::MediumHigh | Quality::LowLatency => 990,
                        Quality::Standard => 660,
                        Quality::Mobile => 330,
                    };
                    if sample_rate % 48000 == 0 {
                        assert_eq!(expected, nominal, "{what}");
                    }
                    assert_eq!(enc.bitrate_kbps(), expected, "{what}");

                    let stats = run(&mut enc, &cfg, 2.0);
                    assert!(stats.frames > 0, "{what}: no output");
                    // Every frame has the quality's size, so the stream's
                    // bit rate is the one reported
                    assert!(stats.frame_bytes.iter().all(|&b| b == fb), "{what}: frame sizes {:?}", stats.frame_bytes);
                    // As many frames per packet as fit 2-DH5, whatever the MTU
                    let fpp = packet_frames(quality, 2);
                    assert_eq!(stats.max_frames_per_packet as usize, fpp, "{what}");
                    // 2 seconds of PCM in frames, less what libldac holds back
                    let expected_frames = 2 * u64::from(sample_rate) / per_frame as u64;
                    assert!(stats.frames + fpp as u64 + 1 >= expected_frames, "{what}: {} frames", stats.frames);
                    assert_eq!(enc.bitrate_kbps(), expected, "{what}");
                }
            }
        }
    }
}

#[test]
fn mono() {
    for sample_rate in [44100, 96000] {
        for quality in [Quality::High, Quality::Mobile] {
            let cfg = config(sample_rate, 1, quality, 16, MTUS[0]);
            let mut enc = LdacEncoder::new(&cfg).unwrap();
            let per_frame = enc.pcm_frames_per_codec_frame();
            let fb = frame_bytes(quality, 1);
            assert_eq!(enc.bitrate_kbps(), bitrate_of(fb, sample_rate, per_frame));
            let stats = run(&mut enc, &cfg, 1.0);
            assert!(stats.frames > 0);
            assert!(stats.frame_bytes.iter().all(|&b| b == fb));
            assert_eq!(stats.max_frames_per_packet as usize, packet_frames(quality, 1));
        }
    }
}

#[test]
fn create_returns_ldac() {
    let enc = codec::create(CodecKind::Ldac, &config(48000, 2, Quality::High, 24, MTUS[1])).unwrap();
    assert_eq!(enc.kind(), CodecKind::Ldac);
    assert_eq!(enc.bitrate_kbps(), 990);
    assert_eq!(enc.max_output_bytes(), 1024);
}

#[test]
fn encoder_is_send() {
    fn send<T: Send>() {}
    send::<LdacEncoder>();
}

#[test]
fn invalid_configurations_are_rejected() {
    let ok = config(48000, 2, Quality::High, 16, MTUS[0]);
    let bad = [
        EncoderConfig { sample_rate: 32000, ..ok },
        EncoderConfig { sample_rate: 192000, ..ok },
        EncoderConfig { channels: 0, ..ok },
        EncoderConfig { channels: 3, ..ok },
        EncoderConfig { bits_per_sample: 8, ..ok },
        EncoderConfig { bits_per_sample: 32, ..ok },
        // Transport MTU 678: below libldac's minimum
        EncoderConfig { mtu: 666, ..ok },
        EncoderConfig { mtu: 0, ..ok },
    ];
    for cfg in bad {
        match LdacEncoder::new(&cfg) {
            Err(EncoderError::Unsupported { codec: CodecKind::Ldac, .. }) => {}
            Err(e) => panic!("{cfg:?}: unexpected error {e}"),
            Ok(_) => panic!("{cfg:?} accepted"),
        }
    }
    assert!(LdacEncoder::new(&ok).is_ok());
}

#[test]
fn invalid_input_is_rejected() {
    let cfg = config(48000, 2, Quality::High, 16, MTUS[0]);
    let mut enc = LdacEncoder::new(&cfg).unwrap();
    let mut out = vec![0u8; enc.max_output_bytes()];
    assert!(matches!(
        enc.encode(Pcm::I16(&[0; 128]), &mut out),
        Err(EncoderError::InputLength { expected: 256, got: 128 })
    ));
    assert!(matches!(enc.encode(Pcm::I32(&[0; 256]), &mut out), Err(EncoderError::Encode { .. })));
    assert!(matches!(
        enc.encode(Pcm::I16(&[0; 256]), &mut out[..600]),
        Err(EncoderError::OutputTooSmall { have: 600, need: 1024 })
    ));
    assert!(enc.encode(Pcm::I16(&[0; 256]), &mut out).is_ok());
}

/// Encodes enough calls (48 kHz stereo) for a packet at any quality, so
/// that a quality change has taken effect (libldac applies it at a packet
/// boundary), and checks that the frames of the last packet have the size
/// of the bit rate reported
fn encode_packets(enc: &mut LdacEncoder, tone: &mut Tone, out: &mut [u8]) {
    let mut last = None;
    for _ in 0..16 {
        let r = enc.encode(tone.next(), out).unwrap();
        if r.bytes > 0 {
            let mut payload = vec![r.frames as u8];
            payload.extend_from_slice(&out[..r.bytes]);
            // Frames of a packet may differ in size while libldac moves to a
            // new quality; the last one has the new size
            let frames = parse_payload(&payload, 1, 2);
            last = frames.last().map(|f| f.total);
        }
    }
    let total = last.expect("no packet in 16 calls");
    assert_eq!(bitrate_of(total, 48000, 128), enc.bitrate_kbps(), "frames of {total} bytes");
}

#[test]
fn abr_lowers_the_bitrate_under_a_deep_queue_and_raises_it_back() {
    let cfg = EncoderConfig { abr: true, ..config(48000, 2, Quality::High, 16, MTUS[1]) };
    let mut enc = LdacEncoder::new(&cfg).unwrap();
    assert!(enc.abr_enabled());
    assert_eq!(enc.bitrate_kbps(), 990);
    let mut tone = Tone::new(48000, 2, PcmFormat::I16, 128);
    let mut out = vec![0u8; enc.max_output_bytes()];

    // A full queue (critical: 12 of 64 slots) steps down to MQ through the
    // intermediate modes
    let mut seen = vec![990];
    for _ in 0..10 {
        enc.abr_adjust(20);
        encode_packets(&mut enc, &mut tone, &mut out);
        if seen.last() != Some(&enc.bitrate_kbps()) {
            seen.push(enc.bitrate_kbps());
        }
    }
    assert_eq!(enc.bitrate_kbps(), 330, "steps {seen:?}");
    assert!(seen.windows(2).all(|w| w[1] < w[0]), "steps {seen:?}");

    // An empty queue brings it back to HQ, a step every few seconds (at the
    // 100 ms cadence)
    let mut steps = 0;
    while enc.bitrate_kbps() != 990 {
        enc.abr_adjust(0);
        encode_packets(&mut enc, &mut tone, &mut out);
        steps += 1;
        assert!(steps < 3000, "still at {} kbps after {steps} ABR steps", enc.bitrate_kbps());
    }
    // Not instantly: libldac waits seconds of an empty queue per step
    assert!(steps > 20, "{steps}");
}

#[test]
fn abr_adjust_is_a_no_op_without_abr() {
    let cfg = config(48000, 2, Quality::Standard, 16, MTUS[0]);
    let mut enc = LdacEncoder::new(&cfg).unwrap();
    let mut tone = Tone::new(48000, 2, PcmFormat::I16, 128);
    let mut out = vec![0u8; enc.max_output_bytes()];
    for _ in 0..10 {
        enc.abr_adjust(60);
        encode_packets(&mut enc, &mut tone, &mut out);
    }
    assert_eq!(enc.bitrate_kbps(), 660);
}

#[test]
fn abr_reset_goes_back_to_the_quality_asked_for() {
    let cfg = EncoderConfig { abr: true, ..config(48000, 2, Quality::High, 16, MTUS[1]) };
    let mut enc = LdacEncoder::new(&cfg).unwrap();
    assert!(enc.abr_active());
    let mut tone = Tone::new(48000, 2, PcmFormat::I16, 128);
    let mut out = vec![0u8; enc.max_output_bytes()];
    for _ in 0..10 {
        enc.abr_adjust(20);
        encode_packets(&mut enc, &mut tone, &mut out);
    }
    assert_eq!(enc.bitrate_kbps(), 330);
    enc.abr_reset();
    encode_packets(&mut enc, &mut tone, &mut out);
    assert_eq!(enc.bitrate_kbps(), 990);
    // ABR still works after the reset
    for _ in 0..10 {
        enc.abr_adjust(20);
        encode_packets(&mut enc, &mut tone, &mut out);
    }
    assert_eq!(enc.bitrate_kbps(), 330);

    // Without ABR there is nothing active and nothing to reset
    let mut fixed = LdacEncoder::new(&config(48000, 2, Quality::Standard, 16, MTUS[0])).unwrap();
    assert!(!fixed.abr_active());
    fixed.abr_reset();
    encode_packets(&mut fixed, &mut tone, &mut out);
    assert_eq!(fixed.bitrate_kbps(), 660);
}
