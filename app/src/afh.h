/*
 * AFH host channel classification from the Wi-Fi around
 *
 * Bluetooth BR/EDR hops over 79 channels (2402-2480 MHz). With Adaptive
 * Frequency Hopping the central's controller leaves out channels it finds
 * bad; the host may help with HCI Set AFH Host Channel Classification,
 * marking channels it knows to be busy. A2DPWB is the central of its links,
 * so what it passes applies to headphones and receivers alike.
 *
 * Policies:
 *   "auto"      Wi-Fi access points this PC hears strongly (WLAN API scan),
 *               their 2.4 GHz channels are marked bad
 *   "off"       nothing is passed (the controller decides alone)
 *   "6,11"      these Wi-Fi channels are marked bad
 *
 * At least AFH_MIN_CHANNELS channels stay usable, as the specification
 * requires: channels of the weakest access points are dropped first.
 *
 * SPDX-License-Identifier: MIT
 */

#ifndef AFH_H
#define AFH_H

#include <cstdint>
#include <string>
#include <vector>

constexpr int AFH_CHANNELS = 79;
constexpr int AFH_MIN_CHANNELS = 20;
/* Access points weaker than this are ignored by "auto" */
constexpr int AFH_AUTO_RSSI_DBM = -75;

struct WifiAp {
    int channel = 0;        /* 2.4 GHz channel number, 1-14 */
    int rssi_dbm = -100;
    std::string ssid;
};

struct AfhClassification {
    uint8_t map[10] = {};             /* bit n = Bluetooth channel n: 1 unknown, 0 bad */
    int usable = AFH_CHANNELS;        /* channels left usable */
    std::vector<int> avoided_wifi;    /* Wi-Fi channels marked bad */
    bool passed = false;              /* something to pass to the controller */
};

/* 2.4 GHz access points this PC hears (empty without a Wi-Fi adapter), from
 * the results of Windows' own background scans: requesting scans would put
 * more traffic on the very band being measured */
std::vector<WifiAp> afh_survey_wifi();

/* Classification for a policy string ("auto", "off", "6,11") */
AfhClassification afh_classify(const std::string &policy);

/* Classification that avoids these Wi-Fi channels, keeping AFH_MIN_CHANNELS */
AfhClassification afh_avoid_wifi(const std::vector<int> &wifi_channels);

/* Count of channels set in a 79-bit map */
int afh_count(const uint8_t map[10]);

/* "6, 11" */
std::string afh_describe(const std::vector<int> &wifi_channels);

#endif /* AFH_H */
