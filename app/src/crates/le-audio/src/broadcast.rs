//! Broadcast source (Auracast, developer mode): what A2DPWB advertises and
//! how the BIG is set up, as plain data.
//!
//! A broadcast is one BIG of one BIS per channel (stereo: Front Left and
//! Front Right, which a pair of earbuds picks from; mono: one BIS without a
//! location), announced by
//! - extended advertising ([`adv_data`]): Broadcast Audio Announcement
//!   (service data 0x1852, Broadcast_ID), Public Broadcast Announcement
//!   (service data 0x1856, PBP features: encrypted, standard / high
//!   quality) and the Broadcast Name (AD type 0x30), and
//! - periodic advertising ([`base_ad`]): the Basic Audio Announcement
//!   (service data 0x1851) with the BASE: presentation delay, one subgroup
//!   (LC3, sampling frequency, frame duration, octets per frame; Media in
//!   the metadata) and the BIS with their channel allocation.
//!
//! The configurations are BAP's broadcast "high reliability" QoS settings
//! (BAP 1.0.1 Table 6.4, written from memory: RTN 4, max transport latency
//! 60 ms for 16 / 24 / 32 kHz and 65 ms for 48 kHz, 10 ms frames, unframed,
//! presentation delay 40 ms). PBP's standard quality is 16_2 / 24_2, high
//! quality 48 kHz.
//!
//! The stream reuses the unicast [`StreamPlan`] (one "CIS" entry per BIS, no
//! device), so that the encode thread and the ISO sender serve both.

use crate::defs::{
    CFG_CHANNEL_ALLOCATION, CFG_FRAME_DURATION, CFG_OCTETS_PER_FRAME, CFG_SAMPLING_FREQUENCY, CODING_FORMAT_LC3,
    CONTEXT_MEDIA, LOCATION_FRONT_LEFT, LOCATION_FRONT_RIGHT, LOCATION_MONO, META_STREAMING_CONTEXTS, PHY_2M,
    TARGET_LATENCY_HIGH_RELIABILITY, sf_hz_to_code,
};
use crate::ltv;
use crate::plan::{ChannelPlan, ChannelSource, CisPlan, StreamPlan};
use crate::presets::StreamPreset;

/// Basic Audio Announcement Service (the BASE, periodic advertising)
pub const UUID_BASIC_AUDIO_ANNOUNCEMENT: u16 = 0x1851;
/// Broadcast Audio Announcement Service (Broadcast_ID)
pub const UUID_BROADCAST_AUDIO_ANNOUNCEMENT: u16 = 0x1852;
/// Public Broadcast Announcement (PBP)
pub const UUID_PUBLIC_BROADCAST_ANNOUNCEMENT: u16 = 0x1856;
/// AD type Broadcast Name
pub const AD_BROADCAST_NAME: u8 = 0x30;
/// AD type Service Data - 16-bit UUID
const AD_SERVICE_DATA_UUID16: u8 = 0x16;

/// PBP features: the broadcast is encrypted
pub const PBP_ENCRYPTED: u8 = 0x01;
/// PBP features: Standard Quality Public Broadcast Audio (16_2 / 24_2)
pub const PBP_STANDARD_QUALITY: u8 = 0x02;
/// PBP features: High Quality Public Broadcast Audio (48 kHz)
pub const PBP_HIGH_QUALITY: u8 = 0x04;

/// The default configuration ("high quality")
pub const DEFAULT_PRESET: &str = "48_4_2";
/// PBP standard quality
pub const STANDARD_PRESET: &str = "24_2_2";

/// Broadcast Name: 4 to 32 octets of UTF-8 (PBP)
pub const NAME_MIN: usize = 4;
pub const NAME_MAX: usize = 32;
/// Broadcast_Code given as a string: 4 to 16 octets of UTF-8
pub const CODE_MIN: usize = 4;
pub const CODE_MAX: usize = 16;

/// The BIG handle of the broadcast
pub const BIG_HANDLE: u8 = 0;
/// Advertising SID of the broadcast's advertising set
pub const ADV_SID: u8 = 1;
/// Extended advertising interval, 0.625 ms units (100-150 ms)
pub const ADV_INTERVAL: (u16, u16) = (0x00A0, 0x00F0);
/// Periodic advertising interval, 1.25 ms units (100-150 ms)
pub const PA_INTERVAL: (u16, u16) = (0x0050, 0x0078);

