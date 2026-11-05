//! aptX, aptX HD and aptX Low Latency encoders (libopenaptx 0.2.0).
//!
//! aptX (Qualcomm/APT vendor 0x4F, codec 0x01) is a fixed 4:1 ADPCM codec:
//! every group of 4 stereo samples becomes 4 bytes (aptX, 352.8 / 384 kbps at
//! 44.1 / 48 kHz) or 6 bytes (aptX HD, 24-bit, 529.2 / 576 kbps). aptX Low
//! Latency is the same bitstream as aptX; only the codec configuration and the
//! smaller packets (see [`aptx_ll_max_packet_bytes`]) differ. The encoder is
//! libopenaptx, an open implementation derived from FFmpeg, not Qualcomm code;
//! it is LGPL-2.1-or-later and stays in the separate `openaptx-sys` crate
//! (static library of its own, pinned to 0.2.0: later releases are
//! GPL-3.0-or-later). aptX is a Qualcomm trademark and may be covered by
//! patents in some jurisdictions (THIRD_PARTY_LICENSES.md §2).

use openaptx_sys::{AptxContext, GROUP_FRAMES, PCM_GROUP_BYTES};

use crate::{CodecKind, Encoded, Encoder, EncoderConfig, EncoderError, Pcm, PcmFormat};

/// PCM frames per encode call as in v1: 32 aptX groups, i.e. 128 bytes
/// (aptX / LL) or 192 bytes (HD) per call instead of 4 or 6, which would cost
/// a whole packet (and for aptX HD an RTP header) per group
const FRAMES_PER_ENCODE: usize = 128;

/// aptX LL packets stay at or below ~7.5 ms of audio like PipeWire, so the
/// sink can keep its small (low latency) buffer. aptX LL carries 1 byte per
/// sample frame (4 bytes per 4 frames), so this is also the byte limit,
/// rounded down to whole 4-byte groups: 328 bytes at 44.1 kHz, 360 at 48 kHz.
/// (Applied by the packetizer, as in v1's encode thread; the encoder itself
/// emits 128 bytes per call.)
pub fn aptx_ll_max_packet_bytes(sample_rate: u32) -> usize {
    (sample_rate as usize * 75 / 10000) & !3
}

/// Encoder for [`CodecKind::Aptx`], [`CodecKind::AptxHd`] and
/// [`CodecKind::AptxLl`]
pub struct AptxEncoder {
    kind: CodecKind,
    ctx: AptxContext,
    format: PcmFormat,
    channels: usize,
    sample_rate: u32,
    /// aptX groups per encode call
    groups: usize,
    /// Encoded bytes per group (4 or 6)
    group_bytes: usize,
    /// Packed 24-bit input for libopenaptx, `groups` x 24 bytes (allocated
    /// once: encode() must not allocate)
    packed: Vec<u8>,
}

impl AptxEncoder {
    /// `kind` must be one of the aptX family. Takes 44.1 or 48 kHz, 1 or 2
    /// channels (mono is duplicated to both channels: aptX is always sent as
    /// stereo) and 16-bit PCM, or with `bits_per_sample` 24 32-bit
    /// MSB-aligned PCM of which the top 24 bits are encoded (what v1 does for
    /// aptX HD unless the profile asks for 16 bits).
    pub fn new(kind: CodecKind, config: &EncoderConfig) -> Result<AptxEncoder, EncoderError> {
        let unsupported = |what: String| Err(EncoderError::Unsupported { codec: kind, what });
        let hd = match kind {
            CodecKind::Aptx | CodecKind::AptxLl => false,
            CodecKind::AptxHd => true,
            _ => return unsupported("not an aptX codec".into()),
        };
        if config.sample_rate != 44100 && config.sample_rate != 48000 {
            return unsupported(format!("sample rate {} (44100 or 48000)", config.sample_rate));
        }
        if config.channels != 1 && config.channels != 2 {
            return unsupported(format!("{} channels (1 or 2)", config.channels));
        }
        let format = match config.bits_per_sample {
            16 => PcmFormat::I16,
            24 => PcmFormat::I32,
            bits => return unsupported(format!("{bits}-bit PCM (16 or 24)")),
        };
        let group_bytes = openaptx_sys::encoded_group_bytes(hd);
        // v1 capped one call's output at the media MTU (and dropped the rest
        // of the input). Here the call itself shrinks so that its output
        // fits: an MTU below 128 / 192 bytes is not seen in practice, but
        // nothing is lost if it is. For aptX and aptX LL, sent without an RTP
        // header, this is conservative (12 more bytes would fit), as in v1.
        let groups = (FRAMES_PER_ENCODE / GROUP_FRAMES).min(usize::from(config.mtu) / group_bytes);
        if groups == 0 {
            return unsupported(format!("media MTU {} below one {group_bytes}-byte aptX frame", config.mtu));
        }
        tracing::debug!(
            "{kind}: {groups} groups of {group_bytes} bytes per call (at most {} by frames, {} by the mtu), {format:?} input",
            FRAMES_PER_ENCODE / GROUP_FRAMES,
            usize::from(config.mtu) / group_bytes
        );
        let ctx = AptxContext::new(hd)
            .ok_or_else(|| EncoderError::Init { codec: kind, what: "aptx_init failed (out of memory)".into() })?;
        let encoder = AptxEncoder {
            kind,
            ctx,
            format,
            channels: config.channels as usize,
            sample_rate: config.sample_rate,
            groups,
            group_bytes,
            packed: vec![0; groups * PCM_GROUP_BYTES],
        };
        tracing::info!(
            "{}: rate={} in_ch={} bits={} mtu={} bytes/call={} bitrate={}kbps",
            kind,
            config.sample_rate,
            config.channels,
            config.bits_per_sample,
            config.mtu,
            encoder.max_output_bytes(),
            encoder.bitrate_kbps()
        );
        Ok(encoder)
    }

