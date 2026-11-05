//! A2DP codecs for A2DPWB.
//!
//! - [`Encoder`]: what the encode thread drives. One call encodes
//!   [`Encoder::pcm_frames_per_encode`] PCM frames.
//! - [`sbc`]: the SBC encoder, ported from Bluedroid to Rust (bit-exact).
//! - LDAC, aptX family, AAC, LHDC V5, Opus (Android's and PipeWire's Opus
//!   05): safe wrappers of the C encoders
//!   (crates `*-sys`), behind cargo features.
//! - [`aptx_adaptive`]: aptX Adaptive, a pure-Rust encoder (crate
//!   `aptx-adaptive`), behind a cargo feature too.
//! - [`caps`]: codec information of the local stream endpoints, parsing of
//!   the remote's capabilities, the configuration sent with
//!   SET_CONFIGURATION, and the codec choice (Auto).

#![forbid(unsafe_code)]

pub mod caps;
pub mod dev;
pub mod sbc;

#[cfg(feature = "aac")]
pub mod aac;
#[cfg(feature = "aptx")]
pub mod aptx;
#[cfg(feature = "aptx-adaptive")]
pub mod aptx_adaptive;
#[cfg(feature = "lc3")]
pub mod lc3;
#[cfg(feature = "ldac")]
pub mod ldac;
#[cfg(feature = "lhdcv5")]
pub mod lhdcv5;
#[cfg(feature = "opus")]
pub mod opus;

use std::fmt;

/// The A2DP codecs A2DPWB can stream
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CodecKind {
    Sbc,
    /// High-bitrate SBC as PipeWire / Android custom ROMs call it: dual
    /// channel, 16 blocks, 8 subbands, loudness, a high bitpool
    SbcXq,
    Aac,
    Aptx,
    AptxHd,
    AptxLl,
    Ldac,
    LhdcV5,
    /// CSR FastStream: SBC frames (joint stereo, bitpool 29) with neither a
    /// media payload header nor an RTP header; music direction only
    FastStream,
    Opus,
    /// PipeWire's own A2DP Opus ("Opus 05", vendor 0x000005F1 / codec
    /// 0x1005), stereo music direction only; never Auto's choice
    Opus05,
    /// aptX Adaptive (vendor 0x000000D7 / codec 0x00AD), 48 / 96 kHz
    /// stereo, HQ or low latency ([`Quality::LowLatency`]); Auto's choice
    /// after LHDC V5 and LDAC (issue #86, confirmed on an EarFun Air Pro 4+)
    AptxAdaptive,
}

/// A device's codec order (its profile's `codec_order`, issue #86 round 8):
/// the codecs of [`CodecKind::AUTO_PRIORITY`] in the order the user dragged
/// the device's cards into on the Codecs screen (with Auto-select off); the
/// default is that list, highest sound quality first. Auto takes the first
/// one the device offers, and so does a chosen codec the device does not
/// offer (the fallback).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutoOrder([CodecKind; 10]);

impl Default for AutoOrder {
    fn default() -> Self {
        AutoOrder(CodecKind::AUTO_PRIORITY)
    }
}

impl AutoOrder {
    /// From codec keys: those of AUTO_PRIORITY in the order given (others
    /// and repeats left out), then any missing in their default order. An
    /// empty list is the default order.
    pub fn from_keys<S: AsRef<str>>(keys: &[S]) -> AutoOrder {
        let mut order = Vec::with_capacity(CodecKind::AUTO_PRIORITY.len());
        let given = keys.iter().filter_map(|k| CodecKind::from_key(k.as_ref()));
        for k in given.chain(CodecKind::AUTO_PRIORITY) {
            if CodecKind::AUTO_PRIORITY.contains(&k) && !order.contains(&k) {
                order.push(k);
            }
        }
        AutoOrder(order.try_into().unwrap_or(CodecKind::AUTO_PRIORITY))
    }

    pub fn codecs(&self) -> [CodecKind; 10] {
        self.0
    }

