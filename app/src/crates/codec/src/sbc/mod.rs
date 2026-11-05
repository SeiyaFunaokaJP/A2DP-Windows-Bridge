//! SBC encoder: Bluedroid's fixed-point encoder (the one BTstack bundles in
//! 3rd-party/bluedroid/encoder), ported from C to safe Rust. The output is
//! bit-exact with the C encoder as BTstack configures it; tests/sbc_bitexact.rs
//! compares the two over all configurations.
//!
//! Ported from Bluedroid, Copyright (C) 1999-2012 Broadcom Corporation,
//! licensed under the Apache License, Version 2.0
//! (http://www.apache.org/licenses/LICENSE-2.0). Each ported file carries
//! the notice.
//!
//! - [`SbcFrameEncoder`]: the general encoder, any [`SbcConfig`].
//! - [`SbcEncoder`]: the A2DP stream encoder (as v1's wrapper): 44.1 / 48
//!   kHz, joint stereo (or mono), 16 blocks, 8 subbands, loudness, the
//!   bitpool by [`Quality`]; [`SbcEncoder::new_xq`] is SBC XQ: dual channel
//!   with a high bitpool per channel ([`xq_bitpool`]). A profile may ask
//!   for other blocks, subbands, allocation and channel mode
//!   ([`SbcParams`]).
//! - [`FastStreamEncoder`]: CSR FastStream, SBC frames of a fixed setup.

mod analysis;
mod bit_alloc;
mod faststream;
mod frame;
mod packing;

pub use faststream::{
    FASTSTREAM_BITPOOL, FASTSTREAM_FRAME_BYTES, FASTSTREAM_FRAMES_PER_PACKET, FASTSTREAM_MAX_PAYLOAD, FastStreamEncoder,
};
pub use frame::{Allocation, ChannelMode, SbcConfig, SbcFrameEncoder};

use crate::{CodecKind, Encoded, Encoder, EncoderConfig, EncoderDetails, EncoderError, Pcm, PcmFormat, Quality};

const BLOCKS: u8 = 16;
const SUBBANDS: u8 = 8;
/// The local SEP's largest bitpool
const SBC_MAX_BITPOOL: u8 = 53;

/// The SBC frame setup a profile asks for: 0 / None is the default (16
/// blocks, 8 subbands, loudness; joint stereo for SBC, while SBC XQ is
/// always dual channel). The SET_CONFIGURATION and the encoder take the
/// same values (`caps::RemoteCaps::sbc_params` drops the ones the sink does
/// not list).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct SbcParams {
    /// 4, 8, 12 or 16
    pub blocks: u8,
    /// 4 or 8
    pub subbands: u8,
    pub allocation: Option<Allocation>,
    /// The channel mode of stereo input (SBC only; mono input is mono).
    /// Mono here mixes stereo input down.
    pub channel_mode: Option<ChannelMode>,
    /// The lowest bitpool the profile asks for (0 = Auto, issue #86): the
    /// SET_CONFIGURATION's minimum bitpool (within the sink's range and the
    /// maximum, [`SbcParams::config_min_bitpool`]), the floor of ABR, and
    /// the stream's bitpool is never below it
    pub min_bitpool: u8,
}

impl SbcParams {
    pub fn blocks(&self) -> u8 {
        if matches!(self.blocks, 4 | 8 | 12) { self.blocks } else { BLOCKS }
    }

    pub fn subbands(&self) -> u8 {
        if self.subbands == 4 { 4 } else { SUBBANDS }
    }

    pub fn allocation(&self) -> Allocation {
        self.allocation.unwrap_or(Allocation::Loudness)
    }

    /// The channel mode of a `kind` stream of `channels` input channels
    pub fn channel_mode(&self, kind: CodecKind, channels: u32) -> ChannelMode {
        if channels == 1 {
            ChannelMode::Mono
        } else if kind == CodecKind::SbcXq {
            ChannelMode::DualChannel
        } else {
            self.channel_mode.unwrap_or(ChannelMode::JointStereo)
        }
    }

    /// Nothing asked for: today's setup
    pub fn is_default(&self) -> bool {
        *self == SbcParams::default()
    }

    /// The minimum bitpool of a SET_CONFIGURATION whose range would be
    /// `min..=max` (the sink's range, up to the codec's cap): the profile's
    /// minimum within it, else (Auto) `min`
    pub fn config_min_bitpool(&self, min: u8, max: u8) -> u8 {
        match self.min_bitpool {
            0 => min,
            n => n.clamp(min.min(max), max),
        }
    }
}

impl ChannelMode {
    /// "mono", "dual_channel", "stereo", "joint_stereo" (profiles, IPC)
    pub fn key(self) -> &'static str {
        match self {
            ChannelMode::Mono => "mono",
            ChannelMode::DualChannel => "dual_channel",
            ChannelMode::Stereo => "stereo",
            ChannelMode::JointStereo => "joint_stereo",
        }
    }

    pub fn from_key(key: &str) -> Option<ChannelMode> {
        ChannelMode::ALL.into_iter().find(|m| m.key() == key)
    }
}

impl Allocation {
    /// "loudness", "snr" (profiles, IPC)
    pub fn key(self) -> &'static str {
        match self {
            Allocation::Loudness => "loudness",
            Allocation::Snr => "snr",
        }
    }

    pub fn from_key(key: &str) -> Option<Allocation> {
        [Allocation::Loudness, Allocation::Snr].into_iter().find(|a| a.key() == key)
    }
}

/// SBC bitpool for a quality mode (joint stereo; mono takes the same)
pub fn quality_bitpool(quality: Quality) -> u8 {
    match quality {
        Quality::High | Quality::MediumHigh | Quality::LowLatency => 53,
        Quality::Standard => 35,
        Quality::Mobile => 19,
    }
}

/// SBC XQ bitpool (per channel, dual channel; mono takes the same) for a
/// quality mode and sample rate:
/// - High: 47 at 44.1 kHz (the "SBC XQ" of Android custom ROMs and
///   pulseaudio-modules-bt: 200-byte frames, ~551 kbps), 43 at 48 kHz (the
///   same ~552 kbps). Three frames and the media payload header fill one
///   2-DH5 packet, five one 3-DH5.
/// - Standard: 43 / 39, PipeWire's SBC-XQ default (a2dp-codec-sbc.c,
///   default_bitpool): the ~512 kbps every A2DP 1.2 sink has to take.
/// - Mobile: 32 / 29, PipeWire's dual channel default without XQ (~385 kbps).
pub fn xq_bitpool(quality: Quality, sample_rate: u32) -> u8 {
    let (at_44100, at_48000) = match quality {
        Quality::High | Quality::MediumHigh | Quality::LowLatency => (47, 43),
        Quality::Standard => (43, 39),
        Quality::Mobile => (32, 29),
    };
    if sample_rate == 44100 { at_44100 } else { at_48000 }
}