    /// Packs the input as libopenaptx wants it: 24-bit signed little-endian,
    /// LLLRRR per sample frame (16-bit input << 8, 32-bit MSB-aligned >> 8;
    /// mono duplicated)
    fn pack<T: Copy>(&mut self, pcm: &[T], to24: impl Fn(T) -> i32) {
        let channels = self.channels;
        for (frame, out) in pcm.chunks_exact(channels).zip(self.packed.as_chunks_mut::<6>().0) {
            let l = to24(frame[0]).to_le_bytes();
            let r = if channels >= 2 { to24(frame[1]).to_le_bytes() } else { l };
            out[..3].copy_from_slice(&l[..3]);
            out[3..].copy_from_slice(&r[..3]);
        }
    }
}

impl Encoder for AptxEncoder {
    fn kind(&self) -> CodecKind {
        self.kind
    }

    fn pcm_format(&self) -> PcmFormat {
        self.format
    }

    fn pcm_frames_per_encode(&self) -> usize {
        self.groups * GROUP_FRAMES
    }

    /// [`Encoded::frames`] counts aptX groups of 4 sample frames (as v1)
    fn pcm_frames_per_codec_frame(&self) -> usize {
        GROUP_FRAMES
    }

    fn max_output_bytes(&self) -> usize {
        self.groups * self.group_bytes
    }

    fn encode(&mut self, pcm: Pcm<'_>, out: &mut [u8]) -> Result<Encoded, EncoderError> {
        let expected = self.pcm_frames_per_encode() * self.channels;
        if pcm.samples() != expected {
            return Err(EncoderError::InputLength { expected, got: pcm.samples() });
        }
        let need = self.max_output_bytes();
        if out.len() < need {
            return Err(EncoderError::OutputTooSmall { have: out.len(), need });
        }
        match (pcm, self.format) {
            (Pcm::I16(s), PcmFormat::I16) => self.pack(s, |v| i32::from(v) << 8),
            (Pcm::I32(s), PcmFormat::I32) => self.pack(s, |v| v >> 8),
            _ => {
                return Err(EncoderError::Encode {
                    codec: self.kind,
                    what: format!("PCM is not {:?} as configured", self.format),
                });
            }
        }
        let (consumed, written) = self.ctx.encode(&self.packed, &mut out[..need]);
        if consumed != self.packed.len() || written != need {
            return Err(EncoderError::Encode {
                codec: self.kind,
                what: format!("aptx_encode took {consumed} of {} bytes, wrote {written}", self.packed.len()),
            });
        }
        Ok(Encoded { bytes: written, frames: self.groups as u32 })
    }

    /// The fixed rate of the bitstream: 4 bytes (HD 6) per 4 sample frames
    fn bitrate_kbps(&self) -> u32 {
        self.sample_rate * (self.group_bytes as u32 * 8 / GROUP_FRAMES as u32) / 1000
    }
}
