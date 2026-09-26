/*
 * BTstack Transport Layer Implementation
 *
 * User-mode Bluetooth transport using BTstack + WinUSB.
 *
 * SPDX-License-Identifier: MIT
 */

#include "btstack_transport.h"

#include <cstdio>
#include <cstring>
#include <windows.h>

/* BTstack headers (C) — must come before our avdtp.h to avoid include guard collision */
extern "C" {
#include "btstack_config.h"
#include "btstack.h"
#include "btstack_run_loop_windows.h"
#include "hci_transport_usb.h"
#include "l2cap.h"
#include "classic/a2dp_source.h"
#include "classic/avdtp_source.h"
#include "classic/avrcp.h"
#include "classic/avrcp_controller.h"
#include "classic/avrcp_target.h"
#include "classic/sdp_server.h"
#include "classic/btstack_link_key_db_memory.h"
#include "btstack_link_key_db_file.h"
#include "hci_dump.h"
#include "hci_dump_windows_stdout.h"
#include "hci_transport_h4.h"
#include "btstack_chipset_realtek.h"
#include "btstack_chipset_bcm.h"
#include "btstack_chipset_intel_firmware.h"
}

#include "afh.h"
#include "bt_adapter_enum.h"
#include "hci_capture.h"
#include "link_stats.h"
#include "btstack_uart_tcp_windows.h"

/*
 * Vendor codec IDs (duplicated from avdtp.h to avoid enum conflicts
 * with BTstack's avdtp.h which defines the same AVDTP enumerator names)
 */
#define LDAC_VENDOR_ID      0x0000012Du  /* Sony Corporation */
#define LDAC_CODEC_ID       0x00AAu      /* LDAC */
#define APTXHD_VENDOR_ID    0x000000D7u  /* Qualcomm Technologies International, Ltd */
#define APTXHD_CODEC_ID     0x0024u      /* aptX HD */
#define APTXLL_VENDOR_ID    0x0000000Au  /* CSR plc (now Qualcomm) */
#define APTXLL_VENDOR_ID2   0x000000D7u  /* Qualcomm: some sinks list aptX LL here */
#define APTXLL_CODEC_ID     0x0002u      /* aptX Low Latency */
#define APTX_VENDOR_ID      0x0000004Fu  /* APT Ltd. (now Qualcomm) */
#define APTX_CODEC_ID       0x0001u      /* aptX (classic) */
/* aptX Adaptive: no open-source encoder; detected for logging only */
#define APTXAD_VENDOR_ID    0x000000D7u  /* Qualcomm Technologies International, Ltd */
#define APTXAD_CODEC_ID     0x00ADu      /* aptX Adaptive */

/* Singleton for static callback dispatch */
BtStackTransport *BtStackTransport::instance_ = nullptr;

/* HCI event callback registration (must be static/persistent) */
static btstack_packet_callback_registration_t hci_event_callback_registration;

/* SDP Service Buffers */
static uint8_t sdp_a2dp_source_service_buffer[150];
static uint8_t sdp_avrcp_controller_service_buffer[200];
static uint8_t sdp_avrcp_target_service_buffer[200];

/*
 * Cross-thread dispatch helper.
 * BTstack is NOT thread-safe.  All BTstack API calls must execute on
 * the BTstack run-loop thread.  We use btstack_run_loop_execute_on_main_thread()
 * to queue a callback, then wait on a Windows Event for completion.
 */
struct RunLoopRequest {
    btstack_context_callback_registration_t reg;
    HANDLE done_event;
    /* Per-request arguments (union-style, kept simple) */
    uint8_t  u8_result;
    int      int_result;
    /* Inquiry */
    uint8_t  inquiry_duration;
    /* Connect */
    bd_addr_t addr;
    uint16_t *cid_out;
    /* Configure */
    uint16_t config_a2dp_cid;
    uint8_t  config_local_seid;
    uint8_t  config_remote_seid;
    uint8_t  config_info[32];    /* vendor codec info; aptX LL ext config = 17 */
    uint8_t  config_info_len;
    AudioCodec config_codec;
    avdtp_configuration_sbc_t sbc_config;
    avdtp_configuration_mpeg_aac_t aac_config;
    /* Start stream */
    uint16_t start_a2dp_cid;
    uint8_t  start_local_seid;
};

/* Timeout constants */
static const uint32_t INIT_TIMEOUT_MS    = 30000;
static const uint32_t CONNECT_TIMEOUT_MS = 30000;  /* Must exceed HCI page timeout (~20s) */
static const uint32_t STREAM_TIMEOUT_MS  = 10000;
static const uint32_t DISCONNECT_TIMEOUT_MS = 5000;

/* HCI error code to string (common codes) */
static const char *hci_error_string(uint8_t status) {
    switch (status) {
    case 0x00: return "Success";
    case 0x01: return "Unknown HCI Command";
    case 0x02: return "Unknown Connection Identifier";
    case 0x04: return "Page Timeout";
    case 0x05: return "Authentication Failure";
    case 0x06: return "PIN or Key Missing";
    case 0x07: return "Memory Capacity Exceeded";
    case 0x08: return "Connection Timeout";
    case 0x09: return "Connection Limit Exceeded";
    case 0x0B: return "ACL Connection Already Exists";
    case 0x0C: return "Command Disallowed";
    case 0x0D: return "Connection Rejected (Limited Resources)";
    case 0x0E: return "Connection Rejected (Security)";
    case 0x0F: return "Connection Rejected (Unacceptable BD_ADDR)";
    case 0x10: return "Connection Accept Timeout Exceeded";
    case 0x11: return "Unsupported Feature or Parameter Value";
    case 0x12: return "Invalid HCI Command Parameters";
    case 0x13: return "Remote User Terminated Connection";
    case 0x22: return "LMP Response Timeout / LL Response Timeout";
    default:   return "Unknown";
    }
}

/*
 * Vendor codec capability blob format for AVDTP:
 *   [media_type(1)] [codec_type(1)=0xFF] [vendor_id(4 LE)] [codec_id(2 LE)] [config_bytes...]
 *
 * For registration with a2dp_source_create_stream_endpoint, we provide
 * the "media codec information" portion (after media_type and codec_type),
 * which is: [vendor_id(4)] [codec_id(2)] [codec_specific_caps...]
 */

/* LDAC capability bytes (after vendor ID + codec ID): 2 separate bytes.
 * Byte 6: Sampling Frequency — 0x20=44.1k, 0x10=48k, 0x08=88.2k, 0x04=96k
 * Byte 7: Channel Mode — 0x04=mono, 0x02=dual, 0x01=stereo */
static const uint8_t LDAC_FREQ_ALL = 0x3C;          /* 44.1k|48k|88.2k|96k */
static const uint8_t LDAC_CH_ALL   = 0x07;           /* mono|dual|stereo */
static const uint8_t LDAC_FREQ_48K = 0x10;
static const uint8_t LDAC_CH_STEREO = 0x01;

/* Classic aptX: codec info = vendor(4 LE) + codec(2 LE) + 1 byte (7 bytes total).
 * High nibble = sampling freq: 0x20=44.1kHz, 0x10=48kHz (0x80=16k, 0x40=32k)
 * Low nibble  = channel mode:  0x02=stereo, 0x01=mono
 * Refs: Android a2dp_vendor_aptx_constants.h (A2DP_APTX_CODEC_LEN=9 incl.
 * LOSC-counted media/codec type bytes), PipeWire a2dp-codec-caps.h a2dp_aptx_t.
 * Only stereo is advertised/configured (as Android and PipeWire do).
 * The same freq/channel byte is used by aptX HD and aptX LL (byte 6). */
static const uint8_t APTX_FREQ_44100   = 0x20;
static const uint8_t APTX_FREQ_48000   = 0x10;
static const uint8_t APTX_FREQ_MASK    = 0xF0;
static const uint8_t APTX_CH_STEREO    = 0x02;
static const uint8_t APTX_CAPS_ALL     = APTX_FREQ_44100 | APTX_FREQ_48000 | APTX_CH_STEREO;
static const uint8_t APTX_CONFIG_DEFAULT = APTX_FREQ_48000 | APTX_CH_STEREO;

/* aptX HD: vendor(4) + codec(2) + freq/channel byte (as classic aptX) +
 * 4 reserved bytes (Android A2DP_APTX_HD_ACL_SPRINT_RESERVED0..3, PipeWire
 * a2dp_aptx_hd_t.rfa) = 11 bytes. Android A2DP_APTX_HD_CODEC_LEN=13 counts
 * LOSC + media type + codec type. Sent WITH an RTP header. */
static const uint16_t APTXHD_INFO_LEN = 11;
static const uint8_t APTXHD_CAPS_ALL = APTX_CAPS_ALL;             /* 44.1+48, stereo */
static const uint8_t APTXHD_CONFIG_DEFAULT = APTX_CONFIG_DEFAULT; /* 48kHz stereo */

/* aptX LL (PipeWire a2dp_aptx_ll_t): classic aptX 7 bytes + 1 byte
 * (bit0 = bidirect_link, bit1 = has_new_caps) = 8 bytes. With has_new_caps
 * the sink appends 9 bytes (a2dp_aptx_ll_ext_t: reserved, target_level LE16,
 * initial_level LE16, sra_max_rate, sra_avg_time, good_working_level LE16)
 * = 17 bytes. Sent WITHOUT an RTP header (PipeWire adds RTP only for HD). */
static const uint16_t APTXLL_INFO_LEN = 8;
static const uint16_t APTXLL_EXT_INFO_LEN = 17;
static const uint8_t APTXLL_BIDIRECT_LINK = 0x01;
static const uint8_t APTXLL_HAS_NEW_CAPS = 0x02;
static const uint8_t APTXLL_CAPS_ALL = APTX_CAPS_ALL;
static const uint8_t APTXLL_CONFIG_DEFAULT = APTX_CONFIG_DEFAULT;
/* PipeWire defaults for the extended LL config (a2dp-codec-caps.h), which
 * PipeWire also bumps by 3/2 (LL_LEVEL_ADJUSTMENT) for stability. */
static const uint16_t APTXLL_TARGET_LEVEL  = 180 * 3 / 2;
static const uint16_t APTXLL_INITIAL_LEVEL = 360 * 3 / 2;
static const uint16_t APTXLL_GOOD_LEVEL    = 180 * 3 / 2;
static const uint8_t  APTXLL_SRA_MAX_RATE  = 50;  /* x/10000 */
static const uint8_t  APTXLL_SRA_AVG_TIME  = 1;   /* seconds */

/* Persistent SET_CONFIGURATION codec info. a2dp_source_set_config_other()
 * stores only the pointer (remote_configuration.media_codec_information) and
 * BTstack reads it later, so it must outlive configure_codec(). Written only
 * on the BTstack thread. */
static uint8_t s_other_config_info[32];

/* Helper: write vendor ID (4 bytes LE) + codec ID (2 bytes LE) into buffer */
static void write_vendor_codec_id(uint8_t *buf, uint32_t vendor_id, uint16_t codec_id) {
    buf[0] = (uint8_t)(vendor_id & 0xFF);
    buf[1] = (uint8_t)((vendor_id >> 8) & 0xFF);
    buf[2] = (uint8_t)((vendor_id >> 16) & 0xFF);
    buf[3] = (uint8_t)((vendor_id >> 24) & 0xFF);
    buf[4] = (uint8_t)(codec_id & 0xFF);
    buf[5] = (uint8_t)((codec_id >> 8) & 0xFF);
}

/* Helper: extract vendor ID from codec info blob */
static uint32_t read_vendor_id(const uint8_t *info) {
    return (uint32_t)info[0] | ((uint32_t)info[1] << 8) |
           ((uint32_t)info[2] << 16) | ((uint32_t)info[3] << 24);
}

static uint16_t read_codec_id(const uint8_t *info) {
    return (uint16_t)info[4] | ((uint16_t)info[5] << 8);
}

static const char *aptx_family_name(AudioCodec codec) {
    switch (codec) {
    case AudioCodec::AptxHD: return "aptX HD";
    case AudioCodec::AptxLL: return "aptX LL";
    default:                 return "aptX";
    }
}

/* Supported-rate check shared by aptX / aptX HD / aptX LL. remote_caps is
 * the remote's freq/channel byte (0 = unknown -> assume 44.1k and 48k).
 * Returns the config frequency bit, or 0 (with a log) if unusable. */
static uint8_t aptx_freq_bits_for(AudioCodec codec, uint8_t remote_caps, uint32_t sample_rate) {
    uint8_t freq = (sample_rate == 44100) ? APTX_FREQ_44100 :
                   (sample_rate == 48000) ? APTX_FREQ_48000 : 0;
    if (freq == 0) {
        fprintf(stderr, "BTstack: %s supports only 44100/48000 Hz capture (got %u Hz)\n",
                aptx_family_name(codec), sample_rate);
        return 0;
    }
    uint8_t remote_freqs = remote_caps & APTX_FREQ_MASK;
    if (remote_freqs != 0 && !(remote_freqs & freq)) {
        fprintf(stderr, "BTstack: Remote %s caps 0x%02x lack %u Hz (remote supports:%s%s)\n",
                aptx_family_name(codec), remote_caps, sample_rate,
                (remote_freqs & APTX_FREQ_44100) ? " 44100" : "",
                (remote_freqs & APTX_FREQ_48000) ? " 48000" : "");
        return 0;
    }
    return freq;
}

/* ======================================================================== */
/* Construction / Destruction                                               */
/* ======================================================================== */

BtStackTransport::BtStackTransport() {
    init_event_ = CreateEventA(nullptr, FALSE, FALSE, nullptr);
    connect_event_ = CreateEventA(nullptr, FALSE, FALSE, nullptr);
    stream_event_ = CreateEventA(nullptr, FALSE, FALSE, nullptr);
    start_event_ = CreateEventA(nullptr, FALSE, FALSE, nullptr);
    disconnect_event_ = CreateEventA(nullptr, FALSE, FALSE, nullptr);
    acl_down_event_ = CreateEventA(nullptr, FALSE, FALSE, nullptr);
    inquiry_event_ = CreateEventA(nullptr, FALSE, FALSE, nullptr);
    cancel_event_ = CreateEventA(nullptr, TRUE, FALSE, nullptr);  /* manual-reset */
    auto *reg = new btstack_context_callback_registration_t();
    memset(reg, 0, sizeof(*reg));
    media_trigger_reg_ = reg;
}

BtStackTransport::~BtStackTransport() {
    shutdown();
    CloseHandle(init_event_);
    CloseHandle(connect_event_);
    CloseHandle(stream_event_);
    CloseHandle(start_event_);
    CloseHandle(disconnect_event_);
    CloseHandle(acl_down_event_);
    CloseHandle(inquiry_event_);
    CloseHandle(cancel_event_);
    delete static_cast<btstack_context_callback_registration_t *>(media_trigger_reg_);
}

