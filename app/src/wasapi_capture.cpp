/*
 * WASAPI Loopback Audio Capture - Implementation
 *
 * Uses WASAPI in shared loopback mode to capture system audio output.
 * The captured PCM data is delivered via callback from a dedicated thread.
 *
 * SPDX-License-Identifier: MIT
 */

#include "wasapi_capture.h"
#include "debug_log.h"
#include <cstdio>
#include <cstring>
#include <avrt.h>
#include <timeapi.h>
#include <functiondiscoverykeys_devpkey.h>
#include <audioclientactivationparams.h>

/* The SDK declares these only for NTDDI_WIN10_FE (build 20348) and later,
 * but the app targets Windows 10 in general and checks the build at run time
 * (app_audio::capture_supported). Same layout as audioclientactivationparams.h. */
#ifndef VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK
#define VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK L"VAD\\Process_Loopback"
enum PROCESS_LOOPBACK_MODE {
    PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE = 0,
    PROCESS_LOOPBACK_MODE_EXCLUDE_TARGET_PROCESS_TREE = 1
};
struct AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS {
    DWORD TargetProcessId;
    PROCESS_LOOPBACK_MODE ProcessLoopbackMode;
};
enum AUDIOCLIENT_ACTIVATION_TYPE {
    AUDIOCLIENT_ACTIVATION_TYPE_DEFAULT = 0,
    AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK = 1
};
struct AUDIOCLIENT_ACTIVATION_PARAMS {
    AUDIOCLIENT_ACTIVATION_TYPE ActivationType;
    union {
        AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS ProcessLoopbackParams;
    };
};
#endif

/* WASAPI CLSID/IID - defined here to avoid linking issues */
static const CLSID CLSID_MMDeviceEnumerator_ = __uuidof(MMDeviceEnumerator);
static const IID IID_IMMDeviceEnumerator_ = __uuidof(IMMDeviceEnumerator);
static const IID IID_IAudioClient_ = __uuidof(IAudioClient);
static const IID IID_IAudioCaptureClient_ = __uuidof(IAudioCaptureClient);

WasapiCapture::WasapiCapture() {
    stop_event_ = CreateEventW(nullptr, TRUE, FALSE, nullptr);
    buffer_event_ = CreateEventW(nullptr, FALSE, FALSE, nullptr); /* auto-reset */
}

WasapiCapture::~WasapiCapture() {
    stop();

    if (capture_client_) { capture_client_->Release(); capture_client_ = nullptr; }
    if (audio_client_) { audio_client_->Release(); audio_client_ = nullptr; }
    if (device_) { device_->Release(); device_ = nullptr; }
    if (enumerator_) { enumerator_->Release(); enumerator_ = nullptr; }
    if (buffer_event_) { CloseHandle(buffer_event_); buffer_event_ = nullptr; }
    if (stop_event_) { CloseHandle(stop_event_); stop_event_ = nullptr; }
}

