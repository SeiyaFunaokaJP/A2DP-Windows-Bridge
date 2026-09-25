/*
 * Application Settings - JSON-based persistent configuration
 * SPDX-License-Identifier: MIT
 */

#ifndef APP_SETTINGS_H
#define APP_SETTINGS_H

#include <string>
#include <cstdint>

#include "media_payload_limit.h"

class AppSettings {
public:
    /* Load settings from %APPDATA%\A2DPWB\settings.json */
    void load();

    /* Save current settings to disk */
    void save() const;

    /* General */
    std::string language = "en";
    std::string theme = "dark";

    /* Startup */
    bool start_with_windows = false;
    bool start_minimized = false;
    bool minimize_to_tray = true;

    /* Updates */
    bool check_updates_on_startup = true;
    std::string last_update_check; /* ISO date of the last automatic check */

    /* Debug */
    bool debug_mode = false;

    /* Advanced: max media packet size in bytes (media_payload_limit.h) */
    uint16_t max_media_payload = MEDIA_PAYLOAD_LIMIT_DEFAULT;

    /* Bluetooth adapter — Realtek chip type for firmware loading.
     * 0 = auto (works for VID=0x0BDA adapters).
     * Nonzero = specific Realtek PID (e.g. 0x8771 for RTL8761BU).
     * Required for OEM adapters (TP-Link, ASUS, etc.) where VID != 0x0BDA. */
    uint16_t bt_chip_pid = 0;

    /* Firmware stem for chips not in chip_db (pid==0 but firmware files present).
     * e.g. "rtl8761bu" → looks for rtl8761bu_fw.bin + rtl8761bu_config.bin. */
    std::string bt_chip_fw_stem;

    /* Window state */
    std::string last_profile;

private:
    std::string get_settings_path() const;
};

#endif /* APP_SETTINGS_H */
