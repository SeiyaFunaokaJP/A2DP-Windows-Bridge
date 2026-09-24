/*
 * A2DP Service Implementation
 *
 * Extracted from gui_app.cpp — streaming, scanning, and BTstack logic.
 *
 * SPDX-License-Identifier: MIT
 */

#include "a2dp_service.h"
#include "audio_device_enum.h"
#include "bt_adapter_enum.h"
#include "btstack_transport.h"
#include "capture_mode.h"
#include "config_path.h"
#include "debug_log.h"
#include "wasapi_capture.h"
#include "ldac_encoder.h"
#include "aptxhd_encoder.h"
#include "aptxll_encoder.h"
#include "aptx_encoder.h"
#include "a2dp_sbc_encoder.h"
#include "aac_encoder.h"
#include "bt_device.h"
#include "localization.h"

#include <algorithm>
#include <cstdio>
#include <cstring>
#include <cstdlib>
#include <memory>
#include <vector>
#include <mutex>
#include <windows.h>
#include <avrt.h>

/* ======================================================================== */
/* SPSC ring buffer for WASAPI → encode thread                               */
/* ======================================================================== */

namespace {

struct PcmRingBuffer {
    std::vector<uint8_t> data;
    std::atomic<uint32_t> head{0};   /* write position (WASAPI callback) */
    std::atomic<uint32_t> tail{0};   /* read position  (encode thread)   */
    uint32_t capacity = 0;           /* power of two */
    HANDLE data_event = nullptr;     /* auto-reset event */

    uint32_t channels = 0;
    uint32_t sample_rate = 0;
    uint32_t bits_per_sample = 0;

    void init(uint32_t cap) {
        capacity = cap;
        data.resize(cap, 0);
        head.store(0, std::memory_order_relaxed);
        tail.store(0, std::memory_order_relaxed);
        data_event = CreateEventW(nullptr, FALSE, FALSE, nullptr);
    }

    void destroy() {
        if (data_event) { CloseHandle(data_event); data_event = nullptr; }
        data.clear();
        capacity = 0;
    }

    uint32_t available_write() const {
        uint32_t h = head.load(std::memory_order_relaxed);
        uint32_t t = tail.load(std::memory_order_acquire);
        return capacity - (h - t);
    }

    uint32_t available_read() const {
        uint32_t h = head.load(std::memory_order_acquire);
        uint32_t t = tail.load(std::memory_order_relaxed);
        return h - t;
    }

    void write(const uint8_t *src, uint32_t len) {
        uint32_t h = head.load(std::memory_order_relaxed);
        uint32_t mask = capacity - 1;
        uint32_t pos = h & mask;
        uint32_t first = (capacity - pos < len) ? capacity - pos : len;
        memcpy(data.data() + pos, src, first);
        if (first < len)
            memcpy(data.data(), src + first, len - first);
        head.store(h + len, std::memory_order_release);
        SetEvent(data_event);
    }

    void read(uint8_t *dst, uint32_t len) {
        uint32_t t = tail.load(std::memory_order_relaxed);
        uint32_t mask = capacity - 1;
        uint32_t pos = t & mask;
        uint32_t first = (capacity - pos < len) ? capacity - pos : len;
        memcpy(dst, data.data() + pos, first);
        if (first < len)
            memcpy(dst + first, data.data(), len - first);
        tail.store(t + len, std::memory_order_release);
    }
};

/* ======================================================================== */
/* Streaming context + encode thread                                         */
/* ======================================================================== */

struct StreamingContext {
    std::atomic<AudioEncoder *>     encoder{nullptr};
    std::atomic<BtStackTransport *> transport{nullptr};
    std::atomic<bool>               running{false};
    AudioCodec                      active_codec = AudioCodec::LDAC;
    uint32_t                        active_channels = 2;
    uint32_t                        timestamp = 0;
    bool                            abr_enabled = false;
    int                             bytes_per_sample = 2;
    int                             capture_mode = 0;

