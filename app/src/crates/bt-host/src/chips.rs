//! Controller chips: the static tables of v1 bt_adapter_enum.cpp
//!
//! - the Realtek chip table (representative PID -> chip name, firmware and
//!   config file stems), for the chip setting and the firmware check
//! - OEM adapters with a Realtek chip under another VID:PID
//! - the vendor guessed from a USB VID:PID, and company names from HCI Read
//!   Local Version
//! - the chip setting's "auto" resolution
//! - the firmware file rules (v1 a2dp_service.cpp check_firmware_present)
//!
//! Plain data and std only: the USB enumeration (win-sys::usb_adapters) and
//! BTstack's Realtek firmware table (btstack-sys::realtek_usb_firmware) are
//! passed in by the caller.

use std::path::{Path, PathBuf};

/// Realtek's USB vendor ID
pub const REALTEK_VID: u16 = 0x0BDA;
/// Chip setting: user-supplied firmware (custom_fw.bin / custom_config.bin)
pub const CUSTOM_CHIP_PID: u16 = 0xFFFF;
/// Chip setting: not a Realtek adapter, skip the Realtek chipset init. The
/// setting's other values: 0 without a firmware stem = Auto (decided from
/// the detected adapter), 0 with a stem = firmware files of a chip not in
/// the table, else a Realtek PID or CUSTOM_CHIP_PID.
pub const NON_REALTEK_CHIP_PID: u16 = 0xFFFE;

/// A Realtek chip family
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RealtekChip {
    /// Representative PID, the key of the chip setting
    pub pid: u16,
    /// e.g. "RTL8761BU"
    pub chip_name: &'static str,
    /// Firmware file name without .bin, e.g. "rtl8761bu_fw"
    pub fw_name: &'static str,
    /// Config file name without .bin, e.g. "rtl8761bu_config"
    pub cfg_name: &'static str,
}

const fn chip(pid: u16, chip_name: &'static str, fw_name: &'static str, cfg_name: &'static str) -> RealtekChip {
    RealtekChip { pid, chip_name, fw_name, cfg_name }
}

/// Known Realtek chip families, one PID each (the most common one), from
/// BTstack's fw_patch_table_usb. All but Custom have firmware in
/// linux-firmware (kernel.org). Each PID must be one BTstack's table has
/// with the same firmware: BTstack picks the files by the PID it is given,
/// and loads none for a PID it does not know. The family's other PIDs (0x4853 and the
/// other RTL8852BE modules for RTL8852BU, ...) resolve to this one by their
/// firmware name: see resolve_realtek_pid.
pub static REALTEK_CHIPS: &[RealtekChip] = &[
    chip(0x8771, "RTL8761BU", "rtl8761bu_fw", "rtl8761bu_config"),
    chip(0xB82C, "RTL8822BU", "rtl8822bu_fw", "rtl8822bu_config"),
    chip(0xC821, "RTL8821CU", "rtl8821cu_fw", "rtl8821cu_config"),
    chip(0xC82C, "RTL8822CU", "rtl8822cu_fw", "rtl8822cu_config"),
    chip(0xD723, "RTL8723DU", "rtl8723du_fw", "rtl8723du_config"),
    chip(0xC852, "RTL8852AU", "rtl8852au_fw", "rtl8852au_config"),
    chip(0xB85B, "RTL8852BU", "rtl8852bu_fw", "rtl8852bu_config"),
    chip(0xC85A, "RTL8852CU", "rtl8852cu_fw", "rtl8852cu_config"),
    chip(0xB851, "RTL8851BU", "rtl8851bu_fw", "rtl8851bu_config"),
    // User-supplied firmware
    chip(CUSTOM_CHIP_PID, "Custom", "custom_fw", "custom_config"),
];

/// An OEM adapter (TP-Link, ASUS, Edimax, ...) with a Realtek chip under its
/// own VID:PID, or a Realtek PID BTstack's Realtek table does not hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OemChipMapping {
    pub vid: u16,
    pub pid: u16,
    /// Key into REALTEK_CHIPS
    pub realtek_pid: u16,
}

/// Verified OEM adapters (sources: linux-firmware, lsusb databases, user
/// reports). Add new ones here; BTstack's transport also registers these
/// VID:PIDs so it opens them.
pub static OEM_TABLE: &[OemChipMapping] = &[
    OemChipMapping { vid: 0x2357, pid: 0x0604, realtek_pid: 0x8771 }, // TP-Link UB500 (RTL8761BU)
    // Generic RTL8761B dongles (issues #66, #86: rtl8761bu firmware)
    OemChipMapping { vid: 0x0BDA, pid: 0xA729, realtek_pid: 0x8771 },
];