bool WasapiCapture::init(uint32_t preferred_sample_rate, const wchar_t *device_id) {
    HRESULT hr;

    /* Initialize COM on this thread if not already done */
    hr = CoInitializeEx(nullptr, COINIT_MULTITHREADED);
    if (FAILED(hr) && hr != RPC_E_CHANGED_MODE && hr != S_FALSE) {
        fprintf(stderr, "WasapiCapture: CoInitializeEx failed: 0x%08lx\n", hr);
        return false;
    }

    /* Create device enumerator */
    hr = CoCreateInstance(
        CLSID_MMDeviceEnumerator_, nullptr, CLSCTX_ALL,
        IID_IMMDeviceEnumerator_, reinterpret_cast<void **>(&enumerator_)
    );
    if (FAILED(hr)) {
        fprintf(stderr, "WasapiCapture: Failed to create device enumerator: 0x%08lx\n", hr);
        return false;
    }

    /* Get audio output device — either by explicit ID or system default */
    if (device_id) {
        hr = enumerator_->GetDevice(device_id, &device_);
        if (FAILED(hr)) {
            fprintf(stderr, "WasapiCapture: Failed to get device by ID: 0x%08lx\n", hr);
            return false;
        }
    } else {
        hr = enumerator_->GetDefaultAudioEndpoint(eRender, eConsole, &device_);
        if (FAILED(hr)) {
            fprintf(stderr, "WasapiCapture: No default audio output device: 0x%08lx\n", hr);
            return false;
        }
    }

    /* Print device name */
    IPropertyStore *props = nullptr;
    hr = device_->OpenPropertyStore(STGM_READ, &props);
    if (SUCCEEDED(hr)) {
        PROPVARIANT varName;
        PropVariantInit(&varName);
        hr = props->GetValue(PKEY_Device_FriendlyName, &varName);
        if (SUCCEEDED(hr) && varName.vt == VT_LPWSTR) {
            fprintf(stderr, "WasapiCapture: Using device: %ls\n", varName.pwszVal);
        }
        PropVariantClear(&varName);
        props->Release();
    }

    /* Activate IAudioClient */
    hr = device_->Activate(
        IID_IAudioClient_, CLSCTX_ALL, nullptr,
        reinterpret_cast<void **>(&audio_client_)
    );
    if (FAILED(hr)) {
        fprintf(stderr, "WasapiCapture: Failed to activate audio client: 0x%08lx\n", hr);
        return false;
    }

    /* Get the mix format (system output format) */
    WAVEFORMATEX *mix_format = nullptr;
    hr = audio_client_->GetMixFormat(&mix_format);
    if (FAILED(hr)) {
        fprintf(stderr, "WasapiCapture: Failed to get mix format: 0x%08lx\n", hr);
        return false;
    }

    channels_ = mix_format->nChannels;

    /* Use container size (wBitsPerSample) for buffer calculations and callback.
     * The audio callback dispatches on 32 (float) vs 16 (int) — using valid bits
     * instead of container bits would cause 24-in-32 formats to be dropped. */
    bits_per_sample_ = mix_format->wBitsPerSample;
    if (mix_format->wFormatTag == WAVE_FORMAT_EXTENSIBLE) {
        WAVEFORMATEXTENSIBLE *ext = reinterpret_cast<WAVEFORMATEXTENSIBLE *>(mix_format);
        uint32_t valid_bits = ext->Samples.wValidBitsPerSample;
        if (valid_bits != 0 && valid_bits != bits_per_sample_) {
            fprintf(stderr, "WasapiCapture: Note: valid bits (%u) != container bits (%u)\n",
                    valid_bits, bits_per_sample_);
        }
    }

    /* Try preferred sample rate if specified and different from system default */
    WAVEFORMATEX *init_format = mix_format;
    WAVEFORMATEXTENSIBLE custom_format_ext = {};
    bool use_custom = false;

    if (preferred_sample_rate > 0 && preferred_sample_rate != mix_format->nSamplesPerSec) {
        /* Build a custom format based on mix_format but with the preferred rate */
        if (mix_format->wFormatTag == WAVE_FORMAT_EXTENSIBLE) {
            custom_format_ext = *reinterpret_cast<WAVEFORMATEXTENSIBLE *>(mix_format);
        } else {
            custom_format_ext.Format = *mix_format;
        }
        
        custom_format_ext.Format.nSamplesPerSec = preferred_sample_rate;
        custom_format_ext.Format.nAvgBytesPerSec = preferred_sample_rate * mix_format->nBlockAlign;

        init_format = reinterpret_cast<WAVEFORMATEX *>(&custom_format_ext);
        use_custom = true;
        fprintf(stderr, "WasapiCapture: Requesting custom sample rate: %u Hz (system: %u Hz)\n",
                preferred_sample_rate, (uint32_t)mix_format->nSamplesPerSec);
    }

    sample_rate_ = init_format->nSamplesPerSec;

    fprintf(stderr, "WasapiCapture: Format: %u Hz, %u ch, %u bit (container: %u bit)\n",
           sample_rate_, channels_, bits_per_sample_, init_format->wBitsPerSample);

    /* Initialize audio client in loopback mode with event-driven buffering.
     * Event-driven mode avoids timer-resolution issues (default 15.6ms) that
     * cause data discontinuity when polling with WaitForSingleObject. */
    REFERENCE_TIME buf_duration = static_cast<REFERENCE_TIME>(BUFFER_DURATION_MS) * 10000;

#ifndef AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
#define AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM 0x80000000
#endif
#ifndef AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY
#define AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY 0x08000000
#endif

    DWORD stream_flags = AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_EVENTCALLBACK;
    if (use_custom) {
        stream_flags |= AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY;
    }

    hr = audio_client_->Initialize(
        AUDCLNT_SHAREMODE_SHARED,
        stream_flags,
        buf_duration,
        0,                  /* periodicity (0 = default for shared mode) */
        init_format,
        nullptr             /* session GUID */
    );

    if (FAILED(hr)) {
        /* Fallback: some drivers don't support event-driven loopback */
        fprintf(stderr, "WasapiCapture: Event-driven init failed (0x%08lx), trying polling mode\n", hr);
        hr = audio_client_->Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_LOOPBACK,
            buf_duration,
            0,
            init_format,
            nullptr
        );
    } else {
        /* Set the event handle for event-driven mode */
        hr = audio_client_->SetEventHandle(buffer_event_);
        if (FAILED(hr)) {
            fprintf(stderr, "WasapiCapture: SetEventHandle failed: 0x%08lx\n", hr);
            CoTaskMemFree(mix_format);
            return false;
        }
        fprintf(stderr, "WasapiCapture: Using event-driven capture\n");
    }

    CoTaskMemFree(mix_format);

    if (FAILED(hr)) {
        fprintf(stderr, "WasapiCapture: Failed to initialize audio client: 0x%08lx\n", hr);
        return false;
    }

    /* Get capture client */
    hr = audio_client_->GetService(
        IID_IAudioCaptureClient_,
        reinterpret_cast<void **>(&capture_client_)
    );
    if (FAILED(hr)) {
        fprintf(stderr, "WasapiCapture: Failed to get capture client: 0x%08lx\n", hr);
        return false;
    }

    fprintf(stderr, "WasapiCapture: Initialized successfully\n");
    return true;
}

