//! Which controller to open and how to bring it up: the Realtek chip for the
//! firmware download (setting, or the detected adapter as v1's "Auto"), the
//! Intel bootloader download and Broadcom PatchRAM (experimental), and the
//! AFH classification from the PC's Wi-Fi.

use std::collections::BTreeMap;
use std::path::Path;

use bt_host::afh;
use bt_host::chips::{self, ChipVendor};
use bt_host::{AfhClassification, AfhSource, HostConfig, TransportConfig};
use win_sys::usb_adapters::{self, UsbDevice};

use crate::settings::Settings;

/// A USB Bluetooth adapter as the UI and the stack setup see it
#[derive(Debug, Clone)]
pub struct Adapter {
    pub device: UsbDevice,
    /// Realtek PID for the firmware lookup (also for OEM adapters)
    pub realtek_pid: Option<u16>,
    pub vendor: ChipVendor,
    pub display_name: String,
}

/// Present USB devices that look like Bluetooth adapters (any driver).
/// `probe_winusb` opens WinUSB-bound ones for a moment (not while the stack
/// has the adapter open).
pub fn adapters(probe_winusb: bool) -> Vec<Adapter> {
    let ids = bt_host::realtek_usb_firmware();
    let devices = match usb_adapters::enumerate(probe_winusb) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("cannot list USB devices: {e}");
            return Vec::new();
        }
    };
    devices
        .into_iter()
        .filter_map(|d| {
            let realtek_pid = chips::resolve_realtek_pid(d.vid, d.pid, ids);
            let vendor = chips::vendor_for_usb(d.vid, d.pid, realtek_pid);
            let bluetooth =
                d.looks_like_bluetooth() || realtek_pid.is_some() || chips::is_bluetooth_vendor_vid(d.vid, d.pid);
            if bluetooth {
                tracing::debug!(
                    "adapter: {:04X}:{:04X} {:?} driver {:?}, realtek pid {realtek_pid:04X?}, vendor {}, class {:?}, problem {:?}, probe {:?}",
                    d.vid,
                    d.pid,
                    d.name,
                    d.driver_kind,
                    vendor.name(),
                    d.usb_class,
                    d.problem_code,
                    d.probe
                );
            } else {
                tracing::trace!("adapter: {:04X}:{:04X} {:?} skipped, not Bluetooth", d.vid, d.pid, d.name);
            }
            bluetooth.then(|| Adapter {
                display_name: chips::display_name(d.vid, d.pid, realtek_pid, vendor),
                device: d,
                realtek_pid,
                vendor,
            })
        })
        .collect()
}

/// Vendor-class (FF/01/01) controllers v1's WinUSB transport knew built in
/// (BTstack's list): they do not identify as Bluetooth class
const BUILTIN_VENDOR_CLASS: &[(u16, u16)] = &[
    (0x0A5C, 0x21E8), // BCM20702A0, DeLOCK Bluetooth 4.0
    (0x0B05, 0x17CB), // BCM20702A0, ASUS BT400
    (0x0A5C, 0x22BE), // BCM20702B0, generic class 1
    (0x2FE3, 0x0100), // nRF5x Zephyr USB HCI
    (0x2FE3, 0x000B),
];

/// VID:PIDs the USB transport opens besides Bluetooth-class devices (bt-host
/// adds BTstack's Realtek table, CSR and Intel): v1's built-in vendor-class
/// list, the OEM adapters with a Realtek chip, and the WinUSB adapters whose
/// interface 0 was found to be vendor-class Bluetooth with the HCI pipes
pub fn usb_known_ids(vendor_class: &[(u16, u16)]) -> Vec<(u16, u16)> {
    let mut ids: Vec<(u16, u16)> = BUILTIN_VENDOR_CLASS.to_vec();
    ids.extend(chips::OEM_TABLE.iter().map(|m| (m.vid, m.pid)));
    for id in vendor_class {
        if !ids.contains(id) {
            ids.push(*id);
        }
    }
    ids
}

