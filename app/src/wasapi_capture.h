/*
 * WASAPI Loopback Audio Capture
 *
 * Captures system audio output using Windows Audio Session API (WASAPI)
 * in loopback mode, providing PCM data for LDAC encoding.
 *
 * SPDX-License-Identifier: MIT
 */

#ifndef WASAPI_CAPTURE_H
#define WASAPI_CAPTURE_H

#include <cstdint>
#include <functional>
#include <atomic>
#include <windows.h>
#include <mmdeviceapi.h>
#include <audioclient.h>

/* Callback invoked when PCM audio data is available.
 * data: interleaved PCM samples (format depends on system output)
 * frames: number of audio frames
 * channels: number of channels
 * sample_rate: sample rate in Hz
 * bits_per_sample: bits per sample (16 or 32) */
using AudioCallback = std::function<void(
    const uint8_t *data, uint32_t frames,
    uint32_t channels, uint32_t sample_rate, uint32_t bits_per_sample)>;

class WasapiCapture {
public:
    WasapiCapture();
    ~WasapiCapture();

    /* Initialize WASAPI loopback capture.
     * preferred_sample_rate: 0 = use system default, otherwise try 44100/48000
     * device_id: nullptr = use default output device, otherwise capture from
     *            the specified device (WASAPI endpoint ID)
     * buffer_ms: capture buffer duration in milliseconds (1-40, default 10) */
    static const uint32_t BUFFER_DURATION_MS = 10;

    bool init(uint32_t preferred_sample_rate = 0, const wchar_t *device_id = nullptr);

    /* Start capturing audio. Callback is invoked from capture thread. */
    bool start(AudioCallback callback);

    /* Stop capturing */
    void stop();

    /* Get capture format info */
    uint32_t get_sample_rate() const { return sample_rate_; }
    uint32_t get_channels() const { return channels_; }
    uint32_t get_bits_per_sample() const { return bits_per_sample_; }

private:
    static DWORD WINAPI capture_thread_proc(LPVOID param);
    void capture_loop();

    uint32_t sample_rate_ = 0;
    uint32_t channels_ = 0;
    uint32_t bits_per_sample_ = 0;
    std::atomic<bool> running_{false};

    /* COM interfaces */
    IMMDeviceEnumerator *enumerator_ = nullptr;
    IMMDevice *device_ = nullptr;
    IAudioClient *audio_client_ = nullptr;
    IAudioCaptureClient *capture_client_ = nullptr;

    /* Capture thread */
    HANDLE thread_handle_ = nullptr;
    HANDLE stop_event_ = nullptr;
    HANDLE buffer_event_ = nullptr;  /* signaled by WASAPI when data is ready */
    AudioCallback callback_;

};

#endif /* WASAPI_CAPTURE_H */
