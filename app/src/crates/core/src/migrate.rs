//! The first start of v2 on a settings folder that v1 used: before v2 reads
//! or writes anything, the folder is copied as it is to `backup-v1\` (once),
//! then v1's files are brought to v2's schema. (Since 3.0.0 the copy is
//! made at `backup\v1\`.)
//!
//! - settings.json gets `schema_version` (v1 wrote none; v2 reads a missing
//!   one as 1). Keys v2 does not know stay, so going back to v1 still works.
//! - Before v1.1 the max media packet size was a setting; profiles saved
//!   without one used it. v2 does not write that setting back, so its value
//!   is copied into those profiles first (what v1 did when it read them).
//! - v1 named profiles "<device> <codec>" and told them apart by position,
//!   so two alike shared a name; v2 finds them by name, so a repeated name
//!   gets " (2)", " (3)"… ([`profiles::load`] does the same in memory when
//!   v1 wrote the file again later).
//! - start_minimized is set: v1 always started hidden in the tray at logon
//!   (whatever that key said), v2's autostart honours it.
//! - devices.txt, link_keys.txt, the firmware and captures are not changed.
//!   v1's debug.log stays; v2 logs to `logs\` (see the core-service's
//!   logging), which the backup leaves out.
//!
//! Call [`prepare`] before the settings are first loaded ([`crate::Service`]
//! does). A folder v2 already uses (schema_version 2 or later) only gets the
//! later schema steps; a new one without v1's files is left alone.
//!
//! Schema 3 (2.0.1): LE Audio is on by default, so a folder of schema 2 gets
//! `le_audio_experimental` turned on once (2.0.0 wrote it off for everyone).
//!
//! Schema 4 (2.0.2): each device's volume is remembered by default (issue
//! #86), so a folder of schema 3 or older gets `remember_volume` turned on
//! once.
//!
//! Schema 5 (issue #86): "Auto-select the codec with the highest quality"
//! is one setting for every device (`auto_codec`) instead of each profile's
//! codec "auto". It starts on only when no profile had a codec chosen, so
//! a device someone chose a codec for keeps streaming with it (v1's folders
//! too: each v1 profile chose its codec).
//!
//! Schema 6 (issue #86): Windows Volume Sync, the media keys and "remember
//! volume" are one setting each for every device. `volume_sync` starts as
//! the profile in use had it (the last one streamed, else the first; on
//! without a profile); a device a profile followed stays in the file but
//! no longer counts (the core's state.rs).
//! The per-device overrides of media keys and remember volume
//! (`device_options`) are dropped: the global values count.
//!
//! Schema 7 (3.0.0): the folder of 2.0.x (renamed from `%APPDATA%\A2DPWB`
//! by [`crate::paths::config_dir`]) gets its sub folders ([`relayout`]):
//! first its small files are copied once to `backup\v2\` (so that 2.0.x can
//! be installed again with its pairings), then each file moves to its place
//! in [`crate::paths`] (only where nothing is yet: a move cut short goes on
//! at the next start), `backup-v1\` becomes `backup\v1\`, the daily log
//! files go into a folder per day, and the leftovers of interrupted writes
//! go. A v1 folder gets the v1 step and then this one, in one start.
//!
//! At every start, the profiles are folded into one per device
//! ([`profiles::consolidate`]: the Codecs screen keeps one); the
//! first time that changes anything, profiles.json is copied as it was to
//! `backup\profiles-before-codecs.json`. A profile that chose dual channel for
//! plain SBC gets joint stereo (2.0.2, issue #86: dual channel is SBC XQ's).

use std::path::Path;

use serde_json::Value;

use crate::paths;
use crate::profiles;
use crate::settings::{FIRST_V2_SCHEMA, SCHEMA_VERSION, Settings};

/// The first schema with the folder's sub folders
const LAYOUT_SCHEMA: u32 = 7;

/// The folder before schema 7 (v1 and 2.0.x): every file at the top level.
/// Only the migration knows these places.
mod old {
    /// The copy of the folder as v1 left it (2.0.x)
    pub const BACKUP_V1: &str = "backup-v1";
    /// Where 2.0.x assembled it
    pub const BACKUP_V1_TMP: &str = "backup-v1.tmp";
    /// profiles.json before the first fold (2.0.x)
    pub const PROFILES_BACKUP: &str = "profiles-before-codecs.json";
    pub const DEVICES: &str = "devices.txt";
    pub const LE_DEVICES: &str = "le_audio_devices.txt";
    pub const DEVICE_CODECS: &str = "device_codecs.json";
    pub const ADAPTER_CAPS: &str = "adapter_caps.json";
    pub const LINK_KEYS: &str = "link_keys.txt";
    pub const LE_BONDS: &str = "le_audio_bonds.tlv";
    /// CSIP sets (bt-host's csip module, in the folder of the LE bonds)
    pub const LE_CSIP: &str = "le_audio_csip.txt";
    pub const DRIVER_RECORDS: &str = "driver";
    /// v1's files that mean "v1 used this folder" even without settings.json
    pub const V1_FILES: [&str; 3] = ["profiles.json", DEVICES, LINK_KEYS];
    /// The daily logs: `<prefix>core.YYYY-MM-DD.log`; 2.0.x wrote a2dpwb-
    /// (compat), builds of the rename before schema 7 bluecaster-
    pub const LOG_PREFIXES: [&str; 2] = ["a2dpwb-", "bluecaster-"];
}

/// Where the copy of a v1 folder is assembled; renamed when complete
fn backup_v1_tmp(dir: &Path) -> std::path::PathBuf {
    paths::backup_dir(dir).join("v1.tmp")
}

const BACKUP_README: &str = "\
This folder is a copy of the BlueCaster settings folder as A2DP Windows Bridge v1
left it. Version 2 made it once, before it first used the folder.

To go back to v1 with these settings: exit BlueCaster v2 (tray icon > Exit), copy
the files of this folder into the folder above it, replacing the files there,
then start v1. Pairings (link_keys.txt) made with v2 are kept only if you
leave link_keys.txt out of the copy.

Nothing reads this folder; it can be deleted once v2 works for you.
";

