/*
 * A2DP Service - Backend streaming logic (framework-agnostic)
 *
 * Extracted from gui_app.cpp to decouple backend from GUI framework.
 * Handles BTstack init, device scanning, codec negotiation,
 * audio capture (WASAPI), encoding, and Bluetooth streaming.
 *
 * SPDX-License-Identifier: MIT
 */

#ifndef A2DP_SERVICE_H
#define A2DP_SERVICE_H

#include "audio_encoder.h"
#include "media_payload_limit.h"
#include "audio_device_enum.h"
#include "bt_device.h"
#include "profile_manager.h"
#include <cstdint>
#include <string>
#include <vector>
#include <atomic>
#include <mutex>
#include <thread>
#include <memory>
#include <functional>

class BtStackTransport;
class WasapiCapture;
class AudioEncoder;
class LdacEncoder;

class A2dpService {
public:
    A2dpService();
    ~A2dpService();

    /* Streaming state */
    enum class State {
        Idle,
        Connecting,
        Streaming,
        Reconnecting,
        Error
    };

    /* Scanned device entry */
    struct DeviceEntry {
        uint8_t     address[6];
        std::string name;
        std::string addr_str;    /* "XX:XX:XX:XX:XX:XX" */
        bool        audio_device;
        bool        saved;
    };

    /* Stream info for display */
    struct StreamInfo {
        std::string codec;
        uint32_t    bitrate_kbps;
        uint32_t    sample_rate;
        uint32_t    channels;
        uint32_t    source_sample_rate = 0;
        uint32_t    source_channels = 0;
        uint32_t    source_bit_depth = 0;
    };

    /* ---- Callbacks (called from worker threads, must be thread-safe) ---- */
    using StateCallback = std::function<void(State state, const std::string &status_text)>;
    using StreamInfoCallback = std::function<void(const StreamInfo &info)>;
    using ScanCompleteCallback = std::function<void()>;

    void set_state_callback(StateCallback cb);
    void set_stream_info_callback(StreamInfoCallback cb);
    void set_scan_complete_callback(ScanCompleteCallback cb);

    /* ---- Streaming control ---- */
    void start_streaming(const ConnectionProfile &profile);
    void stop_streaming();

    /* ---- Device scanning ---- */
    void start_scan();
    bool is_scanning() const { return scanning_.load(); }

    /* ---- Device list (thread-safe access) ---- */
    std::vector<DeviceEntry> get_devices() const;
    void load_saved_devices();
    void save_device(const char *addr_str, const std::string &name);
    void delete_saved_device(int index);

    /* ---- State query ---- */
    State state() const { return state_.load(); }

    /* ---- Firmware management ---- */
    bool is_firmware_present() const { return firmware_present_; }
    bool is_firmware_downloading() const { return firmware_downloading_.load(); }
    bool was_firmware_updated() { return firmware_updated_.exchange(false); }
    std::string firmware_status() const;
    void check_firmware_present();
    void download_firmware();

    /* ---- Bluetooth adapter ---- */
    void set_bt_chip_pid(uint16_t pid) { bt_chip_pid_ = pid; }
    uint16_t get_bt_chip_pid() const { return bt_chip_pid_; }
    void set_bt_chip_fw_stem(const std::string &stem) { bt_chip_fw_stem_ = stem; }
    const std::string &get_bt_chip_fw_stem() const { return bt_chip_fw_stem_; }

    /* ---- Debug ---- */
    void set_debug_mode(bool enabled) { debug_mode_ = enabled; }

    /* ---- Advanced ---- */
    /* Max media packet size (BtStackTransport::set_media_payload_limit); applies from the next connection */
    void set_media_payload_limit(uint16_t limit) { media_payload_limit_.store(limit); }

    /* Force BTstack shutdown so next connect reinitializes with new settings */
    void reset_btstack();

    /* ---- Config ---- */
    std::string get_config_dir() const;
    std::string get_exe_dir() const;

private:
    /* ---- BTstack lifecycle ---- */
    bool ensure_btstack_init();
    void shutdown_btstack();

    /* ---- Scanning thread ---- */
    void scan_thread_func();
    void scan_thread_func_inner();

    /* ---- Streaming thread ---- */
    void streaming_thread_func();
    void streaming_thread_func_inner();

    /* ---- State ---- */
    std::atomic<State> state_{State::Idle};

    /* ---- Callbacks ---- */
    StateCallback state_cb_;
    StreamInfoCallback stream_info_cb_;
    ScanCompleteCallback scan_complete_cb_;
    std::mutex cb_mutex_;

    void notify_state(State s, const std::string &text);
    void notify_stream_info(const StreamInfo &info);

    /* ---- Device list ---- */
    mutable std::mutex device_mutex_;
    std::vector<DeviceEntry> device_list_;
    std::atomic<bool> scanning_{false};

    /* ---- BTstack transport ---- */
    std::unique_ptr<BtStackTransport> transport_;
    std::mutex transport_mutex_;
    std::atomic<bool> btstack_ready_{false};
    std::atomic<bool> btstack_init_failed_{false};

    /* ---- Worker thread ---- */
    std::thread worker_thread_;
    std::atomic<bool> running_{false};
    std::atomic<bool> stop_requested_{false};

    /* ---- Active streaming profile (copied on start) ---- */
    ConnectionProfile active_profile_;

    /* ---- Audio device list ---- */
    std::vector<AudioDeviceInfo> audio_device_list_;
    std::wstring original_default_device_;

    /* ---- Bluetooth adapter ---- */
    uint16_t bt_chip_pid_ = 0;  /* 0 = auto-detect */
    std::string bt_chip_fw_stem_;  /* Firmware stem for unknown chips (pid==0) */

    /* ---- Debug ---- */
    bool debug_mode_ = false;

    /* ---- Advanced ---- */
    std::atomic<uint16_t> media_payload_limit_{MEDIA_PAYLOAD_LIMIT_DEFAULT};

    /* ---- Firmware ---- */
    bool firmware_present_ = false;
    std::atomic<bool> firmware_downloading_{false};
    std::atomic<bool> firmware_updated_{false};
    mutable std::mutex firmware_status_mutex_;
    std::string firmware_status_text_;
};

#endif /* A2DP_SERVICE_H */
