//! LDAC: Sony's high-resolution Bluetooth codec (44.1 / 48 / 88.2 / 96 kHz,
//! 16 or 24-bit, 330 / 660 / 990 kbps), encoded by libldac from AOSP
//! (Apache-2.0, crate `ldac-sys`) with its adaptive bit rate module.
//!
//! libldac takes 128 PCM frames per call at every sample rate but encodes
//! 128-sample frames at 44.1 / 48 kHz and 256-sample frames at 88.2 /
//! 96 kHz, and it holds frames back until a packet's worth is ready: most
//! [`Encoder::encode`] calls return nothing, and the RTP timestamp has to
//! advance by the frames actually emitted ([`Encoder::pcm_frames_per_codec_frame`]),
//! not by the calls. libldac sizes its packets for a 2-DH5 baseband packet
//! (at most 661 bytes of frames) whatever the MTU, and it wants the
//! transport (L2CAP) MTU, of at least 679 bytes.

use ldac_sys::{LdacAbr, LdacBt, SampleFormat, ffi};

use crate::{CodecKind, Encoded, Encoder, EncoderConfig, EncoderError, Pcm, PcmFormat, Quality};

/// RTP header in front of the media payload (EncoderConfig::mtu excludes
/// it; libldac wants the whole transport MTU)
const RTP_HEADER_SIZE: usize = 12;

/// How often the encode thread calls [`LdacEncoder::abr_adjust`] (v1: every
/// 100 ms from the encode thread); ldac_ABR_Init derives its observation
/// windows from it
pub const ABR_INTERVAL_MS: u32 = 100;

/// ABR thresholds in packets for A2DPWB's 64-slot send queue (v1): critical,
/// dangerous trend, safety for HQ / SQ. libldac's defaults (6, 4, 2) suit
/// Android's shallower queue.
const ABR_THRESHOLDS: (u32, u32, u32) = (12, 8, 4);

const CODEC: CodecKind = CodecKind::Ldac;

pub struct LdacEncoder {
    bt: LdacBt,
    /// Some when EncoderConfig::abr is set and the ABR module started
    abr: Option<LdacAbr>,
    /// The quality mode asked for, where ABR starts again after a reset
    eqmid: i32,
    format: PcmFormat,
    channels: usize,
    sample_rate: u32,
    /// libldac's bit rate of the frames being encoded, refreshed after each
    /// call (a quality change takes effect at the next packet boundary)
    bitrate_kbps: u32,
}

impl LdacEncoder {
    pub fn new(config: &EncoderConfig) -> Result<LdacEncoder, EncoderError> {
        let unsupported = |what: String| EncoderError::Unsupported { codec: CODEC, what };
        if !matches!(config.sample_rate, 44100 | 48000 | 88200 | 96000) {
            return Err(unsupported(format!(
                "sample rate {} Hz (use 44100, 48000, 88200 or 96000)",
                config.sample_rate
            )));
        }
        let channel_mode = match config.channels {
            1 => ffi::LDACBT_CHANNEL_MODE_MONO,
            2 => ffi::LDACBT_CHANNEL_MODE_STEREO,
            n => return Err(unsupported(format!("{n} channels"))),
        };
        // 24-bit audio goes in as int32, MSB-aligned (libldac's S32 format)
        let (format, sample_format) = match config.bits_per_sample {
            16 => (PcmFormat::I16, SampleFormat::S16),
            24 => (PcmFormat::I32, SampleFormat::S32),
            n => return Err(unsupported(format!("{n}-bit samples (use 16 or 24)"))),
        };
        // libldac wants the AVDTP transport channel (L2CAP) MTU and rejects
        // anything below 679, but config.mtu has the 12-byte RTP header
        // already subtracted: a sink with the minimum 679-byte MTU arrives
        // as 667. libldac sizes its packets to fit (L2CAP MTU - 18), which
        // stays within the mtu - 1 bytes left after the media payload header.
        let transport_mtu = usize::from(config.mtu) + RTP_HEADER_SIZE;
        if transport_mtu < ffi::LDACBT_MTU_REQUIRED {
            return Err(unsupported(format!(
                "media MTU {} (LDAC needs at least {})",
                config.mtu,
                ffi::LDACBT_MTU_REQUIRED - RTP_HEADER_SIZE
            )));
        }
        let eqmid = match config.quality {
            Quality::High | Quality::MediumHigh | Quality::LowLatency => ffi::LDACBT_EQMID_HQ,
            Quality::Standard => ffi::LDACBT_EQMID_SQ,
            Quality::Mobile => ffi::LDACBT_EQMID_MQ,
        };

        tracing::debug!(
            "LDAC: quality {:?} -> eqmid {eqmid}, channel mode {channel_mode}, {format:?} input, transport mtu \
             {transport_mtu} (media mtu {}), abr asked {}",
            config.quality,
            config.mtu,
            config.abr
        );
        let init_err = |e: ldac_sys::Error| EncoderError::Init { codec: CODEC, what: e.to_string() };
        let mut bt = LdacBt::new().map_err(init_err)?;
        bt.init_encode(
            i32::try_from(transport_mtu).unwrap_or(i32::MAX),
            eqmid,
            channel_mode,
            sample_format,
            config.sample_rate as i32,
        )
        .map_err(init_err)?;

        // ABR is optional: without it the stream still works at the fixed
        // quality (v1 turned ABR off when its setup failed)
        let abr = if config.abr {
            match new_abr() {
                Ok(abr) => Some(abr),
                Err(e) => {
                    tracing::warn!("LDAC: ABR setup failed, keeping a fixed quality: {e}");
                    None
                }
            }
        } else {
            None
        };

        let bitrate_kbps = u32::try_from(bt.bitrate()).unwrap_or(0);
        tracing::info!(
            "LDAC: initialized (rate={}, ch={}, {}-bit, quality={:?}, transport mtu={}, bitrate={} kbps, abr={})",
            config.sample_rate,
            config.channels,
            config.bits_per_sample,
            config.quality,
            transport_mtu,
            bitrate_kbps,
            abr.is_some()
        );
        Ok(LdacEncoder {
            bt,
            abr,
            eqmid,
            format,
            channels: config.channels as usize,
            sample_rate: config.sample_rate,
            bitrate_kbps,
        })
    }

