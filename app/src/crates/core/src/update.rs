//! Update check: the newest release on GitHub — v1's update_checker.cpp.
//!
//! Stable channel: `/releases/latest` (GitHub never returns a pre-release
//! there). Beta channel (`include_prereleases`): the highest version among
//! the last 20 published releases, pre-release or not. Versions compare with
//! semver precedence, so 2.0.0-beta.1 < 2.0.0-beta.10 < 2.0.0-rc.1 < 2.0.0.
//!
//! Blocking (ureq with rustls; the system proxy as v1's WinHTTP): call from a
//! worker thread. The automatic check runs at most once a day
//! ([`automatic_check_due`] with Settings.last_update_check).

use std::cmp::Ordering;
use std::time::Duration;

use semver::Version;
use serde::Serialize;
use serde_json::Value;

use crate::settings::Settings;

pub const REPO: &str = "SeiyaFunaokaJP/A2DP-Windows-Bridge";
/// Releases page: the link when the API is unreachable
pub const RELEASES_PAGE: &str = "https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/releases";

const API: &str = "https://api.github.com/repos/SeiyaFunaokaJP/A2DP-Windows-Bridge/releases";
/// Newest first; 20 reaches back past any run of pre-releases
const LIST_PER_PAGE: u32 = 20;
/// /releases/latest is a few kB, 20 list entries some 100 kB
const MAX_RESPONSE: u64 = 1 << 20;
const TIMEOUT: Duration = Duration::from_secs(20);

/// A published release
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Release {
    /// e.g. "2.0.1" (the tag without its leading 'v')
    pub version: String,
    /// The tag as published, e.g. "v2.0.1"
    pub tag: String,
    /// Its release page (html_url)
    pub url: String,
    pub prerelease: bool,
    /// What the app shows of its notes ([`in_app_notes`]); empty without
    pub notes: String,
}

#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("HTTP status {0}")]
    Status(u16),
    #[error("request failed: {0}")]
    Http(String),
    #[error("unexpected response: {0}")]
    Response(String),
}

/// Parses a version leniently: a leading 'v'/'V', and a core of one or two
/// numbers ("1.2" = 1.2.0), as tags are sometimes written. Build metadata is
/// kept but does not count for comparisons ([`compare_versions`]).
pub fn parse_version(s: &str) -> Option<Version> {
    let s = s.trim();
    let s = s.strip_prefix(['v', 'V']).unwrap_or(s);
    if let Ok(v) = Version::parse(s) {
        return Some(v);
    }
    // Pad a short core: "1.2-beta.1+x" -> "1.2.0-beta.1+x"
    let core_end = s.find(['-', '+']).unwrap_or(s.len());
    let (core, rest) = s.split_at(core_end);
    let parts: Vec<&str> = core.split('.').collect();
    if parts.is_empty()
        || parts.len() > 3
        || parts.iter().any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    let mut padded = parts.join(".");
    for _ in parts.len()..3 {
        padded.push_str(".0");
    }
    Version::parse(&format!("{padded}{rest}")).ok()
}

/// Semver precedence (build metadata ignored); a version that does not
/// parse ranks below every one that does
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    match (parse_version(a), parse_version(b)) {
        (Some(a), Some(b)) => a.cmp_precedence(&b),
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        (None, None) => Ordering::Equal,
    }
}

/// `candidate` is a newer version than `current`
pub fn is_newer(candidate: &str, current: &str) -> bool {
    parse_version(candidate).is_some() && compare_versions(candidate, current) == Ordering::Greater
}

fn release_of(j: &Value) -> Option<Release> {
    let tag = j.get("tag_name")?.as_str()?.trim();
    if tag.is_empty() {
        return None;
    }
    let version = tag.strip_prefix(['v', 'V']).unwrap_or(tag).to_string();
    let url = j.get("html_url").and_then(Value::as_str).filter(|u| !u.is_empty()).unwrap_or(RELEASES_PAGE).to_string();
    let prerelease = j
        .get("prerelease")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| parse_version(&version).is_some_and(|v| !v.pre.is_empty()));
    let notes = in_app_notes(j.get("body").and_then(Value::as_str).unwrap_or_default());
    Some(Release { version, tag: tag.to_string(), url, prerelease, notes })
}

