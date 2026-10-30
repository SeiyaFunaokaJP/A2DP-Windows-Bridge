//! profiles.json in v1's format: {"profiles": [ ... ]}. Unknown keys of a
//! profile are kept.
//!
//! Since the Codecs screen (issue #86) the UI keeps one profile per device
//! entry (address, and whether it is the LE Audio entry): choosing a codec
//! card changes that profile. The settings of the codecs it is not using
//! are kept in its `codec_settings`; [`consolidate`] folds the profiles of
//! earlier versions into one per device.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use bt_host::media::clamp_media_payload_limit;
use codec::{CodecKind, Quality};

use crate::stream::{SourceRequest, StreamRequest, VolumeSync};

fn auto() -> String {
    "auto".into()
}
fn hq() -> String {
    "hq".into()
}
fn loopback() -> String {
    "loopback".into()
}
fn yes() -> bool {
    true
}
fn is_true(b: &bool) -> bool {
    *b
}
fn is_zero(n: &u8) -> bool {
    *n == 0
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Profile {
    pub name: String,
    /// XX:XX:XX:XX:XX:XX
    pub device_address: String,
    pub device_name: String,
    /// auto, ldac, aptxhd, aptxll, aptx, aptxad, sbcxq, sbc, aac, lhdcv5, opus, opus05, faststream
    #[serde(default = "auto")]
    pub codec: String,
    /// hq, sq, mq; mhq for LHDC V5 only; ll for aptX Adaptive only
    #[serde(default = "hq")]
    pub quality: String,
    pub abr: bool,
    /// Legacy (v1, and v2 before issue #86's Codecs redesign): whether a
    /// codec the device does not offer falls back to another. Kept in the
    /// file but no longer used: a profile always falls back, to the first
    /// codec in its order the device offers (an unsupported codec is never
    /// tried)
    #[serde(default = "yes")]
    pub codec_fallback: bool,
    /// 0 = auto
    pub sample_rate: u32,
    /// 0 = auto, 16, 24
    pub bit_depth: u32,
    /// loopback, virtual, app
    #[serde(default = "loopback")]
    pub capture_mode: String,
    pub audio_device_id: String,
    pub audio_device_name: String,
    /// Virtual device mode: make the device the default output while
    /// streaming
    #[serde(default = "yes")]
    pub auto_switch_device: bool,
    pub app_exe: String,
    pub app_name: String,
    /// auto, off, device. Kept as saved, but streams follow
    /// Settings::volume_sync alone (schema 6; issue #86: no screen sets a
    /// followed device any more): off, none syncs; on, Auto (the core's
    /// state.rs)
    #[serde(default = "auto")]
    pub volume_sync: String,
    pub volume_device_id: String,
    pub volume_device_name: String,
    /// Max media packet size (None in files from before v1.1: the legacy
    /// setting applies)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_media_payload: Option<u16>,
    /// SBC / SBC XQ: the bitpool to encode with, within what the device
    /// allows (0 = by quality)
    #[serde(skip_serializing_if = "is_zero")]
    pub sbc_bitpool: u8,
    /// The same for SBC XQ (kept apart: XQ is dual channel and goes lower)
    #[serde(skip_serializing_if = "is_zero")]
    pub sbcxq_bitpool: u8,
    /// SBC / SBC XQ: the lowest bitpool to use, the SET_CONFIGURATION's
    /// minimum and the floor of ABR, within what the device allows (0 =
    /// Auto, issue #86)
    #[serde(skip_serializing_if = "is_zero")]
    pub sbc_min_bitpool: u8,
    /// The same for SBC XQ
    #[serde(skip_serializing_if = "is_zero")]
    pub sbcxq_min_bitpool: u8,
    /// SBC / SBC XQ: block length 4, 8, 12, 16 (0 = 16)
    #[serde(skip_serializing_if = "is_zero")]
    pub sbc_blocks: u8,
    /// SBC / SBC XQ: subbands 4, 8 (0 = 8)
    #[serde(skip_serializing_if = "is_zero")]
    pub sbc_subbands: u8,
    /// SBC / SBC XQ: loudness, snr ("" = loudness)
    #[serde(skip_serializing_if = "String::is_empty")]
    pub sbc_allocation: String,
    /// SBC: joint_stereo, stereo, mono ("" = joint stereo; dual channel is
    /// SBC XQ's, which is always dual channel)
    #[serde(skip_serializing_if = "String::is_empty")]
    pub sbc_channel_mode: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub le_audio: bool,
    /// LE Audio: GMAP low-latency presets with a gaming headset. On by
    /// default (written only when off): other headsets get the media
    /// configuration either way.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub le_low_latency: bool,
    /// The settings of the other codecs, by codec key, as they were last
    /// chosen: choosing one of them again brings them back (the codec in
    /// use has its own in the fields above)
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub codec_settings: BTreeMap<String, CodecValues>,
    /// The device's codec order (codec keys), as its cards were dragged on
    /// the Codecs screen with Auto-select off (issue #86 round 8): Auto, and
    /// a chosen codec the device does not offer, take the first one in it
    /// the device offers. Empty: highest sound quality first. Under
    /// Auto-select it is kept but not used ([`Profile::as_auto`]).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub codec_order: Vec<String>,
    #[serde(flatten)]
    pub other: BTreeMap<String, serde_json::Value>,
}

impl Default for Profile {
    fn default() -> Self {
        Profile {
            name: String::new(),
            device_address: String::new(),
            device_name: String::new(),
            codec: auto(),
            quality: hq(),
            abr: false,
            codec_fallback: true,
            sample_rate: 0,
            bit_depth: 0,
            capture_mode: loopback(),
            audio_device_id: String::new(),
            audio_device_name: String::new(),
            auto_switch_device: true,
            app_exe: String::new(),
            app_name: String::new(),
            volume_sync: auto(),
            volume_device_id: String::new(),
            volume_device_name: String::new(),
            max_media_payload: None,
            sbc_bitpool: 0,
            sbcxq_bitpool: 0,
            sbc_min_bitpool: 0,
            sbcxq_min_bitpool: 0,
            sbc_blocks: 0,
            sbc_subbands: 0,
            sbc_allocation: String::new(),
            sbc_channel_mode: String::new(),
            le_audio: false,
            le_low_latency: true,
            codec_settings: BTreeMap::new(),
            codec_order: Vec::new(),
            other: BTreeMap::new(),
        }
    }
}

/// One codec's settings in a profile (what its "..." on the Codecs screen
/// sets)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct CodecValues {
    pub quality: String,
    pub sample_rate: u32,
    pub bit_depth: u32,
    pub abr: bool,
    #[serde(skip_serializing_if = "is_zero")]
    pub sbc_bitpool: u8,
    #[serde(skip_serializing_if = "is_zero")]
    pub sbcxq_bitpool: u8,
    #[serde(skip_serializing_if = "is_zero")]
    pub sbc_min_bitpool: u8,
    #[serde(skip_serializing_if = "is_zero")]
    pub sbcxq_min_bitpool: u8,
    #[serde(skip_serializing_if = "is_zero")]
    pub sbc_blocks: u8,
    #[serde(skip_serializing_if = "is_zero")]
    pub sbc_subbands: u8,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub sbc_allocation: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub sbc_channel_mode: String,
}