/* ======================================================================== */
/* Initialization                                                           */
/* ======================================================================== */

bool BtStackTransport::init(const char *usb_path) {
    (void)usb_path;  /* TODO: support specific USB device selection */

    if (instance_ != nullptr) {
        fprintf(stderr, "BTstack: Only one BtStackTransport instance is supported\n");
        return false;
    }
    instance_ = this;
    running_.store(true);
    init_failed_.store(false);

    /* Launch BTstack run loop in a dedicated thread */
    thread_handle_ = CreateThread(nullptr, 0,
        (LPTHREAD_START_ROUTINE)btstack_thread_proc, this, 0, nullptr);
    if (!thread_handle_) {
        fprintf(stderr, "BTstack: Failed to create run loop thread\n");
        instance_ = nullptr;
        return false;
    }

    /* Wait for HCI to power on (poll-based to avoid event conflicts with BTstack run loop) */
    {
        uint32_t elapsed = 0;
        const uint32_t poll_interval = 100;
        while (elapsed < INIT_TIMEOUT_MS) {
            if (init_result_.load()) {
                fprintf(stderr, "BTstack: HCI ready after %lu ms\n", (unsigned long)elapsed);
                fflush(stderr);
                return true;
            }
            if (init_failed_.load()) break;
            Sleep(poll_interval);
            elapsed += poll_interval;
        }
    }

    fprintf(stderr, init_failed_.load() ? "BTstack: HCI initialization failed\n"
                                        : "BTstack: HCI initialization timed out\n");
    shutdown();
    return false;
}

unsigned long __stdcall BtStackTransport::btstack_thread_proc(void *param) {
    auto *self = static_cast<BtStackTransport *>(param);

    /* Initialize BTstack memory pools (must be first!) */
    btstack_memory_init();

    /* Initialize BTstack run loop */
    btstack_run_loop_init(btstack_run_loop_windows_get_instance());

    /* HCI dump: on-demand capture (debug mode) takes priority over stdout dump */
    if (self->hci_capture_enabled_) {
        hci_capture::reset_tracking();
        hci_dump_init(hci_capture::instance());
    } else if (self->hci_dump_enabled_) {
        hci_dump_init(hci_dump_windows_stdout_get_instance());
    }

    fprintf(stderr, "BTstack: thread started, initializing HCI...\n");
    fflush(stderr);

    /* Auto-register all Realtek USB Bluetooth VID/PIDs from BTstack's firmware table.
     * This covers RTL8761B, RTL8822CU, and many third-party adapters. */
    {
        uint16_t num_rtk = btstack_chipset_realtek_get_num_usb_controllers();
        fprintf(stderr, "BTstack: Registering %u Realtek USB device(s)\n", num_rtk);
        for (uint16_t i = 0; i < num_rtk; i++) {
            uint16_t vid, pid;
            btstack_chipset_realtek_get_vendor_product_id(i, &vid, &pid);
            hci_transport_usb_add_device(vid, pid);
        }
    }
    /* Register OEM adapters (TP-Link, ASUS, etc.) that use Realtek chips
     * but have a different USB VID. See oem_table in bt_adapter_enum.cpp. */
    {
        const OemChipMapping *m = BtAdapterEnumerator::get_oem_table();
        for (; m->vid != 0; m++) {
            hci_transport_usb_add_device(m->vid, m->pid);
            fprintf(stderr, "BTstack: Registered OEM device %04X:%04X\n", m->vid, m->pid);
        }
    }
    /* Non-Realtek adapters */
    hci_transport_usb_add_device(0x0A12, 0x0001);  /* CSR (Cambridge Silicon Radio) */
    hci_transport_usb_add_device(0x8087, 0x0029);  /* Intel AX200/AX201 */
    hci_transport_usb_add_device(0x8087, 0x0032);  /* Intel AX210 */

    /* Resolve firmware directory: explicit setting → exe dir fallback.
     * Stored in a member so the c_str() handed to BTstack survives
     * subsequent chipset_init() calls (hci_power_control_on() invokes
     * chipset_init again after hci_set_chipset). */
    self->resolved_fw_dir_.clear();
    if (!self->firmware_dir_.empty()) {
        self->resolved_fw_dir_ = self->firmware_dir_;
    } else {
        char exe_dir[MAX_PATH];
        if (GetModuleFileNameA(NULL, exe_dir, MAX_PATH)) {
            char *last_sep = strrchr(exe_dir, '\\');
            if (!last_sep) last_sep = strrchr(exe_dir, '/');
            if (last_sep) *last_sep = '\0';
            self->resolved_fw_dir_ = exe_dir;
        }
    }

    /* Non-Realtek controllers (experimental): see which WinUSB adapters are
     * present. Only an adapter bound to WinUSB passes the class check, so an
     * Intel adapter still on the Intel driver does not count. */
    self->usb_adapters_.clear();
    self->intel_loader_ = false;
    self->bcm_checked_ = false;
    self->bcm_hcd_path_.clear();
    if (self->hci_tcp_.empty() && self->product_id_ == 0) {
        self->usb_adapters_ = BtAdapterEnumerator::enumerate();
        for (const auto &a : self->usb_adapters_) {
            if (a.vendor == BtChipVendor::Intel) self->intel_loader_ = true;
            if (a.vendor == BtChipVendor::MediaTek || a.vendor == BtChipVendor::Qualcomm) {
                fprintf(stderr, "BTstack: WARNING %s adapter %04X:%04X — firmware loading "
                        "for this vendor is not implemented; it may not work\n",
                        BtAdapterEnumerator::vendor_name(a.vendor), a.vid, a.pid);
            }
        }
    }

    if (self->intel_loader_) {
        /* Intel controllers boot into a bootloader and need ibt-*.sfi/.ddc
         * uploaded before regular HCI init. The loader sends HCI Reset first
         * and finishes at once if the controller is already operational
         * (e.g. the Intel driver loaded it before the switch to WinUSB). */
        const std::string &fw_dir = self->resolved_fw_dir_;
        btstack_chipset_intel_set_firmware_path(fw_dir.empty() ? "." : fw_dir.c_str());
        fprintf(stderr, "BTstack: Intel adapter — checking bootloader firmware "
                "(experimental), folder %s\n", fw_dir.c_str());
        fflush(stderr);
        btstack_chipset_intel_download_firmware(hci_transport_usb_instance(),
                                                &BtStackTransport::intel_firmware_done);
    } else {
        self->setup_hci_and_power_on();
    }

    /* Run the event loop (blocks until shutdown) */
    btstack_run_loop_execute();

    /* Run loop has exited. Do NOT call hci_power_control/hci_close here —
     * they need the run loop to process HCI commands and will block.
     * Global cleanup (deinit) is handled by shutdown() after this thread exits. */
    fprintf(stderr, "BTstack: run loop exited, thread finishing\n");
    fflush(stderr);

    return 0;
}

void BtStackTransport::setup_hci_and_power_on() {
    if (!hci_tcp_.empty()) {
        /* Development / testing: H4 over TCP to a virtual controller (tools/emu) */
        static hci_transport_config_uart_t tcp_config = {
            HCI_TRANSPORT_CONFIG_UART, 115200, 0, 0, nullptr, BTSTACK_UART_PARITY_OFF};
        tcp_config.device_name = hci_tcp_.c_str();
        fprintf(stderr, "BTstack: H4 over TCP -> %s\n", tcp_config.device_name);
        hci_init(hci_transport_h4_instance_for_uart(btstack_uart_tcp_windows_instance()), &tcp_config);
    } else {
        /* Initialize HCI with WinUSB transport */
        hci_init(hci_transport_usb_instance(), nullptr);
    }

    /* Realtek chipset initialization — only when a Realtek PID is configured.
     * set_product_id() must be called before init() with the detected PID.
     * For non-Realtek adapters (PID==0), skip chipset-specific init. */
    if (product_id_ != 0) {
        fprintf(stderr, "BTstack: Realtek adapter PID=0x%04X\n", product_id_);
        btstack_chipset_realtek_set_product_id(product_id_);

        /* BTstack's chipset_init() unconditionally rebuilds firmware/config paths
         * as "${folder}/${patch_name}" when product_id is set, ignoring any
         * set_firmware_file_path() we call. patch_name has NO .bin extension
         * (e.g. "rtl8761bu_fw"), but our distribution and linux-firmware use
         * .bin. Use folder-based lookup and materialise no-extension aliases
         * (hard-link, falling back to copy) from the .bin files we ship. */
        const std::string &fw_dir = resolved_fw_dir_;
        if (!fw_dir.empty()) {
            btstack_chipset_realtek_set_firmware_folder_path(fw_dir.c_str());
            btstack_chipset_realtek_set_config_folder_path(fw_dir.c_str());
            fprintf(stderr, "BTstack: Firmware search path: %s\n", fw_dir.c_str());

            const char *fw_name = BtAdapterEnumerator::realtek_fw_name(product_id_);
            const char *cfg_name = BtAdapterEnumerator::realtek_cfg_name(product_id_);
            std::string fw_stem, cfg_stem;
            if (fw_name && cfg_name) {
                fw_stem = fw_name;
                cfg_stem = cfg_name;
            } else if (!fw_stem_.empty()) {
                fw_stem = fw_stem_ + "_fw";
                cfg_stem = fw_stem_ + "_config";
            }

            auto ensure_alias = [&fw_dir](const std::string &stem) {
                if (stem.empty()) return;
                std::string alias = fw_dir + "\\" + stem;        /* no extension */
                std::string source = alias + ".bin";              /* shipped file */
                DWORD attrs = GetFileAttributesA(alias.c_str());
                if (attrs != INVALID_FILE_ATTRIBUTES) return;     /* alias already exists */
                if (GetFileAttributesA(source.c_str()) == INVALID_FILE_ATTRIBUTES) {
                    fprintf(stderr, "BTstack: WARNING firmware source missing: %s\n", source.c_str());
                    return;
                }
                if (CreateHardLinkA(alias.c_str(), source.c_str(), NULL)) {
                    fprintf(stderr, "BTstack: linked %s -> %s\n", stem.c_str(), source.c_str());
                } else if (CopyFileA(source.c_str(), alias.c_str(), TRUE)) {
                    fprintf(stderr, "BTstack: copied %s <- %s\n", alias.c_str(), source.c_str());
                } else {
                    fprintf(stderr, "BTstack: ERROR could not create alias %s (err=%lu)\n",
                            alias.c_str(), GetLastError());
                }
            };
            ensure_alias(fw_stem);
            ensure_alias(cfg_stem);
        }

        /* Register the Realtek chipset driver. This calls chipset_init()
         * immediately, which uses the product ID and folder path set above. */
        hci_set_chipset(btstack_chipset_realtek_instance());
    } else {
        fprintf(stderr, "BTstack: No Realtek PID configured, skipping Realtek chipset init\n");
    }

    fprintf(stderr, "BTstack: hci_init done, powering on...\n");
    fflush(stderr);

    /* Link key storage: file-backed if path was set, otherwise memory-only */
    if (!link_key_dir_.empty()) {
        btstack_link_key_db_file_set_path(link_key_dir_.c_str());
        hci_set_link_key_db(btstack_link_key_db_file_instance());
    } else {
        hci_set_link_key_db(btstack_link_key_db_memory_instance());
    }

    /* SSP: Just Works (no display, no keyboard) */
    gap_ssp_set_io_capability(SSP_IO_CAPABILITY_NO_INPUT_NO_OUTPUT);
    /* General Bonding without MITM (BTstack default). 0 = No Bonding would never
     * store link keys, and some headsets ignore profile connections from
     * non-bonded peers (e.g. no reply to AVDTP Discover). */
    gap_ssp_set_authentication_requirement(SSP_IO_AUTHREQ_MITM_PROTECTION_NOT_REQUIRED_GENERAL_BONDING);

    gap_set_local_name("A2DPWB");

    /* Set device class: Audio (Major=0x04), Hi-Fi Audio Device (Minor=0x28) — A2DP Source */
    gap_set_class_of_device(0x200428);

    /* Allow role switch — many headphones require being master */
    gap_set_allow_role_switch(true);

    /* Enable role switch and sniff mode in link policy */
    gap_set_default_link_policy_settings(0x0005);

    /* Extend page timeout to ~20 seconds (0x8000 * 0.625ms = 20.48s) */
    gap_set_page_timeout(0x8000);

    /* Initialize L2CAP — MUST be called before any profile init.
     * Registers L2CAP's event handler with HCI so L2CAP receives
     * Connection Complete events and can process channel state machines. */
    l2cap_init();

    /* Initialize SDP server (needed for remote devices querying our services) */
    sdp_init();

    /* Register HCI event handler (for BTSTACK_EVENT_STATE, pairing events, etc.) */
    hci_event_callback_registration.callback = &packet_handler_trampoline;
    hci_add_event_handler(&hci_event_callback_registration);

    /* Initialize A2DP Source */
    a2dp_source_init();
    a2dp_source_register_packet_handler(&packet_handler_trampoline);

    /* Initialize AVRCP (volume control) */
    avrcp_init();
    avrcp_register_packet_handler(&packet_handler_trampoline);
    avrcp_controller_init();
    avrcp_controller_register_packet_handler(&packet_handler_trampoline);
    avrcp_target_init();
    avrcp_target_register_packet_handler(&packet_handler_trampoline);

    /* Setup A2DP Source SDP record */
    memset(sdp_a2dp_source_service_buffer, 0, sizeof(sdp_a2dp_source_service_buffer));
    a2dp_source_create_sdp_record(sdp_a2dp_source_service_buffer, sdp_create_service_record_handle(), AVDTP_SOURCE_FEATURE_MASK_PLAYER, NULL, NULL);
    sdp_register_service(sdp_a2dp_source_service_buffer);

    /* Setup AVRCP Controller SDP record */
    memset(sdp_avrcp_controller_service_buffer, 0, sizeof(sdp_avrcp_controller_service_buffer));
    /* Category 2 (Monitor/Amplifier): our CT sends absolute volume */
    uint16_t controller_supported_features = AVRCP_FEATURE_MASK_CATEGORY_MONITOR_OR_AMPLIFIER;
    avrcp_controller_create_sdp_record(sdp_avrcp_controller_service_buffer, sdp_create_service_record_handle(), controller_supported_features, NULL, NULL);
    sdp_register_service(sdp_avrcp_controller_service_buffer);

    /* Setup AVRCP Target SDP record */
    memset(sdp_avrcp_target_service_buffer, 0, sizeof(sdp_avrcp_target_service_buffer));
    uint16_t target_supported_features = AVRCP_FEATURE_MASK_CATEGORY_PLAYER_OR_RECORDER;
    avrcp_target_create_sdp_record(sdp_avrcp_target_service_buffer, sdp_create_service_record_handle(), target_supported_features, NULL, NULL);
    sdp_register_service(sdp_avrcp_target_service_buffer);

    /* Register vendor codec stream endpoints */
    register_codec_endpoints();

    /* Enable custom pre-init so Realtek chipset driver can send
     * vendor commands before HCI Reset (Phase 1: read LMP subversion).
     * Without this, Phase 1 runs during Phase 2's slot and the
     * firmware download (Phase 2) never executes. */
    if (product_id_ != 0) {
        hci_enable_custom_pre_init();
    }

    /* Power on HCI — record start time to measure firmware loading */
    init_start_tick_ = GetTickCount();
    hci_power_control(HCI_POWER_ON);
}