    std::vector<uint8_t>            pcm_buffer;
    std::vector<uint8_t>            encode_buffer;
    std::vector<uint8_t>            pcm_residual;
    PcmRingBuffer                   ring;
};

static StreamingContext g_ctx;

static void convert_f32_to_i16(const float *src, int16_t *dst, uint32_t n) {
    for (uint32_t i = 0; i < n; i++) {
        float s = src[i];
        if (s > 1.0f) s = 1.0f;
        if (s < -1.0f) s = -1.0f;
        dst[i] = static_cast<int16_t>(s * 32767.0f);
    }
}

static void convert_f32_to_i32(const float *src, int32_t *dst, uint32_t n) {
    for (uint32_t i = 0; i < n; i++) {
        float s = src[i];
        if (s > 1.0f) s = 1.0f;
        if (s < -1.0f) s = -1.0f;
        dst[i] = static_cast<int32_t>(static_cast<double>(s) * 2147483647.0);
    }
}

/* WASAPI callback — just copy raw PCM into the ring buffer.
 * All conversion, encoding, and sending happens in encode_thread_func. */
static std::atomic<uint32_t> ring_overflow_count{0};

static void service_audio_callback(
    const uint8_t *data, uint32_t frames,
    uint32_t channels, uint32_t sample_rate, uint32_t bits_per_sample)
{
    if (!g_ctx.running.load(std::memory_order_relaxed)) return;
    uint32_t byte_size = frames * channels * (bits_per_sample / 8);
    if (byte_size == 0) return;

    g_ctx.ring.channels = channels;
    g_ctx.ring.sample_rate = sample_rate;
    g_ctx.ring.bits_per_sample = bits_per_sample;

    if (g_ctx.ring.available_write() < byte_size) {
        ring_overflow_count.fetch_add(1, std::memory_order_relaxed);
        return;
    }
    g_ctx.ring.write(data, byte_size);
}

/* Encode thread — reads raw PCM from ring buffer, converts, encodes, sends.
 * Processes data in small chunks (~10ms) to avoid bursty packet delivery. */
static DWORD WINAPI encode_thread_func(LPVOID) {
    DWORD task_index = 0;
    HANDLE avrt_handle = AvSetMmThreadCharacteristicsW(L"Pro Audio", &task_index);

    std::vector<uint8_t> read_buf;
    std::vector<uint8_t> downmixed_buf;
    std::vector<uint8_t> combined;
    uint8_t accum[2048];
    uint32_t abr_last_tick = 0;
    uint32_t log_tick = 0;

    while (g_ctx.running.load(std::memory_order_relaxed)) {
        WaitForSingleObject(g_ctx.ring.data_event, 10);
        if (!g_ctx.running.load(std::memory_order_relaxed)) break;

        /* Inner drain loop — process available data in small chunks
         * to spread send_media calls over time instead of bursting. */
        for (;;) {
            if (!g_ctx.running.load(std::memory_order_relaxed)) break;

            AudioEncoder *encoder = g_ctx.encoder.load(std::memory_order_acquire);
            if (!encoder) break;

            uint32_t avail = g_ctx.ring.available_read();
            if (avail == 0) break;

            uint32_t channels = g_ctx.ring.channels;
            uint32_t bits_per_sample = g_ctx.ring.bits_per_sample;
            if (channels == 0 || bits_per_sample == 0) break;

            uint32_t frame_bytes = channels * (bits_per_sample / 8);
            uint32_t frames = avail / frame_bytes;
            if (frames == 0) break;

            /* Cap read size to ~10ms of audio to avoid large encode bursts */
            uint32_t sample_rate = g_ctx.ring.sample_rate;
            if (sample_rate > 0) {
                uint32_t max_frames = sample_rate / 100; /* ~10ms */
                if (max_frames < 128) max_frames = 128;
                if (frames > max_frames) frames = max_frames;
            }

            uint32_t read_bytes = frames * frame_bytes;
            read_buf.resize(read_bytes);
            g_ctx.ring.read(read_buf.data(), read_bytes);

            try {

            /* Convert float32 to integer PCM */
            uint32_t use_channels = g_ctx.active_channels;
            int bps = g_ctx.bytes_per_sample;
            uint32_t total_samples = frames * channels;

            const uint8_t *pcm_data;
            if (bits_per_sample == 32) {
                uint32_t buf_bytes = total_samples * static_cast<uint32_t>(bps);
                if (g_ctx.pcm_buffer.size() < buf_bytes)
                    g_ctx.pcm_buffer.resize(buf_bytes);
                if (bps == 4) {
                    convert_f32_to_i32(reinterpret_cast<const float *>(read_buf.data()),
                                       reinterpret_cast<int32_t *>(g_ctx.pcm_buffer.data()),
                                       total_samples);
                } else {
                    convert_f32_to_i16(reinterpret_cast<const float *>(read_buf.data()),
                                       reinterpret_cast<int16_t *>(g_ctx.pcm_buffer.data()),
                                       total_samples);
                }
                pcm_data = g_ctx.pcm_buffer.data();
            } else if (bits_per_sample == 16 && bps == 2) {
                pcm_data = read_buf.data();
            } else if (bits_per_sample == 16 && bps == 4) {
                /* int16 capture feeding a 32-bit (MSB-aligned) encoder */
                uint32_t buf_bytes = total_samples * 4u;
                if (g_ctx.pcm_buffer.size() < buf_bytes)
                    g_ctx.pcm_buffer.resize(buf_bytes);
                const int16_t *src = reinterpret_cast<const int16_t *>(read_buf.data());
                int32_t *dst = reinterpret_cast<int32_t *>(g_ctx.pcm_buffer.data());
                for (uint32_t i = 0; i < total_samples; i++)
                    dst[i] = static_cast<int32_t>(src[i]) * 65536;
                pcm_data = g_ctx.pcm_buffer.data();
            } else {
                break;
            }

            /* Downmix if needed */
            if (channels > use_channels) {
                uint32_t dm_bytes = frames * use_channels * static_cast<uint32_t>(bps);
                downmixed_buf.resize(dm_bytes);
                if (bps == 4) {
                    const int32_t *src = reinterpret_cast<const int32_t *>(pcm_data);
                    int32_t *dst = reinterpret_cast<int32_t *>(downmixed_buf.data());
                    for (uint32_t f = 0; f < frames; f++)
                        for (uint32_t c = 0; c < use_channels; c++)
                            dst[f * use_channels + c] = src[f * channels + c];
                } else {
                    const int16_t *src = reinterpret_cast<const int16_t *>(pcm_data);
                    int16_t *dst = reinterpret_cast<int16_t *>(downmixed_buf.data());
                    for (uint32_t f = 0; f < frames; f++)
                        for (uint32_t c = 0; c < use_channels; c++)
                            dst[f * use_channels + c] = src[f * channels + c];
                }
                pcm_data = downmixed_buf.data();
            }

            /* Combine with residual and encode */
            uint32_t new_pcm_bytes = frames * use_channels * static_cast<uint32_t>(bps);
            uint32_t pcm_frames_per_encode = encoder->get_pcm_frames_per_encode();
            uint32_t bytes_per_encode = pcm_frames_per_encode * use_channels * static_cast<uint32_t>(bps);

            if (g_ctx.encode_buffer.size() < 2048)
                g_ctx.encode_buffer.resize(2048);

            uint32_t residual_bytes = static_cast<uint32_t>(g_ctx.pcm_residual.size());
            uint32_t pcm_bytes = residual_bytes + new_pcm_bytes;
            combined.resize(pcm_bytes);
            if (residual_bytes > 0)
                memcpy(combined.data(), g_ctx.pcm_residual.data(), residual_bytes);
            memcpy(combined.data() + residual_bytes, pcm_data, new_pcm_bytes);
            pcm_data = combined.data();

            uint32_t offset = 0;

            BtStackTransport *transport = g_ctx.transport.load(std::memory_order_acquire);
            if (!transport) break;

            uint16_t mtu = transport->get_media_mtu();
            if (mtu == 0) mtu = 679;
            uint32_t max_raw = (g_ctx.active_codec == AudioCodec::LDAC ||
                                g_ctx.active_codec == AudioCodec::SBC) ? (mtu - 1) : mtu;
            if (g_ctx.active_codec == AudioCodec::Aptx ||
                g_ctx.active_codec == AudioCodec::AptxLL) {
                /* aptX and aptX LL are sent without the 12-byte RTP header, so
                 * the whole L2CAP MTU is payload (get_media_mtu() already has
                 * the RTP header subtracted). Cap to the transport's per-packet
                 * buffer and keep whole 4-byte aptX frames. */
                max_raw = (static_cast<uint32_t>(mtu) + BtStackTransport::RTP_HEADER_SIZE);
                if (max_raw > BtStackTransport::MAX_MEDIA_PACKET_SIZE)
                    max_raw = BtStackTransport::MAX_MEDIA_PACKET_SIZE;
                if (g_ctx.active_codec == AudioCodec::AptxLL && sample_rate > 0) {
                    /* Low latency: keep packets <= ~7.5 ms like PipeWire */
                    uint32_t ll_max = sample_rate * 75u / 10000u;  /* samples = bytes (4 B / 4 samples) */
                    if (ll_max >= 4 && max_raw > ll_max) max_raw = ll_max;
                }
                max_raw &= ~3u;
            }

            uint32_t accum_size = 0;
            uint32_t accum_frames = 0;
            uint32_t first_ts = g_ctx.timestamp;

            while (offset + bytes_per_encode <= pcm_bytes) {
                uint32_t out_size = static_cast<uint32_t>(g_ctx.encode_buffer.size());
                uint32_t out_frames = 0;

                bool ok = encoder->encode(pcm_data + offset, bytes_per_encode,
                                          g_ctx.encode_buffer.data(), &out_size, &out_frames);

                if (ok && out_size > 0) {
                    if (accum_size + out_size > max_raw && accum_frames > 0) {
                        transport->send_media(accum, accum_size, first_ts,
                            static_cast<uint8_t>(accum_frames), g_ctx.active_codec);
                        accum_size = 0;
                        accum_frames = 0;
                        first_ts = g_ctx.timestamp;
                    }
                    if (accum_size + out_size <= sizeof(accum)) {
                        memcpy(accum + accum_size, g_ctx.encode_buffer.data(), out_size);
                        accum_size += out_size;
                        accum_frames += out_frames;
                    }
                }

                g_ctx.timestamp += pcm_frames_per_encode;
                offset += bytes_per_encode;
            }

            if (accum_frames > 0) {
                transport->send_media(accum, accum_size, first_ts,
                    static_cast<uint8_t>(accum_frames), g_ctx.active_codec);
            }

            uint32_t remaining = pcm_bytes - offset;
            if (remaining > 0 && remaining < bytes_per_encode) {
                g_ctx.pcm_residual.resize(remaining);
                memcpy(g_ctx.pcm_residual.data(), pcm_data + offset, remaining);
            } else {
                g_ctx.pcm_residual.clear();
            }

            /* ABR: periodically adjust LDAC quality based on queue depth */
            if (g_ctx.abr_enabled && encoder->codec_type() == AudioCodec::LDAC) {
                LdacEncoder *ldac = static_cast<LdacEncoder *>(encoder);
                if (ldac->is_abr_enabled()) {
                    uint32_t now = GetTickCount();
                    if (now - abr_last_tick >= 100) {
                        abr_last_tick = now;
                        uint32_t queue_depth = 0;
                        BtStackTransport *t = g_ctx.transport.load();
                        if (t) queue_depth = t->get_queue_depth();
                        ldac->abr_adjust(queue_depth);
                    }
                }
            }

            } catch (const std::exception &e) {
                fprintf(stderr, "encode_thread: exception: %s\n", e.what());
                fflush(stderr);
                g_ctx.running.store(false);
                break;
            } catch (...) {
                fprintf(stderr, "encode_thread: unknown exception\n");
                fflush(stderr);
                g_ctx.running.store(false);
                break;
            }
        } /* inner drain loop */

        /* Periodic diagnostics */
        uint32_t now = GetTickCount();
        if (now - log_tick >= 5000) {
            log_tick = now;
            uint32_t oflow = ring_overflow_count.exchange(0, std::memory_order_relaxed);
            if (oflow > 0) {
                fprintf(stderr, "encode_thread: ring overflow dropped %u callbacks in last 5s\n", oflow);
                fflush(stderr);
            }
        }
    }

    if (avrt_handle) AvRevertMmThreadCharacteristics(avrt_handle);
    return 0;
}

static const char *codec_name_for(AudioCodec c) {
    switch (c) {
    case AudioCodec::LDAC:   return "LDAC";
    case AudioCodec::AptxHD: return "aptX HD";
    case AudioCodec::AptxLL: return "aptX Low Latency";
    case AudioCodec::Aptx:   return "aptX";
    case AudioCodec::SBC:    return "SBC";
    case AudioCodec::AAC:    return "AAC";
    }
    return "Unknown";
}

} /* anonymous namespace */

