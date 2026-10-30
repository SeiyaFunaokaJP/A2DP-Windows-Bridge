//! The settings folder and its layout: the one place that knows where each
//! file lives. `%APPDATA%\BlueCaster`, or BLUECASTER_CONFIG_DIR (development
//! and tests: tools/emu pairs with virtual devices and must not touch the
//! user's real link keys).
//!
//! ```text
//! BlueCaster\
//!   settings.json  profiles.json
//!   devices\    devices.txt  le_audio_devices.txt  device_codecs.json
//!   pairing\    link_keys.txt  le_audio_bonds.tlv   (the secrets: never in a bundle)
//!   adapters\   adapter_caps.json  driver\<instance id>.json
//!   firmware\   *.bin *.hcd *.sfi *.ddc
//!   logs\       <YYYY-MM-DD>\core.log ui.log driver.log  hci\  crash\  bundles\  verbose.json
//!   backup\     v1\  v2\  profiles-before-codecs.json
//! ```
//!
//! Up to 2.0.x the folder was `%APPDATA%\A2DPWB` with every file at its top
//! level: [`config_dir`] renames that folder once, and the core's schema 7
//! step (crate::migrate) moves the files into their sub folders.
//!
//! The window process (bluecaster-ui) compiles this file too (`#[path]`),
//! so it uses only std, tracing and time.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// The settings folder's name in %APPDATA%
pub const FOLDER_NAME: &str = "BlueCaster";
/// Its name up to 2.0.x // compat
pub const OLD_FOLDER_NAME: &str = "A2DPWB";

/// What [`config_dir`] did to the folder of 2.0.x, for the log (tracing is
/// not set up yet when it first runs)
static FOLDER_NOTE: Mutex<Option<String>> = Mutex::new(None);

pub fn config_dir() -> PathBuf {
    let dir = match std::env::var_os("BLUECASTER_CONFIG_DIR").filter(|v| !v.is_empty()) {
        Some(d) => PathBuf::from(d),
        None => match std::env::var_os("APPDATA") {
            Some(a) => {
                let appdata = PathBuf::from(a);
                let dir = appdata.join(FOLDER_NAME);
                if let Some(note) = move_old_folder(&appdata.join(OLD_FOLDER_NAME), &dir) {
                    *FOLDER_NOTE.lock().unwrap_or_else(|e| e.into_inner()) = Some(note);
                }
                dir
            }
            None => PathBuf::from("."),
        },
    };
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// What [`config_dir`] did to the old folder, once (the caller logs it
/// after its logging is set up)
pub fn take_folder_note() -> Option<String> {
    FOLDER_NOTE.lock().unwrap_or_else(|e| e.into_inner()).take()
}

/// Moves the settings folder of 2.0.x (`old`) to `new` unless `new` exists:
/// a rename (atomic: of the core and the window starting together, one
/// wins and the other finds `new`), else (another volume, a file held
/// open) a copy assembled in a folder of this process' own, renamed to
/// `new` when complete, then the old folder deleted. What happened, or
/// None when there was nothing to do.
pub fn move_old_folder(old: &Path, new: &Path) -> Option<String> {
    if new.exists() || !old.is_dir() {
        return None;
    }
    let rename_error = match std::fs::rename(old, new) {
        Ok(()) => return Some(format!("settings folder {} renamed to {}", old.display(), new.display())),
        Err(_) if new.exists() => return None,
        Err(e) => e,
    };
    match copy_folder(old, new) {
        Ok(None) => None,
        Ok(Some((bytes, deleted))) => {
            let deleted = match deleted {
                Ok(()) => "deleted".to_string(),
                Err(e) => format!("not deleted: {e}"),
            };
            Some(format!(
                "settings folder {} copied to {} ({bytes} bytes; renaming failed: {rename_error}), the old one {deleted}",
                old.display(),
                new.display()
            ))
        }
        Err(e) => Some(format!(
            "settings folder {} could not be moved to {}: {rename_error}; {e}",
            old.display(),
            new.display()
        )),
    }
}

/// [`move_old_folder`] without the rename: `old` copied to a folder of this
/// process' own, renamed to `new`, then `old` deleted -> bytes copied and
/// whether `old` went. None: the other process made `new` first.
fn copy_folder(old: &Path, new: &Path) -> std::io::Result<Option<(u64, std::io::Result<()>)>> {
    let mut tmp_name = new.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(format!(".tmp-{}", std::process::id()));
    let tmp = new.with_file_name(tmp_name);
    let _ = std::fs::remove_dir_all(&tmp);
    match copy_dir(old, &tmp, &|_| true).and_then(|bytes| std::fs::rename(&tmp, new).map(|()| bytes)) {
        Ok(bytes) => Ok(Some((bytes, std::fs::remove_dir_all(old)))),
        Err(_) if new.exists() => {
            let _ = std::fs::remove_dir_all(&tmp);
            Ok(None)
        }
        Err(e) => {
            let _ = std::fs::remove_dir_all(&tmp);
            Err(e)
        }
    }
}

/// Copies `from` into `to` (created), recursively; `keep` decides on each
/// entry of `from` itself (not deeper). Links are left out.
pub fn copy_dir(from: &Path, to: &Path, keep: &dyn Fn(&std::ffi::OsStr) -> bool) -> std::io::Result<u64> {
    std::fs::create_dir_all(to)?;
    let mut bytes = 0;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if !keep(&name) {
            continue;
        }
        let ty = entry.file_type()?;
        let dest = to.join(&name);
        if ty.is_dir() {
            bytes += copy_dir(&entry.path(), &dest, &|_| true)?;
        } else if ty.is_file() {
            bytes += std::fs::copy(entry.path(), &dest)?;
        }
    }
    Ok(bytes)
}