    pub fn is_default(&self) -> bool {
        self.0 == CodecKind::AUTO_PRIORITY
    }

    /// Where the codec stands (0 = first). Opus 05 stands with Opus and
    /// FastStream after the last.
    pub fn rank(&self, kind: CodecKind) -> usize {
        let at = |k: CodecKind| self.0.iter().position(|&c| c == k).unwrap_or(0);
        match kind {
            CodecKind::Opus05 => at(CodecKind::Opus),
            CodecKind::FastStream => self.0.len(),
            k => at(k),
        }
    }

    /// The codec names in order, for the log
    pub fn names(&self) -> String {
        self.0.iter().map(|k| k.name()).collect::<Vec<_>>().join(" > ")
    }
}

impl CodecKind {
    pub const ALL: [CodecKind; 12] = [
        CodecKind::Ldac,
        CodecKind::AptxHd,
        CodecKind::AptxLl,
        CodecKind::Aptx,
        CodecKind::SbcXq,
        CodecKind::Sbc,
        CodecKind::Aac,
        CodecKind::LhdcV5,
        CodecKind::Opus,
        CodecKind::Opus05,
        CodecKind::FastStream,
        CodecKind::AptxAdaptive,
    ];

    /// The choice of "Auto", highest sound quality first (issue #86): LHDC
    /// V5, LDAC, aptX Adaptive, Opus (Android's A2DP Opus, 256 kbps stereo),
    /// aptX HD, aptX, then aptX Low Latency (aptX's bit rate, for latency),
    /// AAC, SBC XQ and SBC. SBC XQ stays below AAC: it carries more bits,
    /// but AAC sounds better per bit and SBC XQ has trouble with some
    /// frequencies. A chosen codec the device does not offer falls back
    /// to the first one here the device offers ([`caps::choose`]). FastStream is not here: it sounds
    /// worse than SBC, so it is used only when chosen. Neither is Opus 05:
    /// only PipeWire implements it (as a sink too), it is not verified with
    /// one, and a PipeWire sink offers Android's Opus as well from PipeWire
    /// 1.2 on.
    pub const AUTO_PRIORITY: [CodecKind; 10] = [
        CodecKind::LhdcV5,
        CodecKind::Ldac,
        CodecKind::AptxAdaptive,
        CodecKind::Opus,
        CodecKind::AptxHd,
        CodecKind::Aptx,
        CodecKind::AptxLl,
        CodecKind::Aac,
        CodecKind::SbcXq,
        CodecKind::Sbc,
    ];

    /// Where the codec stands in [`CodecKind::AUTO_PRIORITY`] (0 = best).
    /// Opus 05 stands with Opus and FastStream below SBC, so a fallback from
    /// them still goes down the list.
    pub fn quality_rank(self) -> usize {
        AutoOrder::default().rank(self)
    }