    /// Whether adaptive bit rate is active
    pub fn abr_enabled(&self) -> bool {
        self.abr.is_some()
    }

    /// Quality mode libldac encodes at now (ABR moves it)
    pub fn eqmid(&self) -> i32 {
        self.bt.eqmid()
    }

    fn encode_error(e: ldac_sys::Error) -> EncoderError {
        match e {
            ldac_sys::Error::InputLength { expected, got } => EncoderError::InputLength { expected, got },
            ldac_sys::Error::OutputTooSmall { have, need } => EncoderError::OutputTooSmall { have, need },
            e => EncoderError::Encode { codec: CODEC, what: e.to_string() },
        }
    }
}

impl Encoder for LdacEncoder {
    fn kind(&self) -> CodecKind {
        CODEC
    }

    fn pcm_format(&self) -> PcmFormat {
        self.format
    }

    /// libldac takes 128 frames per call at every sample rate
    fn pcm_frames_per_encode(&self) -> usize {
        ffi::LDACBT_ENC_LSU
    }

    /// An LDAC frame holds 128 samples at 44.1 / 48 kHz and 256 at 88.2 /
    /// 96 kHz
    fn pcm_frames_per_codec_frame(&self) -> usize {
        if self.sample_rate > 48000 { 256 } else { 128 }
    }

    /// libldac's internal packet buffer, the most one call can write. The
    /// packets themselves stay within 661 bytes (2-DH5 sizing).
    fn max_output_bytes(&self) -> usize {
        ffi::LDACBT_ENC_STREAM_BUF_SZ
    }

    fn encode(&mut self, pcm: Pcm<'_>, out: &mut [u8]) -> Result<Encoded, EncoderError> {
        let expected = ffi::LDACBT_ENC_LSU * self.channels;
        if pcm.samples() != expected {
            return Err(EncoderError::InputLength { expected, got: pcm.samples() });
        }
        let r = match (pcm, self.format) {
            (Pcm::I16(s), PcmFormat::I16) => self.bt.encode_i16(s, out),
            (Pcm::I32(s), PcmFormat::I32) => self.bt.encode_i32(s, out),
            _ => {
                return Err(EncoderError::Encode { codec: CODEC, what: format!("input is not {:?} PCM", self.format) });
            }
        }
        .map_err(Self::encode_error)?;
        self.bitrate_kbps = u32::try_from(self.bt.bitrate()).unwrap_or(self.bitrate_kbps);
        Ok(Encoded { bytes: r.bytes, frames: r.frames })
    }

    /// The bit rate of the frames being encoded: 990 / 660 / 330 kbps at 48 /
    /// 96 kHz stereo (909 / 606 / 303 at 44.1 / 88.2 kHz, half for mono),
    /// or 492 / 396 kbps on the way down under ABR
    fn bitrate_kbps(&self) -> u32 {
        self.bitrate_kbps
    }

    /// One ldac_ABR_Proc step. Call every [`ABR_INTERVAL_MS`] (100 ms) with
    /// the send queue depth in packets at that moment; libldac's observation
    /// windows assume that cadence. A deep queue moves the quality down
    /// (HQ, SQ, 492 kbps, 396 kbps, MQ), an empty one back up step by step
    /// after a few seconds. The new quality applies from the next packet.
    /// Does nothing when ABR is off.
    fn abr_adjust(&mut self, queue_depth: u32) {
        let Some(abr) = self.abr.as_mut() else { return };
        let before = self.bt.eqmid();
        match abr.process(&mut self.bt, queue_depth, true) {
            Ok(eqmid) if eqmid != before => {
                tracing::debug!("LDAC: ABR quality mode {before} -> {eqmid} (queue {queue_depth})");
            }
            Ok(_) => {}
            Err(e) => tracing::warn!("LDAC: ABR step failed: {e}"),
        }
    }

    fn abr_active(&self) -> bool {
        self.abr.is_some()
    }

    /// Back to the quality mode asked for, with a fresh ABR module (its
    /// observation windows start over)
    fn abr_reset(&mut self) {
        if self.abr.is_none() {
            return;
        }
        match new_abr() {
            Ok(abr) => self.abr = Some(abr),
            Err(e) => tracing::warn!("LDAC: ABR restart failed, keeping its state: {e}"),
        }
        if self.bt.eqmid() != self.eqmid {
            match self.bt.set_eqmid(self.eqmid) {
                Ok(()) => tracing::debug!("LDAC: ABR reset to quality mode {}", self.eqmid),
                Err(e) => tracing::warn!("LDAC: ABR reset to quality mode {} failed: {e}", self.eqmid),
            }
        }
    }
}

fn new_abr() -> Result<LdacAbr, ldac_sys::Error> {
    let (critical, dangerous, safety) = ABR_THRESHOLDS;
    let mut abr = LdacAbr::new(ABR_INTERVAL_MS)?;
    abr.set_thresholds(critical, dangerous, safety)?;
    Ok(abr)
}
