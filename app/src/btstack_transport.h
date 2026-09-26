/*
 * BTstack Transport Layer
 *
 * User-mode Bluetooth transport using BTstack + WinUSB.
 * Manages HCI, L2CAP, AVDTP, and A2DP via BTstack's event-driven API,
 * exposing synchronous methods to the caller.
 *
 * SPDX-License-Identifier: MIT
 */

#ifndef BTSTACK_TRANSPORT_H
#define BTSTACK_TRANSPORT_H

#include "audio_encoder.h"
#include "media_payload_limit.h"
#include "link_stats.h"
#include "bt_adapter_enum.h"
#include <cstdint>
#include <string>
#include <vector>
#include <atomic>
#include <functional>
#include <mutex>

/* BTstack forward declarations (avoid exposing full BTstack headers) */
struct btstack_timer_source;
struct avdtp_stream_endpoint;

class BtStackTransport {
public:
    /* RTP media header size (BTstack AVDTP_MEDIA_PAYLOAD_HEADER_SIZE); already
     * subtracted from get_media_mtu(). Codecs sent without RTP (aptX, aptX LL)
     * may use get_media_mtu() + RTP_HEADER_SIZE bytes of payload. */
    static constexpr uint32_t RTP_HEADER_SIZE = 12;
    /* Capacity of one queued media packet (MediaPacket::data): the largest
     * L2CAP payload with HCI_ACL_PAYLOAD_SIZE (1691 + 4-byte L2CAP header) */
    static constexpr uint32_t MAX_MEDIA_PACKET_SIZE = 1691;
    static_assert(MEDIA_PAYLOAD_LIMIT_MAX + RTP_HEADER_SIZE <= MAX_MEDIA_PACKET_SIZE,
                  "media payload limit must fit a queue slot (aptX: payload + RTP header size)");

    BtStackTransport();
    ~BtStackTransport();

    /*
     * Initialize BTstack with WinUSB HCI transport.
     * usb_path: optional USB device path for specific adapter (nullptr = auto-detect)
     * Returns true when HCI is powered on and ready.
     */
    bool init(const char *usb_path = nullptr);

    /* Disable HCI packet dump to stdout (call before init for GUI mode) */
    void set_hci_dump_enabled(bool enabled) { hci_dump_enabled_ = enabled; }

    /* Install the on-demand HCI capture (hci_capture.h) as the HCI dump.
     * Debug mode only; capture itself is started / stopped via hci_capture. */
    void set_hci_capture_enabled(bool enabled) { hci_capture_enabled_ = enabled; }

    /* Development / testing: use H4 over TCP ("host:port") to a virtual
     * controller instead of a WinUSB adapter (call before init) */
    void set_hci_tcp(const std::string &host_port) { hci_tcp_ = host_port; }

    /* Set directory for persistent link key storage (call before init) */
    void set_link_key_dir(const std::string &path) { link_key_dir_ = path; }

    /* Set directory where firmware files are stored (call before init) */
    void set_firmware_dir(const std::string &path) { firmware_dir_ = path; }

    /* Set Realtek USB Product ID for firmware loading (call before init).
     * 0 = non-Realtek adapter (skip Realtek chipset init). */
    void set_product_id(uint16_t pid) { product_id_ = pid; }

    /* Set firmware stem for unknown chips (pid==0 but firmware files present) */
    void set_fw_stem(const std::string &stem) { fw_stem_ = stem; }

    /* Shut down BTstack and release USB adapter */
    void shutdown();

    /* AFH host channel classification policy of the process: "auto",
     * "off" or Wi-Fi channels to avoid such as "6,11" (afh.h). A2DPWB is the
     * central of its links, so it applies to headphones and receivers alike.
     * Takes effect within seconds, also on a running link. */
    static void set_afh_policy(const std::string &policy);
    static std::string afh_policy();
    /* The radio state of the link is published as link_radio() (link_stats.h) */

    /* Discovered device info from GAP inquiry */
    struct DiscoveredDevice {
        uint8_t     address[6];  /* big-endian (BTstack format) */
        std::string name;
        uint32_t    cod;         /* Class of Device */
        int8_t      rssi;
    };