/// Fills the chipset part of `config` from the settings and the present
/// adapters
pub fn configure(config: &mut HostConfig, settings: &Settings, _config_dir: &Path) {
    config.afh = Some(Box::new(WifiAfh));
    if matches!(config.transport, TransportConfig::Tcp(_)) {
        return; // a virtual controller: no chipset, no firmware
    }
    let auto = chips::is_auto_chip(settings.bt_chip_pid, &settings.bt_chip_fw_stem);
    // The stack is not running yet (start and restart shut the old one
    // down first): the probe reads interface 0 of each WinUSB adapter, as
    // v1 did before opening one, and tells one another program holds
    let present = adapters(true);
    let winusb: Vec<&Adapter> =
        present.iter().filter(|a| a.device.driver_kind == win_sys::driver::DriverKind::WinUsb).collect();
    tracing::debug!(
        "adapter: {} present, {} on WinUSB, chip setting pid {:04X} stem {:?} (auto {auto})",
        present.len(),
        winusb.len(),
        settings.bt_chip_pid,
        settings.bt_chip_fw_stem
    );
    if let TransportConfig::Usb(sel) = &mut config.transport {
        let vendor_class: Vec<(u16, u16)> = winusb
            .iter()
            .filter(|a| a.device.winusb_bluetooth() == Some(true))
            .map(|a| (a.device.vid, a.device.pid))
            .collect();
        for (vid, pid) in &vendor_class {
            tracing::info!("vendor-class (FF/01/01) adapter {vid:04X}:{pid:04X}: the transport may open it");
        }
        for id in usb_known_ids(&vendor_class) {
            if !sel.known.contains(&id) {
                sel.known.push(id);
            }
        }
        tracing::debug!("adapter: usb selector {sel:?}");
    }
    let pid =
        chips::resolve_chip_pid(settings.bt_chip_pid, &settings.bt_chip_fw_stem, winusb.iter().map(|a| a.realtek_pid));
    if pid != 0 {
        let chip_name = chips::realtek_chip_name(pid);
        // The adapter's own VID:PID when it differs from the PID BTstack is
        // given (a family's other PID, an OEM adapter): "0BDA:4853 -> 0xB85B"
        let usb = winusb
            .iter()
            .find(|a| a.realtek_pid == Some(pid) && (a.device.vid, a.device.pid) != (chips::REALTEK_VID, pid))
            .map(|a| format!("{:04X}:{:04X} -> ", a.device.vid, a.device.pid))
            .unwrap_or_default();
        tracing::info!(
            "chip: {} ({usb}0x{pid:04X}){}",
            chip_name.unwrap_or("unknown"),
            if auto { " (auto)" } else { "" }
        );
        if chip_name.is_none() && settings.bt_chip_fw_stem.is_empty() {
            tracing::warn!(
                "chip 0x{pid:04X} is not in BlueCaster's chip table: choose its firmware on the Adapter screen (Firmware), else no firmware file is checked or prepared for it"
            );
        }
        config.realtek_pid = pid;
        // The transport opens the first WinUSB adapter it can: with another
        // one on WinUSB too (a built-in Intel chosen by mistake), BTstack
        // would bring that one up with this chip's setup, and its quick init
        // read as "firmware not loaded". The adapter of this chip goes first.
        if let TransportConfig::Usb(sel) = &mut config.transport
            && winusb.len() > 1
            && let Some(a) = winusb.iter().find(|a| a.realtek_pid == Some(pid) && !a.device.in_use())
        {
            tracing::info!(
                "{} adapters on WinUSB: {:04X}:{:04X} (the Realtek chip's) is opened first",
                winusb.len(),
                a.device.vid,
                a.device.pid
            );
            sel.preferred = Some(a.device.instance_id.clone());
        }
    } else {
        tracing::info!(
            "no Realtek chip {}, skipping the Realtek chipset init",
            if auto { "detected" } else { "selected" }
        );
    }
    config.realtek_files = chips::firmware_files(pid, &settings.bt_chip_fw_stem).map(|(fw, cfg)| {
        let stem = |s: String| s.strip_suffix(".bin").map(str::to_owned).unwrap_or(s);
        (stem(fw), stem(cfg))
    });
    tracing::debug!("adapter: realtek pid {pid:04X}, firmware files {:?}", config.realtek_files);
    if pid == 0 {
        // Other vendors (experimental): Intel needs its bootloader
        // firmware, Broadcom may take a PatchRAM
        // One held by another program cannot be loaded either (v1 skipped
        // it too)
        config.intel_loader = winusb.iter().any(|a| a.vendor == ChipVendor::Intel && !a.device.in_use());
        for a in winusb.iter().filter(|a| a.vendor == ChipVendor::Intel && a.device.in_use()) {
            tracing::info!(
                "Intel adapter {:04X}:{:04X} is in use by another program: no bootloader download for it",
                a.device.vid,
                a.device.pid
            );
        }
        config.broadcom_ids =
            winusb.iter().filter(|a| a.vendor == ChipVendor::Broadcom).map(|a| (a.device.vid, a.device.pid)).collect();
        tracing::debug!(
            "adapter: intel bootloader download {}, broadcom patchram ids {:04X?}",
            config.intel_loader,
            config.broadcom_ids
        );
        for a in &winusb {
            if matches!(a.vendor, ChipVendor::MediaTek | ChipVendor::Qualcomm) {
                tracing::warn!(
                    "{} adapter {:04X}:{:04X}: firmware loading for this vendor is not implemented; it may not work",
                    a.vendor.name(),
                    a.device.vid,
                    a.device.pid
                );
            }
        }
    }
}

