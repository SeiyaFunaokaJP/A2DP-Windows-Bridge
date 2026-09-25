/*
 * Bluetooth Adapter Enumeration Implementation
 *
 * Enumerates ALL USB Bluetooth adapters via SetupAPI + WinUSB class check.
 * No OEM mapping tables — unknown adapters are shown as-is and the user
 * selects the Realtek chip type separately if needed.
 *
 * SPDX-License-Identifier: MIT
 */

#include "bt_adapter_enum.h"

#include <windows.h>
#include <setupapi.h>
#include <winusb.h>
#include <usbiodef.h>   /* GUID_DEVINTERFACE_USB_DEVICE */

#include <algorithm>
#include <cctype>
#include <cstdio>
#include <cstring>

/* BTstack Realtek chipset API (C linkage) */
extern "C" {
    uint16_t btstack_chipset_realtek_get_num_usb_controllers(void);
    void btstack_chipset_realtek_get_vendor_product_id(uint16_t index,
                                                       uint16_t *out_vendor_id,
                                                       uint16_t *out_product_id);
}

/* ======================================================================== */
/* Realtek chip database                                                     */
/* Maps representative PID → chip name, firmware filename, config filename.  */
/* Sourced from BTstack's fw_patch_table_usb[].                              */
/* Only lists one PID per chip family (the most common one).                 */
/* ======================================================================== */

static const std::vector<RealtekChipInfo> &chip_db()
{
    static const std::vector<RealtekChipInfo> db = {
        /* Chips with firmware available in linux-firmware (kernel.org) */
        { 0x8771, "RTL8761BU",  "rtl8761bu_fw",  "rtl8761bu_config"  },
        { 0x876C, "RTL8761CU",  "rtl8761cu_fw",  "rtl8761cu_config"  },
        { 0xB82C, "RTL8822BU",  "rtl8822bu_fw",  "rtl8822bu_config"  },
        { 0xC821, "RTL8821CU",  "rtl8821cu_fw",  "rtl8821cu_config"  },
        { 0xC82C, "RTL8822CU",  "rtl8822cu_fw",  "rtl8822cu_config"  },
        { 0xD723, "RTL8723DU",  "rtl8723du_fw",  "rtl8723du_config"  },
        { 0xC852, "RTL8852AU",  "rtl8852au_fw",  "rtl8852au_config"  },
        { 0xB85B, "RTL8852BU",  "rtl8852bu_fw",  "rtl8852bu_config"  },
        { 0xB85D, "RTL8852BTU", "rtl8852btu_fw", "rtl8852btu_config" },
        { 0xC85A, "RTL8852CU",  "rtl8852cu_fw",  "rtl8852cu_config"  },
        { 0xB083, "RTL8851BU",  "rtl8851bu_fw",  "rtl8851bu_config"  },
        { 0x922A, "RTL8922AU",  "rtl8922au_fw",  "rtl8922au_config"  },
        /* User-supplied firmware */
        { CUSTOM_CHIP_PID, "Custom", "custom_fw", "custom_config"     },
    };
    return db;
}

/* ======================================================================== */
/* OEM VID:PID → Realtek chip PID mapping                                   */
/* BTstack's fw_patch_table_usb only stores Realtek PIDs (VID always 0x0BDA).*/
/* OEM adapters (TP-Link, ASUS, Edimax, etc.) use a different VID and often  */
/* a different PID. This table maps known OEM products to the Realtek PID    */
/* that identifies the internal chip (and thus the firmware files needed).    */
/* Sources: linux-firmware, lsusb databases, user reports.                   */
/* ======================================================================== */

/*
 * OEM adapter mapping table.
 *
 * Add new OEM adapters here. Each entry maps an OEM USB VID:PID
 * to the Realtek chip PID used for firmware file lookup (chip_db).
 *
 *   { USB_VID, USB_PID, REALTEK_CHIP_PID }
 *
 * REALTEK_CHIP_PID values (from chip_db above):
 *   0x8771 = RTL8761BU    0x876C = RTL8761CU    0xB82C = RTL8822BU
 *   0xC821 = RTL8821CU    0xC82C = RTL8822CU    0xD723 = RTL8723DU
 *   0xC852 = RTL8852AU    0xB85B = RTL8852BU    0xB85D = RTL8852BTU
 *   0xC85A = RTL8852CU    0xB083 = RTL8851BU    0x922A = RTL8922AU
 */
static const OemChipMapping oem_table[] = {
    /* VID      PID     CHIP           Product                  */
    { 0x2357, 0x0604, 0x8771 },  /* TP-Link UB500             */
    /* --- add verified OEM adapters above this line --- */
    { 0, 0, 0 },                 /* sentinel (do not remove)   */
};