/// What [`prepare`] did
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// schema_version of settings.json before (None: no settings.json)
    pub schema_before: Option<u32>,
    /// The folder held v1's files and had no v2 schema yet
    pub from_v1: bool,
    /// backup-v1\ was made by this call
    pub backed_up: bool,
    /// Profiles that got the old max media packet size setting
    pub profiles_seeded: usize,
    /// Profiles renamed because another one had the same name
    pub profiles_renamed: usize,
    /// settings.json was rewritten with the current schema_version
    pub schema_written: bool,
    /// Profiles folded into another one of the same device
    pub profiles_folded: usize,
    /// Schema 7's sub folders were made by this call
    pub relaid_out: bool,
}

/// schema_version of the settings file: None without a readable file, 1 for
/// v1's files (which have none)
fn schema_of(dir: &Path) -> Option<u32> {
    let text = std::fs::read_to_string(Settings::path(dir)).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    Some(v.get("schema_version").and_then(Value::as_u64).map_or(1, |n| n.min(u32::MAX as u64) as u32))
}

/// A v1 backup exists (2.0.x's place or this one's)
fn has_backup_v1(dir: &Path) -> bool {
    paths::backup_v1_dir(dir).exists() || dir.join(old::BACKUP_V1).exists()
}

/// Copies the entries of `dir` that `keep` takes to `to`, assembled as
/// `tmp` first, so an interrupted copy is never taken for a complete one
/// (it is redone at the next start)
fn backup_to(
    dir: &Path,
    to: &Path,
    tmp: &Path,
    readme: &str,
    keep: &dyn Fn(&std::ffi::OsStr) -> bool,
) -> std::io::Result<u64> {
    if tmp.exists() {
        tracing::debug!("migrate: removing an interrupted backup {}", tmp.display());
        std::fs::remove_dir_all(tmp)?;
    }
    let result = paths::copy_dir(dir, tmp, keep).and_then(|bytes| {
        std::fs::write(tmp.join("README.txt"), readme)?;
        std::fs::rename(tmp, to)?;
        Ok(bytes)
    });
    if result.is_err() {
        let _ = std::fs::remove_dir_all(tmp);
    }
    result
}

/// Makes `backup\v1\` from the folder's current content, without the
/// backups and the logs
fn backup(dir: &Path) -> std::io::Result<u64> {
    std::fs::create_dir_all(paths::backup_dir(dir))?;
    let skip = |n: &std::ffi::OsStr| {
        [old::BACKUP_V1, old::BACKUP_V1_TMP, "backup", "logs"].iter().any(|s| n.eq_ignore_ascii_case(s))
    };
    backup_to(dir, &paths::backup_v1_dir(dir), &backup_v1_tmp(dir), BACKUP_README, &|n| !skip(n))
}

/// Brings a settings folder from v1 to v2 (see the module documentation).
/// Never fails: problems are logged, and a failed backup leaves v1's files
/// unchanged so that the next start tries again.
pub fn prepare(dir: &Path) -> Report {
    if let Some(note) = paths::take_folder_note() {
        tracing::info!("{note}");
    }
    let mut report = prepare_settings(dir);
    // After v1's files were backed up as they were
    crate::paths::move_loose_firmware(dir);
    report.profiles_folded = consolidate_profiles(dir);
    report
}

/// One profile per device (see the module documentation); how many were
/// folded away. Nothing changes when the backup cannot be made.
fn consolidate_profiles(dir: &Path) -> usize {
    if !profiles::path(dir).is_file() {
        return 0;
    }
    let (mut list, _, dual) = profiles::load_fixed(dir);
    let folded = profiles::consolidate(&mut list, &Settings::load(dir).last_profile);
    if folded.is_empty() {
        // SBC's dual channel dropped (issue #86) is written at once
        if dual > 0
            && let Err(e) = profiles::save(dir, &list)
        {
            tracing::error!("profiles: cannot save {}: {e}", profiles::path(dir).display());
        }
        return 0;
    }
    let backup = paths::profiles_backup_file(dir);
    if !backup.exists()
        && let Err(e) =
            std::fs::create_dir_all(paths::backup_dir(dir)).and_then(|()| std::fs::copy(profiles::path(dir), &backup))
    {
        tracing::error!("profiles: cannot copy them to {}: {e}; not folded", backup.display());
        return 0;
    }
    match profiles::save(dir, &list) {
        Ok(()) => {
            tracing::info!(
                "profiles: {} folded into the profile of their device ({} kept), the file before in {}",
                folded.len(),
                list.len(),
                backup.display()
            );
            folded.len()
        }
        Err(e) => {
            tracing::error!("profiles: cannot save {}: {e}", profiles::path(dir).display());
            0
        }
    }
}