/// AFH host channel classification from the 2.4 GHz channel this PC's Wi-Fi
/// is connected on ("auto")
struct WifiAfh;

impl AfhSource for WifiAfh {
    fn classify(&mut self, policy: &str) -> Option<AfhClassification> {
        let c = afh::classify(policy, || match win_sys::wlan::survey() {
            Ok(s) => s.connected_channels_24ghz(),
            Err(e) => {
                tracing::debug!("Wi-Fi survey failed: {e}");
                Vec::new()
            }
        });
        tracing::debug!(
            "afh: policy {policy:?}: passed {}, usable {}, avoided Wi-Fi {:?}",
            c.passed,
            c.usable,
            c.avoided_wifi
        );
        c.passed.then(|| AfhClassification {
            map: c.map,
            usable: c.usable as u32,
            avoided: afh::describe(&c.avoided_wifi),
        })
    }
}

// ---- What a stack learnt about an adapter (adapter_caps.json) ----

/// Writes adapter_caps.json (in `adapters\`)
fn write_caps(dir: &Path, caps: &BTreeMap<String, AdapterCaps>) {
    let text = serde_json::to_string_pretty(caps).unwrap_or_default();
    let path = crate::paths::adapter_caps_file(dir);
    if let Err(e) = std::fs::create_dir_all(crate::paths::adapters_dir(dir)).and_then(|()| std::fs::write(&path, text))
    {
        tracing::warn!("cannot write {}: {e}", path.display());
    }
}

/// What BlueCaster learnt about an adapter when a stack ran on it
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct AdapterCaps {
    /// LE Audio's isochronous channels (issue #86)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    le_audio: Option<bool>,
    /// The adapter's own Bluetooth address
    #[serde(default, skip_serializing_if = "Option::is_none")]
    address: Option<String>,
    /// The HCI version the controller reports (Bluetooth core version)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    hci_version: Option<u8>,
    /// Device Manager's name while Windows' Bluetooth ran it
    #[serde(default, skip_serializing_if = "Option::is_none")]
    windows_name: Option<String>,
}

fn caps_key(vid: u16, pid: u16) -> String {
    format!("{vid:04X}:{pid:04X}")
}