fn make(name: &str, hz: u32, octets: u16, latency_ms: u16) -> StreamPreset {
    StreamPreset {
        name: name.into(),
        sampling_hz: hz,
        frame_us: 10000,
        octets,
        target_latency: TARGET_LATENCY_HIGH_RELIABILITY,
        framing: 0,
        rtn: 4,
        max_transport_latency_ms: latency_ms,
        presentation_delay_us: 40000,
    }
}

/// The broadcast configurations by name (BAP 1.0.1 Table 6.4, high
/// reliability, 10 ms frames)
pub fn presets() -> Vec<StreamPreset> {
    vec![
        make("16_2_2", 16000, 40, 60),
        make("24_2_2", 24000, 60, 60),
        make("32_2_2", 32000, 80, 60),
        make("48_2_2", 48000, 100, 65),
        make("48_4_2", 48000, 120, 65),
        make("48_6_2", 48000, 155, 65),
    ]
}

pub fn find_preset(name: &str) -> Option<StreamPreset> {
    presets().into_iter().find(|p| p.name == name)
}

/// What the user chose for a broadcast
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BroadcastConfig {
    /// Broadcast Name, [`NAME_MIN`]..=[`NAME_MAX`] octets ([`check_name`])
    pub name: String,
    /// Broadcast_Code ([`broadcast_code`]); None: open
    pub code: Option<[u8; 16]>,
    pub preset: StreamPreset,
    /// Two BIS (Front Left / Right), else one mono BIS
    pub stereo: bool,
    /// 24 bits, random per broadcast
    pub broadcast_id: u32,
}

/// The Broadcast Name as advertised: trimmed, at most [`NAME_MAX`] octets
/// (cut at a character boundary). Err when shorter than [`NAME_MIN`].
pub fn check_name(name: &str) -> Result<String, String> {
    let mut s = name.trim().to_string();
    while s.len() > NAME_MAX {
        s.pop();
    }
    if s.len() < NAME_MIN {
        return Err(format!("the broadcast name needs {NAME_MIN} to {NAME_MAX} bytes"));
    }
    Ok(s)
}

/// The Broadcast_Code of a code string: its UTF-8 octets, first character
/// first, zero padded to 16 (as the HCI parameter LE Create BIG takes).
/// Err when not [`CODE_MIN`]..=[`CODE_MAX`] octets.
pub fn broadcast_code(code: &str) -> Result<[u8; 16], String> {
    let b = code.as_bytes();
    if !(CODE_MIN..=CODE_MAX).contains(&b.len()) {
        return Err(format!("the broadcast code needs {CODE_MIN} to {CODE_MAX} bytes"));
    }
    let mut out = [0u8; 16];
    out[..b.len()].copy_from_slice(b);
    Ok(out)
}

/// A Broadcast_ID from any random value: 24 bits, never 0 or 0xFFFFFF
pub fn broadcast_id(random: u64) -> u32 {
    let id = (random ^ (random >> 24) ^ (random >> 48)) as u32 & 0x00FF_FFFF;
    match id {
        0 => 1,
        0x00FF_FFFF => 0x00FF_FFFE,
        id => id,
    }
}

/// PBP features of the configuration
pub fn pbp_features(config: &BroadcastConfig) -> u8 {
    let mut f = 0;
    if config.code.is_some() {
        f |= PBP_ENCRYPTED;
    }
    match config.preset.sampling_hz {
        16000 | 24000 => f |= PBP_STANDARD_QUALITY,
        48000 => f |= PBP_HIGH_QUALITY,
        _ => {}
    }
    f
}

fn put_ad(out: &mut Vec<u8>, ty: u8, value: &[u8]) {
    out.push(value.len() as u8 + 1);
    out.push(ty);
    out.extend_from_slice(value);
}

