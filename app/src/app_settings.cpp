/*
 * Application Settings - JSON-based persistent configuration
 * SPDX-License-Identifier: MIT
 */

#include "app_settings.h"
#include "config_path.h"

#include <nlohmann/json.hpp>

#include <fstream>
#include <string>

using json = nlohmann::json;

std::string AppSettings::get_settings_path() const
{
    return get_config_dir() + "settings.json";
}

void AppSettings::load()
{
    std::ifstream ifs(get_settings_path());
    if (!ifs.is_open())
        return;

    json j;
    try {
        ifs >> j;
    } catch (const json::parse_error &) {
        return;
    }

    /* General */
    language = j.value("language", language);
    theme    = j.value("theme", theme);

    /* Startup */
    start_with_windows = j.value("start_with_windows", start_with_windows);
    start_minimized    = j.value("start_minimized", start_minimized);
    minimize_to_tray   = j.value("minimize_to_tray", minimize_to_tray);

    /* Debug */
    debug_mode = j.value("debug_mode", debug_mode);

    /* Bluetooth adapter */
    bt_chip_pid = static_cast<uint16_t>(j.value("bt_chip_pid", static_cast<int>(bt_chip_pid)));
    bt_chip_fw_stem = j.value("bt_chip_fw_stem", bt_chip_fw_stem);

    /* Window state */
    last_profile  = j.value("last_profile", last_profile);
}

void AppSettings::save() const
{
    json j;

    /* General */
    j["language"] = language;
    j["theme"]    = theme;

    /* Startup */
    j["start_with_windows"] = start_with_windows;
    j["start_minimized"]    = start_minimized;
    j["minimize_to_tray"]   = minimize_to_tray;

    /* Debug */
    j["debug_mode"] = debug_mode;

    /* Bluetooth adapter */
    j["bt_chip_pid"] = static_cast<int>(bt_chip_pid);
    j["bt_chip_fw_stem"] = bt_chip_fw_stem;

    /* Window state */
    j["last_profile"]  = last_profile;

    std::ofstream ofs(get_settings_path());
    if (ofs.is_open())
        ofs << j.dump(2) << std::endl;
}
