/*
 * A2DP Windows Bridge (A2DPWB): Bluetooth Audio Streaming Application
 *
 * Supports multiple Bluetooth audio codecs:
 *   - LDAC (Sony, up to 990 kbps)
 *   - aptX HD (Qualcomm, 576 kbps, 24-bit)
 *   - aptX Low Latency (Qualcomm/CSR, 352 kbps, ~32ms latency)
 *   - AAC (MPEG-2/4 AAC-LC, up to 256 kbps)
 *   - SBC (mandatory A2DP codec, up to ~345 kbps)
 *
 * Main entry point. Orchestrates:
 *   1. Bluetooth device discovery and selection
 *   2. WASAPI loopback audio capture
 *   3. Audio encoding via selected codec
 *   4. AVDTP signaling and media streaming via BTstack + WinUSB
 *
 * SPDX-License-Identifier: MIT
 */

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <atomic>
#include <memory>
#include <mutex>
#include <vector>
#include <windows.h>

#include "audio_encoder.h"
#include "audio_device_enum.h"
#include "capture_mode.h"
#include "wasapi_capture.h"
#include "ldac_encoder.h"
#include "aptxhd_encoder.h"
#include "a2dp_sbc_encoder.h"
#include "aac_encoder.h"
#include "aptxll_encoder.h"
#include "bt_device.h"
#include "btstack_transport.h"
#include "config_path.h"
#include "wx_app.h"

/* Global state */
static std::atomic<bool> g_running{true};
static std::mutex g_encode_mutex;

/* Audio buffer for float32->PCM conversion (sized per encoder sample width) */
static std::vector<uint8_t> g_pcm_buffer;
static std::vector<uint8_t> g_encode_buffer;

/* Residual PCM buffer: leftover samples from previous WASAPI callback
 * that didn't fill a complete encoder frame (e.g. 480 samples / 128 per frame
 * = 3 frames + 96 leftover). Without this, 20% of audio data is lost. */
static std::vector<uint8_t> g_pcm_residual;

/* Streaming components */
static std::atomic<AudioEncoder *> g_encoder{nullptr};
static AudioCodec g_active_codec = AudioCodec::LDAC;
static uint32_t g_timestamp = 0;
static uint32_t g_active_channels = 2;
static bool g_abr_enabled = false;
static uint32_t g_encoder_sample_bytes = 2; /* 2 for int16, 4 for int32 (24/32-bit) */

/* BTstack transport */
static std::atomic<BtStackTransport *> g_transport{nullptr};

/* Convert float32 PCM (WASAPI default) to int16 PCM */
static void convert_float32_to_int16(const float *src, int16_t *dst, uint32_t samples) {
    for (uint32_t i = 0; i < samples; i++) {
        float s = src[i];
        if (s > 1.0f) s = 1.0f;
        if (s < -1.0f) s = -1.0f;
        dst[i] = static_cast<int16_t>(s * 32767.0f);
    }
}

/* Convert float32 PCM (WASAPI default) to int32 PCM (24/32-bit encoder input).
 * Uses double arithmetic to avoid float32 precision overflow at INT32_MAX. */
static void convert_float32_to_int32(const float *src, int32_t *dst, uint32_t samples) {
    for (uint32_t i = 0; i < samples; i++) {
        float s = src[i];
        if (s > 1.0f) s = 1.0f;
        if (s < -1.0f) s = -1.0f;
        dst[i] = static_cast<int32_t>(static_cast<double>(s) * 2147483647.0);
    }
}

