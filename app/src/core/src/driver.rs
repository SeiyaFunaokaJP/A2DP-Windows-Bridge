//! WinUSB assignment and restore through the elevated helper
//! bluecaster-driver.exe (the core itself never runs as administrator).
//!
//! The helper writes its JSON result to a file we name (an elevated process
//! started with ShellExecuteEx gives no stdout). The "previous driver"
//! record of an assignment is kept in the settings folder, per adapter, for
//! the restore.

use std::path::PathBuf;
use std::sync::Arc;

use ipc::{ErrorInfo, Msg};
use serde_json::Value;
use win_sys::elevation::{self, ElevationError};

use crate::state::CoreState;

fn err(code: &str, key: &str) -> ErrorInfo {
    ErrorInfo { code: code.into(), message: Msg::new(key) }
}

fn helper() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("bluecaster-driver.exe")))
        .unwrap_or_else(|| "bluecaster-driver.exe".into())
}

/// File of the assignment record of an adapter
fn record_path(state: &CoreState, instance_id: &str) -> PathBuf {
    let safe: String = instance_id.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    state.config_dir().join("driver").join(format!("{safe}.json"))
}

fn run(args: &[&str]) -> Result<(u32, Value), ErrorInfo> {
    let out = std::env::temp_dir().join(format!("bluecaster-driver-{}.json", std::process::id()));
    let _ = std::fs::remove_file(&out);
    let mut all: Vec<&str> = args.to_vec();
    let out_s = out.to_string_lossy().into_owned();
    all.extend(["--out", &out_s]);
    // The elevated run's log next to the core's
    let log = bluecaster_core::paths::log_dir(&bluecaster_core::paths::config_dir()).join("bluecaster-driver.log");
    let log_s = log.to_string_lossy().into_owned();
    all.extend(["--log", &log_s]);
    if crate::logging::verbose_now().is_some() {
        all.push("--verbose");
    }
    let code = elevation::run_elevated(&helper(), &all).map_err(|e| match e {
        ElevationError::Cancelled => err("adapter.uac_cancelled", "error.uac_cancelled"),
        _ => {
            tracing::error!("driver helper: {e}");
            err("adapter.driver_failed", "error.driver_failed")
        }
    })?;
    let json =
        std::fs::read_to_string(&out).ok().and_then(|t| serde_json::from_str(t.trim_start_matches('\u{feff}')).ok());
    let _ = std::fs::remove_file(&out);
    tracing::info!("driver helper {} -> exit {code}: {json:?}", args.join(" "));
    Ok((code, json.unwrap_or(Value::Null)))
}

pub fn assign(state: &Arc<CoreState>, instance_id: &str) -> Result<Value, ErrorInfo> {
    // The stack must let go of the adapter (it may be ours already)
    state.service.release_adapter();
    let (code, result) = run(&["assign", instance_id])?;
    if code != 0 {
        state.refresh_adapters();
        // Some environments refuse DiInstallDevice with the inbox winusb.inf;
        // Zadig's signed package is the way out there (docs/setup.md#zadig)
        return Err(err("adapter.driver_failed", "error.driver_assign_failed"));
    }
    let path = record_path(state, instance_id);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = std::fs::write(&path, serde_json::to_vec_pretty(&result).unwrap_or_default()) {
        tracing::warn!("cannot keep the driver record {}: {e}", path.display());
    }
    let mut adapters = state.refresh_adapters();
    // Even after the helper restarted the device, Windows may load WinUSB
    // only after a replug or a restart (issue #81: until then opening the
    // adapter fails with "failed to initialize WinUSB (error 50)")
    if result.get("reboot_required").and_then(Value::as_bool) == Some(true) {
        tracing::warn!(
            "driver: WinUSB takes effect for {instance_id} after the adapter is replugged or Windows restarts"
        );
        for a in adapters.iter_mut().filter(|a| a.instance_id == instance_id) {
            a.restart_needed = true;
        }
    }
    Ok(serde_json::to_value(adapters).unwrap_or(Value::Null))
}

pub fn restore(state: &Arc<CoreState>, instance_id: &str) -> Result<Value, ErrorInfo> {
    state.service.release_adapter();
    let path = record_path(state, instance_id);
    let previous = path.exists().then(|| format!("@{}", path.display()));
    let mut args = vec!["restore", instance_id];
    if let Some(p) = previous.as_deref() {
        args.extend(["--previous", p]);
    }
    let (code, _) = run(&args)?;
    state.refresh_adapters();
    match code {
        0 => {
            let _ = std::fs::remove_file(&path);
            Ok(serde_json::to_value(state.snapshot().1.adapter.adapters).unwrap_or(Value::Null))
        }
        // Done, but not on the recorded driver (e.g. Zadig's WinUSB package
        // still in the driver store outranks Windows' Bluetooth driver)
        5 => Err(err("adapter.restore_unverified", "error.driver_restore_unverified")),
        _ => Err(err("adapter.driver_failed", "error.driver_failed")),
    }
}
