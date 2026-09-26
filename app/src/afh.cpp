/*
 * AFH host channel classification - Implementation
 * SPDX-License-Identifier: MIT
 */

#include "afh.h"

#include <windows.h>
#include <wlanapi.h>

#include <algorithm>
#include <cstdlib>
#include <map>

namespace {

/* Center frequency of a 2.4 GHz Wi-Fi channel */
int wifi_center_mhz(int channel) {
    return channel == 14 ? 2484 : 2407 + 5 * channel;
}

/* A 20 MHz Wi-Fi signal (22 MHz for 802.11b) spreads +-11 MHz around its center */
const int WIFI_HALF_WIDTH_MHZ = 11;

void mark_bad(uint8_t map[10], int wifi_channel) {
    int center = wifi_center_mhz(wifi_channel);
    for (int n = 0; n < AFH_CHANNELS; n++) {
        int f = 2402 + n;
        if (std::abs(f - center) <= WIFI_HALF_WIDTH_MHZ)
            map[n / 8] &= static_cast<uint8_t>(~(1u << (n % 8)));
    }
}

void all_usable(uint8_t map[10]) {
    for (int i = 0; i < 10; i++) map[i] = 0xFF;
    map[9] = 0x7F;  /* bit 79 is reserved */
}

} // namespace

int afh_count(const uint8_t map[10]) {
    int n = 0;
    for (int i = 0; i < AFH_CHANNELS; i++)
        if (map[i / 8] & (1u << (i % 8))) n++;
    return n;
}

std::string afh_describe(const std::vector<int> &wifi_channels) {
    std::string s;
    for (int c : wifi_channels) {
        if (!s.empty()) s += ", ";
        s += std::to_string(c);
    }
    return s;
}

AfhClassification afh_avoid_wifi(const std::vector<int> &wifi_channels) {
    AfhClassification c;
    all_usable(c.map);
    for (int ch : wifi_channels) {
        if (ch < 1 || ch > 14) continue;
        if (std::find(c.avoided_wifi.begin(), c.avoided_wifi.end(), ch) != c.avoided_wifi.end()) continue;
        uint8_t trial[10];
        std::copy(c.map, c.map + 10, trial);
        mark_bad(trial, ch);
        if (afh_count(trial) < AFH_MIN_CHANNELS) continue;  /* would leave too few */
        std::copy(trial, trial + 10, c.map);
        c.avoided_wifi.push_back(ch);
    }
    c.usable = afh_count(c.map);
    c.passed = !c.avoided_wifi.empty();
    return c;
}

std::vector<WifiAp> afh_survey_wifi() {
    std::vector<WifiAp> aps;
    HANDLE h = nullptr;
    DWORD version = 0;
    if (WlanOpenHandle(2, nullptr, &version, &h) != ERROR_SUCCESS) return aps;
    PWLAN_INTERFACE_INFO_LIST ifs = nullptr;
    if (WlanEnumInterfaces(h, nullptr, &ifs) == ERROR_SUCCESS && ifs) {
        for (DWORD i = 0; i < ifs->dwNumberOfItems; i++) {
            const GUID &guid = ifs->InterfaceInfo[i].InterfaceGuid;
            PWLAN_BSS_LIST list = nullptr;
            if (WlanGetNetworkBssList(h, &guid, nullptr, dot11_BSS_type_any, FALSE, nullptr, &list) ==
                    ERROR_SUCCESS && list) {
                for (DWORD b = 0; b < list->dwNumberOfItems; b++) {
                    const WLAN_BSS_ENTRY &e = list->wlanBssEntries[b];
                    int mhz = static_cast<int>(e.ulChCenterFrequency / 1000);
                    int channel = mhz == 2484 ? 14 : (mhz >= 2412 && mhz <= 2472 ? (mhz - 2407) / 5 : 0);
                    if (channel == 0) continue;  /* not 2.4 GHz */
                    WifiAp ap;
                    ap.channel = channel;
                    ap.rssi_dbm = static_cast<int>(e.lRssi);
                    ap.ssid.assign(reinterpret_cast<const char *>(e.dot11Ssid.ucSSID),
                                   std::min<ULONG>(e.dot11Ssid.uSSIDLength, 32));
                    aps.push_back(ap);
                }
                WlanFreeMemory(list);
            }
        }
        WlanFreeMemory(ifs);
    }
    WlanCloseHandle(h, nullptr);
    return aps;
}

AfhClassification afh_classify(const std::string &policy) {
    if (policy.empty() || policy == "off") {
        AfhClassification c;
        all_usable(c.map);
        return c;
    }
    if (policy == "auto") {
        /* Strongest access point per channel, loudest channels first */
        std::map<int, int> loudest;
        for (const WifiAp &ap : afh_survey_wifi()) {
            if (ap.rssi_dbm < AFH_AUTO_RSSI_DBM) continue;
            auto it = loudest.find(ap.channel);
            if (it == loudest.end() || ap.rssi_dbm > it->second) loudest[ap.channel] = ap.rssi_dbm;
        }
        std::vector<std::pair<int, int>> order(loudest.begin(), loudest.end());
        std::sort(order.begin(), order.end(),
                  [](const auto &a, const auto &b) { return a.second > b.second; });
        std::vector<int> channels;
        for (const auto &o : order) channels.push_back(o.first);
        return afh_avoid_wifi(channels);
    }
    /* "6,11" */
    std::vector<int> channels;
    const char *p = policy.c_str();
    while (*p) {
        char *end = nullptr;
        long v = strtol(p, &end, 10);
        if (end == p) { p++; continue; }
        channels.push_back(static_cast<int>(v));
        p = end;
    }
    return afh_avoid_wifi(channels);
}