/* Audio callback: receives PCM data, encodes, sends via transport */
static void audio_callback(
    const uint8_t *data, uint32_t frames,
    uint32_t channels, uint32_t sample_rate, uint32_t bits_per_sample)
{

    if (!g_running.load()) return;

    std::lock_guard<std::mutex> lock(g_encode_mutex);

    AudioEncoder *encoder = g_encoder.load();
    if (!encoder) return;

    uint32_t use_channels = g_active_channels;
    uint32_t sb = g_encoder_sample_bytes;
    bool need_downmix = (channels > use_channels);
    uint32_t out_channels = need_downmix ? use_channels : channels;

    /* Convert WASAPI output to encoder's expected integer format.
     * For multichannel, downmix to stereo in float domain BEFORE integer
     * conversion using ITU-R BS.775 coefficients so that center, LFE,
     * and surround channels are mixed in rather than discarded. */
    const uint8_t *pcm_data;

    if (bits_per_sample == 32) {
        /* WASAPI float32 → downmix (if needed) → encoder integer format */
        const float *float_src = reinterpret_cast<const float *>(data);

        static std::vector<float> float_stereo;
        if (need_downmix && use_channels == 2) {
            float_stereo.resize(frames * 2);
            for (uint32_t f = 0; f < frames; f++) {
                const float *ch = float_src + f * channels;
                float fl = ch[0], fr = ch[1];
                float fc  = (channels > 2) ? ch[2] : 0.0f;
                float lfe = (channels > 3) ? ch[3] : 0.0f;
                float bl  = (channels > 4) ? ch[4] : 0.0f;
                float br  = (channels > 5) ? ch[5] : 0.0f;
                float sl  = (channels > 6) ? ch[6] : 0.0f;
                float sr  = (channels > 7) ? ch[7] : 0.0f;
                /* ITU-R BS.775 stereo downmix + LFE at -6dB */
                float_stereo[f * 2 + 0] = fl + 0.707107f * fc
                    + 0.707107f * (bl + sl) + 0.5f * lfe;
                float_stereo[f * 2 + 1] = fr + 0.707107f * fc
                    + 0.707107f * (br + sr) + 0.5f * lfe;
            }
            float_src = float_stereo.data();
        } else if (need_downmix) {
            float_stereo.resize(frames * use_channels);
            for (uint32_t f = 0; f < frames; f++)
                for (uint32_t c = 0; c < use_channels; c++)
                    float_stereo[f * use_channels + c] = float_src[f * channels + c];
            float_src = float_stereo.data();
        }

        uint32_t out_samples = frames * out_channels;
        uint32_t buf_bytes = out_samples * sb;
        if (g_pcm_buffer.size() < buf_bytes)
            g_pcm_buffer.resize(buf_bytes);
        if (sb == 4) {
            convert_float32_to_int32(
                float_src,
                reinterpret_cast<int32_t *>(g_pcm_buffer.data()),
                out_samples);
        } else {
            convert_float32_to_int16(
                float_src,
                reinterpret_cast<int16_t *>(g_pcm_buffer.data()),
                out_samples);
        }
        pcm_data = g_pcm_buffer.data();
    } else if (bits_per_sample == 16) {
        uint32_t total_samples = frames * channels;
        if (need_downmix && use_channels == 2) {
            /* Downmix int16 multichannel to stereo (+ optional int16→int32 upscale) */
            uint32_t buf_bytes = frames * 2 * sb;
            if (g_pcm_buffer.size() < buf_bytes)
                g_pcm_buffer.resize(buf_bytes);
            const int16_t *src = reinterpret_cast<const int16_t *>(data);
            if (sb == 4) {
                int32_t *dst = reinterpret_cast<int32_t *>(g_pcm_buffer.data());
                for (uint32_t f = 0; f < frames; f++) {
                    const int16_t *ch = src + f * channels;
                    int64_t fl = static_cast<int32_t>(ch[0]) << 16;
                    int64_t fr = static_cast<int32_t>(ch[1]) << 16;
                    int64_t fc  = (channels > 2) ? (static_cast<int32_t>(ch[2]) << 16) : 0;
                    int64_t lfe = (channels > 3) ? (static_cast<int32_t>(ch[3]) << 16) : 0;
                    int64_t bl  = (channels > 4) ? (static_cast<int32_t>(ch[4]) << 16) : 0;
                    int64_t br  = (channels > 5) ? (static_cast<int32_t>(ch[5]) << 16) : 0;
                    int64_t sl  = (channels > 6) ? (static_cast<int32_t>(ch[6]) << 16) : 0;
                    int64_t sr  = (channels > 7) ? (static_cast<int32_t>(ch[7]) << 16) : 0;
                    int64_t l = fl + fc*707/1000 + (bl+sl)*707/1000 + lfe*500/1000;
                    int64_t r = fr + fc*707/1000 + (br+sr)*707/1000 + lfe*500/1000;
                    if (l > INT32_MAX) l = INT32_MAX; if (l < INT32_MIN) l = INT32_MIN;
                    if (r > INT32_MAX) r = INT32_MAX; if (r < INT32_MIN) r = INT32_MIN;
                    dst[f * 2 + 0] = static_cast<int32_t>(l);
                    dst[f * 2 + 1] = static_cast<int32_t>(r);
                }
            } else {
                int16_t *dst = reinterpret_cast<int16_t *>(g_pcm_buffer.data());
                for (uint32_t f = 0; f < frames; f++) {
                    const int16_t *ch = src + f * channels;
                    int32_t fl = ch[0], fr = ch[1];
                    int32_t fc  = (channels > 2) ? ch[2] : 0;
                    int32_t lfe = (channels > 3) ? ch[3] : 0;
                    int32_t bl  = (channels > 4) ? ch[4] : 0;
                    int32_t br  = (channels > 5) ? ch[5] : 0;
                    int32_t sl  = (channels > 6) ? ch[6] : 0;
                    int32_t sr  = (channels > 7) ? ch[7] : 0;
                    int32_t l = fl + fc*707/1000 + (bl+sl)*707/1000 + lfe*500/1000;
                    int32_t r = fr + fc*707/1000 + (br+sr)*707/1000 + lfe*500/1000;
                    if (l > 32767) l = 32767; if (l < -32768) l = -32768;
                    if (r > 32767) r = 32767; if (r < -32768) r = -32768;
                    dst[f * 2 + 0] = static_cast<int16_t>(l);
                    dst[f * 2 + 1] = static_cast<int16_t>(r);
                }
            }
            pcm_data = g_pcm_buffer.data();
        } else if (sb == 4) {
            /* Upscale int16 → int32 for high-bit-depth encoder */
            uint32_t buf_bytes = total_samples * 4;
            if (g_pcm_buffer.size() < buf_bytes)
                g_pcm_buffer.resize(buf_bytes);
            const int16_t *src16 = reinterpret_cast<const int16_t *>(data);
            int32_t *dst32 = reinterpret_cast<int32_t *>(g_pcm_buffer.data());
            for (uint32_t i = 0; i < total_samples; i++)
                dst32[i] = static_cast<int32_t>(src16[i]) << 16;
            pcm_data = g_pcm_buffer.data();
        } else {
            pcm_data = data;
        }
    } else {
        return;
    }

    uint32_t new_pcm_bytes = frames * use_channels * sb;
    uint32_t pcm_frames_per_encode = encoder->get_pcm_frames_per_encode();
    uint32_t bytes_per_encode = pcm_frames_per_encode * use_channels * sb;

    if (g_encode_buffer.size() < 2048) {
        g_encode_buffer.resize(2048);
    }

    /* Combine residual from previous callback with new data.
     * Without this, leftover samples (e.g. 480 % 128 = 96 samples at 48kHz)
     * are lost each callback, causing ~20% audio data loss → slow playback. */
    static std::vector<uint8_t> combined_pcm;
    uint32_t residual_bytes = static_cast<uint32_t>(g_pcm_residual.size());
    uint32_t pcm_bytes = residual_bytes + new_pcm_bytes;
    combined_pcm.resize(pcm_bytes);
    if (residual_bytes > 0) {
        memcpy(combined_pcm.data(), g_pcm_residual.data(), residual_bytes);
    }
    memcpy(combined_pcm.data() + residual_bytes, pcm_data, new_pcm_bytes);
    pcm_data = combined_pcm.data();

    uint32_t offset = 0;

    /*
     * Accumulate encoded frames and send as MTU-sized RTP packets.
     * Multiple frames per packet reduces overhead and ensures all
     * encoded data is sent (not overwritten).
     */
    BtStackTransport *transport = g_transport.load();
    if (!transport) return;

    uint16_t mtu = transport->get_media_mtu();
    if (mtu == 0) mtu = 679;
    /* Reserve 1 byte for LDAC/SBC media payload header (added by send_media) */
    uint32_t max_raw = (g_active_codec == AudioCodec::LDAC ||
                        g_active_codec == AudioCodec::SBC) ? (mtu - 1) : mtu;
    if (g_active_codec == AudioCodec::AptxLL) {
        /* aptX LL has no RTP header: the full L2CAP MTU is payload */
        max_raw = static_cast<uint32_t>(mtu) + BtStackTransport::RTP_HEADER_SIZE;
        if (max_raw > BtStackTransport::MAX_MEDIA_PACKET_SIZE)
            max_raw = BtStackTransport::MAX_MEDIA_PACKET_SIZE;
        if (sample_rate > 0) {
            /* Low latency: keep packets <= ~7.5 ms like PipeWire */
            uint32_t ll_max = sample_rate * 75u / 10000u;
            if (ll_max >= 4 && max_raw > ll_max) max_raw = ll_max;
        }
        max_raw &= ~3u;
    }

    static thread_local uint8_t accum[2048];
    uint32_t accum_size = 0;
    uint32_t accum_frames = 0;
    uint32_t first_ts = g_timestamp;

    while (offset + bytes_per_encode <= pcm_bytes) {
        uint32_t out_size = static_cast<uint32_t>(g_encode_buffer.size());
        uint32_t out_frames = 0;

        bool ok = encoder->encode(
            pcm_data + offset, bytes_per_encode,
            g_encode_buffer.data(), &out_size, &out_frames
        );

        if (ok && out_size > 0) {
            /* Flush if adding this frame would exceed MTU */
            if (accum_size + out_size > max_raw && accum_frames > 0) {
                transport->send_media(
                    accum, accum_size, first_ts,
                    static_cast<uint8_t>(accum_frames), g_active_codec);
                accum_size = 0;
                accum_frames = 0;
                first_ts = g_timestamp;
            }

            if (accum_size + out_size <= sizeof(accum)) {
                memcpy(accum + accum_size, g_encode_buffer.data(), out_size);
                accum_size += out_size;
                accum_frames += out_frames;
            }
        }

        g_timestamp += pcm_frames_per_encode;
        offset += bytes_per_encode;
    }

    /* Flush remaining accumulated frames */
    if (accum_frames > 0) {
        transport->send_media(
            accum, accum_size, first_ts,
            static_cast<uint8_t>(accum_frames), g_active_codec);
    }

    /* Save leftover PCM samples for next callback */
    uint32_t remaining = pcm_bytes - offset;
    if (remaining > 0 && remaining < bytes_per_encode) {
        g_pcm_residual.resize(remaining);
        memcpy(g_pcm_residual.data(), pcm_data + offset, remaining);
    } else {
        g_pcm_residual.clear();
    }

    /* ABR: periodically adjust LDAC quality based on queue depth.
     * Rate-limited to 100ms to match ldac_ABR_Init() interval. */
    if (g_abr_enabled && encoder->codec_type() == AudioCodec::LDAC) {
        LdacEncoder *ldac = static_cast<LdacEncoder *>(encoder);
        if (ldac->is_abr_enabled()) {
            static uint32_t abr_last_tick = 0;
            uint32_t now = GetTickCount();
            if (now - abr_last_tick >= 100) {
                abr_last_tick = now;
                uint32_t queue_depth = 0;
                BtStackTransport *abr_transport = g_transport.load();
                if (abr_transport) queue_depth = abr_transport->get_queue_depth();
                ldac->abr_adjust(queue_depth);
            }
        }
    }
}

