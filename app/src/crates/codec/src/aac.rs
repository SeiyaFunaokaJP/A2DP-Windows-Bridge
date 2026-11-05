//! AAC encoder (Fraunhofer FDK AAC).
//!
//! A2DP's AAC is MPEG-2/4 AAC-LC; each media packet carries one access unit
//! as a LATM AudioMuxElement with the StreamMuxConfig in band
//! (muxConfigPresent = 1, no LOAS sync layer) and no media payload header,
//! as Android and PipeWire send it. The encoder is fdk-aac, under the
//! Fraunhofer FDK AAC licence (free redistribution with the licence text, but
//! no patent licence: AAC is patent-encumbered and licensing it is the
//! distributor's matter); it is built in the separate `fdkaac-sys` crate
//! (THIRD_PARTY_LICENSES.md §3).

use fdkaac_sys::{Encoder as FdkEncoder, ffi};

use crate::{CodecKind, Encoded, Encoder, EncoderConfig, EncoderError, Pcm, PcmFormat, Quality};

const CODEC: CodecKind = CodecKind::Aac;

/// Target bit rate per quality, as v1 (MediumHigh is LHDC V5 only: High)
fn quality_bitrate(quality: Quality) -> u32 {
    match quality {
        Quality::High | Quality::MediumHigh | Quality::LowLatency => 256_000,
        Quality::Standard => 192_000,
        Quality::Mobile => 128_000,
    }
}

/// fdk-aac's variable bit rate mode (3 medium .. 5 very high) per quality
fn vbr_mode(quality: Quality) -> u32 {
    match quality {
        Quality::High | Quality::MediumHigh | Quality::LowLatency => 5,
        Quality::Standard => 4,
        Quality::Mobile => 3,
    }
}

/// AAC-LC encoder for A2DP: 44.1 / 48 kHz, mono or stereo, 16-bit PCM, one
/// 1024-sample access unit per [`Encoder::encode`] call
pub struct AacEncoder {
    enc: FdkEncoder,
    channels: usize,
    /// PCM frames per access unit (1024 for AAC-LC)
    frame_length: usize,
    /// Largest access unit fdk-aac may write (maxOutBufBytes)
    max_out: usize,
    /// Target bit rate in bit/s after the MTU limit
    bitrate: u32,
}