/// Largest SBC XQ bitpool at a sample rate (any quality): the maximum the
/// SET_CONFIGURATION gives
pub fn xq_max_bitpool(sample_rate: u32) -> u8 {
    xq_bitpool(Quality::High, sample_rate)
}

/// `wanted` within the sink's SBC bitpool range when it is known (as the
/// SET_CONFIGURATION's range, `caps::RemoteCaps::sbc_bitpool_range`)
fn sink_bitpool(wanted: u8, config: &EncoderConfig) -> u8 {
    let (min, max) = (config.peer.sbc_min_bitpool, config.peer.sbc_max_bitpool);
    if max < 2 {
        tracing::debug!("SBC: bitpool {wanted} (profile {}, sink range not known)", config.peer.sbc_bitpool);
        return wanted;
    }
    let bitpool = wanted.min(max).max(min.min(max)).max(2);
    tracing::debug!(
        "SBC: bitpool {bitpool} (wanted {wanted}, profile {}, sink range {min}..{max})",
        config.peer.sbc_bitpool
    );
    bitpool
}

/// The bitpool a profile's Auto takes for a channel mode: the quality's
/// joint stereo bitpool (mono and stereo the same); dual channel halves it
/// per channel, for about the same bit rate
pub fn auto_bitpool(quality: Quality, channel_mode: ChannelMode) -> u8 {
    let bitpool = quality_bitpool(quality);
    if channel_mode == ChannelMode::DualChannel { bitpool.div_ceil(2) } else { bitpool }
}

/// Bit rate of SBC frames of `frame_len` bytes ([`SbcConfig::frame_len`],
/// the A2DP specification's formula), each `pcm_frames` PCM frames at
/// `sample_rate`: kbps, rounded down (as v1)
pub fn bitrate_kbps(frame_len: usize, sample_rate: u32, pcm_frames: usize) -> u32 {
    (8 * frame_len as u64 * sample_rate as u64 / pcm_frames.max(1) as u64 / 1000) as u32
}

// Adaptive bitpool (SBC and SBC XQ under ABR), after PipeWire's SBC
// (spa/plugins/bluez5/a2dp-codec-sbc.c, codec_reduce_bitpool -2 and
// codec_increase_bitpool +1, clamped to min_bitpool = max(the
// configuration's minimum, 12) .. the configured maximum; media-sink.c
// lowers it when a write finds the socket full, at most every 0.5 s, and
// raises it when a packet goes out with nothing unsent and nothing was
// lowered or raised for 1 s). A2DPWB has no socket: the encode thread
// passes the send queue depth every 100 ms (ldac::ABR_INTERVAL_MS), so a
// queue past LDAC's "dangerous" threshold stands for the full socket and a
// nearly empty one for nothing unsent.

/// Send queue depth (packets of the 64-slot queue) that lowers the bitpool:
/// LDAC's "dangerous trend" threshold (`ldac::ABR_THRESHOLDS`). About 170
/// ms of SBC High at 1023-byte packets.
pub const ABR_DOWN_DEPTH: u32 = 8;
/// Depth at or below which the queue counts as drained (the packets of the
/// round just encoded may still wait for the controller)
pub const ABR_UP_DEPTH: u32 = 2;
/// PipeWire's steps
pub const ABR_DOWN_STEP: u8 = 2;
pub const ABR_UP_STEP: u8 = 1;
/// ABR steps (100 ms each) between two steps down: PipeWire's 0.5 s
pub const ABR_DOWN_TICKS: u32 = 5;
/// ABR steps the queue has to stay drained for one step up: PipeWire's 1 s
pub const ABR_UP_TICKS: u32 = 10;
/// PipeWire's lowest bitpool
pub const ABR_MIN_BITPOOL: u8 = 12;

/// The lowest bitpool ABR goes to: never below the sink's minimum or
/// PipeWire's 12, and not below the Mobile quality's bitpool of the stream
/// ([`auto_bitpool`] for SBC: 19 joint stereo, stereo and mono, 10 -> 12
/// dual channel; [`xq_bitpool`] for SBC XQ: 32 at 44.1 kHz, 29 at 48 kHz,
/// ~385 kbps). Below Mobile the stream sounds worse than what a profile can
/// ask for, and SBC XQ below its Mobile is no longer XQ (SBC is the codec
/// for that). Never above `ceiling`.
pub fn abr_floor(kind: CodecKind, channel_mode: ChannelMode, sample_rate: u32, sink_min: u8, ceiling: u8) -> u8 {
    let mobile = if kind == CodecKind::SbcXq {
        xq_bitpool(Quality::Mobile, sample_rate)
    } else {
        auto_bitpool(Quality::Mobile, channel_mode)
    };
    mobile.max(ABR_MIN_BITPOOL).max(sink_min).min(ceiling)
}

/// The lowest bitpool ABR goes to when the profile sets a minimum
/// ([`SbcParams::min_bitpool`]): that minimum, in place of Mobile and
/// PipeWire's 12, still never below the sink's minimum (nor 2) or above
/// `ceiling`
pub fn user_abr_floor(min: u8, sink_min: u8, ceiling: u8) -> u8 {
    min.max(sink_min).max(2).min(ceiling)
}

/// The adaptive bitpool state: the bitpool the stream started with is the
/// ceiling, congestion lowers it by [`ABR_DOWN_STEP`] (at most every
/// [`ABR_DOWN_TICKS`]), a drained queue raises it by [`ABR_UP_STEP`] every
/// [`ABR_UP_TICKS`]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BitpoolAbr {
    pub floor: u8,
    pub ceiling: u8,
    /// Steps since the last step down
    since_down: u32,
    /// Steps the queue stayed drained since the last change
    calm: u32,
}

impl BitpoolAbr {
    pub fn new(floor: u8, ceiling: u8) -> BitpoolAbr {
        BitpoolAbr { floor: floor.min(ceiling), ceiling, since_down: ABR_DOWN_TICKS, calm: 0 }
    }

    /// One step (every 100 ms) with the send queue depth: the bitpool for
    /// the next frames
    pub fn step(&mut self, bitpool: u8, queue_depth: u32) -> u8 {
        self.since_down = self.since_down.saturating_add(1);
        if queue_depth >= ABR_DOWN_DEPTH {
            self.calm = 0;
            if self.since_down >= ABR_DOWN_TICKS && bitpool > self.floor {
                self.since_down = 0;
                return bitpool.saturating_sub(ABR_DOWN_STEP).max(self.floor);
            }
            return bitpool;
        }
        if queue_depth <= ABR_UP_DEPTH {
            self.calm += 1;
        } else {
            self.calm = 0;
        }
        if self.calm >= ABR_UP_TICKS && bitpool < self.ceiling {
            self.calm = 0;
            return bitpool.saturating_add(ABR_UP_STEP).min(self.ceiling);
        }
        bitpool
    }

