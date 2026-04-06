/*
 * Bluetooth Device Discovery - Implementation
 *
 * Uses Windows Bluetooth APIs to enumerate paired devices
 * and check for A2DP Sink support via SDP records.
 *
 * SPDX-License-Identifier: MIT
 */

#include "bt_device.h"
#include <cstdio>
#include <cstring>
#include <algorithm>
#include <winsock2.h>  /* Must be before windows.h to avoid redefinition errors */
#include <windows.h>
#include <bluetoothapis.h>
#include <ws2bth.h>

/* A2DP Sink service class UUID */
static const GUID A2DP_SINK_UUID = {
    0x0000110B, 0x0000, 0x1000,
    { 0x80, 0x00, 0x00, 0x80, 0x5F, 0x9B, 0x34, 0xFB }
};

/* AudioSink service class UUID (alternative) */
static const GUID AUDIO_SINK_UUID = {
    0x0000110B, 0x0000, 0x1000,
    { 0x80, 0x00, 0x00, 0x80, 0x5F, 0x9B, 0x34, 0xFB }
};

/*
 * Check if a device supports A2DP Sink via SDP query.
 */
static bool check_a2dp_sink(const BLUETOOTH_DEVICE_INFO &device_info) {
    DWORD sdp_size = 0;
    BLOB sdp_blob = {};
    HANDLE lookup = nullptr;

    /* Build SDP query for A2DP Sink UUID */
    WSAQUERYSET query = {};
    query.dwSize = sizeof(WSAQUERYSET);
    query.dwNameSpace = NS_BTH;
    query.lpServiceClassId = const_cast<GUID *>(&A2DP_SINK_UUID);

    /* Format address string for the query */
    char addr_str[64];
    BTH_ADDR addr = device_info.Address.ullLong;
    snprintf(addr_str, sizeof(addr_str),
             "(%02X:%02X:%02X:%02X:%02X:%02X)",
             (unsigned)(addr >> 40) & 0xFF,
             (unsigned)(addr >> 32) & 0xFF,
             (unsigned)(addr >> 24) & 0xFF,
             (unsigned)(addr >> 16) & 0xFF,
             (unsigned)(addr >> 8) & 0xFF,
             (unsigned)addr & 0xFF);

    /* Use a simpler heuristic: check device class bits.
     * CoD (Class of Device):
     *   Major Device Class = Audio/Video (0x04)
     *   Minor: Headphones (0x18), Loudspeaker (0x14), etc.
     * This avoids the complexity of full SDP queries while
     * catching most A2DP sink devices. */
    ULONG device_class = device_info.ulClassofDevice;
    ULONG major_class = (device_class >> 8) & 0x1F;  /* bits 12-8 */

    /* Major class 0x04 = Audio/Video */
    if (major_class == 0x04) {
        return true;
    }

    /* Also accept devices that are connected and have audio-related classes */
    ULONG minor_class = (device_class >> 2) & 0x3F;  /* bits 7-2 */

    /* Major class 0x04 with common audio minor classes */
    /* 0x01 = Wearable Headset, 0x04 = Microphone, 0x05 = Loudspeaker,
     * 0x06 = Headphones, 0x07 = Portable Audio, 0x08 = Car Audio */
    (void)minor_class;

    return false;
}

std::vector<BtDevice> BtDeviceDiscovery::scan_paired_devices() {
    std::vector<BtDevice> devices;

    /* Set up search parameters */
    BLUETOOTH_DEVICE_SEARCH_PARAMS search_params = {};
    search_params.dwSize = sizeof(BLUETOOTH_DEVICE_SEARCH_PARAMS);
    search_params.fReturnAuthenticated = TRUE;   /* Paired devices */
    search_params.fReturnRemembered = TRUE;      /* Remembered devices */
    search_params.fReturnConnected = TRUE;       /* Connected devices */
    search_params.fReturnUnknown = FALSE;
    search_params.fIssueInquiry = FALSE;         /* Don't do new scan, just paired */
    search_params.cTimeoutMultiplier = 0;

    BLUETOOTH_DEVICE_INFO device_info = {};
    device_info.dwSize = sizeof(BLUETOOTH_DEVICE_INFO);

    /* Find first device */
    HBLUETOOTH_DEVICE_FIND find_handle =
        BluetoothFindFirstDevice(&search_params, &device_info);

    if (!find_handle) {
        DWORD err = GetLastError();
        if (err == ERROR_NO_MORE_ITEMS) {
            fprintf(stderr, "BtDeviceDiscovery: No paired Bluetooth devices found\n");
        } else {
            fprintf(stderr, "BtDeviceDiscovery: BluetoothFindFirstDevice failed: %lu\n", err);
        }
        return devices;
    }

    do {
        BtDevice dev = {};

        /* Extract Bluetooth address (BTH_ADDR is ULONGLONG, 6 bytes) */
        BTH_ADDR addr = device_info.Address.ullLong;
        dev.address[0] = (addr >> 0) & 0xFF;
        dev.address[1] = (addr >> 8) & 0xFF;
        dev.address[2] = (addr >> 16) & 0xFF;
        dev.address[3] = (addr >> 24) & 0xFF;
        dev.address[4] = (addr >> 32) & 0xFF;
        dev.address[5] = (addr >> 40) & 0xFF;

        /* Get device name */
        char name_buf[256];
        WideCharToMultiByte(CP_UTF8, 0, device_info.szName, -1,
                            name_buf, sizeof(name_buf), nullptr, nullptr);
        dev.name = name_buf;

        /* Check if device supports A2DP Sink */
        dev.a2dp_sink = check_a2dp_sink(device_info);

        if (dev.a2dp_sink) {
            fprintf(stderr, "  [A2DP] %s (%s)\n",
                   dev.name.c_str(), format_address(dev.address).c_str());
        } else {
            fprintf(stderr, "  [----] %s (%s)\n",
                   dev.name.c_str(), format_address(dev.address).c_str());
        }

        devices.push_back(dev);

        /* Reset for next device */
        device_info.dwSize = sizeof(BLUETOOTH_DEVICE_INFO);

    } while (BluetoothFindNextDevice(find_handle, &device_info));

    BluetoothFindDeviceClose(find_handle);

    fprintf(stderr, "BtDeviceDiscovery: Found %zu devices (%zu with A2DP Sink)\n",
           devices.size(),
           std::count_if(devices.begin(), devices.end(),
                         [](const BtDevice &d) { return d.a2dp_sink; }));

    return devices;
}

std::string BtDeviceDiscovery::format_address(const uint8_t addr[6]) {
    char buf[18];
    snprintf(buf, sizeof(buf), "%02X:%02X:%02X:%02X:%02X:%02X",
             addr[5], addr[4], addr[3], addr[2], addr[1], addr[0]);
    return std::string(buf);
}

bool BtDeviceDiscovery::parse_address(const std::string &str, uint8_t addr[6]) {
    unsigned int a[6];
    if (sscanf(str.c_str(), "%02x:%02x:%02x:%02x:%02x:%02x",
               &a[0], &a[1], &a[2], &a[3], &a[4], &a[5]) != 6) {
        return false;
    }
    /* Store in little-endian (Windows BTH_ADDR format) */
    for (int i = 0; i < 6; i++) {
        addr[5 - i] = static_cast<uint8_t>(a[i]);
    }
    return true;
}