impl AacEncoder {
    pub fn new(config: &EncoderConfig) -> Result<AacEncoder, EncoderError> {
        let unsupported = |what: String| Err(EncoderError::Unsupported { codec: CODEC, what });
        if config.sample_rate != 44100 && config.sample_rate != 48000 {
            return unsupported(format!("sample rate {} (44100 or 48000)", config.sample_rate));
        }
        if config.channels != 1 && config.channels != 2 {
            return unsupported(format!("{} channels (1 or 2)", config.channels));
        }
        // fdk-aac takes 16-bit PCM (INT_PCM); v1 never fed AAC anything else
        if config.bits_per_sample != 16 {
            return unsupported(format!("{}-bit PCM (16)", config.bits_per_sample));
        }

        // Every AAC frame goes out whole in one media packet (no
        // fragmentation), so cap the bit rate and the peak bit rate so a
        // frame fits the media payload (mtu = remote L2CAP MTU minus the RTP
        // header), as PipeWire does. Without the cap 256 kbps frames average
        // 683 bytes: more than the 660 bytes a 672-byte L2CAP MTU leaves
        // (BlueZ sinks, some headphones), and every packet was dropped as
        // oversized.
        let mtu_bitrate =
            u32::try_from(u64::from(config.mtu) * 8 * u64::from(config.sample_rate) / 1024).unwrap_or(u32::MAX);
        // The configuration's bit rate (the sink's largest) caps it too
        let max_bitrate = match config.peer.aac_max_bitrate {
            0 => mtu_bitrate,
            b => b.min(mtu_bitrate),
        };
        let dev = crate::dev::overrides();
        let mut bitrate = match dev.aac_bitrate_kbps {
            0 => quality_bitrate(config.quality),
            k => k * 1000,
        };
        if bitrate > max_bitrate {
            if max_bitrate == mtu_bitrate {
                tracing::info!("AAC: bit rate limited to {max_bitrate} bps by the {}-byte media payload", config.mtu);
            } else {
                tracing::info!("AAC: bit rate limited to {max_bitrate} bps by the sink");
            }
            bitrate = max_bitrate;
        }

        let init_err =
            |what: &str, e: fdkaac_sys::EncError| EncoderError::Init { codec: CODEC, what: format!("{what}: {e}") };
        let mut enc = FdkEncoder::open(0, config.channels).map_err(|e| init_err("aacEncOpen", e))?;
        let channel_mode = if config.channels == 1 { ffi::MODE_1 } else { ffi::MODE_2 };
        // AAC-LC (what A2DP requires), CBR (the default bit rate mode; the
        // codec configuration says VBR off), LATM with in-band
        // StreamMuxConfig in every frame (fdk-aac's default of every 10th
        // frame leaves some sinks silent), afterburner for quality.
        // AudioMuxVersion 1 (fdk-aac's 2: taraBufferFullness and the ASC
        // length in the StreamMuxConfig) and explicit backward-compatible
        // signaling, as Android (a2dp_aac_encoder.cc) and PipeWire
        // (a2dp-codec-aac.c) send it; fdk-aac's default AudioMuxVersion 0
        // left the JBL Tune 510BT silent (issue #87). The developer override
        // aac_latm_v0 brings back the old StreamMuxConfig.
        let vbr = config.abr;
        let mut params = vec![
            // 0 = constant bit rate; 3..5 variable (very high is the
            // High quality's), the sink having listed VBR
            (ffi::AACENC_BITRATEMODE, "AACENC_BITRATEMODE", if vbr { vbr_mode(config.quality) } else { 0 }),
            (ffi::AACENC_AOT, "AACENC_AOT", ffi::AOT_AAC_LC as u32),
            (ffi::AACENC_SAMPLERATE, "AACENC_SAMPLERATE", config.sample_rate),
            (ffi::AACENC_CHANNELMODE, "AACENC_CHANNELMODE", channel_mode as u32),
            (ffi::AACENC_BITRATE, "AACENC_BITRATE", bitrate),
            (ffi::AACENC_TRANSMUX, "AACENC_TRANSMUX", ffi::TT_MP4_LATM_MCP1 as u32),
            (ffi::AACENC_HEADER_PERIOD, "AACENC_HEADER_PERIOD", 1),
            (ffi::AACENC_AUDIOMUXVER, "AACENC_AUDIOMUXVER", if dev.aac_latm_v0 { 0 } else { 2 }),
            (ffi::AACENC_SIGNALING_MODE, "AACENC_SIGNALING_MODE", if dev.aac_latm_v0 { 0 } else { 1 }),
        ];
        if !dev.aac_no_peak {
            params.push((ffi::AACENC_PEAK_BITRATE, "AACENC_PEAK_BITRATE", max_bitrate));
        }
        if !dev.aac_no_afterburner {
            params.push((ffi::AACENC_AFTERBURNER, "AACENC_AFTERBURNER", 1));
        }
        tracing::debug!(
            "AAC: bit rate {bitrate} bps (quality {:?}, dev override {} kbps), mtu cap {mtu_bitrate}, sink cap {}, \
             parameters {:?}",
            config.quality,
            dev.aac_bitrate_kbps,
            config.peer.aac_max_bitrate,
            params.iter().map(|&(_, name, value)| (name, value)).collect::<Vec<_>>()
        );
        for (param, name, value) in params {
            enc.set_param(param, value).map_err(|e| init_err(&format!("{name} = {value}"), e))?;
        }
        enc.init().map_err(|e| init_err("initialization", e))?;
        let info = enc.info().map_err(|e| init_err("aacEncInfo", e))?;

        // fdk-aac raises a bit rate below what the configuration needs; then
        // the frames would no longer fit the media payload
        let effective = enc.param(ffi::AACENC_BITRATE);
        tracing::debug!(
            "AAC: fdk-aac effective bit rate {effective} bps, frame length {}, max output {} bytes",
            info.frameLength,
            info.maxOutBufBytes
        );
        if !vbr && effective > mtu_bitrate {
            return unsupported(format!(
                "media MTU {} allows {mtu_bitrate} bps, below the {effective} bps AAC needs",
                config.mtu
            ));
        }

        let encoder = AacEncoder {
            enc,
            channels: config.channels as usize,
            frame_length: info.frameLength as usize,
            max_out: info.maxOutBufBytes as usize,
            bitrate,
        };
        tracing::info!(
            "AAC: rate={} ch={} mtu={} bitrate={}kbps frame_len={} max_out={}",
            config.sample_rate,
            config.channels,
            config.mtu,
            bitrate / 1000,
            encoder.frame_length,
            encoder.max_out
        );
        Ok(encoder)
    }
}

impl Encoder for AacEncoder {
    fn kind(&self) -> CodecKind {
        CODEC
    }

    fn pcm_format(&self) -> PcmFormat {
        PcmFormat::I16
    }

    /// One access unit: the encoder gets exactly what it encodes per frame,
    /// so it never holds input back and each call yields one frame
    fn pcm_frames_per_encode(&self) -> usize {
        self.frame_length
    }

    fn max_output_bytes(&self) -> usize {
        self.max_out
    }

    fn encode(&mut self, pcm: Pcm<'_>, out: &mut [u8]) -> Result<Encoded, EncoderError> {
        let expected = self.frame_length * self.channels;
        let Pcm::I16(samples) = pcm else {
            return Err(EncoderError::Encode { codec: CODEC, what: "PCM is not I16".into() });
        };
        if samples.len() != expected {
            return Err(EncoderError::InputLength { expected, got: samples.len() });
        }
        if out.len() < self.max_out {
            return Err(EncoderError::OutputTooSmall { have: out.len(), need: self.max_out });
        }
        let result = self
            .enc
            .encode(samples, out)
            .map_err(|e| EncoderError::Encode { codec: CODEC, what: format!("aacEncEncode: {e}") })?;
        if result.in_samples != expected {
            // Not expected with whole frames; samples left out would be lost
            return Err(EncoderError::Encode {
                codec: CODEC,
                what: format!("aacEncEncode took {} of {expected} samples", result.in_samples),
            });
        }
        Ok(Encoded { bytes: result.out_bytes, frames: u32::from(result.out_bytes > 0) })
    }

    fn bitrate_kbps(&self) -> u32 {
        self.bitrate / 1000
    }
}