fn load_caps(dir: &Path) -> BTreeMap<String, AdapterCaps> {
    std::fs::read_to_string(crate::paths::adapter_caps_file(dir))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// Whether the adapter with this VID:PID has LE Audio's isochronous
/// channels, as a stack found out; None if none ran on it yet
pub fn le_audio(dir: &Path, vid: u16, pid: u16) -> Option<bool> {
    load_caps(dir).get(&caps_key(vid, pid)).and_then(|c| c.le_audio)
}

/// Remembers what a stack found out about the adapter's LE Audio
pub fn save_le_audio(dir: &Path, vid: u16, pid: u16, capable: bool) {
    let mut caps = load_caps(dir);
    let entry = caps.entry(caps_key(vid, pid)).or_default();
    if entry.le_audio == Some(capable) {
        return;
    }
    entry.le_audio = Some(capable);
    tracing::debug!("adapter caps: {} LE Audio {capable}, saving adapter_caps.json", caps_key(vid, pid));
    write_caps(dir, &caps);
}

/// The adapter's own Bluetooth address, as a stack running on it read it
pub fn address(dir: &Path, vid: u16, pid: u16) -> Option<String> {
    load_caps(dir).get(&caps_key(vid, pid)).and_then(|c| c.address.clone())
}

/// Remembers the adapter's own address
pub fn save_address(dir: &Path, vid: u16, pid: u16, address: &str) {
    let mut caps = load_caps(dir);
    let entry = caps.entry(caps_key(vid, pid)).or_default();
    if entry.address.as_deref() == Some(address) {
        return;
    }
    entry.address = Some(address.to_owned());
    tracing::debug!("adapter caps: {} address {address}, saving adapter_caps.json", caps_key(vid, pid));
    write_caps(dir, &caps);
}

/// The HCI version the controller on this VID:PID reported, as a stack
/// running on it read it
pub fn hci_version(dir: &Path, vid: u16, pid: u16) -> Option<u8> {
    load_caps(dir).get(&caps_key(vid, pid)).and_then(|c| c.hci_version)
}

/// Remembers the controller's HCI version
pub fn save_hci_version(dir: &Path, vid: u16, pid: u16, version: u8) {
    let mut caps = load_caps(dir);
    let entry = caps.entry(caps_key(vid, pid)).or_default();
    if entry.hci_version == Some(version) {
        return;
    }
    entry.hci_version = Some(version);
    tracing::debug!("adapter caps: {} HCI version {version}, saving adapter_caps.json", caps_key(vid, pid));
    write_caps(dir, &caps);
}

/// Device Manager's name for the adapter's radio, as Windows' Bluetooth
/// named it when it last ran the adapter
pub fn windows_name(dir: &Path, vid: u16, pid: u16) -> Option<String> {
    load_caps(dir).get(&caps_key(vid, pid)).and_then(|c| c.windows_name.clone())
}

/// What Windows' Bluetooth tells about the adapter while it runs it: its
/// name, and the address and version a stack of BlueCaster would read
pub fn save_windows_radio(dir: &Path, vid: u16, pid: u16, name: &str, address: Option<&str>, hci_version: Option<u8>) {
    let mut caps = load_caps(dir);
    let entry = caps.entry(caps_key(vid, pid)).or_default();
    let before = entry.clone();
    if !name.is_empty() {
        entry.windows_name = Some(name.to_owned());
    }
    if let Some(a) = address {
        entry.address = Some(a.to_owned());
    }
    if hci_version.is_some() {
        entry.hci_version = hci_version;
    }
    if *entry == before {
        return;
    }
    tracing::debug!(
        "adapter caps: {} Windows radio {name:?} {address:?} HCI {hci_version:?}, saving adapter_caps.json",
        caps_key(vid, pid)
    );
    write_caps(dir, &caps);
}

/// The Bluetooth address a Realtek adapter reports as its USB serial number
/// ("E848B8C82000" on a UB500 whose controller answers E8:48:B8:C8:20:00),
/// from the device's instance ID (`USB\VID_2357&PID_0604\E848B8C82000`).
/// Windows tells it whatever the driver and whether its Bluetooth runs the
/// adapter (a second radio it does not use answers no IOCTL and stores no
/// radio properties: issue #86). None for another vendor, or a device with
/// no serial (Windows then makes up an ID with `&`s) or another kind of one.
pub fn serial_address(vendor: ChipVendor, instance_id: &str) -> Option<String> {
    if vendor != ChipVendor::Realtek {
        return None;
    }
    let serial = instance_id.rsplit('\\').next()?;
    if serial.len() != 12 || !serial.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let addr = u64::from_str_radix(serial, 16).ok()?;
    if addr == 0 || addr == 0xFFFF_FFFF_FFFF {
        return None;
    }
    Some(win_sys::bt_radios::format_address(addr))
}

/// The Bluetooth core specification of an HCI version ("5.3" for 12);
/// None for one the table does not know
pub fn bt_core_version(hci_version: u8) -> Option<&'static str> {
    const VERSIONS: [&str; 16] = [
        "1.0b", "1.1", "1.2", "2.0", "2.1", "3.0", "4.0", "4.1", "4.2", "5.0", "5.1", "5.2", "5.3", "5.4", "6.0", "6.1",
    ];
    VERSIONS.get(hci_version as usize).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use win_sys::driver::{DriverInfo, DriverKind};
    use win_sys::usb_adapters::{InterfaceProbe, UsbClass};

    fn device(vid: u16, pid: u16, probe: Option<Result<InterfaceProbe, String>>) -> UsbDevice {
        UsbDevice {
            instance_id: format!(r"USB\VID_{vid:04X}&PID_{pid:04X}\1"),
            device_path: String::new(),
            vid,
            pid,
            name: String::new(),
            bus_reported_name: None,
            manufacturer: String::new(),
            hardware_ids: Vec::new(),
            compatible_ids: Vec::new(),
            usb_class: None,
            driver: DriverInfo::default(),
            driver_kind: DriverKind::WinUsb,
            problem_code: None,
            probe,
        }
    }

    #[test]
    fn le_audio_remembered_per_adapter() {
        let dir = std::env::temp_dir().join(format!("bluecaster-caps-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(le_audio(&dir, 0x0BDA, 0xA729), None);
        save_le_audio(&dir, 0x0BDA, 0xA729, false);
        save_le_audio(&dir, 0x8087, 0x0032, true);
        assert_eq!(le_audio(&dir, 0x0BDA, 0xA729), Some(false));
        assert_eq!(le_audio(&dir, 0x8087, 0x0032), Some(true));
        assert_eq!(address(&dir, 0x0BDA, 0xA729), None);
        save_address(&dir, 0x0BDA, 0xA729, "00:11:22:33:44:55");
        assert_eq!(address(&dir, 0x0BDA, 0xA729).as_deref(), Some("00:11:22:33:44:55"));
        assert_eq!(le_audio(&dir, 0x0BDA, 0xA729), Some(false));
        assert!(std::fs::read_to_string(crate::paths::adapter_caps_file(&dir)).unwrap().contains("\"0BDA:A729\""));
        assert_eq!(hci_version(&dir, 0x0BDA, 0xA729), None);
        save_hci_version(&dir, 0x0BDA, 0xA729, 10);
        assert_eq!(hci_version(&dir, 0x0BDA, 0xA729), Some(10));
        assert_eq!(hci_version(&dir, 0x8087, 0x0032), None);
        // The other values stay
        assert_eq!(address(&dir, 0x0BDA, 0xA729).as_deref(), Some("00:11:22:33:44:55"));
        assert_eq!(le_audio(&dir, 0x0BDA, 0xA729), Some(false));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn realtek_serial_is_its_address() {
        // A UB500 (Realtek under TP-Link's VID) whose stack read E8:48:B8:C8:20:00
        assert_eq!(
            serial_address(ChipVendor::Realtek, r"USB\VID_2357&PID_0604\E848B8C82000").as_deref(),
            Some("E8:48:B8:C8:20:00")
        );
        assert_eq!(
            serial_address(ChipVendor::Realtek, r"USB\VID_0BDA&PID_A729\00e04c239987").as_deref(),
            Some("00:E0:4C:23:99:87")
        );
        // No serial: Windows' made-up ID
        assert_eq!(serial_address(ChipVendor::Realtek, r"USB\VID_2357&PID_0604\6&28E8DB3B&0&1"), None);
        // Another kind of serial
        assert_eq!(serial_address(ChipVendor::Realtek, r"USB\VID_0BDA&PID_A729\0000000001"), None);
        assert_eq!(serial_address(ChipVendor::Realtek, r"USB\VID_0BDA&PID_A729\00E04C23998Z"), None);
        assert_eq!(serial_address(ChipVendor::Realtek, r"USB\VID_0BDA&PID_A729\000000000000"), None);
        assert_eq!(serial_address(ChipVendor::Realtek, r"USB\VID_0BDA&PID_A729\FFFFFFFFFFFF"), None);
        // Other vendors' serials are not addresses
        assert_eq!(serial_address(ChipVendor::MediaTek, r"USB\VID_0E8D&PID_7925\C0E3502E0009"), None);
        assert_eq!(serial_address(ChipVendor::Intel, r"USB\VID_8087&PID_0033\5&3B72E4CF&0&14"), None);
    }

    #[test]
    fn hci_version_to_core_version() {
        assert_eq!(bt_core_version(0), Some("1.0b"));
        assert_eq!(bt_core_version(6), Some("4.0"));
        assert_eq!(bt_core_version(9), Some("5.0"));
        assert_eq!(bt_core_version(12), Some("5.3"));
        assert_eq!(bt_core_version(14), Some("6.0"));
        assert_eq!(bt_core_version(15), Some("6.1"));
        assert_eq!(bt_core_version(16), None);
        assert_eq!(bt_core_version(0xFF), None);
    }

    #[test]
    fn known_ids_cover_v1s_vendor_class_adapters() {
        let ids = usb_known_ids(&[]);
        // v1's built-in list (BTstack's WinUSB transport)
        for id in [(0x0A5C, 0x21E8), (0x0B05, 0x17CB), (0x0A5C, 0x22BE), (0x2FE3, 0x0100), (0x2FE3, 0x000B)] {
            assert!(ids.contains(&id), "{id:04X?}");
        }
        // The OEM table (TP-Link UB500)
        assert!(ids.contains(&(0x2357, 0x0604)));
        // An enumerated vendor-class adapter, once
        let ids = usb_known_ids(&[(0x1234, 0x5678), (0x0B05, 0x17CB)]);
        assert_eq!(ids.iter().filter(|&&id| id == (0x1234, 0x5678)).count(), 1);
        assert_eq!(ids.iter().filter(|&&id| id == (0x0B05, 0x17CB)).count(), 1);
    }

    #[test]
    fn probe_tells_vendor_class_and_in_use() {
        let vendor = InterfaceProbe { class: UsbClass::VENDOR_BLUETOOTH, num_endpoints: 3, hci_endpoints: true };
        assert_eq!(device(0x1234, 0x5678, Some(Ok(vendor.clone()))).winusb_bluetooth(), Some(true));
        let no_pipes = InterfaceProbe { hci_endpoints: false, ..vendor };
        assert_eq!(device(0x1234, 0x5678, Some(Ok(no_pipes))).winusb_bluetooth(), None);
        assert!(device(0x8087, 0x0029, Some(Err("open failed (error 5)".into()))).in_use());
        assert!(!device(0x8087, 0x0029, Some(Err("open failed (error 2)".into()))).in_use());
        assert!(!device(0x8087, 0x0029, None).in_use());
    }
}
