//! Tests of the chip tables (src/chips.rs)

use bt_host::chips;

use chips::*;
use std::path::PathBuf;

#[test]
fn chip_table_lookups() {
    assert_eq!(realtek_chip_name(0x8771), Some("RTL8761BU"));
    assert_eq!(realtek_fw_name(0x8771), Some("rtl8761bu_fw"));
    assert_eq!(realtek_cfg_name(0x8771), Some("rtl8761bu_config"));
    assert_eq!(realtek_chip_name(CUSTOM_CHIP_PID), Some("Custom"));
    assert_eq!(realtek_chip_name(0x1234), None);
    // PIDs are unique, and every name follows the file naming
    for (i, c) in REALTEK_CHIPS.iter().enumerate() {
        assert!(REALTEK_CHIPS[i + 1..].iter().all(|o| o.pid != c.pid));
        let stem = c.fw_name.strip_suffix("_fw").unwrap();
        assert_eq!(c.cfg_name, format!("{stem}_config"));
    }
    for m in OEM_TABLE {
        assert!(realtek_chip(m.realtek_pid).is_some(), "OEM entry {:04X}:{:04X}", m.vid, m.pid);
    }
}

#[test]
fn realtek_pid_resolution() {
    let btstack = [
        (0x0BDA, 0x8771, "rtl8761bu_fw"),
        (0x0BDA, 0xB82C, "rtl8822bu_fw"),
        (0x0BDA, 0xB85B, "rtl8852bu_fw"),
        (0x0BDA, 0x4853, "rtl8852bu_fw"),
        (0x0BDA, 0xB733, "rtl8723fu_fw"),
        (0x0BDA, 0x1234, "rtl8851bu_fw"),
    ];
    assert_eq!(resolve_realtek_pid(0x0BDA, 0x8771, &btstack), Some(0x8771));
    // Another PID of a family in the chip table: the family's PID (issue #81)
    assert_eq!(resolve_realtek_pid(0x0BDA, 0x4853, &btstack), Some(0xB85B));
    // A family not in the chip table keeps its own PID, and so does one
    // whose chip table PID is not in BTstack's table (here 0xB851)
    assert_eq!(resolve_realtek_pid(0x0BDA, 0xB733, &btstack), Some(0xB733));
    assert_eq!(resolve_realtek_pid(0x0BDA, 0x1234, &btstack), Some(0x1234));
    // Realtek's VID but not in BTstack's table: a card reader, Wi-Fi
    assert_eq!(resolve_realtek_pid(0x0BDA, 0x0129, &btstack), None);
    // TP-Link UB500
    assert_eq!(resolve_realtek_pid(0x2357, 0x0604, &btstack), Some(0x8771));
    // A Realtek PID BTstack lacks, from the OEM table (issue #86)
    assert_eq!(resolve_realtek_pid(0x0BDA, 0xA729, &btstack), Some(0x8771));
    assert_eq!(resolve_realtek_pid(0x8087, 0x0033, &btstack), None);
}

/// Against BTstack's own table: every PID of a family in the chip table gets
/// the family's name and firmware files
#[test]
fn realtek_pid_resolution_btstack_table() {
    let table = bt_host::realtek_usb_firmware();
    assert!(table.iter().any(|&(v, p, fw)| (v, p, fw) == (0x0BDA, 0x4853, "rtl8852bu_fw")), "{table:?}");
    // Every chip table PID is one BTstack loads the same firmware for
    for c in REALTEK_CHIPS.iter().filter(|c| c.pid != CUSTOM_CHIP_PID) {
        assert!(table.iter().any(|&(_, p, fw)| p == c.pid && fw == c.fw_name), "{} 0x{:04X}", c.chip_name, c.pid);
    }
    // A family PID BTstack knows, with the same firmware: what it is given
    let in_btstack = |c: &RealtekChip| table.iter().any(|&(_, p, fw)| p == c.pid && fw == c.fw_name);
    for &(vid, pid, fw) in table {
        // A PID listed twice (0xC125): BTstack takes the first entry
        if table.iter().find(|&&(_, p, _)| p == pid).is_some_and(|&(_, _, f)| f != fw) {
            continue;
        }
        let resolved = resolve_realtek_pid(vid, pid, table).unwrap();
        match REALTEK_CHIPS.iter().find(|c| c.fw_name == fw).filter(|c| in_btstack(c)) {
            Some(c) => assert_eq!(resolved, c.pid, "0x{pid:04X} ({fw})"),
            None => assert_eq!(resolved, pid, "0x{pid:04X} ({fw})"),
        }
        assert!(table.iter().any(|&(_, p, f)| p == resolved && f == fw), "0x{pid:04X} -> 0x{resolved:04X}");
    }
    let rtl8852be = resolve_realtek_pid(0x0BDA, 0x4853, table).unwrap();
    assert_eq!(realtek_chip_name(rtl8852be), Some("RTL8852BU"));
    assert_eq!(
        firmware_files(rtl8852be, ""),
        Some(("rtl8852bu_fw.bin".to_string(), "rtl8852bu_config.bin".to_string()))
    );
}

