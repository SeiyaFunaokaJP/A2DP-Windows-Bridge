//! Codec information (A2DP "Media Codec Specific Information Elements"):
//! the local stream endpoints, the remote's capabilities, the configuration
//! sent with SET_CONFIGURATION, and the codec choice.
//!
//! Vendor codec information here is what BTstack calls the media codec
//! information: vendor ID (4 bytes LE), codec ID (2 bytes LE), then the
//! codec-specific bytes. The values follow v1 (btstack_transport.cpp), which
//! follows Android and PipeWire.

use crate::sbc::{self, Allocation, ChannelMode};
use crate::{AutoOrder, CodecKind};

pub const LDAC_VENDOR_ID: u32 = 0x0000_012D; // Sony
pub const LDAC_CODEC_ID: u16 = 0x00AA;
pub const APTXHD_VENDOR_ID: u32 = 0x0000_00D7; // Qualcomm
pub const APTXHD_CODEC_ID: u16 = 0x0024;
pub const APTXLL_VENDOR_ID: u32 = 0x0000_000A; // CSR
pub const APTXLL_VENDOR_ID2: u32 = 0x0000_00D7; // some sinks list aptX LL under Qualcomm
pub const APTXLL_CODEC_ID: u16 = 0x0002;
pub const APTX_VENDOR_ID: u32 = 0x0000_004F; // APT
pub const APTX_CODEC_ID: u16 = 0x0001;
/// aptX Adaptive (Qualcomm)
pub const APTXAD_VENDOR_ID: u32 = 0x0000_00D7;
pub const APTXAD_CODEC_ID: u16 = 0x00AD;
pub const LHDCV5_VENDOR_ID: u32 = 0x0000_053A; // Savitech
pub const LHDCV5_CODEC_ID: u16 = 0x4C35;
pub const OPUS_VENDOR_ID: u32 = 0x0000_00E0; // Google
pub const OPUS_CODEC_ID: u16 = 0x0001;
pub const OPUS05_VENDOR_ID: u32 = 0x0000_05F1; // The Linux Foundation (PipeWire)
pub const OPUS05_CODEC_ID: u16 = 0x1005;
pub const FASTSTREAM_VENDOR_ID: u32 = 0x0000_000A; // CSR
pub const FASTSTREAM_CODEC_ID: u16 = 0x0001;

/// AVDTP media codec types
pub const CODEC_TYPE_SBC: u8 = 0x00;
pub const CODEC_TYPE_AAC: u8 = 0x02;
pub const CODEC_TYPE_VENDOR: u8 = 0xFF;

// LDAC byte 6: 0x20 44.1k, 0x10 48k, 0x08 88.2k, 0x04 96k; byte 7: 0x04 mono, 0x02 dual, 0x01 stereo
const LDAC_FREQ_ALL: u8 = 0x3C;
/// LDAC rates by byte 6 bit, highest first
const LDAC_RATES: [(u8, u32); 4] = [(0x04, 96000), (0x08, 88200), (0x10, 48000), (0x20, 44100)];
const LDAC_CH_ALL: u8 = 0x07;

// aptX family byte 6: high nibble frequency (0x20 44.1k, 0x10 48k), low nibble channels (0x02 stereo, 0x01 mono)
const APTX_FREQ_44100: u8 = 0x20;
const APTX_FREQ_48000: u8 = 0x10;
const APTX_FREQ_MASK: u8 = 0xF0;
const APTX_CH_STEREO: u8 = 0x02;
const APTX_CAPS_ALL: u8 = APTX_FREQ_44100 | APTX_FREQ_48000 | APTX_CH_STEREO;
const APTX_CONFIG_DEFAULT: u8 = APTX_FREQ_48000 | APTX_CH_STEREO;

/// aptX HD: classic aptX 7 bytes + 4 reserved
const APTXHD_INFO_LEN: usize = 11;
/// aptX LL: classic 7 bytes + flags (bit0 bidirect_link, bit1 has_new_caps);
/// with has_new_caps the sink appends 9 bytes
const APTXLL_INFO_LEN: usize = 8;
const APTXLL_EXT_INFO_LEN: usize = 17;
const APTXLL_HAS_NEW_CAPS: u8 = 0x02;
/// PipeWire's defaults for the extended LL configuration, raised by 3/2
const APTXLL_TARGET_LEVEL: u16 = 180 * 3 / 2;
const APTXLL_INITIAL_LEVEL: u16 = 360 * 3 / 2;
const APTXLL_GOOD_LEVEL: u16 = 180 * 3 / 2;
const APTXLL_SRA_MAX_RATE: u8 = 50;
const APTXLL_SRA_AVG_TIME: u8 = 1;

// LHDC V5 (Android a2dp_vendor_lhdcv5_constants.h): 11 bytes
//   P6 rates 0x20 44.1k 0x10 48k 0x04 96k 0x01 192k
//   P7 bit depth 0x04 16 0x02 24, bits 7..4 bit rate limits
//   P8 version bitmap (0x01 V5 ver.1), 0x10 5 ms frames
const LHDCV5_INFO_LEN: usize = 11;
pub const LHDCV5_SR_44100: u8 = 0x20;
pub const LHDCV5_SR_48000: u8 = 0x10;
pub const LHDCV5_SR_96000: u8 = 0x04;
pub const LHDCV5_SR_192000: u8 = 0x01;
const LHDCV5_SR_MASK: u8 = 0x35;
const LHDCV5_BITS_16: u8 = 0x04;
const LHDCV5_BITS_24: u8 = 0x02;
const LHDCV5_BITS_MASK: u8 = 0x07;
const LHDCV5_BITRATE_LIMITS_MASK: u8 = 0xF0;
const LHDCV5_VERSION_1: u8 = 0x01;
const LHDCV5_VERSION_MASK: u8 = 0x0F;
const LHDCV5_FRAME_5MS: u8 = 0x10;
const LHDCV5_CAPS_P8: u8 = LHDCV5_VERSION_1 | LHDCV5_FRAME_5MS;

// Opus (Android a2dp_vendor_opus_constants.h; PipeWire OPUS_G_*): one byte,
//   bits 2..0 channel mode 0x01 mono 0x02 stereo 0x04 dual mono,
//   bits 4..3 frame duration 0x08 10 ms 0x10 20 ms, bit 7 0x80 48 kHz
const OPUS_INFO_LEN: usize = 7;
pub const OPUS_CH_MONO: u8 = 0x01;
pub const OPUS_CH_STEREO: u8 = 0x02;
pub const OPUS_CH_DUAL_MONO: u8 = 0x04;
pub const OPUS_FRAME_10MS: u8 = 0x08;
pub const OPUS_FRAME_20MS: u8 = 0x10;
const OPUS_FRAME_MASK: u8 = 0x18;
pub const OPUS_FREQ_48000: u8 = 0x80;
/// We offer mono and stereo, 10 and 20 ms
const OPUS_CAPS: u8 = OPUS_FREQ_48000 | OPUS_FRAME_10MS | OPUS_FRAME_20MS | OPUS_CH_MONO | OPUS_CH_STEREO;
/// Android's default configuration: 48 kHz, 20 ms, stereo
const OPUS_CONFIG_DEFAULT: u8 = OPUS_FREQ_48000 | OPUS_FRAME_20MS | OPUS_CH_STEREO;

// Opus 05 (PipeWire a2dp-codec-caps.h, a2dp_opus_05_t): 24 bytes, the
// vendor header then two 9-byte directions, "main" (source to sink: the
// music) and "bidi" (the return channel; 0 channels = none), each
//   +0 channels (capabilities: the most), +1 coupled streams,
//   +2..+5 audio location (LE; Bluetooth audio location bits), +6 frame
//   durations (0x01 2.5 ms, 0x02 5, 0x04 10, 0x08 20, 0x10 40), +7..+8
//   bit rate (LE, units of 1024 bps; 0 = no limit)
// BlueCaster offers and configures the main direction only: stereo (2
// channels, 1 coupled stream, front left + right) or mono, 10 or 20 ms.
const OPUS05_INFO_LEN: usize = 24;
const OPUS05_MAIN: usize = 6;
const OPUS05_BIDI: usize = 15;
pub const OPUS05_FRAME_10MS: u8 = 0x04;
pub const OPUS05_FRAME_20MS: u8 = 0x08;
const OPUS05_LOCATION_STEREO: u32 = 0x0000_0003; // front left, front right
/// PipeWire's largest Opus 05 stereo bit rate (BITRATE_MAX), the
/// configuration's limit when the sink sets none: 320000 / 1024
pub const OPUS05_MAX_BITRATE_UNITS: u16 = 312;
/// The bit rate field's unit
pub const OPUS05_BITRATE_UNIT: u32 = 1024;

// aptX Adaptive (AKS-Logic/aptx-adaptive-linux spec/a2dp-integration.md §1,
// §7G-§7J; the public CodeAurora / LineageOS headers): 40 bytes, the vendor
// header then
//   6: bits 7..3 rates, bits 2..0 the source type (0x02: a host encoder,
//      the sink times playback itself; 0x01 is the sink's own flag, never
//      sent). The rates as Qualcomm's Android stack defines them
//      (a2dp_vendor_aptx_adaptive_constants.h): 0x40 44.1k (0x08 in older
//      headers), 0x10 48k, 0xA0 96k (0x80 | 0x20; 0x80 alone is "reserved").
//      Android takes a sink with any bit of 0xA0 for 96 kHz and configures
//      0xA0: an EarFun Air Pro 4+ lists 0xD1 (0x80, no 0x20), a Sennheiser
//      MOMENTUM 5 0x71 (0x20, no 0x80), a Bose QC Ultra 0x51 (neither, and
//      it refuses 96 kHz). Issue #86.
//   7: channel modes (0x01 mono, 0x02 stereo, 0x04 TWS stereo, 0x08 joint
//      stereo, 0x10 TWS mono, 0x20 TWS+)
//   8..13: target latencies (LL0, LL1 in ms; HQ, TWS in 2 ms units)
//   14: reserved; R1: 15 0x00, 16 0xAA (end marker); R2: 15 the capability
//      extension version 1, 16..19 the features word (LE), 20..23 setup
//      preferences, 24..25 the end marker 00 AA
// BTstack reports at most 30 bytes of a remote's codec information, enough
// for all of it (bytes 26..39 are reserved).
const APTXAD_INFO_LEN: usize = 40;
/// The R1 layout's end (its end marker), the least a usable SEP reports
const APTXAD_R1_LEN: usize = 17;
const APTXAD_RATE_48000: u8 = 0x10;
/// 96 kHz as configured (both bits, as a Snapdragon phone sends it)
const APTXAD_RATE_96000: u8 = 0xA0;
const APTXAD_SOURCE_TYPE_HOST: u8 = 0x02;
const APTXAD_CH_STEREO: u8 = 0x02;
const APTXAD_CH_JOINT_STEREO: u8 = 0x08;
const APTXAD_DATA: usize = 8;
const APTXAD_EXT_VERSION: usize = 15;
const APTXAD_R2_DATA_END: usize = 26;
/// The target latencies a host source sends where the sink's are 0 (a 0 ms
/// HQ target makes the sink stutter after a minute or two, §7G): LL0 80 ms,
/// LL1 100 ms, HQ 200 ms, TWS 200 ms; LL0 at least 80 ms for host jitter
const APTXAD_TTP_DEFAULTS: [u8; 4] = [0x50, 0x64, 0x64, 0x64];
const APTXAD_LL0_MIN: u8 = 0x50;
/// The R1 block a source advertises: target latencies, reserved, then R1's
/// end marker
const APTXAD_R1_DATA: [u8; 9] = [0x50, 0x64, 0x64, 0x64, 0xFF, 0xFF, 0x00, 0x00, 0xAA];
/// What a source sends to an R2 sink from byte 8 on: its own target
/// latencies, extension version 1, features 0x0F000092 (R2.0's word with the
/// low byte a Snapdragon phone sends: a zero low byte makes a Bose QC Ultra 2
/// restart), setup preferences 02 03 03 03, the end marker
const APTXAD_R2_DATA: [u8; 18] =
    [0x50, 0x64, 0x64, 0x64, 0xFF, 0xFF, 0x00, 0x01, 0x92, 0x00, 0x00, 0x0F, 0x02, 0x03, 0x03, 0x03, 0x00, 0xAA];

/// aptX Adaptive codec information: the vendor header, `rate`,
/// `channel_mode`, then `data` from byte 8 on
fn aptxad_element(rate: u8, channel_mode: u8, data: &[u8]) -> Vec<u8> {
    let mut info = vendor_header(APTXAD_VENDOR_ID, APTXAD_CODEC_ID);
    info.extend_from_slice(&[rate, channel_mode]);
    info.extend_from_slice(data);
    info.resize(APTXAD_INFO_LEN, 0);
    info
}

/// One Opus 05 direction (9 bytes)
fn opus05_direction(channels: u8, coupled: u8, location: u32, frames: u8, bitrate: u16) -> [u8; 9] {
    let l = location.to_le_bytes();
    let b = bitrate.to_le_bytes();
    [channels, coupled, l[0], l[1], l[2], l[3], frames, b[0], b[1]]
}

// SBC codec information (A2DP 4.3.2), as BTstack's SBC capability event
// splits it: sampling frequency 0x08 16k 0x04 32k 0x02 44.1k 0x01 48k;
// channel mode 0x08 mono 0x04 dual 0x02 stereo 0x01 joint stereo; block
// length 0x08 4 .. 0x01 16; subbands 0x02 4 0x01 8; allocation 0x02 SNR
// 0x01 loudness
const SBC_FREQ_44100: u8 = 0x02;
const SBC_FREQ_48000: u8 = 0x01;
const SBC_CH_DUAL: u8 = 0x04;
const SBC_BLOCKS_16: u8 = 0x01;
const SBC_SUBBANDS_8: u8 = 0x01;
const SBC_ALLOC_LOUDNESS: u8 = 0x01;
/// SBC XQ needs the sink to take at least this bitpool per channel: the
/// lowest XQ bitpool (Mobile at 48 kHz, PipeWire's dual channel default),
/// ~353 kbps at 44.1 kHz, still above the best plain SBC (bitpool 53 joint
/// stereo, ~328 kbps). Below it the sink gets plain SBC. PipeWire's SBC-XQ
/// has no floor at all: it caps its bitpool (43 / 39) at the sink's maximum,
/// so a sink taking up to 39 (Moto buds+, issue #86) streams XQ at 39.
pub const SBC_XQ_MIN_MAX_BITPOOL: u8 = 29;
/// The local SBC SEP's largest bitpool
const SBC_MAX_BITPOOL: u8 = 53;

/// A remote SEP's SBC capabilities (the bitmaps of BTstack's
/// A2DP_SUBEVENT_SIGNALING_MEDIA_CODEC_SBC_CAPABILITY)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SbcCapabilities {
    pub sampling_frequency: u8,
    pub channel_mode: u8,
    pub block_length: u8,
    pub subbands: u8,
    pub allocation_method: u8,
    pub min_bitpool: u8,
    pub max_bitpool: u8,
}

impl SbcCapabilities {
    /// Dual channel with 16 blocks, 8 subbands, loudness, 44.1 or 48 kHz and
    /// a bitpool up to at least [`SBC_XQ_MIN_MAX_BITPOOL`]
    pub fn allows_xq(&self) -> bool {
        self.xq_refusal().is_none()
    }

    /// Why these capabilities do not allow SBC XQ (English, for the log and
    /// diagnostics); None when they do
    pub fn xq_refusal(&self) -> Option<&'static str> {
        if self.channel_mode & SBC_CH_DUAL == 0 {
            Some("no dual channel")
        } else if self.block_length & SBC_BLOCKS_16 == 0 {
            Some("no 16 blocks")
        } else if self.subbands & SBC_SUBBANDS_8 == 0 {
            Some("no 8 subbands")
        } else if self.allocation_method & SBC_ALLOC_LOUDNESS == 0 {
            Some("no loudness allocation")
        } else if self.sampling_frequency & (SBC_FREQ_44100 | SBC_FREQ_48000) == 0 {
            Some("neither 44.1 nor 48 kHz")
        } else if self.max_bitpool < SBC_XQ_MIN_MAX_BITPOOL {
            Some("largest bitpool below 29")
        } else if self.min_bitpool > SBC_XQ_MIN_MAX_BITPOOL {
            Some("smallest bitpool above 29")
        } else {
            None
        }
    }

    /// Sampling frequencies offered, in Hz
    pub fn sample_rates(&self) -> Vec<u32> {
        offered(self.sampling_frequency.into(), &[(0x08, 16000), (0x04, 32000), (0x02, 44100), (0x01, 48000)])
    }

    pub fn channel_modes(&self) -> Vec<ChannelMode> {
        use ChannelMode::*;
        offered(self.channel_mode.into(), &[(0x08, Mono), (0x04, DualChannel), (0x02, Stereo), (0x01, JointStereo)])
    }

    pub fn block_lengths(&self) -> Vec<u8> {
        offered(self.block_length.into(), &[(0x08, 4), (0x04, 8), (0x02, 12), (0x01, 16)])
    }

    pub fn subband_counts(&self) -> Vec<u8> {
        offered(self.subbands.into(), &[(0x02, 4), (0x01, 8)])
    }

    pub fn allocations(&self) -> Vec<Allocation> {
        offered(self.allocation_method.into(), &[(0x02, Allocation::Snr), (0x01, Allocation::Loudness)])
    }
}