    /// Display name (as v1 shows it)
    pub fn name(self) -> &'static str {
        match self {
            CodecKind::Sbc => "SBC",
            CodecKind::SbcXq => "SBC XQ",
            CodecKind::Aac => "AAC",
            CodecKind::Aptx => "aptX",
            CodecKind::AptxHd => "aptX HD",
            CodecKind::AptxLl => "aptX Low Latency",
            CodecKind::Ldac => "LDAC",
            CodecKind::LhdcV5 => "LHDC V5",
            CodecKind::Opus => "Opus",
            CodecKind::Opus05 => "Opus 05",
            CodecKind::FastStream => "FastStream",
            CodecKind::AptxAdaptive => "aptX Adaptive",
        }
    }

    /// Key in profiles.json and on the command line
    pub fn key(self) -> &'static str {
        match self {
            CodecKind::Sbc => "sbc",
            CodecKind::SbcXq => "sbcxq",
            CodecKind::Aac => "aac",
            CodecKind::Aptx => "aptx",
            CodecKind::AptxHd => "aptxhd",
            CodecKind::AptxLl => "aptxll",
            CodecKind::Ldac => "ldac",
            CodecKind::LhdcV5 => "lhdcv5",
            CodecKind::Opus => "opus",
            CodecKind::Opus05 => "opus05",
            CodecKind::FastStream => "faststream",
            CodecKind::AptxAdaptive => "aptxad",
        }
    }

    pub fn from_key(key: &str) -> Option<CodecKind> {
        CodecKind::ALL.into_iter().find(|c| c.key().eq_ignore_ascii_case(key))
    }

    /// Bytes of the A2DP media payload header in front of the codec frames:
    /// SBC (XQ too), LDAC and Opus (both) 1 (frame count), LHDC V5 2 (frame
    /// count, sequence), the others none
    pub fn media_header_len(self) -> usize {
        match self {
            CodecKind::Sbc | CodecKind::SbcXq | CodecKind::Ldac | CodecKind::Opus | CodecKind::Opus05 => 1,
            CodecKind::LhdcV5 => 2,
            _ => 0,
        }
    }

    /// Android's Opus or PipeWire's Opus 05: the same libopus encoder and
    /// packets (one frame per packet behind a 1-byte frame count)
    pub fn is_opus(self) -> bool {
        matches!(self, CodecKind::Opus | CodecKind::Opus05)
    }

    /// Classic aptX, aptX LL and FastStream go out without an RTP header
    /// (Android A2DP_APTX_OFFSET, PipeWire); all others with BTstack's.
    /// aptX Adaptive has an RTP header of its own, which its encoder writes
    /// (the timestamp and SSRC carry codec information), so BTstack adds
    /// none either.
    pub fn uses_rtp(self) -> bool {
        !matches!(self, CodecKind::Aptx | CodecKind::AptxLl | CodecKind::FastStream | CodecKind::AptxAdaptive)
    }

    /// Highest capture sample rate the codec takes
    pub fn max_sample_rate(self) -> u32 {
        match self {
            CodecKind::Ldac | CodecKind::AptxAdaptive => 96000,
            CodecKind::LhdcV5 => 192000,
            _ => 48000,
        }
    }

    /// Experimental: passes the encoder and emulator tests but is not yet
    /// confirmed with real headphones (the README's 🧪). The app uses them
    /// as any other codec (issue #86); the CLI's --no-experimental leaves
    /// them out, by Auto and as a fallback.
    pub fn experimental(self) -> bool {
        matches!(
            self,
            CodecKind::AptxHd | CodecKind::AptxLl | CodecKind::FastStream | CodecKind::Opus | CodecKind::Opus05
        )
    }

    /// Whether the encoder for this codec is built in
    pub fn available(self) -> bool {
        match self {
            CodecKind::Sbc | CodecKind::SbcXq | CodecKind::FastStream => true,
            CodecKind::Aac => cfg!(feature = "aac"),
            CodecKind::Aptx | CodecKind::AptxHd | CodecKind::AptxLl => cfg!(feature = "aptx"),
            CodecKind::Ldac => cfg!(feature = "ldac"),
            CodecKind::LhdcV5 => cfg!(feature = "lhdcv5"),
            CodecKind::Opus | CodecKind::Opus05 => cfg!(feature = "opus"),
            CodecKind::AptxAdaptive => cfg!(feature = "aptx-adaptive"),
        }
    }
}

impl fmt::Display for CodecKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Quality mode (codec-specific meaning, as in v1). Since issue #86 each
/// codec offers only some ([`Quality::for_codec`]); the encoders still map
/// every one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Quality {
    /// LDAC 990 kbps, SBC bitpool 53, SBC XQ ~552 kbps, AAC 256 kbps,
    /// LHDC V5 1000 kbps, Opus 256 kbps
    #[default]
    High,
    /// LDAC 660 kbps, SBC bitpool 35, SBC XQ ~505 kbps, AAC 192 kbps, Opus
    /// 192 kbps
    Standard,
    /// LDAC 330 kbps, SBC bitpool 19, SBC XQ ~385 kbps, AAC 128 kbps, Opus
    /// 128 kbps
    Mobile,
    /// LHDC V5 only (900 kbps); other codecs treat it as High
    MediumHigh,
    /// aptX Adaptive only: its low latency mode (~265 kbps); other codecs
    /// treat it as High
    LowLatency,
}

