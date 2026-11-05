//! Opus: Android's A2DP Opus vendor codec (vendor 0x000000E0 Google, codec
//! 0x0001; AOSP system/stack/a2dp/a2dp_vendor_opus*.cc, Android 13+),
//! encoded by libopus (crate `opus-sys`, BSD-3-Clause).
//!
//! The stream is 48 kHz, mono or stereo, in 10 or 20 ms frames (20 ms is
//! Android's default and the only duration it has a bit rate for). Each
//! media packet carries one Opus packet of one frame behind a 1-byte media
//! payload header with the frame count in its low 4 bits
//! (A2DP_BuildMediaPayloadHeaderOpus; PipeWire's decoder rejects any other
//! count), with an RTP header in front.
//!
//! Encoder setup as AOSP a2dp_vendor_opus_encoder.cc: OPUS_APPLICATION_AUDIO
//! and libopus' default constrained VBR (Android never changes it). Android
//! asks for 256 kbps stereo / 128 kbps mono (A2DP_VendorGetBitRateOpus);
//! PipeWire (a2dp-codec-opus-g.c) uses the same 128 kbps per channel. Here
//! that is Quality::High, with Standard 192 / Mobile 128 kbps (stereo; half
//! for mono), as for AAC. The bit rate is capped so a frame fits the media
//! payload, and every packet is encoded with that payload as its limit, so
//! a VBR peak never exceeds the MTU. Complexity 10 (libopus' default, as
//! PipeWire; Android uses 5 to save phone CPU).
//!
//! PipeWire's own "Opus 05" (vendor 0x000005F1, codec 0x1005; PipeWire
//! a2dp-codec-opus.c, `opus_05`) streams the same way for stereo music: an
//! Opus multistream packet of one stream (one coupled stream for stereo,
//! which is a plain Opus packet) per media packet, behind the same 1-byte
//! header (PipeWire's rtp_payload: frame count 1, no fragmentation, which
//! its decoder takes as is), with an RTP header. PipeWire's encoder uses
//! OPUS_APPLICATION_AUDIO and libopus' default constrained VBR too, from 96
//! to 320 kbps; here the quality levels are Android Opus' and the target
//! is also capped at the bit rate configured (the sink's limit, units of
//! 1024 bps). PipeWire adapts the bit rate to the send queue; A2DPWB does
//! not (no ABR).

use opus_sys::{OpusEnc, ffi};

use crate::{CodecKind, Encoded, Encoder, EncoderConfig, EncoderError, Pcm, PcmFormat, Quality};

/// The media payload header in front of the Opus packet
/// (A2DP_OPUS_MPL_HDR_LEN)
pub const MPL_HDR_LEN: usize = 1;

/// The only rate of the A2DP Opus codec (A2DP_OPUS_SAMPLING_FREQ_48000)
pub const SAMPLE_RATE: u32 = 48000;

/// Largest Opus packet of one frame (RFC 6716 §3.2.1)
const MAX_PACKET: usize = 1275;

/// libopus' complexity range is 0..10
const COMPLEXITY: i32 = 10;

/// Target bit rate per quality, stereo (mono: half)
fn quality_bitrate(quality: Quality, channels: u32) -> u32 {
    let stereo = match quality {
        Quality::High | Quality::MediumHigh | Quality::LowLatency => 256_000,
        Quality::Standard => 192_000,
        Quality::Mobile => 128_000,
    };
    if channels == 1 { stereo / 2 } else { stereo }
}

pub struct OpusEncoder {
    /// Opus or Opus 05
    kind: CodecKind,
    enc: OpusEnc,
    /// Channels of the input
    in_channels: usize,
    /// Channels encoded (the stream's): mono input is duplicated for stereo
    channels: usize,
    /// PCM frames per Opus frame (480 / 960)
    frame_size: usize,
    /// Largest Opus packet: what fits the media payload after its header
    max_packet: usize,
    bitrate: u32,
    /// Input repacked when mono is duplicated to stereo
    stereo: Vec<i16>,
}

