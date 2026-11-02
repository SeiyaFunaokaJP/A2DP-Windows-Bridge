//! Per-app volume on the default output device, as Windows' volume mixer
//! sets it (issue #223): a session's volume comes before the output's mute
//! and before loopback capture, so it works while the output is muted.
//! One entry per executable name: a browser's sessions move together.

use std::collections::BTreeMap;

use win_sys::audio_policy::{self, ComApartment, OutputSession};
use win_sys::process;

/// The key of the system sounds session
pub const SYSTEM_SOUNDS: &str = "*system";

/// An app in the mixer
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MixerApp {
    /// Lowercase executable name, or [`SYSTEM_SOUNDS`]
    pub key: String,
    /// Its description, or the executable name without one; empty for the
    /// system sounds
    pub name: String,
    /// 0-100
    pub volume: u8,
    pub muted: bool,
}

fn file_name(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

fn key_of(s: &OutputSession) -> Option<(String, String)> {
    if s.system_sounds {
        return Some((SYSTEM_SOUNDS.to_owned(), String::new()));
    }
    let path = process::image_path(s.process_id)?;
    let exe = file_name(&path).to_owned();
    let name = process::file_description(&path).unwrap_or_else(|| exe.clone());
    Some((exe.to_lowercase(), name))
}

/// The apps with a session on the default output device, by name
/// (case-insensitive), the system sounds last
pub fn mixer_apps() -> Vec<MixerApp> {
    let Ok(_com) = ComApartment::init_mta() else {
        return Vec::new();
    };
    let sessions = audio_policy::output_sessions().unwrap_or_else(|e| {
        tracing::debug!("mixer: listing sessions failed: {e}");
        Vec::new()
    });
    let mut by_key: BTreeMap<String, MixerApp> = BTreeMap::new();
    for s in &sessions {
        let Some((key, name)) = key_of(s) else {
            continue;
        };
        by_key.entry(key.clone()).or_insert(MixerApp {
            key,
            name,
            volume: (s.volume.clamp(0.0, 1.0) * 100.0).round() as u8,
            muted: s.muted,
        });
    }
    let mut out: Vec<MixerApp> = by_key.into_values().collect();
    out.sort_by_key(|a| (a.key == SYSTEM_SOUNDS, a.name.to_lowercase()));
    out
}

/// Sets the volume (0-100) and / or the mute of an app's sessions. False if
/// it has none, or Windows refused.
pub fn set_mixer_app(key: &str, volume: Option<u8>, muted: Option<bool>) -> bool {
    let Ok(_com) = ComApartment::init_mta() else {
        return false;
    };
    let level = volume.map(|v| f32::from(v.min(100)) / 100.0);
    let r = audio_policy::set_output_sessions(|s| key_of(s).is_some_and(|(k, _)| k == key), level, muted);
    tracing::debug!("mixer: {key}: volume {volume:?}, muted {muted:?}: {r:?}");
    r.is_ok_and(|n| n > 0)
}
