/*
 * Audio Device Enumeration and Virtual Device Detection - Implementation
 *
 * Uses WASAPI IMMDeviceEnumerator to list render endpoints, applies
 * heuristics to detect virtual/loopback devices, and switches the
 * system default output via the undocumented IPolicyConfig interface.
 *
 * SPDX-License-Identifier: MIT
 */

#include <initguid.h>
#include "audio_device_enum.h"
#include <mmreg.h>
#include <cstdio>
#include <cstring>
#include <algorithm>
#include <cctype>
#include <functiondiscoverykeys_devpkey.h>

/* ------------------------------------------------------------------ */
/*  IPolicyConfig — undocumented COM interface for default-device      */
/*  switching.  The CLSID below is for Windows 10+ (CPolicyConfigClient). */
/* ------------------------------------------------------------------ */

struct DeviceShareMode; /* opaque, never dereferenced */

MIDL_INTERFACE("568b9108-44bf-40b4-92d0-968cf737f7d7")
IPolicyConfig : public IUnknown {
public:
    virtual HRESULT STDMETHODCALLTYPE GetMixFormat(
        PCWSTR, WAVEFORMATEX **) = 0;
    virtual HRESULT STDMETHODCALLTYPE GetDeviceFormat(
        PCWSTR, INT, WAVEFORMATEX **) = 0;
    virtual HRESULT STDMETHODCALLTYPE ResetDeviceFormat(PCWSTR) = 0;
    virtual HRESULT STDMETHODCALLTYPE SetDeviceFormat(
        PCWSTR, WAVEFORMATEX *, WAVEFORMATEX *) = 0;
    virtual HRESULT STDMETHODCALLTYPE GetProcessingPeriod(
        PCWSTR, INT, PINT64, PINT64) = 0;
    virtual HRESULT STDMETHODCALLTYPE SetProcessingPeriod(
        PCWSTR, PINT64) = 0;
    virtual HRESULT STDMETHODCALLTYPE GetShareMode(
        PCWSTR, DeviceShareMode *) = 0;
    virtual HRESULT STDMETHODCALLTYPE SetShareMode(
        PCWSTR, DeviceShareMode *) = 0;
    virtual HRESULT STDMETHODCALLTYPE GetPropertyValue(
        PCWSTR, BOOL, const PROPERTYKEY &, PROPVARIANT *) = 0;
    virtual HRESULT STDMETHODCALLTYPE SetPropertyValue(
        PCWSTR, BOOL, const PROPERTYKEY &, const PROPVARIANT *) = 0;
    virtual HRESULT STDMETHODCALLTYPE SetDefaultEndpoint(
        PCWSTR pszDeviceId, ERole role) = 0;
    virtual HRESULT STDMETHODCALLTYPE SetEndpointVisibility(
        PCWSTR, INT) = 0;
};

static const CLSID CLSID_CPolicyConfigClient = {
    0x870af99c, 0x171d, 0x4f9e,
    {0xaf, 0x0d, 0xe6, 0x3d, 0xf4, 0x0c, 0x2b, 0xc9}
};

static const IID IID_IPolicyConfig = {
    0x568b9108, 0x44bf, 0x40b4,
    {0x92, 0xd0, 0x96, 0x8c, 0xf7, 0x37, 0xf7, 0xd7}
};

/* WASAPI CLSID/IID — same definitions used by wasapi_capture.cpp */
static const CLSID CLSID_MMDeviceEnumerator_ = __uuidof(MMDeviceEnumerator);
static const IID   IID_IMMDeviceEnumerator_   = __uuidof(IMMDeviceEnumerator);

/* ------------------------------------------------------------------ */
/*  Helpers                                                            */
/* ------------------------------------------------------------------ */

/* Convert a wide string to UTF-8 */
static std::string wstr_to_utf8(const wchar_t *wstr) {
    if (!wstr || !wstr[0]) return {};
    int len = WideCharToMultiByte(CP_UTF8, 0, wstr, -1, nullptr, 0, nullptr, nullptr);
    if (len <= 0) return {};
    std::string out(static_cast<size_t>(len - 1), '\0');
    WideCharToMultiByte(CP_UTF8, 0, wstr, -1, &out[0], len, nullptr, nullptr);
    return out;
}

