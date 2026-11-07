//! Saved devices, in v1's files: devices.txt for the classic (A2DP) ones
//! and le_audio_devices.txt for the experimental LE Audio ones, one
//! "XX:XX:XX:XX:XX:XX|name" per line. Older versions read devices.txt only,
//! and would show an LE Audio entry there as one more classic device, so LE
//! Audio devices have a file of their own (none while there are none). A
//! dual-mode device can have a classic and an LE Audio entry.
//!
//! An early LE Audio prototype wrote "addr|name|le" lines into devices.txt:
//! they are read as LE Audio entries and moved to le_audio_devices.txt the
//! next time the files are written (as v1 did).

use std::collections::BTreeMap;
use std::path::Path;

use crate::paths;
use crate::stream::KnownCodecs;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SavedDevice {
    pub address: String,
    pub name: String,
    /// An LE Audio (experimental) entry
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub le_audio: bool,
}

/// The suffix of the prototype's LE Audio lines in devices.txt
const LEGACY_LE_SUFFIX: &str = "|le";

/// One "addr|name" line (v1 parse_saved_device_line); the address in upper
/// case
fn parse_line(line: &str) -> Option<(String, &str)> {
    let line = line.trim_end_matches(['\r', '\n']);
    let (addr, name) = line.split_once('|')?;
    bt_host::parse_addr(addr)?;
    Some((addr.to_uppercase(), name))
}

fn read_lines(file: &Path) -> Vec<(String, String)> {
    let Ok(text) = std::fs::read_to_string(file) else {
        tracing::trace!("devices: no {}", file.display());
        return Vec::new();
    };
    let lines: Vec<(String, String)> = text.lines().filter_map(parse_line).map(|(a, n)| (a, n.to_string())).collect();
    tracing::trace!("devices: {}: {} device(s) of {} line(s)", file.display(), lines.len(), text.lines().count());
    lines
}

/// Both files: the classic devices, then the LE Audio ones (le_audio set;
/// the prototype's "|le" lines of devices.txt first, an address listed in
/// both places once)
fn load_all(dir: &Path) -> Vec<SavedDevice> {
    let mut classic = Vec::new();
    let mut le: Vec<SavedDevice> = Vec::new();
    for (address, name) in read_lines(&paths::devices_file(dir)) {
        match name.strip_suffix(LEGACY_LE_SUFFIX) {
            Some(name) => le.push(SavedDevice { address, name: name.to_string(), le_audio: true }),
            None => classic.push(SavedDevice { address, name, le_audio: false }),
        }
    }
    for (address, name) in read_lines(&paths::le_devices_file(dir)) {
        if !le.iter().any(|d| d.address == address) {
            le.push(SavedDevice { address, name, le_audio: true });
        }
    }
    classic.extend(le);
    classic
}

