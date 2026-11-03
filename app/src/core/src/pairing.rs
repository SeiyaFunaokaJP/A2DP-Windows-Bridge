//! Developer mode: a classic device's pairing as A2DPWB keeps it, the
//! services the device lists, and taking over the link keys Windows'
//! Bluetooth kept for the adapter A2DPWB now uses (issue #86: devices paired
//! in Windows then connect without pairing again).
//!
//! Windows keeps its keys where only SYSTEM may read them: the elevated
//! helper reads them (a2dpwb-driver.exe `windows-keys`, one administrator
//! prompt). A key A2DPWB has already is never replaced: the device was
//! paired with A2DPWB later, so Windows' key is the old one.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bt_host::ServicesError;
use bt_host::link_keys::FileLinkKeyStore;
use ipc::{DevicePairing, DeviceService, ErrorInfo, LinkKeyInfo, Msg, ServiceList, WindowsImport};
use serde_json::Value;

use crate::state::CoreState;

/// Windows keeps no key type: HCI's "unauthenticated combination key from
/// P-192". BTstack refuses a link without Secure Connections whose key type
/// says P-256, never one that says P-192, so this type holds whichever
/// kind of pairing Windows made.
const WINDOWS_KEY_TYPE: i32 = 4;

fn err(code: &str, key: &str) -> ErrorInfo {
    ErrorInfo { code: code.into(), message: Msg::new(key) }
}

fn developer(state: &CoreState) -> Result<(), ErrorInfo> {
    if state.service.settings().debug_mode { Ok(()) } else { Err(err("developer.off", "ipc.error.developer_only")) }
}

fn address(address: &str) -> Result<[u8; 6], ErrorInfo> {
    bt_host::parse_addr(address).ok_or_else(|| err("device.invalid_address", "error.invalid_address_format"))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// device.pairing: the link keys file is written in full on every change,
/// also while a stack runs, so it is read from there
pub fn pairing(state: &CoreState, addr: &str) -> Result<Value, ErrorInfo> {
    developer(state)?;
    let addr = address(addr)?;
    let store = FileLinkKeyStore::open(&state.config_dir());
    let link_key = store
        .entries()
        .iter()
        .find(|e| e.addr == addr)
        .map(|e| LinkKeyInfo { key: hex(&e.key), key_type: e.key_type.clamp(0, 255) as u8 });
    Ok(serde_json::to_value(DevicePairing { link_key }).unwrap_or(Value::Null))
}

/// Each device's services from its last answer since the core started (by
/// address), shown while it is not connected (issue #86)
static SEEN: Mutex<BTreeMap<[u8; 6], Vec<DeviceService>>> = Mutex::new(BTreeMap::new());

fn seen() -> std::sync::MutexGuard<'static, BTreeMap<[u8; 6], Vec<DeviceService>>> {
    SEEN.lock().unwrap_or_else(|e| e.into_inner())
}

fn list(services: Vec<DeviceService>, from_last_connection: bool) -> Value {
    serde_json::to_value(ServiceList { services, from_last_connection }).unwrap_or(Value::Null)
}

/// device.services: over the link the running stack has to the device; not
/// connected, the list it gave last
pub fn services(state: &CoreState, addr: &str) -> Result<Value, ErrorInfo> {
    developer(state)?;
    let addr = address(addr)?;
    let not_connected = || match seen().get(&addr) {
        Some(last) => Ok(list(last.clone(), true)),
        None => Err(err("device.not_connected", "ipc.error.services_not_connected")),
    };
    let Some(host) = state.service.host() else {
        return not_connected();
    };
    let records = match host.device_services(addr) {
        Ok(r) => r,
        Err(ServicesError::NotConnected) => return not_connected(),
        Err(ServicesError::Busy) => return Err(err("device.busy", "ipc.error.services_busy")),
        Err(e @ (ServicesError::NoAnswer | ServicesError::Failed(_))) => {
            tracing::info!("services of {}: {e:?}", bt_host::addr_str(&addr));
            return Err(err("device.no_answer", "ipc.error.services_no_answer"));
        }
    };
    let services: Vec<DeviceService> =
        records.into_iter().map(|r| DeviceService { uuids: r.classes, name: r.name }).collect();
    seen().insert(addr, services.clone());
    Ok(list(services, false))
}