/* ======================================================================== */
/* A2dpService                                                               */
/* ======================================================================== */

A2dpService::A2dpService() {
    check_firmware_present();
}

A2dpService::~A2dpService() {
    stop_streaming();
    /* A joinable std::thread must be joined before destruction or
     * std::terminate is called. Blocks briefly if the worker is still
     * winding down after the stop_streaming poll timed out. */
    if (worker_thread_.joinable())
        worker_thread_.join();
}

std::string A2dpService::get_config_dir() const {
    return ::get_config_dir();
}

std::string A2dpService::get_exe_dir() const {
    char buf[MAX_PATH];
    if (GetModuleFileNameA(NULL, buf, MAX_PATH)) {
        char *sep = strrchr(buf, '\\');
        if (!sep) sep = strrchr(buf, '/');
        if (sep) {
            *sep = '\0';
            return buf;
        }
    }
    return ".";
}

void A2dpService::set_state_callback(StateCallback cb) {
    std::lock_guard<std::mutex> lock(cb_mutex_);
    state_cb_ = std::move(cb);
}

void A2dpService::set_stream_info_callback(StreamInfoCallback cb) {
    std::lock_guard<std::mutex> lock(cb_mutex_);
    stream_info_cb_ = std::move(cb);
}

void A2dpService::set_scan_complete_callback(ScanCompleteCallback cb) {
    std::lock_guard<std::mutex> lock(cb_mutex_);
    scan_complete_cb_ = std::move(cb);
}

void A2dpService::notify_state(State s, const std::string &text) {
    state_.store(s);
    fprintf(stderr, "A2dpService: state=%d text=%s\n", (int)s, text.c_str());
    fflush(stderr);
    std::lock_guard<std::mutex> lock(cb_mutex_);
    if (state_cb_) state_cb_(s, text);
}

void A2dpService::notify_stream_info(const StreamInfo &info) {
    std::lock_guard<std::mutex> lock(cb_mutex_);
    if (stream_info_cb_) stream_info_cb_(info);
}

/* ======================================================================== */
/* Device list                                                               */
/* ======================================================================== */

std::vector<A2dpService::DeviceEntry> A2dpService::get_devices() const {
    std::lock_guard<std::mutex> lock(device_mutex_);
    return device_list_;
}