    /// Where a stream starts: the ceiling, the history forgotten
    pub fn reset(&mut self) -> u8 {
        *self = BitpoolAbr::new(self.floor, self.ceiling);
        self.ceiling
    }
}

/// The A2DP SBC encoder: one SBC frame (blocks x subbands PCM frames, 128
/// by default) per `encode`
pub struct SbcEncoder {
    kind: CodecKind,
    frame: SbcFrameEncoder,
    /// Length of the frames encoded now (ABR changes it)
    frame_len: usize,
    /// ... and of those at the ceiling, the largest
    max_frame_len: usize,
    sample_rate: u32,
    /// Mono frames from stereo input: the input mixed down into this
    downmix: Option<Vec<i16>>,
    /// Adaptive bitpool: Some when EncoderConfig::abr is set and the floor
    /// is below the bitpool asked for
    abr: Option<BitpoolAbr>,
}

impl SbcEncoder {
    /// SBC: joint stereo (mono for one channel) unless the profile asks for
    /// another setup, the bitpool by quality within the sink's range
    pub fn new(config: &EncoderConfig) -> Result<SbcEncoder, EncoderError> {
        let mode = config.peer.sbc.channel_mode(CodecKind::Sbc, config.channels);
        let wanted = match config.peer.sbc_bitpool {
            0 => auto_bitpool(config.quality, mode),
            n => n.min(SBC_MAX_BITPOOL),
        };
        // Never below the profile's minimum
        let wanted = wanted.max(config.peer.sbc.min_bitpool.min(SBC_MAX_BITPOOL));
        let bitpool = sink_bitpool(wanted, config);
        Self::with_mode(CodecKind::Sbc, config, bitpool)
    }

    /// SBC XQ: dual channel (mono for one channel), [`xq_bitpool`] within
    /// the sink's range
    pub fn new_xq(config: &EncoderConfig) -> Result<SbcEncoder, EncoderError> {
        // Never above the configuration's maximum for the rate
        let cap = xq_max_bitpool(config.sample_rate);
        let wanted = match config.peer.sbc_bitpool {
            0 => xq_bitpool(config.quality, config.sample_rate),
            n => n.min(cap),
        };
        // Never below the profile's minimum
        let wanted = wanted.max(config.peer.sbc.min_bitpool.min(cap));
        let bitpool = sink_bitpool(wanted, config);
        Self::with_mode(CodecKind::SbcXq, config, bitpool)
    }

    fn with_mode(kind: CodecKind, config: &EncoderConfig, bitpool: u8) -> Result<SbcEncoder, EncoderError> {
        let unsupported = |what: String| EncoderError::Unsupported { codec: kind, what };
        if config.sample_rate != 44100 && config.sample_rate != 48000 {
            return Err(unsupported(format!("sample rate {} Hz (44100 or 48000)", config.sample_rate)));
        }
        if !matches!(config.channels, 1 | 2) {
            return Err(unsupported(format!("{} channels", config.channels)));
        }
        let params = config.peer.sbc;
        let channel_mode = params.channel_mode(kind, config.channels);
        let frame = SbcFrameEncoder::new(SbcConfig {
            sample_rate: config.sample_rate,
            channel_mode,
            blocks: params.blocks(),
            subbands: params.subbands(),
            allocation: params.allocation(),
            bitpool,
        })?;
        // Bitpool is never 0 here, so every frame has exactly this length
        let frame_len = frame.max_frame_len();
        let downmix = (config.channels == 2 && channel_mode == ChannelMode::Mono).then(|| vec![0; frame.pcm_samples()]);
        let ceiling = frame.config().effective_bitpool();
        let abr = config.abr.then(|| {
            let floor = match params.min_bitpool {
                0 => abr_floor(kind, channel_mode, config.sample_rate, config.peer.sbc_min_bitpool, ceiling),
                n => user_abr_floor(n, config.peer.sbc_min_bitpool, ceiling),
            };
            tracing::debug!("{kind}: ABR bitpool {floor}..{ceiling}");
            BitpoolAbr::new(floor, ceiling)
        });
        let abr = abr.filter(|a| {
            if a.floor >= a.ceiling {
                tracing::info!("{kind}: no ABR: bitpool {ceiling} is already at its floor");
            }
            a.floor < a.ceiling
        });
        Ok(SbcEncoder {
            kind,
            frame,
            frame_len,
            max_frame_len: frame_len,
            sample_rate: config.sample_rate,
            downmix,
            abr,
        })
    }

    fn pcm_frames(&self) -> usize {
        self.frame.config().pcm_frames_per_frame()
    }

    /// Length of the SBC frames encoded now (all of them without ABR)
    pub fn frame_len(&self) -> usize {
        self.frame_len
    }

    /// The bitpool of the frames encoded now
    pub fn bitpool(&self) -> u8 {
        self.frame.config().bitpool
    }

    /// The adaptive bitpool's range (floor, ceiling), when ABR runs
    pub fn abr_range(&self) -> Option<(u8, u8)> {
        self.abr.map(|a| (a.floor, a.ceiling))
    }

    fn set_bitpool(&mut self, bitpool: u8) {
        if bitpool == self.bitpool() {
            return;
        }
        self.frame.set_bitpool(bitpool);
        self.frame_len = self.frame.max_frame_len();
    }
}

impl Encoder for SbcEncoder {
    fn kind(&self) -> CodecKind {
        self.kind
    }

    fn pcm_format(&self) -> PcmFormat {
        PcmFormat::I16
    }

    fn pcm_frames_per_encode(&self) -> usize {
        self.pcm_frames()
    }

    /// The frame length at the ceiling: ABR only ever shrinks frames
    fn max_output_bytes(&self) -> usize {
        self.max_frame_len
    }

    fn encode(&mut self, pcm: Pcm<'_>, out: &mut [u8]) -> Result<Encoded, EncoderError> {
        let Pcm::I16(pcm) = pcm else {
            return Err(EncoderError::Encode { codec: self.kind, what: "SBC takes 16-bit PCM".into() });
        };
        let bytes = match self.downmix.as_mut() {
            Some(mono) => {
                if pcm.len() != 2 * mono.len() {
                    return Err(EncoderError::InputLength { expected: 2 * mono.len(), got: pcm.len() });
                }
                for (m, [l, r]) in mono.iter_mut().zip(pcm.as_chunks::<2>().0) {
                    *m = ((*l as i32 + *r as i32) >> 1) as i16;
                }
                self.frame.encode(mono, out)?
            }
            None => self.frame.encode(pcm, out)?,
        };
        Ok(Encoded { bytes, frames: 1 })
    }

    /// As v1: 8 * frame length * sample rate / (subbands * blocks), in kbps,
    /// rounded down; under ABR that of the bitpool now
    fn bitrate_kbps(&self) -> u32 {
        bitrate_kbps(self.frame_len, self.sample_rate, self.pcm_frames())
    }