/* Completion handler for ActivateAudioInterfaceAsync. It must be agile
 * (IAgileObject) because Windows calls it on one of its own MTA threads. */
namespace {
class ActivateHandler : public IActivateAudioInterfaceCompletionHandler, public IAgileObject {
public:
    ActivateHandler() { done_ = CreateEventW(nullptr, TRUE, FALSE, nullptr); }
    ~ActivateHandler() { if (done_) CloseHandle(done_); }

    STDMETHODIMP QueryInterface(REFIID riid, void **ppv) override {
        if (riid == __uuidof(IUnknown) || riid == __uuidof(IActivateAudioInterfaceCompletionHandler)) {
            *ppv = static_cast<IActivateAudioInterfaceCompletionHandler *>(this);
        } else if (riid == __uuidof(IAgileObject)) {
            *ppv = static_cast<IAgileObject *>(this);
        } else {
            *ppv = nullptr;
            return E_NOINTERFACE;
        }
        AddRef();
        return S_OK;
    }
    STDMETHODIMP_(ULONG) AddRef() override { return InterlockedIncrement(&refs_); }
    STDMETHODIMP_(ULONG) Release() override {
        ULONG r = InterlockedDecrement(&refs_);
        if (r == 0) delete this;
        return r;
    }
    STDMETHODIMP ActivateCompleted(IActivateAudioInterfaceAsyncOperation *op) override {
        HRESULT activate_hr = E_FAIL;
        IUnknown *unk = nullptr;
        result_ = op->GetActivateResult(&activate_hr, &unk);
        if (SUCCEEDED(result_)) result_ = activate_hr;
        if (SUCCEEDED(result_) && unk)
            result_ = unk->QueryInterface(__uuidof(IAudioClient), reinterpret_cast<void **>(&client_));
        if (unk) unk->Release();
        SetEvent(done_);
        return S_OK;
    }

    HANDLE done_ = nullptr;
    HRESULT result_ = E_FAIL;
    IAudioClient *client_ = nullptr;  /* owned by the caller once done_ is set */

private:
    LONG refs_ = 1;
};
} /* namespace */

/* Activate a process loopback IAudioClient for process_id (and its children). */
static HRESULT activate_process_loopback(DWORD process_id, IAudioClient **client) {
    AUDIOCLIENT_ACTIVATION_PARAMS params = {};
    params.ActivationType = AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK;
    params.ProcessLoopbackParams.TargetProcessId = process_id;
    params.ProcessLoopbackParams.ProcessLoopbackMode = PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE;
    PROPVARIANT activate_params = {};
    activate_params.vt = VT_BLOB;
    activate_params.blob.cbSize = sizeof(params);
    activate_params.blob.pBlobData = reinterpret_cast<BYTE *>(&params);

    *client = nullptr;
    auto *handler = new ActivateHandler();
    IActivateAudioInterfaceAsyncOperation *op = nullptr;
    HRESULT hr = ActivateAudioInterfaceAsync(VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK, __uuidof(IAudioClient),
                                             &activate_params, handler, &op);
    if (SUCCEEDED(hr) && WaitForSingleObject(handler->done_, 5000) != WAIT_OBJECT_0) hr = E_ABORT;
    if (SUCCEEDED(hr)) hr = handler->result_;
    if (SUCCEEDED(hr)) *client = handler->client_;
    else if (handler->client_) handler->client_->Release();
    if (op) op->Release();
    handler->Release();
    return hr;
}

