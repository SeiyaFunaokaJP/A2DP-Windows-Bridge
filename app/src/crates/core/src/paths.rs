//! The settings folder: %APPDATA%\BlueCaster, or BLUECASTER_CONFIG_DIR (development
//! and tests: tools/emu pairs with virtual devices and must not touch the
//! user's real link keys). Shared with v1.

use std::path::{Path, PathBuf};

pub fn config_dir() -> PathBuf {
    let dir = match std::env::var_os("BLUECASTER_CONFIG_DIR").filter(|v| !v.is_empty()) {
        Some(d) => PathBuf::from(d),
        None => match std::env::var_os("APPDATA") {
            Some(a) => PathBuf::from(a).join("BlueCaster"),
            None => PathBuf::from("."),
        },
    };
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// The daily log files: `logs\` in the settings folder
pub fn log_dir(config: &Path) -> PathBuf {
    config.join("logs")
}

/// The HCI flight recorder's files (bt_host::HciCapture::save_recent)
pub fn recorder_dir(config: &Path) -> PathBuf {
    log_dir(config).join("hci")
}

/// Crash reports (win_sys::crash)
pub fn crash_dir(config: &Path) -> PathBuf {
    log_dir(config).join("crash")
}

/// "YYYYMMDD_HHMMSS", local time: file names that sort by time
pub fn local_stamp() -> String {
    let t = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    format!("{:04}{:02}{:02}_{:02}{:02}{:02}", t.year(), u8::from(t.month()), t.day(), t.hour(), t.minute(), t.second())
}

/// Where the adapters' firmware files live: `firmware\` in the settings
/// folder (they were loose in the folder itself before 2.0.3)
pub fn firmware_dir(config: &Path) -> PathBuf {
    let dir = config.join("firmware");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Firmware files a version before 2.0.3 left in the settings folder itself
/// (Realtek *_fw.bin / *_config.bin, Broadcom *.hcd, Intel *.sfi / *.ddc)
/// move to [`firmware_dir`]; a file already there is not replaced
pub fn move_loose_firmware(config: &Path) {
    let Ok(entries) = std::fs::read_dir(config) else { return };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_ascii_lowercase();
        let firmware = name.ends_with("_fw.bin")
            || name.ends_with("_config.bin")
            || name.ends_with(".hcd")
            || name.ends_with(".sfi")
            || name.ends_with(".ddc");
        if !firmware || !e.path().is_file() {
            continue;
        }
        let to = firmware_dir(config).join(e.file_name());
        if to.exists() {
            continue;
        }
        match std::fs::rename(e.path(), &to) {
            Ok(()) => tracing::info!("firmware file {} moved to {}", e.path().display(), to.display()),
            Err(err) => tracing::warn!("cannot move {} to {}: {err}", e.path().display(), to.display()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loose_firmware_moves_into_its_folder_once() {
        let dir = std::env::temp_dir().join(format!("bluecaster-fw-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("rtl8761bu_fw.bin"), [1u8]).unwrap();
        std::fs::write(dir.join("rtl8761bu_config.bin"), [2u8]).unwrap();
        std::fs::write(dir.join("profiles.json"), "{}").unwrap();
        move_loose_firmware(&dir);
        assert_eq!(std::fs::read(dir.join("firmware").join("rtl8761bu_fw.bin")).unwrap(), [1]);
        assert!(dir.join("firmware").join("rtl8761bu_config.bin").is_file());
        assert!(!dir.join("rtl8761bu_fw.bin").exists());
        assert!(dir.join("profiles.json").is_file());
        // A file that is there already stays; the loose one is left alone
        std::fs::write(dir.join("rtl8761bu_fw.bin"), [9u8]).unwrap();
        move_loose_firmware(&dir);
        assert_eq!(std::fs::read(dir.join("firmware").join("rtl8761bu_fw.bin")).unwrap(), [1]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
