//! The two windows: the main window (960×640) and the tray flyout (360 wide, as tall as its page,
//! frameless, near the notification area), and the rule that ends the UI
//! process: closing a window only hides it, so that reopening it from the
//! tray is instant, and the process exits after everything was hidden for
//! a few minutes (the core keeps streaming without it). With "Minimize to
//! System Tray" off, closing the main window exits BlueCaster, core included,
//! as the tray's Exit does (v1).

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::utils::config::WindowEffectsConfig;
use tauri::window::{Effect, EffectState};
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, Theme, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    WindowEvent,
};

pub const MAIN: &str = "main";
pub const FLYOUT: &str = "flyout";

/// Exit after the windows have been hidden this long
const HIDDEN_EXIT_AFTER: Duration = Duration::from_secs(180);
/// Gap between the flyout and the screen edge / taskbar (logical pixels)
const FLYOUT_MARGIN: f64 = 12.0;
/// The flyout's width, and its height until its page says how tall it is
/// (`fit_flyout`, issue #86: no empty space under the controls)
const FLYOUT_WIDTH: f64 = 360.0;
const FLYOUT_HEIGHT: f64 = 340.0;
/// The heights `fit_flyout` takes (logical pixels)
const FLYOUT_MIN_HEIGHT: f64 = 160.0;
const FLYOUT_MAX_HEIGHT: f64 = 560.0;
/// How long the flyout counts as opening after it was shown (or its page
/// appeared) (#140). Meanwhile another click on the tray icon (a double
/// click, or a second click while a cold start is still loading the page)
/// does not close it, and losing the focus does not hide it: the taskbar
/// can take the focus back just after a click on the icon.
const FLYOUT_OPENING: Duration = Duration::from_millis(800);

/// When the flyout was last shown or its page appeared
static FLYOUT_OPENED: Mutex<Option<Instant>> = Mutex::new(None);

fn flyout_opened_now() {
    *FLYOUT_OPENED.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
}

fn flyout_opening() -> bool {
    FLYOUT_OPENED.lock().unwrap_or_else(|e| e.into_inner()).is_some_and(|t| t.elapsed() < FLYOUT_OPENING)
}

/// The flyout's page has loaded: on a cold start this is when it appears
pub fn page_loaded(label: &str) {
    if label == FLYOUT {
        flyout_opened_now();
    }
}

/// Mica needs Windows 11 (build 22000). On Windows 10 a transparent window
/// would show the desktop through it, so there the windows stay opaque.
pub fn mica_supported() -> bool {
    windows_version::OsVersion::current().build >= 22000
}

fn mica() -> WindowEffectsConfig {
    WindowEffectsConfig {
        effects: vec![Effect::Mica],
        state: Some(EffectState::FollowsWindowActiveState),
        ..Default::default()
    }
}

fn builder<'a>(app: &'a AppHandle, label: &'a str) -> WebviewWindowBuilder<'a, tauri::Wry, AppHandle> {
    // One page for both windows; the frontend picks the view by the label
    let b = WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
        .title("BlueCaster")
        .visible(false)
        .theme(Some(Theme::Dark));
    if mica_supported() {
        b.transparent(true).effects(mica())
    } else {
        // The dark theme's background, so no white flash before the page
        b.background_color(tauri::window::Color(0x15, 0x17, 0x1c, 0xff))
    }
}

fn main_window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    if let Some(w) = app.get_webview_window(MAIN) {
        return Ok(w);
    }
    let w = builder(app, MAIN).inner_size(960.0, 640.0).min_inner_size(720.0, 520.0).center().build()?;
    crate::recovery::watch(&w);
    Ok(w)
}

fn flyout_window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    if let Some(w) = app.get_webview_window(FLYOUT) {
        return Ok(w);
    }
    let w = builder(app, FLYOUT)
        .inner_size(FLYOUT_WIDTH, FLYOUT_HEIGHT)
        .decorations(false)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .shadow(true)
        .build()?;
    crate::recovery::watch(&w);
    Ok(w)
}

/// Shows the main window; `route` ("adapter", "settings", ...) opens that screen
pub fn show_main(app: &AppHandle, route: Option<String>) -> tauri::Result<()> {
    let w = main_window(app)?;
    if w.is_minimized().unwrap_or(false) {
        w.unminimize()?;
    }
    w.show()?;
    w.set_focus()?;
    tracing::info!("[{MAIN}] shown{}", route.as_deref().map(|r| format!(" at {r}")).unwrap_or_default());
    crate::logging::probe(app, &w);
    if let Some(route) = route {
        let _ = w.emit_to(MAIN, "ui://navigate", route);
    }
    if let Some(f) = app.get_webview_window(FLYOUT) {
        let _ = f.hide();
    }
    Ok(())
}

/// Shows the flyout above the notification area (bottom right of the work
/// area, where the taskbar's tray is in the default layout)
pub fn show_flyout(app: &AppHandle) -> tauri::Result<()> {
    let w = flyout_window(app)?;
    place_flyout(&w)?;
    flyout_opened_now();
    w.show()?;
    w.set_focus()?;
    tracing::info!("[{FLYOUT}] shown");
    crate::logging::probe(app, &w);
    Ok(())
}

/// Bottom right of the work area, above the notification area
fn place_flyout(w: &WebviewWindow) -> tauri::Result<()> {
    let monitor = w.primary_monitor()?.or(w.current_monitor()?);
    if let Some(m) = monitor {
        let area = m.work_area();
        let size = w.outer_size()?;
        let margin = (FLYOUT_MARGIN * m.scale_factor()).round() as i32;
        let x = area.position.x + area.size.width as i32 - size.width as i32 - margin;
        let y = area.position.y + area.size.height as i32 - size.height as i32 - margin;
        w.set_position(PhysicalPosition::new(x.max(area.position.x), y.max(area.position.y)))?;
    }
    Ok(())
}