bool WasapiCapture::init_process(DWORD process_id, uint32_t sample_rate) {
    HRESULT hr = CoInitializeEx(nullptr, COINIT_MULTITHREADED);
    if (FAILED(hr) && hr != RPC_E_CHANGED_MODE && hr != S_FALSE) {
        fprintf(stderr, "WasapiCapture: CoInitializeEx failed: 0x%08lx\n", hr);
        return false;
    }

    /* A process loopback client has no mix format (GetMixFormat fails):
     * ask for float stereo and let Windows convert to it. */
    if (sample_rate == 0) sample_rate = 48000;
    WAVEFORMATEX fmt = {};
    fmt.wFormatTag = WAVE_FORMAT_IEEE_FLOAT;
    fmt.nChannels = 2;
    fmt.nSamplesPerSec = sample_rate;
    fmt.wBitsPerSample = 32;
    fmt.nBlockAlign = fmt.nChannels * fmt.wBitsPerSample / 8;
    fmt.nAvgBytesPerSec = fmt.nSamplesPerSec * fmt.nBlockAlign;

#ifndef AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
#define AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM 0x80000000
#endif
    /* Microsoft documents process loopback from build 20348 only. If an
     * earlier build rejects AUTOCONVERTPCM, retry without it (as OBS does)
     * on a freshly activated client, since a failed Initialize can leave
     * the old one unusable. */
    const DWORD base_flags = AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_EVENTCALLBACK;
    const DWORD attempts[] = {base_flags | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM, base_flags};
    REFERENCE_TIME buf_duration = static_cast<REFERENCE_TIME>(BUFFER_DURATION_MS) * 10000;
    for (DWORD flags : attempts) {
        hr = activate_process_loopback(process_id, &audio_client_);
        if (FAILED(hr)) {
            fprintf(stderr, "WasapiCapture: Process loopback activation failed for PID %lu: 0x%08lx\n",
                    process_id, hr);
            return false;
        }
        hr = audio_client_->Initialize(AUDCLNT_SHAREMODE_SHARED, flags, buf_duration, 0, &fmt, nullptr);
        if (SUCCEEDED(hr)) break;
        fprintf(stderr, "WasapiCapture: Process loopback Initialize (flags 0x%08lx) failed: 0x%08lx\n",
                flags, hr);
        audio_client_->Release();
        audio_client_ = nullptr;
    }
    if (FAILED(hr)) return false;
    hr = audio_client_->SetEventHandle(buffer_event_);
    if (FAILED(hr)) {
        fprintf(stderr, "WasapiCapture: SetEventHandle failed: 0x%08lx\n", hr);
        return false;
    }
    hr = audio_client_->GetService(IID_IAudioCaptureClient_, reinterpret_cast<void **>(&capture_client_));
    if (FAILED(hr)) {
        fprintf(stderr, "WasapiCapture: Failed to get capture client: 0x%08lx\n", hr);
        return false;
    }

    sample_rate_ = sample_rate;
    channels_ = 2;
    bits_per_sample_ = 32;
    fprintf(stderr, "WasapiCapture: Capturing process %lu (and its child processes), "
            "%u Hz, 2 ch, 32-bit float\n", process_id, sample_rate_);
    return true;
}

bool WasapiCapture::start(AudioCallback callback) {
    if (running_.load()) {
        fprintf(stderr, "WasapiCapture: Already running\n");
        return false;
    }
    if (!audio_client_ || !capture_client_) {
        fprintf(stderr, "WasapiCapture: Not initialized\n");
        return false;
    }

    callback_ = callback;
    ResetEvent(stop_event_);
    running_.store(true);

    /* Start the audio client */
    HRESULT hr = audio_client_->Start();
    if (FAILED(hr)) {
        fprintf(stderr, "WasapiCapture: Failed to start capture: 0x%08lx\n", hr);
        running_.store(false);
        return false;
    }

    /* Create capture thread */
    thread_handle_ = CreateThread(
        nullptr, 0, capture_thread_proc, this, 0, nullptr
    );
    if (!thread_handle_) {
        fprintf(stderr, "WasapiCapture: Failed to create capture thread\n");
        audio_client_->Stop();
        running_.store(false);
        return false;
    }

    fprintf(stderr, "WasapiCapture: Capture started\n");
    return true;
}