// ---- Top level -----------------------------------------------------------

pub fn settings_file(config: &Path) -> PathBuf {
    config.join("settings.json")
}

pub fn profiles_file(config: &Path) -> PathBuf {
    config.join("profiles.json")
}

/// A capture started by hand (diagnostics): `hci_<stamp>.pklg` at the top
/// of the folder (it keeps the pairing keys, so it is never in a bundle)
pub fn capture_file(config: &Path, stamp: &str) -> PathBuf {
    config.join(format!("hci_{stamp}.pklg"))
}

// ---- devices\ ----------------------------------------------------------

/// The known devices and what they offered
pub fn devices_dir(config: &Path) -> PathBuf {
    config.join("devices")
}

pub fn devices_file(config: &Path) -> PathBuf {
    devices_dir(config).join("devices.txt")
}

pub fn le_devices_file(config: &Path) -> PathBuf {
    devices_dir(config).join("le_audio_devices.txt")
}

pub fn device_codecs_file(config: &Path) -> PathBuf {
    devices_dir(config).join("device_codecs.json")
}

// ---- pairing\ ----------------------------------------------------------

/// The secrets (link keys, LE bonds, CSIP sets): never in a bundle
pub fn pairing_dir(config: &Path) -> PathBuf {
    config.join("pairing")
}

pub fn link_keys_file(config: &Path) -> PathBuf {
    pairing_dir(config).join("link_keys.txt")
}

pub fn le_bonds_file(config: &Path) -> PathBuf {
    pairing_dir(config).join("le_audio_bonds.tlv")
}

// ---- adapters\ ---------------------------------------------------------

pub fn adapters_dir(config: &Path) -> PathBuf {
    config.join("adapters")
}

pub fn adapter_caps_file(config: &Path) -> PathBuf {
    adapters_dir(config).join("adapter_caps.json")
}

/// The WinUSB assignment records (`<instance id>.json`), read by the
/// uninstaller too
pub fn driver_records_dir(config: &Path) -> PathBuf {
    adapters_dir(config).join("driver")
}

/// The assignment record of an adapter
pub fn driver_record_file(config: &Path, instance_id: &str) -> PathBuf {
    let safe: String = instance_id.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    driver_records_dir(config).join(format!("{safe}.json"))
}

// ---- firmware\ ---------------------------------------------------------

/// Where the adapters' firmware files live (created)
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

// ---- logs\ -------------------------------------------------------------

pub fn log_dir(config: &Path) -> PathBuf {
    config.join("logs")
}

/// One folder per day (local date, "YYYY-MM-DD")
pub fn day_log_dir(config: &Path, day: &str) -> PathBuf {
    log_dir(config).join(day)
}

/// The core's log of a day
pub fn core_log_file(config: &Path, day: &str) -> PathBuf {
    day_log_dir(config, day).join(LogFile::Core.name())
}

/// The window's log of a day
pub fn ui_log_file(config: &Path, day: &str) -> PathBuf {
    day_log_dir(config, day).join(LogFile::Ui.name())
}

/// The elevated driver helper's log of a day (it appends)
pub fn driver_log_file(config: &Path, day: &str) -> PathBuf {
    day_log_dir(config, day).join(LogFile::Driver.name())
}

