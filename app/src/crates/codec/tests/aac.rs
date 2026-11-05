//! AAC: encode a 1 kHz (left) / 1.5 kHz (right) tone at 44.1 / 48 kHz, mono
//! and stereo, every quality; check the LATM framing (AudioMuxElement with
//! in-band StreamMuxConfig), frame counting / timestamps, bit rates, the MTU
//! limit and rejected configurations. Decoding the result is tested in
//! fdkaac-sys (tests/aac_round_trip.rs), which has the decoder feature.

#![cfg(feature = "aac")]

use codec::aac::AacEncoder;
use codec::{CodecKind, Encoder, EncoderConfig, EncoderError, Pcm, PcmFormat, PeerParams, Quality};
use std::f64::consts::PI;

const SECONDS: usize = 3;

fn config(sample_rate: u32, channels: u32, quality: Quality, mtu: u16) -> EncoderConfig {
    EncoderConfig { sample_rate, channels, quality, mtu, bits_per_sample: 16, abr: false, peer: PeerParams::default() }
}

fn tone(n: usize, ch: usize, sample_rate: u32) -> i16 {
    let hz = if ch == 0 { 1000.0 } else { 1500.0 };
    (0.5 * 32767.0 * (2.0 * PI * hz * n as f64 / f64::from(sample_rate)).sin()).round() as i16
}

struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Bits<'_> {
    fn read(&mut self, n: usize) -> u32 {
        let mut v = 0;
        for _ in 0..n {
            let byte = self.data[self.pos / 8];
            v = (v << 1) | u32::from((byte >> (7 - self.pos % 8)) & 1);
            self.pos += 1;
        }
        v
    }
}

/// What one AudioMuxElement holds
#[derive(Debug)]
struct Latm {
    /// StreamMuxConfig present (useSameStreamMux = 0)
    smc: bool,
    payload_len: usize,
}

/// LatmGetValue(): bytesForValue, then that many bytes plus one
fn latm_value(b: &mut Bits) -> u32 {
    let bytes = b.read(2) + 1;
    b.read(8 * bytes as usize)
}

/// Parses one AudioMuxElement(muxConfigPresent = 1) of ISO/IEC 14496-3
/// 1.7.3 as fdk-aac writes it for AAC-LC, checking every StreamMuxConfig
/// field (AudioMuxVersion `version`), that the payload length covers the
/// rest of the packet and that the padding is zero
fn parse_latm(packet: &[u8], sample_rate: u32, channels: u32, version: u32) -> Latm {
    let mut b = Bits { data: packet, pos: 0 };
    let smc = b.read(1) == 0; // useSameStreamMux
    if smc {
        assert_eq!(b.read(1), version, "audioMuxVersion");
        if version == 1 {
            assert_eq!(b.read(1), 0, "audioMuxVersionA");
            latm_value(&mut b); // taraBufferFullness
        }
        assert_eq!(b.read(1), 1, "allStreamsSameTimeFraming");
        assert_eq!(b.read(6), 0, "numSubFrames");
        assert_eq!(b.read(4), 0, "numProgram");
        assert_eq!(b.read(3), 0, "numLayer");
        // AudioSpecificConfig: inline with AudioMuxVersion 0, after its
        // length in bits with 1 (which may hold more than the fields here)
        let asc_end = if version == 1 { Some(latm_value(&mut b) as usize + b.pos) } else { None };
        assert_eq!(b.read(5), 2, "audioObjectType AAC-LC");
        let sf_index = if sample_rate == 48000 { 3 } else { 4 };
        assert_eq!(b.read(4), sf_index, "samplingFrequencyIndex");
        assert_eq!(b.read(4), channels, "channelConfiguration");
        assert_eq!(b.read(1), 0, "frameLengthFlag (1024)");
        assert_eq!(b.read(1), 0, "dependsOnCoreCoder");
        assert_eq!(b.read(1), 0, "extensionFlag");
        if let Some(end) = asc_end {
            assert!(end >= b.pos, "ASC length");
            b.pos = end;
        }
        assert_eq!(b.read(3), 0, "frameLengthType");
        b.read(8); // latmBufferFullness
        assert_eq!(b.read(1), 0, "otherDataPresent");
        assert_eq!(b.read(1), 0, "crcCheckPresent");
    }
    // PayloadLengthInfo
    let mut payload_len = 0;
    loop {
        let tmp = b.read(8) as usize;
        payload_len += tmp;
        if tmp != 255 {
            break;
        }
    }
    // PayloadMux, then byte alignment
    let end = b.pos + payload_len * 8;
    assert_eq!(end.div_ceil(8), packet.len(), "payload length {payload_len} vs packet {} bytes", packet.len());
    b.pos = end;
    let pad = packet.len() * 8 - end;
    assert_eq!(b.read(pad), 0, "alignment bits");
    Latm { smc, payload_len }
}