void A2dpService::load_saved_devices() {
    std::string path = get_config_dir() + "\\devices.txt";
    FILE *f = nullptr;
    fopen_s(&f, path.c_str(), "r");
    if (!f) return;

    char line[256];
    std::lock_guard<std::mutex> lock(device_mutex_);
    while (fgets(line, sizeof(line), f)) {
        char *nl = strchr(line, '\n');
        if (nl) *nl = '\0';
        char *sep = strchr(line, '|');
        if (!sep || (sep - line) < 17) continue;

        DeviceEntry dev{};
        *sep = '\0';
        dev.addr_str = line;
        dev.name = sep + 1;
        dev.audio_device = true;
        dev.saved = true;

        unsigned a[6];
        if (sscanf(line, "%02X:%02X:%02X:%02X:%02X:%02X",
                   &a[0], &a[1], &a[2], &a[3], &a[4], &a[5]) == 6) {
            for (int i = 0; i < 6; i++) dev.address[i] = static_cast<uint8_t>(a[i]);
            device_list_.push_back(std::move(dev));
        }
    }
    fclose(f);
}

void A2dpService::save_device(const char *addr_str, const std::string &name) {
    std::lock_guard<std::mutex> lock(device_mutex_);

    for (auto &d : device_list_) {
        if (d.addr_str == addr_str) {
            d.name = name;
            d.saved = true;
            break;
        }
    }

    std::string path = get_config_dir() + "\\devices.txt";
    FILE *f = nullptr;
    fopen_s(&f, path.c_str(), "w");
    if (!f) return;

    for (auto &d : device_list_) {
        if (d.saved)
            fprintf(f, "%s|%s\n", d.addr_str.c_str(), d.name.c_str());
    }
    fclose(f);
}

void A2dpService::delete_saved_device(int index) {
    std::lock_guard<std::mutex> lock(device_mutex_);

    if (index < 0 || index >= static_cast<int>(device_list_.size())) return;

    auto &d = device_list_[index];
    d.saved = false;

    /* If device was only in the list because it was saved (not discovered
       via scan), remove it entirely so it disappears from the UI. */
    if (!d.audio_device) {
        device_list_.erase(device_list_.begin() + index);
    }

    /* Rewrite devices.txt without deleted entry */
    std::string path = get_config_dir() + "\\devices.txt";
    FILE *f = nullptr;
    fopen_s(&f, path.c_str(), "w");
    if (!f) return;

    for (auto &dd : device_list_) {
        if (dd.saved)
            fprintf(f, "%s|%s\n", dd.addr_str.c_str(), dd.name.c_str());
    }
    fclose(f);
}

/* ======================================================================== */
/* Firmware                                                                  */
/* ======================================================================== */

std::string A2dpService::firmware_status() const {
    std::lock_guard<std::mutex> lock(firmware_status_mutex_);
    return firmware_status_text_;
}

void A2dpService::check_firmware_present() {
    auto validate_file = [](const char *path, uint32_t min_size) -> bool {
        FILE *f = nullptr;
        if (fopen_s(&f, path, "rb") != 0 || !f) return false;
        fseek(f, 0, SEEK_END);
        long sz = ftell(f);
        fseek(f, 0, SEEK_SET);
        char hdr[16] = {};
        fread(hdr, 1, sizeof(hdr), f);
        fclose(f);
        if ((uint32_t)sz < min_size) return false;
        if (memcmp(hdr, "<!doctype", 9) == 0 || memcmp(hdr, "<html", 5) == 0) return false;
        return true;
    };

    /* Check if a file exists (any size, including 0). */
    auto file_exists = [](const char *path) -> bool {
        FILE *f = nullptr;
        if (fopen_s(&f, path, "rb") != 0 || !f) return false;
        fclose(f);
        return true;
    };

    /* Determine firmware filenames based on selected chip type.
     * pid==0 means no chip selected — try stem-based lookup. */
    uint16_t pid = bt_chip_pid_;
    const char *fw = BtAdapterEnumerator::realtek_fw_name(pid);
    const char *cfg = BtAdapterEnumerator::realtek_cfg_name(pid);

    std::string cfg_dir = get_config_dir();
    std::string fw_path, cfg_path;

    if (fw && cfg) {
        /* Known chip — use chip_db filenames */
        fw_path = cfg_dir + "\\" + fw + ".bin";
        cfg_path = cfg_dir + "\\" + cfg + ".bin";
    } else if (!bt_chip_fw_stem_.empty()) {
        /* Unknown chip but firmware stem set from file scan */
        fw_path = cfg_dir + "\\" + bt_chip_fw_stem_ + "_fw.bin";
        cfg_path = cfg_dir + "\\" + bt_chip_fw_stem_ + "_config.bin";
    } else {
        firmware_present_ = false;
        return;
    }

    /* Firmware must be valid binary >10KB. Config may be empty (BTstack uses efuse). */
    firmware_present_ = validate_file(fw_path.c_str(), 10000)
                     && file_exists(cfg_path.c_str());
}

void A2dpService::download_firmware() {
    /* No-op: firmware download is now manual (user downloads via browser).
     * Kept for API compatibility. */
}

/* ======================================================================== */
/* BTstack lifecycle                                                         */
/* ======================================================================== */

bool A2dpService::ensure_btstack_init() {
    std::lock_guard<std::mutex> lock(transport_mutex_);
    if (btstack_ready_.load()) return true;
    if (btstack_init_failed_.load()) return false;

    transport_ = std::make_unique<BtStackTransport>();
    transport_->set_hci_dump_enabled(false);
    if (debug_mode_) {
        transport_->set_hci_dump_file(get_config_dir() + "\\hci_dump.pklg");
    }
    transport_->set_firmware_dir(get_config_dir());
    transport_->set_link_key_dir(get_config_dir());

    /* Use the chip PID selected in Firmware settings.
     * pid==0 means non-Realtek or no chip selected — skip chipset init. */
    {
        uint16_t pid = bt_chip_pid_;
        if (pid != 0) {
            const char *name = BtAdapterEnumerator::realtek_chip_name(pid);
            fprintf(stderr, "A2dpService: Using chip %s (0x%04X)\n",
                    name ? name : "unknown", pid);
        } else {
            fprintf(stderr, "A2dpService: No Realtek chip selected, skipping chipset init\n");
        }
        transport_->set_product_id(pid);
        if (!bt_chip_fw_stem_.empty()) {
            transport_->set_fw_stem(bt_chip_fw_stem_);
        }
    }

    if (!transport_->init(nullptr)) {
        btstack_init_failed_.store(true);
        return false;
    }
    btstack_ready_.store(true);
    return true;
}

void A2dpService::reset_btstack() {
    shutdown_btstack();
    btstack_init_failed_.store(false);
}