/* Console Ctrl+C handler */
static BOOL WINAPI console_handler(DWORD ctrl_type) {
    if (ctrl_type == CTRL_C_EVENT || ctrl_type == CTRL_BREAK_EVENT) {
        printf("\nStopping...\n");
        g_running.store(false);
        return TRUE;
    }
    return FALSE;
}

static void print_usage(const char *prog) {
    printf("A2DP Windows Bridge (A2DPWB)\n\n");
    printf("Usage: %s [options]\n", prog);
    printf("\nModes:\n");
    printf("  (default)    Launch GUI application\n");
    printf("  --cli        Run in command-line mode\n");
    printf("\nOptions (CLI mode):\n");
    printf("  -c <codec>   Codec: ldac, aptxhd, aptxll, sbc, aac, auto (default: auto)\n");
    printf("  -q <mode>    Quality mode: hq (990kbps), sq (660kbps), mq (330kbps)\n");
    printf("               Only affects LDAC. Default: hq\n");
    printf("  -d <addr>    Bluetooth device address (XX:XX:XX:XX:XX:XX)\n");
    printf("               If not specified, scans for compatible devices\n");
    printf("  -a           Enable LDAC ABR (Adaptive Bit Rate)\n");
    printf("  -m <mode>    Capture mode: loopback, virtual (default: loopback)\n");
    printf("  --audio-device <id>  Audio device ID for virtual mode\n");
    printf("  -l           List available Bluetooth audio devices and exit\n");
    printf("  -u <path>    USB device path for BTstack (optional)\n");
    printf("  -h           Show this help\n");
    printf("\nCodec priority (auto mode): LDAC > aptX HD > aptX LL > AAC > SBC\n");
}