/// Marks the part of a release's notes the app shows with its update
/// (issue #86: a few lines, the rest on the release page). HTML comments, so
/// the page shows the text as part of the notes.
pub const IN_APP_NOTES_START: &str = "<!-- in-app -->";
pub const IN_APP_NOTES_END: &str = "<!-- /in-app -->";
/// The longest notes shown in the app (characters); longer ones are cut
const IN_APP_NOTES_MAX: usize = 1000;

/// The part of a release body between [`IN_APP_NOTES_START`] and
/// [`IN_APP_NOTES_END`] (trimmed, CRLF as LF, cut at [`IN_APP_NOTES_MAX`]);
/// empty when the body has none
pub fn in_app_notes(body: &str) -> String {
    let Some((_, rest)) = body.split_once(IN_APP_NOTES_START) else { return String::new() };
    let notes = rest.split_once(IN_APP_NOTES_END).map_or(rest, |(n, _)| n);
    let notes = notes.replace("\r\n", "\n");
    let notes = notes.trim();
    match notes.char_indices().nth(IN_APP_NOTES_MAX) {
        Some((cut, _)) => format!("{}…", notes[..cut].trim_end()),
        None => notes.to_string(),
    }
}

/// The updater manifest of a release: every release (pre-release or not)
/// carries its own latest.json (installer/build-release.ps1), so the
/// pre-release channel never sees a manifest of the stable one's and the
/// other way round
pub fn manifest_url(tag: &str) -> String {
    format!("https://github.com/{REPO}/releases/download/{tag}/latest.json")
}

/// The answer of `/releases/latest`
pub fn parse_latest(json: &str) -> Result<Release, UpdateError> {
    let j: Value = serde_json::from_str(json).map_err(|e| UpdateError::Response(format!("JSON: {e}")))?;
    release_of(&j).ok_or_else(|| UpdateError::Response("no tag_name".into()))
}

/// The answer of `/releases?per_page=n`: the highest version among the
/// published releases, pre-release or not (drafts are listed only to
/// maintainers; tags that are no version are skipped)
pub fn parse_list(json: &str) -> Result<Release, UpdateError> {
    let j: Value = serde_json::from_str(json).map_err(|e| UpdateError::Response(format!("JSON: {e}")))?;
    let list = j.as_array().ok_or_else(|| UpdateError::Response("not a list".into()))?;
    list.iter()
        .filter(|r| !r.get("draft").and_then(Value::as_bool).unwrap_or(false))
        .filter_map(release_of)
        .filter_map(|r| parse_version(&r.version).map(|v| (v, r)))
        .max_by(|(a, _), (b, _)| a.cmp_precedence(b))
        .map(|(_, r)| r)
        .ok_or_else(|| UpdateError::Response("no releases".into()))
}

fn get(url: &str) -> Result<String, UpdateError> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .user_agent(concat!("A2DPWB/", env!("CARGO_PKG_VERSION")))
        .build()
        .into();
    let started = std::time::Instant::now();
    tracing::debug!("update check: GET {url}");
    let mut response = agent
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .call()
        .map_err(|e| {
            tracing::debug!("update check: request failed after {} ms: {e}", started.elapsed().as_millis());
            match e {
                ureq::Error::StatusCode(code) => UpdateError::Status(code),
                e => UpdateError::Http(e.to_string()),
            }
        })?;
    tracing::debug!("update check: status {} in {} ms", response.status(), started.elapsed().as_millis());
    let body = response
        .body_mut()
        .with_config()
        .limit(MAX_RESPONSE)
        .read_to_string()
        .map_err(|e| UpdateError::Http(e.to_string()));
    tracing::debug!("update check: body {:?}", body.as_ref().map(String::len));
    body
}

/// The newest release of the channel (blocking)
pub fn latest_release(include_prereleases: bool) -> Result<Release, UpdateError> {
    if include_prereleases {
        parse_list(&get(&format!("{API}?per_page={LIST_PER_PAGE}"))?)
    } else {
        parse_latest(&get(&format!("{API}/latest"))?)
    }
}