void A2dpService::shutdown_btstack() {
    std::lock_guard<std::mutex> lock(transport_mutex_);
    if (transport_) {
        transport_->shutdown();
        transport_.reset();
    }
    btstack_ready_.store(false);
}

/* ======================================================================== */
/* Scanning                                                                  */
/* ======================================================================== */

void A2dpService::start_scan() {
    if (scanning_.load() || running_.load()) return;
    scanning_.store(true);
    notify_state(state_.load(), L("status.initializing_scan"));

    std::thread([this]() {
        scan_thread_func();
    }).detach();
}

/* SEH wrapper — must be a free function without C++ objects to satisfy MSVC */
static int seh_call(void (*func)(void *), void *arg) {
    __try {
        func(arg);
        return 0;
    } __except(EXCEPTION_EXECUTE_HANDLER) {
        return static_cast<int>(GetExceptionCode());
    }
}

void A2dpService::scan_thread_func() {
    int code = seh_call([](void *p) { static_cast<A2dpService *>(p)->scan_thread_func_inner(); }, this);
    if (code != 0) {
        notify_state(State::Error, L("error.btstack_crash_scan"));
        scanning_.store(false);
    }
}

void A2dpService::scan_thread_func_inner() {
    if (!ensure_btstack_init()) {
        if (btstack_init_failed_.load())
            notify_state(State::Error, L("error.btstack_init_restart"));
        else
            notify_state(State::Error, L("error.btstack_init"));
        scanning_.store(false);
        {
            std::lock_guard<std::mutex> lock(cb_mutex_);
            if (scan_complete_cb_) scan_complete_cb_();
        }
        return;
    }

    notify_state(state_.load(), L("status.scanning_devices"));

    transport_->scan_devices(8);

    auto &results = transport_->get_discovered_devices();
    {
        std::lock_guard<std::mutex> lock(device_mutex_);
        device_list_.erase(
            std::remove_if(device_list_.begin(), device_list_.end(),
                           [](const DeviceEntry &e) { return !e.saved; }),
            device_list_.end());

        for (auto &d : results) {
            char buf[18];
            snprintf(buf, sizeof(buf), "%02X:%02X:%02X:%02X:%02X:%02X",
                     d.address[0], d.address[1], d.address[2],
                     d.address[3], d.address[4], d.address[5]);

            bool exists = false;
            for (auto &existing : device_list_) {
                if (existing.addr_str == buf) {
                    if (!d.name.empty()) existing.name = d.name;
                    exists = true;
                    break;
                }
            }
            if (!exists) {
                DeviceEntry sd{};
                memcpy(sd.address, d.address, 6);
                sd.name = d.name;
                sd.addr_str = buf;
                sd.audio_device = (((d.cod >> 8) & 0x1F) == 0x04);
                sd.saved = false;
                device_list_.push_back(std::move(sd));
            }
        }
    }

    char msg[64];
    snprintf(msg, sizeof(msg), "Scan complete: %zu device(s) found",
             transport_->get_discovered_devices().size());
    notify_state(state_.load(), msg);

    scanning_.store(false);
    {
        std::lock_guard<std::mutex> lock(cb_mutex_);
        if (scan_complete_cb_) scan_complete_cb_();
    }
}

/* ======================================================================== */
/* Streaming                                                                 */
/* ======================================================================== */

void A2dpService::start_streaming(const ConnectionProfile &profile) {
    fprintf(stderr, "A2dpService: start_streaming called, running=%d\n", running_.load());
    fprintf(stderr, "A2dpService:   profile='%s' codec=%s bit_depth=%u sample_rate=%u\n",
            profile.name.c_str(), profile.codec.c_str(), profile.bit_depth, profile.sample_rate);
    fprintf(stderr, "A2dpService:   capture_mode=%s audio_device='%s' auto_switch=%d\n",
            profile.capture_mode.c_str(), profile.audio_device_name.c_str(), profile.auto_switch_device);
    fflush(stderr);

    if (running_.load()) {
        /* A previous session is still active (e.g. the auto-reconnect loop
         * after the device was powered off). Ask it to stop and abort any
         * blocking transport waits so the join below is bounded.
         * stop_requested_ must stay set until the old thread has exited —
         * clearing it earlier would revive the old reconnect loop. */
        fprintf(stderr, "A2dpService: stopping previous session before start\n");
        fflush(stderr);
        stop_requested_.store(true);
        g_ctx.running.store(false);
        std::unique_lock<std::mutex> lock(transport_mutex_, std::try_to_lock);
        if (lock.owns_lock() && transport_)
            transport_->cancel_pending_waits();
        /* If the mutex is contended the worker is inside BTstack init or
         * shutdown, not in a cancellable wait — it will see stop_requested_. */
    }

    /* Join any previous thread before touching shared session state */
    if (worker_thread_.joinable()) {
        fprintf(stderr, "A2dpService: joining previous worker thread\n");
        fflush(stderr);
        worker_thread_.join();
    }

    active_profile_ = profile;
    stop_requested_.store(false);
    running_.store(true);
    notify_state(State::Connecting, L("status.initializing"));

    worker_thread_ = std::thread(&A2dpService::streaming_thread_func, this);
    fprintf(stderr, "A2dpService: new worker thread started\n");
    fflush(stderr);
}

void A2dpService::stop_streaming() {
    fprintf(stderr, "A2dpService: stop_streaming called, running=%d\n", running_.load());
    fflush(stderr);
    stop_requested_.store(true);
    g_ctx.running.store(false);

    /* Cancel any blocking waits in transport layer (e.g. connect/reconnect).
     * try_lock: if the worker holds transport_mutex_ it is inside BTstack
     * init/shutdown, not in a cancellable wait — don't block the UI on it. */
    {
        std::unique_lock<std::mutex> lock(transport_mutex_, std::try_to_lock);
        if (lock.owns_lock() && transport_)
            transport_->cancel_pending_waits();
    }

    /* Poll for thread to finish (don't block UI forever) */
    int wait = 0;
    while (running_.load() && wait < 100) {
        Sleep(50);
        wait++;
    }
    fprintf(stderr, "A2dpService: stop_streaming wait=%d running=%d\n", wait, running_.load());
    fflush(stderr);

    if (running_.load()) {
        /* Worker is still winding down. Do NOT force running_ to false —
         * start_streaming() relies on it to know it must stop/join the old
         * session first. The worker exits on its own shortly. */
        fprintf(stderr, "A2dpService: worker still active after stop timeout\n");
        fflush(stderr);
    } else if (worker_thread_.joinable()) {
        worker_thread_.join();
    }
    notify_state(State::Idle, L("status.ready"));
}