void BtStackTransport::intel_firmware_done(int result) {
    BtStackTransport *self = instance_;
    if (!self) return;
    if (result != 0) {
        fprintf(stderr, "BTstack: Intel firmware download failed (%d). Place the matching "
                "ibt-*.sfi and ibt-*.ddc (linux-firmware intel/) in %s. Newer TLV-mode "
                "controllers (AX210 and later) are not supported by BTstack yet.\n",
                result, self->resolved_fw_dir_.c_str());
        fflush(stderr);
        /* hci_init() never ran, so close the adapter here or the next
         * attempt cannot open it. */
        hci_transport_usb_instance()->close();
        self->init_failed_.store(true);
        return;
    }
    fprintf(stderr, "BTstack: Intel controller operational\n");
    fflush(stderr);
    self->setup_hci_and_power_on();
}

std::string BtStackTransport::find_bcm_hcd() const {
    const std::string &dir = resolved_fw_dir_;
    if (dir.empty()) return {};

    std::vector<std::string> files;
    WIN32_FIND_DATAA fd;
    HANDLE h = FindFirstFileA((dir + "\\*.hcd").c_str(), &fd);
    if (h == INVALID_HANDLE_VALUE) return {};
    do {
        if (!(fd.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY)) files.push_back(fd.cFileName);
    } while (FindNextFileA(h, &fd));
    FindClose(h);

    /* linux-firmware brcm/ naming: <chip>-<vid>-<pid>.hcd, e.g.
     * BCM20702A1-0b05-17cb.hcd. Prefer the one for a present adapter. */
    for (const auto &a : usb_adapters_) {
        if (a.vendor != BtChipVendor::Broadcom) continue;
        char tag[16];
        snprintf(tag, sizeof(tag), "-%04x-%04x.", a.vid, a.pid);
        for (const auto &f : files) {
            std::string lower = f;
            for (auto &c : lower) c = (char)tolower((unsigned char)c);
            if (lower.find(tag) != std::string::npos) return dir + "\\" + f;
        }
    }
    /* A lone .hcd is taken as meant for this adapter */
    if (files.size() == 1) return dir + "\\" + files[0];
    fprintf(stderr, "BTstack: %zu .hcd files but none named for the adapter's "
            "VID:PID — skipping PatchRAM\n", files.size());
    return {};
}

void BtStackTransport::on_local_version(const uint8_t *packet) {
    /* Command Complete: [3..4]=opcode [5]=status [6]=hci_ver [7..8]=hci_rev
     * [9]=lmp_ver [10..11]=manufacturer [12..13]=lmp_subver */
    if (packet[5] != 0) return;
    uint8_t  hci_ver = packet[6];
    uint16_t manufacturer = little_endian_read_16(packet, 10);
    uint16_t lmp_subver = little_endian_read_16(packet, 12);
    const char *company = BtAdapterEnumerator::company_name(manufacturer);
    fprintf(stderr, "BTstack: Controller %s (company 0x%04X), HCI version %u, "
            "LMP subversion 0x%04X\n",
            company ? company : "unknown", manufacturer, hci_ver, lmp_subver);

    switch (manufacturer) {
    case 0x0046:  /* MediaTek */
    case 0x001D:  /* Qualcomm */
    case 0x0045:  /* Atheros */
        fprintf(stderr, "BTstack: WARNING this vendor needs a firmware download that is "
                "not implemented; the controller runs its ROM firmware only\n");
        break;
    default:
        break;
    }
    fflush(stderr);

    /* Broadcom (hci.c maps Cypress/Infineon to it before we see the event).
     * The PatchRAM goes in during custom init, which follows this command;
     * after the upload BTstack resets without re-reading the version, so
     * this runs once per power-on. */
    if (bcm_checked_ || product_id_ != 0 || !hci_tcp_.empty()) return;
    if (manufacturer != BLUETOOTH_COMPANY_ID_BROADCOM_CORPORATION) return;
    if (hci_get_state() != HCI_STATE_INITIALIZING) return;
    bcm_checked_ = true;

    bcm_hcd_path_ = find_bcm_hcd();
    if (bcm_hcd_path_.empty()) {
        fprintf(stderr, "BTstack: Broadcom controller — no usable .hcd in %s, "
                "running ROM firmware\n", resolved_fw_dir_.c_str());
        return;
    }
    fprintf(stderr, "BTstack: Broadcom PatchRAM %s (experimental)\n", bcm_hcd_path_.c_str());
    fflush(stderr);
    btstack_chipset_bcm_set_hcd_file_path(bcm_hcd_path_.c_str());
    hci_set_chipset(btstack_chipset_bcm_instance());
}

void BtStackTransport::register_codec_endpoints() {
    /*
     * Register one AVDTP stream endpoint per codec.
     * Each endpoint has capabilities (what we support) and a default config.
     * BTstack matches these against remote device capabilities during connection.
     */

    /* LDAC endpoint — 8 bytes: vendor_id(4) + codec_id(2) + freq(1) + ch(1) */
    {
        static uint8_t ldac_caps[8];
        static uint8_t ldac_config[8];
        write_vendor_codec_id(ldac_caps, LDAC_VENDOR_ID, LDAC_CODEC_ID);
        ldac_caps[6] = LDAC_FREQ_ALL;
        ldac_caps[7] = LDAC_CH_ALL;
        write_vendor_codec_id(ldac_config, LDAC_VENDOR_ID, LDAC_CODEC_ID);
        ldac_config[6] = LDAC_FREQ_48K;
        ldac_config[7] = LDAC_CH_STEREO;

        ldac_ep_ = a2dp_source_create_stream_endpoint(
            AVDTP_AUDIO, AVDTP_CODEC_NON_A2DP,
            ldac_caps, sizeof(ldac_caps),
            ldac_config, sizeof(ldac_config)
        );
        if (ldac_ep_) {
            ldac_local_seid_ = avdtp_local_seid(ldac_ep_);
            fprintf(stderr, "BTstack: Registered LDAC endpoint (SEID=%u)\n", ldac_local_seid_);
        } else {
            fprintf(stderr, "BTstack: WARNING — failed to register LDAC endpoint\n");
        }
    }

    /* aptX HD endpoint */
    {
        static uint8_t aptxhd_caps[APTXHD_INFO_LEN];     /* bytes 7..10 reserved = 0 */
        static uint8_t aptxhd_config[APTXHD_INFO_LEN];
        write_vendor_codec_id(aptxhd_caps, APTXHD_VENDOR_ID, APTXHD_CODEC_ID);
        aptxhd_caps[6] = APTXHD_CAPS_ALL;
        write_vendor_codec_id(aptxhd_config, APTXHD_VENDOR_ID, APTXHD_CODEC_ID);
        aptxhd_config[6] = APTXHD_CONFIG_DEFAULT;

        aptxhd_ep_ = a2dp_source_create_stream_endpoint(
            AVDTP_AUDIO, AVDTP_CODEC_NON_A2DP,
            aptxhd_caps, sizeof(aptxhd_caps),
            aptxhd_config, sizeof(aptxhd_config)
        );
        if (aptxhd_ep_) {
            aptxhd_local_seid_ = avdtp_local_seid(aptxhd_ep_);
            fprintf(stderr, "BTstack: Registered aptX HD endpoint (SEID=%u)\n", aptxhd_local_seid_);
        } else {
            fprintf(stderr, "BTstack: WARNING — failed to register aptX HD endpoint\n");
        }
    }

    /* aptX (classic) endpoint */
    {
        static uint8_t aptx_caps[7];
        static uint8_t aptx_config[7];
        write_vendor_codec_id(aptx_caps, APTX_VENDOR_ID, APTX_CODEC_ID);
        aptx_caps[6] = APTX_CAPS_ALL;
        write_vendor_codec_id(aptx_config, APTX_VENDOR_ID, APTX_CODEC_ID);
        aptx_config[6] = APTX_CONFIG_DEFAULT;

        aptx_ep_ = a2dp_source_create_stream_endpoint(
            AVDTP_AUDIO, AVDTP_CODEC_NON_A2DP,
            aptx_caps, sizeof(aptx_caps),
            aptx_config, sizeof(aptx_config)
        );
        if (aptx_ep_) {
            aptx_local_seid_ = avdtp_local_seid(aptx_ep_);
            fprintf(stderr, "BTstack: Registered aptX endpoint (SEID=%u)\n", aptx_local_seid_);
        } else {
            fprintf(stderr, "BTstack: WARNING — failed to register aptX endpoint\n");
        }
    }

    /* aptX LL endpoint */
    {
        /* Byte 7 = 0: no bidirectional link, no new caps. One local SEP is
         * used for sinks listing aptX LL under either vendor ID; the
         * SET_CONFIGURATION carries the remote's vendor ID. */
        static uint8_t aptxll_caps[APTXLL_INFO_LEN];
        static uint8_t aptxll_config[APTXLL_INFO_LEN];
        write_vendor_codec_id(aptxll_caps, APTXLL_VENDOR_ID, APTXLL_CODEC_ID);
        aptxll_caps[6] = APTXLL_CAPS_ALL;
        write_vendor_codec_id(aptxll_config, APTXLL_VENDOR_ID, APTXLL_CODEC_ID);
        aptxll_config[6] = APTXLL_CONFIG_DEFAULT;

        aptxll_ep_ = a2dp_source_create_stream_endpoint(
            AVDTP_AUDIO, AVDTP_CODEC_NON_A2DP,
            aptxll_caps, sizeof(aptxll_caps),
            aptxll_config, sizeof(aptxll_config)
        );
        if (aptxll_ep_) {
            aptxll_local_seid_ = avdtp_local_seid(aptxll_ep_);
            fprintf(stderr, "BTstack: Registered aptX LL endpoint (SEID=%u)\n", aptxll_local_seid_);
        } else {
            fprintf(stderr, "BTstack: WARNING — failed to register aptX LL endpoint\n");
        }
    }

    /* SBC endpoint — standard A2DP codec (mandatory) */
    {
        /*
         * SBC capability: 4 bytes
         * Byte 0: sampling_freq (44.1k=0x20, 48k=0x10) | channel_mode (joint_stereo=0x01, stereo=0x02, dual=0x04, mono=0x08)
         * Byte 1: block_length (16=0x10, 12=0x20, 8=0x40, 4=0x80) | subbands (8=0x04, 4=0x08) | alloc_method (loudness=0x01, SNR=0x02)
         * Byte 2: min_bitpool
         * Byte 3: max_bitpool
         */
        static uint8_t sbc_caps[4]   = { 0x3F, 0x15, 2, 53 }; /* 44.1+48k, all ch modes, 16 blocks, 8 subbands, loudness */
        static uint8_t sbc_config[4] = { 0x11, 0x15, 2, 53 }; /* 48k, joint stereo */

        sbc_ep_ = a2dp_source_create_stream_endpoint(
            AVDTP_AUDIO, AVDTP_CODEC_SBC,
            sbc_caps, sizeof(sbc_caps),
            sbc_config, sizeof(sbc_config)
        );
        if (sbc_ep_) {
            sbc_local_seid_ = avdtp_local_seid(sbc_ep_);
            fprintf(stderr, "BTstack: Registered SBC endpoint (SEID=%u)\n", sbc_local_seid_);
        } else {
            fprintf(stderr, "BTstack: WARNING — failed to register SBC endpoint\n");
        }
    }

    /* AAC endpoint — MPEG-2/4 AAC-LC */
    {
        /*
         * AAC capability: 6 bytes per A2DP spec
         * Byte 0: object_type bitmap (MPEG-2 AAC-LC = 0x80)
         * Byte 1: sampling_freq high byte (48k=0x04, 44.1k=0x08)
         * Byte 2: sampling_freq low byte | channels high nibble (2ch=0x04)
         * Byte 3-5: bit_rate (VBR flag in byte 3 bit 7)
         */
        static uint8_t aac_caps[6]   = { 0x80, 0x0C, 0x04, 0x03, 0xE8, 0x00 }; /* AAC-LC, 44.1+48k, 2ch, 256kbps */
        static uint8_t aac_config[6] = { 0x80, 0x04, 0x04, 0x03, 0xE8, 0x00 }; /* AAC-LC, 48k, 2ch, 256kbps */

        aac_ep_ = a2dp_source_create_stream_endpoint(
            AVDTP_AUDIO, AVDTP_CODEC_MPEG_2_4_AAC,
            aac_caps, sizeof(aac_caps),
            aac_config, sizeof(aac_config)
        );
        if (aac_ep_) {
            aac_local_seid_ = avdtp_local_seid(aac_ep_);
            fprintf(stderr, "BTstack: Registered AAC endpoint (SEID=%u)\n", aac_local_seid_);
        } else {
            fprintf(stderr, "BTstack: WARNING — failed to register AAC endpoint\n");
        }
    }
}

/* ======================================================================== */
/* Shutdown                                                                 */
/* ======================================================================== */

/* ---- AFH host channel classification / radio readings ---- */

namespace {
std::mutex g_afh_mutex;
std::string g_afh_policy = "auto";
std::atomic<int> g_afh_policy_version{0};

/* Not in BTstack's command table: HCI Set AFH Host Channel Classification
 * (10-byte map as 8 + 1 + 1 bytes) and HCI Read AFH Channel Map */
const hci_cmd_t CMD_SET_AFH_HOST_CHANNEL_CLASSIFICATION = {0x0C3F, "D11"};
const hci_cmd_t CMD_READ_AFH_CHANNEL_MAP = {0x1406, "H"};
const uint16_t OPCODE_READ_RSSI = 0x1405;

const uint32_t AFH_TICK_MS = 5000;
const int AFH_CLASSIFY_EVERY_TICKS = 6;  /* re-survey the Wi-Fi every 30 s */
} // namespace

void BtStackTransport::set_afh_policy(const std::string &policy) {
    std::lock_guard<std::mutex> lock(g_afh_mutex);
    if (policy == g_afh_policy) return;
    g_afh_policy = policy;
    g_afh_policy_version.fetch_add(1);
}

