//! LHDC V5 encoder: a test tone through every supported sample rate,
//! quality, bit depth and typical MTU, checking the packets the way
//! tools/a2dp_decode does, the bit rate, the packet sizes, the RTP timestamp
//! bookkeeping, the sink's bit rate limits and ABR.

#![cfg(feature = "lhdcv5")]

use codec::lhdcv5::LhdcV5Encoder;
use codec::{CodecKind, Encoder, EncoderConfig, EncoderError, Pcm, PcmFormat, PeerParams, Quality};

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

fn config(sample_rate: u32, quality: Quality, bits: u8, mtu: u16) -> EncoderConfig {
    EncoderConfig {
        sample_rate,
        channels: 2,
        quality,
        mtu,
        bits_per_sample: bits,
        abr: false,
        peer: PeerParams::default(),
    }
}

/// The encoder's bit rate table (kbps) for the four quality modes
fn nominal_kbps(sample_rate: u32, quality: Quality) -> u32 {
    match quality {
        Quality::High | Quality::LowLatency => 1000,
        Quality::MediumHigh => 900,
        Quality::Standard if sample_rate == 44100 => 480,
        Quality::Standard => 500,
        Quality::Mobile => 400,
    }
}

/// Checks one media payload like tools/a2dp_decode check_lhdcv5 (2-byte
/// header [frames << 2 | latency] [sequence], then frames each starting
/// with a 2-byte LE header whose low 10 bits are the encoded bytes per
/// channel: frame = 2 + 2 * that); panics on what it reports as an issue.
/// Returns the frame sizes.
fn parse_payload(p: &[u8]) -> Vec<usize> {
    assert!(p.len() >= 2, "LHDC V5: media payload shorter than its 2-byte header");
    let announced = usize::from(p[0] >> 2);
    let mut frames = Vec::new();
    let mut off = 2;
    while off < p.len() {
        assert!(p.len() - off >= 2, "LHDC V5: trailing bytes after the last frame");
        let per_ch = (usize::from(p[off]) | usize::from(p[off + 1]) << 8) & 0x3FF;
        let total = 2 + 2 * per_ch;
        assert_ne!(per_ch, 0, "LHDC V5: frame header with zero length");
        assert!(off + total <= p.len(), "LHDC V5: frame runs past the end of the packet");
        frames.push(total);
        off += total;
    }
    assert_eq!(announced, frames.len(), "LHDC V5: media payload header frame count differs from frames in packet");
    frames
}

struct Stats {
    frames: u64,
    frame_bytes: u64,
    max_frames_per_packet: u32,
    max_lag: u64,
}

/// PCM frames fed to an encoder and codec frames it emitted, over its life
#[derive(Default)]
struct Clock {
    consumed: u64,
    frames: u64,
}

/// Encodes `seconds` of the tone with a new encoder's clock
fn run(enc: &mut LhdcV5Encoder, cfg: &EncoderConfig, seconds: f64) -> Stats {
    run_on(enc, cfg, seconds, &mut Clock::default())
}

/// Encodes `seconds` of the tone as the encode thread would (one packet per
/// encoder output), checking every packet and the timestamp bookkeeping
fn run_on(enc: &mut LhdcV5Encoder, cfg: &EncoderConfig, seconds: f64, clock: &mut Clock) -> Stats {
    let per_encode = enc.pcm_frames_per_encode();
    let per_frame = enc.pcm_frames_per_codec_frame();
    let mut tone = Tone::new(cfg.sample_rate, cfg.channels, enc.pcm_format(), per_encode);
    let mut out = vec![0u8; enc.max_output_bytes()];
    let mut payload = Vec::with_capacity(usize::from(cfg.mtu));
    let calls = (seconds * f64::from(cfg.sample_rate) / per_encode as f64) as usize;
    let mut stats = Stats { frames: 0, frame_bytes: 0, max_frames_per_packet: 0, max_lag: 0 };
    let mut seq = 0u8;
    for _ in 0..calls {
        let r = enc.encode(tone.next(), &mut out).unwrap();
        clock.consumed += per_encode as u64;
        assert_eq!(r.bytes == 0, r.frames == 0, "bytes {} frames {}", r.bytes, r.frames);
        if r.bytes > 0 {
            assert!(2 + r.bytes <= usize::from(cfg.mtu), "packet of {} bytes over MTU {}", 2 + r.bytes, cfg.mtu);
            assert!(r.frames < 64, "frame count {} does not fit 6 bits", r.frames);
            payload.clear();
            payload.extend_from_slice(&[(r.frames as u8) << 2, seq]);
            payload.extend_from_slice(&out[..r.bytes]);
            seq = seq.wrapping_add(1);
            let frames = parse_payload(&payload);
            assert_eq!(frames.len(), r.frames as usize);
            stats.frame_bytes += frames.iter().sum::<usize>() as u64;
            stats.frames += u64::from(r.frames);
            clock.frames += u64::from(r.frames);
            stats.max_frames_per_packet = stats.max_frames_per_packet.max(r.frames);
        }
        // The RTP timestamp advances by frames sent x samples per frame: it
        // never runs ahead of the PCM consumed, and lags by what the encoder
        // holds for the next packet
        let emitted = clock.frames * per_frame as u64;
        assert!(emitted <= clock.consumed, "timestamp {emitted} ahead of PCM {}", clock.consumed);
        stats.max_lag = stats.max_lag.max(clock.consumed - emitted);
    }
    // At most a packet's worth less one frame waits in the encoder
    let bound = u64::from(stats.max_frames_per_packet.saturating_sub(1)) * per_frame as u64;
    assert!(stats.max_lag <= bound, "timestamp lagged PCM by {} > {bound}", stats.max_lag);
    stats
}

