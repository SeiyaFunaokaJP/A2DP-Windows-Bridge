//! The crash reports the UI shows until the user dismisses them: the
//! `.txt` files win_sys::crash writes into `logs\crash\` for a panic or a
//! crash of the core or the UI (`<process>-YYYYMMDD-HHMMSS-mmm.txt`).
//!
//! The newest dismissed report's stamp is kept in settings.json
//! (`crash_seen`); only newer ones are shown. Without it (the first start
//! of a version that shows them) only reports of the last days are, not
//! one from long ago.

use std::path::Path;
use std::time::{Duration, SystemTime};

use a2dpwb_core::paths;
use a2dpwb_core::settings::Settings;
use ipc::CrashReport;
use serde_json::Value;

const SEEN_KEY: &str = "crash_seen";
/// Without a dismissed report: reports this recent are shown
const RECENT: Duration = Duration::from_secs(7 * 24 * 3600);
/// At most this many are listed (win_sys::crash keeps 5 per process)
const MAX_LISTED: usize = 3;
/// A report's text is cut to this many characters (it goes into a URL)
const MAX_TEXT: usize = 4000;

/// The reports not dismissed yet, newest first
pub fn unseen(config: &Path, settings: &Settings) -> Vec<CrashReport> {
    let seen = settings.other.get(SEEN_KEY).and_then(Value::as_str);
    let Ok(entries) = std::fs::read_dir(paths::crash_dir(config)) else { return Vec::new() };
    let now = SystemTime::now();
    let mut found: Vec<(String, CrashReport)> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            let name = path.file_stem()?.to_str()?.to_owned();
            if path.extension()? != "txt" {
                return None;
            }
            let (process, stamp) = split_name(&name)?;
            let new = match seen {
                Some(seen) => stamp > seen,
                None => e
                    .metadata()
                    .and_then(|m| m.modified())
                    .is_ok_and(|t| now.duration_since(t).unwrap_or_default() < RECENT),
            };
            if !new {
                return None;
            }
            let text = std::fs::read_to_string(&path).ok()?;
            Some((stamp.to_owned(), report(&name, process, stamp, &text)))
        })
        .collect();
    found.sort_by(|a, b| b.0.cmp(&a.0));
    found.into_iter().take(MAX_LISTED).map(|(_, r)| r).collect()
}

/// Reports up to `name` (the newest shown) were seen
pub fn mark_seen(settings: &mut Settings, name: &str) {
    let Some((_, stamp)) = split_name(name) else { return };
    let newer = settings.other.get(SEEN_KEY).and_then(Value::as_str).is_none_or(|s| stamp > s);
    if newer {
        settings.other.insert(SEEN_KEY.into(), Value::String(stamp.into()));
    }
}

/// "a2dpwb-core-20261022-012338-512" -> ("a2dpwb-core", "20261022-012338-512")
fn split_name(name: &str) -> Option<(&str, &str)> {
    // The stamp is the last 19 characters, after a '-'
    let at = name.len().checked_sub(20)?;
    let (process, rest) = name.split_at_checked(at)?;
    let stamp = rest.strip_prefix('-')?;
    let b = stamp.as_bytes();
    let digits = |r: std::ops::Range<usize>| b[r].iter().all(u8::is_ascii_digit);
    (b.len() == 19
        && digits(0..8)
        && b[8] == b'-'
        && digits(9..15)
        && b[15] == b'-'
        && digits(16..19)
        && !process.is_empty())
    .then_some((process, stamp))
}

/// The report's fields: its third line is the reason (win_sys::crash)
fn report(name: &str, process: &str, stamp: &str, text: &str) -> CrashReport {
    let time = format!(
        "{}-{}-{} {}:{}:{}",
        &stamp[0..4],
        &stamp[4..6],
        &stamp[6..8],
        &stamp[9..11],
        &stamp[11..13],
        &stamp[13..15]
    );
    let reason = text.lines().nth(2).unwrap_or_default().trim().to_owned();
    let cut = text.chars().count() > MAX_TEXT;
    let mut text: String = text.chars().take(MAX_TEXT).collect();
    if cut {
        text.push_str("\n... (cut: the whole report is in logs\\crash)");
    }
    CrashReport { name: name.into(), process: process.into(), time, reason, text }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_split_into_process_and_stamp() {
        assert_eq!(split_name("a2dpwb-core-20261022-012338-512"), Some(("a2dpwb-core", "20261022-012338-512")));
        assert_eq!(split_name("a2dpwb-ui-20261022-012338-512"), Some(("a2dpwb-ui", "20261022-012338-512")));
        assert_eq!(split_name("notes"), None);
        assert_eq!(split_name("a2dpwb-core-2026102x-012338-512"), None);
        assert_eq!(split_name("-20261022-012338-512"), None);
        // Another file in the folder, not ASCII: no panic
        assert_eq!(split_name("メモメモメモメモメモメモメモ"), None);
    }

    #[test]
    fn only_reports_newer_than_the_dismissed_one_are_listed() {
        let dir = std::env::temp_dir().join(format!("a2dpwb-crashes-{}", std::process::id()));
        let crash = paths::crash_dir(&dir);
        std::fs::create_dir_all(&crash).unwrap();
        let write = |name: &str, reason: &str| {
            std::fs::write(
                crash.join(format!("{name}.txt")),
                format!("a2dpwb-core 2.0.2\nprocess 1 thread 2\n{reason}\nno dump\n"),
            )
            .unwrap();
        };
        write("a2dpwb-core-20261020-100000-000", "panic in thread main: old");
        write("a2dpwb-ui-20261022-012338-512", "panic in thread main: ui");
        write("a2dpwb-core-20261022-090000-000", "unhandled exception 0xC0000005 at 0x1");
        std::fs::write(crash.join("a2dpwb-core-20261022-090000-000.dmp"), b"x").unwrap();

        let mut s = Settings::default();
        // Never dismissed: all recent ones (the files were just written)
        let all = unseen(&dir, &s);
        assert_eq!(
            all.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
            ["a2dpwb-core-20261022-090000-000", "a2dpwb-ui-20261022-012338-512", "a2dpwb-core-20261020-100000-000"]
        );
        assert_eq!(all[0].process, "a2dpwb-core");
        assert_eq!(all[0].time, "2026-10-22 09:00:00");
        assert_eq!(all[0].reason, "unhandled exception 0xC0000005 at 0x1");

        mark_seen(&mut s, "a2dpwb-ui-20261022-012338-512");
        let left = unseen(&dir, &s);
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].name, "a2dpwb-core-20261022-090000-000");
        // An older name does not move the mark back
        mark_seen(&mut s, "a2dpwb-core-20261020-100000-000");
        assert_eq!(unseen(&dir, &s).len(), 1);
        mark_seen(&mut s, &left[0].name);
        assert!(unseen(&dir, &s).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