/// A release newer than `current_version`, if there is one (blocking)
pub fn check(current_version: &str, include_prereleases: bool) -> Result<Option<Release>, UpdateError> {
    tracing::debug!("update check: running {current_version}, pre-releases {include_prereleases}");
    let latest = latest_release(include_prereleases)?;
    tracing::info!(
        "update check: latest {}{} (running {current_version})",
        latest.version,
        if latest.prerelease { " (pre-release)" } else { "" }
    );
    tracing::debug!(
        "update check: {} ({}) vs {current_version}: {:?}, newer {}",
        latest.tag,
        latest.url,
        compare_versions(&latest.version, current_version),
        is_newer(&latest.version, current_version)
    );
    Ok(is_newer(&latest.version, current_version).then_some(latest))
}

/// The running version (the workspace's)
pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Today's local date, ISO ("2026-10-17"), as v1 kept it in
/// Settings.last_update_check
pub fn today() -> String {
    let now = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    format!("{:04}-{:02}-{:02}", now.year(), u8::from(now.month()), now.day())
}

/// The automatic check is on and has not run today. The caller then sets
/// `last_update_check = today` and saves before checking, so a failing
/// check is not retried at every start either (as v1).
pub fn automatic_check_due(settings: &Settings, today: &str) -> bool {
    settings.check_updates_on_startup && settings.last_update_check != today
}

#[cfg(test)]
mod tests {
    use super::*;
    use semver::BuildMetadata;

    #[test]
    fn version_order() {
        let ordered = [
            "0.9",
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "v1.0.0",
            "1.0.2",
            "1.1.0-beta.2",
            "1.1.0",
            "1.10.0",
            "2.0.0-alpha.1",
            "2.0.0-beta.1",
            "2.0.0-beta.10",
            "2.0.0",
        ];
        for w in ordered.windows(2) {
            assert_eq!(compare_versions(w[0], w[1]), Ordering::Less, "{} < {}", w[0], w[1]);
            assert_eq!(compare_versions(w[1], w[0]), Ordering::Greater, "{} > {}", w[1], w[0]);
        }
        assert_eq!(compare_versions("v1.1.0", "1.1.0"), Ordering::Equal);
        assert_eq!(compare_versions("1.1.0+abc", "1.1.0+def"), Ordering::Equal);
        assert_eq!(compare_versions("1.2", "1.2.0"), Ordering::Equal);
        assert_eq!(compare_versions("1", "1.0.0"), Ordering::Equal);
        assert_eq!(compare_versions("nightly", "0.0.1"), Ordering::Less);
        assert!(is_newer("2.0.0", "2.0.0-beta.1"));
        assert!(is_newer("v2.0.0-beta.2", "2.0.0-beta.1"));
        assert!(!is_newer("2.0.0-beta.1", "2.0.0"));
        assert!(!is_newer("2.0.0", "2.0.0"));
        assert!(!is_newer("garbage", "1.0.0"));
        assert!(parse_version("1.2.3.4").is_none());
        assert!(parse_version("1..2").is_none());
    }