/// Bit rate of the encoded frames (240-sample frames at 44.1 kHz last
/// 5.44 ms, and the encoder sizes them for that)
fn stream_kbps(stats: &Stats, sample_rate: u32, per_frame: usize) -> f64 {
    let seconds = stats.frames as f64 * per_frame as f64 / f64::from(sample_rate);
    stats.frame_bytes as f64 * 8.0 / seconds / 1000.0
}

/// The stream carries the bit rate reported (within the frame header and
/// rounding)
fn assert_stream_bitrate(stats: &Stats, sample_rate: u32, per_frame: usize, kbps: u32, what: &str) {
    let expected = f64::from(kbps);
    let got = stream_kbps(stats, sample_rate, per_frame);
    assert!((got - expected).abs() <= expected * 0.02, "{what}: stream at {got:.1} kbps, expected {expected:.1}");
}

#[test]
fn every_rate_quality_depth_and_mtu() {
    for sample_rate in [44100, 48000, 96000, 192000] {
        for quality in [Quality::High, Quality::MediumHigh, Quality::Standard, Quality::Mobile] {
            for bits in [16, 24] {
                for mtu in MTUS {
                    let cfg = config(sample_rate, quality, bits, mtu);
                    let what = format!("{cfg:?}");
                    let mut enc = LhdcV5Encoder::new(&cfg).unwrap_or_else(|e| panic!("{what}: {e}"));
                    assert_eq!(enc.kind(), CodecKind::LhdcV5);
                    assert_eq!(enc.pcm_format(), if bits == 16 { PcmFormat::I16 } else { PcmFormat::I32 });
                    let block = match sample_rate {
                        192000 => 960,
                        96000 => 480,
                        _ => 240,
                    };
                    assert_eq!(enc.pcm_frames_per_encode(), block);
                    assert_eq!(enc.pcm_frames_per_codec_frame(), block);
                    assert_eq!(enc.max_output_bytes(), usize::from(mtu) - 2);
                    assert!(!enc.abr_enabled());
                    // A 1000 kbps frame at 44.1 kHz (682 bytes) does not fit
                    // the smallest MTU: capped at 900 kbps
                    let kbps = if sample_rate == 44100 && quality == Quality::High && mtu == MTUS[0] {
                        900
                    } else {
                        nominal_kbps(sample_rate, quality)
                    };
                    assert_eq!(enc.bitrate_kbps(), kbps, "{what}");

                    let stats = run(&mut enc, &cfg, 2.0);
                    assert!(stats.frames > 0, "{what}: no output");
                    assert_stream_bitrate(&stats, sample_rate, block, kbps, &what);
                    // 2 seconds in 5 ms frames, less what the encoder holds
                    let expected_frames = 2 * u64::from(sample_rate) / block as u64;
                    assert!(stats.frames + u64::from(stats.max_frames_per_packet) >= expected_frames, "{what}");
                    assert_eq!(enc.bitrate_kbps(), kbps, "{what}");
                }
            }
        }
    }
}

#[test]
fn mono_is_duplicated() {
    for sample_rate in [48000, 96000] {
        let cfg = EncoderConfig { channels: 1, ..config(sample_rate, Quality::Standard, 24, MTUS[0]) };
        let mut enc = LhdcV5Encoder::new(&cfg).unwrap();
        let block = enc.pcm_frames_per_encode();
        let stats = run(&mut enc, &cfg, 1.0);
        assert!(stats.frames > 0);
        assert_stream_bitrate(&stats, sample_rate, block, 500, "mono");
    }
}