/// The controller vendor, guessed from the USB VID:PID. Decides which
/// firmware loader (if any) runs before HCI init. Only Realtek is well
/// tested; Intel and Broadcom loaders are experimental; MediaTek and
/// Qualcomm need loaders BTstack does not have.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChipVendor {
    #[default]
    Unknown,
    /// rtl*_fw.bin + rtl*_config.bin (btstack_chipset_realtek)
    Realtek,
    /// ibt-*.sfi + ibt-*.ddc, legacy bootloader only (experimental)
    Intel,
    /// optional *.hcd PatchRAM (experimental)
    Broadcom,
    /// ROM firmware, nothing to load
    Csr,
    /// needs WMT firmware download: not supported
    MediaTek,
    /// needs rampatch/NVM download: not supported
    Qualcomm,
}

impl ChipVendor {
    /// Short name for logs and the adapter list
    pub fn name(self) -> &'static str {
        match self {
            ChipVendor::Realtek => "Realtek",
            ChipVendor::Intel => "Intel",
            ChipVendor::Broadcom => "Broadcom",
            ChipVendor::Csr => "CSR",
            ChipVendor::MediaTek => "MediaTek",
            ChipVendor::Qualcomm => "Qualcomm",
            ChipVendor::Unknown => "Unknown",
        }
    }

    /// Tag shown after the name: loaders that are experimental or missing
    pub fn support_tag(self) -> Option<&'static str> {
        match self {
            ChipVendor::Intel | ChipVendor::Broadcom => Some("experimental"),
            ChipVendor::MediaTek | ChipVendor::Qualcomm => Some("unsupported"),
            _ => None,
        }
    }
}

/// The chip table entry of a Realtek PID
pub fn realtek_chip(pid: u16) -> Option<&'static RealtekChip> {
    REALTEK_CHIPS.iter().find(|c| c.pid == pid)
}

pub fn realtek_chip_name(pid: u16) -> Option<&'static str> {
    realtek_chip(pid).map(|c| c.chip_name)
}

pub fn realtek_fw_name(pid: u16) -> Option<&'static str> {
    realtek_chip(pid).map(|c| c.fw_name)
}

pub fn realtek_cfg_name(pid: u16) -> Option<&'static str> {
    realtek_chip(pid).map(|c| c.cfg_name)
}

/// The Realtek PID for firmware lookup of any USB VID:PID: Realtek's own
/// VID when BTstack's firmware table has the PID (`btstack_table`: VID,
/// PID, firmware name, from btstack-sys::realtek_usb_firmware), else the
/// OEM table. A PID of a family in REALTEK_CHIPS gives the family's PID
/// when BTstack's table has that one with the same firmware (BTstack picks
/// the files by the PID it is given, so it loads the same ones): an RTL8852BE
/// module at 0BDA:4853 is the RTL8852BU of 0xB85B. None if not a known
/// Realtek-based adapter.
pub fn resolve_realtek_pid(vid: u16, pid: u16, btstack_table: &[(u16, u16, &str)]) -> Option<u16> {
    let oem = OEM_TABLE.iter().find(|m| m.vid == vid && m.pid == pid).map(|m| m.realtek_pid);
    if vid == REALTEK_VID && oem.is_none() {
        // Realtek's VID is also on card readers and Wi-Fi: only PIDs
        // BTstack or the OEM table know are Bluetooth
        let &(_, _, fw_name) = btstack_table.iter().find(|&&(v, p, _)| v == vid && p == pid)?;
        let family = REALTEK_CHIPS.iter().find(|c| {
            c.fw_name == fw_name && btstack_table.iter().any(|&(v, p, f)| v == vid && p == c.pid && f == fw_name)
        });
        let resolved = family.map_or(pid, |c| c.pid);
        tracing::debug!("realtek: 0x{vid:04X}:0x{pid:04X} in BTstack's table ({fw_name}), chip PID 0x{resolved:04X}");
        return Some(resolved);
    }
    if let Some(r) = oem {
        tracing::debug!("realtek: 0x{vid:04X}:0x{pid:04X} is an OEM adapter, chip PID 0x{r:04X}");
    }
    oem
}

/// The vendor of a USB VID:PID (`realtek_pid` from resolve_realtek_pid:
/// Some means Realtek whatever the VID)
pub fn vendor_for_usb(vid: u16, pid: u16, realtek_pid: Option<u16>) -> ChipVendor {
    if realtek_pid.is_some() {
        return ChipVendor::Realtek;
    }
    match vid {
        0x0BDA => ChipVendor::Realtek,
        0x8087 => ChipVendor::Intel,
        0x0A5C => ChipVendor::Broadcom,
        0x0A12 => ChipVendor::Csr,
        0x0E8D => ChipVendor::MediaTek,
        0x0CF3 => ChipVendor::Qualcomm, // Qualcomm Atheros
        // OEM adapters with a Broadcom chip under their own VID
        0x0B05 if pid == 0x17CB => ChipVendor::Broadcom, // ASUS BT400
        _ => ChipVendor::Unknown,
    }
}

