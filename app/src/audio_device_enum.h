/*
 * Audio Device Enumeration and Virtual Device Detection
 *
 * Enumerates WASAPI render endpoints, detects virtual audio devices,
 * and provides default device switching via IPolicyConfig.
 *
 * SPDX-License-Identifier: MIT
 */

#ifndef AUDIO_DEVICE_ENUM_H
#define AUDIO_DEVICE_ENUM_H

#include <cstdint>
#include <string>
#include <vector>
#include <windows.h>
#include <mmdeviceapi.h>

struct AudioDeviceInfo {
    std::wstring id;           /* WASAPI device ID (from IMMDevice::GetId) */
    std::string  display_name; /* UTF-8 friendly name */
    bool         is_virtual;   /* virtual device detection result */
    bool         is_default;   /* currently the default device */
};

struct AudioDeviceFormat {
    uint32_t sample_rate = 0;
    uint32_t bits_per_sample = 0;
    uint32_t channels = 0;
    bool valid = false;
};

class AudioDeviceEnumerator {
public:
    AudioDeviceEnumerator();
    ~AudioDeviceEnumerator();

    /* Initialize COM device enumerator. Must be called before enumerate(). */
    bool init();

    /* Release COM resources */
    void shutdown();

    /* Enumerate all active render devices */
    std::vector<AudioDeviceInfo> enumerate();

    /* Enumerate only virtual audio devices */
    std::vector<AudioDeviceInfo> get_virtual_devices();

    /* Set the default audio endpoint for eConsole and eMultimedia roles.
     * Uses the undocumented IPolicyConfig COM interface. */
    static bool set_default_device(const std::wstring &device_id);

    /* Get the current default render device ID */
    static std::wstring get_default_device_id();

    /* Get the configured audio format (sample rate, bit depth) of a device.
     * device_id: empty for default render device. */
    static AudioDeviceFormat get_device_format(const std::wstring &device_id = L"");

private:
    IMMDeviceEnumerator *enumerator_ = nullptr;

    /* Heuristic: check device name patterns, form factor, and container ID */
    static bool is_virtual_device(IMMDevice *device, const std::string &name);
};

#endif /* AUDIO_DEVICE_ENUM_H */