/// The extended advertising data: Broadcast Audio Announcement, Public
/// Broadcast Announcement (no metadata), Broadcast Name
pub fn adv_data(config: &BroadcastConfig) -> Vec<u8> {
    let mut out = Vec::new();
    let id = config.broadcast_id.to_le_bytes();
    let mut v = UUID_BROADCAST_AUDIO_ANNOUNCEMENT.to_le_bytes().to_vec();
    v.extend_from_slice(&id[..3]);
    put_ad(&mut out, AD_SERVICE_DATA_UUID16, &v);
    let mut v = UUID_PUBLIC_BROADCAST_ANNOUNCEMENT.to_le_bytes().to_vec();
    v.extend_from_slice(&[pbp_features(config), 0]);
    put_ad(&mut out, AD_SERVICE_DATA_UUID16, &v);
    put_ad(&mut out, AD_BROADCAST_NAME, config.name.as_bytes());
    out
}

/// The stream plan of a broadcast: one entry per BIS (BIS index =
/// `cis_id`, from 1), Front Left / Right from the left / right audio, or
/// one mono BIS of the downmix
pub fn plan(config: &BroadcastConfig) -> StreamPlan {
    let bis = |index: u8, allocation: u32, source: ChannelSource| CisPlan {
        device_id: 0,
        ase_id: 0,
        cis_id: index,
        acl: 0xFFFF,
        cis_handle: 0xFFFF,
        channels: vec![ChannelPlan { allocation, source }],
    };
    let cis = if config.stereo {
        vec![bis(1, LOCATION_FRONT_LEFT, ChannelSource::Left), bis(2, LOCATION_FRONT_RIGHT, ChannelSource::Right)]
    } else {
        vec![bis(1, LOCATION_MONO, ChannelSource::Mono)]
    };
    StreamPlan {
        preset: config.preset.clone(),
        cig_id: BIG_HANDLE,
        phy: PHY_2M,
        requested_contexts: CONTEXT_MEDIA,
        contexts: CONTEXT_MEDIA,
        presentation_delay_us: config.preset.presentation_delay_us,
        cis,
    }
}

/// One line for the log and the UI, e.g. "LC3 48 kHz 10 ms 120 octets (96
/// kbps/ch), 2 ch over 2 BIS, 48_4_2"
pub fn describe(plan: &StreamPlan) -> String {
    plan.describe().replace(" CIS, ", " BIS, ")
}

/// The BASE (BAP 1.0.1 3.7.2.2): presentation delay, one subgroup of LC3
/// with the plan's codec configuration and Media as Streaming Audio
/// Contexts, then each BIS with its channel allocation (none for mono)
pub fn base(plan: &StreamPlan) -> Vec<u8> {
    let p = &plan.preset;
    let mut out = Vec::new();
    out.extend_from_slice(&plan.presentation_delay_us.to_le_bytes()[..3]);
    out.push(1); // subgroups
    out.push(plan.cis.len() as u8);
    out.extend_from_slice(&[CODING_FORMAT_LC3, 0, 0, 0, 0]);
    let mut config = Vec::new();
    ltv::put(&mut config, CFG_SAMPLING_FREQUENCY, &[sf_hz_to_code(p.sampling_hz)]);
    ltv::put(&mut config, CFG_FRAME_DURATION, &[if p.frame_us == 7500 { 0x00 } else { 0x01 }]);
    ltv::put(&mut config, CFG_OCTETS_PER_FRAME, &p.octets.to_le_bytes());
    out.push(config.len() as u8);
    out.extend_from_slice(&config);
    let mut metadata = Vec::new();
    ltv::put(&mut metadata, META_STREAMING_CONTEXTS, &plan.contexts.to_le_bytes());
    out.push(metadata.len() as u8);
    out.extend_from_slice(&metadata);
    for bis in &plan.cis {
        out.push(bis.cis_id);
        let mut config = Vec::new();
        let allocation = bis.allocation();
        if allocation != 0 {
            ltv::put(&mut config, CFG_CHANNEL_ALLOCATION, &allocation.to_le_bytes());
        }
        out.push(config.len() as u8);
        out.extend_from_slice(&config);
    }
    out
}

