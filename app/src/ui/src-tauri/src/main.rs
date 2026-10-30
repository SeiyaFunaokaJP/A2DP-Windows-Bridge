//! bluecaster.exe: the BlueCaster window process. It shows the state of
//! bluecaster-core.exe and sends it the user's requests; the core keeps
//! streaming when this process ends.
//!
//! Arguments: `--flyout` opens the tray flyout instead of the main window
//! (the core starts the UI this way when the tray icon is clicked).

#![forbid(unsafe_code)]
// No console window in release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod connection;
mod logging;
mod recovery;
mod update;
mod windows;

use tauri::Manager;
use tauri::webview::PageLoadEvent;

fn main() {
    let _log = logging::init();
    let flyout = std::env::args().any(|a| a == "--flyout");
    tracing::info!(
        "bluecaster {} starting{} (WebView2 {})",
        env!("CARGO_PKG_VERSION"),
        if flyout { " with the flyout" } else { "" },
        tauri::webview_version().unwrap_or_else(|e| format!("unknown: {e}"))
    );
    let ipc = connection::Ipc::new();
    let app = tauri::Builder::default()
        // Must be the first plugin: a second launch ends before anything
        // else starts
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| windows::on_second_instance(app, &args)))
        // Its commands are not open to the webview (capabilities): updates
        // go through update.rs only
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(ipc.clone())
        .manage(logging::Probes::default())
        .on_page_load(|webview, payload| {
            let event = match payload.event() {
                PageLoadEvent::Started => "loading",
                PageLoadEvent::Finished => {
                    windows::page_loaded(webview.label());
                    "loaded"
                }
            };
            tracing::info!("[{}] page {event}: {}", webview.label(), payload.url());
        })
        .invoke_handler(tauri::generate_handler![
            logging::ui_log,
            logging::ui_probe,
            commands::ipc_request,
            commands::ipc_status,
            commands::ui_info,
            commands::show_main,
            commands::show_flyout,
            commands::fit_flyout,
            commands::set_theme,
            commands::open_url,
            commands::open_folder,
            commands::reveal_path,
            commands::update_status,
            commands::update_install,
        ])
        .on_window_event(windows::on_window_event)
        .setup(move |app| {
            let handle = app.handle().clone();
            app.manage(update::Updates::new(&handle));
            connection::start(handle.clone(), ipc);
            if flyout {
                windows::show_flyout(&handle)?;
            } else {
                windows::show_main(&handle, None)?;
            }
            windows::start_idle_exit(handle);
            Ok(())
        })
        .build(tauri::generate_context!());
    let app = match app {
        Ok(app) => app,
        Err(e) => {
            tracing::error!("cannot start the window: {e}");
            eprintln!("bluecaster: cannot start the window: {e}");
            drop(_log);
            std::process::exit(1);
        }
    };
    app.run(|_, event| match event {
        // Windows are hidden, not closed (see windows.rs), so this comes
        // only from app.exit() or the last window being destroyed by the
        // system; only the explicit exit ends the process
        tauri::RunEvent::ExitRequested { api, code: None, .. } => {
            tracing::info!("the last window is gone; staying (tray)");
            api.prevent_exit();
        }
        tauri::RunEvent::Exit => {
            tracing::info!("bluecaster exiting");
            recovery::relaunch_if_due();
        }
        _ => {}
    });
}
