//! WinUSB assignment and restore through the elevated helper
//! a2dpwb-driver.exe (the core itself never runs as administrator).
//!
//! The helper writes its JSON result to a file we name (an elevated process
//! started with ShellExecuteEx gives no stdout). The "previous driver"
//! record of an assignment is kept in the settings folder, per adapter, for
//! the restore.

use std::path::PathBuf;
use std::sync::Arc;

use ipc::{ErrorInfo, Msg};
use serde_json::Value;
use win_sys::bt_radios::WindowsRadio;
use win_sys::elevation::{self, ElevationError};

use crate::state::CoreState;

fn err(code: &str, key: &str) -> ErrorInfo {
    ErrorInfo { code: code.into(), message: Msg::new(key) }
}

fn helper() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("a2dpwb-driver.exe")))
        .unwrap_or_else(|| "a2dpwb-driver.exe".into())
}

/// File of the assignment record of an adapter
fn record_path(state: &CoreState, instance_id: &str) -> PathBuf {
    let safe: String = instance_id.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    state.config_dir().join("driver").join(format!("{safe}.json"))
}

pub(crate) fn run(args: &[&str]) -> Result<(u32, Value), ErrorInfo> {
    let out = std::env::temp_dir().join(format!("a2dpwb-driver-{}.json", std::process::id()));
    let _ = std::fs::remove_file(&out);
    let mut all: Vec<&str> = args.to_vec();
    let out_s = out.to_string_lossy().into_owned();
    all.extend(["--out", &out_s]);
    // The elevated run's log next to the core's
    let log = a2dpwb_core::paths::log_dir(&a2dpwb_core::paths::config_dir()).join("a2dpwb-driver.log");
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
    let json: Option<Value> =
        std::fs::read_to_string(&out).ok().and_then(|t| serde_json::from_str(t.trim_start_matches('\u{feff}')).ok());
    let _ = std::fs::remove_file(&out);
    // The result of windows-keys holds link keys: not for the log
    match json.as_ref().and_then(|j| j.get("keys")).and_then(Value::as_array) {
        Some(keys) => tracing::info!("driver helper {} -> exit {code}: {} key(s)", args.join(" "), keys.len()),
        None => tracing::info!("driver helper {} -> exit {code}: {json:?}", args.join(" ")),
    }
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
    state.driver_changed();
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
            state.driver_changed();
            if let Some(other) = other_radio_in_use(instance_id) {
                return Err(ErrorInfo {
                    code: "adapter.restore_other_radio".into(),
                    message: Msg { key: "error.driver_restore_other_radio".into(), args: vec![other] },
                });
            }
            Ok(serde_json::to_value(state.snapshot().1.adapter.adapters).unwrap_or(Value::Null))
        }
        // Done, but not on the recorded driver (e.g. Zadig's WinUSB package
        // still in the driver store outranks Windows' Bluetooth driver)
        5 => Err(err("adapter.restore_unverified", "error.driver_restore_unverified")),
        _ => Err(err("adapter.driver_failed", "error.driver_failed")),
    }
}

/// After a restore that left the adapter on its driver but not working:
/// the name of another radio Windows' Bluetooth runs. Windows runs one
/// radio at a time (BTHUSB event 6, the adapter fails with Code 31), so
/// the restored one stays off until that one is disabled.
fn other_radio_in_use(instance_id: &str) -> Option<String> {
    let state = win_sys::driver::driver_state(instance_id).ok()?;
    let name = blocking_radio(instance_id, state.problem_code, &win_sys::bt_radios::radios())?;
    tracing::warn!(
        "driver: {instance_id} is back on {} but has problem code {:?}; Windows runs {name:?}",
        state.driver.service,
        state.problem_code
    );
    Some(name)
}

/// The radio that keeps a failed adapter off: one not on that adapter
fn blocking_radio(instance_id: &str, problem: Option<u32>, radios: &[WindowsRadio]) -> Option<String> {
    problem?;
    let other = radios.iter().find(|r| !r.on_device(instance_id))?;
    Some(if other.name.is_empty() {
        other.instance_ids.first().cloned().unwrap_or_default()
    } else {
        other.name.clone()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn radio(name: &str, ids: &[&str]) -> WindowsRadio {
        WindowsRadio {
            instance_ids: ids.iter().map(|s| s.to_string()).collect(),
            name: name.into(),
            address: None,
            hci_version: None,
            lmp_version: None,
        }
    }

    const TPLINK: &str = r"USB\VID_2357&PID_0604\E848B8C82000";
    const INTEL: &str = r"USB\VID_8087&PID_0033\5&3B72E4CF&0&14";

    #[test]
    fn another_radio_keeps_the_restored_adapter_off() {
        let intel = [radio("Intel Wireless Bluetooth", &[INTEL, r"USB\ROOT_HUB30\4&1"])];
        assert_eq!(blocking_radio(TPLINK, Some(31), &intel).as_deref(), Some("Intel Wireless Bluetooth"));
        // Working after the restore: nothing to say
        assert_eq!(blocking_radio(TPLINK, None, &intel), None);
        // Failed, but Windows runs no other radio: not this cause
        assert_eq!(blocking_radio(TPLINK, Some(31), &[radio("TP-Link", &[TPLINK])]), None);
        assert_eq!(blocking_radio(TPLINK, Some(31), &[]), None);
        // A radio without a name is named by its device
        assert_eq!(blocking_radio(TPLINK, Some(31), &[radio("", &[INTEL])]).as_deref(), Some(INTEL));
    }
}
