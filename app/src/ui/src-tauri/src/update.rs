//! In-app updates with tauri-plugin-updater. The core's daily check finds
//! the release (and its manifest, latest.json); here the installer is
//! downloaded and its signature checked against the key built into this
//! program, then installed once nothing streams:
//!
//! 1. download (the progress goes to the windows as `update://phase`);
//! 2. ask the core `app.quit_for_update`. It refuses while it streams or
//!    connects, and the request is repeated every few seconds until it
//!    agrees: an update never cuts the audio;
//! 3. wait until the core has exited (stream stopped, adapter released),
//!    without starting it again or ending this process on its Bye;
//! 4. run the installer (Inno Setup, `/UPDATE /SILENT`: progress only, no
//!    questions); this process exits. The installer starts a2dpwb.exe again,
//!    which starts the core.
//!
//! Only an installed copy with an updater key updates itself. The portable
//! copy (no unins000.exe next to it) and builds without a key show the
//! release page instead (Phase::Unavailable).

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

use crate::connection::Ipc;

pub const PHASE_EVENT: &str = "update://phase";
/// Manifests are taken from this repository's releases only
const MANIFEST_PREFIX: &str = "https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/releases/download/";
/// Asking the core again while it streams
const RETRY_INTERVAL: Duration = Duration::from_secs(3);
/// Stopping a stream and resetting the adapter takes a few seconds
const CORE_EXIT_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Phase {
    /// No in-app update: "portable" (nothing installed to update) or
    /// "unsigned" (built without the updater key)
    Unavailable {
        reason: &'static str,
    },
    Idle,
    Downloading {
        version: String,
        downloaded: u64,
        total: Option<u64>,
    },
    /// Downloaded and verified; installed as soon as nothing streams
    Waiting {
        version: String,
    },
    Installing {
        version: String,
    },
    Failed {
        code: String,
        detail: String,
    },
}

pub struct Updates {
    phase: Mutex<Phase>,
    ready: Mutex<Option<(tauri_plugin_updater::Update, Vec<u8>)>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The updater key built into this program (tauri.conf.json's
/// plugins.updater.pubkey, set by installer/build-release.ps1)
fn has_pubkey(app: &AppHandle) -> bool {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|u| u.get("pubkey"))
        .and_then(|k| k.as_str())
        .is_some_and(|k| !k.trim().is_empty())
}

/// Installed by the Inno Setup installer (installer/A2DPWB.iss), which
/// leaves its uninstaller here
fn installed() -> bool {
    std::env::current_exe().map(|e| e.with_file_name("unins000.exe").is_file()).unwrap_or(false)
}

impl Updates {
    pub fn new(app: &AppHandle) -> Arc<Updates> {
        let phase = if !installed() {
            Phase::Unavailable { reason: "portable" }
        } else if !has_pubkey(app) {
            Phase::Unavailable { reason: "unsigned" }
        } else {
            Phase::Idle
        };
        Arc::new(Updates { phase: Mutex::new(phase), ready: Mutex::new(None) })
    }

    pub fn phase(&self) -> Phase {
        lock(&self.phase).clone()
    }

    /// Downloading or waiting to install: the process must stay
    pub fn busy(&self) -> bool {
        matches!(*lock(&self.phase), Phase::Downloading { .. } | Phase::Waiting { .. } | Phase::Installing { .. })
    }

    fn set(&self, app: &AppHandle, phase: Phase) {
        *lock(&self.phase) = phase.clone();
        let _ = app.emit(PHASE_EVENT, phase);
    }