#[test]
fn vendors_and_names() {
    assert_eq!(vendor_for_usb(0x2357, 0x0604, Some(0x8771)), ChipVendor::Realtek);
    assert_eq!(vendor_for_usb(0x0BDA, 0x0129, None), ChipVendor::Realtek);
    assert_eq!(vendor_for_usb(0x8087, 0x0033, None), ChipVendor::Intel);
    assert_eq!(vendor_for_usb(0x0A5C, 0x21E8, None), ChipVendor::Broadcom);
    assert_eq!(vendor_for_usb(0x0B05, 0x17CB, None), ChipVendor::Broadcom); // ASUS BT400
    assert_eq!(vendor_for_usb(0x0B05, 0x1234, None), ChipVendor::Unknown);
    assert_eq!(vendor_for_usb(0x0A12, 0x0001, None), ChipVendor::Csr);
    assert_eq!(vendor_for_usb(0x0E8D, 0x0608, None), ChipVendor::MediaTek);
    assert_eq!(vendor_for_usb(0x0CF3, 0xE300, None), ChipVendor::Qualcomm);
    assert!(is_bluetooth_vendor_vid(0x8087, 0x0033));
    assert!(!is_bluetooth_vendor_vid(0x0BDA, 0x8771));
    assert!(!is_bluetooth_vendor_vid(0x046D, 0xC52B));
    assert_eq!(ChipVendor::Csr.name(), "CSR");
    assert_eq!(company_name(0x005D), Some("Realtek"));
    assert_eq!(company_name(0x000A), Some("Qualcomm (CSR)"));
    assert_eq!(company_name(0x0009), Some("Infineon"));
    assert_eq!(company_name(0x1234), None);
}

#[test]
fn display_names_like_v1() {
    assert_eq!(display_name(0x2357, 0x0604, Some(0x8771), ChipVendor::Realtek), "RTL8761BU (0x2357:0x0604)");
    assert_eq!(display_name(0x8087, 0x0033, None, ChipVendor::Intel), "Intel (0x8087:0x0033) [experimental]");
    assert_eq!(display_name(0x0E8D, 0x0608, None, ChipVendor::MediaTek), "MediaTek (0x0E8D:0x0608) [unsupported]");
    assert_eq!(display_name(0x0A12, 0x0001, None, ChipVendor::Csr), "CSR (0x0A12:0x0001)");
    assert_eq!(display_name(0x0BDA, 0x1234, None, ChipVendor::Realtek), "0x0BDA:0x1234");
    assert_eq!(display_name(0x1234, 0x5678, None, ChipVendor::Unknown), "0x1234:0x5678");
}

#[test]
fn chip_setting_resolution() {
    assert!(is_auto_chip(0, ""));
    assert!(!is_auto_chip(0, "rtl8723bu"));
    assert!(!is_auto_chip(0x8771, ""));
    // Auto: the first detected Realtek adapter
    assert_eq!(resolve_chip_pid(0, "", [None, Some(0xB82C), Some(0x8771)]), 0xB82C);
    assert_eq!(resolve_chip_pid(0, "", [None, None]), 0);
    // A chosen chip wins over what is detected
    assert_eq!(resolve_chip_pid(0x8771, "", [Some(0xB82C)]), 0x8771);
    assert_eq!(resolve_chip_pid(CUSTOM_CHIP_PID, "", []), CUSTOM_CHIP_PID);
    // Not Realtek: no Realtek init
    assert_eq!(resolve_chip_pid(NON_REALTEK_CHIP_PID, "", [Some(0x8771)]), 0);
    // A chip known only by its files: 0, the stem names the files
    assert_eq!(resolve_chip_pid(0, "rtl8723bu", [Some(0x8771)]), 0);
}

#[test]
fn firmware_file_names() {
    assert_eq!(firmware_files(0x8771, ""), Some(("rtl8761bu_fw.bin".to_string(), "rtl8761bu_config.bin".to_string())));
    assert_eq!(
        firmware_files(0, "rtl8723bu"),
        Some(("rtl8723bu_fw.bin".to_string(), "rtl8723bu_config.bin".to_string()))
    );
    assert_eq!(firmware_files(CUSTOM_CHIP_PID, ""), Some(("custom_fw.bin".into(), "custom_config.bin".into())));
    assert_eq!(firmware_files(0, ""), None);
}

/// Firmware as BTstack's Realtek loader takes it: a signature at the start,
/// the extension signature at the end
fn realtek_firmware(signature: &[u8; 8], len: usize) -> Vec<u8> {
    let mut fw = signature.to_vec();
    fw.resize(len - 4, 0x42);
    fw.extend_from_slice(&[0x51, 0x04, 0xFD, 0x77]);
    fw
}

