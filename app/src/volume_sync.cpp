/*
 * Volume Sync - one volume for Windows and the headphones
 *
 * SPDX-License-Identifier: MIT
 */

#include "volume_sync.h"
#include "btstack_transport.h"
#include "debug_log.h"

#include <mmdeviceapi.h>
#include <endpointvolume.h>

namespace {

/* After a Windows change is sent to the headphones, their volume may still
 * report older values for a moment (queued command, confirmation in flight).
 * Wait before reading it as a change made on the headphones. */
const DWORD HOLD_MS = 1000;

int scalar_to_level(float scalar) {
    int level = static_cast<int>(scalar * 127.0f + 0.5f);
    return level < 0 ? 0 : level > 127 ? 127 : level;
}

int percent(int level) { return (level * 100 + 63) / 127; }

} /* namespace */

VolumeSync::VolumeSync(const std::wstring &device_id) : fixed_id_(device_id) {}

VolumeSync::~VolumeSync() {
    stop();
    if (enumerator_) {
        enumerator_->Release();
        enumerator_ = nullptr;
    }
}

void VolumeSync::stop() {
    detach();
}

bool VolumeSync::attach() {
    if (!enumerator_ &&
        FAILED(CoCreateInstance(__uuidof(MMDeviceEnumerator), nullptr, CLSCTX_ALL,
                                __uuidof(IMMDeviceEnumerator),
                                reinterpret_cast<void **>(&enumerator_)))) {
        enumerator_ = nullptr;
        return false;
    }

    /* The default output device can change while streaming: follow it */
    std::wstring id = fixed_id_;
    if (id.empty()) {
        IMMDevice *device = nullptr;
        if (FAILED(enumerator_->GetDefaultAudioEndpoint(eRender, eConsole, &device)))
            return false;
        LPWSTR wid = nullptr;
        if (SUCCEEDED(device->GetId(&wid)) && wid) {
            id = wid;
            CoTaskMemFree(wid);
        }
        device->Release();
        if (id.empty()) return false;
    }
    if (endpoint_ && id == id_) return true;

    detach();
    IMMDevice *device = nullptr;
    if (FAILED(enumerator_->GetDevice(id.c_str(), &device))) return false;
    HRESULT hr = device->Activate(__uuidof(IAudioEndpointVolume), CLSCTX_ALL, nullptr,
                                  reinterpret_cast<void **>(&endpoint_));
    device->Release();
    if (FAILED(hr)) {
        endpoint_ = nullptr;
        return false;
    }
    id_ = id;
    LOG_INFO("VolumeSync: syncing the headphone volume with the %s",
             fixed_id_.empty() ? "default output device" : "selected output device");
    return true;
}

void VolumeSync::detach() {
    if (endpoint_) {
        /* Outside streaming the device plays on its own again: give it its
         * previous volume back */
        if (original_ >= 0.0f) {
            endpoint_->SetMasterVolumeLevelScalar(original_, nullptr);
            LOG_INFO("VolumeSync: Windows volume restored to %d%%",
                     percent(scalar_to_level(original_)));
        }
        endpoint_->Release();
        endpoint_ = nullptr;
    }
    id_.clear();
    original_ = -1.0f;
    last_windows_ = -1;
    last_headphones_ = -1;
}

int VolumeSync::read_level() const {
    float scalar = 0.0f;
    if (!endpoint_ || FAILED(endpoint_->GetMasterVolumeLevelScalar(&scalar))) return -1;
    return scalar_to_level(scalar);
}

void VolumeSync::tick(bool enabled) {
    if (!enabled) {
        stop();
        return;
    }
    int headphones = BtStackTransport::remote_volume();
    if (headphones < 0) {
        /* No AVRCP volume (yet, or the link dropped): when it comes back,
         * Windows takes over the headphones' volume again */
        last_windows_ = -1;
        last_headphones_ = -1;
        return;
    }
    if (!attach()) return;
    int windows = read_level();
    if (windows < 0) return;

    auto set_windows = [&](int level) {
        if (original_ < 0.0f && FAILED(endpoint_->GetMasterVolumeLevelScalar(&original_)))
            original_ = -1.0f;
        endpoint_->SetMasterVolumeLevelScalar(level / 127.0f, nullptr);
        /* What the device actually took (it may have volume steps) */
        int taken = read_level();
        last_windows_ = taken >= 0 ? taken : level;
        last_headphones_ = level;
    };

    DWORD now = GetTickCount();
    if (last_windows_ < 0 || last_headphones_ < 0) {
        /* Start in sync with what the headphones play at */
        LOG_INFO("VolumeSync: Windows volume %d%% -> %d%% (headphones)",
                 percent(windows), percent(headphones));
        set_windows(headphones);
        return;
    }
    if (windows != last_windows_) {
        /* Volume keys, the flyout or the mixer: the headphones follow */
        BtStackTransport::request_volume(static_cast<uint8_t>(windows));
        last_windows_ = windows;
        last_headphones_ = windows;
        hold_until_ = now + HOLD_MS;
        return;
    }
    if (static_cast<LONG>(now - hold_until_) < 0) return;
    if (headphones != last_headphones_) {
        /* The headphones' buttons or gestures, or the app's slider */
        set_windows(headphones);
    }
}