std::string BtStackTransport::afh_policy() {
    std::lock_guard<std::mutex> lock(g_afh_mutex);
    return g_afh_policy;
}

void BtStackTransport::afh_start() {
    if (!afh_timer_) afh_timer_ = new btstack_timer_source_t();
    auto *t = static_cast<btstack_timer_source_t *>(afh_timer_);
    afh_ticks_ = 0;
    afh_policy_version_seen_ = -1;
    afh_sent_ = false;
    btstack_run_loop_set_timer_handler(t, [](btstack_timer_source_t *ts) {
        auto *self = static_cast<BtStackTransport *>(btstack_run_loop_get_timer_context(ts));
        self->afh_tick();
        btstack_run_loop_set_timer(ts, AFH_TICK_MS);
        btstack_run_loop_add_timer(ts);
    });
    btstack_run_loop_set_timer_context(t, this);
    btstack_run_loop_set_timer(t, 200);
    btstack_run_loop_add_timer(t);
}

void BtStackTransport::afh_stop() {
    if (afh_timer_) btstack_run_loop_remove_timer(static_cast<btstack_timer_source_t *>(afh_timer_));
    std::lock_guard<std::mutex> lock(link_radio_mutex());
    link_radio_unlocked() = LinkRadio{};
}

void BtStackTransport::afh_tick() {
    if (!hci_ready_.load()) return;
    int version = g_afh_policy_version.load();
    bool classify = version != afh_policy_version_seen_ || afh_ticks_ % AFH_CLASSIFY_EVERY_TICKS == 0;
    afh_ticks_++;

    /* 1. The classification, when it changes (one command per tick) */
    if (classify) {
        AfhClassification c = afh_classify(afh_policy());
        {
            std::lock_guard<std::mutex> lock(link_radio_mutex());
            link_radio_unlocked().afh_usable = c.passed ? c.usable : -1;
            link_radio_unlocked().afh_avoided = afh_describe(c.avoided_wifi);
        }
        /* Nothing to pass and nothing passed before: leave the controller alone */
        bool changed = c.passed ? (!afh_sent_ || memcmp(c.map, afh_sent_map_, 10) != 0) : afh_sent_;
        if (changed && hci_can_send_command_packet_now()) {
            hci_send_cmd(&CMD_SET_AFH_HOST_CHANNEL_CLASSIFICATION, c.map, c.map[8], c.map[9]);
            memcpy(afh_sent_map_, c.map, 10);
            afh_sent_ = c.passed;
            afh_policy_version_seen_ = version;
            fprintf(stderr, "BTstack: AFH host classification: %s (%d of 79 channels usable)\n",
                    c.passed ? ("avoid Wi-Fi " + afh_describe(c.avoided_wifi)).c_str() : "cleared", c.usable);
            return;
        }
        if (!changed) afh_policy_version_seen_ = version;
    }

    /* 2. Readings of the current link: RSSI and AFH map in turn */
    uint16_t handle = a2dp_con_handle_.load();
    if (handle == 0xFFFF) {
        std::lock_guard<std::mutex> lock(link_radio_mutex());
        link_radio_unlocked().has_rssi = false;
        link_radio_unlocked().afh_in_use = -1;
        return;
    }
    if (!hci_can_send_command_packet_now()) return;
    if (afh_ticks_ % 2)
        hci_send_cmd(&hci_read_rssi, handle);
    else
        hci_send_cmd(&CMD_READ_AFH_CHANNEL_MAP, handle);
}

void BtStackTransport::on_radio_command_complete(uint8_t *packet, uint16_t size) {
    uint16_t opcode = hci_event_command_complete_get_command_opcode(packet);
    const uint8_t *rp = hci_event_command_complete_get_return_parameters(packet);
    size_t rp_len = size >= 6 ? size - 6 : 0;
    if (opcode == CMD_SET_AFH_HOST_CHANNEL_CLASSIFICATION.opcode) {
        if (rp_len >= 1 && rp[0] != ERROR_CODE_SUCCESS)
            fprintf(stderr, "BTstack: AFH host classification refused (0x%02x)\n", rp[0]);
    } else if (opcode == OPCODE_READ_RSSI && rp_len >= 4) {
        std::lock_guard<std::mutex> lock(link_radio_mutex());
        link_radio_unlocked().has_rssi = rp[0] == ERROR_CODE_SUCCESS;
        link_radio_unlocked().rssi = static_cast<int8_t>(rp[3]);
    } else if (opcode == CMD_READ_AFH_CHANNEL_MAP.opcode && rp_len >= 14) {
        /* status, handle, mode, map[10] */
        std::lock_guard<std::mutex> lock(link_radio_mutex());
        link_radio_unlocked().afh_in_use = (rp[0] == ERROR_CODE_SUCCESS && rp[3] == 1) ? afh_count(rp + 4)
                                         : (rp[0] == ERROR_CODE_SUCCESS ? AFH_CHANNELS : -1);
    }
}

void BtStackTransport::shutdown() {
    if (!running_.load()) return;

    /* Power off HCI properly so the USB adapter is released.
     * Without this, the next launch can't open the adapter. */
    if (hci_ready_.load()) {
        fprintf(stderr, "BTstack: powering off HCI...\n");
        fflush(stderr);

        /* Dispatch hci_power_control(HCI_POWER_OFF) to BTstack thread */
        RunLoopRequest req = {};
        req.done_event = CreateEventA(nullptr, FALSE, FALSE, nullptr);
        req.reg.callback = [](void *ctx) {
            auto *r = static_cast<RunLoopRequest *>(ctx);
            hci_power_control(HCI_POWER_OFF);
            SetEvent(r->done_event);
        };
        req.reg.context = &req;
        btstack_run_loop_execute_on_main_thread(&req.reg);
        WaitForSingleObject(req.done_event, 3000);
        CloseHandle(req.done_event);

        /* Wait for HCI_STATE_OFF event (signals init_event_) */
        WaitForSingleObject(init_event_, 5000);
        fprintf(stderr, "BTstack: HCI powered off\n");
        fflush(stderr);
    }

    running_.store(false);
    streaming_.store(false);
    connected_.store(false);
    hci_ready_.store(false);

    /* Request BTstack run loop to exit */
    btstack_run_loop_trigger_exit();

    if (thread_handle_) {
        WaitForSingleObject(thread_handle_, 10000);
        CloseHandle(thread_handle_);
        thread_handle_ = nullptr;
    }

    fprintf(stderr, "BTstack: shutdown complete\n");
    fflush(stderr);

    instance_ = nullptr;
}

/* ======================================================================== */
/* Connection                                                               */
/* ======================================================================== */

bool BtStackTransport::scan_devices(uint8_t duration_seconds) {
    if (!hci_ready_.load()) return false;

    /* duration is in units of 1.28 seconds */
    uint8_t duration_units = (uint8_t)((duration_seconds * 10 + 12) / 13);  /* convert to 1.28s units */
    if (duration_units < 3) duration_units = 3;
    if (duration_units > 30) duration_units = 30;

    fprintf(stderr, "BTstack: Starting inquiry scan (~%u seconds)...\n",
           (unsigned)(duration_units * 128 / 100));
    fflush(stderr);
    /* Fresh user-initiated operation — clear a cancel latched by a
     * previous stop, or the inquiry wait below aborts immediately. */
    ResetEvent(static_cast<HANDLE>(cancel_event_));
    discovered_devices_.clear();
    inquiry_found_count_.store(0);
    inquiry_active_.store(true);

    /* Dispatch gap_inquiry_start to BTstack thread */
    RunLoopRequest req = {};
    req.done_event = CreateEventA(nullptr, FALSE, FALSE, nullptr);
    req.inquiry_duration = duration_units;
    req.reg.callback = [](void *ctx) {
        auto *r = static_cast<RunLoopRequest *>(ctx);
        r->int_result = gap_inquiry_start(r->inquiry_duration);
        SetEvent(r->done_event);
    };
    req.reg.context = &req;
    btstack_run_loop_execute_on_main_thread(&req.reg);
    WaitForSingleObject(req.done_event, 5000);
    CloseHandle(req.done_event);

    fprintf(stderr, "BTstack: gap_inquiry_start result=%d\n", req.int_result);
    fflush(stderr);
    if (req.int_result != 0) {
        fprintf(stderr, "BTstack: gap_inquiry_start failed (%d)\n", req.int_result);
        fflush(stderr);
        inquiry_active_.store(false);
        return false;
    }

    /* Wait for inquiry to complete */
    uint32_t timeout_ms = (duration_units * 1280) + 5000;  /* inquiry time + margin */
    if (!wait_for_event(inquiry_event_, timeout_ms)) {
        fprintf(stderr, "BTstack: Inquiry timed out\n");
        /* Dispatch gap_inquiry_stop to BTstack thread */
        RunLoopRequest stop_req = {};
        stop_req.done_event = CreateEventA(nullptr, FALSE, FALSE, nullptr);
        stop_req.reg.callback = [](void *ctx) {
            auto *r = static_cast<RunLoopRequest *>(ctx);
            gap_inquiry_stop();
            SetEvent(r->done_event);
        };
        stop_req.reg.context = &stop_req;
        btstack_run_loop_execute_on_main_thread(&stop_req.reg);
        WaitForSingleObject(stop_req.done_event, 2000);
        CloseHandle(stop_req.done_event);
        inquiry_active_.store(false);
        return false;
    }

    fprintf(stderr, "BTstack: scan_devices returning, found %u device(s)\n",
            inquiry_found_count_.load());
    fflush(stderr);
    return inquiry_found_count_.load() > 0;
}

bool BtStackTransport::connect_a2dp(const uint8_t remote_addr[6]) {
    if (!hci_ready_.load()) return false;

    /* BTstack uses bd_addr_t as big-endian, but our addr is little-endian (Windows BTH_ADDR) */
    bd_addr_t addr;
    for (int i = 0; i < 6; i++) {
        addr[i] = remote_addr[5 - i];
    }

    /* Store address for reconnection */
    memcpy(remote_addr_be_, addr, 6);
    has_remote_addr_ = true;

    /* Reset remote capabilities */
    remote_caps_ = {};
    disconnect_occurred_.store(false);
    last_connect_failure_.store(ConnectFailure::None);
    target_acl_handle_.store(0xFFFF);
    target_acl_down_reason_.store(0);

    /* Clear stale events from any previous attempt */
    ResetEvent(static_cast<HANDLE>(cancel_event_));
    ResetEvent(static_cast<HANDLE>(connect_event_));
    connect_result_.store(false);
    ResetEvent(static_cast<HANDLE>(stream_event_));
    stream_result_.store(false);

    fprintf(stderr, "BTstack: Connecting to %02X:%02X:%02X:%02X:%02X:%02X...\n",
           addr[0], addr[1], addr[2], addr[3], addr[4], addr[5]);
    release_leftover_avdtp();

    /* Dispatch a2dp_source_establish_stream to BTstack thread */
    RunLoopRequest req = {};
    req.done_event = CreateEventA(nullptr, FALSE, FALSE, nullptr);
    memcpy(req.addr, addr, 6);
    req.cid_out = &a2dp_cid_;
    req.reg.callback = [](void *ctx) {
        auto *r = static_cast<RunLoopRequest *>(ctx);
        r->u8_result = a2dp_source_establish_stream(r->addr, r->cid_out);
        SetEvent(r->done_event);
    };
    req.reg.context = &req;
    btstack_run_loop_execute_on_main_thread(&req.reg);
    WaitForSingleObject(req.done_event, 5000);
    CloseHandle(req.done_event);

    if (req.u8_result != ERROR_CODE_SUCCESS) {
        fprintf(stderr, "BTstack: a2dp_source_establish_stream failed (0x%02x)\n", req.u8_result);
        last_connect_failure_.store(ConnectFailure::Other);
        return false;
    }

    /* Wait for stream establishment (includes signaling connection + SEP discovery) */
    if (!wait_for_event(connect_event_, CONNECT_TIMEOUT_MS)) {
        fprintf(stderr, "BTstack: Connection timed out\n");
        /* An answer that never came (e.g. a lost SDP response) */
        last_connect_failure_.store(ConnectFailure::LinkLost);
        classify_link_lost();
        abort_pending_connection();
        return false;
    }

    if (!connect_result_.load()) {
        classify_link_lost();
        abort_pending_connection();
        return false;
    }
    return true;
}

void BtStackTransport::classify_link_lost() {
    if (last_connect_failure_.load() != ConnectFailure::LinkLost) return;
    /* L2CAP reports a link that went down as "baseband disconnect" before the
     * HCI Disconnection Complete reaches us: wait for it to learn the reason */
    uint8_t reason = target_acl_down_reason_.load();
    for (int i = 0; reason == 0 && target_acl_handle_.load() != 0xFFFF && i < 20; i++) {
        Sleep(50);
        reason = target_acl_down_reason_.load();
    }
    /* The device ended the link itself: not the radio */
    if (reason == ERROR_CODE_REMOTE_USER_TERMINATED_CONNECTION ||
        reason == ERROR_CODE_REMOTE_DEVICE_TERMINATED_CONNECTION_DUE_TO_LOW_RESOURCES ||
        reason == ERROR_CODE_REMOTE_DEVICE_TERMINATED_CONNECTION_DUE_TO_POWER_OFF) {
        fprintf(stderr, "BTstack: the device ended the link itself (0x%02x)\n", reason);
        last_connect_failure_.store(ConnectFailure::Refused);
    }
}

bool BtStackTransport::connect_a2dp_retrying(const uint8_t remote_addr[6],
                                             const std::function<bool()> &cancelled,
                                             const std::function<void(int, int)> &on_retry) {
    bool repaired = false;
    for (int attempt = 1;; attempt++) {
        if (connect_a2dp(remote_addr)) return true;
        if (cancelled && cancelled()) return false;
        ConnectFailure f = last_connect_failure_.load();
        if (f == ConnectFailure::AuthFailed && !repaired) {
            /* Stale link key: forget it and pair again, once */
            repaired = true;
            fprintf(stderr, "BTstack: authentication failed, dropping the stored link key and pairing again\n");
            RunLoopRequest req = {};
            req.done_event = CreateEventA(nullptr, FALSE, FALSE, nullptr);
            memcpy(req.addr, remote_addr_be_, 6);
            req.reg.callback = [](void *ctx) {
                auto *r = static_cast<RunLoopRequest *>(ctx);
                gap_drop_link_key_for_bd_addr(r->addr);
                SetEvent(r->done_event);
            };
            req.reg.context = &req;
            btstack_run_loop_execute_on_main_thread(&req.reg);
            WaitForSingleObject(req.done_event, 2000);
            CloseHandle(req.done_event);
            if (on_retry) on_retry(attempt + 1, CONNECT_ATTEMPTS);
            drop_acl_link(5000);
            continue;
        }
        bool transient = f == ConnectFailure::NoAnswer || f == ConnectFailure::LinkLost;
        if (!transient || attempt >= CONNECT_ATTEMPTS) return false;
        fprintf(stderr, "BTstack: connection attempt %d/%d failed (%s), trying again\n", attempt,
                CONNECT_ATTEMPTS, f == ConnectFailure::NoAnswer ? "no answer" : "link lost");
        if (on_retry) on_retry(attempt + 1, CONNECT_ATTEMPTS);
        drop_acl_link(5000);
        /* Give both controllers a moment before paging again */
        for (int i = 0; i < 10; i++) {
            if (cancelled && cancelled()) return false;
            Sleep(100);
        }
    }
}