static EncoderQuality parse_quality(const char *mode) {
    if (_stricmp(mode, "hq") == 0) return EncoderQuality::High;
    if (_stricmp(mode, "sq") == 0) return EncoderQuality::Standard;
    if (_stricmp(mode, "mq") == 0) return EncoderQuality::Mobile;
    fprintf(stderr, "Unknown quality mode '%s', using HQ\n", mode);
    return EncoderQuality::High;
}

static const char *codec_name_str(AudioCodec codec) {
    switch (codec) {
    case AudioCodec::LDAC:   return "LDAC";
    case AudioCodec::AptxHD: return "aptX HD";
    case AudioCodec::AptxLL: return "aptX Low Latency";
    case AudioCodec::SBC:    return "SBC";
    case AudioCodec::AAC:    return "AAC";
    }
    return "Unknown";
}

/*
 * Select best codec from BTstack remote capabilities.
 * Returns true if a compatible codec was found.
 */
static bool find_best_btstack_codec(const BtStackTransport::RemoteCodecCaps &caps,
                                     AudioCodec requested_codec,
                                     bool auto_mode,
                                     AudioCodec *selected_codec) {
    if (!auto_mode) {
        switch (requested_codec) {
        case AudioCodec::LDAC:   if (caps.ldac)    { *selected_codec = AudioCodec::LDAC;   return true; } break;
        case AudioCodec::AptxHD: if (caps.aptx_hd) { *selected_codec = AudioCodec::AptxHD; return true; } break;
        case AudioCodec::AptxLL: if (caps.aptx_ll) { *selected_codec = AudioCodec::AptxLL; return true; } break;
        case AudioCodec::SBC:    if (caps.sbc)     { *selected_codec = AudioCodec::SBC;    return true; } break;
        case AudioCodec::AAC:    if (caps.aac)     { *selected_codec = AudioCodec::AAC;    return true; } break;
        }
        printf("Requested codec %s not available, falling back...\n",
               codec_name_str(requested_codec));
    }

    /* Priority: LDAC > aptX HD > aptX LL > AAC > SBC */
    if (caps.ldac)    { *selected_codec = AudioCodec::LDAC;   return true; }
    if (caps.aptx_hd) { *selected_codec = AudioCodec::AptxHD; return true; }
    if (caps.aptx_ll) { *selected_codec = AudioCodec::AptxLL; return true; }
    if (caps.aac)     { *selected_codec = AudioCodec::AAC;    return true; }
    if (caps.sbc)     { *selected_codec = AudioCodec::SBC;    return true; }

    return false;
}