impl OpusEncoder {
    /// An encoder for `kind`: [`CodecKind::Opus`] or [`CodecKind::Opus05`]
    pub fn new(kind: CodecKind, config: &EncoderConfig) -> Result<OpusEncoder, EncoderError> {
        let unsupported = |what: String| EncoderError::Unsupported { codec: kind, what };
        if !kind.is_opus() {
            return Err(unsupported(format!("{kind} is not Opus")));
        }
        if config.sample_rate != SAMPLE_RATE {
            return Err(unsupported(format!("sample rate {} Hz (Opus over A2DP is 48000 only)", config.sample_rate)));
        }
        if !matches!(config.channels, 1 | 2) {
            return Err(unsupported(format!("{} channels", config.channels)));
        }
        if config.bits_per_sample != 16 {
            return Err(unsupported(format!("{}-bit PCM (16)", config.bits_per_sample)));
        }
        let channels = match config.peer.opus_channels {
            0 => config.channels,
            n @ (1 | 2) => u32::from(n),
            n => return Err(unsupported(format!("{n} channels on the wire"))),
        };
        if channels < config.channels {
            return Err(unsupported("stereo input for a mono stream".into()));
        }
        let frame_ms = match config.peer.opus_frame_ms {
            0 | 20 => 20,
            10 => 10,
            n => return Err(unsupported(format!("{n} ms frames (10 or 20)"))),
        };
        let frame_size = (SAMPLE_RATE / 1000 * frame_ms) as usize;

        let max_packet = usize::from(config.mtu).saturating_sub(MPL_HDR_LEN).min(MAX_PACKET);
        if max_packet < 3 {
            return Err(unsupported(format!("media MTU {}", config.mtu)));
        }
        // One frame per packet: cap the target at what the payload carries
        // (as PipeWire); constrained VBR stays close to the target, and the
        // packet limit passed to opus_encode catches the rest
        let max_bitrate = u32::try_from(max_packet as u64 * 8 * 1000 / u64::from(frame_ms)).unwrap_or(u32::MAX);
        let mut bitrate = quality_bitrate(config.quality, channels);
        let configured = config.peer.opus_max_bitrate;
        tracing::debug!(
            "{kind}: {frame_ms} ms frames (configured {}), {channels} ch on the wire (configured {}), bit rate {bitrate} \
             bps by quality {:?}, limits: configuration {configured}, payload {max_bitrate} ({max_packet} bytes)",
            config.peer.opus_frame_ms,
            config.peer.opus_channels,
            config.quality
        );
        if kind == CodecKind::Opus05 && configured > 0 && bitrate > configured {
            tracing::info!("{kind}: bit rate limited to {configured} bps by the configuration");
            bitrate = configured;
        }
        if bitrate > max_bitrate {
            tracing::info!("{kind}: bit rate limited to {max_bitrate} bps by the {}-byte media payload", config.mtu);
            bitrate = max_bitrate;
        }

        let init_err =
            |what: &str, e: opus_sys::Error| EncoderError::Init { codec: kind, what: format!("{what}: {e}") };
        let mut enc =
            OpusEnc::new(SAMPLE_RATE, channels, ffi::OPUS_APPLICATION_AUDIO).map_err(|e| init_err("create", e))?;
        enc.set_bitrate(bitrate as i32).map_err(|e| init_err("OPUS_SET_BITRATE", e))?;
        enc.set(ffi::OPUS_SET_VBR_REQUEST, 1).map_err(|e| init_err("OPUS_SET_VBR", e))?;
        enc.set(ffi::OPUS_SET_VBR_CONSTRAINT_REQUEST, 1).map_err(|e| init_err("OPUS_SET_VBR_CONSTRAINT", e))?;
        enc.set(ffi::OPUS_SET_COMPLEXITY_REQUEST, COMPLEXITY).map_err(|e| init_err("OPUS_SET_COMPLEXITY", e))?;

        tracing::info!(
            "{kind}: initialized ({}, rate={SAMPLE_RATE}, ch={channels}{}, {frame_ms} ms frames, {} kbps CVBR, \
             packets up to {max_packet} bytes, lookahead {} samples)",
            opus_sys::version(),
            if channels as u32 > config.channels { " from mono" } else { "" },
            bitrate / 1000,
            enc.lookahead().unwrap_or(-1)
        );
        Ok(OpusEncoder {
            kind,
            enc,
            in_channels: config.channels as usize,
            channels: channels as usize,
            frame_size,
            max_packet,
            bitrate,
            stereo: if channels as u32 > config.channels { vec![0; frame_size * 2] } else { Vec::new() },
        })
    }
}

impl Encoder for OpusEncoder {
    fn kind(&self) -> CodecKind {
        self.kind
    }

    fn pcm_format(&self) -> PcmFormat {
        PcmFormat::I16
    }

    /// One Opus frame (10 / 20 ms) per call
    fn pcm_frames_per_encode(&self) -> usize {
        self.frame_size
    }

    fn max_output_bytes(&self) -> usize {
        self.max_packet
    }

    fn encode(&mut self, pcm: Pcm<'_>, out: &mut [u8]) -> Result<Encoded, EncoderError> {
        let Pcm::I16(samples) = pcm else {
            return Err(EncoderError::Encode { codec: self.kind, what: "input is not 16-bit PCM".into() });
        };
        let expected = self.frame_size * self.in_channels;
        if samples.len() != expected {
            return Err(EncoderError::InputLength { expected, got: samples.len() });
        }
        if out.len() < self.max_packet {
            return Err(EncoderError::OutputTooSmall { have: out.len(), need: self.max_packet });
        }
        let input: &[i16] = if self.channels > self.in_channels {
            for (dst, &s) in self.stereo.as_chunks_mut::<2>().0.iter_mut().zip(samples) {
                dst[0] = s;
                dst[1] = s;
            }
            &self.stereo
        } else {
            samples
        };
        let n = self
            .enc
            .encode(input, &mut out[..self.max_packet])
            .map_err(|e| EncoderError::Encode { codec: self.kind, what: e.to_string() })?;
        Ok(Encoded { bytes: n, frames: 1 })
    }

    fn bitrate_kbps(&self) -> u32 {
        self.bitrate / 1000
    }
}