#[test]
fn create_returns_lhdcv5() {
    let enc = codec::create(CodecKind::LhdcV5, &config(96000, Quality::High, 24, MTUS[1])).unwrap();
    assert_eq!(enc.kind(), CodecKind::LhdcV5);
    assert_eq!(enc.bitrate_kbps(), 1000);
    assert_eq!(enc.pcm_frames_per_encode(), 480);
}

#[test]
fn encoder_is_send() {
    fn send<T: Send>() {}
    send::<LhdcV5Encoder>();
}

#[test]
fn peer_bitrate_limits() {
    let with = |p7: u8, quality: Quality, abr: bool| {
        let cfg = EncoderConfig {
            peer: PeerParams { lhdcv5_bitrate_limits: p7, ..PeerParams::default() },
            abr,
            ..config(48000, quality, 16, MTUS[1])
        };
        LhdcV5Encoder::new(&cfg).unwrap().bitrate_kbps()
    };
    // P7[5:4]: maximum 400 / 500 / 900 kbps, or none
    assert_eq!(with(0x10, Quality::High, false), 400);
    assert_eq!(with(0x20, Quality::High, false), 500);
    assert_eq!(with(0x30, Quality::High, false), 900);
    assert_eq!(with(0x30, Quality::MediumHigh, false), 900);
    assert_eq!(with(0x30, Quality::Standard, false), 500);
    assert_eq!(with(0x00, Quality::High, false), 1000);
    // P7[7:6]: minimum 160 / 256 / 400 kbps (above Mobile's 400 only at 0xC0)
    assert_eq!(with(0xC0, Quality::Mobile, false), 400);
    assert_eq!(with(0xD0, Quality::High, false), 400);
    // ABR starts at 400 kbps whatever the quality mode
    assert_eq!(with(0x00, Quality::High, true), 400);
    assert_eq!(with(0x00, Quality::Mobile, true), 400);

    // The device's maximum is told when it lowers the quality's bit rate,
    // never under ABR (it stays at 400 kbps or below, as on Android)
    let capped = |p7: u8, quality: Quality, abr: bool| {
        let cfg = EncoderConfig {
            peer: PeerParams { lhdcv5_bitrate_limits: p7, ..PeerParams::default() },
            abr,
            ..config(48000, quality, 16, MTUS[1])
        };
        codec::create(CodecKind::LhdcV5, &cfg).unwrap().bitrate_capped()
    };
    assert!(capped(0x10, Quality::High, false));
    assert!(capped(0x20, Quality::MediumHigh, false));
    assert!(capped(0x30, Quality::High, false));
    assert!(!capped(0x30, Quality::MediumHigh, false)); // 900 asked, 900 allowed
    assert!(!capped(0x10, Quality::Mobile, false));
    assert!(!capped(0x00, Quality::High, false));
    assert!(!capped(0x10, Quality::High, true));
    assert!(!capped(0x00, Quality::High, true));

    // The limits hold while encoding
    let cfg = EncoderConfig {
        peer: PeerParams { lhdcv5_bitrate_limits: 0x20, ..PeerParams::default() },
        ..config(48000, Quality::High, 16, MTUS[1])
    };
    let mut enc = LhdcV5Encoder::new(&cfg).unwrap();
    let stats = run(&mut enc, &cfg, 1.0);
    assert_stream_bitrate(&stats, 48000, 240, 500, "max 500");
}

#[test]
fn invalid_configurations_are_rejected() {
    let ok = config(48000, Quality::High, 16, MTUS[0]);
    let bad = [
        EncoderConfig { sample_rate: 32000, ..ok },
        EncoderConfig { sample_rate: 88200, ..ok },
        EncoderConfig { channels: 0, ..ok },
        EncoderConfig { channels: 3, ..ok },
        EncoderConfig { bits_per_sample: 8, ..ok },
        EncoderConfig { bits_per_sample: 32, ..ok },
        // 299 bytes for the encoder: below LHDC_MTU_MIN (300)
        EncoderConfig { mtu: 301, ..ok },
        EncoderConfig { mtu: 0, ..ok },
    ];
    for cfg in bad {
        match LhdcV5Encoder::new(&cfg) {
            Err(EncoderError::Unsupported { codec: CodecKind::LhdcV5, .. }) => {}
            Err(e) => panic!("{cfg:?}: unexpected error {e}"),
            Ok(_) => panic!("{cfg:?} accepted"),
        }
    }
    assert!(LhdcV5Encoder::new(&EncoderConfig { mtu: 302, ..ok }).is_ok());
}

#[test]
fn small_mtu_still_fits() {
    // The smallest MTU the encoder takes: frames shrink to fit
    let cfg = config(48000, Quality::Mobile, 16, 302);
    let mut enc = LhdcV5Encoder::new(&cfg).unwrap();
    let stats = run(&mut enc, &cfg, 1.0);
    assert!(stats.frames > 0);
}

