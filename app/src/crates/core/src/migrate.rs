//! The first start of v2 on a settings folder that v1 used: before v2 reads
//! or writes anything, the folder is copied as it is to `backup-v1\` (once),
//! then v1's files are brought to v2's schema.
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
//! At every start, the profiles are folded into one per device
//! ([`profiles::consolidate`]: the Codecs screen keeps one); the
//! first time that changes anything, profiles.json is copied as it was to
//! `profiles-before-codecs.json`. A profile that chose dual channel for
//! plain SBC gets joint stereo (2.0.2, issue #86: dual channel is SBC XQ's).

use std::path::Path;

use serde_json::Value;

use crate::profiles;
use crate::settings::{FIRST_V2_SCHEMA, SCHEMA_VERSION, Settings};

/// The copy of the folder as v1 left it
pub const BACKUP_DIR: &str = "backup-v1";
/// Where the copy is assembled; renamed to BACKUP_DIR when complete
const BACKUP_TMP: &str = "backup-v1.tmp";
/// Not copied: the backups themselves and v2's own logs
const NOT_COPIED: [&str; 3] = [BACKUP_DIR, BACKUP_TMP, "logs"];
/// v1's files that mean "v1 used this folder" even without settings.json
const V1_FILES: [&str; 3] = ["profiles.json", "devices.txt", "link_keys.txt"];
/// profiles.json as it was before profiles were first folded into one per device
pub const PROFILES_BACKUP: &str = "profiles-before-codecs.json";

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
}

/// schema_version of the settings file: None without a readable file, 1 for
/// v1's files (which have none)
fn schema_of(dir: &Path) -> Option<u32> {
    let text = std::fs::read_to_string(Settings::path(dir)).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    Some(v.get("schema_version").and_then(Value::as_u64).map_or(1, |n| n.min(u32::MAX as u64) as u32))
}

/// Copies `from` into `to` (created), recursively, except the top-level
/// entries named in NOT_COPIED
fn copy_tree(from: &Path, to: &Path, top: bool) -> std::io::Result<u64> {
    std::fs::create_dir_all(to)?;
    let mut bytes = 0;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if top && NOT_COPIED.iter().any(|n| name.eq_ignore_ascii_case(n)) {
            continue;
        }
        let ty = entry.file_type()?;
        let dest = to.join(&name);
        if ty.is_dir() {
            bytes += copy_tree(&entry.path(), &dest, false)?;
        } else if ty.is_file() {
            bytes += std::fs::copy(entry.path(), &dest)?;
        }
        // Links are left out: the folder has none of its own
    }
    Ok(bytes)
}

/// Makes `backup-v1\` from the folder's current content. The copy is
/// assembled under another name first, so an interrupted copy is never taken
/// for a complete one (it is redone at the next start).
fn backup(dir: &Path) -> std::io::Result<u64> {
    let tmp = dir.join(BACKUP_TMP);
    if tmp.exists() {
        tracing::debug!("migrate: removing an interrupted backup {}", tmp.display());
        std::fs::remove_dir_all(&tmp)?;
    }
    let result = copy_tree(dir, &tmp, true).and_then(|bytes| {
        std::fs::write(tmp.join("README.txt"), BACKUP_README)?;
        std::fs::rename(&tmp, dir.join(BACKUP_DIR))?;
        Ok(bytes)
    });
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&tmp);
    }
    result
}