/// Encodes SECONDS of the tone and checks framing and frame counting.
/// Returns the packets.
fn encode_tone(cfg: &EncoderConfig) -> (AacEncoder, Vec<Vec<u8>>) {
    let mut enc = AacEncoder::new(cfg).unwrap();
    let ch = cfg.channels as usize;
    let per_call = enc.pcm_frames_per_encode();
    assert_eq!(per_call, 1024);
    assert_eq!(enc.pcm_frames_per_codec_frame(), 1024);
    let calls = SECONDS * cfg.sample_rate as usize / per_call;
    let mut out = vec![0u8; enc.max_output_bytes()];
    let mut pcm = vec![0i16; per_call * ch];
    let mut packets = Vec::new();
    let mut timestamp = 0usize;
    for call in 0..calls {
        for f in 0..per_call {
            for c in 0..ch {
                pcm[f * ch + c] = tone(call * per_call + f, c, cfg.sample_rate);
            }
        }
        let e = enc.encode(Pcm::I16(&pcm), &mut out).unwrap();
        // One access unit per call: the input is exactly one frame
        assert_eq!(e.frames, 1, "call {call}");
        assert!(e.bytes > 0 && e.bytes <= usize::from(cfg.mtu), "call {call}: {} bytes, MTU {}", e.bytes, cfg.mtu);
        // AudioMuxVersion 1, as Android and PipeWire (issue #87)
        let latm = parse_latm(&out[..e.bytes], cfg.sample_rate, cfg.channels, 1);
        // StreamMuxConfig in every frame (issue #87)
        assert!(latm.smc, "no StreamMuxConfig in frame {}", packets.len());
        assert!(latm.payload_len > 0);
        timestamp += e.frames as usize * enc.pcm_frames_per_codec_frame();
        packets.push(out[..e.bytes].to_vec());
    }
    // The RTP timestamp (1024 per frame) follows the input exactly
    assert_eq!(timestamp, calls * per_call);
    (enc, packets)
}

fn average_kbps(packets: &[Vec<u8>], sample_rate: u32) -> f64 {
    let bytes: usize = packets.iter().map(Vec::len).sum();
    bytes as f64 * 8.0 / (packets.len() as f64 * 1024.0 / f64::from(sample_rate)) / 1000.0
}

#[test]
fn tone_all_configurations() {
    for rate in [44100, 48000] {
        for channels in [1, 2] {
            for (quality, kbps) in
                [(Quality::High, 256), (Quality::Standard, 192), (Quality::Mobile, 128), (Quality::MediumHigh, 256)]
            {
                let what = format!("{rate} Hz {channels} ch {quality:?}");
                // A large media payload: the quality's bit rate applies as is
                let cfg = config(rate, channels, quality, 1000);
                let (enc, packets) = encode_tone(&cfg);
                assert_eq!(enc.kind(), CodecKind::Aac);
                assert_eq!(enc.pcm_format(), PcmFormat::I16);
                assert_eq!(enc.bitrate_kbps(), kbps, "{what}");
                // CBR: the stream carries the bit rate (LATM headers included)
                let real = average_kbps(&packets, rate);
                assert!((real - f64::from(kbps)).abs() < f64::from(kbps) * 0.03, "{what}: {real} kbps");
            }
        }
    }
}

#[test]
fn mtu_limits_bitrate_and_frame_size() {
    // 660 bytes: a 672-byte L2CAP MTU minus the RTP header (BlueZ sinks).
    // 256 kbps would average 683-byte frames; v1 caps the bit rate and the
    // peak bit rate to the payload so every frame fits.
    for rate in [44100, 48000] {
        let cfg = config(rate, 2, Quality::High, 660);
        let (enc, packets) = encode_tone(&cfg);
        let cap = 660 * 8 * rate / 1024 / 1000;
        assert_eq!(enc.bitrate_kbps(), cap);
        assert!(packets.iter().all(|p| p.len() <= 660));
        let real = average_kbps(&packets, rate);
        assert!(real <= f64::from(cap) + 1.0, "{real} kbps over the {cap} kbps cap");
    }
    // A small payload with a bit rate fdk-aac still accepts
    let cfg = config(48000, 2, Quality::High, 300);
    let (enc, packets) = encode_tone(&cfg);
    assert_eq!(enc.bitrate_kbps(), 300 * 8 * 48000 / 1024 / 1000);
    assert!(packets.iter().all(|p| p.len() <= 300));
}