    #[test]
    fn parses_latest() {
        let json = r#"{"url":"https://api.github.com/repos/x/y/releases/1","html_url":
            "https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/releases/tag/v1.1.0","tag_name":"v1.1.0",
            "name":"A2DPWB 1.1.0","draft":false,"prerelease":false,"assets":[]}"#;
        let r = parse_latest(json).unwrap();
        assert_eq!(
            r,
            Release {
                version: "1.1.0".into(),
                tag: "v1.1.0".into(),
                url: "https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/releases/tag/v1.1.0".into(),
                prerelease: false,
                notes: String::new(),
            }
        );
        assert_eq!(
            manifest_url(&r.tag),
            "https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/releases/download/v1.1.0/latest.json"
        );
        // No html_url: the releases page
        assert_eq!(parse_latest(r#"{"tag_name":"v1.0.0"}"#).unwrap().url, RELEASES_PAGE);
        assert!(parse_latest(r#"{"message":"Not Found"}"#).is_err());
        assert!(parse_latest("<html>").is_err());
    }

    #[test]
    fn parses_list_with_prereleases() {
        let json = r#"[
            {"tag_name":"v2.0.0-beta.10","html_url":"u10","draft":false,"prerelease":true},
            {"tag_name":"v2.1.0","html_url":"draft","draft":true,"prerelease":false},
            {"tag_name":"v2.0.0-beta.2","html_url":"u2","draft":false,"prerelease":true},
            {"tag_name":"le-audio-preview","html_url":"x","draft":false,"prerelease":true},
            {"tag_name":"v1.1.0","html_url":"u110","draft":false,"prerelease":false}
        ]"#;
        let r = parse_list(json).unwrap();
        assert_eq!((r.version.as_str(), r.url.as_str(), r.prerelease), ("2.0.0-beta.10", "u10", true));

        // The release outranks its pre-releases
        let json = r#"[{"tag_name":"v2.0.0-rc.1","prerelease":true},{"tag_name":"v2.0.0","prerelease":false}]"#;
        let r = parse_list(json).unwrap();
        assert_eq!((r.version.as_str(), r.prerelease), ("2.0.0", false));

        assert!(parse_list("[]").is_err());
        assert!(parse_list(r#"{"tag_name":"v1.0.0"}"#).is_err());
    }

    #[test]
    fn in_app_notes_are_the_marked_part() {
        let body = "Intro\r\n\r\n<!-- in-app -->\r\n- Faster scan\r\n- A toast when it connects\r\n<!-- /in-app -->\r\n\r\n## Changes\r\n- more";
        assert_eq!(in_app_notes(body), "- Faster scan\n- A toast when it connects");
        let r = parse_latest(&serde_json::json!({ "tag_name": "v2.0.3", "body": body }).to_string()).unwrap();
        assert_eq!(r.notes, "- Faster scan\n- A toast when it connects");
        // None marked: no notes; no end: to the end of the body
        assert_eq!(in_app_notes("## Changes\n- a"), "");
        assert_eq!(in_app_notes("<!-- in-app -->\nonly this"), "only this");
        // Cut at 1000 characters
        let long = format!("{IN_APP_NOTES_START}{}{IN_APP_NOTES_END}", "é".repeat(1200));
        assert_eq!(in_app_notes(&long).chars().count(), 1001);
    }

    #[test]
    fn prerelease_flag_falls_back_to_the_version() {
        assert!(parse_latest(r#"{"tag_name":"v2.0.0-beta.1"}"#).unwrap().prerelease);
        assert!(!parse_latest(r#"{"tag_name":"v2.0.0"}"#).unwrap().prerelease);
    }

    #[test]
    fn daily_check() {
        let mut s = Settings::default();
        let today = today();
        assert_eq!(today.len(), 10);
        assert_eq!(&today[4..5], "-");
        assert!(automatic_check_due(&s, &today));
        s.last_update_check = today.clone();
        assert!(!automatic_check_due(&s, &today));
        assert!(automatic_check_due(&s, "2099-01-01"));
        s.check_updates_on_startup = false;
        assert!(!automatic_check_due(&s, "2099-01-01"));
    }

    /// Really asks GitHub: `cargo test -p a2dpwb-core update -- --ignored`
    #[test]
    #[ignore = "network"]
    fn queries_github() {
        let stable = latest_release(false).unwrap();
        assert!(parse_version(&stable.version).is_some(), "{stable:?}");
        assert!(!stable.prerelease);
        assert!(stable.url.starts_with("https://github.com/"));
        let beta = latest_release(true).unwrap();
        eprintln!(
            "stable channel: {stable:?}
beta channel: {beta:?}"
        );
        assert_ne!(compare_versions(&beta.version, &stable.version), Ordering::Less, "{beta:?} {stable:?}");
        // Whatever is out, the running alpha is not newer than itself
        let _ = check(current_version(), true).unwrap();
    }

    #[test]
    fn build_metadata_is_not_precedence() {
        let mut v = parse_version("1.0.0+1").unwrap();
        assert_ne!(v.build, BuildMetadata::EMPTY);
        v.build = BuildMetadata::EMPTY;
        assert_eq!(v, parse_version("1.0.0").unwrap());
    }
}