/* Case-insensitive substring search in a UTF-8 string */
static bool contains_ci(const std::string &haystack, const char *needle) {
    std::string h = haystack;
    std::string n = needle;
    std::transform(h.begin(), h.end(), h.begin(),
                   [](unsigned char c) { return static_cast<char>(std::tolower(c)); });
    std::transform(n.begin(), n.end(), n.begin(),
                   [](unsigned char c) { return static_cast<char>(std::tolower(c)); });
    return h.find(n) != std::string::npos;
}

/* ------------------------------------------------------------------ */
/*  AudioDeviceEnumerator                                              */
/* ------------------------------------------------------------------ */

AudioDeviceEnumerator::AudioDeviceEnumerator() = default;

AudioDeviceEnumerator::~AudioDeviceEnumerator() {
    shutdown();
}

bool AudioDeviceEnumerator::init() {
    if (enumerator_) return true; /* already initialised */

    HRESULT hr = CoCreateInstance(
        CLSID_MMDeviceEnumerator_, nullptr, CLSCTX_ALL,
        IID_IMMDeviceEnumerator_, reinterpret_cast<void **>(&enumerator_)
    );
    if (FAILED(hr)) {
        fprintf(stderr, "AudioDeviceEnumerator: Failed to create enumerator: 0x%08lx\n", hr);
        return false;
    }
    return true;
}

void AudioDeviceEnumerator::shutdown() {
    if (enumerator_) {
        enumerator_->Release();
        enumerator_ = nullptr;
    }
}

/* ------------------------------------------------------------------ */
/*  enumerate()                                                        */
/* ------------------------------------------------------------------ */

std::vector<AudioDeviceInfo> AudioDeviceEnumerator::enumerate() {
    std::vector<AudioDeviceInfo> devices;
    if (!enumerator_) return devices;

    /* Determine current default device ID */
    std::wstring default_id = get_default_device_id();

    /* Enumerate active render endpoints */
    IMMDeviceCollection *collection = nullptr;
    HRESULT hr = enumerator_->EnumAudioEndpoints(
        eRender, DEVICE_STATE_ACTIVE, &collection
    );
    if (FAILED(hr)) {
        fprintf(stderr, "AudioDeviceEnumerator: EnumAudioEndpoints failed: 0x%08lx\n", hr);
        return devices;
    }

    UINT count = 0;
    collection->GetCount(&count);

    for (UINT i = 0; i < count; ++i) {
        IMMDevice *dev = nullptr;
        hr = collection->Item(i, &dev);
        if (FAILED(hr)) continue;

        AudioDeviceInfo info;

        /* Device ID */
        LPWSTR id_raw = nullptr;
        hr = dev->GetId(&id_raw);
        if (SUCCEEDED(hr) && id_raw) {
            info.id = id_raw;
            CoTaskMemFree(id_raw);
        }

        /* Friendly name */
        IPropertyStore *props = nullptr;
        hr = dev->OpenPropertyStore(STGM_READ, &props);
        if (SUCCEEDED(hr)) {
            PROPVARIANT var_name;
            PropVariantInit(&var_name);
            hr = props->GetValue(PKEY_Device_FriendlyName, &var_name);
            if (SUCCEEDED(hr) && var_name.vt == VT_LPWSTR) {
                info.display_name = wstr_to_utf8(var_name.pwszVal);
            }
            PropVariantClear(&var_name);
            props->Release();
        }

        /* Default check */
        info.is_default = (!info.id.empty() && info.id == default_id);

        /* Virtual device heuristic */
        info.is_virtual = is_virtual_device(dev, info.display_name);

        devices.push_back(std::move(info));
        dev->Release();
    }

    collection->Release();
    return devices;
}

/* ------------------------------------------------------------------ */
/*  get_virtual_devices()                                              */
/* ------------------------------------------------------------------ */

std::vector<AudioDeviceInfo> AudioDeviceEnumerator::get_virtual_devices() {
    std::vector<AudioDeviceInfo> all = enumerate();
    std::vector<AudioDeviceInfo> virtuals;
    for (auto &d : all) {
        if (d.is_virtual) {
            virtuals.push_back(std::move(d));
        }
    }
    return virtuals;
}

