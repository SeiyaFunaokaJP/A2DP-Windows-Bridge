//! The two windows: the main window (960×640, 760×640 for the setup) and the tray flyout (360 wide, as tall as its page,
//! frameless, near the notification area), and the rule that ends the UI
//! process: closing a window only hides it, so that reopening it from the
//! tray is instant, and the process exits after everything was hidden for
//! a few minutes (the core keeps streaming without it). With "Minimize to
//! System Tray" off, closing the main window exits BlueCaster, core included,
//! as the tray's Exit does (v1). With its setting on, the flyout also opens
//! and closes with Windows 11's Quick Settings, above them.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tauri::utils::config::WindowEffectsConfig;
use tauri::window::{Effect, EffectState};
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, Theme, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    WindowEvent,
};
use win_sys::quick_settings::Panels;

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
/// The main window's size during the first-run setup (logical pixels)
const SETUP_WIDTH: f64 = 760.0;
/// The main window's minimum: Home keeps two columns of devices (issue #86)
const MAIN_MIN_WIDTH: f64 = 960.0;
const MAIN_MIN_HEIGHT: f64 = 520.0;
const SETUP_HEIGHT: f64 = 640.0;
/// How long the flyout counts as opening after it was shown (or its page
/// appeared) (#140). Meanwhile another click on the tray icon (a double
/// click, or a second click while a cold start is still loading the page)
/// does not close it, and losing the focus does not hide it: the taskbar
/// can take the focus back just after a click on the icon.
const FLYOUT_OPENING: Duration = Duration::from_millis(800);
/// How often the flyout checks on Quick Settings while it goes with them
const FOLLOW_EVERY: Duration = Duration::from_millis(150);
/// How often it looks for the pointer meanwhile
const POINTER_EVERY: Duration = Duration::from_millis(30);

/// When the flyout was last shown or its page appeared
static FLYOUT_OPENED: Mutex<Option<Instant>> = Mutex::new(None);
/// The flyout goes with Quick Settings, which are here
static QUICK_SETTINGS: Mutex<Option<Panels>> = Mutex::new(None);
/// The top of the panels and the cards on them, kept while the flyout is in
/// use: a card leaving then must not move the flyout under the pointer
static CARDS_TOP: Mutex<Option<i32>> = Mutex::new(None);
static FLYOUT_IN_USE: AtomicBool = AtomicBool::new(false);
/// The main window's size and whether it was maximized, while the setup has it
static BEFORE_SETUP: Mutex<Option<(LogicalSize<f64>, bool)>> = Mutex::new(None);

fn flyout_opened_now() {
    *FLYOUT_OPENED.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
}

fn quick_settings() -> Option<Panels> {
    *QUICK_SETTINGS.lock().unwrap_or_else(|e| e.into_inner())
}

fn set_quick_settings(panels: Option<Panels>) {
    *QUICK_SETTINGS.lock().unwrap_or_else(|e| e.into_inner()) = panels;
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
    let b =
        WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into())).title("BlueCaster").visible(false);
    if mica_supported() {
        b.transparent(true).effects(mica())
    } else {
        // The dark theme's background, so no white flash before the page
        // (a light page replaces it as quickly)
        b.background_color(tauri::window::Color(0x15, 0x17, 0x1c, 0xff))
    }
}

fn main_window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    if let Some(w) = app.get_webview_window(MAIN) {
        return Ok(w);
    }
    let w =
        builder(app, MAIN).inner_size(960.0, 640.0).min_inner_size(MAIN_MIN_WIDTH, MAIN_MIN_HEIGHT).center().build()?;
    set_icons(&w.as_ref().window());
    crate::recovery::watch(&w);
    Ok(w)
}

