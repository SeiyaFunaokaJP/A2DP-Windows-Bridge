/*
 * Profile Manager - Connection profile persistence
 * SPDX-License-Identifier: MIT
 */

#include "profile_manager.h"
#include "config_path.h"
#include <nlohmann/json.hpp>
#include <fstream>
#include <cstdio>

using json = nlohmann::json;

std::string ProfileManager::get_profiles_path() const {
    return get_config_dir() + "\\profiles.json";
}

void ProfileManager::load() {
    profiles_.clear();
    std::ifstream ifs(get_profiles_path());
    if (!ifs.is_open()) return;

    try {
        json j = json::parse(ifs);
        if (!j.contains("profiles") || !j["profiles"].is_array()) return;

        for (auto &item : j["profiles"]) {
            ConnectionProfile p;
            p.name           = item.value("name", "");
            p.device_address = item.value("device_address", "");
            p.device_name    = item.value("device_name", "");
            p.codec          = item.value("codec", "auto");
            p.quality        = item.value("quality", "hq");
            p.abr            = item.value("abr", false);
            p.sample_rate    = item.value("sample_rate", 0u);
            p.bit_depth      = item.value("bit_depth", 0u);
            p.capture_mode     = item.value("capture_mode", std::string("loopback"));
            p.audio_device_id  = item.value("audio_device_id", std::string());
            p.audio_device_name = item.value("audio_device_name", std::string());
            p.auto_switch_device = item.value("auto_switch_device", true);
            if (!p.name.empty() && !p.device_address.empty()) {
                profiles_.push_back(std::move(p));
            }
        }
    } catch (...) {
        /* Ignore parse errors; keep whatever was loaded */
    }
}

void ProfileManager::save() const {
    json arr = json::array();
    for (auto &p : profiles_) {
        arr.push_back({
            {"name",           p.name},
            {"device_address", p.device_address},
            {"device_name",    p.device_name},
            {"codec",          p.codec},
            {"quality",        p.quality},
            {"abr",            p.abr},
            {"sample_rate",    p.sample_rate},
            {"bit_depth",      p.bit_depth},
            {"capture_mode",     p.capture_mode},
            {"audio_device_id",  p.audio_device_id},
            {"audio_device_name", p.audio_device_name},
            {"auto_switch_device", p.auto_switch_device}
        });
    }

    json j;
    j["profiles"] = arr;

    std::string path = get_profiles_path();
    fprintf(stderr, "ProfileManager::save: path='%s'\n", path.c_str());
    std::ofstream ofs(path);
    if (ofs.is_open()) {
        ofs << j.dump(2) << std::endl;
        fprintf(stderr, "ProfileManager::save: wrote %zu profiles OK\n", profiles_.size());
    } else {
        fprintf(stderr, "ProfileManager::save: FAILED to open file!\n");
    }
}

size_t ProfileManager::add(const ConnectionProfile &profile) {
    profiles_.push_back(profile);
    save();
    return profiles_.size() - 1;
}

bool ProfileManager::update(size_t index, const ConnectionProfile &profile) {
    if (index >= profiles_.size()) return false;
    profiles_[index] = profile;
    save();
    return true;
}

bool ProfileManager::remove(size_t index) {
    if (index >= profiles_.size()) return false;
    profiles_.erase(profiles_.begin() + static_cast<ptrdiff_t>(index));
    save();
    return true;
}

int ProfileManager::find_by_name(const std::string &name) const {
    for (size_t i = 0; i < profiles_.size(); i++) {
        if (profiles_[i].name == name) return static_cast<int>(i);
    }
    return -1;
}

int ProfileManager::codec_to_index(const std::string &codec) {
    if (codec == "auto")   return 0;
    if (codec == "ldac")   return 1;
    if (codec == "aptxhd") return 2;
    if (codec == "aptxll") return 3;
    if (codec == "sbc")    return 4;
    if (codec == "aac")    return 5;
    return 0;
}

std::string ProfileManager::index_to_codec(int index) {
    switch (index) {
    case 1: return "ldac";
    case 2: return "aptxhd";
    case 3: return "aptxll";
    case 4: return "sbc";
    case 5: return "aac";
    default: return "auto";
    }
}

int ProfileManager::quality_to_index(const std::string &quality) {
    if (quality == "hq") return 0;
    if (quality == "sq") return 1;
    if (quality == "mq") return 2;
    return 0;
}

std::string ProfileManager::index_to_quality(int index) {
    switch (index) {
    case 1: return "sq";
    case 2: return "mq";
    default: return "hq";
    }
}

int ProfileManager::bit_depth_to_index(uint32_t bit_depth) {
    if (bit_depth == 16) return 1;
    if (bit_depth == 24) return 2;
    return 0; /* auto */
}

uint32_t ProfileManager::index_to_bit_depth(int index) {
    switch (index) {
    case 1: return 16;
    case 2: return 24;
    default: return 0; /* auto */
    }
}

int ProfileManager::capture_mode_to_index(const std::string &mode) {
    if (mode == "virtual") return 1;
    return 0; /* "loopback" or default (including legacy "apo") */
}

std::string ProfileManager::index_to_capture_mode(int index) {
    switch (index) {
    case 1: return "virtual";
    default: return "loopback";
    }
}