impl Quality {
    /// "hq" / "sq" / "mq" / "mhq" / "ll" (profiles.json)
    pub fn from_key(key: &str) -> Quality {
        match key {
            "sq" => Quality::Standard,
            "mq" => Quality::Mobile,
            "mhq" => Quality::MediumHigh,
            "ll" => Quality::LowLatency,
            _ => Quality::High,
        }
    }

    /// The quality a `kind` stream runs with (issue #86): LDAC and LHDC V5
    /// offer High, Medium (Standard) and Low (Mobile), LHDC V5 also
    /// Medium-High (900 kbps, as Android; issue #203), aptX Adaptive High
    /// and Low Latency, every other codec High only (its highest bit rate
    /// the device takes; ABR / VBR does the rest). A quality the codec no
    /// longer offers (a profile saved before, the CLI's -q) is High.
    pub fn for_codec(self, kind: CodecKind) -> Quality {
        match (kind, self) {
            (CodecKind::Ldac | CodecKind::LhdcV5, Quality::Standard | Quality::Mobile) => self,
            (CodecKind::LhdcV5, Quality::MediumHigh) => self,
            (CodecKind::AptxAdaptive, Quality::LowLatency) => self,
            _ => Quality::High,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Quality::High => "hq",
            Quality::Standard => "sq",
            Quality::Mobile => "mq",
            Quality::MediumHigh => "mhq",
            Quality::LowLatency => "ll",
        }
    }
}

/// Sample format an encoder takes (interleaved)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcmFormat {
    /// 16-bit signed
    I16,
    /// 32-bit signed, MSB-aligned (24-bit codecs use the top 24 bits)
    I32,
}

impl PcmFormat {
    pub fn bytes(self) -> usize {
        match self {
            PcmFormat::I16 => 2,
            PcmFormat::I32 => 4,
        }
    }
}

/// Interleaved PCM handed to [`Encoder::encode`]
#[derive(Debug, Clone, Copy)]
pub enum Pcm<'a> {
    I16(&'a [i16]),
    I32(&'a [i32]),
}

impl Pcm<'_> {
    pub fn samples(&self) -> usize {
        match self {
            Pcm::I16(s) => s.len(),
            Pcm::I32(s) => s.len(),
        }
    }
}

/// What the remote's capabilities add to the encoder setup
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PeerParams {
    /// LHDC V5: the sink's bit rate limits (codec info byte 7, bits 7..4)
    pub lhdcv5_bitrate_limits: u8,
    /// SBC / SBC XQ: the sink's SBC bitpool range (0 = not known); the
    /// encoder keeps its bitpool within it
    pub sbc_min_bitpool: u8,
    pub sbc_max_bitpool: u8,
    /// SBC / SBC XQ: the bitpool the profile asks for (0 = by quality),
    /// still within the sink's range
    pub sbc_bitpool: u8,
    /// SBC / SBC XQ: blocks, subbands, allocation and channel mode, those
    /// of the SET_CONFIGURATION (`caps::RemoteCaps::sbc_params`)
    pub sbc: sbc::SbcParams,
    /// Opus / Opus 05: the frame duration configured, 10 or 20 ms (0 = 20)
    pub opus_frame_ms: u8,
    /// Opus / Opus 05: the channels configured on the wire, 1 or 2 (0 = as
    /// the input); mono input is duplicated for a stereo stream
    pub opus_channels: u8,
    /// Opus 05: the largest bit rate configured, in bps (0 = no limit)
    pub opus_max_bitrate: u32,
    /// AAC: the largest bit rate configured, in bps (0 = no limit)
    pub aac_max_bitrate: u32,
    /// aptX Adaptive: the sink is R2 (its codec information has the
    /// capability extension), which changes the bitstream and the tables
    pub aptxad_r2: bool,
}

