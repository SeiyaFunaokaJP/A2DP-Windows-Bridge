/*
 * Bluetooth Adapter Enumeration & Realtek Chip Info
 *
 * Enumerates USB Bluetooth adapters using Windows SetupAPI + WinUSB.
 * Provides Realtek chip metadata (PID → firmware filename) for firmware
 * download and chipset initialization.
 *
 * SPDX-License-Identifier: MIT
 */

#ifndef BT_ADAPTER_ENUM_H
#define BT_ADAPTER_ENUM_H

#include <cstdint>
#include <string>
#include <vector>

/* Controller vendor, guessed from the USB VID:PID. Decides which firmware
 * loader (if any) runs before HCI init. Only Realtek is well tested; Intel
 * and Broadcom loaders are experimental; MediaTek and Qualcomm need
 * loaders BTstack does not have. */
enum class BtChipVendor : uint8_t {
    Unknown,
    Realtek,    /* rtl*_fw.bin + rtl*_config.bin (btstack_chipset_realtek) */
    Intel,      /* ibt-*.sfi + ibt-*.ddc, legacy bootloader only (experimental) */
    Broadcom,   /* optional *.hcd PatchRAM (experimental) */
    Csr,        /* ROM firmware, nothing to load */
    MediaTek,   /* needs WMT firmware download: not supported */
    Qualcomm,   /* needs rampatch/NVM download: not supported */
};

struct BtAdapterInfo {
    uint16_t    vid;            /* USB Vendor ID */
    uint16_t    pid;            /* USB Product ID */
    uint16_t    realtek_pid;    /* Realtek PID for firmware lookup (0 if unknown) */
    BtChipVendor vendor = BtChipVendor::Unknown;
    std::string display_name;   /* e.g. "2357:0604" or "Realtek RTL8761BU" */
    std::string device_path;    /* Full USB device path from SetupAPI */
};

/* Known Realtek chip family for user selection */
struct RealtekChipInfo {
    uint16_t    pid;            /* Representative PID for firmware table lookup */
    const char *chip_name;      /* e.g. "RTL8761BU" */
    const char *fw_name;        /* Firmware filename without .bin, e.g. "rtl8761bu_fw" */
    const char *cfg_name;       /* Config filename without .bin, e.g. "rtl8761bu_config" */
};

/* Firmware file pair found in config directory */
struct FirmwareFileEntry {
    std::string stem;          /* e.g. "rtl8761bu" (derived from filename) */
    std::string display_name;  /* e.g. "RTL8761BU" (from chip_db) or uppercase stem */
    uint16_t    pid;           /* From chip_db if matched, else 0 */
    bool        has_config;    /* true if matching *_config.bin also exists */
};

/* Sentinel PID for user-supplied custom firmware */
static constexpr uint16_t CUSTOM_CHIP_PID = 0xFFFF;

/* OEM VID:PID → Realtek chip PID mapping for non-0x0BDA adapters */
struct OemChipMapping {
    uint16_t vid;
    uint16_t pid;
    uint16_t realtek_pid;   /* maps to RealtekChipInfo::pid */
};

class BtAdapterEnumerator {
public:
    /* Enumerate all connected USB Bluetooth adapters. */
    static std::vector<BtAdapterInfo> enumerate();

    /* Scan config directory for *_fw.bin / *_config.bin pairs. */
    static std::vector<FirmwareFileEntry> scan_firmware_files(const std::string &config_dir);

    /* Get human-readable chip name for a Realtek PID (nullptr if unknown). */
    static const char *realtek_chip_name(uint16_t pid);

    /* Get firmware/config filenames for a Realtek PID (nullptr if unknown). */
    static const char *realtek_fw_name(uint16_t pid);
    static const char *realtek_cfg_name(uint16_t pid);

    /* Get the list of known Realtek chip families (for UI dropdown). */
    static const std::vector<RealtekChipInfo> &get_realtek_chips();

    /* Guess the controller vendor from a USB VID:PID (realtek_pid from
     * enumerate(); nonzero means Realtek regardless of VID). */
    static BtChipVendor vendor_for_usb(uint16_t vid, uint16_t pid, uint16_t realtek_pid);

    /* Short vendor name for logs and the adapter list ("Intel", ...). */
    static const char *vendor_name(BtChipVendor vendor);

    /* Name for a Bluetooth SIG company ID from HCI Read Local Version
     * (nullptr if not one we care about). */
    static const char *company_name(uint16_t company_id);

    /* Get the OEM VID:PID mapping table (sentinel: vid==0).
     * Used by btstack_transport to register OEM devices with HCI. */
    static const OemChipMapping *get_oem_table();
};

#endif /* BT_ADAPTER_ENUM_H */