void BtStackTransport::release_leftover_avdtp() {
    ResetEvent(static_cast<HANDLE>(disconnect_event_));
    RunLoopRequest req = {};
    req.done_event = CreateEventA(nullptr, FALSE, FALSE, nullptr);
    memcpy(req.addr, remote_addr_be_, 6);
    req.reg.callback = [](void *ctx) {
        auto *r = static_cast<RunLoopRequest *>(ctx);
        r->int_result = 0;
        avdtp_connection_t *c = avdtp_get_connection_for_bd_addr(r->addr);
        if (c != nullptr) {
            r->int_result = (c->state == AVDTP_SIGNALING_CONNECTION_OPENED ||
                             c->state == AVDTP_SIGNALING_CONNECTION_W4_L2CAP_DISCONNECTED) ? 1 : 2;
            r->config_a2dp_cid = c->avdtp_cid;
            a2dp_source_disconnect(c->avdtp_cid);
        }
        SetEvent(r->done_event);
    };
    req.reg.context = &req;
    btstack_run_loop_execute_on_main_thread(&req.reg);
    WaitForSingleObject(req.done_event, 2000);
    CloseHandle(req.done_event);
    if (req.int_result == 0) return;
    fprintf(stderr, "BTstack: closing a leftover AVDTP connection (cid=0x%04x) first\n", req.config_a2dp_cid);
    /* An open signaling channel is released asynchronously; one that never
     * opened is freed at once */
    if (req.int_result == 1 &&
        WaitForSingleObject(static_cast<HANDLE>(disconnect_event_), DISCONNECT_TIMEOUT_MS) != WAIT_OBJECT_0)
        fprintf(stderr, "BTstack: leftover AVDTP release timed out\n");
    connected_.store(false);
    streaming_.store(false);
    disconnect_occurred_.store(false);
    ResetEvent(static_cast<HANDLE>(connect_event_));
    ResetEvent(static_cast<HANDLE>(stream_event_));
}

void BtStackTransport::drop_acl_link(uint32_t timeout_ms) {
    if (!has_remote_addr_ || !hci_ready_.load()) return;
    ResetEvent(static_cast<HANDLE>(acl_down_event_));
    RunLoopRequest req = {};
    req.done_event = CreateEventA(nullptr, FALSE, FALSE, nullptr);
    memcpy(req.addr, remote_addr_be_, 6);
    req.reg.callback = [](void *ctx) {
        auto *r = static_cast<RunLoopRequest *>(ctx);
        hci_connection_t *conn = hci_connection_for_bd_addr_and_type(r->addr, BD_ADDR_TYPE_ACL);
        r->int_result = 0;
        if (conn && gap_disconnect(conn->con_handle) == ERROR_CODE_SUCCESS) r->int_result = 1;
        SetEvent(r->done_event);
    };
    req.reg.context = &req;
    btstack_run_loop_execute_on_main_thread(&req.reg);
    WaitForSingleObject(req.done_event, 2000);
    CloseHandle(req.done_event);
    if (req.int_result != 1) return;
    fprintf(stderr, "BTstack: dropping the ACL link before trying again\n");
    if (WaitForSingleObject(static_cast<HANDLE>(acl_down_event_), timeout_ms) != WAIT_OBJECT_0)
        fprintf(stderr, "BTstack: no Disconnection Complete within %u ms\n", timeout_ms);
}

uint32_t BtStackTransport::pick_aptx_sample_rate(AudioCodec codec, uint32_t wanted) const {
    uint8_t caps;
    switch (codec) {
    case AudioCodec::Aptx:   caps = remote_caps_.aptx_caps;   break;
    case AudioCodec::AptxHD: caps = remote_caps_.aptxhd_caps; break;
    case AudioCodec::AptxLL: caps = remote_caps_.aptxll_caps; break;
    default: return wanted;
    }
    uint8_t freqs = caps & APTX_FREQ_MASK;
    bool has441 = (freqs == 0) || (freqs & APTX_FREQ_44100);
    bool has48  = (freqs == 0) || (freqs & APTX_FREQ_48000);
    if (wanted == 48000 && has48) return 48000;
    if (wanted == 44100 && has441) return 44100;
    /* Other rate (e.g. 32k/88.2k/96k) or unsupported by the remote:
     * prefer 48k, then 44.1k; WASAPI auto-resamples to the requested rate */
    if (has48) return 48000;
    if (has441) return 44100;
    return wanted;  /* remote lists neither; configure_codec will reject */
}

bool BtStackTransport::configure_codec(AudioCodec codec, uint32_t sample_rate, uint8_t channels) {
    if (!connected_.load()) return false;

    /* Clear stale stream event from any previous attempt */
    ResetEvent(static_cast<HANDLE>(stream_event_));
    stream_result_.store(false);

    selected_codec_ = codec;
    sample_rate_ = sample_rate;
    channels_ = channels;

    /* Determine local and remote SEIDs based on codec */
    uint8_t local = 0, remote = 0;
    switch (codec) {
    case AudioCodec::LDAC:
        if (!remote_caps_.ldac) return false;
        local = ldac_local_seid_;
        remote = remote_caps_.ldac_seid;
        break;
    case AudioCodec::AptxHD:
        if (!remote_caps_.aptx_hd) return false;
        local = aptxhd_local_seid_;
        remote = remote_caps_.aptxhd_seid;
        break;
    case AudioCodec::AptxLL:
        if (!remote_caps_.aptx_ll) return false;
        local = aptxll_local_seid_;
        remote = remote_caps_.aptxll_seid;
        break;
    case AudioCodec::Aptx:
        if (!remote_caps_.aptx) return false;
        local = aptx_local_seid_;
        remote = remote_caps_.aptx_seid;
        break;
    case AudioCodec::SBC:
        if (!remote_caps_.sbc) return false;
        local = sbc_local_seid_;
        remote = remote_caps_.sbc_seid;
        break;
    case AudioCodec::AAC:
        if (!remote_caps_.aac) return false;
        local = aac_local_seid_;
        remote = remote_caps_.aac_seid;
        break;
    }

    if (local == 0 || remote == 0) return false;
    local_seid_ = local;
    remote_seid_ = remote;

    /* Build codec configuration for SET_CONFIGURATION */
    uint8_t config_info[sizeof(RunLoopRequest::config_info)] = {};
    uint8_t config_len = 0;

    switch (codec) {
    case AudioCodec::LDAC: {
        write_vendor_codec_id(config_info, LDAC_VENDOR_ID, LDAC_CODEC_ID);
        uint8_t freq = (sample_rate == 44100) ? 0x20 :
                       (sample_rate == 48000) ? 0x10 :
                       (sample_rate == 88200) ? 0x08 : 0x04;
        uint8_t ch = (channels == 1) ? 0x04 : 0x01;  /* mono or stereo */
        config_info[6] = freq;
        config_info[7] = ch;
        config_len = 8;
        break;
    }
    case AudioCodec::AptxHD: {
        uint8_t freq = aptx_freq_bits_for(codec, remote_caps_.aptxhd_caps, sample_rate);
        if (freq == 0) return false;
        memset(config_info, 0, APTXHD_INFO_LEN);  /* reserved bytes 7..10 = 0 */
        write_vendor_codec_id(config_info, APTXHD_VENDOR_ID, APTXHD_CODEC_ID);
        /* Always stereo on the wire; the encoder duplicates mono input */
        config_info[6] = freq | APTX_CH_STEREO;
        config_len = APTXHD_INFO_LEN;
        break;
    }
    case AudioCodec::AptxLL: {
        uint8_t freq = aptx_freq_bits_for(codec, remote_caps_.aptxll_caps, sample_rate);
        if (freq == 0) return false;
        memset(config_info, 0, APTXLL_EXT_INFO_LEN);
        write_vendor_codec_id(config_info, remote_caps_.aptxll_vendor_id, APTXLL_CODEC_ID);
        config_info[6] = freq | APTX_CH_STEREO;
        config_len = APTXLL_INFO_LEN;
        /* No backchannel support: bidirect_link stays 0 */
        if (remote_caps_.aptxll_info_len >= APTXLL_EXT_INFO_LEN &&
            (remote_caps_.aptxll_info[7] & APTXLL_HAS_NEW_CAPS)) {
            /* Sink sent the extended caps: echo them back like PipeWire
             * (codec_select_config_ll), raising the buffer levels to at
             * least PipeWire's adjusted defaults. */
            const uint8_t *rc = remote_caps_.aptxll_info;
            auto le16 = [](const uint8_t *p) { return (uint16_t)(p[0] | (p[1] << 8)); };
            auto put16 = [](uint8_t *p, uint16_t v) { p[0] = (uint8_t)v; p[1] = (uint8_t)(v >> 8); };
            uint16_t target  = le16(rc + 9);
            uint16_t initial = le16(rc + 11);
            uint16_t good    = le16(rc + 15);
            if (target  < APTXLL_TARGET_LEVEL)  target  = APTXLL_TARGET_LEVEL;
            if (initial < APTXLL_INITIAL_LEVEL) initial = APTXLL_INITIAL_LEVEL;
            if (good    < APTXLL_GOOD_LEVEL)    good    = APTXLL_GOOD_LEVEL;
            config_info[7] = APTXLL_HAS_NEW_CAPS;
            config_info[8] = rc[8];                                     /* reserved */
            put16(config_info + 9, target);
            put16(config_info + 11, initial);
            config_info[13] = rc[13] ? rc[13] : APTXLL_SRA_MAX_RATE;
            config_info[14] = rc[14] ? rc[14] : APTXLL_SRA_AVG_TIME;
            put16(config_info + 15, good);
            config_len = APTXLL_EXT_INFO_LEN;
        }
        break;
    }
    case AudioCodec::Aptx: {
        uint8_t freq = aptx_freq_bits_for(codec, remote_caps_.aptx_caps, sample_rate);
        if (freq == 0) return false;
        write_vendor_codec_id(config_info, APTX_VENDOR_ID, APTX_CODEC_ID);
        /* Always stereo on the wire; AptxEncoder duplicates mono input */
        config_info[6] = freq | APTX_CH_STEREO;
        config_len = 7;
        break;
    }
    case AudioCodec::SBC:
    case AudioCodec::AAC:
        /* SBC and AAC use typed config structs, handled below */
        config_len = 0;
        break;
    }

    /* Dispatch codec configuration to BTstack thread */
    RunLoopRequest req = {};
    req.done_event = CreateEventA(nullptr, FALSE, FALSE, nullptr);
    req.config_a2dp_cid = a2dp_cid_;
    req.config_local_seid = local;
    req.config_remote_seid = remote;
    req.config_codec = codec;
    memcpy(req.config_info, config_info, config_len);
    req.config_info_len = config_len;

    /* Prepare SBC/AAC typed config structs */
    if (codec == AudioCodec::SBC) {
        memset(&req.sbc_config, 0, sizeof(req.sbc_config));
        req.sbc_config.sampling_frequency = static_cast<uint16_t>(sample_rate);
        req.sbc_config.channel_mode = (channels == 1) ? AVDTP_CHANNEL_MODE_MONO : AVDTP_CHANNEL_MODE_JOINT_STEREO;
        req.sbc_config.block_length = AVDTP_SBC_BLOCK_LENGTH_16;
        req.sbc_config.subbands = AVDTP_SBC_SUBBANDS_8;
        req.sbc_config.allocation_method = AVDTP_SBC_ALLOCATION_METHOD_LOUDNESS;
        req.sbc_config.min_bitpool_value = 2;
        req.sbc_config.max_bitpool_value = 53;
    } else if (codec == AudioCodec::AAC) {
        memset(&req.aac_config, 0, sizeof(req.aac_config));
        req.aac_config.object_type = AVDTP_AAC_MPEG2_LC;
        req.aac_config.sampling_frequency = sample_rate;
        req.aac_config.channels = channels;
        req.aac_config.bit_rate = 256000;
        req.aac_config.vbr = 0;
        req.aac_config.drc = false;
    }

    req.reg.callback = [](void *ctx) {
        auto *r = static_cast<RunLoopRequest *>(ctx);
        switch (r->config_codec) {
        case AudioCodec::SBC:
            r->u8_result = a2dp_source_set_config_sbc(
                r->config_a2dp_cid, r->config_local_seid, r->config_remote_seid,
                &r->sbc_config);
            break;
        case AudioCodec::AAC:
            r->u8_result = a2dp_source_set_config_mpeg_aac(
                r->config_a2dp_cid, r->config_local_seid, r->config_remote_seid,
                &r->aac_config);
            break;
        default:
            /* BTstack keeps the pointer: pass the persistent copy, not the
             * request (which lives on configure_codec()'s stack) */
            memcpy(s_other_config_info, r->config_info, r->config_info_len);
            r->u8_result = a2dp_source_set_config_other(
                r->config_a2dp_cid, r->config_local_seid, r->config_remote_seid,
                s_other_config_info, r->config_info_len);
            break;
        }
        SetEvent(r->done_event);
    };
    req.reg.context = &req;
    btstack_run_loop_execute_on_main_thread(&req.reg);
    WaitForSingleObject(req.done_event, 5000);
    CloseHandle(req.done_event);

    uint8_t status = req.u8_result;
    if (status != ERROR_CODE_SUCCESS) {
        fprintf(stderr, "BTstack: set_config_other failed (0x%02x)\n", status);
        return false;
    }

    /* Wait for stream established event */
    if (!wait_for_event(stream_event_, STREAM_TIMEOUT_MS)) {
        fprintf(stderr, "BTstack: Stream configuration timed out\n");
        return false;
    }

    return stream_result_.load();
}

bool BtStackTransport::disconnect() {
    if (a2dp_cid_ == 0) return true;

    if (streaming_.load()) {
        stop_stream();
    }

    fprintf(stderr, "BTstack: Disconnecting A2DP (cid=0x%04x)\n", a2dp_cid_);

    /* Same teardown as a failed connect: resets disconnect_event_ before
     * dispatching, closes the AVDTP signaling channel on the BTstack thread
     * (or frees a half-open connection / drops the ACL), and waits for
     * SIGNALING_CONNECTION_RELEASED with a plain bounded
     * WaitForSingleObject. wait_for_event() must not be used here: after a
     * user stop cancel_event_ stays latched, so it would return at once and
     * a2dp_cid_ would be zeroed before the link was actually released. */
    abort_pending_connection();
    return true;
}