/// Encoder setup, fixed for a stream
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncoderConfig {
    pub sample_rate: u32,
    /// 1 or 2
    pub channels: u32,
    pub quality: Quality,
    /// Media payload size per packet, RTP header excluded (the stream's MTU
    /// after the media packet size limit)
    pub mtu: u16,
    /// 16 or 24: LDAC, aptX HD and LHDC V5 take 24-bit PCM (as I32)
    pub bits_per_sample: u8,
    /// Adaptive bit rate (LDAC, LHDC V5, SBC and SBC XQ: the bitpool)
    pub abr: bool,
    pub peer: PeerParams,
}

/// What an encoder runs with besides its bit rate (Diagnostics)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EncoderDetails {
    /// Bytes of every codec frame, when they all have the same length
    /// (SBC under ABR: of the frames encoded now)
    pub frame_bytes: Option<u32>,
    /// SBC, SBC XQ and FastStream: the frame setup, with the bitpool the
    /// encoder uses
    pub sbc: Option<sbc::SbcConfig>,
    /// SBC, SBC XQ under ABR: the range the bitpool moves in (floor,
    /// ceiling)
    pub sbc_abr: Option<(u8, u8)>,
}

/// One [`Encoder::encode`] call's output
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Encoded {
    /// Bytes written to the output
    pub bytes: usize,
    /// Codec frames in them (counted in the media payload header; 0 while an
    /// encoder that buffers input has nothing to emit)
    pub frames: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum EncoderError {
    #[error("{0} is not built into this version")]
    NotAvailable(CodecKind),
    #[error("{codec}: unsupported configuration: {what}")]
    Unsupported { codec: CodecKind, what: String },
    #[error("{codec}: encoder initialization failed: {what}")]
    Init { codec: CodecKind, what: String },
    #[error("{codec}: encoding failed: {what}")]
    Encode { codec: CodecKind, what: String },
    #[error("output buffer too small ({have} < {need} bytes)")]
    OutputTooSmall { have: usize, need: usize },
    #[error("expected {expected} samples, got {got}")]
    InputLength { expected: usize, got: usize },
}

/// An A2DP encoder. Created per stream, used on the encode thread only.
/// `encode` must not allocate.
pub trait Encoder: Send {
    fn kind(&self) -> CodecKind;

    /// Sample format of the input
    fn pcm_format(&self) -> PcmFormat;

    /// PCM frames (samples per channel) one `encode` call consumes
    fn pcm_frames_per_encode(&self) -> usize;

    /// PCM frames one codec frame of [`Encoded::frames`] stands for. The
    /// RTP timestamp advances by this per frame sent, so it stays exact
    /// when an encoder buffers input (LDAC).
    fn pcm_frames_per_codec_frame(&self) -> usize {
        self.pcm_frames_per_encode()
    }

    /// Largest output of one `encode` call, for sizing the buffer
    fn max_output_bytes(&self) -> usize;

    /// Encodes `pcm_frames_per_encode() * channels` samples into `out`
    fn encode(&mut self, pcm: Pcm<'_>, out: &mut [u8]) -> Result<Encoded, EncoderError>;

    /// Current bit rate in kbps
    fn bitrate_kbps(&self) -> u32;

    /// The device's maximum bit rate lowered the fixed bit rate the quality
    /// mode asked for (LHDC V5 only; never under ABR)
    fn bitrate_capped(&self) -> bool {
        false
    }

    /// Adaptive bit rate step with the send queue depth (packets waiting);
    /// called by the encode thread when [`EncoderConfig::abr`] is set
    fn abr_adjust(&mut self, _queue_depth: u32) {}

    /// Adaptive bit rate is running: asked for and set up (LDAC keeps a
    /// fixed quality when its ABR module fails to start)
    fn abr_active(&self) -> bool {
        false
    }

    /// Puts ABR back to where a stream starts (after a reconnect: what the
    /// old link's queue made it do says nothing about the new one)
    fn abr_reset(&mut self) {}

    /// The setup for Diagnostics; nothing beyond the bit rate by default
    fn details(&self) -> EncoderDetails {
        EncoderDetails::default()
    }
}

/// Creates the encoder for `kind`
pub fn create(kind: CodecKind, config: &EncoderConfig) -> Result<Box<dyn Encoder>, EncoderError> {
    if !kind.available() {
        return Err(EncoderError::NotAvailable(kind));
    }
    tracing::debug!("codec: creating {kind} encoder: {config:?}");
    let encoder = create_inner(kind, config);
    match &encoder {
        Ok(e) => tracing::debug!(
            "codec: {kind} encoder ready: {:?} input, {} frames per encode, {} per codec frame, up to {} bytes per \
             encode, {} kbps, abr {}, bit rate capped {}, {:?}",
            e.pcm_format(),
            e.pcm_frames_per_encode(),
            e.pcm_frames_per_codec_frame(),
            e.max_output_bytes(),
            e.bitrate_kbps(),
            e.abr_active(),
            e.bitrate_capped(),
            e.details()
        ),
        Err(e) => tracing::debug!("codec: creating the {kind} encoder failed: {e}"),
    }
    encoder
}

fn create_inner(kind: CodecKind, config: &EncoderConfig) -> Result<Box<dyn Encoder>, EncoderError> {
    match kind {
        CodecKind::Sbc => Ok(Box::new(sbc::SbcEncoder::new(config)?)),
        CodecKind::SbcXq => Ok(Box::new(sbc::SbcEncoder::new_xq(config)?)),
        CodecKind::FastStream => Ok(Box::new(sbc::FastStreamEncoder::new(config)?)),
        #[cfg(feature = "aac")]
        CodecKind::Aac => Ok(Box::new(aac::AacEncoder::new(config)?)),
        #[cfg(feature = "aptx")]
        CodecKind::Aptx | CodecKind::AptxHd | CodecKind::AptxLl => Ok(Box::new(aptx::AptxEncoder::new(kind, config)?)),
        #[cfg(feature = "ldac")]
        CodecKind::Ldac => Ok(Box::new(ldac::LdacEncoder::new(config)?)),
        #[cfg(feature = "lhdcv5")]
        CodecKind::LhdcV5 => Ok(Box::new(lhdcv5::LhdcV5Encoder::new(config)?)),
        #[cfg(feature = "opus")]
        CodecKind::Opus | CodecKind::Opus05 => Ok(Box::new(opus::OpusEncoder::new(kind, config)?)),
        #[cfg(feature = "aptx-adaptive")]
        CodecKind::AptxAdaptive => Ok(Box::new(aptx_adaptive::AptxAdaptiveEncoder::new(config)?)),
        #[allow(unreachable_patterns)]
        _ => Err(EncoderError::NotAvailable(kind)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qualities_each_codec_offers() {
        use Quality::*;
        for q in [Standard, Mobile] {
            assert_eq!(q.for_codec(CodecKind::Ldac), q);
            assert_eq!(q.for_codec(CodecKind::LhdcV5), q);
        }
        // LHDC V5's Medium-High is back (issue #203); LDAC has none
        assert_eq!(MediumHigh.for_codec(CodecKind::LhdcV5), MediumHigh);
        assert_eq!(MediumHigh.for_codec(CodecKind::Ldac), High);
        assert_eq!(LowLatency.for_codec(CodecKind::AptxAdaptive), LowLatency);
        for q in [Standard, Mobile, MediumHigh] {
            assert_eq!(q.for_codec(CodecKind::AptxAdaptive), High, "{q:?}");
        }
        for kind in [CodecKind::Aac, CodecKind::Sbc, CodecKind::SbcXq, CodecKind::Opus, CodecKind::Opus05] {
            for q in [Standard, Mobile, MediumHigh, LowLatency] {
                assert_eq!(q.for_codec(kind), High, "{kind:?} {q:?}");
            }
        }
    }
}