void WasapiCapture::stop() {
    if (!running_.load()) return;

    running_.store(false);
    SetEvent(stop_event_);

    if (thread_handle_) {
        WaitForSingleObject(thread_handle_, 5000);
        CloseHandle(thread_handle_);
        thread_handle_ = nullptr;
    }

    if (audio_client_) {
        audio_client_->Stop();
    }

    fprintf(stderr, "WasapiCapture: Capture stopped\n");
}

DWORD WINAPI WasapiCapture::capture_thread_proc(LPVOID param) {
    /* Initialize COM for this thread */
    CoInitializeEx(nullptr, COINIT_MULTITHREADED);

    auto *self = static_cast<WasapiCapture *>(param);
    self->capture_loop();

    CoUninitialize();
    return 0;
}

void WasapiCapture::capture_loop() {
    /* Set this thread to multimedia priority for low-latency audio */
    DWORD task_index = 0;
    HANDLE avrt_handle = AvSetMmThreadCharacteristicsW(L"Audio", &task_index);
    if (!avrt_handle) {
        fprintf(stderr, "WasapiCapture: MMCSS registration failed, using timeBeginPeriod\n");
    }
    /* Ensure 1ms timer resolution as safety net */
    timeBeginPeriod(1);

    /* Wait on buffer_event (event-driven) + stop_event, or poll as fallback */
    HANDLE wait_handles[2] = { stop_event_, buffer_event_ };
    int handle_count = buffer_event_ ? 2 : 1;

    /* Discontinuities of this capture (start one excluded) */
    const DWORD DISCONTINUITY_START_MS = 500;
    const DWORD loop_start = GetTickCount();
    uint32_t disc_count = 0;
    DWORD disc_tick = 0;

    while (running_.load()) {
        DWORD wait_result;
        if (handle_count == 2) {
            /* Event-driven: WASAPI signals buffer_event_ when data is ready */
            wait_result = WaitForMultipleObjects(2, wait_handles, FALSE, 100);
        } else {
            /* Polling fallback */
            wait_result = WaitForSingleObject(stop_event_, 5);
        }
        if (wait_result == WAIT_OBJECT_0) {
            break; /* stop_event_ signaled */
        }

        /* Get available captured data */
        UINT32 packet_length = 0;
        HRESULT hr = capture_client_->GetNextPacketSize(&packet_length);
        if (FAILED(hr)) {
            fprintf(stderr, "WasapiCapture: GetNextPacketSize failed: 0x%08lx\n", hr);
            break;
        }

        while (packet_length > 0) {
            BYTE *data = nullptr;
            UINT32 num_frames = 0;
            DWORD flags = 0;
            UINT64 device_position = 0;
            UINT64 qpc_position = 0;

            hr = capture_client_->GetBuffer(
                &data, &num_frames, &flags,
                &device_position, &qpc_position
            );
            if (FAILED(hr)) {
                fprintf(stderr, "WasapiCapture: GetBuffer failed: 0x%08lx\n", hr);
                break;
            }

            if (flags & AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY) {
                DWORD now = GetTickCount();
                if (now - loop_start < DISCONTINUITY_START_MS) {
                    /* Windows usually flags the first buffers after Start():
                     * nothing was lost */
                    LOG_INFO("WasapiCapture: Data discontinuity at capture start (expected)");
                } else {
                    /* A gap in the captured audio (CPU load, other audio
                     * apps): counted per capture, reported every 2 s at most */
                    disc_count++;
                    if (disc_count == 1 || now - disc_tick >= 2000) {
                        LOG_WARN("WasapiCapture: Data discontinuity (count=%u)", disc_count);
                        disc_tick = now;
                    }
                }
            }

            if (num_frames > 0 && callback_) {
                if (flags & AUDCLNT_BUFFERFLAGS_SILENT) {
                    /* Buffer contains silence — use thread-local zero buffer
                     * to avoid heap allocation in the hot path. */
                    uint32_t bytes = num_frames * channels_ *
                                     (bits_per_sample_ / 8);
                    static thread_local std::vector<uint8_t> silence;
                    if (silence.size() < bytes) silence.resize(bytes, 0);
                    callback_(silence.data(), num_frames,
                              channels_, sample_rate_, bits_per_sample_);
                } else {
                    callback_(data, num_frames,
                              channels_, sample_rate_, bits_per_sample_);
                }
            }

            capture_client_->ReleaseBuffer(num_frames);

            hr = capture_client_->GetNextPacketSize(&packet_length);
            if (FAILED(hr)) break;
        }
    }

    timeEndPeriod(1);
    if (avrt_handle) {
        AvRevertMmThreadCharacteristics(avrt_handle);
    }
}