    fn fail(&self, app: &AppHandle, code: &str, detail: impl ToString) {
        let detail = detail.to_string();
        eprintln!("a2dpwb: update failed ({code}): {detail}");
        self.set(app, Phase::Failed { code: code.into(), detail });
    }
}

/// Starts the update to the release whose manifest is `manifest_url`.
/// Ok also when one is already under way.
pub fn start(app: &AppHandle, manifest_url: String) -> Result<(), String> {
    let updates = app.state::<Arc<Updates>>().inner().clone();
    match updates.phase() {
        Phase::Unavailable { reason } => return Err(format!("no in-app update ({reason})")),
        Phase::Downloading { .. } | Phase::Waiting { .. } | Phase::Installing { .. } => return Ok(()),
        Phase::Idle | Phase::Failed { .. } => {}
    }
    if !manifest_url.starts_with(MANIFEST_PREFIX) || !manifest_url.ends_with("/latest.json") {
        return Err(format!("not a manifest of A2DPWB's releases: {manifest_url}"));
    }
    let url: tauri::Url = manifest_url.parse().map_err(|e| format!("{manifest_url}: {e}"))?;
    updates.set(app, Phase::Downloading { version: String::new(), downloaded: 0, total: None });
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err((code, detail)) = download(&app, &updates, url).await {
            updates.fail(&app, code, detail);
            return;
        }
        let ipc = app.state::<Arc<Ipc>>().inner().clone();
        loop {
            match install(&app, &updates, &ipc).await {
                Ok(true) => tokio::time::sleep(RETRY_INTERVAL).await,
                // install() returns only on failure: success ends the process
                Ok(false) => return,
                Err((code, detail)) => {
                    ipc.set_updating(false);
                    updates.fail(&app, code, detail);
                    return;
                }
            }
        }
    });
    Ok(())
}

type Failure = (&'static str, String);

async fn download(app: &AppHandle, updates: &Updates, url: tauri::Url) -> Result<(), Failure> {
    // The plugin runs an .exe as an NSIS installer: `/UPDATE` and, with
    // tauri.conf.json's installMode "basicUi", none of NSIS's own options.
    // Inno Setup's /SILENT is NSIS's passive mode; A2DPWB.iss starts
    // a2dpwb.exe again after an /UPDATE, so no /R /ARGS
    let updater = app
        .updater_builder()
        .installer_args(["/SILENT", "/NORESTART"])
        .restart_after_install(false)
        .endpoints(vec![url])
        .and_then(|b| b.build())
        .map_err(|e| ("update.config", e.to_string()))?;
    let update = updater.check().await.map_err(|e| ("update.check", e.to_string()))?;
    let Some(update) = update else {
        return Err(("update.not_newer", "the manifest has no newer version for this platform".into()));
    };
    let version = update.version.clone();
    let mut downloaded = 0u64;
    let mut last_emit = Instant::now();
    // download() also checks the signature against the built-in key
    let bytes = update
        .download(
            |chunk, total| {
                downloaded += chunk as u64;
                if last_emit.elapsed() >= Duration::from_millis(200) {
                    last_emit = Instant::now();
                    updates.set(app, Phase::Downloading { version: version.clone(), downloaded, total });
                }
            },
            || {},
        )
        .await
        .map_err(|e| ("update.download", e.to_string()))?;
    *lock(&updates.ready) = Some((update, bytes));
    updates.set(app, Phase::Waiting { version });
    Ok(())
}

/// Ok(true): the core streams, ask again later. Ok(false) cannot happen
/// on Windows (install ends the process) but ends the loop elsewhere.
async fn install(app: &AppHandle, updates: &Updates, ipc: &Ipc) -> Result<bool, Failure> {
    // From here a Bye of the core does not end this process, and the core
    // is not started again when its pipe goes away
    ipc.set_updating(true);
    match ipc.request("update", "app.quit_for_update", None).await {
        Ok(_) => {}
        Err(e) if e.code == "app.streaming" => {
            ipc.set_updating(false);
            return Ok(true);
        }
        // Not running (or gone while answering): nothing to stop
        Err(e) if e.code.starts_with("ipc.") => {}
        Err(e) => return Err(("update.core", e.code)),
    }
    let start = Instant::now();
    while win_sys::single_instance::is_held(&ipc::core_mutex_name()) {
        if start.elapsed() >= CORE_EXIT_TIMEOUT {
            return Err(("update.core_running", "a2dpwb-core.exe did not exit".into()));
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    let Some((update, bytes)) = lock(&updates.ready).take() else {
        return Err(("update.lost", "the downloaded installer is gone".into()));
    };
    updates.set(app, Phase::Installing { version: update.version.clone() });
    update.install(bytes).map_err(|e| ("update.install", e.to_string()))?;
    Ok(false)
}