/* Lookup helpers */

static const RealtekChipInfo *find_chip_info(uint16_t pid)
{
    for (const auto &c : chip_db()) {
        if (c.pid == pid) return &c;
    }
    return nullptr;
}

/* Check if PID is in BTstack's Realtek firmware table (all entries use VID=0x0BDA). */
static bool is_in_btstack_table(uint16_t pid)
{
    uint16_t count = btstack_chipset_realtek_get_num_usb_controllers();
    for (uint16_t i = 0; i < count; i++) {
        uint16_t tv, tp;
        btstack_chipset_realtek_get_vendor_product_id(i, &tv, &tp);
        if (tp == pid) return true;
    }
    return false;
}

/* Resolve any VID:PID to a Realtek firmware PID.
 * Returns 0 if the adapter is not a known Realtek-based device. */
static uint16_t resolve_realtek_pid(uint16_t vid, uint16_t pid)
{
    /* Direct Realtek VID — PID is the firmware PID */
    if (vid == 0x0BDA) {
        if (is_in_btstack_table(pid)) return pid;
        return 0;
    }
    /* Check OEM mapping table */
    for (const OemChipMapping *m = oem_table; m->vid != 0; m++) {
        if (m->vid == vid && m->pid == pid) return m->realtek_pid;
    }
    return 0;
}

/* ======================================================================== */
/* Helpers                                                                   */
/* ======================================================================== */

static bool parse_vid_pid(const char *device_path, uint16_t &vid, uint16_t &pid)
{
    const char *v = strstr(device_path, "vid_");
    if (!v) v = strstr(device_path, "VID_");
    if (!v) return false;

    const char *p = strstr(device_path, "pid_");
    if (!p) p = strstr(device_path, "PID_");
    if (!p) return false;

    vid = (uint16_t)strtoul(v + 4, nullptr, 16);
    pid = (uint16_t)strtoul(p + 4, nullptr, 16);
    return true;
}

/* Bluetooth class verification via WinUSB (0xE0/0x01/0x01) */
static bool is_bluetooth_class(const char *device_path)
{
    HANDLE dev = CreateFileA(device_path,
        GENERIC_WRITE | GENERIC_READ,
        FILE_SHARE_WRITE | FILE_SHARE_READ,
        nullptr, OPEN_EXISTING,
        FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OVERLAPPED,
        nullptr);
    if (dev == INVALID_HANDLE_VALUE) return false;

    WINUSB_INTERFACE_HANDLE usb_handle = nullptr;
    BOOL ok = WinUsb_Initialize(dev, &usb_handle);
    if (!ok) {
        CloseHandle(dev);
        return false;
    }

    USB_INTERFACE_DESCRIPTOR desc = {};
    ok = WinUsb_QueryInterfaceSettings(usb_handle, 0, &desc);
    bool is_bt = ok &&
                 desc.bInterfaceClass == 0xE0 &&
                 desc.bInterfaceSubClass == 0x01 &&
                 desc.bInterfaceProtocol == 0x01;

    WinUsb_Free(usb_handle);
    CloseHandle(dev);
    return is_bt;
}

static std::string build_display_name(uint16_t vid, uint16_t pid, uint16_t realtek_pid,
                                      BtChipVendor vendor)
{
    char buf[80];
    if (realtek_pid != 0) {
        const char *chip = BtAdapterEnumerator::realtek_chip_name(realtek_pid);
        if (chip) {
            snprintf(buf, sizeof(buf), "%s (0x%04X:0x%04X)", chip, vid, pid);
            return buf;
        }
    }

    const char *tag = nullptr;
    switch (vendor) {
    case BtChipVendor::Intel:
    case BtChipVendor::Broadcom: tag = " [experimental]"; break;
    case BtChipVendor::MediaTek:
    case BtChipVendor::Qualcomm: tag = " [unsupported]"; break;
    default: break;
    }
    if (vendor != BtChipVendor::Unknown && vendor != BtChipVendor::Realtek) {
        snprintf(buf, sizeof(buf), "%s (0x%04X:0x%04X)%s",
                 BtAdapterEnumerator::vendor_name(vendor), vid, pid, tag ? tag : "");
        return buf;
    }

    /* Unknown — show VID:PID only */
    snprintf(buf, sizeof(buf), "0x%04X:0x%04X", vid, pid);
    return buf;
}

/* ======================================================================== */
/* Public API                                                                */
/* ======================================================================== */