    /// One [`BitpoolAbr::step`]: call every 100 ms with the send queue
    /// depth. The new bitpool applies from the next frame. Does nothing
    /// without ABR.
    fn abr_adjust(&mut self, queue_depth: u32) {
        let Some(abr) = self.abr.as_mut() else { return };
        let before = self.frame.config().bitpool;
        let bitpool = abr.step(before, queue_depth);
        if bitpool != before {
            self.set_bitpool(bitpool);
            tracing::debug!(
                "{}: ABR bitpool {before} -> {bitpool} ({} kbps, queue {queue_depth})",
                self.kind,
                self.bitrate_kbps()
            );
        }
    }

    fn abr_active(&self) -> bool {
        self.abr.is_some()
    }

    /// Back to the ceiling
    fn abr_reset(&mut self) {
        let Some(abr) = self.abr.as_mut() else { return };
        let bitpool = abr.reset();
        self.set_bitpool(bitpool);
    }

    fn details(&self) -> EncoderDetails {
        let c = *self.frame.config();
        EncoderDetails {
            frame_bytes: Some(self.frame_len as u32),
            sbc: Some(SbcConfig { bitpool: c.effective_bitpool(), ..c }),
            sbc_abr: self.abr_range(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PeerParams;

    fn config(sample_rate: u32, channels: u32, quality: Quality) -> EncoderConfig {
        EncoderConfig {
            sample_rate,
            channels,
            quality,
            mtu: 895,
            bits_per_sample: 16,
            abr: false,
            peer: PeerParams::default(),
        }
    }

    #[test]
    fn profile_bitpool_overrides_quality_within_limits() {
        let with = |bitpool: u8, peer_max: u8| EncoderConfig {
            peer: crate::PeerParams { sbc_bitpool: bitpool, sbc_max_bitpool: peer_max, ..Default::default() },
            ..config(44100, 2, Quality::Mobile)
        };
        assert_eq!(SbcEncoder::new(&with(0, 0)).unwrap().bitpool(), 19);
        assert_eq!(SbcEncoder::new(&with(40, 0)).unwrap().bitpool(), 40);
        assert_eq!(SbcEncoder::new(&with(200, 0)).unwrap().bitpool(), 53);
        // The sink's range still wins
        assert_eq!(SbcEncoder::new(&with(50, 35)).unwrap().bitpool(), 35);
        // XQ never goes above its configured maximum
        assert_eq!(SbcEncoder::new_xq(&with(53, 0)).unwrap().bitpool(), xq_max_bitpool(44100));
    }

    #[test]
    fn frame_length_and_bitrate() {
        // (rate, channels, quality, bitpool, frame length, kbps)
        let cases = [
            (44100, 2, Quality::High, 53, 119, 327),
            (44100, 2, Quality::Standard, 35, 83, 228),
            (44100, 2, Quality::Mobile, 19, 51, 140),
            (48000, 2, Quality::High, 53, 119, 357),
            (48000, 2, Quality::Standard, 35, 83, 249),
            (48000, 2, Quality::Mobile, 19, 51, 153),
            (44100, 1, Quality::High, 53, 114, 314),
            (44100, 1, Quality::Standard, 35, 78, 214),
            (44100, 1, Quality::Mobile, 19, 46, 126),
            (48000, 1, Quality::High, 53, 114, 342),
            (48000, 1, Quality::Standard, 35, 78, 234),
            (48000, 1, Quality::Mobile, 19, 46, 138),
        ];
        for (rate, ch, quality, bitpool, len, kbps) in cases {
            let mut e = SbcEncoder::new(&config(rate, ch, quality)).unwrap();
            assert_eq!(e.bitpool(), bitpool);
            assert_eq!(e.frame_len(), len, "{rate} {ch} {quality:?}");
            assert_eq!(e.max_output_bytes(), len);
            assert_eq!(e.bitrate_kbps(), kbps, "{rate} {ch} {quality:?}");
            assert_eq!(e.pcm_frames_per_encode(), 128);
            assert_eq!(e.pcm_format(), PcmFormat::I16);

            // Every frame has that length, whatever the input
            let mut out = vec![0u8; len];
            let mut seed = 1u32;
            for i in 0..50 {
                let pcm: Vec<i16> = (0..128 * ch as usize)
                    .map(|_| {
                        seed ^= seed << 13;
                        seed ^= seed >> 17;
                        seed ^= seed << 5;
                        if i % 10 == 0 { 0 } else { seed as i16 }
                    })
                    .collect();
                let r = e.encode(Pcm::I16(&pcm), &mut out).unwrap();
                assert_eq!(r, Encoded { bytes: len, frames: 1 });
                assert_eq!(out[0], 0x9C);
                assert_eq!(out[2], bitpool);
            }
        }
    }

    #[test]
    fn medium_high_is_high() {
        let e = SbcEncoder::new(&config(48000, 2, Quality::MediumHigh)).unwrap();
        assert_eq!(e.bitpool(), 53);
    }

    #[test]
    fn header() {
        let mut e = SbcEncoder::new(&config(44100, 2, Quality::High)).unwrap();
        let mut out = [0u8; 119];
        e.encode(Pcm::I16(&[0; 256]), &mut out).unwrap();
        // 44.1 kHz (10), 16 blocks (11), joint stereo (11), loudness (0), 8
        // subbands (1)
        assert_eq!(out[1], 0b1011_1101);
        let mut e = SbcEncoder::new(&config(48000, 1, Quality::High)).unwrap();
        e.encode(Pcm::I16(&[0; 128]), &mut out).unwrap();
        // 48 kHz, 16 blocks, mono, loudness, 8 subbands
        assert_eq!(out[1], 0b1111_0001);
    }

    #[test]
    fn unsupported_configurations() {
        for (rate, ch) in [(32000, 2), (16000, 1), (96000, 2), (44100, 0), (48000, 3)] {
            assert!(matches!(
                SbcEncoder::new(&config(rate, ch, Quality::High)),
                Err(EncoderError::Unsupported { codec: CodecKind::Sbc, .. })
            ));
        }
    }

    #[test]
    fn input_and_output_length_errors() {
        let mut e = SbcEncoder::new(&config(44100, 2, Quality::High)).unwrap();
        let mut out = [0u8; 119];
        for n in [0, 128, 255, 257, 512] {
            let pcm = vec![0i16; n];
            assert!(matches!(
                e.encode(Pcm::I16(&pcm), &mut out),
                Err(EncoderError::InputLength { expected: 256, got }) if got == n
            ));
        }
        assert!(matches!(
            e.encode(Pcm::I16(&[0; 256]), &mut [0u8; 118]),
            Err(EncoderError::OutputTooSmall { have: 118, need: 119 })
        ));
        assert!(matches!(e.encode(Pcm::I32(&[0; 256]), &mut out), Err(EncoderError::Encode { .. })));
        // Still usable
        assert_eq!(e.encode(Pcm::I16(&[0; 256]), &mut out).unwrap().bytes, 119);

        let mut e = SbcEncoder::new(&config(48000, 1, Quality::Mobile)).unwrap();
        assert!(matches!(
            e.encode(Pcm::I16(&[0; 256]), &mut out),
            Err(EncoderError::InputLength { expected: 128, got: 256 })
        ));
    }

    #[test]
    fn create_returns_sbc() {
        let e = crate::create(CodecKind::Sbc, &config(48000, 2, Quality::Standard)).unwrap();
        assert_eq!(e.kind(), CodecKind::Sbc);
        assert_eq!(e.max_output_bytes(), 83);
    }

    #[test]
    fn xq_frame_length_and_bitrate() {
        // (rate, channels, quality, bitpool, frame length, kbps, header byte 1)
        let cases = [
            (44100, 2, Quality::High, 47, 200, 551, 0b1011_0101),
            (44100, 2, Quality::Standard, 43, 184, 507, 0b1011_0101),
            (44100, 2, Quality::Mobile, 32, 140, 385, 0b1011_0101),
            (48000, 2, Quality::High, 43, 184, 552, 0b1111_0101),
            (48000, 2, Quality::Standard, 39, 168, 504, 0b1111_0101),
            (48000, 2, Quality::Mobile, 29, 128, 384, 0b1111_0101),
            (44100, 1, Quality::High, 47, 102, 281, 0b1011_0001),
            (48000, 1, Quality::High, 43, 94, 282, 0b1111_0001),
        ];
        for (rate, ch, quality, bitpool, len, kbps, hdr) in cases {
            let mut e = SbcEncoder::new_xq(&config(rate, ch, quality)).unwrap();
            assert_eq!(e.kind(), CodecKind::SbcXq);
            assert_eq!(e.bitpool(), bitpool, "{rate} {ch} {quality:?}");
            assert_eq!(e.frame_len(), len, "{rate} {ch} {quality:?}");
            assert_eq!(e.bitrate_kbps(), kbps, "{rate} {ch} {quality:?}");
            let mut out = vec![0u8; len];
            let pcm: Vec<i16> = (0..128 * ch as usize).map(|i| ((i * 7919) % 20000) as i16 - 10000).collect();
            for _ in 0..4 {
                assert_eq!(e.encode(Pcm::I16(&pcm), &mut out).unwrap(), Encoded { bytes: len, frames: 1 });
                assert_eq!((out[0], out[1], out[2]), (0x9C, hdr, bitpool));
            }
        }
        assert_eq!(SbcEncoder::new_xq(&config(44100, 2, Quality::MediumHigh)).unwrap().bitpool(), 47);
        assert_eq!((xq_max_bitpool(44100), xq_max_bitpool(48000)), (47, 43));
    }

    #[test]
    fn xq_fills_packets() {
        // High: three frames and the media payload header in one 2-DH5 (679
        // bytes less 4 L2CAP and 12 RTP), five in one 3-DH5 (1021 less 16)
        for rate in [44100, 48000] {
            let len = SbcEncoder::new_xq(&config(rate, 2, Quality::High)).unwrap().frame_len();
            let packet = |frames: usize| 1 + frames * len;
            assert!(packet(3) <= 679 - 16 && packet(4) > 679 - 16, "{rate}");
            assert!(packet(5) <= 1021 - 16 && packet(6) > 1021 - 16, "{rate}");
        }
    }

    #[test]
    fn sbc_bitpool_within_the_sink_range() {
        // (sink min, sink max, quality) -> bitpool
        for (min, max, quality, want) in [
            (0, 0, Quality::High, 53), // not known
            (2, 53, Quality::High, 53),
            (2, 35, Quality::High, 35),
            (2, 35, Quality::Standard, 35),
            (2, 35, Quality::Mobile, 19),
            (2, 250, Quality::High, 53),
            (25, 40, Quality::Mobile, 25),
        ] {
            let mut c = config(48000, 2, quality);
            c.peer.sbc_min_bitpool = min;
            c.peer.sbc_max_bitpool = max;
            let mut e = SbcEncoder::new(&c).unwrap();
            assert_eq!(e.bitpool(), want, "{min}-{max} {quality:?}");
            let mut out = vec![0u8; e.frame_len()];
            e.encode(Pcm::I16(&[0; 256]), &mut out).unwrap();
            assert_eq!(out[2], want);
        }
        // 35: 83-byte frames, as Standard
        let mut c = config(48000, 2, Quality::High);
        c.peer.sbc_max_bitpool = 35;
        assert_eq!(SbcEncoder::new(&c).unwrap().frame_len(), 83);
    }

    #[test]
    fn profile_minimum_bitpool() {
        let with = |min: u8, bitpool: u8, sink: (u8, u8), quality: Quality| EncoderConfig {
            peer: PeerParams {
                sbc: SbcParams { min_bitpool: min, ..Default::default() },
                sbc_bitpool: bitpool,
                sbc_min_bitpool: sink.0,
                sbc_max_bitpool: sink.1,
                ..Default::default()
            },
            ..config(44100, 2, quality)
        };
        // (min, bitpool, sink range, quality) -> bitpool
        for (min, bitpool, sink, quality, want) in [
            (0, 0, (2, 53), Quality::Mobile, 19),  // Auto: as before
            (30, 0, (2, 53), Quality::Mobile, 30), // the quality's below the minimum
            (30, 0, (2, 53), Quality::High, 53),
            (30, 40, (2, 53), Quality::High, 40),
            (45, 40, (2, 53), Quality::High, 45),  // the maximum is never below it
            (60, 0, (0, 0), Quality::Mobile, 53),  // never above SBC's 53
            (45, 0, (2, 35), Quality::Mobile, 35), // the sink's maximum wins
        ] {
            let e = SbcEncoder::new(&with(min, bitpool, sink, quality)).unwrap();
            assert_eq!(e.bitpool(), want, "{min} {bitpool} {sink:?} {quality:?}");
        }
        // SBC XQ: up to its cap of the rate
        assert_eq!(SbcEncoder::new_xq(&with(40, 0, (2, 53), Quality::Mobile)).unwrap().bitpool(), 40);
        assert_eq!(SbcEncoder::new_xq(&with(53, 0, (2, 53), Quality::Mobile)).unwrap().bitpool(), 47);
    }

    #[test]
    fn profile_minimum_bitpool_is_the_abr_floor() {
        let with = |min: u8, sink_min: u8| EncoderConfig {
            peer: PeerParams {
                sbc: SbcParams { min_bitpool: min, ..Default::default() },
                sbc_min_bitpool: sink_min,
                sbc_max_bitpool: 53,
                ..Default::default()
            },
            ..abr_config(48000, 2, Quality::High)
        };
        // Auto: Mobile's 19
        assert_eq!(SbcEncoder::new(&with(0, 2)).unwrap().abr_range(), Some((19, 53)));
        // Higher or lower than Mobile, the profile's minimum
        assert_eq!(SbcEncoder::new(&with(40, 2)).unwrap().abr_range(), Some((40, 53)));
        let mut e = SbcEncoder::new(&with(10, 2)).unwrap();
        assert_eq!(e.abr_range(), Some((10, 53)));
        for _ in 0..200 {
            e.abr_adjust(64);
        }
        assert_eq!(e.bitpool(), 10);
        // Still never below the sink's minimum
        assert_eq!(SbcEncoder::new(&with(10, 25)).unwrap().abr_range(), Some((25, 53)));
        // SBC XQ: the profile's minimum in place of its Mobile
        let e = SbcEncoder::new_xq(&with(20, 2)).unwrap();
        assert_eq!(e.abr_range(), Some((20, 43)));
        assert_eq!(user_abr_floor(1, 0, 53), 2);
        assert_eq!(user_abr_floor(50, 2, 40), 40);
    }

    #[test]
    fn xq_bitpool_capped_by_the_sink() {
        let mut c = config(44100, 2, Quality::High);
        c.peer.sbc_max_bitpool = 45;
        let e = SbcEncoder::new_xq(&c).unwrap();
        assert_eq!((e.bitpool(), e.frame_len()), (45, 192));
        c.peer.sbc_max_bitpool = 53;
        assert_eq!(SbcEncoder::new_xq(&c).unwrap().bitpool(), 47);
        let e = crate::create(CodecKind::SbcXq, &config(48000, 2, Quality::Standard)).unwrap();
        assert_eq!((e.kind(), e.max_output_bytes()), (CodecKind::SbcXq, 168));
        assert!(matches!(
            SbcEncoder::new_xq(&config(32000, 2, Quality::High)),
            Err(EncoderError::Unsupported { codec: CodecKind::SbcXq, .. })
        ));
    }

    fn with_params(rate: u32, ch: u32, quality: Quality, sbc: SbcParams, bitpool: u8) -> EncoderConfig {
        EncoderConfig {
            peer: PeerParams { sbc, sbc_bitpool: bitpool, ..Default::default() },
            ..config(rate, ch, quality)
        }
    }

    #[test]
    fn frame_length_and_bitrate_of_other_setups() {
        // (rate, blocks, subbands, mode, bitpool, frame length, kbps): the
        // A2DP specification's formula (12.9)
        use ChannelMode::*;
        let cases = [
            (44100, 16, 8, JointStereo, 53, 119, 327),
            (44100, 16, 8, Stereo, 53, 118, 325),
            (44100, 16, 8, DualChannel, 32, 140, 385),
            (44100, 16, 8, Mono, 31, 70, 192),
            (44100, 8, 4, JointStereo, 35, 44, 485),
            (48000, 8, 4, Stereo, 35, 43, 516),
            (44100, 4, 8, JointStereo, 53, 40, 441),
            (48000, 12, 8, JointStereo, 53, 93, 372),
            (48000, 12, 4, DualChannel, 20, 68, 544),
            (44100, 4, 4, Mono, 30, 21, 463),
        ];
        for (rate, blocks, subbands, mode, bitpool, len, kbps) in cases {
            let c = SbcConfig {
                sample_rate: rate,
                channel_mode: mode,
                blocks,
                subbands,
                allocation: Allocation::Snr,
                bitpool,
            };
            assert_eq!(c.frame_len(), len, "{rate} {blocks}x{subbands} {mode:?} {bitpool}");
            assert_eq!(bitrate_kbps(len, rate, c.pcm_frames_per_frame()), kbps, "{rate} {blocks}x{subbands} {mode:?}");
            // The encoder agrees
            let p = SbcParams { blocks, subbands, allocation: None, channel_mode: Some(mode), min_bitpool: 0 };
            let e = SbcEncoder::new(&with_params(rate, 2, Quality::High, p, bitpool)).unwrap();
            assert_eq!((e.frame_len(), e.bitrate_kbps()), (len, kbps), "{rate} {blocks}x{subbands} {mode:?}");
        }
    }

    #[test]
    fn every_block_and_subband_count_and_allocation_encodes() {
        let mut seed = 7u32;
        for blocks in [4u8, 8, 12, 16] {
            for subbands in [4u8, 8] {
                for allocation in [Allocation::Loudness, Allocation::Snr] {
                    for (ch, mode) in [
                        (2, ChannelMode::JointStereo),
                        (2, ChannelMode::Stereo),
                        (2, ChannelMode::Mono),
                        (1, ChannelMode::Mono),
                    ] {
                        let p = SbcParams {
                            blocks,
                            subbands,
                            allocation: Some(allocation),
                            channel_mode: Some(mode),
                            min_bitpool: 0,
                        };
                        let mut e = SbcEncoder::new(&with_params(48000, ch, Quality::Standard, p, 0)).unwrap();
                        let what = format!("{blocks}x{subbands} {allocation:?} {ch} ch {mode:?}");
                        let frames = blocks as usize * subbands as usize;
                        assert_eq!(e.pcm_frames_per_encode(), frames, "{what}");
                        let len = e.frame_len();
                        let want = SbcConfig {
                            sample_rate: 48000,
                            channel_mode: mode,
                            blocks,
                            subbands,
                            allocation,
                            bitpool: 35,
                        }
                        .frame_len();
                        assert_eq!(len, want, "{what}");
                        let mut out = vec![0u8; len];
                        for _ in 0..8 {
                            let pcm: Vec<i16> = (0..frames * ch as usize)
                                .map(|_| {
                                    seed ^= seed << 13;
                                    seed ^= seed >> 17;
                                    seed ^= seed << 5;
                                    (seed >> 3) as i16
                                })
                                .collect();
                            let r = e.encode(Pcm::I16(&pcm), &mut out).unwrap();
                            assert_eq!(r, Encoded { bytes: len, frames: 1 }, "{what}");
                            // Header: 48 kHz, blocks, mode, allocation, subbands
                            let b =
                                (blocks / 4 - 1) << 4 | (mode as u8) << 2 | (allocation as u8) << 1 | (subbands / 8);
                            assert_eq!((out[0], out[1], out[2]), (0x9C, 0xC0 | b, 35), "{what}");
                        }
                        // The wrong input length is refused
                        assert!(matches!(
                            e.encode(Pcm::I16(&vec![0; frames * ch as usize + 2]), &mut out),
                            Err(EncoderError::InputLength { .. })
                        ));
                    }
                }
            }
        }
    }

    #[test]
    fn mono_from_stereo_mixes_down() {
        let p = SbcParams { channel_mode: Some(ChannelMode::Mono), ..Default::default() };
        let mut mixed = SbcEncoder::new(&with_params(44100, 2, Quality::High, p, 0)).unwrap();
        let mut mono = SbcEncoder::new(&config(44100, 1, Quality::High)).unwrap();
        assert_eq!((mixed.frame_len(), mixed.pcm_frames_per_encode()), (mono.frame_len(), 128));
        let (mut a, mut b) = (vec![0u8; mono.frame_len()], vec![0u8; mono.frame_len()]);
        for n in 0..4 {
            let m: Vec<i16> = (0..128).map(|i| ((i * 977 + n * 31) % 16000) as i16 - 8000).collect();
            let lr: Vec<i16> = m.iter().flat_map(|&s| [s, s]).collect();
            mixed.encode(Pcm::I16(&lr), &mut a).unwrap();
            mono.encode(Pcm::I16(&m), &mut b).unwrap();
            assert_eq!(a, b);
        }
    }

    #[test]
    fn auto_bitpool_by_channel_mode() {
        // Dual channel halves the joint stereo bitpool per channel: about
        // the same bit rate
        let dual = SbcParams { channel_mode: Some(ChannelMode::DualChannel), ..Default::default() };
        let e = SbcEncoder::new(&with_params(44100, 2, Quality::High, dual, 0)).unwrap();
        assert_eq!((e.bitpool(), e.frame_len(), e.bitrate_kbps()), (27, 120, 330));
        assert_eq!(auto_bitpool(Quality::Mobile, ChannelMode::DualChannel), 10);
        assert_eq!(auto_bitpool(Quality::Standard, ChannelMode::Stereo), 35);
        // SBC XQ keeps its dual channel and bitpools whatever the mode asked
        let stereo = SbcParams { channel_mode: Some(ChannelMode::Stereo), ..Default::default() };
        let e = SbcEncoder::new_xq(&with_params(44100, 2, Quality::High, stereo, 0)).unwrap();
        assert_eq!((e.bitpool(), e.frame_len()), (47, 200));
        // ... but its frame setup
        let small = SbcParams { blocks: 8, ..Default::default() };
        let e = SbcEncoder::new_xq(&with_params(44100, 2, Quality::High, small, 0)).unwrap();
        assert_eq!((e.frame_len(), e.pcm_frames_per_encode()), (4 + 8 + 94, 64));
    }

    fn abr_config(rate: u32, ch: u32, quality: Quality) -> EncoderConfig {
        EncoderConfig { abr: true, ..config(rate, ch, quality) }
    }

    #[test]
    fn abr_policy_down_on_congestion_with_a_pause() {
        let mut a = BitpoolAbr::new(19, 53);
        // The first congested step lowers at once, by 2
        assert_eq!(a.step(53, ABR_DOWN_DEPTH), 51);
        // ... then waits 0.5 s (5 steps) before the next
        let mut bp = 51;
        for _ in 0..ABR_DOWN_TICKS - 1 {
            bp = a.step(bp, 64);
            assert_eq!(bp, 51);
        }
        assert_eq!(a.step(bp, 64), 49);
        // A queue below the threshold lowers nothing
        let mut a = BitpoolAbr::new(19, 53);
        assert_eq!(a.step(53, ABR_DOWN_DEPTH - 1), 53);
    }

    #[test]
    fn abr_policy_floor() {
        let mut a = BitpoolAbr::new(19, 53);
        let mut bp = 53;
        for _ in 0..200 {
            bp = a.step(bp, 64);
        }
        assert_eq!(bp, 19);
        // An odd distance ends on the floor, not below it
        let mut a = BitpoolAbr::new(20, 23);
        assert_eq!(a.step(23, 64), 21);
        for _ in 0..ABR_DOWN_TICKS {
            bp = a.step(21, 64);
        }
        assert_eq!(bp, 20);
        // The floor never goes above the ceiling
        assert_eq!(BitpoolAbr::new(30, 20).floor, 20);
    }

    #[test]
    fn abr_policy_up_slowly_to_the_ceiling() {
        let mut a = BitpoolAbr::new(19, 53);
        let mut bp = a.step(53, 64);
        assert_eq!(bp, 51);
        // One step up per second (10 steps) of a drained queue
        for _ in 0..ABR_UP_TICKS - 1 {
            bp = a.step(bp, 0);
            assert_eq!(bp, 51);
        }
        assert_eq!(a.step(bp, ABR_UP_DEPTH), 52);
        // A queue between the thresholds starts the second over
        let mut bp = 52;
        for _ in 0..ABR_UP_TICKS - 1 {
            bp = a.step(bp, 0);
        }
        bp = a.step(bp, ABR_UP_DEPTH + 1);
        assert_eq!(bp, 52);
        for _ in 0..ABR_UP_TICKS - 1 {
            bp = a.step(bp, 1);
        }
        assert_eq!(bp, 52);
        assert_eq!(a.step(bp, 1), 53);
        // Never above the ceiling
        for _ in 0..100 {
            bp = a.step(bp, 0);
        }
        assert_eq!(bp, 53);
        // 34 steps (34 s) from the floor back to 53
        let mut a = BitpoolAbr::new(19, 53);
        let (mut bp, mut steps) = (19, 0);
        while bp < 53 {
            bp = a.step(bp, 0);
            steps += 1;
        }
        assert_eq!(steps, 34 * ABR_UP_TICKS);
    }

    #[test]
    fn abr_policy_reset() {
        let mut a = BitpoolAbr::new(19, 53);
        let bp = a.step(53, 64);
        assert_eq!(bp, 51);
        assert_eq!(a.reset(), 53);
        // The pause after a step down is forgotten too
        assert_eq!(a.step(53, 64), 51);
    }

    #[test]
    fn abr_floors() {
        use ChannelMode::*;
        let sbc = |mode, min, ceiling| abr_floor(CodecKind::Sbc, mode, 48000, min, ceiling);
        assert_eq!(sbc(JointStereo, 2, 53), 19);
        assert_eq!(sbc(Stereo, 2, 53), 19);
        assert_eq!(sbc(Mono, 2, 53), 19);
        // Dual channel: Mobile's 10 per channel, raised to PipeWire's 12
        assert_eq!(sbc(DualChannel, 2, 27), 12);
        // The sink's minimum
        assert_eq!(sbc(JointStereo, 25, 53), 25);
        // Never above the ceiling
        assert_eq!(sbc(JointStereo, 2, 15), 15);
        let xq = |rate| abr_floor(CodecKind::SbcXq, DualChannel, rate, 2, 47);
        assert_eq!((xq(44100), xq(48000)), (32, 29));
    }

    #[test]
    fn sbc_abr_lowers_the_bitpool_of_the_next_frames() {
        let mut e = SbcEncoder::new(&abr_config(48000, 2, Quality::High)).unwrap();
        assert!(e.abr_active());
        assert_eq!(e.abr_range(), Some((19, 53)));
        assert_eq!((e.bitpool(), e.frame_len(), e.bitrate_kbps()), (53, 119, 357));
        assert_eq!(e.max_output_bytes(), 119);
        let mut out = vec![0u8; e.max_output_bytes()];
        let pcm: Vec<i16> = (0..256).map(|i| ((i * 7919) % 20000) as i16 - 10000).collect();
        assert_eq!(e.encode(Pcm::I16(&pcm), &mut out).unwrap().bytes, 119);
        assert_eq!(out[2], 53);

        // Congestion: 53 -> 51, the frames shrink, the buffer size stays
        e.abr_adjust(64);
        let want = SbcConfig { bitpool: 51, ..*e.frame.config() }.frame_len();
        assert_eq!((e.bitpool(), e.frame_len()), (51, want));
        assert_eq!(want, 115);
        assert_eq!(e.max_output_bytes(), 119);
        assert_eq!(e.bitrate_kbps(), bitrate_kbps(115, 48000, 128));
        let r = e.encode(Pcm::I16(&pcm), &mut out).unwrap();
        assert_eq!(r, Encoded { bytes: 115, frames: 1 });
        // The header carries the new bitpool, the rest of it unchanged
        assert_eq!((out[0], out[1], out[2]), (0x9C, 0b1111_1101, 51));
        let d = e.details();
        assert_eq!((d.frame_bytes, d.sbc.unwrap().bitpool, d.sbc_abr), (Some(115), 51, Some((19, 53))));

        // Down to the floor: 19, 51-byte frames
        for _ in 0..200 {
            e.abr_adjust(64);
        }
        assert_eq!((e.bitpool(), e.frame_len()), (19, 51));
        let r = e.encode(Pcm::I16(&pcm), &mut out).unwrap();
        assert_eq!((r.bytes, out[2]), (51, 19));

        // Back up one step per second, and at once after a reset
        for _ in 0..ABR_UP_TICKS {
            e.abr_adjust(0);
        }
        assert_eq!(e.bitpool(), 20);
        e.abr_reset();
        assert_eq!((e.bitpool(), e.frame_len(), e.bitrate_kbps()), (53, 119, 357));
        assert_eq!(e.encode(Pcm::I16(&pcm), &mut out).unwrap().bytes, 119);
        assert_eq!(out[2], 53);
    }

    #[test]
    fn sbc_abr_frames_match_a_fresh_encoder_at_that_bitpool() {
        // The changed bitpool encodes as an encoder set up with it (the
        // analysis history aside: compare from silence)
        let mut e = SbcEncoder::new(&abr_config(44100, 2, Quality::High)).unwrap();
        e.abr_adjust(64);
        let mut fresh = SbcEncoder::new(&EncoderConfig {
            peer: PeerParams { sbc_bitpool: 51, ..Default::default() },
            ..config(44100, 2, Quality::High)
        })
        .unwrap();
        let (mut a, mut b) = (vec![0u8; 119], vec![0u8; 119]);
        let n = e.encode(Pcm::I16(&[0; 256]), &mut a).unwrap().bytes;
        let m = fresh.encode(Pcm::I16(&[0; 256]), &mut b).unwrap().bytes;
        assert_eq!((n, m), (115, 115));
        assert_eq!(a[..n], b[..m]);
    }

    #[test]
    fn sbc_xq_abr() {
        let mut e = SbcEncoder::new_xq(&abr_config(44100, 2, Quality::High)).unwrap();
        assert_eq!(e.abr_range(), Some((32, 47)));
        let mut out = vec![0u8; e.max_output_bytes()];
        assert_eq!(out.len(), 200);
        for _ in 0..100 {
            e.abr_adjust(64);
        }
        assert_eq!((e.bitpool(), e.frame_len(), e.bitrate_kbps()), (32, 140, 385));
        let r = e.encode(Pcm::I16(&[0; 256]), &mut out).unwrap();
        assert_eq!((r.bytes, out[1], out[2]), (140, 0b1011_0101, 32));
        e.abr_reset();
        assert_eq!(e.bitpool(), 47);
    }

    #[test]
    fn no_abr_without_the_setting_or_room() {
        // Not asked: the queue changes nothing
        let mut e = SbcEncoder::new(&config(48000, 2, Quality::High)).unwrap();
        assert!(!e.abr_active());
        e.abr_adjust(64);
        assert_eq!(e.bitpool(), 53);
        assert_eq!(e.details().sbc_abr, None);
        // Mobile is the floor already
        let e = SbcEncoder::new(&abr_config(48000, 2, Quality::Mobile)).unwrap();
        assert!(!e.abr_active());
        let e = SbcEncoder::new_xq(&abr_config(48000, 2, Quality::Mobile)).unwrap();
        assert!(!e.abr_active());
        // The sink's minimum is the floor
        let mut c = abr_config(48000, 2, Quality::High);
        c.peer.sbc_min_bitpool = 40;
        c.peer.sbc_max_bitpool = 53;
        let mut e = SbcEncoder::new(&c).unwrap();
        for _ in 0..100 {
            e.abr_adjust(64);
        }
        assert_eq!(e.bitpool(), 40);
        // The profile's minimum at the bitpool: no room either
        let mut c = abr_config(48000, 2, Quality::High);
        c.peer.sbc.min_bitpool = 53;
        assert!(!SbcEncoder::new(&c).unwrap().abr_active());
        // FastStream keeps its fixed bitpool
        let f = crate::create(CodecKind::FastStream, &abr_config(48000, 2, Quality::High)).unwrap();
        assert!(!f.abr_active());
    }

    #[test]
    fn keys() {
        for m in ChannelMode::ALL {
            assert_eq!(ChannelMode::from_key(m.key()), Some(m));
        }
        assert_eq!(Allocation::from_key("snr"), Some(Allocation::Snr));
        assert_eq!(Allocation::from_key(""), None);
        assert_eq!(ChannelMode::from_key("auto"), None);
        let d = SbcParams::default();
        assert_eq!((d.blocks(), d.subbands(), d.allocation()), (16, 8, Allocation::Loudness));
        assert_eq!(d.channel_mode(CodecKind::Sbc, 2), ChannelMode::JointStereo);
        assert_eq!(d.channel_mode(CodecKind::SbcXq, 2), ChannelMode::DualChannel);
        assert_eq!(d.channel_mode(CodecKind::Sbc, 1), ChannelMode::Mono);
    }

    #[test]
    fn frame_config_validation() {
        let ok = SbcConfig {
            sample_rate: 16000,
            channel_mode: ChannelMode::DualChannel,
            blocks: 12,
            subbands: 4,
            allocation: Allocation::Snr,
            bitpool: 200,
        };
        let e = SbcFrameEncoder::new(ok).unwrap();
        assert_eq!(ok.effective_bitpool(), 64);
        assert_eq!(e.pcm_samples(), 96);
        assert!(SbcFrameEncoder::new(SbcConfig { blocks: 6, ..ok }).is_err());
        assert!(SbcFrameEncoder::new(SbcConfig { subbands: 6, ..ok }).is_err());
        assert!(SbcFrameEncoder::new(SbcConfig { sample_rate: 22050, ..ok }).is_err());
    }
}