    /*
     * Scan for nearby Bluetooth devices using GAP inquiry.
     * duration_seconds: how long to scan (3-30)
     * Returns true if any devices were found.
     */
    bool scan_devices(uint8_t duration_seconds = 10);

    /* Get devices discovered during last scan_devices() call */
    const std::vector<DiscoveredDevice> &get_discovered_devices() const { return discovered_devices_; }

    /*
     * Connect to a remote Bluetooth A2DP sink device.
     * Performs: signaling connection, SEP discovery, capability check.
     * Blocks until connection + stream establishment completes or fails.
     */
    bool connect_a2dp(const uint8_t remote_addr[6]);

    /* How the last connect_a2dp() failed */
    enum class ConnectFailure {
        None,
        NoAnswer,   /* page timeout: the device did not answer */
        LinkLost,   /* link up, then no answer on it (L2CAP RTX / connection timeout) */
        Refused,    /* link up, then ended by the device itself (e.g. it is turning
                     * off or does not accept connections now) */
        AuthFailed, /* authentication / security refused, e.g. a stale link key */
        Other       /* anything else, e.g. no A2DP service */
    };
    ConnectFailure last_connect_failure() const { return last_connect_failure_.load(); }

    /*
     * connect_a2dp(), tried again after a transient failure (NoAnswer /
     * LinkLost) up to CONNECT_ATTEMPTS times in all: a link can fail right
     * after it came up and work on the next attempt. After an authentication
     * failure with a stored link key (the device was reset or paired
     * elsewhere) the key is dropped and pairing is tried once more.
     * cancelled() is polled between attempts; on_retry(attempt, attempts)
     * reports each new attempt.
     */
    static constexpr int CONNECT_ATTEMPTS = 3;

    bool connect_a2dp_retrying(const uint8_t remote_addr[6],
                               const std::function<bool()> &cancelled,
                               const std::function<void(int, int)> &on_retry);

    /* HCI Disconnect of the ACL link to the last remote, if one exists, and
     * wait for Disconnection Complete (up to timeout_ms). A link that came
     * up and then went silent otherwise stays until the supervision timeout
     * and the next attempt would reuse it. */
    void drop_acl_link(uint32_t timeout_ms);

    /* An AVDTP connection to the remote still known to BTstack before a new
     * connect (e.g. left over when a stream was given up on) makes
     * a2dp_source_establish_stream() fail with COMMAND_DISALLOWED: close it
     * and wait for the release. */
    void release_leftover_avdtp();

    /* Disconnect from the remote device */
    bool disconnect();

    /* Cancel any blocking wait_for_event() calls (thread-safe) */
    void cancel_pending_waits();

    /*
     * Configure the stream with the specified codec.
     * Must be called after connect_a2dp() succeeds.
     * If codec is not supported by remote, returns false.
     */
    bool configure_codec(AudioCodec codec, uint32_t sample_rate, uint8_t channels);

    /* Start A2DP streaming. Blocks until AVDTP Start is acknowledged. */
    bool start_stream();

    /* Stop (suspend) A2DP streaming. */
    bool stop_stream();

    /*
     * Send encoded media data.
     * Thread-safe: can be called from WASAPI callback thread.
     * data: encoded audio frame(s) with codec-specific payload header
     * timestamp: RTP timestamp (sample count)
     * frames: number of codec frames in this packet
     * codec: which codec produced the data (for payload header)
     */
    bool send_media(const uint8_t *data, uint32_t size,
                    uint32_t timestamp, uint8_t frames,
                    AudioCodec codec);

    /* Media payload size to use (RTP header excluded): the remote's media
     * channel MTU, limited by set_media_payload_limit(), fixed when the
     * stream is established */
    uint16_t get_media_mtu() const;

    /* Upper limit for get_media_mtu(), clamped to MEDIA_PAYLOAD_LIMIT_MIN..MAX.
     * Applies to streams established afterwards. */
    void set_media_payload_limit(uint32_t limit) { media_payload_limit_.store(clamp_media_payload_limit(limit)); }

    /* Check if connected and stream is active */
    bool is_connected() const;
    bool is_streaming() const;