const char *BtAdapterEnumerator::realtek_chip_name(uint16_t pid)
{
    /* First check our curated chip DB (has nice names) */
    auto *info = find_chip_info(pid);
    if (info) return info->chip_name;
    return nullptr;
}

const char *BtAdapterEnumerator::realtek_fw_name(uint16_t pid)
{
    auto *info = find_chip_info(pid);
    return info ? info->fw_name : nullptr;
}

const char *BtAdapterEnumerator::realtek_cfg_name(uint16_t pid)
{
    auto *info = find_chip_info(pid);
    return info ? info->cfg_name : nullptr;
}

const std::vector<RealtekChipInfo> &BtAdapterEnumerator::get_realtek_chips()
{
    return chip_db();
}

const OemChipMapping *BtAdapterEnumerator::get_oem_table()
{
    return oem_table;
}

BtChipVendor BtAdapterEnumerator::vendor_for_usb(uint16_t vid, uint16_t pid, uint16_t realtek_pid)
{
    if (realtek_pid != 0) return BtChipVendor::Realtek;
    switch (vid) {
    case 0x0BDA: return BtChipVendor::Realtek;
    case 0x8087: return BtChipVendor::Intel;
    case 0x0A5C: return BtChipVendor::Broadcom;
    case 0x0A12: return BtChipVendor::Csr;
    case 0x0E8D: return BtChipVendor::MediaTek;
    case 0x0CF3: return BtChipVendor::Qualcomm;   /* Qualcomm Atheros */
    default: break;
    }
    /* OEM adapters with a Broadcom chip under their own VID */
    if (vid == 0x0B05 && pid == 0x17CB) return BtChipVendor::Broadcom;  /* ASUS BT400 */
    return BtChipVendor::Unknown;
}

const char *BtAdapterEnumerator::vendor_name(BtChipVendor vendor)
{
    switch (vendor) {
    case BtChipVendor::Realtek:  return "Realtek";
    case BtChipVendor::Intel:    return "Intel";
    case BtChipVendor::Broadcom: return "Broadcom";
    case BtChipVendor::Csr:      return "CSR";
    case BtChipVendor::MediaTek: return "MediaTek";
    case BtChipVendor::Qualcomm: return "Qualcomm";
    default:                     return "Unknown";
    }
}

const char *BtAdapterEnumerator::company_name(uint16_t company_id)
{
    switch (company_id) {
    case 0x0002: return "Intel";
    case 0x000A: return "Qualcomm (CSR)";
    case 0x000F: return "Broadcom";
    case 0x001D: return "Qualcomm";
    case 0x0045: return "Atheros";
    case 0x0046: return "MediaTek";
    case 0x005D: return "Realtek";
    case 0x0131: return "Cypress";
    case 0x0009: return "Infineon";
    default:     return nullptr;
    }
}

std::vector<FirmwareFileEntry> BtAdapterEnumerator::scan_firmware_files(const std::string &config_dir)
{
    std::vector<FirmwareFileEntry> result;

    /* Scan for *_fw.bin files */
    std::string pattern = config_dir + "\\*_fw.bin";
    WIN32_FIND_DATAA fd;
    HANDLE hFind = FindFirstFileA(pattern.c_str(), &fd);
    if (hFind == INVALID_HANDLE_VALUE) return result;

    do {
        std::string fname = fd.cFileName;
        /* Strip _fw.bin suffix to get stem */
        const std::string suffix = "_fw.bin";
        if (fname.size() <= suffix.size()) continue;
        std::string stem = fname.substr(0, fname.size() - suffix.size());

        /* Skip the "custom" stem — it's added separately by the UI */
        {
            std::string lower_stem = stem;
            std::transform(lower_stem.begin(), lower_stem.end(), lower_stem.begin(),
                           [](unsigned char c) { return (char)std::tolower(c); });
            if (lower_stem == "custom") continue;
        }

        /* Check if matching config file exists */
        std::string cfg_path = config_dir + "\\" + stem + "_config.bin";
        bool has_config = (GetFileAttributesA(cfg_path.c_str()) != INVALID_FILE_ATTRIBUTES);

        /* Reverse-lookup against chip_db: match stem + "_fw" to entry's fw_name */
        std::string fw_key = stem + "_fw";
        uint16_t pid = 0;
        std::string display_name;
        for (const auto &c : chip_db()) {
            if (c.pid == CUSTOM_CHIP_PID) continue;
            if (fw_key == c.fw_name) {
                pid = c.pid;
                display_name = c.chip_name;
                break;
            }
        }

        /* Unknown chip: use uppercase stem as display name */
        if (display_name.empty()) {
            display_name = stem;
            std::transform(display_name.begin(), display_name.end(),
                           display_name.begin(),
                           [](unsigned char c) { return (char)std::toupper(c); });
        }

        result.push_back({stem, display_name, pid, has_config});
    } while (FindNextFileA(hFind, &fd));

    FindClose(hFind);

    /* Sort: known chips (pid != 0) first, then unknown */
    std::sort(result.begin(), result.end(), [](const FirmwareFileEntry &a, const FirmwareFileEntry &b) {
        if ((a.pid != 0) != (b.pid != 0)) return a.pid != 0;
        return a.display_name < b.display_name;
    });

    fprintf(stderr, "FirmwareScan: found %zu firmware file(s) in %s\n",
            result.size(), config_dir.c_str());
    for (const auto &e : result) {
        fprintf(stderr, "  %s (pid=0x%04X, config=%s)\n",
                e.display_name.c_str(), e.pid, e.has_config ? "yes" : "no");
    }

    return result;
}