#[test]
fn invalid_input_is_rejected() {
    let cfg = config(48000, Quality::High, 16, MTUS[0]);
    let mut enc = LhdcV5Encoder::new(&cfg).unwrap();
    let mut out = vec![0u8; enc.max_output_bytes()];
    assert!(matches!(
        enc.encode(Pcm::I16(&[0; 240]), &mut out),
        Err(EncoderError::InputLength { expected: 480, got: 240 })
    ));
    assert!(matches!(enc.encode(Pcm::I32(&[0; 480]), &mut out), Err(EncoderError::Encode { .. })));
    assert!(matches!(
        enc.encode(Pcm::I16(&[0; 480]), &mut out[..100]),
        Err(EncoderError::OutputTooSmall { have: 100, need: 665 })
    ));
    assert!(enc.encode(Pcm::I16(&[0; 480]), &mut out).is_ok());
}

#[test]
fn abr_lowers_the_bitrate_under_a_deep_queue_and_raises_it_back() {
    for (sample_rate, floor) in [(44100, 160), (48000, 160), (96000, 256), (192000, 256)] {
        let cfg = EncoderConfig { abr: true, ..config(sample_rate, Quality::High, 24, MTUS[1]) };
        let mut enc = LhdcV5Encoder::new(&cfg).unwrap();
        assert!(enc.abr_enabled());
        assert_eq!(enc.bitrate_kbps(), 400);

        // A queue that does not drain: down to the floor after 4 packets
        for _ in 0..5 {
            enc.abr_adjust(3);
        }
        assert_eq!(enc.bitrate_kbps(), floor, "{sample_rate}");
        // The encoder follows
        let block = enc.pcm_frames_per_encode();
        let mut clock = Clock::default();
        let stats = run_on(&mut enc, &cfg, 1.0, &mut clock);
        assert_stream_bitrate(&stats, sample_rate, block, floor, "ABR floor");

        // An empty queue: one stage up per 3000 packets, back to 400 kbps
        let mut ticks = 0;
        let mut seen = vec![floor];
        while enc.bitrate_kbps() != 400 {
            enc.abr_adjust(0);
            ticks += 1;
            if seen.last() != Some(&enc.bitrate_kbps()) {
                seen.push(enc.bitrate_kbps());
            }
            assert!(ticks < 20 * 3000, "{sample_rate}: still at {} kbps", enc.bitrate_kbps());
        }
        assert!(seen.windows(2).all(|w| w[1] > w[0]), "{sample_rate}: {seen:?}");
        assert!(ticks >= 3000 * (seen.len() - 1), "{sample_rate}: {ticks} ticks for {seen:?}");
        let stats = run_on(&mut enc, &cfg, 1.0, &mut clock);
        assert_stream_bitrate(&stats, sample_rate, block, 400, "ABR back up");

        // A single busy packet in an up window holds the rate
        for _ in 0..5 {
            enc.abr_adjust(3);
        }
        assert_eq!(enc.bitrate_kbps(), floor);
        for i in 0..2 * 3000 {
            enc.abr_adjust(u32::from(i == 4000));
        }
        assert_eq!(enc.bitrate_kbps(), floor, "{sample_rate}");
    }
}

#[test]
fn abr_adjust_is_a_no_op_without_abr() {
    let cfg = config(48000, Quality::Standard, 16, MTUS[0]);
    let mut enc = LhdcV5Encoder::new(&cfg).unwrap();
    for _ in 0..100 {
        enc.abr_adjust(30);
    }
    assert_eq!(enc.bitrate_kbps(), 500);
}

#[test]
fn abr_reset_goes_back_to_the_top_of_the_table() {
    let cfg = EncoderConfig { abr: true, ..config(48000, Quality::High, 24, MTUS[1]) };
    let mut enc = LhdcV5Encoder::new(&cfg).unwrap();
    assert!(enc.abr_active());
    for _ in 0..5 {
        enc.abr_adjust(3);
    }
    assert_eq!(enc.bitrate_kbps(), 160);
    enc.abr_reset();
    assert_eq!(enc.bitrate_kbps(), 400);
    // The counters started over: 4 packets again before it drops
    for _ in 0..4 {
        enc.abr_adjust(3);
    }
    assert_eq!(enc.bitrate_kbps(), 400);
    enc.abr_adjust(3);
    assert_eq!(enc.bitrate_kbps(), 160);

    let fixed = LhdcV5Encoder::new(&config(48000, Quality::Standard, 16, MTUS[0])).unwrap();
    assert!(!fixed.abr_active());
}