    /* Get and reset the count of failed send_media calls (for ABR) */
    uint32_t get_and_reset_send_failure_count();

    /* Get current queue depth (instantaneous packet count, for ABR TxQueueDepth) */
    uint32_t get_queue_depth() const;

    /* Get the codec selected during connect (for auto-mode) */
    AudioCodec get_selected_codec() const;

    /*
     * Reconnect to the previously connected device.
     * Uses stored remote address, codec, and config from the last connection.
     * Performs full A2DP reconnect + codec config + stream start.
     * Returns true if streaming was re-established.
     */
    bool reconnect();

    /*
     * Check if a disconnect has occurred since last check.
     * Returns true once per disconnect event (auto-resets).
     */
    bool check_disconnected();

    /* Codec support flags discovered from remote device */
    struct RemoteCodecCaps {
        bool ldac = false;
        bool aptx_hd = false;
        bool aptx_ll = false;
        bool aptx = false;          /* classic aptX (0x4F / 0x0001) */
        bool sbc = false;
        bool aac = false;
        /* Remote SEIDs for each codec */
        uint8_t ldac_seid = 0;
        uint8_t aptxhd_seid = 0;
        uint8_t aptxll_seid = 0;
        uint8_t aptx_seid = 0;
        uint8_t sbc_seid = 0;
        uint8_t aac_seid = 0;
        /* Remote aptX-family capability byte 6 (freq bits high nibble:
         * 0x20=44.1k 0x10=48k; channel bits low: 0x02=stereo 0x01=mono).
         * 0 = not reported (assume 44.1k+48k). */
        uint8_t aptx_caps = 0;
        uint8_t aptxhd_caps = 0;
        uint8_t aptxll_caps = 0;
        /* aptX LL: vendor ID the remote used (0x0A or 0xD7) and its raw
         * codec info (8 bytes, or 17 with has_new_caps) */
        uint32_t aptxll_vendor_id = 0;
        uint8_t aptxll_info[17] = {};
        uint8_t aptxll_info_len = 0;
        /* aptX Adaptive (0xD7 / 0x00AD): detected for logging only — there
         * is no open-source encoder */
        bool aptx_adaptive = false;
        uint8_t aptx_adaptive_seid = 0;
    };

    /* Get discovered remote capabilities (valid after connect_a2dp) */
    const RemoteCodecCaps &get_remote_caps() const { return remote_caps_; }

    /*
     * For aptX / aptX HD / aptX LL: choose a capture sample rate (44100 or
     * 48000) that the remote advertises, preferring `wanted`. Returns
     * `wanted` unchanged for other codecs or when no better choice exists.
     */
    uint32_t pick_aptx_sample_rate(AudioCodec codec, uint32_t wanted) const;

private:
    /* BTstack event handler (static, dispatches to instance) */
    static void packet_handler_trampoline(uint8_t packet_type, uint16_t channel,
                                          uint8_t *packet, uint16_t size);
    void handle_packet(uint8_t packet_type, uint16_t channel,
                       uint8_t *packet, uint16_t size);

    /* A2DP-specific event handlers */
    void handle_a2dp_event(uint8_t *packet, uint16_t size);
    void handle_avrcp_event(uint8_t *packet, uint16_t size);
    void handle_hci_event(uint8_t *packet, uint16_t size);

    /* BTstack run loop thread */
    static unsigned long __stdcall btstack_thread_proc(void *param);

    /* hci_init() through hci_power_control(ON). Runs on the BTstack thread,
     * directly or once a pre-HCI firmware loader (Intel) has finished. */
    void setup_hci_and_power_on();

    /* Intel bootloader firmware download finished (BTstack thread) */
    static void intel_firmware_done(int result);

    /* On HCI Read Local Version complete during init: log the controller and,
     * for Broadcom, register the chipset driver with a PatchRAM (.hcd) file. */
    void on_local_version(const uint8_t *packet);

    /* Pick a Broadcom .hcd from the firmware folder (empty if none fits) */
    std::string find_bcm_hcd() const;

    /* Register codec stream endpoints with BTstack */
    void register_codec_endpoints();