/* ======================================================================== */
/* Main streaming flow                                                      */
/* ======================================================================== */

static int run_streaming(const uint8_t target_addr[6],
                             const char *usb_path,
                             AudioCodec requested_codec, bool auto_codec,
                             EncoderQuality quality, bool enable_abr,
                             CaptureMode capture_mode = CaptureMode::SystemLoopback,
                             const wchar_t *audio_device_id = nullptr) {
    BtStackTransport transport;
    transport.set_link_key_dir(get_config_dir());

    /* --- Step 2: Initialize BTstack --- */
    printf("\n[2/5] Initializing BTstack (WinUSB transport)...\n");
    if (!transport.init(usb_path)) {
        fprintf(stderr,
            "Failed to initialize BTstack.\n"
            "Ensure a USB Bluetooth adapter is connected and its driver\n"
            "has been replaced with WinUSB using Zadig.\n");
        return 1;
    }

    /* Optional scan: skip if connecting to a known device address */
    printf("\n[2.5/5] Radio ready. Skipping scan (connecting to specified device).\n");

    /* --- Step 3: Connect and discover codecs --- */
    printf("\n[3/5] Connecting and negotiating codec...\n");
    printf("(This may take up to 30 seconds if the device is slow to respond.)\n");
    if (!transport.connect_a2dp(target_addr)) {
        fprintf(stderr,
            "Failed to connect to Bluetooth device.\n"
            "Troubleshooting:\n"
            "  - Ensure the headphones are in PAIRING mode (not just powered on)\n"
            "    (Typically hold the power button until the LED blinks rapidly)\n"
            "  - Ensure no other device is currently connected to the headphones\n"
            "  - Ensure the headphones are within range of the USB adapter\n"
            "  - Try running again -- the first attempt after entering pairing mode may fail\n");
        transport.shutdown();
        return 1;
    }

    /* Select codec */
    AudioCodec selected_codec;
    if (!find_best_btstack_codec(transport.get_remote_caps(),
                                  requested_codec, auto_codec, &selected_codec)) {
        fprintf(stderr, "No compatible codec found on device.\n"
                "Device must support LDAC, aptX HD, or aptX Low Latency.\n");
        transport.disconnect();
        transport.shutdown();
        return 1;
    }

    g_active_codec = selected_codec;
    printf("Selected codec: %s\n", codec_name_str(selected_codec));

    /* --- Step 4: Initialize audio capture and encoder --- */
    printf("\n[4/5] Initializing audio capture and %s encoder...\n",
           codec_name_str(selected_codec));

    WasapiCapture wasapi_capture;
    std::wstring saved_default_device;

    switch (capture_mode) {
    case CaptureMode::SystemLoopback:
        printf("Capture mode: System Loopback\n");
        if (!wasapi_capture.init()) {
            fprintf(stderr, "Failed to initialize WASAPI capture\n");
            transport.disconnect();
            transport.shutdown();
            return 1;
        }
        break;

    case CaptureMode::VirtualDevice:
        printf("Capture mode: Virtual Device\n");
        if (!audio_device_id) {
            fprintf(stderr, "No audio device ID specified for virtual mode\n");
            transport.disconnect();
            transport.shutdown();
            return 1;
        }
        /* Save and switch default device */
        saved_default_device = AudioDeviceEnumerator::get_default_device_id();
        AudioDeviceEnumerator::set_default_device(audio_device_id);
        if (!wasapi_capture.init(0, audio_device_id)) {
            fprintf(stderr, "Failed to initialize capture on virtual device\n");
            if (!saved_default_device.empty())
                AudioDeviceEnumerator::set_default_device(saved_default_device);
            transport.disconnect();
            transport.shutdown();
            return 1;
        }
        break;
    }

    uint32_t sample_rate = wasapi_capture.get_sample_rate();
    uint32_t channels = wasapi_capture.get_channels();

    /* Warn if system sample rate is above 48kHz — LDAC frame size is fixed at
     * 128 samples, so higher rates halve the bits per frame and degrade quality.
     * 96kHz at 990kbps ≈ 495kbps effective at 48kHz (below SQ mode). */
    if (sample_rate > 48000) {
        printf("\n*** WARNING: System sample rate is %u Hz. ***\n"
               "*** LDAC quality degrades at high sample rates because the    ***\n"
               "*** per-frame bitrate is halved (990kbps@96kHz ≈ 495kbps@48kHz). ***\n"
               "*** For best quality, set Windows audio output to 48000 Hz.  ***\n\n",
               sample_rate);
    }

    if (channels > 2) {
        printf("System output has %u channels, downmixing to stereo\n", channels);
    }
    g_active_channels = (channels > 2) ? 2 : channels;

    if ((selected_codec == AudioCodec::AptxHD || selected_codec == AudioCodec::AptxLL)
        && g_active_channels < 2) {
        fprintf(stderr, "%s requires stereo output. Current system output is mono.\n",
                codec_name_str(selected_codec));
        transport.disconnect();
        transport.shutdown();
        return 1;
    }

    /* Configure stream */
    if (!transport.configure_codec(selected_codec, sample_rate,
                                    static_cast<uint8_t>(g_active_channels))) {
        fprintf(stderr, "Failed to configure %s stream\n", codec_name_str(selected_codec));
        transport.disconnect();
        transport.shutdown();
        return 1;
    }

    /* Create encoder */
    std::unique_ptr<AudioEncoder> encoder;
    uint16_t media_mtu = transport.get_media_mtu();
    if (media_mtu == 0) media_mtu = 679;

    switch (selected_codec) {
    case AudioCodec::LDAC: {
        auto ldac = std::make_unique<LdacEncoder>();
        ldac->set_bit_depth(32);
        encoder = std::move(ldac);
        break;
    }
    case AudioCodec::AptxHD: {
        /* aptX HD carries 24-bit PCM: feed it int32 like LDAC */
        auto hd = std::make_unique<AptxHdEncoder>();
        hd->set_bit_depth(32);
        encoder = std::move(hd);
        break;
    }
    case AudioCodec::AptxLL: encoder = std::make_unique<AptxLlEncoder>(); break;
    case AudioCodec::SBC:    encoder = std::make_unique<SbcEncoder>(); break;
#ifdef AAC_ENCODER_AVAILABLE
    case AudioCodec::AAC:    encoder = std::make_unique<AacEncoder>(); break;
#else
    case AudioCodec::AAC:
        fprintf(stderr, "AAC encoder not available (fdk-aac not built)\n");
        transport.disconnect();
        transport.shutdown();
        return 1;
#endif
    }

    if (!encoder->init(media_mtu, quality, sample_rate, g_active_channels)) {
        fprintf(stderr, "Failed to initialize %s encoder\n", codec_name_str(selected_codec));
        transport.disconnect();
        transport.shutdown();
        return 1;
    }

    /* Set sample width for audio callback based on encoder bit depth */
    g_encoder_sample_bytes = (selected_codec == AudioCodec::LDAC ||
                              selected_codec == AudioCodec::AptxHD) ? 4 : 2;

    if (enable_abr && selected_codec == AudioCodec::LDAC) {
        LdacEncoder *ldac = static_cast<LdacEncoder *>(encoder.get());
        if (ldac->init_abr(100)) {
            g_abr_enabled = true;
        }
    }

    /* Start streaming */
    if (!transport.start_stream()) {
        fprintf(stderr, "Failed to start stream\n");
        transport.disconnect();
        transport.shutdown();
        return 1;
    }

    /* --- Step 5: Stream --- */
    printf("\n[5/5] Streaming %s audio (BTstack/WinUSB)...\n", codec_name_str(selected_codec));
    printf("Codec: %s | Bitrate: %u kbps | Sample rate: %u Hz | Channels: %u%s\n",
           encoder->codec_name(), encoder->get_bitrate_kbps(),
           sample_rate, g_active_channels,
           g_abr_enabled ? " | ABR: on" : "");
    printf("Press Ctrl+C to stop.\n\n");

    g_encoder.store(encoder.get());
    g_transport.store(&transport);
    g_timestamp = 0;

    bool cli_capture_started = wasapi_capture.start(audio_callback);
    if (!cli_capture_started) {
        fprintf(stderr, "Failed to start audio capture\n");
        if (!saved_default_device.empty())
            AudioDeviceEnumerator::set_default_device(saved_default_device);
        transport.disconnect();
        transport.shutdown();
        return 1;
    }

    /* Main loop: keep streaming, auto-reconnect on disconnect */
    const uint32_t RECONNECT_DELAY_MS = 3000;
    const int MAX_RECONNECT_ATTEMPTS = 10;

    while (g_running.load()) {
        Sleep(200);

        /* Check if connection was lost */
        if (transport.check_disconnected()) {
            printf("\n*** Connection lost — attempting to reconnect ***\n");

            /* Pause audio capture during reconnect to avoid buffering stale data */
            g_transport.store(nullptr);
            g_encoder.store(nullptr);

            bool reconnected = false;
            for (int attempt = 1; attempt <= MAX_RECONNECT_ATTEMPTS && g_running.load(); attempt++) {
                printf("Reconnect attempt %d/%d (waiting %u ms)...\n",
                       attempt, MAX_RECONNECT_ATTEMPTS, RECONNECT_DELAY_MS);
                Sleep(RECONNECT_DELAY_MS);

                if (!g_running.load()) break;

                if (transport.reconnect()) {
                    /* Update MTU in case it changed */
                    uint16_t new_mtu = transport.get_media_mtu();
                    if (new_mtu != media_mtu) {
                        printf("Media MTU changed: %u -> %u\n", media_mtu, new_mtu);
                        media_mtu = new_mtu;
                    }
                    reconnected = true;
                    break;
                }
            }

            if (!reconnected) {
                fprintf(stderr, "Failed to reconnect after %d attempts. Exiting.\n",
                        MAX_RECONNECT_ATTEMPTS);
                break;
            }

            /* Resume audio pipeline */
            g_timestamp = 0;
            g_pcm_residual.clear();
            g_encoder.store(encoder.get());
            g_transport.store(&transport);
            printf("*** Reconnected — resuming LDAC streaming ***\n\n");
        }
    }

    /* Cleanup */
    printf("\nShutting down...\n");
    wasapi_capture.stop();
    g_encoder.store(nullptr);
    g_transport.store(nullptr);

    transport.stop_stream();
    transport.disconnect();
    transport.shutdown();

    /* Restore default device if we switched it */
    if (!saved_default_device.empty()) {
        AudioDeviceEnumerator::set_default_device(saved_default_device);
    }

    encoder->shutdown();
    printf("Done.\n");
    return 0;
}