/// A VID only Bluetooth controllers use (not Realtek's: it is also on card
/// readers and Wi-Fi). With it, a device with no driver and no Bluetooth
/// compatible ID (a vendor-class adapter such as the BT400) is still
/// listed, as v1's enumeration log did.
pub fn is_bluetooth_vendor_vid(vid: u16, pid: u16) -> bool {
    !matches!(vendor_for_usb(vid, pid, None), ChipVendor::Unknown | ChipVendor::Realtek)
}

/// Name for a Bluetooth SIG company ID from HCI Read Local Version (None if
/// not one BlueCaster cares about)
pub fn company_name(company_id: u16) -> Option<&'static str> {
    Some(match company_id {
        0x0002 => "Intel",
        0x000A => "Qualcomm (CSR)",
        0x000F => "Broadcom",
        0x001D => "Qualcomm",
        0x0045 => "Atheros",
        0x0046 => "MediaTek",
        0x005D => "Realtek",
        0x0131 => "Cypress",
        0x0009 => "Infineon",
        _ => return None,
    })
}

/// The adapter's name in the list: "RTL8761BU (0x2357:0x0604)" for a known
/// Realtek chip, "Intel (0x8087:0x0033) [experimental]" for another known
/// vendor, else "0x1234:0x5678"
pub fn display_name(vid: u16, pid: u16, realtek_pid: Option<u16>, vendor: ChipVendor) -> String {
    if let Some(chip) = realtek_pid.and_then(realtek_chip_name) {
        return format!("{chip} (0x{vid:04X}:0x{pid:04X})");
    }
    if vendor != ChipVendor::Unknown && vendor != ChipVendor::Realtek {
        let tag = vendor.support_tag().map(|t| format!(" [{t}]")).unwrap_or_default();
        return format!("{} (0x{vid:04X}:0x{pid:04X}){tag}", vendor.name());
    }
    format!("0x{vid:04X}:0x{pid:04X}")
}

/// True if the chip setting is Auto
pub fn is_auto_chip(setting_pid: u16, fw_stem: &str) -> bool {
    setting_pid == 0 && fw_stem.is_empty()
}

/// The Realtek PID the transport should use for a chip setting (0 = no
/// Realtek chipset init). Auto takes the first detected Realtek adapter:
/// `detected` are the realtek_pid values of the enumerated adapters, in
/// enumeration order.
pub fn resolve_chip_pid(setting_pid: u16, fw_stem: &str, detected: impl IntoIterator<Item = Option<u16>>) -> u16 {
    if setting_pid == NON_REALTEK_CHIP_PID {
        return 0;
    }
    if !is_auto_chip(setting_pid, fw_stem) {
        return setting_pid;
    }
    detected.into_iter().flatten().next().unwrap_or(0)
}

/// Firmware and config file names for a resolved chip PID, or for the
/// firmware stem of a chip known only by its files ("rtl8723bu" ->
/// rtl8723bu_fw.bin / rtl8723bu_config.bin). None if there is no Realtek
/// chip to load (another vendor, or Not Realtek).
pub fn firmware_files(chip_pid: u16, fw_stem: &str) -> Option<(String, String)> {
    if let Some(c) = realtek_chip(chip_pid) {
        return Some((format!("{}.bin", c.fw_name), format!("{}.bin", c.cfg_name)));
    }
    (!fw_stem.is_empty()).then(|| (format!("{fw_stem}_fw.bin"), format!("{fw_stem}_config.bin")))
}

/// Signatures BTstack's Realtek loader checks (btstack_chipset_realtek.c):
/// the firmware starts with one of these ...
const FW_SIGNATURES: [&[u8; 8]; 2] = [b"Realtech", b"RTBTCore"];
/// ... and ends with the extension signature
const FW_EXTENSION_SIGNATURE: [u8; 4] = [0x51, 0x04, 0xFD, 0x77];
/// A config file starts with 0x8723AB55 (little-endian)
const CONFIG_MAGIC: [u8; 4] = [0x55, 0xAB, 0x23, 0x87];

/// A firmware or config file in the settings folder: missing, or there but
/// not what BTstack's Realtek loader accepts. A web page saved instead of
/// the file is the usual case (issue #66: kernel.org's preview page instead
/// of the "plain" link).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileState {
    Missing,
    Invalid,
    Ok,
}