    /* Build vendor codec capability blobs */
    void build_ldac_capabilities(uint8_t *caps, uint16_t *len,
                                 uint8_t *config, uint16_t *config_len);
    void build_aptxhd_capabilities(uint8_t *caps, uint16_t *len,
                                   uint8_t *config, uint16_t *config_len);
    void build_aptxll_capabilities(uint8_t *caps, uint16_t *len,
                                   uint8_t *config, uint16_t *config_len);

    /* Signal a waiting synchronous call */
    void signal_event(void *event_handle, bool success);
    bool wait_for_event(void *event_handle, uint32_t timeout_ms);

    /* Tear down a half-open AVDTP connection after a failed/timed-out connect */
    void abort_pending_connection();
    /* LinkLost -> Refused when the device ended the link itself */
    void classify_link_lost();

    /* Thread handle */
    void *thread_handle_ = nullptr;

    /* Windows Event objects for async→sync bridge */
    void *init_event_ = nullptr;
    void *connect_event_ = nullptr;
    void *stream_event_ = nullptr;
    void *start_event_ = nullptr;
    void *disconnect_event_ = nullptr;
    void *acl_down_event_ = nullptr;     /* auto-reset; HCI Disconnection Complete */
    void *inquiry_event_ = nullptr;
    void *cancel_event_ = nullptr;       /* manual-reset; signaled to abort blocking waits */

    /* Result flags for sync operations */
    std::atomic<bool> init_result_{false};
    std::atomic<bool> connect_result_{false};
    /* ACL handle the A2DP signaling runs on (0xFFFF = none). Disconnections
     * of other links, e.g. a device used before that drops its idle link a
     * moment later, must not end this stream. */
    std::atomic<uint16_t> a2dp_con_handle_{0xFFFF};
    std::atomic<ConnectFailure> last_connect_failure_{ConnectFailure::None};
    /* ACL link to the device being connected (0xFFFF = none) and the reason
     * it went down (0 = still up / none): tells a device that ends the link
     * itself from a link lost on the radio */
    std::atomic<uint16_t> target_acl_handle_{0xFFFF};
    std::atomic<uint8_t> target_acl_down_reason_{0};

    /* AFH classification and radio readings, on the BTstack thread */
    void afh_start();
    void afh_stop();
    void afh_tick();
    void on_radio_command_complete(uint8_t *packet, uint16_t size);
    void *afh_timer_ = nullptr;          /* btstack_timer_source_t */
    uint8_t afh_sent_map_[10] = {};
    bool afh_sent_ = false;              /* a classification is in force at the controller */
    int afh_policy_version_seen_ = -1;
    int afh_ticks_ = 0;
    std::atomic<bool> stream_result_{false};
    std::atomic<bool> start_result_{false};

    /* BTstack state */
    uint16_t a2dp_cid_ = 0;        /* A2DP connection ID */
    uint16_t avrcp_cid_ = 0;       /* AVRCP connection ID (volume control) */
    uint8_t local_seid_ = 0;       /* Currently active local SEID */
    uint8_t remote_seid_ = 0;      /* Currently active remote SEID */
    uint16_t media_mtu_ = 0;
    std::atomic<uint16_t> media_payload_limit_{MEDIA_PAYLOAD_LIMIT_DEFAULT};

    /* Stream endpoint SEIDs */
    uint8_t ldac_local_seid_ = 0;
    uint8_t aptxhd_local_seid_ = 0;
    uint8_t aptxll_local_seid_ = 0;
    uint8_t aptx_local_seid_ = 0;
    uint8_t sbc_local_seid_ = 0;
    uint8_t aac_local_seid_ = 0;

    /* Stream endpoint pointers (owned by BTstack) */
    avdtp_stream_endpoint *ldac_ep_ = nullptr;
    avdtp_stream_endpoint *aptxhd_ep_ = nullptr;
    avdtp_stream_endpoint *aptxll_ep_ = nullptr;
    avdtp_stream_endpoint *aptx_ep_ = nullptr;
    avdtp_stream_endpoint *sbc_ep_ = nullptr;
    avdtp_stream_endpoint *aac_ep_ = nullptr;