/// Writes both files (v1 write_saved_devices): devices.txt with the classic
/// devices only, le_audio_devices.txt with the LE Audio ones, deleted when
/// there are none
fn write_all(dir: &Path, devices: &[SavedDevice]) -> std::io::Result<()> {
    let lines = |le: bool| -> String {
        devices.iter().filter(|d| d.le_audio == le).map(|d| format!("{}|{}\n", d.address, d.name)).collect()
    };
    std::fs::create_dir_all(paths::devices_dir(dir))?;
    std::fs::write(paths::devices_file(dir), lines(false))?;
    let le = lines(true);
    let le_path = paths::le_devices_file(dir);
    if le.is_empty() {
        match std::fs::remove_file(&le_path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    } else {
        std::fs::write(le_path, le)
    }
}

/// The classic (A2DP) devices
pub fn load(dir: &Path) -> Vec<SavedDevice> {
    load_all(dir).into_iter().filter(|d| !d.le_audio).collect()
}

/// The LE Audio (experimental) devices, incl. the prototype's "|le" lines
pub fn load_le(dir: &Path) -> Vec<SavedDevice> {
    load_all(dir).into_iter().filter(|d| d.le_audio).collect()
}

fn save_kind(dir: &Path, address: &str, name: &str, le_audio: bool) -> std::io::Result<()> {
    let mut devices = load_all(dir);
    let address = address.to_uppercase();
    match devices.iter_mut().find(|d| d.address == address && d.le_audio == le_audio) {
        Some(d) => {
            if !name.is_empty() {
                d.name = name.to_string();
            }
        }
        None => {
            tracing::debug!("devices: adding {address} \"{name}\" (LE Audio {le_audio})");
            devices.push(SavedDevice { address, name: name.to_string(), le_audio });
        }
    }
    let r = write_all(dir, &devices);
    tracing::debug!("devices: saved {} device(s) to {}: {r:?}", devices.len(), dir.display());
    r
}

fn remove_kind(dir: &Path, address: &str, le_audio: bool) -> std::io::Result<()> {
    let mut devices = load_all(dir);
    let before = devices.len();
    devices.retain(|d| !(d.le_audio == le_audio && d.address.eq_ignore_ascii_case(address)));
    let r = write_all(dir, &devices);
    tracing::debug!("devices: removing {address} (LE Audio {le_audio}): {} removed, {r:?}", before - devices.len());
    r
}

/// Adds or renames a classic device (an empty name keeps the known one)
pub fn save(dir: &Path, address: &str, name: &str) -> std::io::Result<()> {
    save_kind(dir, address, name, false)
}

/// Forgets a classic device (its LE Audio entry stays)
pub fn remove(dir: &Path, address: &str) -> std::io::Result<()> {
    remove_kind(dir, address, false)
}

/// Adds or renames an LE Audio device (an empty name keeps the known one)
pub fn save_le(dir: &Path, address: &str, name: &str) -> std::io::Result<()> {
    save_kind(dir, address, name, true)
}

/// Forgets an LE Audio device (its classic entry stays)
pub fn remove_le(dir: &Path, address: &str) -> std::io::Result<()> {
    remove_kind(dir, address, true)
}

/// What each classic device offered when it last connected, by upper-case
/// address: kept so the Codecs screen knows it after a restart (issue #86)
pub fn load_codecs(dir: &Path) -> BTreeMap<String, KnownCodecs> {
    std::fs::read_to_string(paths::device_codecs_file(dir))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save_codecs(dir: &Path, codecs: &BTreeMap<String, KnownCodecs>) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(codecs).map_err(std::io::Error::other)?;
    std::fs::create_dir_all(paths::devices_dir(dir))?;
    std::fs::write(paths::device_codecs_file(dir), text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("bluecaster-dev-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(paths::devices_dir(&dir)).unwrap();
        dir
    }

    #[test]
    fn save_load_remove() {
        let dir = temp_dir("classic");
        std::fs::write(paths::devices_file(&dir), "00:11:22:33:44:55|Old\nbad line\n").unwrap();
        save(&dir, "aa:bb:cc:dd:ee:ff", "Phones").unwrap();
        save(&dir, "00:11:22:33:44:55", "").unwrap();
        let d = load(&dir);
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].name, "Old");
        assert_eq!(d[1].address, "AA:BB:CC:DD:EE:FF");
        assert!(!d[1].le_audio);
        remove(&dir, "00:11:22:33:44:55").unwrap();
        assert_eq!(load(&dir).len(), 1);
        // No LE Audio device: no file of them
        assert!(!paths::le_devices_file(&dir).exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn codecs_survive_a_restart() {
        let dir = temp_dir("codecs");
        assert!(load_codecs(&dir).is_empty());
        let mut map = BTreeMap::new();
        map.insert(
            "AA:BB:CC:DD:EE:FF".to_string(),
            KnownCodecs { keys: vec!["sbc".into(), "aac".into()], ..Default::default() },
        );
        save_codecs(&dir, &map).unwrap();
        assert_eq!(load_codecs(&dir), map);
        // A damaged file is as none
        std::fs::write(paths::device_codecs_file(&dir), "{ not json").unwrap();
        assert!(load_codecs(&dir).is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn le_audio_devices_have_their_own_file() {
        let dir = temp_dir("le");
        std::fs::write(paths::le_devices_file(&dir), "00:A2:D0:00:00:12|Buds\r\nnot an address|x\n").unwrap();
        save(&dir, "00:a2:d0:00:00:12", "Buds classic").unwrap();
        save_le(&dir, "00:a2:d0:00:00:13", "Buds R").unwrap();
        save_le(&dir, "00:A2:D0:00:00:12", "").unwrap();

        // The dual-mode device has two entries
        let classic = load(&dir);
        let le = load_le(&dir);
        assert_eq!(classic.len(), 1);
        assert_eq!((classic[0].name.as_str(), classic[0].le_audio), ("Buds classic", false));
        let names: Vec<_> = le.iter().map(|d| (d.address.as_str(), d.name.as_str(), d.le_audio)).collect();
        assert_eq!(names, [("00:A2:D0:00:00:12", "Buds", true), ("00:A2:D0:00:00:13", "Buds R", true)]);
        assert_eq!(std::fs::read_to_string(paths::devices_file(&dir)).unwrap(), "00:A2:D0:00:00:12|Buds classic\n");
        assert_eq!(
            std::fs::read_to_string(paths::le_devices_file(&dir)).unwrap(),
            "00:A2:D0:00:00:12|Buds\n00:A2:D0:00:00:13|Buds R\n"
        );

        // Forgetting one kind keeps the other
        remove_le(&dir, "00:a2:d0:00:00:12").unwrap();
        assert_eq!(load(&dir).len(), 1);
        assert_eq!(load_le(&dir).len(), 1);
        remove(&dir, "00:A2:D0:00:00:12").unwrap();
        remove_le(&dir, "00:A2:D0:00:00:13").unwrap();
        assert!(load(&dir).is_empty() && load_le(&dir).is_empty());
        assert!(!paths::le_devices_file(&dir).exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn prototype_le_lines_move_to_the_le_file() {
        let dir = temp_dir("legacy");
        std::fs::write(paths::devices_file(&dir), "00:11:22:33:44:55|Phones\n00:A2:D0:00:00:12|Buds|le\n").unwrap();
        // Also listed in the LE file: one entry, the prototype's line first
        std::fs::write(paths::le_devices_file(&dir), "00:A2:D0:00:00:12|Buds (new)\n").unwrap();
        assert_eq!(load(&dir).len(), 1);
        let le = load_le(&dir);
        assert_eq!(le.len(), 1);
        assert_eq!((le[0].name.as_str(), le[0].le_audio), ("Buds", true));

        // The next write moves it out of devices.txt, which older versions
        // read as a classic device
        save(&dir, "00:11:22:33:44:55", "").unwrap();
        assert_eq!(std::fs::read_to_string(paths::devices_file(&dir)).unwrap(), "00:11:22:33:44:55|Phones\n");
        assert_eq!(std::fs::read_to_string(paths::le_devices_file(&dir)).unwrap(), "00:A2:D0:00:00:12|Buds\n");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
