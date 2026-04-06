/*
 * Bluetooth Device Discovery
 *
 * Discovers Bluetooth A2DP Sink devices using Windows Bluetooth APIs.
 *
 * SPDX-License-Identifier: MIT
 */

#ifndef BT_DEVICE_H
#define BT_DEVICE_H

#include <cstdint>
#include <string>
#include <vector>

struct BtDevice {
    uint8_t address[6];     /* Bluetooth address */
    std::string name;       /* Device friendly name */
    bool a2dp_sink;         /* Supports A2DP Sink profile */
};

class BtDeviceDiscovery {
public:
    /*
     * Scan for paired Bluetooth devices that support A2DP Sink.
     * Returns list of discovered devices.
     */
    static std::vector<BtDevice> scan_paired_devices();

    /* Format a Bluetooth address as XX:XX:XX:XX:XX:XX */
    static std::string format_address(const uint8_t addr[6]);

    /* Parse a Bluetooth address from XX:XX:XX:XX:XX:XX string */
    static bool parse_address(const std::string &str, uint8_t addr[6]);
};

#endif /* BT_DEVICE_H */