/* ------------------------------------------------------------------ */
/*  is_virtual_device()                                                */
/* ------------------------------------------------------------------ */

bool AudioDeviceEnumerator::is_virtual_device(IMMDevice *device,
                                              const std::string &name) {
    /* 1. Name-based patterns (case-insensitive) */
    static const char *patterns[] = {
        "CABLE",           /* VB-Cable */
        "VoiceMeeter",
        "Virtual Audio",
        "SAR",             /* Synchronous Audio Router */
        "BlackHole",
        "Stereo Mix",
        "What U Hear",
        "VBVMVAIO",        /* VoiceMeeter driver name */
    };

    for (const char *pattern : patterns) {
        if (contains_ci(name, pattern)) {
            return true;
        }
    }

    /* 2. Property-based checks */
    IPropertyStore *props = nullptr;
    HRESULT hr = device->OpenPropertyStore(STGM_READ, &props);
    if (FAILED(hr)) return false;

    bool result = false;

    /* Check PKEY_AudioEndpoint_FormFactor — virtual devices often report
     * UnknownFormFactor (value 8) */
    {
        PROPVARIANT var;
        PropVariantInit(&var);
        hr = props->GetValue(PKEY_AudioEndpoint_FormFactor, &var);
        if (SUCCEEDED(hr) && var.vt == VT_UI4) {
            /* EndpointFormFactor::UnknownFormFactor == 8 */
            if (var.uintVal == 8) {
                result = true;
            }
        }
        PropVariantClear(&var);
    }

    /* Check PKEY_Device_ContainerId — virtual devices typically have a NULL
     * container or the well-known {00000000-0000-0000-ffff-ffffffffffff} */
    if (!result) {
        PROPVARIANT var;
        PropVariantInit(&var);
        hr = props->GetValue(PKEY_Device_ContainerId, &var);
        if (SUCCEEDED(hr) && var.vt == VT_CLSID && var.puuid) {
            static const GUID null_container =
                {0x00000000, 0x0000, 0x0000,
                 {0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff}};
            static const GUID zero_container =
                {0x00000000, 0x0000, 0x0000,
                 {0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00}};
            if (IsEqualGUID(*var.puuid, null_container) ||
                IsEqualGUID(*var.puuid, zero_container)) {
                result = true;
            }
        }
        PropVariantClear(&var);
    }

    props->Release();
    return result;
}

/* ------------------------------------------------------------------ */
/*  set_default_device()                                               */
/* ------------------------------------------------------------------ */

bool AudioDeviceEnumerator::set_default_device(const std::wstring &device_id) {
    if (device_id.empty()) return false;

    /* Try Windows 10+ CLSID first, then fall back to Windows 7+ CLSID */
    static const CLSID CLSID_PolicyConfigClient_W7 = {
        0x294935CE, 0xF637, 0x4E7C,
        {0xA4, 0x1B, 0xAB, 0x25, 0x54, 0x60, 0xB8, 0x62}
    };

    IPolicyConfig *policy = nullptr;
    HRESULT hr = CoCreateInstance(
        CLSID_CPolicyConfigClient, nullptr, CLSCTX_ALL,
        IID_IPolicyConfig, reinterpret_cast<void **>(&policy)
    );
    if (FAILED(hr) || !policy) {
        /* Fallback: try the Windows 7+ CLSID */
        hr = CoCreateInstance(
            CLSID_PolicyConfigClient_W7, nullptr, CLSCTX_ALL,
            IID_IPolicyConfig, reinterpret_cast<void **>(&policy)
        );
    }
    if (FAILED(hr) || !policy) {
        fprintf(stderr, "AudioDeviceEnumerator: Failed to create IPolicyConfig: 0x%08lx\n", hr);
        return false;
    }

    /* Set as default for both eConsole and eMultimedia roles */
    HRESULT hr1 = policy->SetDefaultEndpoint(device_id.c_str(), eConsole);
    HRESULT hr2 = policy->SetDefaultEndpoint(device_id.c_str(), eMultimedia);

    policy->Release();

    if (FAILED(hr1) || FAILED(hr2)) {
        fprintf(stderr, "AudioDeviceEnumerator: SetDefaultEndpoint failed: "
                "eConsole=0x%08lx, eMultimedia=0x%08lx\n", hr1, hr2);
        return false;
    }

    fprintf(stderr, "AudioDeviceEnumerator: Default device set successfully\n");
    return true;
}