    /* Remote capabilities discovered during connection */
    RemoteCodecCaps remote_caps_;

    /* Selected codec and config */
    AudioCodec selected_codec_ = AudioCodec::LDAC;
    uint32_t sample_rate_ = 48000;
    uint8_t channels_ = 2;

    /* Stored remote address for reconnection (big-endian / BTstack format) */
    uint8_t remote_addr_be_[6] = {};
    bool has_remote_addr_ = false;

    /* Configuration */
    bool hci_dump_enabled_ = true;
    bool hci_capture_enabled_ = false; /* debug mode: hci_capture installed as HCI dump */
    std::string hci_tcp_;           /* non-empty: H4 over TCP instead of WinUSB */
    std::string link_key_dir_;      /* directory for persistent link keys (empty = memory-only) */
    std::string firmware_dir_;      /* directory for firmware files (empty = exe dir fallback) */
    std::string fw_stem_;           /* firmware stem for unknown chips (e.g. "rtl8761bu") */
    uint16_t product_id_ = 0;      /* Realtek USB Product ID (0 = non-Realtek, skip chipset init) */

    /* Resolved firmware folder path (must outlive BTstack: chipset_init() is
     * called repeatedly, including from hci_power_control_on()). */
    std::string resolved_fw_dir_;

    /* Non-Realtek controller support (experimental). usb_adapters_ is the
     * WinUSB adapter list taken at thread start (empty for TCP / Realtek). */
    std::vector<BtAdapterInfo> usb_adapters_;
    bool intel_loader_ = false;     /* ran the Intel bootloader firmware download */
    bool bcm_checked_ = false;      /* Broadcom PatchRAM decided for this init */
    std::string bcm_hcd_path_;      /* must outlive BTstack (chipset keeps the pointer) */
    std::atomic<bool> init_failed_{false};  /* firmware loader gave up: stop waiting */

    /* Init timing (for firmware loading detection) */
    uint32_t init_start_tick_ = 0;

    /* Connection state */
    std::atomic<bool> running_{false};
    std::atomic<bool> connected_{false};
    std::atomic<bool> streaming_{false};
    std::atomic<bool> hci_ready_{false};

    /* Disconnect notification flag (set by event handler, cleared by check_disconnected) */
    std::atomic<bool> disconnect_occurred_{false};

    /* Inquiry state */
    std::atomic<bool> inquiry_active_{false};
    std::atomic<uint32_t> inquiry_found_count_{0};
    std::vector<DiscoveredDevice> discovered_devices_;

    /* Media send failure tracking (for ABR) */
    std::atomic<uint32_t> send_failure_count_{0};
    std::atomic<bool> oversize_logged_{false};  /* oversized payload reported once */

    /* Media packet queue: WASAPI thread writes, BTstack thread reads.
     * Ring buffer avoids dropping frames when multiple LDAC frames are
     * produced per WASAPI callback (e.g. 4 frames/10ms at 990kbps). */
    struct MediaPacket {
        uint8_t  data[MAX_MEDIA_PACKET_SIZE];
        uint32_t size = 0;
        uint32_t timestamp = 0;
        uint8_t  frames = 0;
        bool     no_rtp = false;   /* send without RTP header (aptX, aptX LL) */
    };
    static const int MEDIA_QUEUE_CAPACITY = static_cast<int>(MEDIA_QUEUE_PACKETS);
    MediaPacket media_queue_[MEDIA_QUEUE_CAPACITY];
    int media_queue_head_ = 0;   /* write position (WASAPI thread) */
    int media_queue_tail_ = 0;   /* read position (BTstack thread) */
    std::atomic<int> media_queue_count_{0};
    std::mutex send_mutex_;

    /* Trigger CAN_SEND_NOW from WASAPI thread without busy-looping.
     * Stored as void* to avoid exposing btstack_context_callback_registration_t. */
    void *media_trigger_reg_ = nullptr;
    std::atomic<bool> media_trigger_pending_{false};

    /* Singleton pointer for static callbacks */
    static BtStackTransport *instance_;
};

#endif /* BTSTACK_TRANSPORT_H */