/// The flyout's page is `height` tall (logical pixels): the window takes
/// that height, its bottom staying above the notification area
pub fn fit_flyout(w: &WebviewWindow, height: f64) -> tauri::Result<()> {
    if w.label() != FLYOUT || !height.is_finite() {
        return Ok(());
    }
    let height = height.clamp(FLYOUT_MIN_HEIGHT, FLYOUT_MAX_HEIGHT).ceil();
    let scale = w.scale_factor()?;
    let now = w.inner_size()?.to_logical::<f64>(scale);
    if (now.height - height).abs() < 1.0 {
        return Ok(());
    }
    w.set_size(LogicalSize::new(FLYOUT_WIDTH, height))?;
    place_flyout(w)
}

pub fn toggle_flyout(app: &AppHandle) -> tauri::Result<()> {
    match app.get_webview_window(FLYOUT) {
        Some(w) if w.is_visible().unwrap_or(false) => {
            if flyout_opening() {
                tracing::info!("[{FLYOUT}] still opening; stays");
                w.set_focus()
            } else {
                w.hide()
            }
        }
        _ => show_flyout(app),
    }
}

/// Close hides (see the module comment); the flyout also hides when it
/// loses the focus, as the system flyouts do
pub fn on_window_event(window: &tauri::Window, event: &WindowEvent) {
    match event {
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            let _ = window.hide();
            if window.label() == MAIN {
                main_closed(window);
            }
        }
        WindowEvent::Focused(false) if window.label() == FLYOUT => {
            if flyout_opening() {
                tracing::info!("[{FLYOUT}] lost the focus while opening; taking it back");
                let _ = window.set_focus();
            } else {
                let _ = window.hide();
            }
        }
        // Gone for good (the system closed it): its subscriptions end
        WindowEvent::Destroyed => {
            tracing::warn!("[{}] window destroyed", window.label());
            if let Some(ipc) = window.try_state::<std::sync::Arc<crate::connection::Ipc>>() {
                ipc.drop_window(window.label());
            }
        }
        _ => {}
    }
}

/// The main window was closed (it is hidden already). With "Minimize to
/// System Tray" the core shows its one-time "still running in the tray"
/// balloon (minimizing does the same, in the frontend); without it the core
/// is asked to exit, and its Bye ends this process (connection.rs).
fn main_closed(window: &tauri::Window) {
    let Some(ipc) = window.try_state::<std::sync::Arc<crate::connection::Ipc>>() else {
        return;
    };
    let ipc = ipc.inner().clone();
    tauri::async_runtime::spawn(async move {
        // Not connected: the setting is not known, the window stays hidden
        // (and the process ends when idle)
        let Ok(snap) = ipc.request(MAIN, "state.snapshot", None).await else { return };
        if quits_on_close(&snap) {
            let _ = ipc.request(MAIN, "app.quit", None).await;
        } else {
            let _ = ipc.request(MAIN, "app.hidden_to_tray", None).await;
        }
    });
}

/// Closing the main window exits BlueCaster: "Minimize to System Tray" is off
/// in the core's snapshot
fn quits_on_close(snapshot: &serde_json::Value) -> bool {
    snapshot.pointer("/settings/minimize_to_tray").and_then(serde_json::Value::as_bool) == Some(false)
}

/// A second `bluecaster.exe` (from the tray or the Start menu) only forwards
/// its arguments here
pub fn on_second_instance(app: &AppHandle, args: &[String]) {
    let flyout = args.iter().any(|a| a == "--flyout");
    let app = app.clone();
    // Off the event loop's own callback: creating a window from there can
    // block on the loop it runs in
    tauri::async_runtime::spawn(async move {
        let _ = if flyout { toggle_flyout(&app) } else { show_main(&app, None) };
    });
}

/// Ends the process once no window has been visible for HIDDEN_EXIT_AFTER
pub fn start_idle_exit(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut hidden_since: Option<Instant> = None;
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            let any_visible = app.webview_windows().values().any(|w| w.is_visible().unwrap_or(false));
            // A downloaded update waits in this process for the stream to end
            let updating = app.try_state::<std::sync::Arc<crate::update::Updates>>().is_some_and(|u| u.busy());
            if any_visible || updating {
                hidden_since = None;
                continue;
            }
            let since = *hidden_since.get_or_insert_with(Instant::now);
            if since.elapsed() >= HIDDEN_EXIT_AFTER {
                app.exit(0);
                return;
            }
        }
    });
}

/// Title bar and Mica follow the app's theme ("system": Windows' own)
pub fn set_theme(app: &AppHandle, theme: &str) {
    // As the page reads it: anything but light and system is dark
    let t = match theme {
        "light" => Some(Theme::Light),
        "system" => None,
        _ => Some(Theme::Dark),
    };
    for w in app.webview_windows().values() {
        let _ = w.set_theme(t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_quits_only_without_minimize_to_tray() {
        let snap = |v: serde_json::Value| serde_json::json!({ "settings": { "minimize_to_tray": v } });
        assert!(quits_on_close(&snap(false.into())));
        assert!(!quits_on_close(&snap(true.into())));
        // Unknown: the window only hides
        assert!(!quits_on_close(&serde_json::json!({})));
    }
}