/* ------------------------------------------------------------------ */
/*  get_default_device_id()                                            */
/* ------------------------------------------------------------------ */

std::wstring AudioDeviceEnumerator::get_default_device_id() {
    std::wstring result;

    IMMDeviceEnumerator *enumerator = nullptr;
    HRESULT hr = CoCreateInstance(
        CLSID_MMDeviceEnumerator_, nullptr, CLSCTX_ALL,
        IID_IMMDeviceEnumerator_, reinterpret_cast<void **>(&enumerator)
    );
    if (FAILED(hr)) return result;

    IMMDevice *device = nullptr;
    hr = enumerator->GetDefaultAudioEndpoint(eRender, eConsole, &device);
    if (SUCCEEDED(hr) && device) {
        LPWSTR id_raw = nullptr;
        hr = device->GetId(&id_raw);
        if (SUCCEEDED(hr) && id_raw) {
            result = id_raw;
            CoTaskMemFree(id_raw);
        }
        device->Release();
    }

    enumerator->Release();
    return result;
}

/* ------------------------------------------------------------------ */
/*  get_device_format()                                                */
/* ------------------------------------------------------------------ */

/* PKEY_AudioEngine_DeviceFormat — gives the format configured in
 * Windows Sound Settings (Properties > Advanced). */
static const PROPERTYKEY PKEY_AudioEngine_DeviceFormat_ = {
    { 0xf19f064d, 0x082c, 0x4e27,
      { 0xbc, 0x73, 0x68, 0x82, 0xa1, 0xbb, 0x8e, 0x4c } }, 0
};

AudioDeviceFormat AudioDeviceEnumerator::get_device_format(const std::wstring &device_id) {
    AudioDeviceFormat fmt;

    IMMDeviceEnumerator *enumerator = nullptr;
    HRESULT hr = CoCreateInstance(
        CLSID_MMDeviceEnumerator_, nullptr, CLSCTX_ALL,
        IID_IMMDeviceEnumerator_, reinterpret_cast<void **>(&enumerator));
    if (FAILED(hr)) return fmt;

    IMMDevice *device = nullptr;
    if (device_id.empty()) {
        hr = enumerator->GetDefaultAudioEndpoint(eRender, eConsole, &device);
    } else {
        hr = enumerator->GetDevice(device_id.c_str(), &device);
    }
    if (FAILED(hr) || !device) {
        enumerator->Release();
        return fmt;
    }

    IPropertyStore *props = nullptr;
    hr = device->OpenPropertyStore(STGM_READ, &props);
    if (SUCCEEDED(hr)) {
        PROPVARIANT var;
        PropVariantInit(&var);
        hr = props->GetValue(PKEY_AudioEngine_DeviceFormat_, &var);
        if (SUCCEEDED(hr) && var.vt == VT_BLOB &&
            var.blob.cbSize >= sizeof(WAVEFORMATEX)) {
            auto *wfx = reinterpret_cast<WAVEFORMATEX *>(var.blob.pBlobData);
            fmt.sample_rate = wfx->nSamplesPerSec;
            fmt.channels = wfx->nChannels;
            fmt.bits_per_sample = wfx->wBitsPerSample;
            if (wfx->wFormatTag == WAVE_FORMAT_EXTENSIBLE &&
                var.blob.cbSize >= sizeof(WAVEFORMATEXTENSIBLE)) {
                auto *ext = reinterpret_cast<WAVEFORMATEXTENSIBLE *>(
                    var.blob.pBlobData);
                if (ext->Samples.wValidBitsPerSample > 0)
                    fmt.bits_per_sample = ext->Samples.wValidBitsPerSample;
            }
            fmt.valid = true;
        }
        PropVariantClear(&var);
        props->Release();
    }

    device->Release();
    enumerator->Release();
    return fmt;
}