#[test]
fn rejects_invalid_configurations() {
    let unsupported = |cfg: EncoderConfig| {
        assert!(matches!(AacEncoder::new(&cfg), Err(EncoderError::Unsupported { .. })), "{cfg:?} accepted");
    };
    unsupported(config(32000, 2, Quality::High, 1000));
    unsupported(config(96000, 2, Quality::High, 1000));
    unsupported(config(48000, 0, Quality::High, 1000));
    unsupported(config(48000, 3, Quality::High, 1000));
    let mut bits24 = config(48000, 2, Quality::High, 1000);
    bits24.bits_per_sample = 24;
    unsupported(bits24);
    // No room for a frame at the lowest bit rate fdk-aac takes
    let tiny = config(48000, 2, Quality::High, 10);
    assert!(
        matches!(AacEncoder::new(&tiny), Err(EncoderError::Unsupported { .. } | EncoderError::Init { .. })),
        "MTU 10 accepted"
    );
    assert!(AacEncoder::new(&config(48000, 2, Quality::High, 0)).is_err());
}

#[test]
fn rejects_wrong_input() {
    let mut enc = AacEncoder::new(&config(48000, 2, Quality::High, 1000)).unwrap();
    let mut out = vec![0u8; enc.max_output_bytes()];
    assert!(matches!(
        enc.encode(Pcm::I16(&[0; 2047]), &mut out),
        Err(EncoderError::InputLength { expected: 2048, got: 2047 })
    ));
    assert!(matches!(enc.encode(Pcm::I32(&[0; 2048]), &mut out), Err(EncoderError::Encode { .. })));
    let need = enc.max_output_bytes();
    assert!(matches!(
        enc.encode(Pcm::I16(&[0; 2048]), &mut out[..need - 1]),
        Err(EncoderError::OutputTooSmall { have, need: n }) if have == need - 1 && n == need
    ));
    // Still works after the rejected calls
    let e = enc.encode(Pcm::I16(&[0; 2048]), &mut out).unwrap();
    assert_eq!(e.frames, 1);
}

#[test]
fn encoder_is_send() {
    fn send<T: Send>() {}
    send::<AacEncoder>();
    let mut enc = AacEncoder::new(&config(44100, 1, Quality::Mobile, 1000)).unwrap();
    std::thread::spawn(move || {
        let mut out = vec![0u8; enc.max_output_bytes()];
        assert_eq!(enc.encode(Pcm::I16(&[0; 1024]), &mut out).unwrap().frames, 1);
    })
    .join()
    .unwrap();
}

#[test]
fn create_dispatches() {
    let enc = codec::create(CodecKind::Aac, &config(44100, 2, Quality::Standard, 1000)).unwrap();
    assert_eq!(enc.kind(), CodecKind::Aac);
    assert_eq!(enc.bitrate_kbps(), 192);
}

/// The sink's largest bit rate (the configuration's) caps every quality
#[test]
fn sink_bit_rate_limit() {
    for (limit, quality, kbps) in
        [(128_000, Quality::High, 128), (200_000, Quality::Standard, 192), (0, Quality::High, 256)]
    {
        let cfg = EncoderConfig {
            peer: PeerParams { aac_max_bitrate: limit, ..PeerParams::default() },
            ..config(44100, 2, quality, 1000)
        };
        let enc = AacEncoder::new(&cfg).unwrap();
        assert_eq!(enc.bitrate_kbps(), kbps, "limit {limit}, {quality:?}");
    }
}

/// Variable bit rate: every access unit still fits a 672-byte L2CAP MTU's
/// media payload (660 bytes), also for noise
#[test]
fn vbr_frames_fit_the_media_payload() {
    for quality in [Quality::High, Quality::Standard, Quality::Mobile] {
        let cfg = EncoderConfig { abr: true, ..config(44100, 2, quality, 660) };
        let mut e = AacEncoder::new(&cfg).unwrap();
        let n = e.pcm_frames_per_encode() * 2;
        let mut out = vec![0u8; e.max_output_bytes()];
        let mut seed = 7u32;
        let mut biggest = 0;
        for _ in 0..200 {
            let pcm: Vec<i16> = (0..n)
                .map(|_| {
                    seed ^= seed << 13;
                    seed ^= seed >> 17;
                    seed ^= seed << 5;
                    (seed >> 16) as i16
                })
                .collect();
            biggest = biggest.max(e.encode(Pcm::I16(&pcm), &mut out).unwrap().bytes);
        }
        assert!(biggest <= 660, "{quality:?}: a frame of {biggest} bytes");
    }
}