/// Verbose logging that stays on across a restart
pub fn verbose_file(config: &Path) -> PathBuf {
    log_dir(config).join("verbose.json")
}

/// The HCI flight recorder's files (bt_host::HciCapture::save_recent)
pub fn recorder_dir(config: &Path) -> PathBuf {
    log_dir(config).join("hci")
}

/// The flight recorder written for a diagnostic bundle, deleted once read
pub fn recorder_tmp_file(config: &Path, stamp: &str) -> PathBuf {
    log_dir(config).join(format!("recorder-{stamp}.tmp"))
}

/// Crash reports (win_sys::crash)
pub fn crash_dir(config: &Path) -> PathBuf {
    log_dir(config).join("crash")
}

/// Diagnostic bundles (.zip)
pub fn bundles_dir(config: &Path) -> PathBuf {
    log_dir(config).join("bundles")
}

// ---- backup\ -----------------------------------------------------------

pub fn backup_dir(config: &Path) -> PathBuf {
    config.join("backup")
}

/// The folder as v1 left it (crate::migrate)
pub fn backup_v1_dir(config: &Path) -> PathBuf {
    backup_dir(config).join("v1")
}

/// The small files as 2.0.x left them (schema 7), to go back to 2.0.x
pub fn backup_v2_dir(config: &Path) -> PathBuf {
    backup_dir(config).join("v2")
}

/// profiles.json before profiles were first folded into one per device
pub fn profiles_backup_file(config: &Path) -> PathBuf {
    backup_dir(config).join("profiles-before-codecs.json")
}

// ---- Day folders of the logs ---------------------------------------------

/// "YYYYMMDD_HHMMSS", local time: file names that sort by time
pub fn local_stamp() -> String {
    let t = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    format!("{:04}{:02}{:02}_{:02}{:02}{:02}", t.year(), u8::from(t.month()), t.day(), t.hour(), t.minute(), t.second())
}

/// "YYYY-MM-DD": a day folder's name
pub fn day_name(date: time::Date) -> String {
    format!("{:04}-{:02}-{:02}", date.year(), u8::from(date.month()), date.day())
}

/// Today's day folder name, local time
pub fn today() -> String {
    day_name(time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc()).date())
}

/// "YYYY-MM-DD"
pub fn is_day_name(name: &str) -> bool {
    let b = name.as_bytes();
    b.len() == 10 && b.iter().enumerate().all(|(i, c)| if i == 4 || i == 7 { *c == b'-' } else { c.is_ascii_digit() })
}

/// The day folders beyond the newest `keep` (at least one stays), oldest
/// first
pub fn expired_day_dirs(logs: &Path, keep: usize) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(logs) else { return Vec::new() };
    let mut days: Vec<String> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_str().map(str::to_owned))
        .filter(|n| is_day_name(n))
        .collect();
    days.sort();
    let n = days.len().saturating_sub(keep.max(1));
    days[..n].iter().map(|d| logs.join(d)).collect()
}

/// Deletes the day folders beyond the newest `keep`; how many went
pub fn prune_day_dirs(logs: &Path, keep: usize) -> usize {
    expired_day_dirs(logs, keep).iter().filter(|p| std::fs::remove_dir_all(p).is_ok()).count()
}

/// The log files of a day folder
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFile {
    Core,
    Ui,
    Driver,
}

impl LogFile {
    pub fn name(self) -> &'static str {
        match self {
            LogFile::Core => "core.log",
            LogFile::Ui => "ui.log",
            LogFile::Driver => "driver.log",
        }
    }
}

/// A process' log file in today's folder (`logs\<YYYY-MM-DD>\core.log`),
/// opened again in the next day's folder after local midnight (the day of
/// `offset`, read once at start as the timestamps'), the folder created on
/// demand (the window may start before the core). At each new day, the
/// day folders beyond the newest `keep()` are deleted.
pub struct DayLogFile {
    config: PathBuf,
    which: LogFile,
    offset: time::UtcOffset,
    keep: fn() -> usize,
    day: Option<time::Date>,
    file: Option<std::fs::File>,
}

impl DayLogFile {
    pub fn new(config: &Path, which: LogFile, offset: time::UtcOffset, keep: fn() -> usize) -> DayLogFile {
        DayLogFile { config: config.to_path_buf(), which, offset, keep, day: None, file: None }
    }

    fn today(&self) -> time::Date {
        time::OffsetDateTime::now_utc().to_offset(self.offset).date()
    }