bool BtStackTransport::check_disconnected() {
    return disconnect_occurred_.exchange(false);
}

void BtStackTransport::cancel_pending_waits() {
    SetEvent(static_cast<HANDLE>(cancel_event_));
}

void BtStackTransport::abort_pending_connection() {
    /*
     * After a failed or timed-out connect, the AVDTP connection may still be
     * alive inside BTstack (outgoing_active set, SEP discovery lock held), so
     * the next a2dp_source_establish_stream() would return COMMAND_DISALLOWED.
     */
    if (a2dp_cid_ != 0) {
        ResetEvent(static_cast<HANDLE>(disconnect_event_));

        RunLoopRequest req = {};
        req.done_event = CreateEventA(nullptr, FALSE, FALSE, nullptr);
        req.config_a2dp_cid = a2dp_cid_;
        memcpy(req.addr, remote_addr_be_, 6);
        req.reg.callback = [](void *ctx) {
            auto *r = static_cast<RunLoopRequest *>(ctx);
            r->int_result = 0;  /* 1 = wait for SIGNALING_CONNECTION_RELEASED */
            avdtp_connection_t *c = avdtp_get_connection_for_avdtp_cid(r->config_a2dp_cid);
            if (c == nullptr) {
                /* Already finalized by BTstack (e.g. signaling connect failed) */
            } else if (c->state == AVDTP_SIGNALING_CONNECTION_OPENED ||
                       c->state == AVDTP_SIGNALING_CONNECTION_W4_L2CAP_DISCONNECTED) {
                /* L2CAP signaling channel is up: normal close, RELEASED follows */
                a2dp_source_disconnect(r->config_a2dp_cid);
                r->int_result = 1;
            } else {
                /* Not open yet (SDP query / W4_L2CAP_CONNECTED): avdtp_disconnect
                 * emits a failed CONNECTION_ESTABLISHED and frees the connection
                 * synchronously, but a pending outgoing L2CAP channel would be
                 * orphaned. Drop the ACL too if it is up; never touch an ACL
                 * that is still paging (no valid con_handle yet). */
                a2dp_source_disconnect(r->config_a2dp_cid);
                hci_connection_t *acl = hci_connection_for_bd_addr_and_type(r->addr, BD_ADDR_TYPE_ACL);
                if (acl != nullptr && acl->state == OPEN) {
                    gap_disconnect(acl->con_handle);
                }
            }
            SetEvent(r->done_event);
        };
        req.reg.context = &req;
        btstack_run_loop_execute_on_main_thread(&req.reg);
        WaitForSingleObject(req.done_event, 5000);
        CloseHandle(req.done_event);

        /* Plain wait (not wait_for_event): cleanup must complete even when
         * cancel_event_ is latched by a user stop. */
        if (req.int_result == 1) {
            if (WaitForSingleObject(static_cast<HANDLE>(disconnect_event_),
                                    DISCONNECT_TIMEOUT_MS) != WAIT_OBJECT_0) {
                fprintf(stderr, "BTstack: AVDTP signaling release timed out\n");
            }
        }
    }

    /* This was never a live stream: don't let the RELEASED/HCI disconnect
     * handlers make the streaming loop think a connection was lost. */
    connected_.store(false);
    streaming_.store(false);
    a2dp_cid_ = 0;
    disconnect_occurred_.store(false);
    /* The failed CONNECTION_ESTABLISHED path signals these synchronously */
    ResetEvent(static_cast<HANDLE>(connect_event_));
    ResetEvent(static_cast<HANDLE>(stream_event_));
}

bool BtStackTransport::reconnect() {
    if (!hci_ready_.load() || !has_remote_addr_) {
        fprintf(stderr, "BTstack: Cannot reconnect — HCI not ready or no stored address\n");
        return false;
    }

    /* Abort immediately if a cancel is pending (user pressed disconnect) */
    if (WaitForSingleObject(static_cast<HANDLE>(cancel_event_), 0) == WAIT_OBJECT_0) {
        fprintf(stderr, "BTstack: reconnect aborted (cancel requested)\n");
        return false;
    }

    /* Ensure previous connection state is fully cleared */
    connected_.store(false);
    streaming_.store(false);
    media_queue_count_.store(0);
    link_stats().queue_depth.store(0, std::memory_order_relaxed);
    media_queue_head_ = 0;
    media_queue_tail_ = 0;
    a2dp_cid_ = 0;
    local_seid_ = 0;
    remote_seid_ = 0;
    media_mtu_ = 0;

    /* Reset remote capabilities for re-discovery */
    remote_caps_ = {};
    disconnect_occurred_.store(false);

    /* Reset sync event flags.
     * cancel_event_ is deliberately NOT reset here: it is armed by
     * cancel_pending_waits() when the user stops the session and must stay
     * latched across auto-reconnect attempts, otherwise a stop arriving
     * between attempts is swallowed and the loop keeps blocking for the
     * full page timeout. It is re-armed only at the start of a fresh
     * user-initiated operation (connect_a2dp / scan_devices). */
    ResetEvent(static_cast<HANDLE>(connect_event_));
    connect_result_.store(false);
    stream_result_.store(false);
    start_result_.store(false);

    fprintf(stderr, "BTstack: Reconnecting to %02X:%02X:%02X:%02X:%02X:%02X...\n",
           remote_addr_be_[0], remote_addr_be_[1], remote_addr_be_[2],
           remote_addr_be_[3], remote_addr_be_[4], remote_addr_be_[5]);
    release_leftover_avdtp();

    /* Dispatch a2dp_source_establish_stream to BTstack thread */
    RunLoopRequest req = {};
    req.done_event = CreateEventA(nullptr, FALSE, FALSE, nullptr);
    memcpy(req.addr, remote_addr_be_, 6);
    req.cid_out = &a2dp_cid_;
    req.reg.callback = [](void *ctx) {
        auto *r = static_cast<RunLoopRequest *>(ctx);
        r->u8_result = a2dp_source_establish_stream(r->addr, r->cid_out);
        SetEvent(r->done_event);
    };
    req.reg.context = &req;
    btstack_run_loop_execute_on_main_thread(&req.reg);
    WaitForSingleObject(req.done_event, 5000);
    CloseHandle(req.done_event);

    if (req.u8_result != ERROR_CODE_SUCCESS) {
        fprintf(stderr, "BTstack: reconnect establish_stream failed (0x%02x)\n", req.u8_result);
        return false;
    }

    /* Wait for connection + capability discovery */
    if (!wait_for_event(connect_event_, CONNECT_TIMEOUT_MS)) {
        fprintf(stderr, "BTstack: Reconnection timed out\n");
        abort_pending_connection();
        return false;
    }

    if (!connect_result_.load()) {
        fprintf(stderr, "BTstack: Reconnection failed\n");
        abort_pending_connection();
        return false;
    }

    /* Re-configure codec with same settings */
    if (!configure_codec(selected_codec_, sample_rate_, channels_)) {
        fprintf(stderr, "BTstack: Reconnect codec config failed\n");
        disconnect();
        return false;
    }

    /* Re-start stream */
    if (!start_stream()) {
        fprintf(stderr, "BTstack: Reconnect stream start failed\n");
        disconnect();
        return false;
    }

    fprintf(stderr, "BTstack: Reconnected and streaming\n");
    return true;
}

/* ======================================================================== */
/* Streaming                                                                */
/* ======================================================================== */

bool BtStackTransport::start_stream() {
    if (!connected_.load() || local_seid_ == 0) return false;

    /* Clear stale start event from any previous attempt */
    ResetEvent(static_cast<HANDLE>(start_event_));
    start_result_.store(false);

    /* Dispatch a2dp_source_start_stream to BTstack thread */
    RunLoopRequest req = {};
    req.done_event = CreateEventA(nullptr, FALSE, FALSE, nullptr);
    req.start_a2dp_cid = a2dp_cid_;
    req.start_local_seid = local_seid_;
    req.reg.callback = [](void *ctx) {
        auto *r = static_cast<RunLoopRequest *>(ctx);
        r->u8_result = a2dp_source_start_stream(r->start_a2dp_cid, r->start_local_seid);
        SetEvent(r->done_event);
    };
    req.reg.context = &req;
    btstack_run_loop_execute_on_main_thread(&req.reg);
    WaitForSingleObject(req.done_event, 5000);
    CloseHandle(req.done_event);

    if (req.u8_result != ERROR_CODE_SUCCESS) {
        fprintf(stderr, "BTstack: start_stream failed (0x%02x)\n", req.u8_result);
        return false;
    }

    if (!wait_for_event(start_event_, STREAM_TIMEOUT_MS)) {
        fprintf(stderr, "BTstack: Stream start timed out\n");
        return false;
    }

    return start_result_.load();
}

bool BtStackTransport::stop_stream() {
    if (!streaming_.load()) return true;

    /* Dispatch a2dp_source_pause_stream to BTstack thread */
    RunLoopRequest req = {};
    req.done_event = CreateEventA(nullptr, FALSE, FALSE, nullptr);
    req.start_a2dp_cid = a2dp_cid_;
    req.start_local_seid = local_seid_;
    req.reg.callback = [](void *ctx) {
        auto *r = static_cast<RunLoopRequest *>(ctx);
        a2dp_source_pause_stream(r->start_a2dp_cid, r->start_local_seid);
        SetEvent(r->done_event);
    };
    req.reg.context = &req;
    btstack_run_loop_execute_on_main_thread(&req.reg);
    WaitForSingleObject(req.done_event, 5000);
    CloseHandle(req.done_event);

    streaming_.store(false);
    return true;
}

bool BtStackTransport::send_media(const uint8_t *data, uint32_t size,
                                   uint32_t timestamp, uint8_t frames,
                                   AudioCodec codec) {
    if (!streaming_.load()) return false;

    /* Enqueue under lock — keep critical section minimal so the BTstack
     * thread (CAN_SEND_NOW handler) isn't blocked during L2CAP writes. */
    {
        std::lock_guard<std::mutex> lock(send_mutex_);

        /* Drop if queue is full */
        if (media_queue_count_.load() >= MEDIA_QUEUE_CAPACITY) {
            send_failure_count_.fetch_add(1);
            link_stats().queue_drops.fetch_add(1, std::memory_order_relaxed);
            return false;
        }

        MediaPacket &slot = media_queue_[media_queue_head_];

        /* Build the media payload. SBC and LDAC start with a 1-byte media
         * payload header; aptX, aptX HD, aptX LL and AAC carry the raw payload.
         * Classic aptX and aptX LL additionally go out without an RTP header
         * (slot.no_rtp), matching Android (A2DP_APTX_OFFSET) and PipeWire. */
        uint32_t header = (codec == AudioCodec::LDAC || codec == AudioCodec::SBC) ? 1 : 0;
        if (size + header > sizeof(slot.data)) {
            /* get_media_mtu() keeps encoders below this; report if it happens anyway */
            send_failure_count_.fetch_add(1);
            link_stats().send_errors.fetch_add(1, std::memory_order_relaxed);
            if (!oversize_logged_.exchange(true))
                fprintf(stderr, "BTstack: media payload of %u bytes exceeds the %u-byte queue slot, dropped\n",
                        size + header, (unsigned)sizeof(slot.data));
            return false;
        }
        if (header) {
            /* Frame count in the low 4 bits, no fragmentation: A2DP spec (SBC),
             * AOSP A2DP_LDAC_HDR_NUM_MSK and PipeWire struct rtp_payload (LDAC) */
            slot.data[0] = frames & 0x0F;
        }
        memcpy(slot.data + header, data, size);
        slot.size = size + header;

        slot.timestamp = timestamp;
        slot.frames = frames;
        /* aptX and aptX LL: no RTP header (PipeWire a2dp-codec-aptx.c
         * codec_start_encode() writes RTP only for aptX HD) */
        slot.no_rtp = (codec == AudioCodec::Aptx || codec == AudioCodec::AptxLL);
        media_queue_head_ = (media_queue_head_ + 1) % MEDIA_QUEUE_CAPACITY;
        int depth = media_queue_count_.fetch_add(1) + 1;
        link_stats().queue_depth.store(static_cast<uint32_t>(depth), std::memory_order_relaxed);
    }
    /* --- send_mutex released --- */

    /* Trigger CAN_SEND_NOW on the BTstack thread (lock-free).
     * Only queue one request at a time to avoid corrupting the linked list. */
    if (!media_trigger_pending_.exchange(true)) {
        auto *reg = static_cast<btstack_context_callback_registration_t *>(media_trigger_reg_);
        reg->callback = [](void *ctx) {
            auto *self = static_cast<BtStackTransport *>(ctx);
            self->media_trigger_pending_.store(false);
            if (self->streaming_.load() && self->a2dp_cid_ != 0) {
                a2dp_source_stream_endpoint_request_can_send_now(
                    self->a2dp_cid_, self->local_seid_);
            }
        };
        reg->context = this;
        btstack_run_loop_execute_on_main_thread(reg);
    }

    return true;
}

uint16_t BtStackTransport::get_media_mtu() const {
    /* media_mtu_ is cached by STREAM_ESTABLISHED handler on the BTstack thread */
    uint16_t mtu = media_mtu_;
    return (mtu > 0) ? mtu : 679;
}

bool BtStackTransport::is_connected() const {
    return connected_.load();
}

bool BtStackTransport::is_streaming() const {
    return streaming_.load();
}

uint32_t BtStackTransport::get_and_reset_send_failure_count() {
    return send_failure_count_.exchange(0);
}

uint32_t BtStackTransport::get_queue_depth() const {
    int count = media_queue_count_.load();
    return (count > 0) ? static_cast<uint32_t>(count) : 0;
}

AudioCodec BtStackTransport::get_selected_codec() const {
    return selected_codec_;
}

/* ======================================================================== */
/* Event Handling                                                           */
/* ======================================================================== */

void BtStackTransport::packet_handler_trampoline(uint8_t packet_type, uint16_t channel,
                                                  uint8_t *packet, uint16_t size) {
    if (instance_) {
        instance_->handle_packet(packet_type, channel, packet, size);
    }
}