/// Realtek firmware as BTstack accepts it: "Realtech" or "RTBTCore" at the
/// start, the extension signature at the end. v1 only rejected files under
/// 10 KB or starting with "<!doctype" (case-sensitively, so the
/// "<!DOCTYPE html>" of kernel.org's page passed).
pub fn firmware_content_valid(data: &[u8]) -> bool {
    data.len() >= 16
        && FW_SIGNATURES.iter().any(|sig| data.starts_with(*sig))
        && data.ends_with(&FW_EXTENSION_SIGNATURE)
}

/// A Realtek config file: empty (BTstack then uses the chip's efuse), or
/// starting with its magic
pub fn config_content_valid(data: &[u8]) -> bool {
    data.is_empty() || (data.len() >= 6 && data.starts_with(&CONFIG_MAGIC))
}

fn file_state(path: &Path, valid: fn(&[u8]) -> bool) -> FileState {
    match std::fs::read(path) {
        Ok(data) if valid(&data) => FileState::Ok,
        Ok(_) => FileState::Invalid,
        Err(_) => FileState::Missing,
    }
}

/// [`firmware_content_valid`] for a file
pub fn firmware_file_state(path: &Path) -> FileState {
    file_state(path, firmware_content_valid)
}

/// [`config_content_valid`] for a file
pub fn config_file_state(path: &Path) -> FileState {
    file_state(path, config_content_valid)
}

/// The firmware of a chip setting, as [`firmware_check`] found it
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirmwareCheck {
    /// Both files there and valid
    pub present: bool,
    /// Name of a file that is there but invalid (the firmware first)
    pub invalid_file: Option<String>,
}

/// Whether the firmware for a chip setting is in `dir` (v1
/// check_firmware_present): the firmware and the config file there and
/// valid. None if the setting names no Realtek firmware to check.
pub fn firmware_check(dir: &Path, chip_pid: u16, fw_stem: &str) -> Option<FirmwareCheck> {
    let (fw, cfg) = firmware_files(chip_pid, fw_stem)?;
    let fw_state = firmware_file_state(&dir.join(&fw));
    let cfg_state = config_file_state(&dir.join(&cfg));
    tracing::debug!(
        "firmware check: chip 0x{chip_pid:04X} in {}: {fw} {fw_state:?}, {cfg} {cfg_state:?}",
        dir.display()
    );
    let invalid_file = if fw_state == FileState::Invalid {
        Some(fw)
    } else if cfg_state == FileState::Invalid {
        Some(cfg)
    } else {
        None
    };
    Some(FirmwareCheck { present: fw_state == FileState::Ok && cfg_state == FileState::Ok, invalid_file })
}

/// [`firmware_check`]'s verdict alone
pub fn firmware_present(dir: &Path, chip_pid: u16, fw_stem: &str) -> Option<bool> {
    firmware_check(dir, chip_pid, fw_stem).map(|c| c.present)
}

/// A firmware pair found in the config folder
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirmwareFileEntry {
    /// e.g. "rtl8761bu" (the file name without "_fw.bin")
    pub stem: String,
    /// From the chip table, else the stem in capitals ("RTL8723BU")
    pub display_name: String,
    /// From the chip table if matched
    pub pid: Option<u16>,
    /// The matching *_config.bin exists
    pub has_config: bool,
    pub fw_path: PathBuf,
}

/// The *_fw.bin files in `dir` (v1 scan_firmware_files): known chips first,
/// then by name. "custom" is left out: the UI adds it separately.
pub fn scan_firmware_files(dir: &Path) -> Vec<FirmwareFileEntry> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut result: Vec<FirmwareFileEntry> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            // Windows file names ignore case, as v1's FindFirstFile did
            let stem_len = name.len().checked_sub("_fw.bin".len()).filter(|&n| n > 0)?;
            if !name.get(stem_len..)?.eq_ignore_ascii_case("_fw.bin") {
                return None;
            }
            let stem = name[..stem_len].to_string();
            if stem.eq_ignore_ascii_case("custom") {
                return None;
            }
            let known = REALTEK_CHIPS
                .iter()
                .filter(|c| c.pid != CUSTOM_CHIP_PID)
                .find(|c| c.fw_name.strip_suffix("_fw").is_some_and(|s| s.eq_ignore_ascii_case(&stem)));
            Some(FirmwareFileEntry {
                has_config: dir.join(format!("{stem}_config.bin")).is_file(),
                display_name: known.map_or_else(|| stem.to_ascii_uppercase(), |c| c.chip_name.to_string()),
                pid: known.map(|c| c.pid),
                fw_path: e.path(),
                stem,
            })
        })
        .collect();
    result.sort_by(|a, b| b.pid.is_some().cmp(&a.pid.is_some()).then_with(|| a.display_name.cmp(&b.display_name)));
    result
}