void A2dpService::streaming_thread_func() {
    int code = seh_call([](void *p) { static_cast<A2dpService *>(p)->streaming_thread_func_inner(); }, this);
    if (code != 0) {
        fprintf(stderr, "A2dpService: SEH exception 0x%08x in streaming thread\n", (unsigned)code);
        fflush(stderr);
        notify_state(State::Error, L("error.btstack_crash_stream"));
        g_ctx.running.store(false);
        g_ctx.encoder.store(nullptr);
        g_ctx.transport.store(nullptr);
        running_.store(false);
    }
}

void A2dpService::streaming_thread_func_inner() {
    CoInitializeEx(nullptr, COINIT_MULTITHREADED);

    auto &p = active_profile_;

    LOG_INFO("A2dpService: streaming_thread started");
    LOG_INFO("A2dpService:   codec=%s quality=%s bit_depth=%u sample_rate=%u",
             p.codec.c_str(), p.quality.c_str(), p.bit_depth, p.sample_rate);
    LOG_INFO("A2dpService:   capture_mode=%s device_addr=%s device_name=%s",
             p.capture_mode.c_str(), p.device_address.c_str(), p.device_name.c_str());

    /* Parse device address */
    uint8_t target_addr[6] = {};
    if (!BtDeviceDiscovery::parse_address(p.device_address.c_str(), target_addr)) {
        notify_state(State::Error, L("error.invalid_address"));
        running_.store(false);
        return;
    }

    /* Determine codec */
    int codec_index = ProfileManager::codec_to_index(p.codec);
    AudioCodec requested_codec = AudioCodec::LDAC;
    bool auto_codec = (codec_index == 0);
    switch (codec_index) {
    case 1: requested_codec = AudioCodec::LDAC;   break;
    case 2: requested_codec = AudioCodec::AptxHD; break;
    case 3: requested_codec = AudioCodec::AptxLL; break;
    case 4: requested_codec = AudioCodec::Aptx;   break;
    case 5: requested_codec = AudioCodec::SBC;    break;
    case 6: requested_codec = AudioCodec::AAC;    break;
    }

    EncoderQuality quality = EncoderQuality::High;
    int quality_index = ProfileManager::quality_to_index(p.quality);
    switch (quality_index) {
    case 1: quality = EncoderQuality::Standard; break;
    case 2: quality = EncoderQuality::Mobile;   break;
    }

    /* Initialize BTstack */
    notify_state(State::Connecting, L("status.initializing_btstack"));
    if (!ensure_btstack_init()) {
        notify_state(State::Error, L("error.btstack_init"));
        running_.store(false);
        return;
    }

    BtStackTransport *transport = transport_.get();
    LOG_INFO("A2dpService: BTstack initialized successfully");

    if (stop_requested_.load()) { running_.store(false); notify_state(State::Idle, L("status.ready")); return; }

    /* Connect */
    notify_state(State::Connecting, L("status.connecting_device"));
    if (!transport->connect_a2dp(target_addr)) {
        if (stop_requested_.load()) {
            running_.store(false);
            notify_state(State::Idle, L("status.ready"));
            return;
        }
        notify_state(State::Error, L("error.pairing_hint"));
        running_.store(false);
        return;
    }

    LOG_INFO("A2dpService: device connected, negotiating codec");

    if (stop_requested_.load()) { transport->disconnect(); running_.store(false); notify_state(State::Idle, L("status.ready")); return; }

    /* Select codec */
    auto caps = transport->get_remote_caps();
    AudioCodec selected_codec = AudioCodec::LDAC;
    bool found = false;

    if (!auto_codec) {
        switch (requested_codec) {
        case AudioCodec::LDAC:   if (caps.ldac)    { selected_codec = AudioCodec::LDAC;   found = true; } break;
        case AudioCodec::AptxHD: if (caps.aptx_hd) { selected_codec = AudioCodec::AptxHD; found = true; } break;
        case AudioCodec::AptxLL: if (caps.aptx_ll) { selected_codec = AudioCodec::AptxLL; found = true; } break;
        case AudioCodec::Aptx:   if (caps.aptx)    { selected_codec = AudioCodec::Aptx;   found = true; } break;
        case AudioCodec::SBC:    if (caps.sbc)     { selected_codec = AudioCodec::SBC;    found = true; } break;
        case AudioCodec::AAC:    if (caps.aac)     { selected_codec = AudioCodec::AAC;    found = true; } break;
        }
        if (!found) {
            char msg[256];
            bool aptx_family = (requested_codec == AudioCodec::Aptx ||
                                requested_codec == AudioCodec::AptxHD ||
                                requested_codec == AudioCodec::AptxLL);
            if (aptx_family && caps.aptx_adaptive)
                /* Sink offers only aptX Adaptive, which has no open encoder */
                snprintf(msg, sizeof(msg), L("error.codec_adaptive_only"), codec_name_for(requested_codec));
            else
                snprintf(msg, sizeof(msg), L("error.codec_not_supported"), codec_name_for(requested_codec));
            notify_state(State::Error, msg);
            transport->disconnect();
            running_.store(false);
            return;
        }
    } else {
        if (caps.ldac)        { selected_codec = AudioCodec::LDAC;   found = true; }
        else if (caps.aptx_hd){ selected_codec = AudioCodec::AptxHD; found = true; }
        else if (caps.aptx_ll){ selected_codec = AudioCodec::AptxLL; found = true; }
        else if (caps.aptx)   { selected_codec = AudioCodec::Aptx;   found = true; }
        else if (caps.aac)    { selected_codec = AudioCodec::AAC;    found = true; }
        else if (caps.sbc)    { selected_codec = AudioCodec::SBC;    found = true; }
    }
    if (!found) {
        notify_state(State::Error, L("error.no_compatible_codec"));
        transport->disconnect();
        running_.store(false);
        return;
    }

    g_ctx.active_codec = selected_codec;
    LOG_INFO("A2dpService: codec selected: %s (caps: ldac=%d aptxhd=%d aptxll=%d aptx=%d "
             "aptx_adaptive=%d[unsupported] aac=%d sbc=%d)",
             codec_name_for(selected_codec), caps.ldac, caps.aptx_hd, caps.aptx_ll, caps.aptx,
             caps.aptx_adaptive, caps.aac, caps.sbc);

    /* Initialize audio capture */
    notify_state(State::Connecting, L("status.initializing_audio"));
    WasapiCapture wasapi_capture;

    int capture_mode_index = ProfileManager::capture_mode_to_index(p.capture_mode);
    CaptureMode cmode = static_cast<CaptureMode>(capture_mode_index);

    uint32_t preferred_sr = p.sample_rate; /* 0 = auto */

    /* Determine max sample rate for codec */
    uint32_t codec_max_sr = 48000;
    if (selected_codec == AudioCodec::LDAC) {
        codec_max_sr = 96000;
    }

    /* If auto, query device native rate */
    if (preferred_sr == 0) {
        WasapiCapture temp_cap;
        std::wstring dev_id;
        if (cmode == CaptureMode::VirtualDevice && !p.audio_device_id.empty()) {
            int wlen = MultiByteToWideChar(CP_UTF8, 0, p.audio_device_id.c_str(),
                (int)p.audio_device_id.size(), nullptr, 0);
            if (wlen > 0) {
                dev_id.resize(wlen);
                MultiByteToWideChar(CP_UTF8, 0, p.audio_device_id.c_str(),
                    (int)p.audio_device_id.size(), &dev_id[0], wlen);
            }
        }
        if (temp_cap.init(0, dev_id.empty() ? nullptr : dev_id.c_str())) {
            preferred_sr = temp_cap.get_sample_rate();
        } else {
            preferred_sr = 48000; /* Fallback */
        }
    }

    /* Cap sample rate */
    if (preferred_sr > codec_max_sr) {
        LOG_INFO("A2dpService: Limiting capture sample rate from %u to %u Hz for codec %s",
                 preferred_sr, codec_max_sr, codec_name_for(selected_codec));
        preferred_sr = codec_max_sr;
    }

    /* aptX family: only 44.1/48 kHz, and only the rates the sink lists.
     * Request one the remote supports; WASAPI auto-resamples to it. */
    if (selected_codec == AudioCodec::Aptx || selected_codec == AudioCodec::AptxHD ||
        selected_codec == AudioCodec::AptxLL) {
        uint32_t aptx_sr = transport->pick_aptx_sample_rate(selected_codec, preferred_sr);
        if (aptx_sr != preferred_sr) {
            LOG_INFO("A2dpService: %s: using %u Hz capture instead of %u Hz "
                     "(remote/codec supported rate)",
                     codec_name_for(selected_codec), aptx_sr, preferred_sr);
            preferred_sr = aptx_sr;
        }
    }

    switch (cmode) {
    case CaptureMode::SystemLoopback:
        if (!wasapi_capture.init(preferred_sr)) {
            notify_state(State::Error, L("error.wasapi_init"));
            transport->disconnect();
            running_.store(false);
            return;
        }
        break;

    case CaptureMode::VirtualDevice: {
        std::wstring dev_id;
        if (!p.audio_device_id.empty()) {
            int wlen = MultiByteToWideChar(CP_UTF8, 0, p.audio_device_id.c_str(),
                (int)p.audio_device_id.size(), nullptr, 0);
            if (wlen > 0) {
                dev_id.resize(wlen);
                MultiByteToWideChar(CP_UTF8, 0, p.audio_device_id.c_str(),
                    (int)p.audio_device_id.size(), &dev_id[0], wlen);
            }
        }
        if (dev_id.empty()) {
            notify_state(State::Error, L("error.no_virtual_device_selected"));
            transport->disconnect();
            running_.store(false);
            return;
        }
        if (p.auto_switch_device) {
            original_default_device_ = AudioDeviceEnumerator::get_default_device_id();
            if (!AudioDeviceEnumerator::set_default_device(dev_id)) {
                fprintf(stderr, "warning: failed to switch default device\n");
            }
        }
        if (!wasapi_capture.init(preferred_sr, dev_id.c_str())) {
            notify_state(State::Error, L("error.virtual_capture_init"));
            if (p.auto_switch_device && !original_default_device_.empty()) {
                AudioDeviceEnumerator::set_default_device(original_default_device_);
                original_default_device_.clear();
            }
            transport->disconnect();
            running_.store(false);
            return;
        }
        break;
    }
    }

    uint32_t sr = wasapi_capture.get_sample_rate();
    uint32_t ch = wasapi_capture.get_channels();
    uint32_t use_ch = (ch > 2) ? 2 : ch;
    g_ctx.active_channels = use_ch;
    LOG_INFO("A2dpService: WASAPI capture init OK (sr=%u ch=%u use_ch=%u)", sr, ch, use_ch);

    /* Configure codec */
    if (!transport->configure_codec(selected_codec, sr, static_cast<uint8_t>(use_ch))) {
        transport->disconnect();
        running_.store(false);
        /* A user stop cancels the blocking wait — that's not an error */
        if (stop_requested_.load())
            notify_state(State::Idle, L("status.ready"));
        else
            notify_state(State::Error, L("error.codec_configure"));
        return;
    }

    /* Create encoder */
    std::unique_ptr<AudioEncoder> encoder;
    uint16_t media_mtu = transport->get_media_mtu();
    if (media_mtu == 0) media_mtu = 679;

    switch (selected_codec) {
    case AudioCodec::LDAC:   encoder = std::make_unique<LdacEncoder>(); break;
    case AudioCodec::AptxHD: encoder = std::make_unique<AptxHdEncoder>(); break;
    case AudioCodec::AptxLL: encoder = std::make_unique<AptxLlEncoder>(); break;
    case AudioCodec::Aptx:   encoder = std::make_unique<AptxEncoder>(); break;
    case AudioCodec::SBC:    encoder = std::make_unique<SbcEncoder>(); break;
#ifdef AAC_ENCODER_AVAILABLE
    case AudioCodec::AAC:    encoder = std::make_unique<AacEncoder>(); break;
#else
    case AudioCodec::AAC:
        notify_state(State::Error, L("error.aac_unavailable"));
        transport->disconnect();
        running_.store(false);
        return;
#endif
    }

    int bit_depth_index = ProfileManager::bit_depth_to_index(p.bit_depth);
    bool use_24bit = false;
    if (selected_codec == AudioCodec::LDAC && bit_depth_index != 1) {
        static_cast<LdacEncoder *>(encoder.get())->set_bit_depth(24);
        use_24bit = true;
    } else if (selected_codec == AudioCodec::AptxHD && bit_depth_index != 1) {
        /* aptX HD carries 24-bit PCM: feed it int32 (top 24 bits encoded) */
        static_cast<AptxHdEncoder *>(encoder.get())->set_bit_depth(24);
        use_24bit = true;
    }
    LOG_INFO("A2dpService: encoder config: bit_depth_index=%d use_24bit=%d mtu=%u",
             bit_depth_index, use_24bit, media_mtu);

    if (!encoder->init(media_mtu, quality, sr, use_ch)) {
        notify_state(State::Error, L("error.encoder_init"));
        transport->disconnect();
        running_.store(false);
        return;
    }

    g_ctx.bytes_per_sample = use_24bit ? 4 : 2;

    bool abr = p.abr && (selected_codec == AudioCodec::LDAC);
    if (abr) {
        LdacEncoder *ldac = static_cast<LdacEncoder *>(encoder.get());
        if (!ldac->init_abr(100)) abr = false;
    }
    g_ctx.abr_enabled = abr;

    LOG_INFO("A2dpService: encoder initialized, bitrate=%u kbps, abr=%d",
             encoder->get_bitrate_kbps(), abr);

    /* Start stream */
    if (!transport->start_stream()) {
        transport->disconnect();
        running_.store(false);
        if (stop_requested_.load())
            notify_state(State::Idle, L("status.ready"));
        else
            notify_state(State::Error, L("error.stream_start"));
        return;
    }

    /* Update status */
    notify_state(State::Streaming, L("status.connected"));
    notify_stream_info({codec_name_for(selected_codec),
                        encoder->get_bitrate_kbps(), sr, use_ch,
                        wasapi_capture.get_sample_rate(),
                        wasapi_capture.get_channels(),
                        wasapi_capture.get_bits_per_sample()});

    /* Save device for future use */
    {
        std::string dev_name;
        {
            std::lock_guard<std::mutex> lock(device_mutex_);
            for (auto &d : device_list_) {
                if (d.addr_str == p.device_address) {
                    dev_name = d.name;
                    break;
                }
            }
        }
        save_device(p.device_address.c_str(), dev_name);
    }

    /* Start audio pipeline */
    g_ctx.timestamp = 0;
    g_ctx.pcm_residual.clear();
    g_ctx.encoder.store(encoder.get());
    g_ctx.transport.store(transport);
    g_ctx.running.store(true);

    /* Initialize ring buffer and encode thread */
    g_ctx.ring.init(256 * 1024);
    HANDLE encode_thread = CreateThread(nullptr, 0, encode_thread_func, nullptr, 0, nullptr);
    if (!encode_thread) {
        notify_state(State::Error, L("error.encode_thread"));
        g_ctx.running.store(false);
        g_ctx.encoder.store(nullptr);
        g_ctx.transport.store(nullptr);
        g_ctx.ring.destroy();
        transport->disconnect();
        if (!original_default_device_.empty()) {
            AudioDeviceEnumerator::set_default_device(original_default_device_);
            original_default_device_.clear();
        }
        running_.store(false);
        return;
    }

    LOG_INFO("A2dpService: A2DP stream started, starting audio capture");
    bool capture_started = wasapi_capture.start(service_audio_callback);
    if (!capture_started) {
        notify_state(State::Error, L("error.audio_capture_start"));
        g_ctx.running.store(false);
        SetEvent(g_ctx.ring.data_event);
        WaitForSingleObject(encode_thread, 5000);
        CloseHandle(encode_thread);
        g_ctx.encoder.store(nullptr);
        g_ctx.transport.store(nullptr);
        g_ctx.ring.destroy();
        transport->disconnect();
        if (!original_default_device_.empty()) {
            AudioDeviceEnumerator::set_default_device(original_default_device_);
            original_default_device_.clear();
        }
        running_.store(false);
        return;
    }

    /* Main loop: keep streaming, auto-reconnect on disconnect */
    try {
    while (!stop_requested_.load()) {
        Sleep(200);

        if (transport->check_disconnected()) {
            LOG_WARN("A2dpService: connection lost, attempting reconnect");
            notify_state(State::Reconnecting, L("status.connection_lost"));

            g_ctx.transport.store(nullptr);
            g_ctx.encoder.store(nullptr);

            bool reconnected = false;
            for (int attempt = 1; attempt <= 10 && !stop_requested_.load(); attempt++) {
                char msg[64];
                snprintf(msg, sizeof(msg), L("status.reconnect_attempt"), attempt, 10);
                notify_state(State::Reconnecting, msg);
                /* Interruptible wait — react to stop within ~100ms */
                for (int i = 0; i < 30 && !stop_requested_.load(); i++)
                    Sleep(100);

                if (stop_requested_.load()) break;

                if (transport->reconnect()) {
                    LOG_INFO("A2dpService: reconnected on attempt %d", attempt);
                    reconnected = true;
                    break;
                }
                LOG_WARN("A2dpService: reconnect attempt %d failed", attempt);
            }

            if (!reconnected) {
                /* User-requested stop is not an error — cleanup notifies Idle */
                if (!stop_requested_.load())
                    notify_state(State::Error, L("error.reconnect_failed"));
                break;
            }

            g_ctx.timestamp = 0;
            g_ctx.pcm_residual.clear();
            /* Flush stale data from ring buffer */
            g_ctx.ring.tail.store(g_ctx.ring.head.load(std::memory_order_relaxed),
                                  std::memory_order_relaxed);
            g_ctx.encoder.store(encoder.get());
            g_ctx.transport.store(transport);
            notify_state(State::Streaming, L("status.reconnected"));
        }
    }
    } catch (...) {
        LOG_ERROR("A2dpService: exception in streaming loop");
    }

    /* Cleanup — order matters:
     * 1. Stop WASAPI (producer)
     * 2. Signal encode thread to exit, wait for it
     * 3. Release transport/encoder
     * 4. Destroy ring buffer */
    LOG_INFO("A2dpService: streaming stopped, cleaning up");
    wasapi_capture.stop();
    g_ctx.running.store(false);
    if (g_ctx.ring.data_event) SetEvent(g_ctx.ring.data_event);
    WaitForSingleObject(encode_thread, 5000);
    CloseHandle(encode_thread);
    g_ctx.encoder.store(nullptr);
    g_ctx.transport.store(nullptr);
    g_ctx.ring.destroy();

    transport->stop_stream();
    transport->disconnect();

    if (!original_default_device_.empty()) {
        AudioDeviceEnumerator::set_default_device(original_default_device_);
        original_default_device_.clear();
    }

    encoder->shutdown();
    running_.store(false);
    if (!stop_requested_.load()) {
        /* Abnormal exit (error in loop) — state already set */
    } else {
        notify_state(State::Idle, L("status.ready"));
    }

    CoUninitialize();
}
