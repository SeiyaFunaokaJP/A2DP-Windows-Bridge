//! Recovery from WebView2's own processes dying under a window. A DLL that
//! other software injects into every process can crash them (RivaTuner's
//! RTSSHooks64.dll took down the browser process 3 s after start on
//! 2026-10-22): the window then stays black, and every call into its
//! webview fails with 0x8007139F.
//!
//! WebView2 reports this (ProcessFailed). A renderer that exited or hangs
//! is reloaded; a browser process that exited cannot be brought back in
//! this process, so a2dpwb.exe starts itself again with the window that
//! was shown. The core keeps streaming meanwhile. Restarts that follow each
//! other quickly are counted (in the environment the new process inherits),
//! so a crash on every start ends the UI instead of looping; the next click
//! on the tray icon starts it afresh.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Manager, WebviewWindow};
use win_sys::webview::ProcessFailed;

use crate::windows::{FLYOUT, MAIN};

/// "<restarts>,<unix seconds of the first>" for the restarts in a row
const RESTARTS_ENV: &str = "A2DPWB_UI_RESTARTS";
/// Restarts closer together than this count as in a row...
const RESTART_SPAN: Duration = Duration::from_secs(120);
/// ...and after this many the UI ends instead
const MAX_RESTARTS: u32 = 2;

/// The arguments and the RESTARTS_ENV value to start again with, set when
/// the browser process exited
static RELAUNCH: Mutex<Option<(Vec<String>, String)>> = Mutex::new(None);

/// Watches the window's webview processes (call once, after the window is built)
pub fn watch(window: &WebviewWindow) {
    let label = window.label().to_string();
    let app = window.app_handle().clone();
    let result = window.with_webview(move |webview| {
        let added =
            win_sys::webview::on_process_failed(&webview.controller(), move |kind| on_failed(&app, &label, kind));
        if let Err(e) = added {
            tracing::warn!("cannot watch the webview's processes: {e}");
        }
    });
    if let Err(e) = result {
        tracing::warn!("[{}] cannot watch the webview's processes: {e}", window.label());
    }
}

fn on_failed(app: &AppHandle, label: &str, kind: ProcessFailed) {
    match kind {
        ProcessFailed::BrowserExited => browser_exited(app),
        ProcessFailed::RendererExited | ProcessFailed::RendererUnresponsive => {
            let what = if kind == ProcessFailed::RendererExited { "exited" } else { "hangs" };
            tracing::warn!("[{label}] the page's renderer process {what}; reloading the page");
            if let Some(w) = app.get_webview_window(label)
                && let Err(e) = w.reload()
            {
                tracing::warn!("[{label}] cannot reload the page: {e}");
            }
        }
        // GPU, utility and the others: WebView2 starts them again itself
        ProcessFailed::Other(kind) => {
            tracing::info!("[{label}] a WebView2 process exited (kind {kind}); WebView2 restarts it")
        }
    }
}

/// The browser process serves all windows: each reports its exit, the first
/// one acts
fn browser_exited(app: &AppHandle) {
    static ONCE: AtomicBool = AtomicBool::new(false);
    if ONCE.swap(true, Ordering::SeqCst) {
        return;
    }
    let shown = |label| app.get_webview_window(label).is_some_and(|w| w.is_visible().unwrap_or(false));
    let args = if shown(MAIN) {
        Some(Vec::new())
    } else if shown(FLYOUT) {
        Some(vec!["--flyout".to_string()])
    } else {
        None
    };
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let (restarts, first) = next_restart(std::env::var(RESTARTS_ENV).ok().as_deref(), now);
    match args {
        None => tracing::error!("the WebView2 browser process exited (no window shown); ending the UI"),
        Some(_) if restarts > MAX_RESTARTS => tracing::error!(
            "the WebView2 browser process exited again ({} restarts within {} s); ending the UI, the tray icon starts it again",
            restarts - 1,
            RESTART_SPAN.as_secs()
        ),
        Some(args) => {
            tracing::error!(
                "the WebView2 browser process exited (window black); starting a2dpwb again ({restarts}/{MAX_RESTARTS})"
            );
            *RELAUNCH.lock().unwrap_or_else(|e| e.into_inner()) = Some((args, format!("{restarts},{first}")));
        }
    }
    app.exit(0);
}

/// The count and start of the restarts in a row, this one included
fn next_restart(previous: Option<&str>, now: u64) -> (u32, u64) {
    let parsed = previous.and_then(|v| {
        let (n, t) = v.split_once(',')?;
        Some((n.parse::<u32>().ok()?, t.parse::<u64>().ok()?))
    });
    match parsed {
        Some((n, first)) if now.saturating_sub(first) < RESTART_SPAN.as_secs() => (n + 1, first),
        _ => (1, now),
    }
}

/// On RunEvent::Exit, after the single-instance plugin let go of its lock
/// (plugins see the event first): starts the new a2dpwb.exe if one is due
pub fn relaunch_if_due() {
    let Some((args, restarts)) = RELAUNCH.lock().unwrap_or_else(|e| e.into_inner()).take() else { return };
    let started = std::env::current_exe()
        .and_then(|exe| std::process::Command::new(exe).args(&args).env(RESTARTS_ENV, restarts).spawn());
    if let Err(e) = started {
        tracing::error!("cannot start a2dpwb again: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restarts_in_a_row_are_counted() {
        assert_eq!(next_restart(None, 1000), (1, 1000));
        assert_eq!(next_restart(Some("1,1000"), 1030), (2, 1000));
        assert_eq!(next_restart(Some("2,1000"), 1119), (3, 1000));
        // Long enough apart: a new series
        assert_eq!(next_restart(Some("2,1000"), 1120), (1, 1120));
        assert_eq!(next_restart(Some("garbage"), 1000), (1, 1000));
    }
}