std::vector<BtAdapterInfo> BtAdapterEnumerator::enumerate()
{
    std::vector<BtAdapterInfo> result;

    HDEVINFO dev_info = SetupDiGetClassDevsA(
        &GUID_DEVINTERFACE_USB_DEVICE, nullptr, nullptr,
        DIGCF_DEVICEINTERFACE | DIGCF_PRESENT);
    if (dev_info == INVALID_HANDLE_VALUE) return result;

    SP_DEVICE_INTERFACE_DATA intf_data = {};
    intf_data.cbSize = sizeof(SP_DEVICE_INTERFACE_DATA);

    for (DWORD idx = 0;
         SetupDiEnumDeviceInterfaces(dev_info, nullptr,
                                     &GUID_DEVINTERFACE_USB_DEVICE,
                                     idx, &intf_data);
         idx++)
    {
        DWORD size = 0;
        SetupDiGetDeviceInterfaceDetailA(dev_info, &intf_data, nullptr, 0, &size, nullptr);
        if (size == 0) continue;

        auto *detail = (SP_DEVICE_INTERFACE_DETAIL_DATA_A *)HeapAlloc(
            GetProcessHeap(), HEAP_ZERO_MEMORY, size);
        if (!detail) continue;
        detail->cbSize = sizeof(SP_DEVICE_INTERFACE_DETAIL_DATA_A);

        SP_DEVINFO_DATA dev_data = {};
        dev_data.cbSize = sizeof(SP_DEVINFO_DATA);

        if (!SetupDiGetDeviceInterfaceDetailA(dev_info, &intf_data,
                                              detail, size, &size, &dev_data)) {
            HeapFree(GetProcessHeap(), 0, detail);
            continue;
        }

        const char *path = detail->DevicePath;

        uint16_t vid = 0, pid = 0;
        if (!parse_vid_pid(path, vid, pid)) {
            HeapFree(GetProcessHeap(), 0, detail);
            continue;
        }

        /* Skip VMware virtual adapters */
        if (vid == 0x0E0F) {
            HeapFree(GetProcessHeap(), 0, detail);
            continue;
        }

        /* Determine if this is a Bluetooth adapter.
         * First check Realtek tables (BTstack + OEM mapping).
         * Fall back to WinUSB BT class check for non-Realtek adapters. */
        bool is_bt = false;
        uint16_t realtek_pid = resolve_realtek_pid(vid, pid);
        if (realtek_pid != 0) {
            is_bt = true;
        } else {
            is_bt = is_bluetooth_class(path);
        }

        if (is_bt) {
            BtAdapterInfo info;
            info.vid = vid;
            info.pid = pid;
            info.device_path = path;
            info.realtek_pid = realtek_pid;
            info.vendor = vendor_for_usb(vid, pid, realtek_pid);

            info.display_name = build_display_name(vid, pid, info.realtek_pid, info.vendor);

            /* Avoid duplicates */
            bool dup = false;
            for (const auto &existing : result) {
                if (existing.vid == vid && existing.pid == pid) {
                    dup = true;
                    break;
                }
            }
            if (!dup) result.push_back(std::move(info));
        }

        HeapFree(GetProcessHeap(), 0, detail);
    }

    SetupDiDestroyDeviceInfoList(dev_info);

    fprintf(stderr, "BtAdapterEnumerator: found %zu adapter(s)\n", result.size());
    for (const auto &a : result) {
        fprintf(stderr, "  %s (vid=%04X pid=%04X rtk_pid=0x%04X vendor=%s)\n",
                a.display_name.c_str(), a.vid, a.pid, a.realtek_pid, vendor_name(a.vendor));
    }

    return result;
}