    /// Opens the file of `day` (and prunes) unless it is open already
    fn open(&mut self, day: time::Date) -> std::io::Result<&mut std::fs::File> {
        if self.day != Some(day) || self.file.is_none() {
            self.file = None;
            let dir = day_log_dir(&self.config, &day_name(day));
            std::fs::create_dir_all(&dir)?;
            let file = std::fs::OpenOptions::new().create(true).append(true).open(dir.join(self.which.name()))?;
            self.file = Some(file);
            if self.day != Some(day) {
                self.day = Some(day);
                prune_day_dirs(&log_dir(&self.config), (self.keep)());
            }
        }
        Ok(self.file.as_mut().expect("opened above"))
    }

    /// The file it writes to now (None before the first line)
    pub fn path(&self) -> Option<PathBuf> {
        self.day.map(|d| day_log_dir(&self.config, &day_name(d)).join(self.which.name()))
    }
}

impl std::io::Write for DayLogFile {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let day = self.today();
        self.open(day)?.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match &mut self.file {
            Some(f) => f.flush(),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use super::*;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new(name: &str) -> TempDir {
            let d = std::env::temp_dir().join(format!("bluecaster-paths-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&d);
            std::fs::create_dir_all(&d).unwrap();
            TempDir(d)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn loose_firmware_moves_into_its_folder_once() {
        let t = TempDir::new("fw");
        let dir = &t.0;
        std::fs::write(dir.join("rtl8761bu_fw.bin"), [1u8]).unwrap();
        std::fs::write(dir.join("rtl8761bu_config.bin"), [2u8]).unwrap();
        std::fs::write(dir.join("profiles.json"), "{}").unwrap();
        move_loose_firmware(dir);
        assert_eq!(std::fs::read(dir.join("firmware").join("rtl8761bu_fw.bin")).unwrap(), [1]);
        assert!(dir.join("firmware").join("rtl8761bu_config.bin").is_file());
        assert!(!dir.join("rtl8761bu_fw.bin").exists());
        assert!(dir.join("profiles.json").is_file());
        // A file that is there already stays; the loose one is left alone
        std::fs::write(dir.join("rtl8761bu_fw.bin"), [9u8]).unwrap();
        move_loose_firmware(dir);
        assert_eq!(std::fs::read(dir.join("firmware").join("rtl8761bu_fw.bin")).unwrap(), [1]);
    }

    #[test]
    fn the_layout() {
        let c = Path::new(r"C:\cfg");
        assert_eq!(devices_file(c), Path::new(r"C:\cfg\devices\devices.txt"));
        assert_eq!(le_devices_file(c), Path::new(r"C:\cfg\devices\le_audio_devices.txt"));
        assert_eq!(device_codecs_file(c), Path::new(r"C:\cfg\devices\device_codecs.json"));
        assert_eq!(link_keys_file(c), Path::new(r"C:\cfg\pairing\link_keys.txt"));
        assert_eq!(le_bonds_file(c), Path::new(r"C:\cfg\pairing\le_audio_bonds.tlv"));
        assert_eq!(adapter_caps_file(c), Path::new(r"C:\cfg\adapters\adapter_caps.json"));
        assert_eq!(
            driver_record_file(c, r"USB\VID_0BDA&PID_A729\1"),
            Path::new(r"C:\cfg\adapters\driver\USB_VID_0BDA_PID_A729_1.json")
        );
        assert_eq!(core_log_file(c, "2026-10-27"), Path::new(r"C:\cfg\logs\2026-10-27\core.log"));
        assert_eq!(ui_log_file(c, "2026-10-27"), Path::new(r"C:\cfg\logs\2026-10-27\ui.log"));
        assert_eq!(driver_log_file(c, "2026-10-27"), Path::new(r"C:\cfg\logs\2026-10-27\driver.log"));
        assert_eq!(verbose_file(c), Path::new(r"C:\cfg\logs\verbose.json"));
        assert_eq!(backup_v1_dir(c), Path::new(r"C:\cfg\backup\v1"));
        assert_eq!(backup_v2_dir(c), Path::new(r"C:\cfg\backup\v2"));
        assert_eq!(profiles_backup_file(c), Path::new(r"C:\cfg\backup\profiles-before-codecs.json"));
        assert!(is_day_name("2026-10-27") && !is_day_name("2026-10-2x") && !is_day_name("hci"));
        assert_eq!(day_name(time::Date::from_calendar_date(2026, time::Month::January, 5).unwrap()), "2026-01-05");
    }

    #[test]
    fn the_old_folder_is_renamed_once() {
        let t = TempDir::new("rename");
        let (old, new) = (t.0.join("A2DPWB"), t.0.join("BlueCaster"));
        std::fs::create_dir_all(old.join("logs").join("hci")).unwrap();
        std::fs::write(old.join("settings.json"), "{}").unwrap();
        std::fs::write(old.join("logs").join("hci").join("hci-1.pklg"), "x").unwrap();
        let note = move_old_folder(&old, &new).unwrap();
        assert!(note.contains("renamed"), "{note}");
        assert!(!old.exists());
        assert_eq!(std::fs::read_to_string(new.join("settings.json")).unwrap(), "{}");
        assert!(new.join("logs").join("hci").join("hci-1.pklg").is_file());
        // Once: a new folder is never replaced, an old one coming back stays
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("settings.json"), "old").unwrap();
        assert_eq!(move_old_folder(&old, &new), None);
        assert_eq!(std::fs::read_to_string(new.join("settings.json")).unwrap(), "{}");
        // Nothing to move
        assert_eq!(move_old_folder(&t.0.join("none"), &t.0.join("other")), None);
        assert!(!t.0.join("other").exists());
    }

    #[test]
    fn the_old_folder_is_copied_when_renaming_fails() {
        let t = TempDir::new("copy");
        let (old, new) = (t.0.join("A2DPWB"), t.0.join("BlueCaster"));
        std::fs::create_dir_all(old.join("driver")).unwrap();
        std::fs::write(old.join("link_keys.txt"), "keys").unwrap();
        std::fs::write(old.join("driver").join("a.json"), "{}").unwrap();
        let (bytes, deleted) = copy_folder(&old, &new).unwrap().unwrap();
        assert_eq!(bytes, 6);
        assert!(deleted.is_ok() && !old.exists());
        assert_eq!(std::fs::read_to_string(new.join("link_keys.txt")).unwrap(), "keys");
        assert!(new.join("driver").join("a.json").is_file());
        // The copy's own folder is gone
        let left: Vec<String> =
            std::fs::read_dir(&t.0).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(left, ["BlueCaster"]);
        // Another process made the new folder first: this copy goes
        std::fs::create_dir_all(old.join("x")).unwrap();
        std::fs::write(new.join("x"), "a file where the copy's folder would go").unwrap();
        // (renaming the copy onto an existing folder fails)
        assert!(copy_folder(&old, &new).unwrap().is_none());
        assert!(old.exists());
        // The copy helper itself, with an entry left out at the top
        let to = t.0.join("copy-of-new");
        let bytes = copy_dir(&new, &to, &|n| n != "driver" && n != "x").unwrap();
        assert_eq!(bytes, 4);
        assert!(to.join("link_keys.txt").is_file() && !to.join("driver").exists());
    }

    #[test]
    fn day_log_writes_into_todays_folder_and_prunes_old_days() {
        let t = TempDir::new("daylog");
        let logs = log_dir(&t.0);
        for d in ["2000-01-01", "2000-01-02", "2000-01-03", "2000-01-04"] {
            std::fs::create_dir_all(logs.join(d)).unwrap();
            std::fs::write(logs.join(d).join("core.log"), "old").unwrap();
        }
        std::fs::create_dir_all(logs.join("hci")).unwrap();
        std::fs::create_dir_all(logs.join("crash")).unwrap();
        let mut f = DayLogFile::new(&t.0, LogFile::Core, time::UtcOffset::UTC, || 3);
        assert_eq!(f.path(), None);
        f.write_all(b"one\n").unwrap();
        f.write_all(b"two\n").unwrap();
        f.flush().unwrap();
        let today = day_name(time::OffsetDateTime::now_utc().date());
        assert_eq!(f.path().unwrap(), core_log_file(&t.0, &today));
        assert_eq!(std::fs::read_to_string(core_log_file(&t.0, &today)).unwrap(), "one\ntwo\n");
        // Today's and the newest two before it stay; other folders are not days
        let mut left: Vec<String> =
            std::fs::read_dir(&logs).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        left.sort();
        assert_eq!(left, ["2000-01-03", "2000-01-04", today.as_str(), "crash", "hci"]);
        // The window's file in the same folder, appended to
        drop(f);
        let mut ui = DayLogFile::new(&t.0, LogFile::Ui, time::UtcOffset::UTC, || 365);
        ui.write_all(b"ui\n").unwrap();
        assert!(ui_log_file(&t.0, &today).is_file());
        assert!(core_log_file(&t.0, &today).is_file());
        assert_eq!(prune_day_dirs(&logs, 0), 2);
        assert!(core_log_file(&t.0, &today).is_file());
    }
}