/// The values whose bit is set in `bits`, in the order of `table`
fn offered<T: Copy>(bits: u16, table: &[(u16, T)]) -> Vec<T> {
    table.iter().filter(|(bit, _)| bits & bit != 0).map(|&(_, v)| v).collect()
}

// AAC codec information (A2DP 4.5.2), as BTstack's AAC capability event
// splits it: object types (byte 0 >> 1) 0x40 MPEG-2 AAC-LC 0x20 MPEG-4
// AAC-LC; sampling frequencies (12 bits) 0x010 44.1k 0x008 48k; channels
// 0x08 one 0x04 two; the largest bit rate (0 = not given)
pub const AAC_OBJ_MPEG2_LC: u8 = 0x40;
pub const AAC_OBJ_MPEG4_LC: u8 = 0x20;
pub const AAC_FREQ_44100: u16 = 0x010;
pub const AAC_FREQ_48000: u16 = 0x008;
pub const AAC_CH_1: u8 = 0x08;
pub const AAC_CH_2: u8 = 0x04;
/// The largest AAC bit rate configured (the High quality's)
const AAC_MAX_BITRATE: u32 = 256_000;

/// A remote SEP's AAC capabilities (the fields of BTstack's
/// A2DP_SUBEVENT_SIGNALING_MEDIA_CODEC_MPEG_AAC_CAPABILITY)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AacCapabilities {
    pub object_types: u8,
    pub sampling_frequency: u16,
    pub channels: u8,
    pub bit_rate: u32,
    pub vbr: bool,
}

impl AacCapabilities {
    /// AAC-LC (MPEG-2 or MPEG-4) at 44.1 or 48 kHz, mono or stereo: what
    /// the encoder makes
    pub fn usable(&self) -> bool {
        self.object_types & (AAC_OBJ_MPEG2_LC | AAC_OBJ_MPEG4_LC) != 0
            && self.sampling_frequency & (AAC_FREQ_44100 | AAC_FREQ_48000) != 0
            && self.channels & (AAC_CH_1 | AAC_CH_2) != 0
    }

    /// Object types offered, by name
    pub fn object_type_names(&self) -> Vec<&'static str> {
        offered(
            self.object_types.into(),
            &[
                (0x40, "MPEG-2 AAC-LC"),
                (0x20, "MPEG-4 AAC-LC"),
                (0x10, "MPEG-4 AAC-LTP"),
                (0x08, "MPEG-4 AAC scalable"),
                (0x04, "MPEG-4 HE-AAC"),
                (0x02, "MPEG-4 HE-AAC v2"),
                (0x01, "MPEG-4 AAC-ELD v2"),
            ],
        )
    }

    /// Sampling frequencies offered, in Hz
    pub fn sample_rates(&self) -> Vec<u32> {
        offered(
            self.sampling_frequency,
            &[
                (0x800, 8000),
                (0x400, 11025),
                (0x200, 12000),
                (0x100, 16000),
                (0x080, 22050),
                (0x040, 24000),
                (0x020, 32000),
                (0x010, 44100),
                (0x008, 48000),
                (0x004, 64000),
                (0x002, 88200),
                (0x001, 96000),
            ],
        )
    }

    pub fn channel_counts(&self) -> Vec<u8> {
        offered(self.channels.into(), &[(AAC_CH_1.into(), 1), (AAC_CH_2.into(), 2)])
    }
}

impl AacObjectType {
    pub fn name(self) -> &'static str {
        match self {
            AacObjectType::Mpeg2Lc => "MPEG-2 AAC-LC",
            AacObjectType::Mpeg4Lc => "MPEG-4 AAC-LC",
        }
    }
}

/// AAC object type of a configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AacObjectType {
    Mpeg2Lc,
    Mpeg4Lc,
}

/// The local SEP a codec streams from: SBC XQ shares SBC's (it offers all
/// channel modes and a bitpool up to 53)
pub fn endpoint_kind(kind: CodecKind) -> CodecKind {
    match kind {
        CodecKind::SbcXq => CodecKind::Sbc,
        k => k,
    }
}

// FastStream (PipeWire a2dp-codec-caps.h, a2dp_faststream_t): 8 bytes, byte
// 6 direction (0x01 sink = music to the headset, 0x02 source = voice back),
// byte 7 low nibble the sink rates (0x02 44.1k, 0x01 48k), high nibble the
// source rate (0x2 16k). BlueCaster offers and configures the sink direction
// only.
const FASTSTREAM_INFO_LEN: usize = 8;
const FASTSTREAM_DIRECTION_SINK: u8 = 0x01;
const FASTSTREAM_SINK_44100: u8 = 0x02;
const FASTSTREAM_SINK_48000: u8 = 0x01;
const FASTSTREAM_SINK_MASK: u8 = 0x03;

fn vendor_header(vendor_id: u32, codec_id: u16) -> Vec<u8> {
    let mut v = vendor_id.to_le_bytes().to_vec();
    v.extend_from_slice(&codec_id.to_le_bytes());
    v
}

fn read_vendor(info: &[u8]) -> Option<(u32, u16)> {
    if info.len() < 6 {
        return None;
    }
    Some((u32::from_le_bytes([info[0], info[1], info[2], info[3]]), u16::from_le_bytes([info[4], info[5]])))
}

/// A local stream endpoint: what we register with BTstack
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalEndpoint {
    pub kind: CodecKind,
    /// AVDTP media codec type ([`CODEC_TYPE_SBC`], ...)
    pub codec_type: u8,
    /// Capabilities we offer
    pub caps: Vec<u8>,
    /// Default configuration (BTstack's initial value; the real one goes
    /// with SET_CONFIGURATION)
    pub config: Vec<u8>,
}

/// The endpoints of the built-in codecs, in v1's order (endpoint slots)
pub fn local_endpoints() -> Vec<LocalEndpoint> {
    let mut eps = Vec::new();
    let vendor = |kind: CodecKind, vid: u32, cid: u16, len: usize, caps_tail: &[u8], config_tail: &[u8]| {
        let mut caps = vendor_header(vid, cid);
        caps.extend_from_slice(caps_tail);
        caps.resize(len, 0);
        let mut config = vendor_header(vid, cid);
        config.extend_from_slice(config_tail);
        config.resize(len, 0);
        LocalEndpoint { kind, codec_type: CODEC_TYPE_VENDOR, caps, config }
    };
    if CodecKind::Ldac.available() {
        eps.push(vendor(
            CodecKind::Ldac,
            LDAC_VENDOR_ID,
            LDAC_CODEC_ID,
            8,
            &[LDAC_FREQ_ALL, LDAC_CH_ALL],
            &[0x10, 0x01],
        ));
    }
    if CodecKind::AptxHd.available() {
        eps.push(vendor(
            CodecKind::AptxHd,
            APTXHD_VENDOR_ID,
            APTXHD_CODEC_ID,
            APTXHD_INFO_LEN,
            &[APTX_CAPS_ALL],
            &[APTX_CONFIG_DEFAULT],
        ));
        eps.push(vendor(CodecKind::Aptx, APTX_VENDOR_ID, APTX_CODEC_ID, 7, &[APTX_CAPS_ALL], &[APTX_CONFIG_DEFAULT]));
        // One local SEP for sinks listing aptX LL under either vendor ID; the
        // configuration carries the remote's
        eps.push(vendor(
            CodecKind::AptxLl,
            APTXLL_VENDOR_ID,
            APTXLL_CODEC_ID,
            APTXLL_INFO_LEN,
            &[APTX_CAPS_ALL],
            &[APTX_CONFIG_DEFAULT],
        ));
    }
    // SBC: 44.1+48k, all channel modes, block lengths, subbands and
    // allocations (a profile may ask for any), bitpool 2..53; configured 16
    // blocks, 8 subbands, loudness (SBC XQ streams from this SEP too, in
    // dual channel)
    eps.push(LocalEndpoint {
        kind: CodecKind::Sbc,
        codec_type: CODEC_TYPE_SBC,
        caps: vec![0x3F, 0xFF, 2, 53],
        config: vec![0x11, 0x15, 2, 53],
    });
    if CodecKind::Aac.available() {
        // MPEG-2 and MPEG-4 AAC-LC, 44.1+48k (byte 1 0x01, byte 2 0x80),
        // 1 or 2 channels, 256 kbps; configured 44.1k stereo
        eps.push(LocalEndpoint {
            kind: CodecKind::Aac,
            codec_type: CODEC_TYPE_AAC,
            caps: vec![0xC0, 0x01, 0x8C, 0x03, 0xE8, 0x00],
            config: vec![0x80, 0x01, 0x04, 0x03, 0xE8, 0x00],
        });
    }
    if CodecKind::LhdcV5.available() {
        eps.push(vendor(
            CodecKind::LhdcV5,
            LHDCV5_VENDOR_ID,
            LHDCV5_CODEC_ID,
            LHDCV5_INFO_LEN,
            &[LHDCV5_SR_MASK, LHDCV5_BITS_16 | LHDCV5_BITS_24, LHDCV5_CAPS_P8],
            &[LHDCV5_SR_48000, LHDCV5_BITS_24, LHDCV5_CAPS_P8],
        ));
    }
    // FastStream: music to the sink at 44.1 / 48 kHz, no voice backchannel
    eps.push(vendor(
        CodecKind::FastStream,
        FASTSTREAM_VENDOR_ID,
        FASTSTREAM_CODEC_ID,
        FASTSTREAM_INFO_LEN,
        &[FASTSTREAM_DIRECTION_SINK, FASTSTREAM_SINK_44100 | FASTSTREAM_SINK_48000],
        &[FASTSTREAM_DIRECTION_SINK, FASTSTREAM_SINK_48000],
    ));
    if CodecKind::Opus.available() {
        eps.push(vendor(
            CodecKind::Opus,
            OPUS_VENDOR_ID,
            OPUS_CODEC_ID,
            OPUS_INFO_LEN,
            &[OPUS_CAPS],
            &[OPUS_CONFIG_DEFAULT],
        ));
    }
    if CodecKind::Opus05.available() {
        // Up to stereo (front left + right), 10 / 20 ms, no bit rate limit,
        // no return channel; the default configuration is PipeWire's stereo
        let frames = OPUS05_FRAME_10MS | OPUS05_FRAME_20MS;
        eps.push(vendor(
            CodecKind::Opus05,
            OPUS05_VENDOR_ID,
            OPUS05_CODEC_ID,
            OPUS05_INFO_LEN,
            &opus05_direction(2, 0, OPUS05_LOCATION_STEREO, frames, 0),
            &opus05_direction(2, 1, OPUS05_LOCATION_STEREO, OPUS05_FRAME_10MS, OPUS05_MAX_BITRATE_UNITS),
        ));
    }
    if CodecKind::AptxAdaptive.available() {
        // 48 kHz from a host encoder, stereo / joint stereo, the R1 block
        // (as upstream's plugin; whether a source should advertise the R2
        // extension is open there)
        let rate = APTXAD_RATE_48000 | APTXAD_SOURCE_TYPE_HOST;
        eps.push(LocalEndpoint {
            kind: CodecKind::AptxAdaptive,
            codec_type: CODEC_TYPE_VENDOR,
            caps: aptxad_element(rate, APTXAD_CH_STEREO | APTXAD_CH_JOINT_STEREO, &APTXAD_R1_DATA),
            config: aptxad_element(rate, APTXAD_CH_STEREO, &APTXAD_R1_DATA),
        });
    }
    eps
}

/// A codec the remote offers on one of its SEPs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RemoteSep {
    pub seid: u8,
}

/// What the remote reported during capability discovery
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RemoteCaps {
    pub sbc: Option<RemoteSep>,
    /// The SBC SEP's capabilities, when reported (SBC XQ needs them)
    pub sbc_caps: Option<SbcCapabilities>,
    pub aac: Option<RemoteSep>,
    /// The AAC SEP's capabilities, when reported
    pub aac_caps: Option<AacCapabilities>,
    pub ldac: Option<RemoteSep>,
    /// LDAC byte 6: the rates the sink lists (0 = not reported: assume all)
    pub ldac_rates: u8,
    pub aptx: Option<RemoteSep>,
    pub aptx_hd: Option<RemoteSep>,
    pub aptx_ll: Option<RemoteSep>,
    pub lhdcv5: Option<RemoteSep>,
    pub opus: Option<RemoteSep>,
    pub opus05: Option<RemoteSep>,
    pub faststream: Option<RemoteSep>,
    /// FastStream sink (music) rates: 0x02 44.1k, 0x01 48k
    pub faststream_rates: u8,
    /// aptX family byte 6 of each (0 = not reported: assume 44.1k + 48k)
    pub aptx_caps: u8,
    pub aptxhd_caps: u8,
    pub aptxll_caps: u8,
    /// aptX LL: the vendor ID the remote used and its codec information
    pub aptxll_vendor_id: u32,
    pub aptxll_info: Vec<u8>,
    /// aptX Adaptive offered (usable or not, for messages)
    pub aptx_adaptive: bool,
    /// aptX Adaptive at 48 kHz stereo, and its codec information (as
    /// reported: up to 30 bytes)
    pub aptxad: Option<RemoteSep>,
    pub aptxad_info: Vec<u8>,
    /// LHDC V5 codec information (11 bytes) when usable
    pub lhdcv5_info: [u8; LHDCV5_INFO_LEN],
    /// Opus codec information byte 6 (channel modes, frame durations, rate)
    pub opus_caps: u8,
    /// Opus 05: the main direction's channels, frame durations and bit rate
    /// limit (units of 1024 bps, 0 = none), and whether the SEP taken has a
    /// return channel (a duplex SEP, used only when there is no other)
    pub opus05_channels: u8,
    pub opus05_frames: u8,
    pub opus05_bitrate: u16,
    pub opus05_bidi: bool,
    /// SEP discovery: audio sink SEPs, and how many of them were in use (a
    /// codec streaming to another device is missing from the above)
    pub audio_sinks: u8,
    pub audio_sinks_in_use: u8,
}