void BtStackTransport::handle_packet(uint8_t packet_type, uint16_t channel,
                                      uint8_t *packet, uint16_t size) {
    (void)channel;
    (void)size;

    if (packet_type == HCI_EVENT_PACKET) {
        uint8_t event_type = hci_event_packet_get_type(packet);

        switch (event_type) {
        case BTSTACK_EVENT_STATE: {
            uint8_t state = btstack_event_state_get_state(packet);
            if (state == HCI_STATE_WORKING) {
                uint32_t elapsed = GetTickCount() - init_start_tick_;
                fprintf(stderr, "BTstack: HCI ready (took %lu ms)\n", (unsigned long)elapsed);
                if (product_id_ == 0) {
                    /* Not Realtek: nothing to infer from the timing */
                } else if (elapsed < 500) {
                    fprintf(stderr, "BTstack: WARNING — HCI init < 500ms, "
                            "Realtek firmware likely NOT loaded\n");
                } else {
                    fprintf(stderr, "BTstack: Firmware loading appears successful "
                            "(>500ms init time)\n");
                }
                fflush(stderr);
                hci_ready_.store(true);
                afh_start();
                init_result_.store(true);
                signal_event(init_event_, true);
            } else if (state == HCI_STATE_OFF) {
                fprintf(stderr, "BTstack: HCI state OFF\n");
                fflush(stderr);
                afh_stop();
                hci_ready_.store(false);
                /* Signal init_event_ so shutdown() can proceed */
                SetEvent(init_event_);
            }
            break;
        }

        case HCI_EVENT_COMMAND_COMPLETE:
            if (size >= 14 && hci_event_command_complete_get_command_opcode(packet) ==
                                  HCI_OPCODE_HCI_READ_LOCAL_VERSION_INFORMATION) {
                on_local_version(packet);
            }
            on_radio_command_complete(packet, size);
            break;

        case HCI_EVENT_USER_CONFIRMATION_REQUEST:
            /* Auto-accept pairing (Just Works) */
            {
                bd_addr_t addr;
                hci_event_user_confirmation_request_get_bd_addr(packet, addr);
                fprintf(stderr, "BTstack: Auto-accepting pairing with %02X:%02X:%02X:%02X:%02X:%02X\n",
                       addr[0], addr[1], addr[2], addr[3], addr[4], addr[5]);
                gap_ssp_confirmation_response(addr);
            }
            break;

        case HCI_EVENT_PIN_CODE_REQUEST:
            /* Legacy pairing: respond with default PIN "0000" */
            {
                bd_addr_t addr;
                hci_event_pin_code_request_get_bd_addr(packet, addr);
                gap_pin_code_response(addr, "0000");
            }
            break;

        case HCI_EVENT_CONNECTION_COMPLETE: {
            uint8_t status = hci_event_connection_complete_get_status(packet);
            bd_addr_t addr;
            hci_event_connection_complete_get_bd_addr(packet, addr);
            if (status != 0) {
                fprintf(stderr, "BTstack: HCI connection to %02X:%02X:%02X:%02X:%02X:%02X "
                        "FAILED: 0x%02X (%s)\n",
                        addr[0], addr[1], addr[2], addr[3], addr[4], addr[5],
                        status, hci_error_string(status));
            } else {
                fprintf(stderr, "BTstack: HCI ACL connection established to "
                       "%02X:%02X:%02X:%02X:%02X:%02X\n",
                       addr[0], addr[1], addr[2], addr[3], addr[4], addr[5]);
                if (has_remote_addr_ && bd_addr_cmp(addr, remote_addr_be_) == 0) {
                    target_acl_down_reason_.store(0);
                    target_acl_handle_.store(hci_event_connection_complete_get_connection_handle(packet));
                }
            }
            break;
        }

        case GAP_EVENT_INQUIRY_RESULT: {
            bd_addr_t addr;
            gap_event_inquiry_result_get_bd_addr(packet, addr);
            uint32_t cod = gap_event_inquiry_result_get_class_of_device(packet);
            int8_t rssi = gap_event_inquiry_result_get_rssi(packet);
            const char *name = "";
            uint8_t name_len = 0;
            if (gap_event_inquiry_result_get_name_available(packet)) {
                name_len = gap_event_inquiry_result_get_name_len(packet);
                name = (const char *)gap_event_inquiry_result_get_name(packet);
            }
            fprintf(stderr, "BTstack: Inquiry result: %02X:%02X:%02X:%02X:%02X:%02X "
                   "CoD=0x%06X RSSI=%d name=%.*s\n",
                   addr[0], addr[1], addr[2], addr[3], addr[4], addr[5],
                   cod, rssi, name_len, name);
            fflush(stderr);
            if (inquiry_active_.load()) {
                inquiry_found_count_.fetch_add(1);
                DiscoveredDevice dev;
                memcpy(dev.address, addr, 6);
                dev.name = std::string(name, name_len);
                dev.cod = cod;
                dev.rssi = rssi;
                discovered_devices_.push_back(dev);
            }
            break;
        }

        case GAP_EVENT_INQUIRY_COMPLETE:
            fprintf(stderr, "BTstack: Inquiry complete (%u devices found)\n",
                   inquiry_found_count_.load());
            fflush(stderr);
            inquiry_active_.store(false);
            signal_event(inquiry_event_, true);
            break;

        case HCI_EVENT_DISCONNECTION_COMPLETE: {
            uint8_t reason = hci_event_disconnection_complete_get_reason(packet);
            uint16_t handle = hci_event_disconnection_complete_get_connection_handle(packet);
            fprintf(stderr, "BTstack: HCI disconnection (handle=0x%04x, reason=0x%02x: %s)\n",
                   handle, reason, hci_error_string(reason));
            SetEvent(static_cast<HANDLE>(acl_down_event_));
            if (handle == target_acl_handle_.load()) {
                target_acl_down_reason_.store(reason);
                target_acl_handle_.store(0xFFFF);
            }
            uint16_t a2dp_handle = a2dp_con_handle_.load();
            if (a2dp_handle != 0xFFFF && handle != a2dp_handle) {
                fprintf(stderr, "BTstack: not the A2DP link (handle=0x%04x), stream unaffected\n", a2dp_handle);
                break;
            }
            a2dp_con_handle_.store(0xFFFF);
            /* Mark connection as lost — main loop will handle reconnect */
            if (connected_.load() || streaming_.load()) {
                streaming_.store(false);
                connected_.store(false);
                media_queue_count_.store(0);
                link_stats().queue_depth.store(0, std::memory_order_relaxed);
                disconnect_occurred_.store(true);
                fprintf(stderr, "BTstack: Connection lost — ready for reconnect\n");
            }
            break;
        }

        case HCI_EVENT_A2DP_META:
            handle_a2dp_event(packet, size);
            break;

        case HCI_EVENT_AVRCP_META:
            handle_avrcp_event(packet, size);
            break;

        default:
            break;
        }
    }
}