fn prepare_settings(dir: &Path) -> Report {
    let schema_before = schema_of(dir);
    let v1_files = old::V1_FILES.iter().any(|f| dir.join(f).is_file());
    let from_v1 = match schema_before {
        Some(n) => n < FIRST_V2_SCHEMA,
        // A settings.json that does not parse is v1's too: Settings::load
        // keeps it as .bad before using the defaults
        None => v1_files || Settings::path(dir).is_file(),
    };
    let mut report = Report { schema_before, from_v1, ..Report::default() };
    tracing::debug!(
        "migrate: {}: schema {schema_before:?} (current {SCHEMA_VERSION}), v1 files {v1_files}, from v1 {from_v1}",
        dir.display()
    );
    if !from_v1 {
        if schema_before.is_some_and(|n| n < SCHEMA_VERSION) {
            tracing::debug!("migrate: upgrading the v2 schema");
            upgrade_v2(dir, &mut report);
        }
        return report;
    }

    tracing::debug!("migrate: v1 backup exists {}", has_backup_v1(dir));
    if !has_backup_v1(dir) {
        let to = paths::backup_v1_dir(dir);
        match backup(dir) {
            Ok(bytes) => {
                tracing::info!("settings from v1: copied to {} ({bytes} bytes)", to.display());
                report.backed_up = true;
            }
            Err(e) => {
                tracing::error!(
                    "settings from v1: cannot copy them to {}: {e}; v1's files are left as they are",
                    to.display()
                );
                return report;
            }
        }
    }

    let mut settings = Settings::load(dir);
    // v1 always started hidden in the tray at logon (it registered
    // --minimized whatever start_minimized said, and offered no switch for
    // it); v2 honours the setting, so it is set to keep that
    settings.start_minimized = true;
    let (mut list, renamed) = profiles::load_counted(dir);
    settings.auto_codec = auto_codec_from(&list);
    settings.volume_sync = volume_sync_from(&settings, &list);
    let mut seeded = 0;
    if settings.max_media_payload.is_some() {
        let legacy = settings.legacy_max_media_payload();
        for p in list.iter_mut().filter(|p| p.max_media_payload.is_none()) {
            p.max_media_payload = Some(legacy);
            seeded += 1;
        }
        if seeded > 0 {
            tracing::info!("settings from v1: max media packet size {legacy} given to {seeded} profile(s)");
        }
    }
    if renamed > 0 {
        tracing::info!("settings from v1: {renamed} profile(s) with a repeated name given a number");
    }
    if seeded > 0 || renamed > 0 {
        match profiles::save(dir, &list) {
            Ok(()) => {
                report.profiles_seeded = seeded;
                report.profiles_renamed = renamed;
            }
            Err(e) => {
                // Keep the old setting in settings.json: it is still used
                tracing::error!("settings from v1: cannot save {}: {e}", profiles::path(dir).display());
                return report;
            }
        }
    }
    // Then schema 7's sub folders, before the schema is written
    if !relayout(dir, &mut report) {
        return report;
    }
    if schema_before.is_some() {
        match settings.save(dir) {
            Ok(()) => report.schema_written = true,
            Err(e) => tracing::error!("settings from v1: cannot save {}: {e}", Settings::path(dir).display()),
        }
    }
    tracing::debug!("migrate: {report:?}");
    report
}

/// Schema 5's Auto-select for every device: on unless a profile chose a
/// codec (LE Audio profiles have only LC3)
fn auto_codec_from(list: &[profiles::Profile]) -> bool {
    let chosen = list.iter().filter(|p| !p.le_audio && p.codec != "auto").count();
    if chosen > 0 {
        tracing::info!("settings: Auto-select for every device off, {chosen} profile(s) chose their codec");
    }
    chosen == 0
}

/// Schema 6's Windows Volume Sync for every device: as the profile in use
/// had it (the last one streamed, else the first); on without a profile
fn volume_sync_from(settings: &Settings, list: &[profiles::Profile]) -> bool {
    let p = list.iter().find(|p| p.name == settings.last_profile).or(list.first());
    let on = p.is_none_or(|p| p.volume_sync != "off");
    if let Some(p) = p {
        tracing::info!(
            "settings: Windows Volume Sync for every device {}, as profile {} had it",
            if on { "on" } else { "off" },
            crate::diag::quoted(&p.name)
        );
    }
    on
}

/// The schema steps after v2's first one, for a folder v2 already used
fn upgrade_v2(dir: &Path, report: &mut Report) {
    let mut settings = Settings::load(dir);
    // 3: LE Audio on by default (2.0.0 wrote it off for everyone)
    if report.schema_before.is_some_and(|n| n < 3) && !settings.le_audio_experimental {
        settings.le_audio_experimental = true;
        tracing::info!("settings: LE Audio (experimental) turned on, as it now is by default");
    }
    // 4: each device's volume remembered by default
    if report.schema_before.is_some_and(|n| n < 4) && !settings.remember_volume {
        settings.remember_volume = true;
        tracing::info!("settings: remembering each device's volume, as it now is by default");
    }
    // 5: one Auto-select for every device
    if report.schema_before.is_some_and(|n| n < 5) {
        settings.auto_codec = auto_codec_from(&profiles::load(dir));
    }
    // 6: one Windows Volume Sync, media keys and remember volume for every
    // device (the overrides in device_options are not written back)
    if report.schema_before.is_some_and(|n| n < 6) {
        settings.volume_sync = volume_sync_from(&settings, &profiles::load(dir));
        if settings.device_options.take().is_some() {
            tracing::info!("settings: the per-device media keys and remember volume give way to the global ones");
        }
    }
    // 7: the sub folders; the schema is written only once they are done
    if report.schema_before.is_some_and(|n| n < LAYOUT_SCHEMA) && !relayout(dir, report) {
        return;
    }
    match settings.save(dir) {
        Ok(()) => report.schema_written = true,
        Err(e) => tracing::error!("settings: cannot save {}: {e}", Settings::path(dir).display()),
    }
    tracing::debug!(
        "migrate: schema {:?} -> {SCHEMA_VERSION}, written {}",
        report.schema_before,
        report.schema_written
    );
}

const BACKUP_V2_README: &str = "\
This folder is a copy of the small files of the BlueCaster settings folder as
version 2.0.x left them (settings, profiles, known devices, pairings, adapter
records). Version 3.0.0 made it once, before it moved the files into sub
folders.

To go back to 2.0.x: uninstall 3.0.0, copy the files of this folder into
%APPDATA%\\A2DPWB (create it), then install 2.0.x.

Nothing reads this folder; it can be deleted once 3.0.0 works for you.
";

/// The files and folders at the top of a 2.0.x folder that go to
/// `backup\v2\` (no logs, no firmware, no backups)
const V2_SMALL: [&str; 10] = [
    "settings.json",
    "profiles.json",
    old::DEVICES,
    old::LE_DEVICES,
    old::DEVICE_CODECS,
    old::ADAPTER_CAPS,
    old::LINK_KEYS,
    old::LE_BONDS,
    old::LE_CSIP,
    old::DRIVER_RECORDS,
];

/// Moves `from` to `to` unless `to` exists or `from` does not; whether it
/// moved
fn move_if_free(from: &Path, to: &Path) -> bool {
    if !from.exists() || to.exists() {
        return false;
    }
    if let Some(parent) = to.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::rename(from, to) {
        Ok(()) => {
            tracing::info!("settings folder: {} moved to {}", from.display(), to.display());
            true
        }
        Err(e) => {
            tracing::warn!("settings folder: cannot move {} to {}: {e}", from.display(), to.display());
            false
        }
    }
}