/// The periodic advertising data: the BASE as Basic Audio Announcement
/// service data
pub fn base_ad(plan: &StreamPlan) -> Vec<u8> {
    let mut v = UUID_BASIC_AUDIO_ANNOUNCEMENT.to_le_bytes().to_vec();
    v.extend_from_slice(&base(plan));
    let mut out = Vec::new();
    put_ad(&mut out, AD_SERVICE_DATA_UUID16, &v);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(stereo: bool, preset: &str, code: Option<&str>) -> BroadcastConfig {
        BroadcastConfig {
            name: "A2DPWB test".into(),
            code: code.map(|c| broadcast_code(c).unwrap()),
            preset: find_preset(preset).unwrap(),
            stereo,
            broadcast_id: 0x123456,
        }
    }

    #[test]
    fn names_and_codes() {
        assert_eq!(check_name("  Living room  ").unwrap(), "Living room");
        assert!(check_name("abc").is_err());
        // Cut to 32 octets at a character boundary (3 octets each)
        let long = "あ".repeat(20);
        let cut = check_name(&long).unwrap();
        assert_eq!(cut.len(), 30);
        assert!(long.starts_with(&cut));

        let c = broadcast_code("1234abcd").unwrap();
        assert_eq!(&c[..8], b"1234abcd");
        assert!(c[8..].iter().all(|&b| b == 0));
        assert!(broadcast_code("123").is_err());
        assert!(broadcast_code("12345678901234567").is_err());
        assert!(broadcast_code("1234567890123456").is_ok());

        assert_eq!(broadcast_id(0), 1);
        assert!(broadcast_id(u64::MAX) <= 0xFFFFFE);
        assert_eq!(broadcast_id(0x42), 0x42);
    }

    #[test]
    fn announcements() {
        let c = config(true, DEFAULT_PRESET, None);
        let adv = adv_data(&c);
        assert_eq!(
            adv[..14],
            [
                6,
                0x16,
                0x52,
                0x18,
                0x56,
                0x34,
                0x12, // Broadcast_ID
                5,
                0x16,
                0x56,
                0x18,
                PBP_HIGH_QUALITY,
                0, // PBP, no metadata
                12
            ]
        );
        assert_eq!(adv[14], AD_BROADCAST_NAME);
        assert_eq!(&adv[15..], b"A2DPWB test");

        let c = config(false, STANDARD_PRESET, Some("1234"));
        assert_eq!(adv_data(&c)[11], PBP_ENCRYPTED | PBP_STANDARD_QUALITY);
        assert_eq!(pbp_features(&config(true, "32_2_2", None)), 0);
    }

    #[test]
    fn base_of_stereo_and_mono() {
        let plan = plan(&config(true, DEFAULT_PRESET, None));
        assert_eq!(describe(&plan), "LC3 48 kHz 10 ms 120 octets (96 kbps/ch), 2 ch over 2 BIS, 48_4_2");
        assert_eq!(plan.cis.iter().map(|b| b.cis_id).collect::<Vec<_>>(), [1, 2]);
        let base = base(&plan);
        #[rustfmt::skip]
        assert_eq!(base, [
            0x40, 0x9C, 0x00,             // 40 ms
            1,                            // subgroups
            2, 0x06, 0, 0, 0, 0,          // BIS, LC3
            10, 2, 1, 8, 2, 2, 1, 3, 4, 120, 0,
            4, 3, 2, 4, 0,                // Streaming Audio Contexts: Media
            1, 6, 5, 3, 1, 0, 0, 0,       // BIS 1: Front Left
            2, 6, 5, 3, 2, 0, 0, 0,       // BIS 2: Front Right
        ]);
        let ad = base_ad(&plan);
        assert_eq!(ad[0] as usize, ad.len() - 1);
        assert_eq!(ad[1..4], [0x16, 0x51, 0x18]);
        assert_eq!(&ad[4..], &base[..]);

        let mono = super::plan(&config(false, "16_2_2", None));
        assert_eq!(mono.cis.len(), 1);
        assert_eq!(mono.cis[0].channels[0].source, ChannelSource::Mono);
        let base = super::base(&mono);
        assert_eq!(base[4], 1);
        assert_eq!(base[base.len() - 2..], [1, 0]); // BIS 1, no allocation
    }

    #[test]
    fn presets_fit_one_iso_packet() {
        for p in presets() {
            assert_eq!(p.frame_us, 10000);
            assert_eq!(p.rtn, 4);
            assert!(p.octets <= 155);
        }
        assert!(find_preset(DEFAULT_PRESET).is_some());
        assert!(find_preset(STANDARD_PRESET).is_some());
        assert!(find_preset("48_4_1").is_none());
    }
}