/// `focus`: its webview takes the focus as it is created, even in a hidden
/// window, which counts as `quick_settings::show_inactive` says
fn flyout_window(app: &AppHandle, focus: bool) -> tauri::Result<WebviewWindow> {
    if let Some(w) = app.get_webview_window(FLYOUT) {
        return Ok(w);
    }
    let w = builder(app, FLYOUT)
        .focused(focus)
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
    let w = flyout_window(app, true)?;
    w.set_focusable(true)?;
    place_flyout(&w)?;
    flyout_opened_now();
    w.show()?;
    w.set_focus()?;
    tracing::info!("[{FLYOUT}] shown");
    crate::logging::probe(app, &w);
    Ok(())
}

/// Shows the flyout above Quick Settings, which just opened, until they
/// close. It takes no focus: that would close them.
pub fn show_with_quick_settings(app: &AppHandle) {
    static FOLLOWING: AtomicBool = AtomicBool::new(false);
    if FOLLOWING.swap(true, Ordering::Relaxed) {
        return;
    }
    let app = app.clone();
    // Its own thread: COM, and reading the panels takes a while
    let _ = std::thread::Builder::new().name("quick-settings".into()).spawn(move || {
        if let Err(e) = follow_quick_settings(&app) {
            tracing::warn!("[{FLYOUT}] above Quick Settings: {e}");
        }
        set_quick_settings(None);
        FOLLOWING.store(false, Ordering::Relaxed);
    });
}

fn follow_quick_settings(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let _com = win_sys::audio_policy::ComApartment::init_mta()?;
    let reader = win_sys::quick_settings::Reader::new()?;
    // Closed again: a first start can take longer than they stay open
    let Some(panels) = reader.read() else { return Ok(()) };
    let w = flyout_window(app, false)?;
    set_quick_settings(Some(panels));
    w.set_focusable(false)?;
    place_flyout(&w)?;
    // Not w.show(), which activates it. On the main thread, to stay behind
    // the move queued above. w.hide() still works, tao hides unconditionally
    let hwnd = w.hwnd()?.0 as isize;
    w.run_on_main_thread(move || win_sys::quick_settings::show_inactive(hwnd))?;
    tracing::info!("[{FLYOUT}] shown above Quick Settings");
    crate::logging::probe(app, &w);
    // The panels change height as a list unfolds in them
    let mut read = Instant::now();
    loop {
        std::thread::sleep(POINTER_EVERY);
        // Before a click can come. Only now, as it costs other apps' cards
        if !FLYOUT_IN_USE.load(Ordering::Relaxed) && win_sys::quick_settings::pointer_over(hwnd) {
            FLYOUT_IN_USE.store(true, Ordering::Relaxed);
            w.run_on_main_thread(move || win_sys::quick_settings::set_active(hwnd))?;
        }
        if read.elapsed() < FOLLOW_EVERY {
            continue;
        }
        read = Instant::now();
        let Some(panels) = reader.read() else { break };
        set_quick_settings(Some(panels));
        place_flyout(&w)?;
    }
    FLYOUT_IN_USE.store(false, Ordering::Relaxed);
    // A list open in the page is a window of its own, and without the focus
    // to lose it would stay on the screen
    w.eval("document.activeElement?.blur()")?;
    w.hide()?;
    w.set_focusable(true)?;
    Ok(())
}