/// `a2dpwb-core.2026-10-27.log` -> ("core", "2026-10-27")
fn flat_log(name: &str) -> Option<(&str, &str)> {
    let rest = old::LOG_PREFIXES.iter().find_map(|p| name.strip_prefix(p))?;
    let rest = rest.strip_suffix(".log")?;
    let (kind, day) = rest.split_once('.')?;
    (matches!(kind, "core" | "ui") && paths::is_day_name(day)).then_some((kind, day))
}

/// A flat log file's place in its day folder: `core.log`, or, when the
/// logger of this start has written that file already, `core.old.log`
fn day_place(dir: &Path, day: &str, kind: &str) -> std::path::PathBuf {
    let target = paths::day_log_dir(dir, day).join(format!("{kind}.log"));
    if target.exists() { paths::day_log_dir(dir, day).join(format!("{kind}.old.log")) } else { target }
}

/// Schema 7's sub folders (see the module documentation). False when the
/// backup of the 2.0.x files could not be made: nothing moved, the next
/// start tries again.
pub fn relayout(dir: &Path, report: &mut Report) -> bool {
    // backup\v1\ from 2.0.x's place first: it is not copied below
    let _ = std::fs::create_dir_all(paths::backup_dir(dir));
    move_if_free(&dir.join(old::BACKUP_V1), &paths::backup_v1_dir(dir));
    let _ = std::fs::remove_dir_all(dir.join(old::BACKUP_V1_TMP));

    let small = |n: &std::ffi::OsStr| V2_SMALL.iter().any(|s| n.eq_ignore_ascii_case(s));
    let any_small = V2_SMALL.iter().any(|f| dir.join(f).exists());
    let v2 = paths::backup_v2_dir(dir);
    if any_small && !v2.exists() {
        match backup_to(dir, &v2, &paths::backup_dir(dir).join("v2.tmp"), BACKUP_V2_README, &small) {
            Ok(bytes) => tracing::info!("settings of 2.0.x: copied to {} ({bytes} bytes)", v2.display()),
            Err(e) => {
                tracing::error!("settings of 2.0.x: cannot copy them to {}: {e}; nothing moved", v2.display());
                return false;
            }
        }
    }

    let mut moved = 0;
    let mut mv = |from: std::path::PathBuf, to: std::path::PathBuf| moved += usize::from(move_if_free(&from, &to));
    mv(dir.join(old::DEVICES), paths::devices_file(dir));
    mv(dir.join(old::LE_DEVICES), paths::le_devices_file(dir));
    mv(dir.join(old::DEVICE_CODECS), paths::device_codecs_file(dir));
    mv(dir.join(old::LINK_KEYS), paths::link_keys_file(dir));
    mv(dir.join(old::LE_BONDS), paths::le_bonds_file(dir));
    mv(dir.join(old::LE_CSIP), paths::pairing_dir(dir).join(old::LE_CSIP));
    mv(dir.join(old::ADAPTER_CAPS), paths::adapter_caps_file(dir));
    mv(dir.join(old::PROFILES_BACKUP), paths::profiles_backup_file(dir));
    let records = dir.join(old::DRIVER_RECORDS);
    if let Ok(entries) = std::fs::read_dir(&records) {
        for e in entries.flatten() {
            mv(e.path(), paths::driver_records_dir(dir).join(e.file_name()));
        }
    }

    // The daily log files into their day folders
    let logs = paths::log_dir(dir);
    let today = paths::today();
    if let Ok(entries) = std::fs::read_dir(&logs) {
        for e in entries.flatten().filter(|e| e.path().is_file()) {
            let name = e.file_name().to_string_lossy().into_owned();
            let driver = |suffix: &str| old::LOG_PREFIXES.iter().any(|p| name == format!("{p}driver{suffix}"));
            if let Some((kind, day)) = flat_log(&name) {
                mv(e.path(), day_place(dir, day, kind));
            } else if driver(".log") {
                mv(e.path(), day_place(dir, &today, "driver"));
            } else if driver(".log.old") {
                mv(e.path(), paths::day_log_dir(dir, &today).join("driver.log.old"));
            } else if name.starts_with("recorder-") && name.ends_with(".tmp") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    paths::prune_day_dirs(&logs, Settings::load(dir).log().keep_days.max(1) as usize);

    // Leftovers: interrupted atomic writes, loose firmware, an empty driver\
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten().filter(|e| e.path().is_file()) {
            let name = e.file_name().to_string_lossy().to_ascii_lowercase();
            if name.ends_with(".json.tmp") || name.ends_with(".txt.tmp") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    paths::move_loose_firmware(dir);
    let _ = std::fs::remove_dir(&records);
    tracing::info!("settings folder: schema 7's sub folders, {moved} file(s) moved");
    report.relaid_out = true;
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(std::path::PathBuf);
    impl TempDir {
        fn new(name: &str) -> TempDir {
            let d = std::env::temp_dir().join(format!("bluecaster-migrate-{name}-{}", std::process::id()));
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

    fn read_json(p: &Path) -> Value {
        serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
    }

    const V1_SETTINGS: &str = r#"{"language":"ja","max_media_payload":1500,"window_x":10}"#;
    const V1_PROFILES: &str = r#"{"profiles":[{"name":"A","device_address":"AA:BB:CC:DD:EE:FF","codec":"ldac"},
        {"name":"B","device_address":"11:22:33:44:55:66","max_media_payload":800}]}"#;

    #[test]
    fn v1_folder_is_backed_up_once_and_migrated() {
        let t = TempDir::new("v1");
        let dir = &t.0;
        std::fs::write(Settings::path(dir), V1_SETTINGS).unwrap();
        std::fs::write(profiles::path(dir), V1_PROFILES).unwrap();
        std::fs::write(dir.join("link_keys.txt"), "AA:BB:CC:DD:EE:FF 00112233445566778899AABBCCDDEEFF 4\n").unwrap();
        std::fs::create_dir_all(dir.join("firmware")).unwrap();
        std::fs::write(dir.join("firmware").join("rtl8761bu_fw.bin"), [1u8, 2, 3]).unwrap();
        std::fs::create_dir_all(dir.join("logs")).unwrap();
        std::fs::write(dir.join("logs").join("bluecaster.log"), "v2's log").unwrap();

        let r = prepare(dir);
        assert_eq!(
            r,
            Report {
                schema_before: Some(1),
                from_v1: true,
                backed_up: true,
                profiles_seeded: 1,
                profiles_renamed: 0,
                schema_written: true,
                profiles_folded: 0,
                relaid_out: true,
            }
        );
        // The backup has v1's files as they were, without v2's logs
        let b = paths::backup_v1_dir(dir);
        assert_eq!(std::fs::read_to_string(b.join("settings.json")).unwrap(), V1_SETTINGS);
        assert_eq!(std::fs::read_to_string(b.join("profiles.json")).unwrap(), V1_PROFILES);
        assert!(b.join("link_keys.txt").is_file());
        assert_eq!(std::fs::read(b.join("firmware").join("rtl8761bu_fw.bin")).unwrap(), [1, 2, 3]);
        assert!(b.join("README.txt").is_file());
        assert!(!b.join("logs").exists());
        assert!(!backup_v1_tmp(dir).exists());

        // settings.json: v2's schema, unknown keys kept, the old setting moved
        let s = read_json(&Settings::path(dir));
        assert_eq!(s["schema_version"], SCHEMA_VERSION);
        assert_eq!(s["language"], "ja");
        // Profile A chose LDAC: no Auto-select for every device
        assert_eq!(s["auto_codec"], false);
        assert_eq!(s["window_x"], 10);
        assert!(s.get("max_media_payload").is_none());
        // v1 started hidden at logon: kept
        assert_eq!(s["start_minimized"], true);
        let p = profiles::load(dir);
        assert_eq!(p[0].max_media_payload, Some(1500));
        assert_eq!(p[1].max_media_payload, Some(800));
        // link keys unchanged, in their new place
        assert!(std::fs::read_to_string(paths::link_keys_file(dir)).unwrap().starts_with("AA:BB"));

        // Second start: nothing to do, the backup stays as it was
        std::fs::write(paths::link_keys_file(dir), "changed by v2\n").unwrap();
        let r = prepare(dir);
        assert_eq!(r, Report { schema_before: Some(SCHEMA_VERSION), ..Report::default() });
        assert!(std::fs::read_to_string(b.join("link_keys.txt")).unwrap().starts_with("AA:BB"));
    }

    #[test]
    fn v2_settings_get_le_audio_on_once() {
        let t = TempDir::new("schema3");
        let dir = &t.0;
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            Settings::path(dir),
            r#"{"schema_version":2,"language":"ja","le_audio_experimental":false,"le_audio_low_latency":false}"#,
        )
        .unwrap();
        let r = prepare(dir);
        assert_eq!(r, Report { schema_before: Some(2), schema_written: true, relaid_out: true, ..Report::default() });
        assert!(!paths::backup_v1_dir(dir).exists());
        let s = read_json(&Settings::path(dir));
        assert_eq!(s["schema_version"], SCHEMA_VERSION);
        assert_eq!(s["le_audio_experimental"], true);
        assert_eq!(s["le_audio_low_latency"], false);
        assert_eq!(s["language"], "ja");

        // Turned off again by the user: stays off
        let mut off = Settings::load(dir);
        off.le_audio_experimental = false;
        off.save(dir).unwrap();
        assert_eq!(prepare(dir), Report { schema_before: Some(SCHEMA_VERSION), ..Report::default() });
        assert!(!Settings::load(dir).le_audio_experimental);
    }

    #[test]
    fn v2_settings_remember_each_volume_once() {
        let t = TempDir::new("schema4");
        let dir = &t.0;
        std::fs::write(
            Settings::path(dir),
            r#"{"schema_version":3,"remember_volume":false,"saved_volumes":{"AA:BB:CC:DD:EE:FF":40},
                "device_options":{"11:22:33:44:55:66":{"remember_volume":false}}}"#,
        )
        .unwrap();
        let r = prepare(dir);
        assert_eq!(r, Report { schema_before: Some(3), schema_written: true, relaid_out: true, ..Report::default() });
        let s = Settings::load(dir);
        assert_eq!(s.schema_version, SCHEMA_VERSION);
        assert!(s.remember_volume);
        assert_eq!(s.saved_volumes["AA:BB:CC:DD:EE:FF"], 40);
        // A device's own value gave way to the one for every device (schema 6)
        assert!(s.device_options.is_none());
        assert!(read_json(&Settings::path(dir)).get("device_options").is_none());

        // Turned off again by the user: stays off
        let mut off = s;
        off.remember_volume = false;
        off.save(dir).unwrap();
        assert_eq!(prepare(dir), Report { schema_before: Some(SCHEMA_VERSION), ..Report::default() });
        assert!(!Settings::load(dir).remember_volume);
    }

    #[test]
    fn v2_settings_get_one_auto_select_from_the_profiles() {
        let t = TempDir::new("schema5");
        let dir = &t.0;
        let write = |profiles: &str| {
            std::fs::write(Settings::path(dir), r#"{"schema_version":4}"#).unwrap();
            std::fs::write(profiles::path(dir), profiles).unwrap();
        };
        // Every device on Auto (an LE Audio profile counts as Auto): on
        write(
            r#"{"profiles":[{"name":"A","device_address":"AA:BB:CC:DD:EE:FF","codec":"auto"},
                {"name":"B","device_address":"11:22:33:44:55:66","codec":"lc3","le_audio":true}]}"#,
        );
        assert!(prepare(dir).schema_written);
        assert!(Settings::load(dir).auto_codec);
        // A device with its codec chosen: off, and it keeps that codec
        write(
            r#"{"profiles":[{"name":"A","device_address":"AA:BB:CC:DD:EE:FF","codec":"auto"},
                {"name":"B","device_address":"11:22:33:44:55:66","codec":"aac"}]}"#,
        );
        prepare(dir);
        let s = Settings::load(dir);
        assert_eq!(s.schema_version, SCHEMA_VERSION);
        assert!(!s.auto_codec);
        assert_eq!(profiles::load(dir)[1].codec, "aac");

        // Turned on by the user: stays on
        let mut on = s;
        on.auto_codec = true;
        on.save(dir).unwrap();
        assert_eq!(prepare(dir), Report { schema_before: Some(SCHEMA_VERSION), ..Report::default() });
        assert!(Settings::load(dir).auto_codec);
    }

    #[test]
    fn v2_settings_get_one_volume_sync_from_the_profile_in_use() {
        let t = TempDir::new("schema6");
        let dir = &t.0;
        let write = |settings: &str| {
            std::fs::write(Settings::path(dir), settings).unwrap();
            std::fs::write(
                profiles::path(dir),
                r#"{"profiles":[{"name":"A","device_address":"AA:BB:CC:DD:EE:FF","volume_sync":"auto"},
                    {"name":"B","device_address":"11:22:33:44:55:66","volume_sync":"off"}]}"#,
            )
            .unwrap();
        };
        // The last profile streamed had it off: off for every device
        write(
            r#"{"schema_version":5,"last_profile":"B","media_keys":true,
                "device_options":{"AA:BB:CC:DD:EE:FF":{"media_keys":false}}}"#,
        );
        assert!(prepare(dir).schema_written);
        let s = Settings::load(dir);
        assert_eq!(s.schema_version, SCHEMA_VERSION);
        assert!(!s.volume_sync);
        // The device's own media keys are gone: the global value counts
        assert!(s.media_keys && s.device_options.is_none());
        assert!(read_json(&Settings::path(dir)).get("device_options").is_none());
        // No profile streamed yet: the first one's (on)
        write(r#"{"schema_version":5}"#);
        prepare(dir);
        assert!(Settings::load(dir).volume_sync);

        // Turned off by the user: stays off
        let mut off = Settings::load(dir);
        off.volume_sync = false;
        off.save(dir).unwrap();
        assert_eq!(prepare(dir), Report { schema_before: Some(SCHEMA_VERSION), ..Report::default() });
        assert!(!Settings::load(dir).volume_sync);
    }

    #[test]
    fn v1_again_after_v2_keeps_the_first_backup() {
        // v1 ran again in between and wrote settings.json without the schema
        let t = TempDir::new("again");
        let dir = &t.0;
        std::fs::create_dir_all(paths::backup_v1_dir(dir)).unwrap();
        std::fs::write(paths::backup_v1_dir(dir).join("settings.json"), "first").unwrap();
        std::fs::write(Settings::path(dir), r#"{"language":"de"}"#).unwrap();
        let r = prepare(dir);
        assert!(r.from_v1 && !r.backed_up && r.schema_written);
        assert_eq!(std::fs::read_to_string(paths::backup_v1_dir(dir).join("settings.json")).unwrap(), "first");
        assert_eq!(read_json(&Settings::path(dir))["schema_version"], SCHEMA_VERSION);
    }

    #[test]
    fn repeated_v1_profile_names_are_numbered() {
        // Of three devices (the profiles of one device are folded into one)
        let t = TempDir::new("dup");
        let dir = &t.0;
        std::fs::write(Settings::path(dir), r#"{"language":"ja","last_profile":"WH LDAC"}"#).unwrap();
        let text = r#"{"profiles":[{"name":"WH LDAC","device_address":"AA:BB:CC:DD:EE:FF","codec":"ldac"},
            {"name":"WH LDAC","device_address":"AA:BB:CC:DD:EE:01","codec":"ldac","quality":"mq"},
            {"name":"WH LDAC","device_address":"AA:BB:CC:DD:EE:02","codec":"ldac","quality":"sq"}]}"#;
        std::fs::write(profiles::path(dir), text).unwrap();
        let r = prepare(dir);
        assert_eq!(r.profiles_renamed, 2);
        assert_eq!(r.profiles_seeded, 0);
        // Written to the file, in v1's order; the backup has v1's file
        let f = read_json(&profiles::path(dir));
        let names: Vec<&str> = f["profiles"].as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["WH LDAC", "WH LDAC (2)", "WH LDAC (3)"]);
        assert_eq!(f["profiles"][1]["quality"], "mq");
        assert_eq!(std::fs::read_to_string(paths::backup_v1_dir(dir).join("profiles.json")).unwrap(), text);
        // last_profile still names the first one
        assert_eq!(read_json(&Settings::path(dir))["last_profile"], "WH LDAC");
    }

    fn tree(dir: &Path) -> Vec<String> {
        fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let rel = e.path().strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
                if e.path().is_dir() {
                    out.push(format!("{rel}/"));
                    walk(base, &e.path(), out);
                } else {
                    out.push(rel);
                }
            }
        }
        let mut out = Vec::new();
        walk(dir, dir, &mut out);
        out.sort();
        out
    }

    /// A folder as 2.0.2 (beta.x) left it
    fn beta_folder(dir: &Path) {
        let w = |rel: &str, text: &str| {
            let p = dir.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        };
        w("settings.json", r#"{"schema_version":6,"language":"ja"}"#);
        w("profiles.json", r#"{"profiles":[{"name":"WH","device_address":"00:A2:D0:00:00:02","codec":"ldac"}]}"#);
        w("devices.txt", "00:A2:D0:00:00:02|WH\n");
        w("le_audio_devices.txt", "00:A2:D0:00:00:12|Buds\n");
        w("device_codecs.json", "{}");
        w("adapter_caps.json", "{}");
        w("link_keys.txt", "keys\n");
        w("le_audio_bonds.tlv", "bonds");
        w("le_audio_csip.txt", "sets");
        w("driver/USB_VID_0BDA_PID_A729_1.json", "{}");
        w("firmware/rtl8761bu_fw.bin", "fw");
        w("rtl8761bu_config.bin", "loose fw");
        w("settings.json.tmp", "half");
        w("profiles-before-codecs.json", "before");
        w("backup-v1/settings.json", "v1");
        w("logs/a2dpwb-core.2000-01-01.log", "core 1");
        w("logs/a2dpwb-ui.2000-01-01.log", "ui 1");
        w("logs/a2dpwb-core.2000-01-02.log", "core 2");
        w("logs/a2dpwb-driver.log", "driver");
        w("logs/recorder-20261027_120000.tmp", "rec");
        w("logs/verbose.json", "{}");
        w("logs/hci/hci-20261027_120000-x.pklg", "pklg");
        w("logs/crash/bluecaster-core-20261027-120000-000.txt", "crash");
        w("logs/bundles/bluecaster-diag-20261027_120000.zip", "zip");
    }

    #[test]
    fn a_beta_folder_gets_the_sub_folders() {
        let t = TempDir::new("schema7");
        let dir = &t.0;
        beta_folder(dir);
        let r = prepare(dir);
        assert_eq!(r, Report { schema_before: Some(6), schema_written: true, relaid_out: true, ..Report::default() });
        assert_eq!(Settings::load(dir).schema_version, 7);
        let today = paths::today();
        let mut want: Vec<String> = [
            "adapters/",
            "adapters/adapter_caps.json",
            "adapters/driver/",
            "adapters/driver/USB_VID_0BDA_PID_A729_1.json",
            "backup/",
            "backup/profiles-before-codecs.json",
            "backup/v1/",
            "backup/v1/settings.json",
            "backup/v2/",
            "backup/v2/README.txt",
            "backup/v2/adapter_caps.json",
            "backup/v2/device_codecs.json",
            "backup/v2/devices.txt",
            "backup/v2/driver/",
            "backup/v2/driver/USB_VID_0BDA_PID_A729_1.json",
            "backup/v2/le_audio_bonds.tlv",
            "backup/v2/le_audio_csip.txt",
            "backup/v2/le_audio_devices.txt",
            "backup/v2/link_keys.txt",
            "backup/v2/profiles.json",
            "backup/v2/settings.json",
            "devices/",
            "devices/device_codecs.json",
            "devices/devices.txt",
            "devices/le_audio_devices.txt",
            "firmware/",
            "firmware/rtl8761bu_config.bin",
            "firmware/rtl8761bu_fw.bin",
            "logs/",
            "logs/2000-01-01/",
            "logs/2000-01-01/core.log",
            "logs/2000-01-01/ui.log",
            "logs/2000-01-02/",
            "logs/2000-01-02/core.log",
            "logs/bundles/",
            "logs/bundles/bluecaster-diag-20261027_120000.zip",
            "logs/crash/",
            "logs/crash/bluecaster-core-20261027-120000-000.txt",
            "logs/hci/",
            "logs/hci/hci-20261027_120000-x.pklg",
            "logs/verbose.json",
            "pairing/",
            "pairing/le_audio_bonds.tlv",
            "pairing/le_audio_csip.txt",
            "pairing/link_keys.txt",
            "profiles.json",
            "settings.json",
        ]
        .map(String::from)
        .into();
        // The driver helper's log has no date: today's
        want.push(format!("logs/{today}/"));
        want.push(format!("logs/{today}/driver.log"));
        want.sort();
        assert_eq!(tree(dir), want);
        assert_eq!(std::fs::read_to_string(paths::core_log_file(dir, "2000-01-02")).unwrap(), "core 2");
        assert_eq!(std::fs::read_to_string(paths::link_keys_file(dir)).unwrap(), "keys\n");
        // backup\v2 has the 2.0.x files as they were
        assert_eq!(
            std::fs::read_to_string(paths::backup_v2_dir(dir).join("settings.json")).unwrap(),
            r#"{"schema_version":6,"language":"ja"}"#
        );
        // The devices are read from their new place
        assert_eq!(crate::devices::load(dir)[0].name, "WH");

        // (d) Again: nothing changes
        let before = tree(dir);
        assert_eq!(prepare(dir), Report { schema_before: Some(7), ..Report::default() });
        let mut again = Report::default();
        assert!(relayout(dir, &mut again));
        assert_eq!(tree(dir), before);
    }

    #[test]
    fn a_move_cut_short_goes_on() {
        let t = TempDir::new("half");
        let dir = &t.0;
        beta_folder(dir);
        // An earlier start took the backup and moved some files, then stopped
        std::fs::create_dir_all(paths::backup_v2_dir(dir)).unwrap();
        std::fs::write(paths::backup_v2_dir(dir).join("settings.json"), "taken before").unwrap();
        std::fs::create_dir_all(paths::devices_dir(dir)).unwrap();
        std::fs::rename(dir.join("devices.txt"), paths::devices_file(dir)).unwrap();
        std::fs::create_dir_all(paths::pairing_dir(dir)).unwrap();
        std::fs::rename(dir.join("link_keys.txt"), paths::link_keys_file(dir)).unwrap();
        // ...and today's logger wrote core.log already
        let today = paths::today();
        std::fs::create_dir_all(paths::day_log_dir(dir, &today)).unwrap();
        std::fs::write(paths::driver_log_file(dir, &today), "new driver").unwrap();
        let r = prepare(dir);
        assert!(r.relaid_out && r.schema_written);
        // The backup is not taken again
        assert_eq!(std::fs::read_to_string(paths::backup_v2_dir(dir).join("settings.json")).unwrap(), "taken before");
        assert!(paths::le_devices_file(dir).is_file() && paths::le_bonds_file(dir).is_file());
        assert_eq!(std::fs::read_to_string(paths::link_keys_file(dir)).unwrap(), "keys\n");
        // The old driver log next to the new one
        let day = paths::day_log_dir(dir, &today);
        assert_eq!(std::fs::read_to_string(day.join("driver.log")).unwrap(), "new driver");
        assert_eq!(std::fs::read_to_string(day.join("driver.old.log")).unwrap(), "driver");
        let top: Vec<String> = tree(dir)
            .into_iter()
            .filter(|n| !n.contains('/') || n.matches('/').count() == 1 && n.ends_with('/'))
            .collect();
        assert_eq!(
            top,
            ["adapters/", "backup/", "devices/", "firmware/", "logs/", "pairing/", "profiles.json", "settings.json"]
        );
    }

    #[test]
    fn a_v1_folder_goes_to_schema_7_in_one_start() {
        let t = TempDir::new("v1to7");
        let dir = &t.0;
        std::fs::write(Settings::path(dir), V1_SETTINGS).unwrap();
        std::fs::write(profiles::path(dir), V1_PROFILES).unwrap();
        std::fs::write(dir.join("devices.txt"), "AA:BB:CC:DD:EE:FF|Phones\n").unwrap();
        std::fs::write(dir.join("link_keys.txt"), "keys\n").unwrap();
        std::fs::write(dir.join("debug.log"), "v1's log").unwrap();
        let r = prepare(dir);
        assert!(r.from_v1 && r.backed_up && r.relaid_out && r.schema_written, "{r:?}");
        assert_eq!(Settings::load(dir).schema_version, 7);
        // Both backups: v1's files as they were, then v2's
        let v1 = paths::backup_v1_dir(dir);
        assert_eq!(std::fs::read_to_string(v1.join("settings.json")).unwrap(), V1_SETTINGS);
        assert!(v1.join("link_keys.txt").is_file() && v1.join("debug.log").is_file());
        let v2 = paths::backup_v2_dir(dir);
        assert!(v2.join("profiles.json").is_file() && v2.join("link_keys.txt").is_file());
        assert!(!v2.join("debug.log").exists());
        assert_eq!(std::fs::read_to_string(paths::link_keys_file(dir)).unwrap(), "keys\n");
        assert_eq!(crate::devices::load(dir)[0].name, "Phones");
        assert!(!dir.join("devices.txt").exists() && !dir.join("backup-v1").exists());
        assert!(!paths::backup_dir(dir).join("v1.tmp").exists() && !paths::backup_dir(dir).join("v2.tmp").exists());
    }

    #[test]
    fn a_new_folder_is_made_in_the_new_layout() {
        let t = TempDir::new("fresh");
        let dir = &t.0;
        assert_eq!(prepare(dir), Report::default());
        assert!(tree(dir).is_empty(), "{:?}", tree(dir));
        crate::devices::save_codecs(dir, &Default::default()).unwrap();
        Settings::default().save(dir).unwrap();
        assert_eq!(tree(dir), ["devices/", "devices/device_codecs.json", "settings.json"]);
        assert_eq!(prepare(dir), Report { schema_before: Some(7), ..Report::default() });
    }

    #[test]
    fn new_folder_is_left_alone() {
        let t = TempDir::new("new");
        let r = prepare(&t.0);
        assert_eq!(r, Report::default());
        assert!(!paths::backup_v1_dir(&t.0).exists());
        assert!(!Settings::path(&t.0).exists());
    }

    #[test]
    fn link_keys_without_settings_count_as_v1() {
        let t = TempDir::new("keys");
        std::fs::write(t.0.join("link_keys.txt"), "x\n").unwrap();
        let r = prepare(&t.0);
        assert!(r.from_v1 && r.backed_up && !r.schema_written && r.relaid_out);
        assert!(paths::backup_v1_dir(&t.0).join("link_keys.txt").is_file());
        assert!(paths::link_keys_file(&t.0).is_file());
        assert!(!Settings::path(&t.0).exists());
    }

    #[test]
    fn a_newer_schema_is_not_touched() {
        let t = TempDir::new("newer");
        let text = r#"{"schema_version":9,"language":"fr"}"#;
        std::fs::write(Settings::path(&t.0), text).unwrap();
        let r = prepare(&t.0);
        assert!(!r.from_v1);
        assert_eq!(std::fs::read_to_string(Settings::path(&t.0)).unwrap(), text);
    }

    #[test]
    fn v1_settings_read_as_schema_1() {
        let s: Settings = serde_json::from_str(r#"{"language":"ja"}"#).unwrap();
        assert_eq!(s.schema_version, 1);
        assert_eq!(Settings::default().schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn profiles_are_folded_once_with_a_backup() {
        let t = TempDir::new("fold");
        let dir = &t.0;
        std::fs::write(Settings::path(dir), r#"{"schema_version":3,"last_profile":"WH | LDAC"}"#).unwrap();
        let text = r#"{"profiles":[{"name":"WH | Auto","device_address":"00:A2:D0:00:00:02"},
            {"name":"WH | LDAC","device_address":"00:A2:D0:00:00:02","codec":"ldac"},
            {"name":"WH AAC","device_address":"00:A2:D0:00:00:02","codec":"aac","abr":true}]}"#;
        std::fs::write(profiles::path(dir), text).unwrap();
        let r = prepare(dir);
        assert_eq!(r.profiles_folded, 2);
        assert_eq!(std::fs::read_to_string(paths::profiles_backup_file(dir)).unwrap(), text);
        let list = profiles::load(dir);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "WH | LDAC");
        assert!(list[0].codec_settings["aac"].abr);
        // Nothing left to fold; a later fold keeps the first backup
        assert_eq!(prepare(dir).profiles_folded, 0);
        let again = r#"{"profiles":[{"name":"A","device_address":"00:A2:D0:00:00:02"},{"name":"B","device_address":"00:A2:D0:00:00:02"}]}"#;
        std::fs::write(profiles::path(dir), again).unwrap();
        assert_eq!(prepare(dir).profiles_folded, 1);
        assert_eq!(std::fs::read_to_string(paths::profiles_backup_file(dir)).unwrap(), text);
    }

    #[test]
    fn sbc_dual_channel_becomes_joint_stereo_in_the_file() {
        let t = TempDir::new("sbcdual");
        let dir = &t.0;
        std::fs::write(Settings::path(dir), r#"{"schema_version":4}"#).unwrap();
        let text = r#"{"profiles":[{"name":"Moto","device_address":"00:A2:D0:00:00:02","codec":"sbc",
            "sbc_channel_mode":"dual_channel","sbc_blocks":8,
            "codec_settings":{"sbcxq":{"sbc_channel_mode":"dual_channel","abr":true}}},
            {"name":"WH","device_address":"00:A2:D0:00:00:03","codec":"sbc","sbc_channel_mode":"stereo"}]}"#;
        std::fs::write(profiles::path(dir), text).unwrap();
        assert_eq!(prepare(dir).profiles_folded, 0);
        let saved = read_json(&profiles::path(dir));
        let moto = &saved["profiles"][0];
        assert!(moto.get("sbc_channel_mode").is_none(), "{moto}");
        assert_eq!(moto["sbc_blocks"], 8);
        assert!(moto["codec_settings"]["sbcxq"].get("sbc_channel_mode").is_none(), "{moto}");
        assert_eq!(moto["codec_settings"]["sbcxq"]["abr"], true);
        assert_eq!(saved["profiles"][1]["sbc_channel_mode"], "stereo");
        // No backup: nothing was folded
        assert!(!paths::profiles_backup_file(dir).exists());
    }
}