/// How a vendor codec capability was taken, for the log
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VendorCapability {
    Taken(CodecKind),
    /// Known codec, but not usable (e.g. mono-only aptX, an LHDC version we
    /// do not encode)
    Ignored(CodecKind, &'static str),
    /// aptX Adaptive, which this build does not encode
    AptxAdaptive,
    Unknown {
        vendor_id: u32,
        codec_id: u16,
    },
    TooShort,
}

impl RemoteCaps {
    pub fn has(&self, kind: CodecKind) -> bool {
        self.sep(kind).is_some()
    }

    pub fn sep(&self, kind: CodecKind) -> Option<RemoteSep> {
        match kind {
            CodecKind::Sbc => self.sbc,
            CodecKind::SbcXq => self.sbc.filter(|_| self.sbc_caps.is_some_and(|c| c.allows_xq())),
            CodecKind::Aac => self.aac.filter(|_| self.aac_caps.is_none_or(|c| c.usable())),
            CodecKind::Ldac => self.ldac,
            CodecKind::Aptx => self.aptx,
            CodecKind::AptxHd => self.aptx_hd,
            CodecKind::AptxLl => self.aptx_ll,
            CodecKind::LhdcV5 => self.lhdcv5,
            CodecKind::Opus => self.opus,
            CodecKind::Opus05 => self.opus05,
            CodecKind::FastStream => self.faststream,
            CodecKind::AptxAdaptive => self.aptxad,
        }
    }

    /// An SBC SEP whose capabilities are not known (no SBC XQ)
    pub fn add_sbc(&mut self, seid: u8) {
        self.sbc = Some(RemoteSep { seid });
        self.sbc_caps = None;
    }

    /// An SBC SEP with its capabilities. With several, the first that
    /// allows SBC XQ wins, else the last.
    pub fn add_sbc_caps(&mut self, seid: u8, caps: SbcCapabilities) {
        if self.sbc_caps.is_some_and(|c| c.allows_xq()) && !caps.allows_xq() {
            tracing::debug!("caps: SBC SEID {seid} skipped, keeping SEID {:?} that allows SBC XQ", self.sbc);
            return;
        }
        tracing::debug!("caps: SBC from SEID {seid}, allows SBC XQ {}", caps.allows_xq());
        self.sbc = Some(RemoteSep { seid });
        self.sbc_caps = Some(caps);
    }

    /// The sink's largest SBC bitpool (0 = not known)
    pub fn sbc_max_bitpool(&self) -> u8 {
        self.sbc_caps.map_or(0, |c| c.max_bitpool)
    }

    /// The sink's smallest SBC bitpool (0 = not known)
    pub fn sbc_min_bitpool(&self) -> u8 {
        self.sbc_caps.map_or(0, |c| c.min_bitpool)
    }

    /// The bitpool range of an SBC / SBC XQ SET_CONFIGURATION: up to `cap`
    /// within the sink's range (as PipeWire's select_config); 2..cap when
    /// the sink's capabilities are not known
    pub fn sbc_bitpool_range(&self, cap: u8) -> (u8, u8) {
        let (min, max) = self.sbc_caps.map_or((2, cap), |c| (c.min_bitpool, c.max_bitpool));
        let max = cap.min(max).max(2);
        (min.max(2).min(max), max)
    }

    /// The SBC setup of a `kind` stream (SBC or SBC XQ) from what the
    /// profile asks for: a value the sink does not list falls back to the
    /// default (the SET_CONFIGURATION would be refused), said in the notes
    /// returned (English, for the log). Unknown capabilities take every
    /// value. SBC XQ has no channel mode of its own choice (dual channel);
    /// plain SBC no dual channel (that is SBC XQ, issue #86).
    pub fn sbc_params(&self, kind: CodecKind, wanted: sbc::SbcParams) -> (sbc::SbcParams, Vec<String>) {
        let mut p = wanted;
        let mut notes = Vec::new();
        if kind == CodecKind::SbcXq {
            p.channel_mode = None;
        } else if p.channel_mode == Some(sbc::ChannelMode::DualChannel) {
            notes.push("SBC: dual channel is SBC XQ's, using JointStereo".to_string());
            p.channel_mode = None;
        }
        if !matches!(p.blocks, 0 | 4 | 8 | 12 | 16) {
            p.blocks = 0;
        }
        if !matches!(p.subbands, 0 | 4 | 8) {
            p.subbands = 0;
        }
        let Some(c) = self.sbc_caps else { return (p, notes) };
        let name = kind.name();
        if p.blocks != 0 && !c.block_lengths().contains(&p.blocks) {
            notes.push(format!(
                "{name}: the device does not list {} blocks, using {}",
                p.blocks,
                sbc::SbcParams::default().blocks()
            ));
            p.blocks = 0;
        }
        if p.subbands != 0 && !c.subband_counts().contains(&p.subbands) {
            notes.push(format!(
                "{name}: the device does not list {} subbands, using {}",
                p.subbands,
                sbc::SbcParams::default().subbands()
            ));
            p.subbands = 0;
        }
        if let Some(a) = p.allocation
            && !c.allocations().contains(&a)
        {
            notes.push(format!("{name}: the device does not list {a:?} allocation, using Loudness"));
            p.allocation = None;
        }
        if let Some(m) = p.channel_mode
            && !c.channel_modes().contains(&m)
        {
            notes.push(format!("{name}: the device does not list {m:?}, using JointStereo"));
            p.channel_mode = None;
        }
        (p, notes)
    }

    /// An AAC SEP whose capabilities are not known
    pub fn add_aac(&mut self, seid: u8) {
        self.aac = Some(RemoteSep { seid });
        self.aac_caps = None;
    }

    /// An AAC SEP with its capabilities. With several, the first usable one
    /// wins, else the last.
    pub fn add_aac_caps(&mut self, seid: u8, caps: AacCapabilities) {
        if self.aac_caps.is_some_and(|c| c.usable()) {
            tracing::debug!("caps: AAC SEID {seid} skipped, keeping usable {:?}", self.aac);
            return;
        }
        tracing::debug!("caps: AAC from SEID {seid}: {caps:?}, usable {}", caps.usable());
        self.aac = Some(RemoteSep { seid });
        self.aac_caps = Some(caps);
    }

    /// The largest AAC bit rate of the configuration: the sink's when it
    /// gives one below [`AAC_MAX_BITRATE`] (as PipeWire's select_config)
    pub fn aac_max_bitrate(&self) -> u32 {
        match self.aac_caps.map_or(0, |c| c.bit_rate) {
            0 => AAC_MAX_BITRATE,
            b => b.min(AAC_MAX_BITRATE),
        }
    }

    /// The sink lists variable bit rate AAC
    pub fn aac_vbr(&self) -> bool {
        self.aac_caps.is_some_and(|c| c.vbr)
    }

    /// Capture rate for AAC: 44.1 or 48 kHz as the sink lists them,
    /// preferring 44.1k for `wanted` 44.1k, else 48k
    pub fn pick_aac_sample_rate(&self, wanted: u32) -> u32 {
        let freqs = self.aac_caps.map_or(AAC_FREQ_44100 | AAC_FREQ_48000, |c| c.sampling_frequency);
        let has441 = freqs & AAC_FREQ_44100 != 0;
        let has48 = freqs & AAC_FREQ_48000 != 0;
        let rate = if (wanted == 44100 && has441) || !has48 { 44100 } else { 48000 };
        tracing::debug!(
            "caps: AAC rate {wanted} -> {rate} (frequencies 0x{freqs:x}, caps known {})",
            self.aac_caps.is_some()
        );
        rate
    }

    /// A vendor codec capability (media codec information) of SEP `seid`
    pub fn add_vendor(&mut self, seid: u8, info: &[u8]) -> VendorCapability {
        let Some((vid, cid)) = read_vendor(info) else {
            return VendorCapability::TooShort;
        };
        let byte = |i: usize| info.get(i).copied().unwrap_or(0);
        if vid == LDAC_VENDOR_ID && cid == LDAC_CODEC_ID {
            self.ldac = Some(RemoteSep { seid });
            self.ldac_rates = byte(6) & LDAC_FREQ_ALL;
            return VendorCapability::Taken(CodecKind::Ldac);
        }
        if vid == APTXHD_VENDOR_ID && cid == APTXHD_CODEC_ID {
            // Only stereo is sent: skip mono-only sinks
            let caps = byte(6);
            if info.len() >= 7 && caps & APTX_CH_STEREO == 0 {
                return VendorCapability::Ignored(CodecKind::AptxHd, "no stereo");
            }
            self.aptx_hd = Some(RemoteSep { seid });
            self.aptxhd_caps = caps;
            return VendorCapability::Taken(CodecKind::AptxHd);
        }
        if (vid == APTXLL_VENDOR_ID || vid == APTXLL_VENDOR_ID2) && cid == APTXLL_CODEC_ID {
            let caps = byte(6);
            if info.len() >= 7 && caps & APTX_CH_STEREO == 0 {
                return VendorCapability::Ignored(CodecKind::AptxLl, "no stereo");
            }
            // Keep the first usable LL SEP if listed under both IDs
            if self.aptx_ll.is_none() {
                self.aptx_ll = Some(RemoteSep { seid });
                self.aptxll_caps = caps;
                self.aptxll_vendor_id = vid;
                self.aptxll_info = info[..info.len().min(APTXLL_EXT_INFO_LEN)].to_vec();
            }
            return VendorCapability::Taken(CodecKind::AptxLl);
        }
        if vid == APTXAD_VENDOR_ID && cid == APTXAD_CODEC_ID {
            self.aptx_adaptive = true;
            if !CodecKind::AptxAdaptive.available() {
                return VendorCapability::AptxAdaptive;
            }
            // 48 kHz (every sink's rate) in stereo or joint stereo; at least
            // the R1 layout
            let usable = info.len() >= APTXAD_R1_LEN
                && byte(6) & APTXAD_RATE_48000 != 0
                && byte(7) & (APTXAD_CH_STEREO | APTXAD_CH_JOINT_STEREO) != 0;
            if !usable {
                return VendorCapability::Ignored(CodecKind::AptxAdaptive, "no 48 kHz stereo");
            }
            if self.aptxad.is_none() {
                self.aptxad = Some(RemoteSep { seid });
                self.aptxad_info = info[..info.len().min(APTXAD_INFO_LEN)].to_vec();
            }
            return VendorCapability::Taken(CodecKind::AptxAdaptive);
        }
        if vid == LHDCV5_VENDOR_ID && cid == LHDCV5_CODEC_ID {
            let usable = info.len() >= LHDCV5_INFO_LEN
                && info[6] & LHDCV5_SR_MASK != 0
                && info[7] & (LHDCV5_BITS_16 | LHDCV5_BITS_24) != 0
                && info[8] & LHDCV5_VERSION_MASK & LHDCV5_VERSION_1 != 0
                && info[8] & LHDCV5_FRAME_5MS != 0;
            if !usable || !CodecKind::LhdcV5.available() {
                return VendorCapability::Ignored(CodecKind::LhdcV5, "version / frame length / rates");
            }
            if self.lhdcv5.is_none() {
                self.lhdcv5 = Some(RemoteSep { seid });
                self.lhdcv5_info.copy_from_slice(&info[..LHDCV5_INFO_LEN]);
            }
            return VendorCapability::Taken(CodecKind::LhdcV5);
        }
        if vid == OPUS_VENDOR_ID && cid == OPUS_CODEC_ID {
            // 48 kHz, a frame duration, and mono or stereo (dual mono, two
            // independent channels, is not sent)
            let caps = byte(6);
            let usable = info.len() >= OPUS_INFO_LEN
                && caps & OPUS_FREQ_48000 != 0
                && caps & OPUS_FRAME_MASK != 0
                && caps & (OPUS_CH_MONO | OPUS_CH_STEREO) != 0;
            if !usable {
                return VendorCapability::Ignored(CodecKind::Opus, "no 48 kHz / frame duration / mono or stereo");
            }
            if self.opus.is_none() {
                self.opus = Some(RemoteSep { seid });
                self.opus_caps = caps;
            }
            return VendorCapability::Taken(CodecKind::Opus);
        }
        if vid == OPUS05_VENDOR_ID && cid == OPUS05_CODEC_ID {
            // The music direction with a channel and 10 or 20 ms frames
            let (channels, frames) = (byte(OPUS05_MAIN), byte(OPUS05_MAIN + 6));
            if info.len() < OPUS05_INFO_LEN || channels == 0 || frames & (OPUS05_FRAME_10MS | OPUS05_FRAME_20MS) == 0 {
                return VendorCapability::Ignored(CodecKind::Opus05, "no music direction with 10 / 20 ms frames");
            }
            // PipeWire offers stereo on its opus_05 SEP and a return channel
            // on its opus_05_duplex SEP: prefer the one without
            let bidi = byte(OPUS05_BIDI) != 0;
            tracing::debug!(
                "caps: Opus 05 SEID {seid}: {channels} ch, frames 0x{frames:02x}, return channel {bidi}, taken {}",
                self.opus05.is_none() || (self.opus05_bidi && !bidi)
            );
            if self.opus05.is_none() || (self.opus05_bidi && !bidi) {
                self.opus05 = Some(RemoteSep { seid });
                self.opus05_channels = channels;
                self.opus05_frames = frames;
                self.opus05_bitrate = u16::from_le_bytes([byte(OPUS05_MAIN + 7), byte(OPUS05_MAIN + 8)]);
                self.opus05_bidi = bidi;
            }
            return VendorCapability::Taken(CodecKind::Opus05);
        }
        if vid == FASTSTREAM_VENDOR_ID && cid == FASTSTREAM_CODEC_ID {
            let rates = byte(7) & FASTSTREAM_SINK_MASK;
            if info.len() < FASTSTREAM_INFO_LEN || byte(6) & FASTSTREAM_DIRECTION_SINK == 0 || rates == 0 {
                return VendorCapability::Ignored(CodecKind::FastStream, "no music direction at 44.1 / 48 kHz");
            }
            if self.faststream.is_none() {
                self.faststream = Some(RemoteSep { seid });
                self.faststream_rates = rates;
            }
            return VendorCapability::Taken(CodecKind::FastStream);
        }
        if vid == APTX_VENDOR_ID && cid == APTX_CODEC_ID {
            let caps = byte(6);
            if info.len() >= 7 && caps & APTX_CH_STEREO == 0 {
                return VendorCapability::Ignored(CodecKind::Aptx, "no stereo");
            }
            self.aptx = Some(RemoteSep { seid });
            self.aptx_caps = caps;
            return VendorCapability::Taken(CodecKind::Aptx);
        }
        VendorCapability::Unknown { vendor_id: vid, codec_id: cid }
    }

    pub fn lhdcv5_rates(&self) -> u8 {
        self.lhdcv5_info[6] & LHDCV5_SR_MASK
    }

    pub fn lhdcv5_24bit(&self) -> bool {
        self.lhdcv5_info[7] & LHDCV5_BITS_24 != 0
    }

    pub fn lhdcv5_bitrate_limits(&self) -> u8 {
        self.lhdcv5_info[7] & LHDCV5_BITRATE_LIMITS_MASK
    }

    /// The sample rates the sink lists for `kind`, lowest first, as the
    /// codec settings offer them (issue #86); empty when it said none.
    /// aptX Adaptive lists 96 kHz only when the sink can take it
    /// ([`Self::aptxad_96k`]); Opus and Opus 05 are 48 kHz.
    pub fn listed_sample_rates(&self, kind: CodecKind) -> Vec<u32> {
        let from_bits = |table: &[(u8, u32)], bits: u8| {
            let mut r: Vec<u32> = table.iter().filter(|&&(b, _)| bits & b != 0).map(|&(_, r)| r).collect();
            r.sort_unstable();
            r
        };
        let aptx = |caps: u8| match caps & APTX_FREQ_MASK {
            0 => vec![44100, 48000],
            f => from_bits(&[(APTX_FREQ_44100, 44100), (APTX_FREQ_48000, 48000)], f),
        };
        match kind {
            _ if !self.has(kind) => Vec::new(),
            CodecKind::Sbc | CodecKind::SbcXq => self.sbc_caps.map(|c| c.sample_rates()).unwrap_or_default(),
            CodecKind::Aac => self.aac_caps.map(|c| c.sample_rates()).unwrap_or_default(),
            CodecKind::Ldac => {
                from_bits(&LDAC_RATES, if self.ldac_rates == 0 { LDAC_FREQ_ALL } else { self.ldac_rates })
            }
            CodecKind::LhdcV5 => from_bits(
                &[
                    (LHDCV5_SR_44100, 44100),
                    (LHDCV5_SR_48000, 48000),
                    (LHDCV5_SR_96000, 96000),
                    (LHDCV5_SR_192000, 192000),
                ],
                self.lhdcv5_rates(),
            ),
            CodecKind::Aptx => aptx(self.aptx_caps),
            CodecKind::AptxHd => aptx(self.aptxhd_caps),
            CodecKind::AptxLl => aptx(self.aptxll_caps),
            CodecKind::FastStream => {
                from_bits(&[(FASTSTREAM_SINK_44100, 44100), (FASTSTREAM_SINK_48000, 48000)], self.faststream_rates)
            }
            CodecKind::AptxAdaptive => {
                if self.aptxad_96k() {
                    vec![48000, 96000]
                } else {
                    vec![48000]
                }
            }
            CodecKind::Opus | CodecKind::Opus05 => vec![48000],
        }
    }

    /// The bit depths the sink lists for `kind` (LHDC V5 only; empty for
    /// the others: no choice, or the sink does not say)
    pub fn listed_bit_depths(&self, kind: CodecKind) -> Vec<u32> {
        if kind != CodecKind::LhdcV5 || !self.has(kind) {
            return Vec::new();
        }
        let bits = self.lhdcv5_info[7] & LHDCV5_BITS_MASK;
        [(LHDCV5_BITS_16, 16), (LHDCV5_BITS_24, 24)]
            .into_iter()
            .filter(|&(b, _)| bits & b != 0)
            .map(|(_, d)| d)
            .collect()
    }

    /// The largest bit rate (bps) the sink lists for `kind`: AAC's bit rate
    /// field, LHDC V5's maximum (400 / 500 / 900 kbps); 0 when it gives none
    pub fn listed_max_bitrate(&self, kind: CodecKind) -> u32 {
        match kind {
            _ if !self.has(kind) => 0,
            CodecKind::Aac => self.aac_caps.map_or(0, |c| c.bit_rate),
            CodecKind::LhdcV5 => match self.lhdcv5_bitrate_limits() & 0x30 {
                0x10 => 400_000,
                0x20 => 500_000,
                0x30 => 900_000,
                _ => 0,
            },
            _ => 0,
        }
    }

    /// Opus: the frame duration (ms) and the channels on the wire for
    /// `channels` of input. 20 ms when offered (the only duration Android
    /// has a bit rate for; PipeWire prefers it too), else 10 ms. Mono only
    /// for mono input to a sink that has it.
    pub fn opus_stream(&self, channels: u32) -> (u8, u8) {
        let frame_ms =
            if self.opus_caps & OPUS_FRAME_10MS != 0 && self.opus_caps & OPUS_FRAME_20MS == 0 { 10 } else { 20 };
        let wire = if channels == 1 && self.opus_caps & OPUS_CH_MONO != 0 { 1 } else { 2 };
        (frame_ms, wire)
    }

    /// Opus 05: the frame duration (ms) and the channels on the wire. 10 ms
    /// when offered (PipeWire's choice), else 20 ms; stereo unless the sink
    /// takes one channel only (mono input is duplicated, as PipeWire's own
    /// opus_05 sends stereo)
    pub fn opus05_stream(&self) -> (u8, u8) {
        let frame_ms = if self.opus05_frames & OPUS05_FRAME_10MS != 0 { 10 } else { 20 };
        (frame_ms, if self.opus05_channels == 1 { 1 } else { 2 })
    }

    /// Opus 05: the bit rate limit of the configuration, units of 1024 bps:
    /// the sink's limit, at most PipeWire's 312 (320 kbps)
    pub fn opus05_bitrate_units(&self) -> u16 {
        match self.opus05_bitrate {
            0 => OPUS05_MAX_BITRATE_UNITS,
            b => b.min(OPUS05_MAX_BITRATE_UNITS),
        }
    }

    /// Opus / Opus 05 encoder parameters for `kind` and `channels` of input:
    /// (frame duration in ms, channels on the wire, bit rate limit in bps
    /// or 0); zeros for other codecs
    pub fn opus_params(&self, kind: CodecKind, channels: u32) -> (u8, u8, u32) {
        match kind {
            CodecKind::Opus => {
                let (frame_ms, wire) = self.opus_stream(channels);
                (frame_ms, wire, 0)
            }
            CodecKind::Opus05 => {
                let (frame_ms, wire) = self.opus05_stream();
                (frame_ms, wire, u32::from(self.opus05_bitrate_units()) * OPUS05_BITRATE_UNIT)
            }
            _ => (0, 0, 0),
        }
    }

    /// aptX Adaptive: the sink is R2 (capability extension version 1), which
    /// decides the bitstream; else R1 (§1.3)
    pub fn aptxad_r2(&self) -> bool {
        self.aptxad_info.get(APTXAD_EXT_VERSION) == Some(&1)
    }

    /// aptX Adaptive at 96 kHz: an R2 sink that lists the rate, by either
    /// of its bits as Android checks it (upstream offers 96 kHz to R2 sinks
    /// only; an R1 sink is always 48 kHz)
    pub fn aptxad_96k(&self) -> bool {
        self.aptxad_r2() && self.aptxad_info.get(6).is_some_and(|&b| b & APTXAD_RATE_96000 != 0)
    }

    /// Capture rate for aptX Adaptive: 96 kHz when the profile asks for it
    /// (`wanted`), the stream is high quality (96 kHz has no low latency
    /// bitstream) and the sink can take it ([`Self::aptxad_96k`]); else 48
    /// kHz, the rate every sink takes
    pub fn pick_aptxad_sample_rate(&self, wanted: u32, quality: crate::Quality) -> u32 {
        let no_96k = self.aptxad_no_96k(quality);
        let rate = if wanted == 96000 && no_96k.is_none() { 96000 } else { 48000 };
        let (r2, rates) = (self.aptxad_r2(), self.aptxad_info.get(6).copied().unwrap_or(0) & 0xF8);
        match no_96k {
            // Asked for and refused: why, with the sink's bytes (issue #86:
            // an EarFun Air Pro 4+ said to take 96 kHz stayed at 48 kHz)
            Some(why) if wanted == 96000 => tracing::info!(
                "caps: aptX Adaptive at 48 kHz, not 96 kHz: {why} (r2 {r2}, rates 0x{rates:02x}, extension {:?})",
                self.aptxad_info.get(APTXAD_EXT_VERSION)
            ),
            _ => tracing::debug!(
                "caps: aptX Adaptive rate {wanted} -> {rate} ({quality:?}, r2 {r2}, rates 0x{rates:02x})"
            ),
        }
        rate
    }

    /// Why aptX Adaptive cannot stream at 96 kHz in `quality`; None when it
    /// can
    pub fn aptxad_no_96k(&self, quality: crate::Quality) -> Option<&'static str> {
        if quality == crate::Quality::LowLatency {
            Some("low latency has no 96 kHz bitstream")
        } else if !self.aptxad_r2() {
            Some("the audio device is aptX Adaptive R1 (upstream streams 96 kHz to R2 only)")
        } else if !self.aptxad_96k() {
            Some("the audio device does not list 96 kHz")
        } else {
            None
        }
    }

    /// The best codec both sides have (Auto)
    pub fn best(&self) -> Option<CodecKind> {
        self.best_of(true)
    }

    /// The best codec both sides have, leaving out the experimental ones
    /// unless `experimental`
    pub fn best_of(&self, experimental: bool) -> Option<CodecKind> {
        self.best_in(&AutoOrder::default(), experimental)
    }

    /// The first codec in `order` both sides have, leaving out the
    /// experimental ones unless `experimental`
    pub fn best_in(&self, order: &AutoOrder, experimental: bool) -> Option<CodecKind> {
        order.codecs().into_iter().find(|&k| self.usable(k, experimental))
    }

    fn usable(&self, kind: CodecKind, experimental: bool) -> bool {
        kind.available() && self.has(kind) && (experimental || !kind.experimental())
    }

    /// Capture rate for the aptX family: 44.1 or 48 kHz as the remote lists
    /// them, preferring `wanted` (other rates: 48k, then 44.1k)
    pub fn pick_aptx_sample_rate(&self, kind: CodecKind, wanted: u32) -> u32 {
        let caps = match kind {
            CodecKind::Aptx => self.aptx_caps,
            CodecKind::AptxHd => self.aptxhd_caps,
            CodecKind::AptxLl => self.aptxll_caps,
            _ => return wanted,
        };
        let freqs = caps & APTX_FREQ_MASK;
        let has441 = freqs == 0 || freqs & APTX_FREQ_44100 != 0;
        let has48 = freqs == 0 || freqs & APTX_FREQ_48000 != 0;
        let rate = if wanted == 48000 && has48 {
            48000
        } else if wanted == 44100 && has441 {
            44100
        } else if has48 {
            48000
        } else if has441 {
            44100
        } else {
            wanted
        };
        tracing::debug!("caps: {kind} rate {wanted} -> {rate} (caps 0x{caps:02x})");
        rate
    }

    /// Capture rate for LDAC: `wanted` when the sink lists it, else the
    /// highest listed rate below it, else the lowest above it (88.2 / 96 kHz
    /// are optional for a sink)
    pub fn pick_ldac_sample_rate(&self, wanted: u32) -> u32 {
        let rates = if self.ldac_rates == 0 { LDAC_FREQ_ALL } else { self.ldac_rates };
        let listed = || LDAC_RATES.into_iter().filter(|&(bit, _)| rates & bit != 0).map(|(_, r)| r);
        let rate = listed().find(|&r| r <= wanted).or_else(|| listed().next_back()).unwrap_or(wanted);
        tracing::debug!("caps: LDAC rate {wanted} -> {rate} (rates 0x{rates:02x})");
        rate
    }

    /// Capture rate for FastStream: 44.1 or 48 kHz as the sink lists them,
    /// preferring `wanted` (other rates: 48k, then 44.1k)
    pub fn pick_faststream_sample_rate(&self, wanted: u32) -> u32 {
        let has441 = self.faststream_rates & FASTSTREAM_SINK_44100 != 0;
        let has48 = self.faststream_rates & FASTSTREAM_SINK_48000 != 0;
        let rate = if (wanted == 44100 && has441) || !has48 { 44100 } else { 48000 };
        tracing::debug!("caps: FastStream rate {wanted} -> {rate} (rates 0x{:02x})", self.faststream_rates);
        rate
    }

    /// Capture rate for LHDC V5: 44.1 / 48 / 96 / 192 kHz as the remote
    /// lists them, preferring `wanted` (88.2k maps to 96k)
    pub fn pick_lhdcv5_sample_rate(&self, wanted: u32) -> u32 {
        let rates = self.lhdcv5_rates();
        tracing::debug!("caps: LHDC V5 rate for {wanted} Hz from rates 0x{rates:02x}");
        let has = |bit: u8| rates & bit != 0;
        let wanted = if wanted == 88200 { 96000 } else { wanted };
        match wanted {
            44100 if has(LHDCV5_SR_44100) => return 44100,
            48000 if has(LHDCV5_SR_48000) => return 48000,
            96000 if has(LHDCV5_SR_96000) => return 96000,
            192000 if has(LHDCV5_SR_192000) => return 192000,
            _ => {}
        }
        for (bit, rate) in
            [(LHDCV5_SR_48000, 48000), (LHDCV5_SR_44100, 44100), (LHDCV5_SR_96000, 96000), (LHDCV5_SR_192000, 192000)]
        {
            if has(bit) {
                return rate;
            }
        }
        wanted
    }

    /// The SET_CONFIGURATION for a stream of `kind`
    pub fn configuration(
        &self,
        kind: CodecKind,
        sample_rate: u32,
        channels: u32,
        bits_per_sample: u8,
    ) -> Result<Configuration, ConfigError> {
        self.configuration_vbr(kind, sample_rate, channels, bits_per_sample, false)
    }

    /// [`Self::configuration`] with AAC's variable bit rate asked for (taken
    /// only where the sink lists it)
    pub fn configuration_vbr(
        &self,
        kind: CodecKind,
        sample_rate: u32,
        channels: u32,
        bits_per_sample: u8,
        vbr: bool,
    ) -> Result<Configuration, ConfigError> {
        self.configuration_with(kind, sample_rate, channels, bits_per_sample, vbr, sbc::SbcParams::default())
    }

    /// [`Self::configuration_vbr`] with the SBC setup a profile asks for
    /// (SBC, SBC XQ): what the sink does not list falls back to the default,
    /// as [`Self::sbc_params`] (the encoder takes the same)
    pub fn configuration_with(
        &self,
        kind: CodecKind,
        sample_rate: u32,
        channels: u32,
        bits_per_sample: u8,
        vbr: bool,
        sbc: sbc::SbcParams,
    ) -> Result<Configuration, ConfigError> {
        tracing::debug!(
            "caps: {kind} configuration for {sample_rate} Hz, {channels} ch, {bits_per_sample} bit, vbr {vbr}, {sbc:?}"
        );
        let Some(sep) = self.sep(kind) else {
            return Err(ConfigError::NotOffered(kind));
        };
        let remote_seid = sep.seid;
        let mono = channels == 1;
        let request = match kind {
            CodecKind::Sbc | CodecKind::SbcXq => {
                if sample_rate != 44100 && sample_rate != 48000 {
                    return Err(ConfigError::SampleRate(kind, sample_rate));
                }
                // SBC: up to 53 (the local SEP's maximum) within the sink's
                // range; the encoder's bitpool is capped the same way.
                // SBC XQ: dual channel (mono for one channel), the bitpool
                // range as PipeWire's select_config: up to the largest XQ
                // bitpool of the rate, within the sink's range; the encoder's
                // bitpool (by quality) is never above it.
                let cap = if kind == CodecKind::SbcXq { sbc::xq_max_bitpool(sample_rate) } else { SBC_MAX_BITPOOL };
                // The profile's minimum bitpool, when set, raises the
                // minimum within that range (the encoder never goes below it)
                let (min_bitpool, max_bitpool) = self.sbc_bitpool_range(cap);
                let (p, _) = self.sbc_params(kind, sbc);
                let min_bitpool = p.config_min_bitpool(min_bitpool, max_bitpool);
                ConfigRequest::Sbc {
                    sampling_frequency: sample_rate as u16,
                    channel_mode: p.channel_mode(kind, channels),
                    blocks: p.blocks(),
                    subbands: p.subbands(),
                    allocation: p.allocation(),
                    min_bitpool,
                    max_bitpool,
                }
            }
            CodecKind::Aac => {
                let caps = self.aac_caps;
                let freq = match sample_rate {
                    44100 => AAC_FREQ_44100,
                    48000 => AAC_FREQ_48000,
                    _ => 0,
                };
                if freq == 0 || caps.is_some_and(|c| c.sampling_frequency & freq == 0) {
                    return Err(ConfigError::SampleRate(kind, sample_rate));
                }
                // MPEG-2 AAC-LC (mandatory for sinks) unless the sink lists
                // MPEG-4 AAC-LC only; the bit stream is the same
                let object_type = match caps {
                    Some(c) if c.object_types & AAC_OBJ_MPEG2_LC == 0 => AacObjectType::Mpeg4Lc,
                    _ => AacObjectType::Mpeg2Lc,
                };
                ConfigRequest::Aac {
                    object_type,
                    sampling_frequency: sample_rate,
                    channels: channels as u8,
                    bit_rate: if crate::dev::overrides().aac_config_bitrate0 { 0 } else { self.aac_max_bitrate() },
                    vbr: vbr && self.aac_vbr(),
                }
            }
            CodecKind::Ldac => {
                let freq = match sample_rate {
                    44100 => 0x20,
                    48000 => 0x10,
                    88200 => 0x08,
                    _ => 0x04,
                };
                let mut info = vendor_header(LDAC_VENDOR_ID, LDAC_CODEC_ID);
                info.extend_from_slice(&[freq, if mono { 0x04 } else { 0x01 }]);
                ConfigRequest::Vendor(info)
            }
            CodecKind::AptxHd => {
                let freq = aptx_freq_bits(kind, self.aptxhd_caps, sample_rate)?;
                let mut info = vendor_header(APTXHD_VENDOR_ID, APTXHD_CODEC_ID);
                // Always stereo on the wire; the encoder duplicates mono input
                info.push(freq | APTX_CH_STEREO);
                info.resize(APTXHD_INFO_LEN, 0);
                ConfigRequest::Vendor(info)
            }
            CodecKind::Aptx => {
                let freq = aptx_freq_bits(kind, self.aptx_caps, sample_rate)?;
                let mut info = vendor_header(APTX_VENDOR_ID, APTX_CODEC_ID);
                info.push(freq | APTX_CH_STEREO);
                ConfigRequest::Vendor(info)
            }
            CodecKind::AptxLl => {
                let freq = aptx_freq_bits(kind, self.aptxll_caps, sample_rate)?;
                let mut info = vendor_header(self.aptxll_vendor_id, APTXLL_CODEC_ID);
                info.push(freq | APTX_CH_STEREO);
                info.push(0); // no backchannel
                let rc = &self.aptxll_info;
                if rc.len() >= APTXLL_EXT_INFO_LEN && rc[7] & APTXLL_HAS_NEW_CAPS != 0 {
                    // The sink sent the extended caps: echo them back like
                    // PipeWire, with the buffer levels at least its defaults
                    let le16 = |i: usize| u16::from_le_bytes([rc[i], rc[i + 1]]);
                    let target = le16(9).max(APTXLL_TARGET_LEVEL);
                    let initial = le16(11).max(APTXLL_INITIAL_LEVEL);
                    let good = le16(15).max(APTXLL_GOOD_LEVEL);
                    info[7] = APTXLL_HAS_NEW_CAPS;
                    info.push(rc[8]);
                    info.extend_from_slice(&target.to_le_bytes());
                    info.extend_from_slice(&initial.to_le_bytes());
                    info.push(if rc[13] != 0 { rc[13] } else { APTXLL_SRA_MAX_RATE });
                    info.push(if rc[14] != 0 { rc[14] } else { APTXLL_SRA_AVG_TIME });
                    info.extend_from_slice(&good.to_le_bytes());
                }
                ConfigRequest::Vendor(info)
            }
            CodecKind::FastStream => {
                // The music direction at one rate; stereo on the wire (the
                // encoder duplicates mono input)
                let freq = match sample_rate {
                    44100 => FASTSTREAM_SINK_44100,
                    48000 => FASTSTREAM_SINK_48000,
                    _ => 0,
                };
                if freq & self.faststream_rates == 0 {
                    return Err(ConfigError::SampleRate(kind, sample_rate));
                }
                let mut info = vendor_header(FASTSTREAM_VENDOR_ID, FASTSTREAM_CODEC_ID);
                info.extend_from_slice(&[FASTSTREAM_DIRECTION_SINK, freq]);
                ConfigRequest::Vendor(info)
            }
            CodecKind::LhdcV5 => {
                // One rate, one bit depth, the sink's bit rate limits echoed,
                // V5 ver.1, 5 ms frames, no optional features; stereo only
                let freq = match sample_rate {
                    44100 => LHDCV5_SR_44100,
                    48000 => LHDCV5_SR_48000,
                    96000 => LHDCV5_SR_96000,
                    192000 => LHDCV5_SR_192000,
                    _ => 0,
                };
                if freq & self.lhdcv5_rates() == 0 {
                    return Err(ConfigError::SampleRate(kind, sample_rate));
                }
                let bits = if bits_per_sample == 24 { LHDCV5_BITS_24 } else { LHDCV5_BITS_16 };
                if bits & self.lhdcv5_info[7] & LHDCV5_BITS_MASK == 0 {
                    return Err(ConfigError::BitDepth(kind, bits_per_sample));
                }
                let mut info = vendor_header(LHDCV5_VENDOR_ID, LHDCV5_CODEC_ID);
                info.extend_from_slice(&[freq, bits | self.lhdcv5_bitrate_limits(), LHDCV5_CAPS_P8]);
                info.resize(LHDCV5_INFO_LEN, 0);
                ConfigRequest::Vendor(info)
            }
            CodecKind::Opus => {
                if sample_rate != 48000 {
                    return Err(ConfigError::SampleRate(kind, sample_rate));
                }
                let (frame_ms, wire) = self.opus_stream(channels);
                let ch = if wire == 1 { OPUS_CH_MONO } else { OPUS_CH_STEREO };
                if self.opus_caps & ch == 0 {
                    return Err(ConfigError::Channels(kind, wire));
                }
                let frame = if frame_ms == 10 { OPUS_FRAME_10MS } else { OPUS_FRAME_20MS };
                let mut info = vendor_header(OPUS_VENDOR_ID, OPUS_CODEC_ID);
                info.push(OPUS_FREQ_48000 | frame | ch);
                ConfigRequest::Vendor(info)
            }
            CodecKind::AptxAdaptive => {
                let freq = match sample_rate {
                    48000 => APTXAD_RATE_48000,
                    96000 if self.aptxad_96k() => APTXAD_RATE_96000,
                    _ => return Err(ConfigError::SampleRate(kind, sample_rate)),
                };
                // As upstream's select_config: 48 (or 96) kHz from a host encoder,
                // stereo (joint stereo if the sink lists only that; the
                // encoder runs stereo either way). An R2 sink gets the
                // source's own R2 block, an R1 sink its own bytes 8..25 back
                // with zero target latencies replaced and LL0 at least 80 ms
                // (§7G, §7H, §7J).
                let rc = &self.aptxad_info;
                let byte = |i: usize| rc.get(i).copied().unwrap_or(0);
                let mode = if byte(7) & APTXAD_CH_STEREO != 0 { APTXAD_CH_STEREO } else { APTXAD_CH_JOINT_STEREO };
                let rate = freq | APTXAD_SOURCE_TYPE_HOST;
                let info = if self.aptxad_r2() {
                    aptxad_element(rate, mode, &APTXAD_R2_DATA)
                } else {
                    let mut data: Vec<u8> = (APTXAD_DATA..APTXAD_R2_DATA_END).map(byte).collect();
                    for (b, default) in data.iter_mut().zip(APTXAD_TTP_DEFAULTS) {
                        if *b == 0 {
                            *b = default;
                        }
                    }
                    data[0] = data[0].max(APTXAD_LL0_MIN);
                    aptxad_element(rate, mode, &data)
                };
                ConfigRequest::Vendor(info)
            }
            CodecKind::Opus05 => {
                // The rate is not in the configuration: 48 kHz is the one
                // PipeWire lists first and the only one BlueCaster encodes
                if sample_rate != 48000 {
                    return Err(ConfigError::SampleRate(kind, sample_rate));
                }
                // As PipeWire's select_config / set_channel_conf for
                // opus_05: one frame duration, stereo as one coupled stream
                // at front left + right (mono: one stream, no location), the
                // bit rate limit, no return channel
                let (frame_ms, wire) = self.opus05_stream();
                let frame = if frame_ms == 10 { OPUS05_FRAME_10MS } else { OPUS05_FRAME_20MS };
                let (coupled, location) = if wire == 2 { (1, OPUS05_LOCATION_STEREO) } else { (0, 0) };
                let mut info = vendor_header(OPUS05_VENDOR_ID, OPUS05_CODEC_ID);
                info.extend_from_slice(&opus05_direction(wire, coupled, location, frame, self.opus05_bitrate_units()));
                info.resize(OPUS05_INFO_LEN, 0);
                ConfigRequest::Vendor(info)
            }
        };
        match &request {
            ConfigRequest::Vendor(info) => {
                tracing::debug!("caps: {kind} SET_CONFIGURATION to SEID {remote_seid}: {info:02X?}")
            }
            other => tracing::debug!("caps: {kind} SET_CONFIGURATION to SEID {remote_seid}: {other:?}"),
        }
        Ok(Configuration { kind, remote_seid, request })
    }
}