/* ======================================================================== */
/* Main                                                                     */
/* ======================================================================== */

int main(int argc, char *argv[]) {
    /* Check for GUI mode (default) vs CLI mode */
    bool cli_mode = false;
    bool start_minimized = false;
    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "--cli") == 0) {
            cli_mode = true;
        } else if (strcmp(argv[i], "--minimized") == 0) {
            start_minimized = true;
        }
    }

    /* Initialize COM: GUI mode needs STA (single-threaded apartment) for
       wxWidgets/OLE message dispatching; CLI mode uses MTA for WASAPI.
       Worker threads initialize their own COM apartments independently. */
    HRESULT hr = CoInitializeEx(nullptr,
        cli_mode ? COINIT_MULTITHREADED : COINIT_APARTMENTTHREADED);
    if (FAILED(hr) && hr != RPC_E_CHANGED_MODE) {
        fprintf(stderr, "Failed to initialize COM: 0x%08lx\n", hr);
        return 1;
    }

    /* Single instance check (GUI mode only) */
    HANDLE single_instance_mutex = nullptr;
    if (!cli_mode) {
        single_instance_mutex = CreateMutexW(nullptr, TRUE, L"A2DPWB_SingleInstance");
        if (GetLastError() == ERROR_ALREADY_EXISTS) {
            /* Another instance is already running. Try to bring its window to front. */
            HWND existing = FindWindowW(nullptr, L"A2DPWB");
            if (existing) {
                if (!IsWindowVisible(existing))
                    ShowWindow(existing, SW_SHOW);
                if (IsIconic(existing))
                    ShowWindow(existing, SW_RESTORE);
                SetForegroundWindow(existing);
            }
            if (single_instance_mutex) CloseHandle(single_instance_mutex);
            CoUninitialize();
            return 0;
        }
    }

    if (!cli_mode) {
        /* GUI mode (wxWidgets) */
        FreeConsole();
        wxDISABLE_DEBUG_SUPPORT();
        int result = wxEntry(argc, argv);
        if (single_instance_mutex) CloseHandle(single_instance_mutex);
        CoUninitialize();
        return result;
    }

    /* CLI mode */
    printf("A2DP Windows Bridge (A2DPWB)\n");
    printf("Codecs: LDAC | aptX HD | aptX Low Latency | AAC | SBC\n");
    printf("===================================================\n\n");

    /* Parse command-line arguments */
    EncoderQuality quality = EncoderQuality::High;
    AudioCodec requested_codec = AudioCodec::LDAC;
    bool auto_codec = true;
    bool enable_abr = false;
    char device_addr_str[32] = {};
    bool list_only = false;
    const char *usb_path = nullptr;
    CaptureMode cli_capture_mode = CaptureMode::SystemLoopback;
    const char *cli_audio_device = nullptr;

    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "--cli") == 0) {
            continue;  /* Already handled */
        } else if (strcmp(argv[i], "-c") == 0 && i + 1 < argc) {
            i++;
            if (_stricmp(argv[i], "ldac") == 0) {
                requested_codec = AudioCodec::LDAC;
                auto_codec = false;
            } else if (_stricmp(argv[i], "aptxhd") == 0) {
                requested_codec = AudioCodec::AptxHD;
                auto_codec = false;
            } else if (_stricmp(argv[i], "aptxll") == 0) {
                requested_codec = AudioCodec::AptxLL;
                auto_codec = false;
            } else if (_stricmp(argv[i], "sbc") == 0) {
                requested_codec = AudioCodec::SBC;
                auto_codec = false;
            } else if (_stricmp(argv[i], "aac") == 0) {
                requested_codec = AudioCodec::AAC;
                auto_codec = false;
            } else if (_stricmp(argv[i], "auto") == 0) {
                auto_codec = true;
            } else {
                fprintf(stderr, "Unknown codec '%s'. Use: ldac, aptxhd, aptxll, sbc, aac, auto\n",
                        argv[i]);
                return 1;
            }
        } else if (strcmp(argv[i], "-q") == 0 && i + 1 < argc) {
            quality = parse_quality(argv[++i]);
        } else if (strcmp(argv[i], "-d") == 0 && i + 1 < argc) {
            strncpy(device_addr_str, argv[++i], sizeof(device_addr_str) - 1);
        } else if (strcmp(argv[i], "-a") == 0) {
            enable_abr = true;
        } else if (strcmp(argv[i], "-l") == 0) {
            list_only = true;
        } else if (strcmp(argv[i], "-u") == 0 && i + 1 < argc) {
            usb_path = argv[++i];
        } else if (strcmp(argv[i], "-m") == 0 && i + 1 < argc) {
            i++;
            if (_stricmp(argv[i], "loopback") == 0) {
                cli_capture_mode = CaptureMode::SystemLoopback;
            } else if (_stricmp(argv[i], "virtual") == 0) {
                cli_capture_mode = CaptureMode::VirtualDevice;
            } else {
                fprintf(stderr, "Unknown capture mode '%s'. Use: loopback, virtual\n",
                        argv[i]);
                return 1;
            }
        } else if (strcmp(argv[i], "--audio-device") == 0 && i + 1 < argc) {
            cli_audio_device = argv[++i];
        } else if (strcmp(argv[i], "-h") == 0) {
            print_usage(argv[0]);
            return 0;
        }
    }

    printf("Transport: BTstack (WinUSB)\n");

    SetConsoleCtrlHandler(console_handler, TRUE);

    /* --- Step 1: Discover/select Bluetooth device --- */
    printf("[1/5] Scanning for Bluetooth audio devices...\n");

    /*
     * Windows BT APIs may not work if the adapter is claimed by WinUSB.
     * Try auto-discovery, but device address can be specified via -d.
     */
    if (list_only) {
        auto bt_devices = BtDeviceDiscovery::scan_paired_devices();
        printf("\nDone.\n");
        CoUninitialize();
        return 0;
    }

    if (device_addr_str[0] == '\0') {
        /* Try Windows discovery (works if a separate adapter is available) */
        auto bt_devices = BtDeviceDiscovery::scan_paired_devices();
        for (const auto &dev : bt_devices) {
            if (dev.a2dp_sink) {
                auto addr_str = BtDeviceDiscovery::format_address(dev.address);
                strncpy(device_addr_str, addr_str.c_str(), sizeof(device_addr_str) - 1);
                printf("Auto-selected device: %s (%s)\n", dev.name.c_str(), device_addr_str);
                break;
            }
        }
        if (device_addr_str[0] == '\0') {
            fprintf(stderr,
                "No Bluetooth audio device found.\n"
                "Use -d XX:XX:XX:XX:XX:XX to specify the device address.\n"
                "(Find the address in Windows Settings > Bluetooth before switching to WinUSB)\n");
            CoUninitialize();
            return 1;
        }
    }

    uint8_t target_addr[6] = {};
    if (device_addr_str[0] != '\0') {
        if (!BtDeviceDiscovery::parse_address(device_addr_str, target_addr)) {
            fprintf(stderr, "Invalid Bluetooth address: %s\n", device_addr_str);
            CoUninitialize();
            return 1;
        }
        printf("Target device: %s\n",
               BtDeviceDiscovery::format_address(target_addr).c_str());
    } else {
        fprintf(stderr, "No Bluetooth audio device found. Use -d to specify an address.\n");
        CoUninitialize();
        return 1;
    }

    /* Convert audio device ID to wide string if specified */
    std::wstring cli_audio_device_w;
    if (cli_audio_device) {
        int wlen = MultiByteToWideChar(CP_UTF8, 0, cli_audio_device, -1, nullptr, 0);
        if (wlen > 0) {
            cli_audio_device_w.resize(wlen - 1);
            MultiByteToWideChar(CP_UTF8, 0, cli_audio_device, -1,
                                &cli_audio_device_w[0], wlen);
        }
    }

    int result = run_streaming(target_addr, usb_path,
                               requested_codec, auto_codec, quality, enable_abr,
                               cli_capture_mode,
                               cli_audio_device_w.empty() ? nullptr : cli_audio_device_w.c_str());

    CoUninitialize();
    return result;
}