/// Bottom right of the work area, above the notification area; by Quick
/// Settings while the flyout goes with them
fn place_flyout(w: &WebviewWindow) -> tauri::Result<()> {
    if let Some(panels) = quick_settings() {
        return place_by_panels(w, panels);
    }
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

fn place_by_panels(w: &WebviewWindow, panels: Panels) -> tauri::Result<()> {
    let Some(m) = w.monitor_from_point(panels.left.into(), panels.bottom.into())? else {
        return Ok(());
    };
    let size = w.inner_size()?;
    let margin = (FLYOUT_MARGIN * m.scale_factor()).round() as i32;
    let panels = {
        let mut top = CARDS_TOP.lock().unwrap_or_else(|e| e.into_inner());
        let held = top.filter(|_| FLYOUT_IN_USE.load(Ordering::Relaxed)).map(|t| t.min(panels.top));
        let now = held.unwrap_or_else(|| win_sys::quick_settings::with_other_cards(panels, 2 * margin).top);
        *top = Some(now);
        Panels { top: now, ..panels }
    };
    let (x, y) = by_panels(panels, size.width as i32, size.height as i32, margin, m.work_area().position.y);
    // The window's frame is its transparent shadow: the page lines up
    let (outer, inner) = (w.outer_position()?, w.inner_position()?);
    let at = PhysicalPosition::new(x - (inner.x - outer.x), y - (inner.y - outer.y));
    if outer != at {
        w.set_position(at)?;
    }
    Ok(())
}

/// Above the panels, as far from them as they are from each other; to their
/// left when the screen is too short for that
fn by_panels(panels: Panels, width: i32, height: i32, margin: i32, screen_top: i32) -> (i32, i32) {
    let above = panels.top - margin - height;
    if above >= screen_top + margin {
        (panels.right - width, above)
    } else {
        (panels.left - margin - width, panels.bottom - height)
    }
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

/// The first-run setup has a window size of its own; leaving it restores
/// the one before. A page reloaded during the setup keeps that size.
pub fn setup_window(w: &WebviewWindow, active: bool) -> tauri::Result<()> {
    if w.label() != MAIN {
        return Ok(());
    }
    let mut before = BEFORE_SETUP.lock().unwrap_or_else(|e| e.into_inner());
    if active {
        if before.is_some() {
            return Ok(());
        }
        let maximized = w.is_maximized()?;
        *before = Some((w.inner_size()?.to_logical::<f64>(w.scale_factor()?), maximized));
        if maximized {
            w.unmaximize()?;
        }
        w.set_min_size(Some(LogicalSize::new(SETUP_WIDTH, MAIN_MIN_HEIGHT)))?;
        w.set_size(LogicalSize::new(SETUP_WIDTH, SETUP_HEIGHT))?;
    } else {
        let Some((size, maximized)) = before.take() else {
            return Ok(());
        };
        w.set_min_size(Some(LogicalSize::new(MAIN_MIN_WIDTH, MAIN_MIN_HEIGHT)))?;
        w.set_size(size)?;
        if maximized {
            w.maximize()?;
        }
    }
    w.center()
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
/// The icon resource tauri-build embeds icons/icon.ico as
const ICON_RESOURCE: u16 = 32512;

/// The title bar and taskbar icons at their own sizes from icon.ico, not
/// Tauri's one image shrunk (issue #86: a blue square at 100 % scaling)
fn set_icons(window: &tauri::Window) {
    match window.hwnd() {
        Ok(h) if win_sys::app_icon::set_window_icons(h.0 as isize, ICON_RESOURCE) => {}
        _ => tracing::warn!("[{}] window icons not set", window.label()),
    }
}

pub fn on_window_event(window: &tauri::Window, event: &WindowEvent) {
    match event {
        // Another screen's scale: the icons at its sizes
        WindowEvent::ScaleFactorChanged { .. } if window.label() == MAIN => set_icons(window),
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            let _ = window.hide();
            if window.label() == MAIN {
                main_closed(window);
            }
        }
        WindowEvent::Focused(false) if window.label() == FLYOUT && quick_settings().is_none() => {
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
    if args.iter().any(|a| a == "--quick-settings") {
        show_with_quick_settings(app);
        return;
    }
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

    #[test]
    fn flyout_stands_above_the_panels_or_beside_them() {
        let panels = Panels { left: 3282, top: 1281, right: 3822, bottom: 2070 };
        // 150%: as wide as the panels, their gap apart, their right edges in line
        assert_eq!(by_panels(panels, 540, 510, 18, 0), (3282, 753));
        // No room above
        assert_eq!(by_panels(panels, 540, 1300, 18, 0), (2724, 770));
        // The whole window, when the panels could not be read
        let window = Panels { left: 3264, top: 0, right: 3840, bottom: 2088 };
        assert_eq!(by_panels(window, 540, 510, 18, 0), (2706, 1578));
    }
}