#[test]
fn firmware_content_rules() {
    assert!(firmware_content_valid(&realtek_firmware(b"RTBTCore", 40_000)));
    assert!(firmware_content_valid(&realtek_firmware(b"Realtech", 30_000)));
    assert!(firmware_content_valid(&realtek_firmware(b"Realtech", 16)));
    // No signature, or no extension signature at the end
    assert!(!firmware_content_valid(&realtek_firmware(b"RTBTCorX", 40_000)));
    let mut cut = realtek_firmware(b"RTBTCore", 40_000);
    cut.pop();
    assert!(!firmware_content_valid(&cut));
    assert!(!firmware_content_valid(b"RTBTCore"));
    assert!(!firmware_content_valid(b""));
    // Saved web pages (issue #66: kernel.org's page starts "<!DOCTYPE")
    let mut page = b"<!DOCTYPE html>
<html lang='en'>"
        .to_vec();
    page.resize(40_000, b' ');
    assert!(!firmware_content_valid(&page));
    assert!(!firmware_content_valid(&[0x42; 40_000]));

    assert!(config_content_valid(b""));
    assert!(config_content_valid(&[0x55, 0xAB, 0x23, 0x87, 0x10, 0x00]));
    assert!(!config_content_valid(&[0x55, 0xAB, 0x23, 0x87])); // magic alone
    assert!(!config_content_valid(b"<!DOCTYPE html>"));
}

/// A scratch folder under the target dir's temp, removed on drop
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let dir = std::env::temp_dir().join(format!("bluecaster_chips_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn firmware_presence_and_scan() {
    let tmp = TempDir::new("scan");
    let dir = &tmp.0;
    let firmware = realtek_firmware(b"RTBTCore", 30_000);
    std::fs::write(dir.join("rtl8761bu_fw.bin"), &firmware).unwrap();
    std::fs::write(dir.join("rtl8761bu_config.bin"), b"").unwrap(); // empty config is fine
    std::fs::write(dir.join("RTL8723BU_FW.BIN"), &firmware).unwrap(); // unknown chip, other case
    std::fs::write(dir.join("rtl8822bu_fw.bin"), b"<!DOCTYPE html>").unwrap(); // a saved web page
    std::fs::write(dir.join("rtl8822bu_config.bin"), [0x55, 0xAB, 0x23, 0x87, 0, 0]).unwrap();
    std::fs::write(dir.join("rtl8821cu_fw.bin"), &firmware).unwrap();
    std::fs::write(dir.join("rtl8821cu_config.bin"), b"<html>").unwrap(); // a page as the config
    std::fs::write(dir.join("custom_fw.bin"), &firmware).unwrap();
    std::fs::write(dir.join("_fw.bin"), &firmware).unwrap(); // no stem
    std::fs::write(dir.join("notes.txt"), b"x").unwrap();

    let ok = FirmwareCheck { present: true, invalid_file: None };
    assert_eq!(firmware_check(dir, 0x8771, ""), Some(ok));
    assert_eq!(
        firmware_check(dir, 0xB82C, ""),
        Some(FirmwareCheck { present: false, invalid_file: Some("rtl8822bu_fw.bin".into()) })
    );
    assert_eq!(
        firmware_check(dir, 0xC821, ""),
        Some(FirmwareCheck { present: false, invalid_file: Some("rtl8821cu_config.bin".into()) })
    );
    // Missing is not invalid
    let missing = FirmwareCheck { present: false, invalid_file: None };
    assert_eq!(firmware_check(dir, 0, "RTL8723BU"), Some(missing.clone())); // no config
    assert_eq!(firmware_check(dir, 0xD723, ""), Some(missing));
    assert_eq!(firmware_check(dir, 0, ""), None);
    assert_eq!(firmware_present(dir, 0x8771, ""), Some(true));
    assert_eq!(firmware_present(dir, 0xB82C, ""), Some(false));
    assert_eq!(firmware_file_state(&dir.join("rtl8761bu_fw.bin")), FileState::Ok);
    assert_eq!(firmware_file_state(&dir.join("rtl8822bu_fw.bin")), FileState::Invalid);
    assert_eq!(firmware_file_state(&dir.join("missing_fw.bin")), FileState::Missing);
    assert_eq!(config_file_state(&dir.join("rtl8761bu_config.bin")), FileState::Ok);

    let found = scan_firmware_files(dir);
    let summary: Vec<(&str, &str, Option<u16>, bool)> =
        found.iter().map(|e| (e.stem.as_str(), e.display_name.as_str(), e.pid, e.has_config)).collect();
    assert_eq!(
        summary,
        [
            ("rtl8761bu", "RTL8761BU", Some(0x8771), true),
            ("rtl8821cu", "RTL8821CU", Some(0xC821), true),
            ("rtl8822bu", "RTL8822BU", Some(0xB82C), true),
            ("RTL8723BU", "RTL8723BU", None, false),
        ]
    );
    assert!(scan_firmware_files(&dir.join("missing")).is_empty());
}