impl Default for CodecValues {
    fn default() -> Self {
        CodecValues {
            quality: hq(),
            sample_rate: 0,
            bit_depth: 0,
            abr: false,
            sbc_bitpool: 0,
            sbcxq_bitpool: 0,
            sbc_min_bitpool: 0,
            sbcxq_min_bitpool: 0,
            sbc_blocks: 0,
            sbc_subbands: 0,
            sbc_allocation: String::new(),
            sbc_channel_mode: String::new(),
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProfileError {
    #[error("invalid Bluetooth address \"{0}\"")]
    Address(String),
}

impl Profile {
    /// The settings of the codec in use
    pub fn codec_values(&self) -> CodecValues {
        CodecValues {
            quality: self.quality.clone(),
            sample_rate: self.sample_rate,
            bit_depth: self.bit_depth,
            abr: self.abr,
            sbc_bitpool: self.sbc_bitpool,
            sbcxq_bitpool: self.sbcxq_bitpool,
            sbc_min_bitpool: self.sbc_min_bitpool,
            sbcxq_min_bitpool: self.sbcxq_min_bitpool,
            sbc_blocks: self.sbc_blocks,
            sbc_subbands: self.sbc_subbands,
            sbc_allocation: self.sbc_allocation.clone(),
            sbc_channel_mode: self.sbc_channel_mode.clone(),
        }
    }

    /// The profile as Auto streams it while "Auto-select" is on for every
    /// device (settings `auto_codec`): codec "auto" with Auto's default
    /// settings, as turning Auto on for one device gave before. The codec
    /// chosen and its settings stay in the saved profile. An LE Audio
    /// profile (LC3) is returned as it is.
    pub fn as_auto(&self) -> Profile {
        if self.le_audio {
            return self.clone();
        }
        let d = CodecValues::default();
        Profile {
            codec: auto(),
            quality: d.quality,
            sample_rate: d.sample_rate,
            bit_depth: d.bit_depth,
            abr: d.abr,
            sbc_bitpool: d.sbc_bitpool,
            sbcxq_bitpool: d.sbcxq_bitpool,
            sbc_min_bitpool: d.sbc_min_bitpool,
            sbcxq_min_bitpool: d.sbcxq_min_bitpool,
            sbc_blocks: d.sbc_blocks,
            sbc_subbands: d.sbc_subbands,
            sbc_allocation: d.sbc_allocation,
            sbc_channel_mode: d.sbc_channel_mode,
            // Auto-select's order is the default one
            codec_order: Vec::new(),
            ..self.clone()
        }
    }

    /// The codec order streams use ([`Profile::codec_order`])
    pub fn order(&self) -> codec::AutoOrder {
        codec::AutoOrder::from_keys(&self.codec_order)
    }

    /// What to stream for this profile. An LE Audio (experimental) profile
    /// gives an LE request (`le` Some: the le_stream session, which refuses
    /// with error.le_audio_disabled while the setting is off); its codec,
    /// quality, rate and depth are not used (LC3 as the device and the
    /// preset decide).
    pub fn to_request(&self, legacy_max_media_payload: u16) -> Result<StreamRequest, ProfileError> {
        let device = bt_host::parse_addr(&self.device_address)
            .ok_or_else(|| ProfileError::Address(self.device_address.clone()))?;
        let source = match self.capture_mode.as_str() {
            "virtual" => SourceRequest::VirtualDevice {
                id: self.audio_device_id.clone(),
                name: self.audio_device_name.clone(),
                auto_switch: self.auto_switch_device,
            },
            "app" => SourceRequest::Application { exe: self.app_exe.clone() },
            // "loopback", and v1's legacy "apo"
            _ => SourceRequest::SystemLoopback,
        };
        let volume_sync = match self.volume_sync.as_str() {
            "off" => VolumeSync::Off,
            "device" => VolumeSync::Device(self.volume_device_id.clone()),
            _ => VolumeSync::Auto,
        };
        tracing::debug!(
            "profile \"{}\": codec {:?} quality {:?} rate {} depth {} capture {:?} volume sync {:?} payload {:?} (legacy {legacy_max_media_payload}) LE {}",
            self.name,
            self.codec,
            self.quality,
            self.sample_rate,
            self.bit_depth,
            self.capture_mode,
            self.volume_sync,
            self.max_media_payload,
            self.le_audio
        );
        Ok(StreamRequest {
            device,
            device_name: self.device_name.clone(),
            codec: CodecKind::from_key(&self.codec),
            codec_fallback: true,
            // The setting's, filled in by whoever starts the stream
            experimental_codecs: true,
            auto_order: self.order(),
            quality: Quality::from_key(&self.quality),
            abr: self.abr,
            sbc_bitpool: self.sbc_bitpool,
            sbcxq_bitpool: self.sbcxq_bitpool,
            sbc_min_bitpool: self.sbc_min_bitpool,
            sbcxq_min_bitpool: self.sbcxq_min_bitpool,
            sbc: codec::sbc::SbcParams {
                blocks: self.sbc_blocks,
                subbands: self.sbc_subbands,
                allocation: codec::sbc::Allocation::from_key(&self.sbc_allocation),
                channel_mode: codec::sbc::ChannelMode::from_key(&self.sbc_channel_mode),
                min_bitpool: 0,
            },
            sample_rate: (self.sample_rate != 0).then_some(self.sample_rate),
            bit_depth: match self.bit_depth {
                16 => Some(16),
                24 => Some(24),
                _ => None,
            },
            source,
            volume_sync,
            max_media_payload: clamp_media_payload_limit(
                self.max_media_payload.unwrap_or(legacy_max_media_payload) as u32
            ),
            le: self
                .le_audio
                .then(|| bt_host::LeRequest { low_latency: self.le_low_latency, ..bt_host::LeRequest::default() }),
        })
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ProfilesFile {
    #[serde(default)]
    profiles: Vec<Profile>,
    #[serde(flatten)]
    other: BTreeMap<String, serde_json::Value>,
}

pub fn path(dir: &Path) -> PathBuf {
    crate::paths::profiles_file(dir)
}

/// Gives every profile a name of its own: v1 named profiles "<device>
/// <codec>" and told them apart by position, so two alike had the same
/// name, while v2 finds them by name. A repeated name gets " (2)", " (3)"…
/// (the first keeps its name, so a last_profile naming it still does).
/// Returns how many were renamed.
pub fn make_names_unique(list: &mut [Profile]) -> usize {
    let mut taken: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut renamed = 0;
    for i in 0..list.len() {
        if taken.insert(list[i].name.clone()) {
            continue;
        }
        let base = list[i].name.clone();
        let name = (2..)
            .map(|n| format!("{base} ({n})"))
            // Not one taken already, nor one a later profile has
            .find(|c| !taken.contains(c) && !list[i + 1..].iter().any(|p| &p.name == c))
            .unwrap_or_default();
        taken.insert(name.clone());
        list[i].name = name;
        renamed += 1;
    }
    renamed
}

/// Folds the profiles of a device entry (address, LE Audio or not) into
/// one, as the Codecs screen keeps them: the one `last_profile` names if it
/// is of that device, else the first. A profile folded away gives its codec
/// settings to the kept one's `codec_settings` when the kept one has none
/// for that codec yet (an Auto profile has none of its own); its name,
/// capture source, volume sync and packet size go (the Audio Devices screen
/// already kept those the same for all of a device's profiles). Profiles
/// without a valid address are left as they are. Returns the names of the
/// profiles folded away, in order.
pub fn consolidate(list: &mut Vec<Profile>, last_profile: &str) -> Vec<String> {
    let key = |p: &Profile| bt_host::parse_addr(&p.device_address).map(|a| (a, p.le_audio));
    // The kept profile of each device
    let mut keeper: std::collections::HashMap<_, usize> = std::collections::HashMap::new();
    for (i, p) in list.iter().enumerate() {
        if let Some(k) = key(p) {
            match keeper.get(&k) {
                None => {
                    keeper.insert(k, i);
                }
                Some(&j) if p.name == last_profile && list[j].name != last_profile => {
                    keeper.insert(k, i);
                }
                _ => {}
            }
        }
    }
    let mut folded = Vec::new();
    let mut kept: Vec<Profile> = Vec::with_capacity(list.len());
    // Where each device's profile is in `kept`: the place of its first one
    let mut placed: std::collections::HashMap<_, usize> = std::collections::HashMap::new();
    let old = std::mem::take(list);
    let keepers: Vec<Profile> = old.clone();
    for (i, p) in old.into_iter().enumerate() {
        let Some(k) = key(&p) else {
            kept.push(p);
            continue;
        };
        let at = *placed.entry(k).or_insert_with(|| {
            kept.push(keepers[keeper[&k]].clone());
            kept.len() - 1
        });
        if i == keeper[&k] {
            continue;
        }
        let into = &mut kept[at];
        let gives =
            !p.le_audio && p.codec != "auto" && p.codec != into.codec && !into.codec_settings.contains_key(&p.codec);
        tracing::info!(
            "profiles: \"{}\" folded into \"{}\" (same device){}",
            p.name,
            into.name,
            if gives { format!(", its {} settings kept", p.codec) } else { String::new() }
        );
        if gives {
            into.codec_settings.insert(p.codec.clone(), p.codec_values());
        }
        for (c, v) in p.codec_settings {
            if c != into.codec {
                into.codec_settings.entry(c).or_insert(v);
            }
        }
        folded.push(p.name);
    }
    *list = kept;
    folded
}

/// Plain SBC no longer offers dual channel (issue #86: SBC XQ is SBC's dual
/// channel): a profile that chose it, for the codec in use or kept in
/// `codec_settings`, gets joint stereo (""). Returns how many profiles
/// changed.
pub fn drop_sbc_dual_channel(list: &mut [Profile]) -> usize {
    let dual = |m: &mut String| {
        let was = m == "dual_channel";
        if was {
            m.clear();
        }
        was
    };
    let mut changed = 0;
    for p in list {
        let mut any = dual(&mut p.sbc_channel_mode);
        for v in p.codec_settings.values_mut() {
            any |= dual(&mut v.sbc_channel_mode);
        }
        if any {
            tracing::info!("profiles: \"{}\": SBC dual channel is now SBC XQ's, joint stereo instead", p.name);
            changed += 1;
        }
    }
    changed
}

/// The saved profiles, names made unique and SBC's dual channel dropped in
/// memory (see make_names_unique, drop_sbc_dual_channel; written at the next
/// save)
pub fn load(dir: &Path) -> Vec<Profile> {
    load_counted(dir).0
}

/// [`load`], and how many profiles it renamed
pub fn load_counted(dir: &Path) -> (Vec<Profile>, usize) {
    let (list, renamed, _) = load_fixed(dir);
    (list, renamed)
}

/// [`load`], how many profiles it renamed and how many lost SBC's dual
/// channel
pub fn load_fixed(dir: &Path) -> (Vec<Profile>, usize, usize) {
    let text = match std::fs::read_to_string(path(dir)) {
        Ok(t) => t,
        Err(e) => {
            tracing::debug!("profiles: {} not read ({e}), no profiles", path(dir).display());
            return (Vec::new(), 0, 0);
        }
    };
    match serde_json::from_str::<ProfilesFile>(&text) {
        Ok(mut f) => {
            let renamed = make_names_unique(&mut f.profiles);
            let dual = drop_sbc_dual_channel(&mut f.profiles);
            tracing::debug!(
                "profiles: loaded {} profile(s) from {}, {renamed} renamed, {dual} off SBC dual channel",
                f.profiles.len(),
                path(dir).display()
            );
            (f.profiles, renamed, dual)
        }
        Err(e) => {
            tracing::warn!("profiles: {} is not valid ({e})", path(dir).display());
            (Vec::new(), 0, 0)
        }
    }
}

pub fn save(dir: &Path, profiles: &[Profile]) -> std::io::Result<()> {
    let file = ProfilesFile { profiles: profiles.to_vec(), other: BTreeMap::new() };
    let text = serde_json::to_string_pretty(&file).map_err(std::io::Error::other)?;
    let tmp = path(dir).with_extension("json.tmp");
    std::fs::write(&tmp, text)?;
    let r = std::fs::rename(&tmp, path(dir));
    tracing::debug!("profiles: saved {} profile(s) to {}: {r:?}", profiles.len(), path(dir).display());
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v1_profile() {
        let text = r#"{"profiles":[{"name":"WH","device_address":"00:A2:D0:00:00:02","codec":"ldac","quality":"sq",
            "bit_depth":24,"capture_mode":"virtual","audio_device_id":"{x}","max_media_payload":1023,"extra":1}]}"#;
        let f: ProfilesFile = serde_json::from_str(text).unwrap();
        let p = &f.profiles[0];
        assert_eq!(p.other.get("extra"), Some(&serde_json::json!(1)));
        let r = p.to_request(679).unwrap();
        assert_eq!(r.codec, Some(CodecKind::Ldac));
        assert_eq!(r.quality, Quality::Standard);
        assert_eq!(r.bit_depth, Some(24));
        assert_eq!(r.max_media_payload, 1023);
        assert!(matches!(r.source, SourceRequest::VirtualDevice { auto_switch: true, .. }));
        let mut legacy = p.clone();
        legacy.max_media_payload = None;
        assert_eq!(legacy.to_request(1500).unwrap().max_media_payload, 1500);
        assert_eq!(r.le, None);
    }

    #[test]
    fn a_profile_always_falls_back() {
        // Saved with the old option off (issue #86: e.g. LHDC chosen for
        // headphones without it): the request still falls back, so the
        // codec is never tried on a device that does not offer it
        let text = r#"{"name":"EarFun | LHDC V5","device_address":"00:A2:D0:00:00:02","codec":"lhdcv5","codec_fallback":false}"#;
        let p: Profile = serde_json::from_str(text).unwrap();
        assert!(!p.codec_fallback);
        let r = p.to_request(1023).unwrap();
        assert_eq!(r.codec, Some(CodecKind::LhdcV5));
        assert!(r.codec_fallback);
        // Auto (switch on) is no codec of its own
        let auto = Profile { codec: "auto".into(), ..p };
        assert_eq!(auto.to_request(1023).unwrap().codec, None);
    }

    #[test]
    fn as_auto_streams_auto_with_its_defaults() {
        let p = Profile {
            device_address: "AA:BB:CC:DD:EE:FF".into(),
            codec: "ldac".into(),
            quality: "mq".into(),
            sample_rate: 96000,
            bit_depth: 24,
            sbc_bitpool: 40,
            volume_sync: "off".into(),
            ..Profile::default()
        };
        let a = p.as_auto();
        assert_eq!(a.codec, "auto");
        assert_eq!(a.codec_values(), CodecValues::default());
        // What belongs to the device stays
        assert_eq!(a.volume_sync, "off");
        assert_eq!(a.to_request(1023).unwrap().codec, None);
        let le = Profile { le_audio: true, ..p };
        assert_eq!(le.as_auto(), le);
    }

    #[test]
    fn codec_order_in_the_request_written_only_when_set() {
        let text =
            r#"{"name":"EarFun","device_address":"00:A2:D0:00:00:02","codec":"opus","codec_order":["sbcxq","sbc"]}"#;
        let p: Profile = serde_json::from_str(text).unwrap();
        let order = p.to_request(1023).unwrap().auto_order;
        assert_eq!(&order.codecs()[..2], &[CodecKind::SbcXq, CodecKind::Sbc]);
        assert_eq!(order.codecs().len(), CodecKind::AUTO_PRIORITY.len());
        // Auto-select: the default order
        assert!(p.as_auto().to_request(1023).unwrap().auto_order.is_default());
        assert_eq!(serde_json::to_value(&p).unwrap()["codec_order"], serde_json::json!(["sbcxq", "sbc"]));
        let plain = Profile { codec_order: Vec::new(), ..p };
        assert!(plain.to_request(1023).unwrap().auto_order.is_default());
        assert!(serde_json::to_value(&plain).unwrap().get("codec_order").is_none());
    }

    #[test]
    fn sbc_setup_in_the_request_and_written_only_when_set() {
        use codec::sbc::{Allocation, ChannelMode, SbcParams};
        let text = r#"{"name":"WH","device_address":"00:A2:D0:00:00:02","codec":"sbc","sbc_bitpool":40,
            "sbc_min_bitpool":20,"sbcxq_min_bitpool":30,
            "sbc_blocks":8,"sbc_subbands":4,"sbc_allocation":"snr","sbc_channel_mode":"stereo"}"#;
        let p: Profile = serde_json::from_str(text).unwrap();
        let r = p.to_request(1023).unwrap();
        assert_eq!(r.sbc_bitpool, 40);
        assert_eq!(
            r.sbc,
            SbcParams {
                blocks: 8,
                subbands: 4,
                allocation: Some(Allocation::Snr),
                channel_mode: Some(ChannelMode::Stereo),
                min_bitpool: 0,
            }
        );
        // Each codec its own minimum bitpool
        assert_eq!(r.sbc_params(CodecKind::Sbc), SbcParams { min_bitpool: 20, ..r.sbc });
        assert_eq!(r.sbc_params(CodecKind::SbcXq).min_bitpool, 30);
        assert_eq!(r.sbc_params(CodecKind::Aac).min_bitpool, 0);
        assert_eq!((p.codec_values().sbc_min_bitpool, p.codec_values().sbcxq_min_bitpool), (20, 30));
        // Auto (the defaults) stays out of the file and gives today's setup
        let plain = Profile { device_address: "00:A2:D0:00:00:02".into(), ..Profile::default() };
        let json = serde_json::to_string(&plain).unwrap();
        assert!(!json.contains("sbc_"), "{json}");
        assert!(plain.to_request(1023).unwrap().sbc.is_default());
        // Unknown values are Auto
        let odd = Profile { sbc_allocation: "max".into(), sbc_channel_mode: "auto".into(), ..plain };
        assert!(odd.to_request(1023).unwrap().sbc.is_default());
    }

    fn named(names: &[&str]) -> Vec<Profile> {
        names.iter().map(|n| Profile { name: n.to_string(), ..Profile::default() }).collect()
    }

    fn names(list: &[Profile]) -> Vec<&str> {
        list.iter().map(|p| p.name.as_str()).collect()
    }

    #[test]
    fn repeated_names_get_a_number() {
        let mut list = named(&["WH LDAC", "WH LDAC", "Buds AAC", "WH LDAC"]);
        assert_eq!(make_names_unique(&mut list), 2);
        assert_eq!(names(&list), ["WH LDAC", "WH LDAC (2)", "Buds AAC", "WH LDAC (3)"]);
        // Already unique: unchanged
        assert_eq!(make_names_unique(&mut list), 0);
        // A number some profile already has is skipped, also a later one's
        let mut list = named(&["A", "A (2)", "A", "A", "A (3)"]);
        assert_eq!(make_names_unique(&mut list), 2);
        assert_eq!(names(&list), ["A", "A (2)", "A (4)", "A (5)", "A (3)"]);
    }

    #[test]
    fn load_makes_names_unique_in_memory() {
        let dir = std::env::temp_dir().join(format!("bluecaster-profiles-dup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let text = r#"{"profiles":[{"name":"WH LDAC","device_address":"00:A2:D0:00:00:02","codec":"ldac"},
            {"name":"WH LDAC","device_address":"00:A2:D0:00:00:02","codec":"ldac","quality":"sq"}]}"#;
        std::fs::write(path(&dir), text).unwrap();
        let list = load(&dir);
        assert_eq!(names(&list), ["WH LDAC", "WH LDAC (2)"]);
        assert_eq!(list[1].quality, "sq");
        // The file itself is left until the next save
        assert_eq!(std::fs::read_to_string(path(&dir)).unwrap(), text);
        save(&dir, &list).unwrap();
        assert_eq!(names(&load(&dir)), ["WH LDAC", "WH LDAC (2)"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn le_audio_profile_gives_an_le_request() {
        let text = r#"{"profiles":[{"name":"Buds LE Audio","device_address":"00:a2:d0:00:00:12","device_name":"Buds",
            "capture_mode":"loopback","le_audio":true}]}"#;
        let f: ProfilesFile = serde_json::from_str(text).unwrap();
        let p = &f.profiles[0];
        assert!(p.le_audio);
        let r = p.to_request(1023).unwrap();
        assert_eq!(r.device, [0x00, 0xA2, 0xD0, 0x00, 0x00, 0x12]);
        // GMAP low latency: on unless the profile says otherwise
        assert_eq!(r.le, Some(bt_host::LeRequest { extra_devices: Vec::new(), preset: None, low_latency: true }));
        assert_eq!(r.source, SourceRequest::SystemLoopback);
        // Written back as v1 wrote it; a classic profile has no key
        let out = serde_json::to_value(p).unwrap();
        assert_eq!(out["le_audio"], true);
        assert!(out.get("le_low_latency").is_none());
        let off = Profile { le_low_latency: false, ..p.clone() };
        assert!(!off.to_request(1023).unwrap().le.unwrap().low_latency);
        assert_eq!(serde_json::to_value(&off).unwrap()["le_low_latency"], false);
        let classic = Profile { le_audio: false, ..p.clone() };
        assert!(serde_json::to_value(&classic).unwrap().get("le_audio").is_none());
        // A bad address is refused as for A2DP
        let bad = Profile { device_address: "nope".into(), ..p.clone() };
        assert_eq!(bad.to_request(1023), Err(ProfileError::Address("nope".into())));
    }

    fn of(name: &str, address: &str, codec: &str) -> Profile {
        Profile { name: name.into(), device_address: address.into(), codec: codec.into(), ..Profile::default() }
    }

    #[test]
    fn consolidate_keeps_one_profile_per_device() {
        const WH: &str = "00:A2:D0:00:00:02";
        const BUDS: &str = "00:A2:D0:00:00:12";
        let mut list = vec![
            of("WH | Auto", WH, "auto"),
            of("Buds | AAC", BUDS, "aac"),
            Profile { quality: "sq".into(), sample_rate: 48000, ..of("WH | LDAC", WH, "ldac") },
            Profile { abr: true, ..of("WH AAC", &WH.to_lowercase(), "aac") },
            of("WH LDAC again", WH, "ldac"),
            Profile { le_audio: true, ..of("Buds LE Audio", BUDS, "auto") },
            of("No address", "", "sbc"),
        ];
        // The last used one is kept, at the place of the device's first
        let folded = consolidate(&mut list, "WH AAC");
        assert_eq!(folded, ["WH | Auto", "WH | LDAC", "WH LDAC again"]);
        assert_eq!(names(&list), ["WH AAC", "Buds | AAC", "Buds LE Audio", "No address"]);
        let wh = &list[0];
        assert_eq!(wh.codec, "aac");
        assert!(wh.abr);
        // The first LDAC profile's settings; Auto has none of its own
        assert_eq!(wh.codec_settings.keys().collect::<Vec<_>>(), ["ldac"]);
        assert_eq!(wh.codec_settings["ldac"].quality, "sq");
        assert_eq!(wh.codec_settings["ldac"].sample_rate, 48000);
        // Nothing more to fold
        assert!(consolidate(&mut list, "WH AAC").is_empty());
        // Without a last profile of the device: the first
        let mut list = vec![of("A", WH, "aac"), of("B", WH, "ldac")];
        assert_eq!(consolidate(&mut list, "Buds | AAC"), ["B"]);
        assert_eq!(names(&list), ["A"]);
        assert!(list[0].codec_settings.contains_key("ldac"));
    }

    #[test]
    fn codec_settings_written_only_when_there_are_some() {
        let mut p = of("WH", "00:A2:D0:00:00:02", "aac");
        assert!(serde_json::to_value(&p).unwrap().get("codec_settings").is_none());
        p.codec_settings.insert("ldac".into(), CodecValues { quality: "mq".into(), ..CodecValues::default() });
        let out = serde_json::to_value(&p).unwrap();
        assert_eq!(
            out["codec_settings"]["ldac"],
            serde_json::json!({"quality": "mq", "sample_rate": 0, "bit_depth": 0, "abr": false})
        );
        let back: Profile = serde_json::from_value(out).unwrap();
        assert_eq!(back, p);
    }
}