/// Brings a settings folder from v1 to v2 (see the module documentation).
/// Never fails: problems are logged, and a failed backup leaves v1's files
/// unchanged so that the next start tries again.
pub fn prepare(dir: &Path) -> Report {
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
    let backup = dir.join(PROFILES_BACKUP);
    if !backup.exists()
        && let Err(e) = std::fs::copy(profiles::path(dir), &backup)
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
    let v1_files = V1_FILES.iter().any(|f| dir.join(f).is_file());
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

    tracing::debug!("migrate: backup {} exists {}", BACKUP_DIR, dir.join(BACKUP_DIR).exists());
    if !dir.join(BACKUP_DIR).exists() {
        match backup(dir) {
            Ok(bytes) => {
                tracing::info!("settings from v1: copied to {} ({bytes} bytes)", dir.join(BACKUP_DIR).display());
                report.backed_up = true;
            }
            Err(e) => {
                tracing::error!(
                    "settings from v1: cannot copy them to {BACKUP_DIR}: {e}; v1's files are left as they are"
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
                profiles_folded: 0
            }
        );
        // The backup has v1's files as they were, without v2's logs
        let b = dir.join(BACKUP_DIR);
        assert_eq!(std::fs::read_to_string(b.join("settings.json")).unwrap(), V1_SETTINGS);
        assert_eq!(std::fs::read_to_string(b.join("profiles.json")).unwrap(), V1_PROFILES);
        assert!(b.join("link_keys.txt").is_file());
        assert_eq!(std::fs::read(b.join("firmware").join("rtl8761bu_fw.bin")).unwrap(), [1, 2, 3]);
        assert!(b.join("README.txt").is_file());
        assert!(!b.join("logs").exists());
        assert!(!dir.join(BACKUP_TMP).exists());

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
        // link keys untouched
        assert!(std::fs::read_to_string(dir.join("link_keys.txt")).unwrap().starts_with("AA:BB"));

        // Second start: nothing to do, the backup stays as it was
        std::fs::write(dir.join("link_keys.txt"), "changed by v2\n").unwrap();
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
        assert_eq!(r, Report { schema_before: Some(2), schema_written: true, ..Report::default() });
        assert!(!dir.join(BACKUP_DIR).exists());
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
        assert_eq!(r, Report { schema_before: Some(3), schema_written: true, ..Report::default() });
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
        std::fs::create_dir_all(dir.join(BACKUP_DIR)).unwrap();
        std::fs::write(dir.join(BACKUP_DIR).join("settings.json"), "first").unwrap();
        std::fs::write(Settings::path(dir), r#"{"language":"de"}"#).unwrap();
        let r = prepare(dir);
        assert!(r.from_v1 && !r.backed_up && r.schema_written);
        assert_eq!(std::fs::read_to_string(dir.join(BACKUP_DIR).join("settings.json")).unwrap(), "first");
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
        assert_eq!(std::fs::read_to_string(dir.join(BACKUP_DIR).join("profiles.json")).unwrap(), text);
        // last_profile still names the first one
        assert_eq!(read_json(&Settings::path(dir))["last_profile"], "WH LDAC");
    }

    #[test]
    fn new_folder_is_left_alone() {
        let t = TempDir::new("new");
        let r = prepare(&t.0);
        assert_eq!(r, Report::default());
        assert!(!t.0.join(BACKUP_DIR).exists());
        assert!(!Settings::path(&t.0).exists());
    }

    #[test]
    fn link_keys_without_settings_count_as_v1() {
        let t = TempDir::new("keys");
        std::fs::write(t.0.join("link_keys.txt"), "x\n").unwrap();
        let r = prepare(&t.0);
        assert!(r.from_v1 && r.backed_up && !r.schema_written);
        assert!(t.0.join(BACKUP_DIR).join("link_keys.txt").is_file());
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
        assert_eq!(std::fs::read_to_string(dir.join(PROFILES_BACKUP)).unwrap(), text);
        let list = profiles::load(dir);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "WH | LDAC");
        assert!(list[0].codec_settings["aac"].abr);
        // Nothing left to fold; a later fold keeps the first backup
        assert_eq!(prepare(dir).profiles_folded, 0);
        let again = r#"{"profiles":[{"name":"A","device_address":"00:A2:D0:00:00:02"},{"name":"B","device_address":"00:A2:D0:00:00:02"}]}"#;
        std::fs::write(profiles::path(dir), again).unwrap();
        assert_eq!(prepare(dir).profiles_folded, 1);
        assert_eq!(std::fs::read_to_string(dir.join(PROFILES_BACKUP)).unwrap(), text);
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
        assert!(!dir.join(PROFILES_BACKUP).exists());
    }
}