/// One key of the helper's answer
fn parse_key(v: &Value) -> Option<([u8; 6], [u8; 16])> {
    let addr = bt_host::parse_addr(v.get("address")?.as_str()?)?;
    let text = v.get("key")?.as_str()?;
    if text.len() != 32 || !text.is_ascii() {
        return None;
    }
    let mut key = [0u8; 16];
    for (i, k) in key.iter_mut().enumerate() {
        *k = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some((addr, key))
}

/// device.import_windows_pairings
pub fn import_windows(state: &Arc<CoreState>, adapter: &str) -> Result<Value, ErrorInfo> {
    developer(state)?;
    let radio = bt_host::addr_str(&address(adapter)?);
    let (code, result) = crate::driver::run(&["windows-keys", &radio])?;
    if code != 0 || result.get("ok").and_then(Value::as_bool) != Some(true) {
        tracing::warn!("Windows' keys of {radio}: helper exit {code}");
        return Err(err("adapter.import_failed", "ipc.error.import_failed"));
    }
    let adapter_found = result.get("radio_found").and_then(Value::as_bool) == Some(true);
    let keys: Vec<([u8; 6], [u8; 16])> =
        result.get("keys").and_then(Value::as_array).into_iter().flatten().filter_map(parse_key).collect();
    let dir = state.config_dir();
    let have: Vec<[u8; 6]> = FileLinkKeyStore::open(&dir).entries().iter().map(|e| e.addr).collect();
    let (mut imported, mut kept) = (Vec::new(), Vec::new());
    let host = state.service.host();
    for (addr, key) in keys {
        if have.contains(&addr) {
            kept.push(bt_host::addr_str(&addr));
            continue;
        }
        // A running stack holds the keys in memory and writes them in full
        let stored = match &host {
            Some(h) => h.store_link_key(addr, key, WINDOWS_KEY_TYPE),
            None => {
                bt_host::link_keys::store(&dir, &addr, &key, WINDOWS_KEY_TYPE);
                true
            }
        };
        if stored {
            imported.push(bt_host::addr_str(&addr));
        } else {
            tracing::warn!("Windows' key of {} not stored: the stack did not answer", bt_host::addr_str(&addr));
        }
    }
    tracing::info!(
        "Windows' keys of {radio}: adapter found {adapter_found}, {} taken, {} kept",
        imported.len(),
        kept.len()
    );
    Ok(serde_json::to_value(WindowsImport { adapter_found, imported, kept }).unwrap_or(Value::Null))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_keys_are_read_strictly() {
        let good = serde_json::json!({ "address": "00:A2:D0:00:00:02", "key": "00112233445566778899aabbccddeeff" });
        let (addr, key) = parse_key(&good).unwrap();
        assert_eq!(addr, [0x00, 0xA2, 0xD0, 0x00, 0x00, 0x02]);
        assert_eq!((key[0], key[15]), (0x00, 0xFF));
        for bad in [
            serde_json::json!({ "address": "00:A2:D0:00:00", "key": "00112233445566778899aabbccddeeff" }),
            serde_json::json!({ "address": "00:A2:D0:00:00:02", "key": "00112233445566778899aabbccddee" }),
            serde_json::json!({ "address": "00:A2:D0:00:00:02", "key": "0011223344556677889\u{e9}abbccddeeff" }),
            serde_json::json!({ "address": "00:A2:D0:00:00:02" }),
        ] {
            assert_eq!(parse_key(&bad), None, "{bad}");
        }
    }
}