void BtStackTransport::handle_a2dp_event(uint8_t *packet, uint16_t size) {
    (void)size;
    uint8_t subevent = hci_event_a2dp_meta_get_subevent_code(packet);

    switch (subevent) {

    case A2DP_SUBEVENT_SIGNALING_CONNECTION_ESTABLISHED: {
        uint8_t status = a2dp_subevent_signaling_connection_established_get_status(packet);
        uint16_t established_cid = a2dp_subevent_signaling_connection_established_get_a2dp_cid(packet);
        if (a2dp_cid_ != 0 && established_cid != a2dp_cid_) {
            /* Late event from an old/aborted connection: must not clobber
             * the cid of the connection currently being established. */
            fprintf(stderr, "BTstack: Ignoring signaling event for stale cid=0x%04x (current=0x%04x, status=0x%02x)\n",
                    established_cid, a2dp_cid_, status);
            break;
        }
        a2dp_cid_ = established_cid;
        if (status != ERROR_CODE_SUCCESS) {
            fprintf(stderr, "BTstack: Signaling connection failed (0x%02x)\n", status);
            /* Page timeout: no answer. Connection timeout / L2CAP RTX timeout /
             * unspecified (a connection BTstack gave up on): the link came up
             * and then carried nothing. */
            last_connect_failure_.store(
                status == ERROR_CODE_PAGE_TIMEOUT ? ConnectFailure::NoAnswer :
                (status == L2CAP_CONNECTION_RESPONSE_RESULT_REFUSED_SECURITY ||
                 status == L2CAP_CONNECTION_PIN_OR_LINK_KEY_MISSING ||
                 status == ERROR_CODE_AUTHENTICATION_FAILURE ||
                 status == ERROR_CODE_PIN_OR_KEY_MISSING) ? ConnectFailure::AuthFailed :
                (status == ERROR_CODE_CONNECTION_TIMEOUT ||
                 status == L2CAP_CONNECTION_RESPONSE_RESULT_RTX_TIMEOUT ||
                 status == L2CAP_CONNECTION_BASEBAND_DISCONNECT ||
                 status == ERROR_CODE_UNSPECIFIED_ERROR) ? ConnectFailure::LinkLost
                                                         : ConnectFailure::Other);
            connect_result_.store(false);
            signal_event(connect_event_, false);
        } else {
            a2dp_con_handle_.store(a2dp_subevent_signaling_connection_established_get_con_handle(packet));
            fprintf(stderr, "BTstack: Signaling connection established (cid=0x%04x, handle=0x%04x)\n",
                    a2dp_cid_, a2dp_con_handle_.load());
            /* Don't signal yet — wait for capability discovery to complete */
        }
        break;
    }

    case A2DP_SUBEVENT_SIGNALING_MEDIA_CODEC_OTHER_CAPABILITY: {
        /* Remote device reports a vendor codec capability on one of its SEPs */
        uint8_t remote_seid =
            a2dp_subevent_signaling_media_codec_other_capability_get_remote_seid(packet);
        uint16_t info_len =
            a2dp_subevent_signaling_media_codec_other_capability_get_media_codec_information_len(packet);
        const uint8_t *info =
            a2dp_subevent_signaling_media_codec_other_capability_get_media_codec_information(packet);

        if (info_len >= 6) {
            uint32_t vid = read_vendor_id(info);
            uint16_t cid = read_codec_id(info);

            if (vid == LDAC_VENDOR_ID && cid == LDAC_CODEC_ID) {
                remote_caps_.ldac = true;
                remote_caps_.ldac_seid = remote_seid;
                fprintf(stderr, "BTstack: Remote supports LDAC (SEID=%u)\n", remote_seid);
            } else if (vid == APTXHD_VENDOR_ID && cid == APTXHD_CODEC_ID) {
                /* 11 bytes expected (7 + 4 reserved); only byte 6 matters */
                uint8_t hd_caps = (info_len >= 7) ? info[6] : 0;
                bool usable = (info_len < 7) || (hd_caps & APTX_CH_STEREO);
                if (usable) {
                    remote_caps_.aptx_hd = true;
                    remote_caps_.aptxhd_seid = remote_seid;
                    remote_caps_.aptxhd_caps = hd_caps;
                }
                fprintf(stderr, "BTstack: Remote supports aptX HD (SEID=%u, caps=0x%02x, len=%u)%s\n",
                        remote_seid, hd_caps, info_len, usable ? "" : " - no stereo, ignored");
            } else if ((vid == APTXLL_VENDOR_ID || vid == APTXLL_VENDOR_ID2) &&
                       cid == APTXLL_CODEC_ID) {
                /* 8 bytes (17 with has_new_caps); byte 6 as classic aptX */
                uint8_t ll_caps = (info_len >= 7) ? info[6] : 0;
                uint8_t ll_flags = (info_len >= 8) ? info[7] : 0;
                bool usable = (info_len < 7) || (ll_caps & APTX_CH_STEREO);
                /* Keep the first usable LL SEP if listed under both IDs */
                if (usable && !remote_caps_.aptx_ll) {
                    remote_caps_.aptx_ll = true;
                    remote_caps_.aptxll_seid = remote_seid;
                    remote_caps_.aptxll_caps = ll_caps;
                    remote_caps_.aptxll_vendor_id = vid;
                    uint16_t n = info_len;
                    if (n > sizeof(remote_caps_.aptxll_info)) n = sizeof(remote_caps_.aptxll_info);
                    memcpy(remote_caps_.aptxll_info, info, n);
                    remote_caps_.aptxll_info_len = (uint8_t)n;
                }
                fprintf(stderr, "BTstack: Remote supports aptX LL (SEID=%u, vid=0x%02X, caps=0x%02x, "
                        "flags=0x%02x, len=%u)%s\n",
                        remote_seid, (unsigned)vid, ll_caps, ll_flags, info_len,
                        usable ? "" : " - no stereo, ignored");
            } else if (vid == APTXAD_VENDOR_ID && cid == APTXAD_CODEC_ID) {
                remote_caps_.aptx_adaptive = true;
                remote_caps_.aptx_adaptive_seid = remote_seid;
                fprintf(stderr, "BTstack: Remote supports aptX Adaptive (SEID=%u) — never selected "
                        "(no open encoder); classic aptX is used only if the remote lists it\n",
                        remote_seid);
            } else if (vid == APTX_VENDOR_ID && cid == APTX_CODEC_ID) {
                uint8_t aptx_caps = (info_len >= 7) ? info[6] : 0;
                /* Stereo is the only mode we send; skip mono-only sinks */
                if (info_len < 7 || (aptx_caps & APTX_CH_STEREO)) {
                    remote_caps_.aptx = true;
                    remote_caps_.aptx_seid = remote_seid;
                    remote_caps_.aptx_caps = aptx_caps;
                }
                fprintf(stderr, "BTstack: Remote supports aptX (SEID=%u, caps=0x%02x)%s\n",
                        remote_seid, aptx_caps,
                        remote_caps_.aptx ? "" : " - no stereo, ignored");
            } else {
                fprintf(stderr, "BTstack: Remote vendor codec vid=0x%08X cid=0x%04X (SEID=%u) — unsupported\n",
                        (unsigned)vid, (unsigned)cid, remote_seid);
            }
        }
        break;
    }

    case A2DP_SUBEVENT_SIGNALING_MEDIA_CODEC_SBC_CAPABILITY: {
        uint8_t remote_seid =
            a2dp_subevent_signaling_media_codec_sbc_capability_get_remote_seid(packet);
        remote_caps_.sbc = true;
        remote_caps_.sbc_seid = remote_seid;
        fprintf(stderr, "BTstack: Remote supports SBC (SEID=%u)\n", remote_seid);
        break;
    }

    case A2DP_SUBEVENT_SIGNALING_MEDIA_CODEC_MPEG_AAC_CAPABILITY: {
        uint8_t remote_seid =
            a2dp_subevent_signaling_media_codec_mpeg_aac_capability_get_remote_seid(packet);
        remote_caps_.aac = true;
        remote_caps_.aac_seid = remote_seid;
        fprintf(stderr, "BTstack: Remote supports AAC (SEID=%u)\n", remote_seid);
        break;
    }

    case A2DP_SUBEVENT_SIGNALING_CAPABILITIES_COMPLETE: {
        /* All SEP capabilities have been discovered */
        fprintf(stderr, "BTstack: Capability discovery complete (LDAC=%d, aptXHD=%d, aptXLL=%d, aptX=%d, "
               "aptXAdaptive=%d [not encodable], SBC=%d, AAC=%d)\n",
               remote_caps_.ldac, remote_caps_.aptx_hd, remote_caps_.aptx_ll, remote_caps_.aptx,
               remote_caps_.aptx_adaptive, remote_caps_.sbc, remote_caps_.aac);
        connected_.store(true);
        connect_result_.store(true);
        signal_event(connect_event_, true);

        /* Establish AVRCP connection for volume control */
        if (has_remote_addr_) {
            uint16_t avrcp_cid_tmp = 0;
            uint8_t rc = avrcp_connect(remote_addr_be_, &avrcp_cid_tmp);
            if (rc != ERROR_CODE_SUCCESS) {
                fprintf(stderr, "BTstack: AVRCP connect request failed (0x%02x)\n", rc);
            }
        }
        break;
    }

    case A2DP_SUBEVENT_SIGNALING_MEDIA_CODEC_SBC_CONFIGURATION: {
        uint8_t local_seid =
            a2dp_subevent_signaling_media_codec_sbc_configuration_get_local_seid(packet);
        uint8_t remote_seid =
            a2dp_subevent_signaling_media_codec_sbc_configuration_get_remote_seid(packet);
        fprintf(stderr, "BTstack: SBC configured (local=%u, remote=%u)\n", local_seid, remote_seid);
        break;
    }

    case A2DP_SUBEVENT_SIGNALING_MEDIA_CODEC_MPEG_AAC_CONFIGURATION: {
        uint8_t local_seid =
            a2dp_subevent_signaling_media_codec_mpeg_aac_configuration_get_local_seid(packet);
        uint8_t remote_seid =
            a2dp_subevent_signaling_media_codec_mpeg_aac_configuration_get_remote_seid(packet);
        fprintf(stderr, "BTstack: AAC configured (local=%u, remote=%u)\n", local_seid, remote_seid);
        break;
    }

    case A2DP_SUBEVENT_SIGNALING_MEDIA_CODEC_OTHER_CONFIGURATION: {
        /* Vendor codec configuration has been set */
        uint8_t local_seid =
            a2dp_subevent_signaling_media_codec_other_configuration_get_local_seid(packet);
        uint8_t remote_seid =
            a2dp_subevent_signaling_media_codec_other_configuration_get_remote_seid(packet);
        fprintf(stderr, "BTstack: Codec configured (local=%u, remote=%u)\n", local_seid, remote_seid);
        break;
    }

    case A2DP_SUBEVENT_STREAM_ESTABLISHED: {
        uint8_t status = a2dp_subevent_stream_established_get_status(packet);
        if (status != ERROR_CODE_SUCCESS) {
            fprintf(stderr, "BTstack: Stream establishment failed (0x%02x)\n", status);
            stream_result_.store(false);
        } else {
            local_seid_ = a2dp_subevent_stream_established_get_local_seid(packet);
            remote_seid_ = a2dp_subevent_stream_established_get_remote_seid(packet);
            /* Cache media MTU on the BTstack thread (safe to call here) */
            int max = a2dp_max_media_payload_size(a2dp_cid_, local_seid_);
            uint16_t remote_mtu = (max > 0) ? (uint16_t)max : 679;
            /* Callers size packets and the SBC / LDAC encoders from
             * get_media_mtu(); the limit also keeps every payload within a
             * MAX_MEDIA_PACKET_SIZE queue slot (send_media() drops larger ones) */
            uint16_t limit = media_payload_limit_.load();
            media_mtu_ = (remote_mtu > limit) ? limit : remote_mtu;
            fprintf(stderr, "BTstack: Stream established (local=%u, remote=%u, mtu=%u, used=%u, limit=%u)\n",
                   local_seid_, remote_seid_, remote_mtu, media_mtu_, limit);
            stream_result_.store(true);
        }
        signal_event(stream_event_, stream_result_.load());
        break;
    }

    case A2DP_SUBEVENT_STREAM_STARTED: {
        fprintf(stderr, "BTstack: Streaming started\n");
        {
            LinkStats &ls = link_stats();
            ls.start_packets.store(ls.packets_sent.load());
            ls.start_bytes.store(ls.bytes_sent.load());
            ls.start_dropped.store(ls.queue_drops.load() + ls.send_errors.load());
            ls.start_capture_frames.store(ls.capture_frames.load());
            ls.start_capture_dropped.store(ls.capture_dropped_frames.load());
            ls.stream_starts.fetch_add(1);
        }
        streaming_.store(true);
        start_result_.store(true);
        signal_event(start_event_, true);

        /* Request first can-send-now to kick off media sending */
        a2dp_source_stream_endpoint_request_can_send_now(a2dp_cid_, local_seid_);
        break;
    }

    case A2DP_SUBEVENT_STREAMING_CAN_SEND_MEDIA_PACKET_NOW:
        /* Dequeue one media packet, then send it OUTSIDE the lock
         * so send_mutex isn't held during the L2CAP write (which can
         * block the WASAPI thread trying to enqueue in send_media). */
        {
            MediaPacket pkt;
            bool have_packet = false;
            {
                std::lock_guard<std::mutex> lock(send_mutex_);
                if (media_queue_count_.load() > 0) {
                    pkt = media_queue_[media_queue_tail_];
                    media_queue_tail_ = (media_queue_tail_ + 1) % MEDIA_QUEUE_CAPACITY;
                    int depth = media_queue_count_.fetch_sub(1) - 1;
                    link_stats().queue_depth.store(static_cast<uint32_t>(depth), std::memory_order_relaxed);
                    have_packet = true;
                }
            }
            if (have_packet) {
                /* max media payload = remote L2CAP MTU - 12-byte RTP header */
                int max_payload = a2dp_max_media_payload_size(a2dp_cid_, local_seid_);
                if (pkt.no_rtp && max_payload > 0)
                    max_payload += (int)RTP_HEADER_SIZE;
                if (max_payload > 0 && pkt.size <= (uint32_t)max_payload) {
                    uint8_t status = pkt.no_rtp
                        ? a2dp_source_stream_send_media_packet(
                              a2dp_cid_, local_seid_, pkt.data, (uint16_t)pkt.size)
                        : a2dp_source_stream_send_media_payload_rtp(
                              a2dp_cid_, local_seid_, 0 /* marker */,
                              pkt.timestamp,
                              pkt.data, (uint16_t)pkt.size);
                    LinkStats &stats = link_stats();
                    if (status != ERROR_CODE_SUCCESS) {
                        send_failure_count_.fetch_add(1);
                        stats.send_errors.fetch_add(1, std::memory_order_relaxed);
                    } else {
                        stats.packets_sent.fetch_add(1, std::memory_order_relaxed);
                        stats.bytes_sent.fetch_add(pkt.size + (pkt.no_rtp ? 0 : RTP_HEADER_SIZE),
                                                   std::memory_order_relaxed);
                    }
                } else {
                    send_failure_count_.fetch_add(1);
                    link_stats().send_errors.fetch_add(1, std::memory_order_relaxed);
                }
            }
            /* Chain next CAN_SEND_NOW if queue still has data.
             * If empty, send_media() will trigger when new data arrives. */
            if (media_queue_count_.load() > 0) {
                a2dp_source_stream_endpoint_request_can_send_now(a2dp_cid_, local_seid_);
            }
        }
        break;

    case A2DP_SUBEVENT_STREAM_SUSPENDED:
        fprintf(stderr, "BTstack: Stream suspended\n");
        streaming_.store(false);
        break;

    case A2DP_SUBEVENT_STREAM_RELEASED:
        fprintf(stderr, "BTstack: Stream released\n");
        streaming_.store(false);
        break;

    case A2DP_SUBEVENT_SIGNALING_CONNECTION_RELEASED: {
        uint16_t released_cid =
            a2dp_subevent_signaling_connection_released_get_a2dp_cid(packet);
        if (a2dp_cid_ != 0 && released_cid != a2dp_cid_) {
            /* Late release of an old/aborted connection (e.g. after
             * abort_pending_connection() gave up waiting): it must not
             * clobber the newer connection's state. */
            fprintf(stderr, "BTstack: Ignoring signaling release for stale cid=0x%04x (current=0x%04x)\n",
                    released_cid, a2dp_cid_);
            break;
        }
        if (a2dp_cid_ == 0) {
            /* No current connection (worker already tore it down or is
             * about to establish a new one): wake a pending disconnect wait
             * and drop the old AVRCP link, but don't report a connection loss
             * to the streaming loop. */
            fprintf(stderr, "BTstack: Signaling connection released (cid=0x%04x, no current connection)\n",
                    released_cid);
            if (avrcp_cid_) {
                avrcp_disconnect(avrcp_cid_);
                avrcp_cid_ = 0;
            }
            signal_event(disconnect_event_, true);
            break;
        }
        fprintf(stderr, "BTstack: Signaling connection released (cid=0x%04x)\n", released_cid);
        connected_.store(false);
        streaming_.store(false);
        media_queue_count_.store(0);
        link_stats().queue_depth.store(0, std::memory_order_relaxed);
        a2dp_cid_ = 0;
        if (avrcp_cid_) {
            avrcp_disconnect(avrcp_cid_);
            avrcp_cid_ = 0;
        }
        disconnect_occurred_.store(true);
        signal_event(disconnect_event_, true);
        break;
    }

    case A2DP_SUBEVENT_COMMAND_REJECTED:
        fprintf(stderr, "BTstack: A2DP command rejected\n");
        break;

    default:
        break;
    }
}

void BtStackTransport::handle_avrcp_event(uint8_t *packet, uint16_t size) {
    (void)size;
    uint8_t subevent = packet[2];

    switch (subevent) {
    case AVRCP_SUBEVENT_CONNECTION_ESTABLISHED: {
        uint8_t status = avrcp_subevent_connection_established_get_status(packet);
        uint16_t cid = avrcp_subevent_connection_established_get_avrcp_cid(packet);
        if (status != ERROR_CODE_SUCCESS) {
            fprintf(stderr, "BTstack: AVRCP connection failed (0x%02x)\n", status);
            break;
        }
        avrcp_cid_ = cid;
        fprintf(stderr, "BTstack: AVRCP connected (cid=0x%04x)\n", cid);

        /* Subscribe to volume change notifications */
        avrcp_controller_enable_notification(avrcp_cid_,
            AVRCP_NOTIFICATION_EVENT_VOLUME_CHANGED);
        break;
    }

    case AVRCP_SUBEVENT_CONNECTION_RELEASED:
        fprintf(stderr, "BTstack: AVRCP disconnected (cid=0x%04x)\n",
                avrcp_subevent_connection_released_get_avrcp_cid(packet));
        avrcp_cid_ = 0;
        break;

    case AVRCP_SUBEVENT_NOTIFICATION_VOLUME_CHANGED: {
        uint8_t vol = avrcp_subevent_notification_volume_changed_get_absolute_volume(packet);
        fprintf(stderr, "BTstack: AVRCP volume changed to %u (%u%%)\n",
                vol, vol * 100 / 127);

        /* Re-register notification (AVRCP spec requires re-subscribing after each) */
        avrcp_controller_enable_notification(avrcp_cid_,
            AVRCP_NOTIFICATION_EVENT_VOLUME_CHANGED);
        break;
    }

    case AVRCP_SUBEVENT_SET_ABSOLUTE_VOLUME_RESPONSE: {
        uint8_t vol = avrcp_subevent_set_absolute_volume_response_get_absolute_volume(packet);
        fprintf(stderr, "BTstack: AVRCP absolute volume confirmed: %u (%u%%)\n",
                vol, vol * 100 / 127);
        break;
    }

    default:
        break;
    }
}

/* ======================================================================== */
/* Synchronization Helpers                                                  */
/* ======================================================================== */

void BtStackTransport::signal_event(void *event_handle, bool success) {
    (void)success;
    SetEvent(static_cast<HANDLE>(event_handle));
}

bool BtStackTransport::wait_for_event(void *event_handle, uint32_t timeout_ms) {
    DWORD before = GetTickCount();
    HANDLE handles[2] = { static_cast<HANDLE>(event_handle), static_cast<HANDLE>(cancel_event_) };
    DWORD result = WaitForMultipleObjects(2, handles, FALSE, timeout_ms);
    DWORD elapsed = GetTickCount() - before;
    fprintf(stderr, "BTstack: wait_for_event handle=%p result=%lu elapsed=%lu ms\n",
            event_handle, result, elapsed);
    fflush(stderr);
    return (result == WAIT_OBJECT_0);  /* only first handle = success */
}

/* ======================================================================== */
/* Codec Capability Builders (for endpoint registration)                    */
/* ======================================================================== */

void BtStackTransport::build_ldac_capabilities(uint8_t *caps, uint16_t *len,
                                                uint8_t *config, uint16_t *config_len) {
    write_vendor_codec_id(caps, LDAC_VENDOR_ID, LDAC_CODEC_ID);
    caps[6] = LDAC_FREQ_ALL;
    caps[7] = LDAC_CH_ALL;
    *len = 8;
    write_vendor_codec_id(config, LDAC_VENDOR_ID, LDAC_CODEC_ID);
    config[6] = LDAC_FREQ_48K;
    config[7] = LDAC_CH_STEREO;
    *config_len = 8;
}

/* caps/config must hold APTXHD_INFO_LEN (11) bytes */
void BtStackTransport::build_aptxhd_capabilities(uint8_t *caps, uint16_t *len,
                                                   uint8_t *config, uint16_t *config_len) {
    memset(caps, 0, APTXHD_INFO_LEN);
    write_vendor_codec_id(caps, APTXHD_VENDOR_ID, APTXHD_CODEC_ID);
    caps[6] = APTXHD_CAPS_ALL;
    *len = APTXHD_INFO_LEN;
    memset(config, 0, APTXHD_INFO_LEN);
    write_vendor_codec_id(config, APTXHD_VENDOR_ID, APTXHD_CODEC_ID);
    config[6] = APTXHD_CONFIG_DEFAULT;
    *config_len = APTXHD_INFO_LEN;
}

/* caps/config must hold APTXLL_INFO_LEN (8) bytes */
void BtStackTransport::build_aptxll_capabilities(uint8_t *caps, uint16_t *len,
                                                   uint8_t *config, uint16_t *config_len) {
    memset(caps, 0, APTXLL_INFO_LEN);
    write_vendor_codec_id(caps, APTXLL_VENDOR_ID, APTXLL_CODEC_ID);
    caps[6] = APTXLL_CAPS_ALL;
    *len = APTXLL_INFO_LEN;
    memset(config, 0, APTXLL_INFO_LEN);
    write_vendor_codec_id(config, APTXLL_VENDOR_ID, APTXLL_CODEC_ID);
    config[6] = APTXLL_CONFIG_DEFAULT;
    *config_len = APTXLL_INFO_LEN;
}