fn aptx_freq_bits(kind: CodecKind, remote_caps: u8, sample_rate: u32) -> Result<u8, ConfigError> {
    let freq = match sample_rate {
        44100 => APTX_FREQ_44100,
        48000 => APTX_FREQ_48000,
        _ => return Err(ConfigError::SampleRate(kind, sample_rate)),
    };
    let remote = remote_caps & APTX_FREQ_MASK;
    if remote != 0 && remote & freq == 0 {
        tracing::debug!("caps: {kind} {sample_rate} Hz not in the device's rates (caps 0x{remote_caps:02x})");
        return Err(ConfigError::SampleRate(kind, sample_rate));
    }
    Ok(freq)
}

/// SET_CONFIGURATION of a stream
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Configuration {
    pub kind: CodecKind,
    pub remote_seid: u8,
    pub request: ConfigRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigRequest {
    /// By default 16 blocks, 8 subbands, loudness; SBC joint stereo, SBC XQ
    /// dual channel (both mono for one channel)
    Sbc {
        sampling_frequency: u16,
        channel_mode: ChannelMode,
        blocks: u8,
        subbands: u8,
        allocation: Allocation,
        min_bitpool: u8,
        max_bitpool: u8,
    },
    /// AAC-LC, constant bit rate up to `bit_rate` (variable when `vbr`)
    Aac { object_type: AacObjectType, sampling_frequency: u32, channels: u8, bit_rate: u32, vbr: bool },
    /// Vendor codec information
    Vendor(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    #[error("the device does not offer {0}")]
    NotOffered(CodecKind),
    #[error("{0} does not support {1} Hz here")]
    SampleRate(CodecKind, u32),
    #[error("{0} does not support {1}-bit here")]
    BitDepth(CodecKind, u8),
    #[error("{0} does not support {1} channel(s) here")]
    Channels(CodecKind, u8),
}

/// Why the codec of a profile is not used
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unavailable {
    /// The codec may be on a SEP another device streams to (multipoint):
    /// BTstack does not query SEPs in use
    SepInUse {
        in_use: u8,
        total: u8,
    },
    /// The sink offers only aptX Adaptive, which has no open encoder
    AdaptiveOnly,
    NotSupported,
    /// An experimental codec while experimental codecs are turned off
    Experimental,
}

/// The codec to stream with
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    pub kind: CodecKind,
    /// The requested codec was replaced by `kind` (codec fallback)
    pub replaced: Option<(CodecKind, Unavailable)>,
}

/// Picks the codec for `requested` (None = Auto). A requested codec the
/// device does not offer is replaced, with `fallback`, by the best one it
/// offers (the first in [`CodecKind::AUTO_PRIORITY`], issue #86 round 8:
/// not the next one below the codec asked for); without, it is an error.
/// Unless `experimental`, the experimental codecs are never used: a
/// requested one is replaced the same way.
pub fn choose(
    caps: &RemoteCaps,
    requested: Option<CodecKind>,
    fallback: bool,
    experimental: bool,
) -> Result<Choice, Option<(CodecKind, Unavailable)>> {
    choose_in(caps, requested, fallback, experimental, &AutoOrder::default())
}

/// [`choose`] in the device's codec order as the user set it (the profile's
/// `codec_order`): Auto, and a fallback, take the first codec in `order`
/// the device offers
pub fn choose_in(
    caps: &RemoteCaps,
    requested: Option<CodecKind>,
    fallback: bool,
    experimental: bool,
    order: &AutoOrder,
) -> Result<Choice, Option<(CodecKind, Unavailable)>> {
    let Some(wanted) = requested else {
        return caps.best_in(order, experimental).map(|kind| Choice { kind, replaced: None }).ok_or(None);
    };
    if caps.usable(wanted, experimental) {
        return Ok(Choice { kind: wanted, replaced: None });
    }
    let why = if wanted.experimental() && !experimental {
        Unavailable::Experimental
    } else if caps.audio_sinks_in_use > 0 {
        Unavailable::SepInUse { in_use: caps.audio_sinks_in_use, total: caps.audio_sinks }
    } else if matches!(wanted, CodecKind::Aptx | CodecKind::AptxHd | CodecKind::AptxLl) && caps.aptx_adaptive {
        Unavailable::AdaptiveOnly
    } else {
        Unavailable::NotSupported
    };
    if fallback && let Some(kind) = caps.best_in(order, experimental) {
        return Ok(Choice { kind, replaced: Some((wanted, why)) });
    }
    Err(Some((wanted, why)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Quality;

    #[test]
    fn auto_prefers_ldac_then_aptx_family() {
        let mut caps = RemoteCaps::default();
        caps.add_sbc(1);
        caps.add_aac(2);
        assert_eq!(caps.best(), if CodecKind::Aac.available() { Some(CodecKind::Aac) } else { Some(CodecKind::Sbc) });
        caps.add_vendor(3, &[0x4F, 0, 0, 0, 0x01, 0x00, 0x22]);
        if CodecKind::Aptx.available() {
            assert_eq!(caps.best(), Some(CodecKind::Aptx));
        }
        caps.add_vendor(4, &[0x2D, 0x01, 0, 0, 0xAA, 0x00, 0x3C, 0x07]);
        if CodecKind::Ldac.available() {
            assert_eq!(caps.best(), Some(CodecKind::Ldac));
        }
    }

    #[test]
    fn mono_only_aptx_is_ignored() {
        let mut caps = RemoteCaps::default();
        let r = caps.add_vendor(3, &[0x4F, 0, 0, 0, 0x01, 0x00, 0x21]);
        assert_eq!(r, VendorCapability::Ignored(CodecKind::Aptx, "no stereo"));
        assert!(!caps.has(CodecKind::Aptx));
    }

    #[test]
    fn aptx_ll_extended_config_echoes_raised_levels() {
        let mut caps = RemoteCaps::default();
        let mut info = vec![0x0A, 0, 0, 0, 0x02, 0x00, 0x32, 0x02, 0x00];
        info.extend_from_slice(&100u16.to_le_bytes()); // target
        info.extend_from_slice(&1000u16.to_le_bytes()); // initial
        info.extend_from_slice(&[0, 0]); // sra
        info.extend_from_slice(&50u16.to_le_bytes()); // good
        assert_eq!(caps.add_vendor(7, &info), VendorCapability::Taken(CodecKind::AptxLl));
        let c = caps.configuration(CodecKind::AptxLl, 48000, 2, 16).unwrap();
        let ConfigRequest::Vendor(v) = c.request else { panic!() };
        assert_eq!(v.len(), 17);
        assert_eq!(v[6], 0x12);
        assert_eq!(v[7], APTXLL_HAS_NEW_CAPS);
        assert_eq!(u16::from_le_bytes([v[9], v[10]]), APTXLL_TARGET_LEVEL);
        assert_eq!(u16::from_le_bytes([v[11], v[12]]), 1000);
        assert_eq!(v[13], APTXLL_SRA_MAX_RATE);
        assert_eq!(u16::from_le_bytes([v[15], v[16]]), APTXLL_GOOD_LEVEL);
    }

    #[test]
    fn auto_order_is_by_sound_quality() {
        use CodecKind::*;
        assert_eq!(CodecKind::AUTO_PRIORITY, [LhdcV5, Ldac, AptxAdaptive, Opus, AptxHd, Aptx, AptxLl, Aac, SbcXq, Sbc]);
        assert!(LhdcV5.quality_rank() < Ldac.quality_rank());
        assert_eq!(Opus05.quality_rank(), Opus.quality_rank());
        assert_eq!(FastStream.quality_rank(), CodecKind::AUTO_PRIORITY.len());
    }

    #[test]
    fn auto_order_from_keys_keeps_every_codec_once() {
        use CodecKind::*;
        assert!(AutoOrder::from_keys::<&str>(&[]).is_default());
        let o = AutoOrder::from_keys(&["aac", "nope", "ldac", "aac", "fs"]);
        let c = o.codecs();
        assert_eq!(&c[..3], &[Aac, Ldac, LhdcV5]);
        assert_eq!(c.len(), CodecKind::AUTO_PRIORITY.len());
        assert_eq!(o.rank(Aac), 0);
        assert_eq!(o.rank(FastStream), c.len());
        assert_eq!(o.rank(Opus05), o.rank(Opus));
    }

    #[test]
    fn auto_and_the_fallback_follow_the_order_set() {
        use CodecKind::*;
        let mut caps = RemoteCaps::default();
        caps.add_sbc(1);
        caps.add_aac(2);
        caps.add_vendor(4, &[0x2D, 0x01, 0, 0, 0xAA, 0x00, 0x3C, 0x07]);
        if !Aac.available() || !Ldac.available() {
            return;
        }
        let aac_first = AutoOrder::from_keys(&["aac"]);
        assert_eq!(choose_in(&caps, None, true, true, &aac_first).unwrap().kind, Aac);
        assert_eq!(choose(&caps, None, true, true).unwrap().kind, Ldac);
        // aptX HD not offered: the first codec in the order set the device
        // offers (issue #86 round 8), SBC here, not LDAC below aptX HD
        let order = AutoOrder::from_keys(&["aptxhd", "sbc", "ldac"]);
        let c = choose_in(&caps, Some(AptxHd), true, true, &order).unwrap();
        assert_eq!(c.kind, Sbc);
        assert_eq!(c.replaced.map(|r| r.0), Some(AptxHd));
        // An offered codec is taken whatever the order
        assert_eq!(choose_in(&caps, Some(Ldac), true, true, &aac_first).unwrap().kind, Ldac);
    }

    #[test]
    fn fallback_takes_the_best_codec_offered() {
        use CodecKind::*;
        // SBC, AAC, aptX and LDAC (no aptX HD, no LHDC)
        let mut caps = RemoteCaps::default();
        caps.add_sbc(1);
        caps.add_aac(2);
        caps.add_vendor(3, &[0x4F, 0, 0, 0, 0x01, 0x00, 0x22]);
        caps.add_vendor(4, &[0x2D, 0x01, 0, 0, 0xAA, 0x00, 0x3C, 0x07]);
        let fell = |want| choose(&caps, Some(want), true, true).unwrap().kind;
        let best = caps.best().unwrap();
        // Whatever is missing: the best the device offers (issue #86 round
        // 8: not the next one below the codec asked for)
        for want in [LhdcV5, AptxHd, Opus, Opus05, SbcXq, FastStream, AptxLl] {
            assert_eq!(fell(want), best, "{want}");
        }
        if Ldac.available() {
            assert_eq!(fell(AptxHd), Ldac);
        }
        // An offered codec is taken as it is
        assert_eq!(fell(Sbc), Sbc);
        let r = choose(&caps, Some(SbcXq), true, true).unwrap();
        assert_eq!(r.replaced, Some((SbcXq, Unavailable::NotSupported)));
        // Only SBC: whatever was asked for ends there
        let mut sbc = RemoteCaps::default();
        sbc.add_sbc(1);
        for k in CodecKind::ALL {
            assert_eq!(choose(&sbc, Some(k), true, true).unwrap().kind, Sbc, "{k}");
        }
    }

    #[test]
    fn opus_missing_falls_back_to_aptx_adaptive_not_aptx() {
        use CodecKind::*;
        // The tester's EarFun Air Pro 4+ (issue #86 round 8): SBC, AAC, aptX
        // and aptX Adaptive; Opus chosen
        let mut caps = RemoteCaps::default();
        caps.add_sbc(1);
        caps.add_aac(2);
        caps.add_vendor(3, &[0x4F, 0, 0, 0, 0x01, 0x00, 0x22]);
        caps.add_vendor(9, &PX7_S2E_APTXAD);
        if !AptxAdaptive.available() || !Aptx.available() || !Aac.available() {
            return;
        }
        let c = choose(&caps, Some(Opus), true, true).unwrap();
        assert_eq!(c.kind, AptxAdaptive);
        assert_eq!(c.replaced.map(|r| r.0), Some(Opus));
        // The device's own order, AAC first: AAC
        let aac_first = AutoOrder::from_keys(&["aac", "sbc"]);
        assert_eq!(choose_in(&caps, Some(Opus), true, true, &aac_first).unwrap().kind, Aac);
    }

    #[test]
    fn fallback_reports_why() {
        let mut caps = RemoteCaps::default();
        caps.add_sbc(1);
        caps.audio_sinks = 2;
        caps.audio_sinks_in_use = 1;
        let c = choose(&caps, Some(CodecKind::Ldac), true, true).unwrap();
        assert_eq!(c.kind, CodecKind::Sbc);
        assert_eq!(c.replaced, Some((CodecKind::Ldac, Unavailable::SepInUse { in_use: 1, total: 2 })));
        assert!(choose(&caps, Some(CodecKind::Ldac), false, true).is_err());
    }

    #[test]
    fn ldac_rate_as_the_sink_lists_it() {
        let ldac = |rates: u8| {
            let mut caps = RemoteCaps::default();
            let mut info = vendor_header(LDAC_VENDOR_ID, LDAC_CODEC_ID);
            info.extend_from_slice(&[rates, LDAC_CH_ALL]);
            assert_eq!(caps.add_vendor(1, &info), VendorCapability::Taken(CodecKind::Ldac));
            caps
        };
        // Every rate: as asked (96 kHz chosen, Auto's 48 kHz)
        let all = ldac(LDAC_FREQ_ALL);
        assert_eq!((all.pick_ldac_sample_rate(96000), all.pick_ldac_sample_rate(48000)), (96000, 48000));
        // Not reported: as every rate
        assert_eq!(RemoteCaps::default().pick_ldac_sample_rate(96000), 96000);
        // 44.1 / 48 kHz only (88.2 / 96 kHz are optional): the highest below
        let base = ldac(0x30);
        assert_eq!((base.pick_ldac_sample_rate(96000), base.pick_ldac_sample_rate(44100)), (48000, 44100));
        // 88.2 kHz the highest
        assert_eq!(ldac(0x38).pick_ldac_sample_rate(96000), 88200);
        // Only above: the lowest of them
        assert_eq!(ldac(0x0C).pick_ldac_sample_rate(48000), 88200);
    }

    #[test]
    fn aac_caps_and_configuration() {
        let all = AacCapabilities {
            object_types: AAC_OBJ_MPEG2_LC | AAC_OBJ_MPEG4_LC,
            sampling_frequency: AAC_FREQ_44100 | AAC_FREQ_48000,
            channels: AAC_CH_1 | AAC_CH_2,
            bit_rate: 320_000,
            vbr: true,
        };
        // Unknown capabilities: as before
        let mut caps = RemoteCaps::default();
        caps.add_aac(2);
        assert_eq!(caps.aac_max_bitrate(), 256_000);
        assert_eq!((caps.pick_aac_sample_rate(44100), caps.pick_aac_sample_rate(96000)), (44100, 48000));

        // A sink above 256 kbps: 256 kbps, MPEG-2 AAC-LC
        caps.add_aac_caps(2, all);
        let c = caps.configuration(CodecKind::Aac, 48000, 2, 16).unwrap();
        assert_eq!(
            c.request,
            ConfigRequest::Aac {
                object_type: AacObjectType::Mpeg2Lc,
                sampling_frequency: 48000,
                channels: 2,
                bit_rate: 256_000,
                vbr: false
            }
        );

        // 128 kbps, 44.1 kHz, MPEG-4 AAC-LC only
        let mut caps = RemoteCaps::default();
        caps.add_aac_caps(
            3,
            AacCapabilities {
                object_types: AAC_OBJ_MPEG4_LC,
                sampling_frequency: AAC_FREQ_44100,
                bit_rate: 128_000,
                ..all
            },
        );
        assert_eq!(caps.aac_max_bitrate(), 128_000);
        assert_eq!(caps.pick_aac_sample_rate(48000), 44100);
        assert_eq!(
            caps.configuration(CodecKind::Aac, 48000, 2, 16),
            Err(ConfigError::SampleRate(CodecKind::Aac, 48000))
        );
        let c = caps.configuration(CodecKind::Aac, 44100, 2, 16).unwrap();
        assert_eq!(
            c.request,
            ConfigRequest::Aac {
                object_type: AacObjectType::Mpeg4Lc,
                sampling_frequency: 44100,
                channels: 2,
                bit_rate: 128_000,
                vbr: false
            }
        );
        // VBR asked for: taken only where the sink lists it
        let vbr = |c: Configuration| matches!(c.request, ConfigRequest::Aac { vbr: true, .. });
        assert!(vbr(caps.configuration_vbr(CodecKind::Aac, 44100, 2, 16, true).unwrap()));
        let mut cbr = RemoteCaps::default();
        cbr.add_aac_caps(
            3,
            AacCapabilities {
                object_types: AAC_OBJ_MPEG4_LC,
                sampling_frequency: AAC_FREQ_44100,
                bit_rate: 128_000,
                vbr: false,
                ..all
            },
        );
        assert!(!vbr(cbr.configuration_vbr(CodecKind::Aac, 44100, 2, 16, true).unwrap()));
        assert!(vbr(caps.configuration_vbr(CodecKind::Aac, 44100, 2, 16, true).unwrap()));
        assert!(!vbr(caps.configuration(CodecKind::Aac, 44100, 2, 16).unwrap()));

        // HE-AAC only: no AAC for Auto; a later usable SEP is taken
        let mut caps = RemoteCaps::default();
        caps.add_sbc(1);
        caps.add_aac_caps(2, AacCapabilities { object_types: 0x08, ..all });
        assert!(!caps.has(CodecKind::Aac));
        assert_eq!(caps.best(), Some(CodecKind::Sbc));
        caps.add_aac_caps(4, all);
        assert_eq!(caps.sep(CodecKind::Aac), Some(RemoteSep { seid: 4 }));
        caps.add_aac_caps(5, AacCapabilities { object_types: 0x08, ..all });
        assert_eq!(caps.sep(CodecKind::Aac), Some(RemoteSep { seid: 4 }));
    }

    /// A headset's SBC SEP: everything, bitpool 2..53
    fn sbc_all() -> SbcCapabilities {
        SbcCapabilities {
            sampling_frequency: 0x0F,
            channel_mode: 0x0F,
            block_length: 0x0F,
            subbands: 0x03,
            allocation_method: 0x03,
            min_bitpool: 2,
            max_bitpool: 53,
        }
    }

    #[test]
    fn capabilities_as_lists() {
        let c = SbcCapabilities {
            sampling_frequency: 0x03,
            channel_mode: 0x09,
            block_length: 0x05,
            subbands: 0x01,
            allocation_method: 0x01,
            ..sbc_all()
        };
        assert_eq!(c.sample_rates(), vec![44100, 48000]);
        assert_eq!(c.channel_modes(), vec![ChannelMode::Mono, ChannelMode::JointStereo]);
        assert_eq!((c.block_lengths(), c.subband_counts()), (vec![8, 16], vec![8]));
        assert_eq!(c.allocations(), vec![Allocation::Loudness]);
        let all = sbc_all();
        assert_eq!((all.sample_rates().len(), all.channel_modes().len(), all.allocations().len()), (4, 4, 2));
        let a = AacCapabilities {
            object_types: AAC_OBJ_MPEG2_LC | 0x04,
            sampling_frequency: AAC_FREQ_44100 | AAC_FREQ_48000 | 0x001,
            channels: AAC_CH_1 | AAC_CH_2,
            bit_rate: 320_000,
            vbr: true,
        };
        assert_eq!(a.object_type_names(), vec!["MPEG-2 AAC-LC", "MPEG-4 HE-AAC"]);
        assert_eq!(a.sample_rates(), vec![44100, 48000, 96000]);
        assert_eq!(a.channel_counts(), vec![1, 2]);
    }

    #[test]
    fn listed_rates_depths_and_bit_rates() {
        // Not offered: nothing listed
        let none = RemoteCaps::default();
        assert!(none.listed_sample_rates(CodecKind::Ldac).is_empty());
        assert_eq!(none.listed_max_bitrate(CodecKind::Aac), 0);

        // An LHDC V5 sink of 44.1 / 48 kHz, 16 / 24-bit, at most 900 kbps
        // (moto buds+: 48 kHz at most, issue #86)
        let mut caps = RemoteCaps { lhdcv5: Some(RemoteSep { seid: 1 }), ..Default::default() };
        caps.lhdcv5_info[6] = LHDCV5_SR_44100 | LHDCV5_SR_48000;
        caps.lhdcv5_info[7] = LHDCV5_BITS_16 | LHDCV5_BITS_24 | 0x30;
        assert_eq!(caps.listed_sample_rates(CodecKind::LhdcV5), vec![44100, 48000]);
        assert_eq!(caps.listed_bit_depths(CodecKind::LhdcV5), vec![16, 24]);
        assert_eq!(caps.listed_max_bitrate(CodecKind::LhdcV5), 900_000);
        caps.lhdcv5_info[7] = LHDCV5_BITS_16;
        assert_eq!(caps.listed_bit_depths(CodecKind::LhdcV5), vec![16]);
        assert_eq!(caps.listed_max_bitrate(CodecKind::LhdcV5), 0);

        // AAC's bit rate field as it is (0x020000 = 131072 bps)
        caps.add_aac_caps(
            2,
            AacCapabilities {
                object_types: AAC_OBJ_MPEG2_LC,
                sampling_frequency: AAC_FREQ_44100 | AAC_FREQ_48000,
                channels: AAC_CH_2,
                bit_rate: 0x02_0000,
                vbr: false,
            },
        );
        assert_eq!(caps.listed_max_bitrate(CodecKind::Aac), 131_072);
        assert_eq!(caps.listed_sample_rates(CodecKind::Aac), vec![44100, 48000]);

        // SBC from its capabilities; LDAC's rates, every one when not said
        caps.add_sbc_caps(3, SbcCapabilities { sampling_frequency: SBC_FREQ_48000, ..sbc_all() });
        assert_eq!(caps.listed_sample_rates(CodecKind::Sbc), vec![48000]);
        caps.ldac = Some(RemoteSep { seid: 4 });
        caps.ldac_rates = 0x30;
        assert_eq!(caps.listed_sample_rates(CodecKind::Ldac), vec![44100, 48000]);
        caps.ldac_rates = 0;
        assert_eq!(caps.listed_sample_rates(CodecKind::Ldac), vec![44100, 48000, 88200, 96000]);

        // aptX Adaptive: 96 kHz only to an R2 sink that lists it
        let mut r2 = RemoteCaps::default();
        r2.add_vendor(5, &r2_aptxad(0x0F00_0000)[..30]);
        assert_eq!(r2.listed_sample_rates(CodecKind::AptxAdaptive), vec![48000, 96000]);
        let mut r1 = RemoteCaps::default();
        r1.add_vendor(5, &PX7_S2E_APTXAD);
        assert_eq!(r1.listed_sample_rates(CodecKind::AptxAdaptive), vec![48000]);
    }

    #[test]
    fn sbc_xq_negotiation() {
        let mut caps = RemoteCaps::default();
        caps.add_sbc_caps(1, sbc_all());
        assert!(caps.has(CodecKind::SbcXq) && caps.has(CodecKind::Sbc));
        assert_eq!(caps.sep(CodecKind::SbcXq), Some(RemoteSep { seid: 1 }));
        assert_eq!(caps.best(), Some(CodecKind::SbcXq));
        assert_eq!(caps.sbc_max_bitpool(), 53);
        assert_eq!(CodecKind::from_key("SBCXQ"), Some(CodecKind::SbcXq));
        assert_eq!((CodecKind::SbcXq.name(), CodecKind::SbcXq.media_header_len()), ("SBC XQ", 1));
        assert!(CodecKind::SbcXq.uses_rtp());
        // Above SBC, below AAC
        caps.add_aac(2);
        if CodecKind::Aac.available() {
            assert_eq!(caps.best(), Some(CodecKind::Aac));
        }

        let c = caps.configuration(CodecKind::SbcXq, 44100, 2, 16).unwrap();
        assert_eq!(endpoint_kind(c.kind), CodecKind::Sbc);
        assert_eq!(
            c.request,
            ConfigRequest::Sbc {
                sampling_frequency: 44100,
                channel_mode: ChannelMode::DualChannel,
                blocks: 16,
                subbands: 8,
                allocation: Allocation::Loudness,
                min_bitpool: 2,
                max_bitpool: 47
            }
        );
        let c = caps.configuration(CodecKind::SbcXq, 48000, 1, 16).unwrap();
        assert_eq!(
            c.request,
            ConfigRequest::Sbc {
                sampling_frequency: 48000,
                channel_mode: ChannelMode::Mono,
                blocks: 16,
                subbands: 8,
                allocation: Allocation::Loudness,
                min_bitpool: 2,
                max_bitpool: 43
            }
        );
        assert!(caps.configuration(CodecKind::SbcXq, 32000, 2, 16).is_err());
        // Plain SBC is unchanged
        let c = caps.configuration(CodecKind::Sbc, 48000, 2, 16).unwrap();
        assert_eq!(
            c.request,
            ConfigRequest::Sbc {
                sampling_frequency: 48000,
                channel_mode: ChannelMode::JointStereo,
                blocks: 16,
                subbands: 8,
                allocation: Allocation::Loudness,
                min_bitpool: 2,
                max_bitpool: 53
            }
        );

        // A sink taking up to 45: XQ capped there
        let mut caps = RemoteCaps::default();
        caps.add_sbc_caps(1, SbcCapabilities { max_bitpool: 45, ..sbc_all() });
        let c = caps.configuration(CodecKind::SbcXq, 44100, 2, 16).unwrap();
        assert!(matches!(c.request, ConfigRequest::Sbc { max_bitpool: 45, .. }));

        // Moto buds+ (issue #86): dual channel up to 39. PipeWire streams
        // SBC-XQ at 39 per channel there (~463 kbps at 44.1 kHz), so do we
        let moto = SbcCapabilities { min_bitpool: 2, max_bitpool: 39, ..sbc_all() };
        assert_eq!(moto.xq_refusal(), None);
        let mut caps = RemoteCaps::default();
        caps.add_sbc_caps(1, moto);
        assert!(caps.has(CodecKind::SbcXq));
        for rate in [44100, 48000] {
            let c = caps.configuration(CodecKind::SbcXq, rate, 2, 16).unwrap();
            let ConfigRequest::Sbc { min_bitpool, max_bitpool, channel_mode, .. } = c.request else { panic!() };
            assert_eq!((min_bitpool, max_bitpool, channel_mode), (2, 39, ChannelMode::DualChannel), "{rate}");
        }
        // Below the lowest XQ bitpool: plain SBC, and the log says why
        let low = SbcCapabilities { max_bitpool: 28, ..sbc_all() };
        assert_eq!(low.xq_refusal(), Some("largest bitpool below 29"));
        assert_eq!(SbcCapabilities { channel_mode: 0x0B, ..sbc_all() }.xq_refusal(), Some("no dual channel"));
    }

    #[test]
    fn sbc_configuration_takes_the_profiles_setup() {
        let mut caps = RemoteCaps::default();
        caps.add_sbc_caps(1, sbc_all());
        let wanted = sbc::SbcParams {
            blocks: 8,
            subbands: 4,
            allocation: Some(Allocation::Snr),
            channel_mode: Some(ChannelMode::Stereo),
            min_bitpool: 0,
        };
        let c = caps.configuration_with(CodecKind::Sbc, 44100, 2, 16, false, wanted).unwrap();
        assert_eq!(
            c.request,
            ConfigRequest::Sbc {
                sampling_frequency: 44100,
                channel_mode: ChannelMode::Stereo,
                blocks: 8,
                subbands: 4,
                allocation: Allocation::Snr,
                min_bitpool: 2,
                max_bitpool: 53
            }
        );
        assert_eq!(caps.sbc_params(CodecKind::Sbc, wanted), (wanted, vec![]));
        // Mono input stays mono; mono asked for stereo input is mono too
        let c = caps.configuration_with(CodecKind::Sbc, 48000, 1, 16, false, wanted).unwrap();
        assert!(matches!(c.request, ConfigRequest::Sbc { channel_mode: ChannelMode::Mono, blocks: 8, .. }));
        let mono = sbc::SbcParams { channel_mode: Some(ChannelMode::Mono), ..Default::default() };
        let c = caps.configuration_with(CodecKind::Sbc, 48000, 2, 16, false, mono).unwrap();
        assert!(matches!(c.request, ConfigRequest::Sbc { channel_mode: ChannelMode::Mono, blocks: 16, .. }));
        // SBC XQ: the frame setup, but always dual channel and its bitpool cap
        let c = caps.configuration_with(CodecKind::SbcXq, 44100, 2, 16, false, wanted).unwrap();
        assert_eq!(
            c.request,
            ConfigRequest::Sbc {
                sampling_frequency: 44100,
                channel_mode: ChannelMode::DualChannel,
                blocks: 8,
                subbands: 4,
                allocation: Allocation::Snr,
                min_bitpool: 2,
                max_bitpool: 47
            }
        );
        // Unknown capabilities: taken as asked
        let mut unknown = RemoteCaps::default();
        unknown.add_sbc(1);
        let c = unknown.configuration_with(CodecKind::Sbc, 44100, 2, 16, false, wanted).unwrap();
        assert!(matches!(c.request, ConfigRequest::Sbc { blocks: 8, subbands: 4, allocation: Allocation::Snr, .. }));
    }

    #[test]
    fn sbc_setup_the_device_does_not_list_falls_back() {
        // 16 blocks, 8 subbands, loudness, joint stereo and mono only
        let mut caps = RemoteCaps::default();
        caps.add_sbc_caps(
            1,
            SbcCapabilities {
                channel_mode: 0x09,
                block_length: 0x01,
                subbands: 0x01,
                allocation_method: 0x01,
                ..sbc_all()
            },
        );
        let wanted = sbc::SbcParams {
            blocks: 4,
            subbands: 4,
            allocation: Some(Allocation::Snr),
            channel_mode: Some(ChannelMode::Stereo),
            min_bitpool: 0,
        };
        let (p, notes) = caps.sbc_params(CodecKind::Sbc, wanted);
        assert!(p.is_default(), "{p:?}");
        assert_eq!(notes.len(), 4, "{notes:?}");
        assert!(notes[0].contains("4 blocks"), "{notes:?}");
        let c = caps.configuration_with(CodecKind::Sbc, 44100, 2, 16, false, wanted).unwrap();
        assert_eq!(c.request, caps.configuration(CodecKind::Sbc, 44100, 2, 16).unwrap().request);
        // What it lists is kept
        let (p, notes) = caps
            .sbc_params(CodecKind::Sbc, sbc::SbcParams { blocks: 16, channel_mode: Some(ChannelMode::Mono), ..wanted });
        assert_eq!((p.blocks, p.subbands, p.allocation, p.channel_mode), (16, 0, None, Some(ChannelMode::Mono)));
        assert_eq!(notes.len(), 2);
        // Values SBC does not have are the default, without a note
        let (p, notes) =
            caps.sbc_params(CodecKind::Sbc, sbc::SbcParams { blocks: 6, subbands: 2, ..Default::default() });
        assert!(p.is_default() && notes.is_empty());
        // SBC XQ never takes a channel mode of its own
        let (p, notes) = caps.sbc_params(
            CodecKind::SbcXq,
            sbc::SbcParams { channel_mode: Some(ChannelMode::Stereo), ..Default::default() },
        );
        assert!(p.is_default() && notes.is_empty());
        // Plain SBC has no dual channel (SBC XQ is that, issue #86), even
        // from a sink that lists it
        let mut all = RemoteCaps::default();
        all.add_sbc_caps(1, sbc_all());
        let dual = sbc::SbcParams { channel_mode: Some(ChannelMode::DualChannel), ..Default::default() };
        let (p, notes) = all.sbc_params(CodecKind::Sbc, dual);
        assert!(p.is_default() && notes.len() == 1, "{p:?} {notes:?}");
        let c = all.configuration_with(CodecKind::Sbc, 44100, 2, 16, false, dual).unwrap();
        assert!(matches!(
            c.request,
            ConfigRequest::Sbc { channel_mode: ChannelMode::JointStereo, max_bitpool: 53, .. }
        ));
    }

    #[test]
    fn local_sbc_endpoint_offers_every_setup() {
        let sbc = local_endpoints().into_iter().find(|e| e.kind == CodecKind::Sbc).unwrap();
        // All block lengths, both subbands and allocations
        assert_eq!(sbc.caps[1], 0xFF);
        // Configured by default: 16 blocks, 8 subbands, loudness
        assert_eq!(sbc.config[1], 0x15);
    }

    #[test]
    fn sbc_uses_the_sinks_bitpool_range() {
        // Not known (no capabilities): 2..53
        let mut caps = RemoteCaps::default();
        caps.add_sbc(1);
        assert_eq!(caps.sbc_bitpool_range(53), (2, 53));
        let c = caps.configuration(CodecKind::Sbc, 48000, 2, 16).unwrap();
        assert!(matches!(c.request, ConfigRequest::Sbc { min_bitpool: 2, max_bitpool: 53, .. }));
        // (sink min, sink max) -> configured (min, max)
        for ((smin, smax), want) in [
            ((2, 53), (2, 53)),
            ((2, 35), (2, 35)),
            ((2, 250), (2, 53)),
            ((10, 40), (10, 40)),
            ((0, 0), (2, 2)),
            ((60, 70), (53, 53)),
        ] {
            let mut caps = RemoteCaps::default();
            caps.add_sbc_caps(1, SbcCapabilities { min_bitpool: smin, max_bitpool: smax, ..sbc_all() });
            assert_eq!((caps.sbc_min_bitpool(), caps.sbc_max_bitpool()), (smin, smax));
            let c = caps.configuration(CodecKind::Sbc, 44100, 2, 16).unwrap();
            let ConfigRequest::Sbc { min_bitpool, max_bitpool, channel_mode, .. } = c.request else { panic!() };
            assert_eq!(((min_bitpool, max_bitpool), channel_mode), (want, ChannelMode::JointStereo), "{smin}-{smax}");
        }
    }

    #[test]
    fn sbc_configuration_takes_the_profiles_minimum_bitpool() {
        let range = |caps: &RemoteCaps, kind: CodecKind, rate: u32, min: u8| {
            let p = sbc::SbcParams { min_bitpool: min, ..Default::default() };
            let c = caps.configuration_with(kind, rate, 2, 16, false, p).unwrap();
            let ConfigRequest::Sbc { min_bitpool, max_bitpool, .. } = c.request else { panic!() };
            (min_bitpool, max_bitpool)
        };
        // (sink min, sink max, profile min) -> configured (min, max)
        for ((smin, smax), min, want) in [
            ((2, 53), 0, (2, 53)), // Auto: as before
            ((2, 53), 30, (30, 53)),
            ((2, 53), 60, (53, 53)),  // never above the maximum
            ((10, 40), 5, (10, 40)),  // never below the sink's minimum
            ((10, 40), 45, (40, 40)), // nor above its maximum
            ((2, 250), 53, (53, 53)),
        ] {
            let mut caps = RemoteCaps::default();
            caps.add_sbc_caps(1, SbcCapabilities { min_bitpool: smin, max_bitpool: smax, ..sbc_all() });
            assert_eq!(range(&caps, CodecKind::Sbc, 44100, min), want, "{smin}-{smax} {min}");
        }
        // SBC XQ: within its cap of the rate
        let mut caps = RemoteCaps::default();
        caps.add_sbc_caps(1, sbc_all());
        assert_eq!(range(&caps, CodecKind::SbcXq, 48000, 0), (2, 43));
        assert_eq!(range(&caps, CodecKind::SbcXq, 48000, 30), (30, 43));
        assert_eq!(range(&caps, CodecKind::SbcXq, 48000, 50), (43, 43));
        // Capabilities not known: 2..53 and the minimum in it
        let mut unknown = RemoteCaps::default();
        unknown.add_sbc(1);
        assert_eq!(range(&unknown, CodecKind::Sbc, 44100, 0), (2, 53));
        assert_eq!(range(&unknown, CodecKind::Sbc, 44100, 20), (20, 53));
    }

    #[test]
    fn sbc_xq_falls_back_to_sbc() {
        let no_xq = [
            SbcCapabilities { channel_mode: 0x0B, ..sbc_all() }, // no dual channel
            SbcCapabilities { max_bitpool: 28, ..sbc_all() },    // bitpool too low
            SbcCapabilities { block_length: 0x0E, ..sbc_all() }, // no 16 blocks
            SbcCapabilities { subbands: 0x02, ..sbc_all() },     // 4 subbands only
            SbcCapabilities { allocation_method: 0x02, ..sbc_all() }, // SNR only
            SbcCapabilities { sampling_frequency: 0x0C, ..sbc_all() }, // 16 / 32k only
        ];
        for sbc in no_xq {
            let mut caps = RemoteCaps::default();
            caps.add_sbc_caps(1, sbc);
            assert!(!caps.has(CodecKind::SbcXq), "{sbc:?}");
            assert_eq!(caps.best(), Some(CodecKind::Sbc));
            assert_eq!(
                caps.configuration(CodecKind::SbcXq, 48000, 2, 16),
                Err(ConfigError::NotOffered(CodecKind::SbcXq))
            );
            let c = choose(&caps, Some(CodecKind::SbcXq), true, true).unwrap();
            assert_eq!(c.kind, CodecKind::Sbc);
            assert_eq!(c.replaced, Some((CodecKind::SbcXq, Unavailable::NotSupported)));
            assert!(choose(&caps, Some(CodecKind::SbcXq), false, true).is_err());
        }
        // Capabilities not known: SBC only
        let mut caps = RemoteCaps::default();
        caps.add_sbc(3);
        assert!(!caps.has(CodecKind::SbcXq));
        // A second SBC SEP without dual channel keeps the one that allows XQ
        let mut caps = RemoteCaps::default();
        caps.add_sbc_caps(1, sbc_all());
        caps.add_sbc_caps(2, SbcCapabilities { channel_mode: 0x01, ..sbc_all() });
        assert_eq!(caps.sep(CodecKind::SbcXq), Some(RemoteSep { seid: 1 }));
    }

    #[test]
    fn faststream_negotiation() {
        // A CSR headset: music 44.1 / 48 kHz plus the 16 kHz voice direction
        let mut caps = RemoteCaps::default();
        caps.add_sbc_caps(1, sbc_all());
        let info = [0x0A, 0, 0, 0, 0x01, 0x00, 0x03, 0x23];
        assert_eq!(caps.add_vendor(5, &info), VendorCapability::Taken(CodecKind::FastStream));
        assert!(caps.has(CodecKind::FastStream));
        // Never Auto's choice, not even over SBC
        assert_eq!(caps.best(), Some(CodecKind::SbcXq));
        let mut only = RemoteCaps::default();
        only.add_vendor(5, &info);
        assert_eq!(only.best(), None);
        assert_eq!(choose(&only, Some(CodecKind::FastStream), false, true).unwrap().kind, CodecKind::FastStream);

        let c = caps.configuration(CodecKind::FastStream, 48000, 2, 16).unwrap();
        assert_eq!(c.remote_seid, 5);
        assert_eq!(c.request, ConfigRequest::Vendor(vec![0x0A, 0, 0, 0, 0x01, 0x00, 0x01, 0x01]));
        let c = caps.configuration(CodecKind::FastStream, 44100, 1, 16).unwrap();
        assert_eq!(c.request, ConfigRequest::Vendor(vec![0x0A, 0, 0, 0, 0x01, 0x00, 0x01, 0x02]));
        assert!(caps.configuration(CodecKind::FastStream, 32000, 2, 16).is_err());
        assert_eq!(caps.pick_faststream_sample_rate(44100), 44100);
        assert_eq!(caps.pick_faststream_sample_rate(96000), 48000);

        // 44.1 kHz only
        let mut caps = RemoteCaps::default();
        caps.add_vendor(5, &[0x0A, 0, 0, 0, 0x01, 0x00, 0x01, 0x02]);
        assert_eq!(caps.pick_faststream_sample_rate(48000), 44100);
        assert!(caps.configuration(CodecKind::FastStream, 48000, 2, 16).is_err());

        // Voice only, no rate, too short: ignored; aptX LL (same vendor) is not FastStream
        for bad in [
            &[0x0A, 0, 0, 0, 0x01, 0x00, 0x02, 0x20][..],
            &[0x0A, 0, 0, 0, 0x01, 0x00, 0x01, 0x20],
            &[0x0A, 0, 0, 0, 0x01, 0x00, 0x01],
        ] {
            let mut caps = RemoteCaps::default();
            assert!(matches!(caps.add_vendor(5, bad), VendorCapability::Ignored(CodecKind::FastStream, _)));
            assert!(!caps.has(CodecKind::FastStream));
        }
        let mut caps = RemoteCaps::default();
        assert_eq!(
            caps.add_vendor(6, &[0x0A, 0, 0, 0, 0x02, 0x00, 0x32, 0x00]),
            VendorCapability::Taken(CodecKind::AptxLl)
        );
        assert!(!caps.has(CodecKind::FastStream));

        // Fallback from FastStream goes to the best codec
        let mut caps = RemoteCaps::default();
        caps.add_sbc(1);
        let c = choose(&caps, Some(CodecKind::FastStream), true, true).unwrap();
        assert_eq!((c.kind, c.replaced), (CodecKind::Sbc, Some((CodecKind::FastStream, Unavailable::NotSupported))));

        // The local SEP: vendor header, music direction, 44.1 + 48 kHz
        let ep = local_endpoints().into_iter().find(|e| e.kind == CodecKind::FastStream).unwrap();
        assert_eq!(ep.caps, vec![0x0A, 0, 0, 0, 0x01, 0x00, 0x01, 0x03]);
        assert_eq!(ep.config, vec![0x0A, 0, 0, 0, 0x01, 0x00, 0x01, 0x01]);
        assert_eq!(CodecKind::from_key("faststream"), Some(CodecKind::FastStream));
        assert!(!CodecKind::FastStream.uses_rtp());
        assert_eq!(CodecKind::FastStream.media_header_len(), 0);
    }

    #[test]
    fn opus_caps_and_configuration() {
        let mut caps = RemoteCaps::default();
        caps.add_sbc(1);
        caps.add_aac(2);
        // Android's sink capabilities: 48 kHz, 20 ms, stereo
        let r = caps.add_vendor(5, &[0xE0, 0, 0, 0, 0x01, 0x00, 0x92]);
        assert_eq!(r, VendorCapability::Taken(CodecKind::Opus));
        if CodecKind::Opus.available() {
            assert_eq!(caps.best(), Some(CodecKind::Opus));
        }
        assert_eq!(caps.opus_stream(2), (20, 2));
        // No mono on the sink: mono input goes out as stereo
        assert_eq!(caps.opus_stream(1), (20, 2));
        let c = caps.configuration(CodecKind::Opus, 48000, 2, 16).unwrap();
        assert_eq!(c.remote_seid, 5);
        assert_eq!(c.request, ConfigRequest::Vendor(vec![0xE0, 0, 0, 0, 0x01, 0x00, 0x92]));
        assert_eq!(
            caps.configuration(CodecKind::Opus, 44100, 2, 16),
            Err(ConfigError::SampleRate(CodecKind::Opus, 44100))
        );
        // Opus is above the aptX family in Auto's order (issue #86)
        caps.add_vendor(3, &[0x4F, 0, 0, 0, 0x01, 0x00, 0x22]);
        if CodecKind::Opus.available() {
            assert_eq!(caps.best(), Some(CodecKind::Opus));
        }

        // PipeWire's capabilities: 10 + 20 ms, mono / stereo / dual mono
        let mut pw = RemoteCaps::default();
        pw.add_vendor(3, &[0xE0, 0, 0, 0, 0x01, 0x00, 0x9F]);
        assert_eq!(pw.opus_stream(1), (20, 1));
        let c = pw.configuration(CodecKind::Opus, 48000, 1, 16).unwrap();
        assert_eq!(c.request, ConfigRequest::Vendor(vec![0xE0, 0, 0, 0, 0x01, 0x00, 0x91]));
        // 10 ms only
        let mut short = RemoteCaps::default();
        short.add_vendor(3, &[0xE0, 0, 0, 0, 0x01, 0x00, 0x8A]);
        assert_eq!(short.opus_stream(2), (10, 2));
        let c = short.configuration(CodecKind::Opus, 48000, 2, 16).unwrap();
        assert_eq!(c.request, ConfigRequest::Vendor(vec![0xE0, 0, 0, 0, 0x01, 0x00, 0x8A]));

        // Unusable: no 48 kHz, no duration, dual mono only, too short
        let bad: [&[u8]; 4] = [
            &[0xE0, 0, 0, 0, 0x01, 0x00, 0x12],
            &[0xE0, 0, 0, 0, 0x01, 0x00, 0x82],
            &[0xE0, 0, 0, 0, 0x01, 0x00, 0x94],
            &[0xE0, 0, 0, 0, 0x01, 0x00],
        ];
        for info in bad {
            let mut c = RemoteCaps::default();
            assert!(matches!(c.add_vendor(3, info), VendorCapability::Ignored(CodecKind::Opus, _)), "{info:02X?}");
            assert!(!c.has(CodecKind::Opus));
        }
    }

    #[test]
    fn opus_local_endpoint() {
        let eps = local_endpoints();
        let Some(ep) = eps.iter().find(|e| e.kind == CodecKind::Opus) else {
            assert!(!CodecKind::Opus.available());
            return;
        };
        assert_eq!(ep.codec_type, CODEC_TYPE_VENDOR);
        assert_eq!(ep.caps, vec![0xE0, 0, 0, 0, 0x01, 0x00, 0x9B]);
        assert_eq!(ep.config, vec![0xE0, 0, 0, 0, 0x01, 0x00, 0x92]);
        // BTstack's static endpoint pool (MAX_NR_AVDTP_STREAM_ENDPOINTS)
        assert!(eps.len() <= 11);
    }

    /// PipeWire's opus_05 sink SEP (codec_fill_caps): stereo at front left +
    /// right, every frame duration, no bit rate limit, no return channel
    fn pipewire_opus05(main_channels: u8, frames: u8, bitrate: u16, bidi_channels: u8) -> Vec<u8> {
        let mut info = vec![0xF1, 0x05, 0, 0, 0x05, 0x10];
        info.extend_from_slice(&opus05_direction(main_channels, 0, 0x3, frames, bitrate));
        info.extend_from_slice(&opus05_direction(bidi_channels, 0, if bidi_channels > 0 { 0x3 } else { 0 }, 0x1F, 0));
        info
    }

    #[test]
    fn opus05_caps_and_configuration() {
        let mut caps = RemoteCaps::default();
        caps.add_sbc_caps(1, sbc_all());
        let r = caps.add_vendor(6, &pipewire_opus05(2, 0x1F, 0, 0));
        assert_eq!(r, VendorCapability::Taken(CodecKind::Opus05));
        assert!(caps.has(CodecKind::Opus05));
        // Never Auto's choice
        assert_eq!(caps.best(), Some(CodecKind::SbcXq));
        let mut only = RemoteCaps::default();
        only.add_vendor(6, &pipewire_opus05(2, 0x1F, 0, 0));
        assert_eq!(only.best(), None);
        if CodecKind::Opus05.available() {
            assert_eq!(choose(&only, Some(CodecKind::Opus05), false, true).unwrap().kind, CodecKind::Opus05);
        }

        // 10 ms, stereo (mono input too), 312 * 1024 bps, no return channel
        assert_eq!(caps.opus05_stream(), (10, 2));
        assert_eq!(caps.opus_params(CodecKind::Opus05, 1), (10, 2, 312 * 1024));
        let mut want = vec![0xF1, 0x05, 0, 0, 0x05, 0x10, 2, 1, 0x03, 0, 0, 0, 0x04, 0x38, 0x01];
        want.resize(24, 0);
        for ch in [2, 1] {
            let c = caps.configuration(CodecKind::Opus05, 48000, ch, 16).unwrap();
            assert_eq!(c.remote_seid, 6);
            assert_eq!(c.request, ConfigRequest::Vendor(want.clone()));
        }
        assert_eq!(
            caps.configuration(CodecKind::Opus05, 44100, 2, 16),
            Err(ConfigError::SampleRate(CodecKind::Opus05, 44100))
        );

        // 20 ms only with a 128-unit limit; a mono-only sink
        let vendor_info = |caps: &RemoteCaps| match caps.configuration(CodecKind::Opus05, 48000, 2, 16).unwrap().request
        {
            ConfigRequest::Vendor(v) => v,
            r => panic!("{r:?}"),
        };
        let mut c20 = RemoteCaps::default();
        c20.add_vendor(6, &pipewire_opus05(2, 0x08, 128, 0));
        assert_eq!(c20.opus_params(CodecKind::Opus05, 2), (20, 2, 128 * 1024));
        let v = vendor_info(&c20);
        assert_eq!((v[12], u16::from_le_bytes([v[13], v[14]])), (0x08, 128));
        let mut mono = RemoteCaps::default();
        mono.add_vendor(6, &pipewire_opus05(1, 0x04, 1000, 0));
        assert_eq!(&vendor_info(&mono)[6..15], &[1, 0, 0, 0, 0, 0, 0x04, 0x38, 0x01]);

        // PipeWire's duplex SEP is taken only when there is no other
        let mut both = RemoteCaps::default();
        both.add_vendor(7, &pipewire_opus05(2, 0x1F, 0, 2));
        assert_eq!(both.sep(CodecKind::Opus05), Some(RemoteSep { seid: 7 }));
        both.add_vendor(6, &pipewire_opus05(8, 0x1F, 0, 0));
        both.add_vendor(8, &pipewire_opus05(2, 0x1F, 0, 2));
        assert_eq!(both.sep(CodecKind::Opus05), Some(RemoteSep { seid: 6 }));
        let v = vendor_info(&both);
        assert_eq!((v[6], v[15]), (2, 0));

        // Unusable: no channel, no 10 / 20 ms, too short; Android's Opus is not Opus 05
        let bad = [
            pipewire_opus05(0, 0x1F, 0, 0),
            pipewire_opus05(2, 0x13, 0, 0),
            pipewire_opus05(2, 0x1F, 0, 0)[..15].to_vec(),
        ];
        for info in bad {
            let mut c = RemoteCaps::default();
            assert!(matches!(c.add_vendor(3, &info), VendorCapability::Ignored(CodecKind::Opus05, _)), "{info:02X?}");
            assert!(!c.has(CodecKind::Opus05));
        }
        let mut android = RemoteCaps::default();
        android.add_vendor(5, &[0xE0, 0, 0, 0, 0x01, 0x00, 0x92]);
        assert!(android.has(CodecKind::Opus) && !android.has(CodecKind::Opus05));
        assert_eq!(android.opus_params(CodecKind::Opus, 2), (20, 2, 0));
        assert_eq!(CodecKind::from_key("opus05"), Some(CodecKind::Opus05));
        assert_eq!((CodecKind::Opus05.name(), CodecKind::Opus05.media_header_len()), ("Opus 05", 1));
        assert!(CodecKind::Opus05.uses_rtp() && CodecKind::Opus05.is_opus());
    }

    #[test]
    fn opus05_local_endpoint() {
        let eps = local_endpoints();
        let Some(ep) = eps.iter().find(|e| e.kind == CodecKind::Opus05) else {
            assert!(!CodecKind::Opus05.available());
            return;
        };
        assert_eq!(ep.codec_type, CODEC_TYPE_VENDOR);
        let mut caps = vec![0xF1, 0x05, 0, 0, 0x05, 0x10, 2, 0, 0x03, 0, 0, 0, 0x0C, 0, 0];
        caps.resize(24, 0);
        assert_eq!(ep.caps, caps);
        let mut config = vec![0xF1, 0x05, 0, 0, 0x05, 0x10, 2, 1, 0x03, 0, 0, 0, 0x04, 0x38, 0x01];
        config.resize(24, 0);
        assert_eq!(ep.config, config);
        // BTstack's static endpoint pool (MAX_NR_AVDTP_STREAM_ENDPOINTS)
        assert!(eps.len() <= 11);
    }

    #[test]
    fn aptx_rates() {
        let mut caps = RemoteCaps::default();
        caps.add_vendor(3, &[0x4F, 0, 0, 0, 0x01, 0x00, 0x22]);
        assert_eq!(caps.pick_aptx_sample_rate(CodecKind::Aptx, 48000), 44100);
        assert_eq!(caps.pick_aptx_sample_rate(CodecKind::Aptx, 96000), 44100);
        assert!(caps.configuration(CodecKind::Aptx, 48000, 2, 16).is_err());
    }

    /// The B&W Px7 S2e's aptX Adaptive capabilities (upstream's
    /// spec/px7s2e-a2dp-caps.md): an R1 sink, 48 + 44.1 kHz, stereo + joint
    /// stereo, vendor-tuned target latencies with HQ 0
    const PX7_S2E_APTXAD: [u8; 40] = [
        0xD7, 0, 0, 0, 0xAD, 0, 0x19, 0x0A, 0x37, 0x6A, 0, 0xAF, 0x64, 0x4B, 0, 0, 0xAA, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];

    /// An R2 sink's capabilities with features `features`
    fn r2_aptxad(features: u32) -> Vec<u8> {
        let mut info = PX7_S2E_APTXAD.to_vec();
        info[6] = 0x18 | 0x20;
        info[15] = 1;
        info[16..20].copy_from_slice(&features.to_le_bytes());
        info[20..24].copy_from_slice(&[0x02, 0x03, 0x03, 0x03]);
        info[24..26].copy_from_slice(&[0x00, 0xAA]);
        info
    }

    #[test]
    fn aptx_adaptive_caps_and_configuration() {
        if !CodecKind::AptxAdaptive.available() {
            let mut caps = RemoteCaps::default();
            assert_eq!(caps.add_vendor(9, &PX7_S2E_APTXAD), VendorCapability::AptxAdaptive);
            assert!(caps.aptx_adaptive && !caps.has(CodecKind::AptxAdaptive));
            return;
        }
        // BTstack cuts the 40 bytes to 30: all that is used is there
        for info in [&PX7_S2E_APTXAD[..], &PX7_S2E_APTXAD[..30]] {
            let mut caps = RemoteCaps::default();
            caps.add_sbc(1);
            assert_eq!(caps.add_vendor(9, info), VendorCapability::Taken(CodecKind::AptxAdaptive));
            assert!(caps.aptx_adaptive && caps.has(CodecKind::AptxAdaptive));
            assert!(!caps.aptxad_r2());
            // Auto's choice above SBC (issue #86)
            assert_eq!(caps.best(), Some(CodecKind::AptxAdaptive));
            // Upstream's verified configuration for the Px7 S2e: 48 kHz from
            // a host encoder (0x12), stereo, the sink's latencies with LL0
            // raised to 80 ms and the zero HQ byte to 200 ms
            let c = caps.configuration(CodecKind::AptxAdaptive, 48000, 2, 24).unwrap();
            assert_eq!(c.remote_seid, 9);
            let mut want = vec![0xD7, 0, 0, 0, 0xAD, 0, 0x12, 0x02, 0x50, 0x6A, 0x64, 0xAF, 0x64, 0x4B, 0, 0, 0xAA];
            want.resize(40, 0);
            assert_eq!(c.request, ConfigRequest::Vendor(want));
            assert_eq!(
                caps.configuration(CodecKind::AptxAdaptive, 44100, 2, 24),
                Err(ConfigError::SampleRate(CodecKind::AptxAdaptive, 44100))
            );
        }

        // R2 sinks get the source's own R2 block, whatever their features
        for features in [0x0F00_0000, 0x0F00_0003, 0x0F00_0017] {
            let mut caps = RemoteCaps::default();
            caps.add_vendor(4, &r2_aptxad(features)[..30]);
            assert!(caps.aptxad_r2());
            let ConfigRequest::Vendor(info) =
                caps.configuration(CodecKind::AptxAdaptive, 48000, 2, 24).unwrap().request
            else {
                panic!("not a vendor configuration")
            };
            assert_eq!(info.len(), 40);
            assert_eq!(info[6..8], [0x12, 0x02]);
            assert_eq!(info[8..26], APTXAD_R2_DATA);
            assert!(info[26..].iter().all(|&b| b == 0));
        }

        // 96 kHz: an R2 sink that lists it, asked for by the profile, high
        // quality; 0xA2 in the configuration (both rate bits, as a
        // Snapdragon phone sends it). Else 48 kHz.
        let mut r2 = RemoteCaps::default();
        r2.add_vendor(4, &r2_aptxad(0x0F00_0000)[..30]);
        assert!(r2.aptxad_96k());
        assert_eq!(r2.pick_aptxad_sample_rate(96000, Quality::High), 96000);
        assert_eq!(r2.pick_aptxad_sample_rate(96000, Quality::Mobile), 96000);
        assert_eq!(r2.pick_aptxad_sample_rate(96000, Quality::LowLatency), 48000);
        for wanted in [44100, 48000, 88200, 192000] {
            assert_eq!(r2.pick_aptxad_sample_rate(wanted, Quality::High), 48000, "{wanted}");
        }
        let ConfigRequest::Vendor(info) = r2.configuration(CodecKind::AptxAdaptive, 96000, 2, 24).unwrap().request
        else {
            panic!("not a vendor configuration")
        };
        assert_eq!(info[6..8], [0xA2, 0x02]);
        assert_eq!(info[8..26], APTXAD_R2_DATA);
        // Real sinks (issue #86): an EarFun Air Pro 4+ lists 0xD1 (0x80
        // only), a MOMENTUM 5 0x71 (0x20 only): both take 96 kHz, configured
        // 0xA2 either way, as Android does
        for byte6 in [0xD1, 0x71, 0xB1] {
            let mut info = r2_aptxad(0x0F00_0087);
            info[6] = byte6;
            let mut caps = RemoteCaps::default();
            caps.add_vendor(4, &info[..30]);
            assert!(caps.aptxad_96k(), "{byte6:#x}");
            assert_eq!(caps.listed_sample_rates(CodecKind::AptxAdaptive), [48000, 96000], "{byte6:#x}");
            assert_eq!(caps.aptxad_no_96k(Quality::High), None, "{byte6:#x}");
            assert_eq!(caps.pick_aptxad_sample_rate(96000, Quality::High), 96000, "{byte6:#x}");
            let ConfigRequest::Vendor(c) = caps.configuration(CodecKind::AptxAdaptive, 96000, 2, 24).unwrap().request
            else {
                panic!("not a vendor configuration")
            };
            assert_eq!(c[6..8], [0xA2, 0x02], "{byte6:#x}");
        }
        // An R2 sink without 96 kHz (a Bose QC Ultra's 0x51: 44.1 and 48 kHz
        // only), an R1 sink that lists it anyway: 48 kHz
        let mut no96 = r2_aptxad(0x0F00_0000);
        no96[6] = 0x51;
        let mut r1_96 = PX7_S2E_APTXAD;
        r1_96[6] |= 0xA0;
        assert_eq!(r2.aptxad_no_96k(Quality::High), None);
        assert!(r2.aptxad_no_96k(Quality::LowLatency).is_some_and(|w| w.contains("low latency")));
        for (info, why) in [(&no96[..30], "does not list"), (&r1_96[..], "R1")] {
            let mut caps = RemoteCaps::default();
            caps.add_vendor(4, info);
            assert!(!caps.aptxad_96k());
            // Not offered in the codec's settings either
            assert_eq!(caps.listed_sample_rates(CodecKind::AptxAdaptive), [48000], "{why}");
            assert!(caps.aptxad_no_96k(Quality::High).is_some_and(|w| w.contains(why)), "{why}");
            assert_eq!(caps.pick_aptxad_sample_rate(96000, Quality::High), 48000);
            assert_eq!(
                caps.configuration(CodecKind::AptxAdaptive, 96000, 2, 24),
                Err(ConfigError::SampleRate(CodecKind::AptxAdaptive, 96000))
            );
        }

        // Joint stereo only: configured as such
        let mut joint = RemoteCaps::default();
        let mut info = PX7_S2E_APTXAD;
        info[7] = 0x08;
        joint.add_vendor(2, &info);
        let ConfigRequest::Vendor(c) = joint.configuration(CodecKind::AptxAdaptive, 48000, 2, 24).unwrap().request
        else {
            panic!("not a vendor configuration")
        };
        assert_eq!(c[7], 0x08);

        // Unusable: no 48 kHz, mono / TWS only, shorter than the R1 layout
        let mut no48 = PX7_S2E_APTXAD;
        no48[6] = 0x09;
        let mut mono = PX7_S2E_APTXAD;
        mono[7] = 0x01;
        for info in [&no48[..], &mono[..], &PX7_S2E_APTXAD[..16]] {
            let mut c = RemoteCaps::default();
            assert!(
                matches!(c.add_vendor(3, info), VendorCapability::Ignored(CodecKind::AptxAdaptive, _)),
                "{info:02X?}"
            );
            assert!(c.aptx_adaptive && !c.has(CodecKind::AptxAdaptive));
        }

        // aptX asked for, aptX Adaptive the only aptX: said so
        let mut only = RemoteCaps::default();
        only.add_sbc(1);
        only.add_vendor(9, &PX7_S2E_APTXAD);
        assert_eq!(
            choose(&only, Some(CodecKind::Aptx), false, true),
            Err(Some((CodecKind::Aptx, Unavailable::AdaptiveOnly)))
        );
        assert_eq!(choose(&only, Some(CodecKind::AptxAdaptive), false, true).unwrap().kind, CodecKind::AptxAdaptive);
        assert_eq!(CodecKind::from_key("aptxad"), Some(CodecKind::AptxAdaptive));
    }

    #[test]
    fn experimental_codecs_off_are_never_used() {
        use CodecKind::*;
        let experimental: Vec<_> = CodecKind::ALL.into_iter().filter(|k| k.experimental()).collect();
        assert_eq!(experimental, [AptxHd, AptxLl, Opus, Opus05, FastStream]);
        if !Opus.available() {
            return;
        }
        // Opus (experimental) above AAC and SBC
        let mut caps = RemoteCaps::default();
        caps.add_sbc(1);
        caps.add_aac(2);
        caps.add_vendor(5, &[0xE0, 0, 0, 0, 0x01, 0x00, 0x92]);
        let fallback = if Aac.available() { Aac } else { Sbc };
        // Auto: Opus while on, the next one down while off
        assert_eq!(choose(&caps, None, true, true).unwrap().kind, Opus);
        assert_eq!(choose(&caps, None, true, false).unwrap(), Choice { kind: fallback, replaced: None });
        // Chosen: falls back, saying why; an error without fallback
        assert_eq!(
            choose(&caps, Some(Opus), true, false).unwrap(),
            Choice { kind: fallback, replaced: Some((Opus, Unavailable::Experimental)) }
        );
        assert_eq!(choose(&caps, Some(Opus), false, false), Err(Some((Opus, Unavailable::Experimental))));
        // Never a fallback for another codec either (LDAC not offered)
        assert_eq!(choose(&caps, Some(Ldac), true, true).unwrap().kind, Opus);
        assert_eq!(choose(&caps, Some(Ldac), true, false).unwrap().kind, fallback);
        // What the device offers is recorded as it is
        assert!(caps.has(Opus));
    }

    #[test]
    fn aptx_adaptive_local_endpoint() {
        let eps = local_endpoints();
        let ep = eps.iter().find(|e| e.kind == CodecKind::AptxAdaptive);
        if !CodecKind::AptxAdaptive.available() {
            assert!(ep.is_none());
            return;
        }
        let ep = ep.unwrap();
        assert_eq!(ep.codec_type, CODEC_TYPE_VENDOR);
        // 48 kHz host source, stereo + joint stereo, the R1 block
        let mut caps = vec![0xD7, 0, 0, 0, 0xAD, 0, 0x12, 0x0A, 0x50, 0x64, 0x64, 0x64, 0xFF, 0xFF, 0, 0, 0xAA];
        caps.resize(40, 0);
        assert_eq!(ep.caps, caps);
        assert_eq!(ep.config[6..8], [0x12, 0x02]);
        assert_eq!(ep.config.len(), 40);
        // BTstack's endpoint pool and the shim's buffers (btstack_config.h,
        // shim.c) hold them all
        assert!(eps.len() <= 11);
        assert!(eps.iter().all(|e| e.caps.len() <= 40 && e.config.len() <= 40));
    }
}
