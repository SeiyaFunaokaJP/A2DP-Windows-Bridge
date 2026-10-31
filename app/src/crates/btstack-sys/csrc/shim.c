/*
 * shim.c - C glue between BTstack and the Rust side of btstack-sys
 *
 * Holds everything whose layout is BTstack's business: the static structs
 * BTstack keeps pointers to (registrations, timers, SDP record buffers,
 * stream endpoint capabilities, the codec configuration of
 * a2dp_source_set_config_other; for LE: the TLV store, per-connection GATT
 * client listeners and write buffers, ATT send registrations, CIG storage),
 * the function tables Rust implements (HCI transport, link key DB, HCI dump,
 * ATT server read / write callbacks), and reads of struct members
 * (hci_connection_t, avdtp_connection_t). No logic of its own beyond the
 * bookkeeping of that storage: every decision is made in Rust. Runs on the
 * BTstack run loop thread only, except bluecaster_wake().
 *
 * SPDX-License-Identifier: Apache-2.0 OR MIT
 */

#include "btstack_config.h"

#include <stdarg.h>
#include <stdio.h>
#include <string.h>
#include <wchar.h>
#include <windows.h>

#include "bluetooth.h"
#include "btstack_defines.h"
#include "btstack_memory.h"
#include "btstack_run_loop.h"
#include "btstack_run_loop_windows.h"
#include "gap.h"
#include "hci.h"
#include "hci_cmd.h"
#include "hci_dump.h"
#include "hci_transport.h"
#include "l2cap.h"
#include "classic/a2dp_source.h"
#include "classic/avdtp.h"
#include "classic/avdtp_sink.h"
#include "classic/avdtp_util.h"
#include "classic/avrcp.h"
#include "classic/avrcp_controller.h"
#include "classic/avrcp_target.h"
#include "classic/btstack_link_key_db.h"
#include "classic/hfp_ag.h"
#include "classic/rfcomm.h"
#include "classic/btstack_link_key_db_memory.h"
#include "classic/sdp_client.h"
#include "classic/sdp_client_rfcomm.h"
#include "classic/sdp_server.h"
#include "classic/sdp_util.h"
#include "btstack_chipset_intel_firmware.h"
#include "btstack_tlv.h"
#include "btstack_tlv_windows.h"
#include "ble/att_db.h"
#include "ble/att_db_util.h"
#include "ble/att_server.h"
#include "ble/gatt_client.h"
#include "ble/le_device_db.h"
#include "ble/le_device_db_tlv.h"
#include "ble/sm.h"

/* ---- Implemented in Rust (btstack-sys/src/callbacks.rs) ---- */
void bluecaster_rs_on_wake(void);
void bluecaster_rs_on_timer(uint32_t id);
void bluecaster_rs_on_packet(uint8_t source, uint8_t packet_type, uint16_t channel, uint8_t *packet, uint16_t size);
int bluecaster_rs_transport_open(void);
int bluecaster_rs_transport_close(void);
int bluecaster_rs_transport_can_send(uint8_t packet_type);
int bluecaster_rs_transport_send(uint8_t packet_type, const uint8_t *packet, int size);
void bluecaster_rs_log_packet(uint8_t packet_type, uint8_t in, const uint8_t *packet, uint16_t len);
void bluecaster_rs_log_message(int level, const char *message);
int bluecaster_rs_lk_get(const uint8_t *addr, uint8_t *key, int *type);
void bluecaster_rs_lk_put(const uint8_t *addr, const uint8_t *key, int type);
void bluecaster_rs_lk_delete(const uint8_t *addr);
int bluecaster_rs_lk_at(int index, uint8_t *addr, uint8_t *key, int *type);
void bluecaster_rs_intel_done(int result);
uint16_t bluecaster_rs_att_read(uint16_t con, uint16_t handle, uint16_t offset, uint8_t *buffer, uint16_t buffer_size);
int bluecaster_rs_att_write(uint16_t con, uint16_t handle, uint16_t mode, uint16_t offset, const uint8_t *buffer,
                        uint16_t size);
void bluecaster_rs_att_can_send(uint16_t con);
void bluecaster_rs_le_disconnected(uint16_t con);

static void cig_slot_release(uint8_t cig_id);

/* ======================================================================== */
/* Run loop: wake-up from other threads, timers                             */
/* ======================================================================== */

static btstack_context_callback_registration_t wake_registration;
static volatile LONG wake_pending;

static void wake_callback(void *context) {
    (void)context;
    /* Cleared first: a wake that comes while Rust runs queues a new callback */
    InterlockedExchange(&wake_pending, 0);
    bluecaster_rs_on_wake();
}

void bluecaster_runloop_init(void) {
    btstack_run_loop_init(btstack_run_loop_windows_get_instance());
    memset(&wake_registration, 0, sizeof(wake_registration));
    wake_registration.callback = &wake_callback;
    InterlockedExchange(&wake_pending, 0);
}

/* Any thread. One callback in the run loop's list at a time: the
 * registration is a list element and must not be added twice. */
void bluecaster_wake(void) {
    if (InterlockedExchange(&wake_pending, 1) == 0) {
        btstack_run_loop_execute_on_main_thread(&wake_registration);
    }
}

#define BLUECASTER_MAX_TIMERS 16
static btstack_timer_source_t timers[BLUECASTER_MAX_TIMERS];

static void timer_callback(btstack_timer_source_t *ts) {
    bluecaster_rs_on_timer((uint32_t)(uintptr_t)btstack_run_loop_get_timer_context(ts));
}

int bluecaster_timer_start(uint32_t id, uint32_t timeout_ms) {
    if (id >= BLUECASTER_MAX_TIMERS) return 0;
    btstack_timer_source_t *t = &timers[id];
    btstack_run_loop_remove_timer(t);
    btstack_run_loop_set_timer_handler(t, &timer_callback);
    btstack_run_loop_set_timer_context(t, (void *)(uintptr_t)id);
    btstack_run_loop_set_timer(t, timeout_ms);
    btstack_run_loop_add_timer(t);
    return 1;
}

void bluecaster_timer_stop(uint32_t id) {
    if (id >= BLUECASTER_MAX_TIMERS) return;
    btstack_run_loop_remove_timer(&timers[id]);
}

void bluecaster_timers_clear(void) {
    for (int i = 0; i < BLUECASTER_MAX_TIMERS; i++) btstack_run_loop_remove_timer(&timers[i]);
}

/* ======================================================================== */
/* HCI transport implemented in Rust                                        */
/* ======================================================================== */

static void (*transport_packet_handler)(uint8_t packet_type, uint8_t *packet, uint16_t size);
/* Incoming packets are copied here with BTstack's pre-buffer in front */
static uint8_t transport_rx_buffer[HCI_INCOMING_PRE_BUFFER_SIZE + HCI_ACL_PAYLOAD_SIZE + 4 + 260];

static void transport_init(const void *config) { (void)config; }
static int transport_open(void) { return bluecaster_rs_transport_open(); }
static int transport_close(void) { return bluecaster_rs_transport_close(); }
static void transport_register_packet_handler(void (*handler)(uint8_t, uint8_t *, uint16_t)) {
    transport_packet_handler = handler;
}
static int transport_can_send_packet_now(uint8_t packet_type) {
    return bluecaster_rs_transport_can_send(packet_type);
}
static int transport_send_packet(uint8_t packet_type, uint8_t *packet, int size) {
    /* LE Remove CIG: hci.c took the CIG out of its list when it sent this,
     * its storage may be used again */
    if (packet_type == HCI_COMMAND_DATA_PACKET && size >= 4 &&
        little_endian_read_16(packet, 0) == HCI_OPCODE_HCI_LE_REMOVE_CIG) {
        cig_slot_release(packet[3]);
    }
    return bluecaster_rs_transport_send(packet_type, packet, size);
}

/* Synchronous (send_packet returns when the packet is out, e.g. TCP): no
 * can_send_packet_now, HCI frees its buffer at once */
static const hci_transport_t transport_sync = {
    "bluecaster-sync", &transport_init, &transport_open, &transport_close,
    &transport_register_packet_handler, NULL, &transport_send_packet, NULL, NULL, NULL};
/* Asynchronous (USB): HCI waits for HCI_EVENT_TRANSPORT_PACKET_SENT */
static const hci_transport_t transport_async = {
    "bluecaster-async", &transport_init, &transport_open, &transport_close,
    &transport_register_packet_handler, &transport_can_send_packet_now, &transport_send_packet,
    NULL, NULL, NULL};

const hci_transport_t *bluecaster_transport_instance(int async) {
    return async ? &transport_async : &transport_sync;
}

/* Returns 0 if the packet does not fit or no handler is registered */
int bluecaster_transport_deliver(uint8_t packet_type, const uint8_t *packet, uint16_t size) {
    if (transport_packet_handler == NULL) return 0;
    if ((size_t)size > sizeof(transport_rx_buffer) - HCI_INCOMING_PRE_BUFFER_SIZE) return 0;
    uint8_t *p = &transport_rx_buffer[HCI_INCOMING_PRE_BUFFER_SIZE];
    memcpy(p, packet, size);
    transport_packet_handler(packet_type, p, size);
    return 1;
}

void bluecaster_transport_packet_sent(void) {
    static uint8_t sent[] = {HCI_EVENT_TRANSPORT_PACKET_SENT, 0};
    if (transport_packet_handler) transport_packet_handler(HCI_EVENT_PACKET, sent, sizeof(sent));
}

void bluecaster_hci_init(int async) {
    transport_packet_handler = NULL;
    hci_init(bluecaster_transport_instance(async), NULL);
}

/* Intel bootloader firmware download: runs on the transport before hci_init */
static void intel_done(int result) { bluecaster_rs_intel_done(result); }

void bluecaster_intel_download_firmware(int async, const char *firmware_path) {
    btstack_chipset_intel_set_firmware_path(firmware_path);
    btstack_chipset_intel_download_firmware(bluecaster_transport_instance(async), &intel_done);
}

/* ======================================================================== */
/* HCI dump: packets and BTstack's log to Rust                              */
/* ======================================================================== */

static void dump_reset(void) {}

static void dump_log_packet(uint8_t packet_type, uint8_t in, uint8_t *packet, uint16_t len) {
    bluecaster_rs_log_packet(packet_type, in, packet, len);
}

static void dump_log_message(int log_level, const char *format, va_list argptr) {
    char buffer[512];
    vsnprintf(buffer, sizeof(buffer), format, argptr);
    bluecaster_rs_log_message(log_level, buffer);
}

static const hci_dump_t hci_dump_rust = {&dump_reset, &dump_log_packet, &dump_log_message};

void bluecaster_hci_dump_init(void) {
    hci_dump_init(&hci_dump_rust);
}

/* ======================================================================== */
/* Link key DB implemented in Rust                                          */
/* ======================================================================== */

static void lk_open(void) {}
static void lk_set_local_bd_addr(bd_addr_t bd_addr) { (void)bd_addr; }
static void lk_close(void) {}

static int lk_get_link_key(bd_addr_t bd_addr, link_key_t link_key, link_key_type_t *type) {
    int t = 0;
    if (!bluecaster_rs_lk_get(bd_addr, link_key, &t)) return 0;
    *type = (link_key_type_t)t;
    return 1;
}

static void lk_put_link_key(bd_addr_t bd_addr, link_key_t link_key, link_key_type_t type) {
    bluecaster_rs_lk_put(bd_addr, link_key, (int)type);
}

static void lk_delete_link_key(bd_addr_t bd_addr) { bluecaster_rs_lk_delete(bd_addr); }

/* The iterator is an index into the Rust store (deleting while iterating
 * is allowed and shifts the entries, as in v1's file DB) */
static int lk_iterator_init(btstack_link_key_iterator_t *it) {
    it->context = (void *)(intptr_t)0;
    return 1;
}

static int lk_iterator_get_next(btstack_link_key_iterator_t *it, bd_addr_t bd_addr, link_key_t link_key,
                                link_key_type_t *type) {
    int index = (int)(intptr_t)it->context;
    int t = 0;
    if (!bluecaster_rs_lk_at(index, bd_addr, link_key, &t)) return 0;
    it->context = (void *)(intptr_t)(index + 1);
    *type = (link_key_type_t)t;
    return 1;
}

static void lk_iterator_done(btstack_link_key_iterator_t *it) { it->context = NULL; }

static const btstack_link_key_db_t link_key_db_rust = {
    &lk_open, &lk_set_local_bd_addr, &lk_close, &lk_get_link_key, &lk_put_link_key,
    &lk_delete_link_key, &lk_iterator_init, &lk_iterator_get_next, &lk_iterator_done};

void bluecaster_set_link_key_db(int rust_store) {
    hci_set_link_key_db(rust_store ? &link_key_db_rust : btstack_link_key_db_memory_instance());
}

/* ======================================================================== */
/* Packet handlers                                                          */
/* ======================================================================== */

enum {
    SOURCE_HCI = 0,
    SOURCE_A2DP = 1,
    SOURCE_AVDTP_SINK = 2,
    SOURCE_AVRCP = 3,
    SOURCE_AVRCP_CONTROLLER = 4,
    SOURCE_AVRCP_TARGET = 5,
    SOURCE_SM = 6,
    SOURCE_GATT_CLIENT = 7,
    SOURCE_ATT_SERVER = 8,
    SOURCE_ISO = 9,
    SOURCE_HFP = 10,
    SOURCE_RFCOMM = 11,
    SOURCE_SDP_CLIENT = 12,
};

static void handler_hci(uint8_t t, uint16_t c, uint8_t *p, uint16_t s) { bluecaster_rs_on_packet(SOURCE_HCI, t, c, p, s); }
static void handler_a2dp(uint8_t t, uint16_t c, uint8_t *p, uint16_t s) { bluecaster_rs_on_packet(SOURCE_A2DP, t, c, p, s); }
static void handler_avdtp_sink(uint8_t t, uint16_t c, uint8_t *p, uint16_t s) { bluecaster_rs_on_packet(SOURCE_AVDTP_SINK, t, c, p, s); }
static void handler_avrcp(uint8_t t, uint16_t c, uint8_t *p, uint16_t s) { bluecaster_rs_on_packet(SOURCE_AVRCP, t, c, p, s); }
static void handler_avrcp_ct(uint8_t t, uint16_t c, uint8_t *p, uint16_t s) { bluecaster_rs_on_packet(SOURCE_AVRCP_CONTROLLER, t, c, p, s); }
static void handler_avrcp_tg(uint8_t t, uint16_t c, uint8_t *p, uint16_t s) { bluecaster_rs_on_packet(SOURCE_AVRCP_TARGET, t, c, p, s); }
static void handler_sm(uint8_t t, uint16_t c, uint8_t *p, uint16_t s) { bluecaster_rs_on_packet(SOURCE_SM, t, c, p, s); }
static void handler_gatt_client(uint8_t t, uint16_t c, uint8_t *p, uint16_t s) { bluecaster_rs_on_packet(SOURCE_GATT_CLIENT, t, c, p, s); }
static void handler_att_server(uint8_t t, uint16_t c, uint8_t *p, uint16_t s) { bluecaster_rs_on_packet(SOURCE_ATT_SERVER, t, c, p, s); }
static void handler_hfp(uint8_t t, uint16_t c, uint8_t *p, uint16_t s) { bluecaster_rs_on_packet(SOURCE_HFP, t, c, p, s); }
static void handler_iso(uint8_t t, uint16_t c, uint8_t *p, uint16_t s) { bluecaster_rs_on_packet(SOURCE_ISO, t, c, p, s); }
static void handler_rfcomm(uint8_t t, uint16_t c, uint8_t *p, uint16_t s) { bluecaster_rs_on_packet(SOURCE_RFCOMM, t, c, p, s); }
static void handler_sdp_client(uint8_t t, uint16_t c, uint8_t *p, uint16_t s) { bluecaster_rs_on_packet(SOURCE_SDP_CLIENT, t, c, p, s); }

static btstack_packet_callback_registration_t hci_event_registration;

void bluecaster_add_hci_event_handler(void) {
    memset(&hci_event_registration, 0, sizeof(hci_event_registration));
    hci_event_registration.callback = &handler_hci;
    hci_add_event_handler(&hci_event_registration);
}

void bluecaster_a2dp_source_init(void) {
    a2dp_source_init();
    a2dp_source_register_packet_handler(&handler_a2dp);
    /* No sink SEPs are registered, so the AVDTP sink callback only gets the
     * events AVDTP emits to both roles, SEP discovery among them */
    avdtp_register_sink_packet_handler(&handler_avdtp_sink);
}

void bluecaster_avrcp_init(void) {
    avrcp_init();
    avrcp_register_packet_handler(&handler_avrcp);
    avrcp_controller_init();
    avrcp_controller_register_packet_handler(&handler_avrcp_ct);
    avrcp_target_init();
    avrcp_target_register_packet_handler(&handler_avrcp_tg);
}

/* ======================================================================== */
/* HFP audio gateway: service level connection only (headphone battery)     */
/* ======================================================================== */

#define BLUECASTER_HFP_RFCOMM_CHANNEL 4

static uint8_t sdp_hfp_ag_service[200];
static uint8_t hfp_codecs[] = { HFP_CODEC_CVSD };
static hfp_ag_indicator_t hfp_ag_indicators[] = {
    /* index, name, min, max, status, mandatory, enabled, status changed */
    {1, "service",   0, 1, 1, 0, 0, 0},
    {2, "call",      0, 1, 0, 1, 1, 0},
    {3, "callsetup", 0, 3, 0, 1, 1, 0},
    {4, "battchg",   0, 5, 5, 0, 0, 0},
    {5, "signal",    0, 5, 5, 0, 1, 0},
    {6, "roam",      0, 1, 0, 0, 1, 0},
    {7, "callheld",  0, 2, 0, 1, 1, 0}
};
/* HF indicators: enhanced safety, battery level (the headphones report it
 * with AT+BIEV) */
static hfp_generic_status_indicator_t hfp_hf_indicators[] = { {1, 1}, {2, 1} };
static const char *hfp_call_hold_services[] = {"1", "2"};

void bluecaster_hfp_ag_init(void) {
    uint16_t features = (1 << HFP_AGSF_HF_INDICATORS) | (1 << HFP_AGSF_EXTENDED_ERROR_RESULT_CODES);
    hfp_ag_init(BLUECASTER_HFP_RFCOMM_CHANNEL);
    hfp_ag_init_supported_features(features);
    hfp_ag_init_codecs(sizeof(hfp_codecs), hfp_codecs);
    hfp_ag_init_ag_indicators(sizeof(hfp_ag_indicators) / sizeof(hfp_ag_indicators[0]), hfp_ag_indicators);
    hfp_ag_init_hf_indicators(sizeof(hfp_hf_indicators) / sizeof(hfp_hf_indicators[0]), hfp_hf_indicators);
    hfp_ag_init_call_hold_services(2, hfp_call_hold_services);
    /* Apple accessories (Sony, Bose...) report their battery with
     * AT+IPHONEACCEV once the AG answers AT+XAPL: as an iPhone does, with
     * the battery (2) and docking (4) features. Sony's WH-1000XM4 sends
     * nothing when the AG gives another name or no features. */
    hfp_ag_init_apple_identification("iPhone", 0x06);
    memset(sdp_hfp_ag_service, 0, sizeof(sdp_hfp_ag_service));
    hfp_ag_create_sdp_record_with_codecs(sdp_hfp_ag_service, sdp_create_service_record_handle(),
                                         BLUECASTER_HFP_RFCOMM_CHANNEL, "BlueCaster", 0, features, sizeof(hfp_codecs),
                                         hfp_codecs);
    sdp_register_service(sdp_hfp_ag_service);
    hfp_ag_register_packet_handler(&handler_hfp);
}

uint8_t bluecaster_hfp_ag_connect(const uint8_t *addr) {
    bd_addr_t a;
    memcpy(a, addr, 6);
    return hfp_ag_establish_service_level_connection(a);
}

uint8_t bluecaster_hfp_ag_release(uint16_t acl_handle) {
    return hfp_ag_release_service_level_connection(acl_handle);
}

void bluecaster_hfp_ag_deinit(void) {
    hfp_ag_deinit();
}

/* ======================================================================== */
/* RFCOMM: the HFP audio gateway's, and client channels for IPC clients     */
/* ======================================================================== */

void bluecaster_rfcomm_init(void) {
    rfcomm_init();
}

static int sdp_rfcomm_wanted;

/* RFCOMM keeps its services in a static list: cleared here, or the next run
 * in this process finds the service freed by btstack_memory_deinit there */
void bluecaster_rfcomm_deinit(void) {
    rfcomm_deinit();
    sdp_rfcomm_wanted = 0;
}

/* Events of client channels (opened, closed, can send now) and their data
 * (RFCOMM_DATA_PACKET, channel = RFCOMM cid): SOURCE_RFCOMM */
uint8_t bluecaster_rfcomm_create_channel(const uint8_t *addr, uint8_t server_channel, uint16_t *out_cid) {
    bd_addr_t a;
    memcpy(a, addr, 6);
    return rfcomm_create_channel(&handler_rfcomm, a, server_channel, out_cid);
}

/* The RFCOMM server channel of a service: one query at a time (Rust queues
 * the others). It waits in the SDP client's queue, which HFP shares, until
 * the client is free; the results (SDP_EVENT_QUERY_RFCOMM_SERVICE, then
 * SDP_EVENT_QUERY_COMPLETE) go to SOURCE_SDP_CLIENT. */
static btstack_context_callback_registration_t sdp_rfcomm_request;
/* In the SDP client's request list. sdp_client_deinit keeps that list, so
 * this stays set across a restart of BTstack in this process until the
 * callback comes. */
static int sdp_rfcomm_listed;
/* Registered only to make the SDP client run its request list (see
 * bluecaster_sdp_query_rfcomm); its own turn does nothing */
static btstack_context_callback_registration_t sdp_rfcomm_kick;
static void sdp_rfcomm_kicked(void *context) {
    (void)context;
}
static bd_addr_t sdp_rfcomm_addr;
static uint8_t sdp_rfcomm_uuid[16];

static void sdp_rfcomm_start(void *context) {
    (void)context;
    sdp_rfcomm_listed = 0;
    if (!sdp_rfcomm_wanted) return;
    sdp_rfcomm_wanted = 0;
    uint8_t status;
    uint32_t short_uuid = big_endian_read_32(sdp_rfcomm_uuid, 0);
    /* A UUID of the Bluetooth base (SPP 0x1101...) is searched for in its
     * 16-bit form, which every SDP server matches */
    if (uuid_has_bluetooth_prefix(sdp_rfcomm_uuid) && short_uuid <= 0xFFFF) {
        status = sdp_client_query_rfcomm_channel_and_name_for_search_pattern(
            &handler_sdp_client, sdp_rfcomm_addr, sdp_service_search_pattern_for_uuid16((uint16_t)short_uuid));
    } else {
        status = sdp_client_query_rfcomm_channel_and_name_for_uuid128(&handler_sdp_client, sdp_rfcomm_addr,
                                                                      sdp_rfcomm_uuid);
    }
    if (status != ERROR_CODE_SUCCESS) {
        uint8_t event[3] = {SDP_EVENT_QUERY_COMPLETE, 1, status};
        handler_sdp_client(HCI_EVENT_PACKET, 0, event, sizeof(event));
    }
}

uint8_t bluecaster_sdp_query_rfcomm(const uint8_t *addr, const uint8_t *uuid128) {
    if (sdp_rfcomm_wanted) return ERROR_CODE_COMMAND_DISALLOWED;
    memcpy(sdp_rfcomm_addr, addr, 6);
    memcpy(sdp_rfcomm_uuid, uuid128, 16);
    sdp_rfcomm_wanted = 1;
    if (!sdp_rfcomm_listed) {
        /* Set first: the callback may come before this returns */
        sdp_rfcomm_listed = 1;
        sdp_rfcomm_request.callback = &sdp_rfcomm_start;
        uint8_t status = sdp_client_register_query_callback(&sdp_rfcomm_request);
        if (status != ERROR_CODE_SUCCESS) {
            sdp_rfcomm_listed = 0;
            sdp_rfcomm_wanted = 0;
            return status;
        }
    } else if (sdp_client_ready()) {
        /* Listed before a restart of BTstack, which left the client idle
         * with the list kept: nothing pops it until a registration does */
        sdp_rfcomm_kick.callback = &sdp_rfcomm_kicked;
        (void)sdp_client_register_query_callback(&sdp_rfcomm_kick);
    }
    return ERROR_CODE_SUCCESS;
}

/* ======================================================================== */
/* GAP, SDP records, stream endpoints                                       */
/* ======================================================================== */

void bluecaster_gap_setup(uint32_t class_of_device, uint16_t page_timeout) {
    /* SSP: Just Works (no display, no keyboard), General Bonding without MITM */
    gap_ssp_set_io_capability(SSP_IO_CAPABILITY_NO_INPUT_NO_OUTPUT);
    gap_ssp_set_authentication_requirement(SSP_IO_AUTHREQ_MITM_PROTECTION_NOT_REQUIRED_GENERAL_BONDING);
    gap_set_local_name("BlueCaster");  /* BTstack keeps the pointer: a literal */
    gap_set_class_of_device(class_of_device);
    gap_set_allow_role_switch(true);
    gap_set_default_link_policy_settings(0x0005);  /* role switch + sniff */
    gap_set_page_timeout(page_timeout);
    /* BTstack's default is the standard inquiry (no RSSI, no extended
     * inquiry response): every device then needed a name request after the
     * inquiry. With EIR most answer with their name, as on other systems. */
    hci_set_inquiry_mode(INQUIRY_MODE_RSSI_AND_EIR);
}

static uint8_t sdp_a2dp_source_service[150];
static uint8_t sdp_avrcp_controller_service[200];
static uint8_t sdp_avrcp_target_service[200];

void bluecaster_sdp_register_records(uint16_t controller_features, uint16_t target_features) {
    memset(sdp_a2dp_source_service, 0, sizeof(sdp_a2dp_source_service));
    a2dp_source_create_sdp_record(sdp_a2dp_source_service, sdp_create_service_record_handle(),
                                  AVDTP_SOURCE_FEATURE_MASK_PLAYER, NULL, NULL);
    sdp_register_service(sdp_a2dp_source_service);

    memset(sdp_avrcp_controller_service, 0, sizeof(sdp_avrcp_controller_service));
    avrcp_controller_create_sdp_record(sdp_avrcp_controller_service, sdp_create_service_record_handle(),
                                       controller_features, NULL, NULL);
    sdp_register_service(sdp_avrcp_controller_service);

    memset(sdp_avrcp_target_service, 0, sizeof(sdp_avrcp_target_service));
    avrcp_target_create_sdp_record(sdp_avrcp_target_service, sdp_create_service_record_handle(),
                                   target_features, NULL, NULL);
    sdp_register_service(sdp_avrcp_target_service);
}

#define BLUECASTER_MAX_ENDPOINTS MAX_NR_AVDTP_STREAM_ENDPOINTS
/* aptX Adaptive's codec information is the longest: 40 bytes */
#define BLUECASTER_CODEC_INFO_MAX 40
static uint8_t endpoint_caps[BLUECASTER_MAX_ENDPOINTS][BLUECASTER_CODEC_INFO_MAX];
static uint8_t endpoint_config[BLUECASTER_MAX_ENDPOINTS][BLUECASTER_CODEC_INFO_MAX];

/* Returns the local SEID, 0 on failure */
uint8_t bluecaster_create_stream_endpoint(int slot, uint8_t media_codec_type, const uint8_t *caps, uint16_t caps_len,
                                      const uint8_t *config, uint16_t config_len) {
    if (slot < 0 || slot >= BLUECASTER_MAX_ENDPOINTS) return 0;
    if (caps_len > BLUECASTER_CODEC_INFO_MAX || config_len > BLUECASTER_CODEC_INFO_MAX) return 0;
    memcpy(endpoint_caps[slot], caps, caps_len);
    memcpy(endpoint_config[slot], config, config_len);
    avdtp_stream_endpoint_t *ep = a2dp_source_create_stream_endpoint(
        AVDTP_AUDIO, (avdtp_media_codec_type_t)media_codec_type, endpoint_caps[slot], caps_len,
        endpoint_config[slot], config_len);
    return ep ? avdtp_local_seid(ep) : 0;
}

/* ======================================================================== */
/* Stream configuration                                                     */
/* ======================================================================== */

uint8_t bluecaster_set_config_sbc(uint16_t a2dp_cid, uint8_t local_seid, uint8_t remote_seid,
                              uint16_t sampling_frequency, int channel_mode, int block_length, int subbands,
                              int allocation_method, uint8_t min_bitpool, uint8_t max_bitpool) {
    avdtp_configuration_sbc_t c;
    memset(&c, 0, sizeof(c));
    c.sampling_frequency = sampling_frequency;
    c.channel_mode = (avdtp_channel_mode_t)channel_mode;
    c.block_length = (avdtp_sbc_block_length_t)block_length;
    c.subbands = (avdtp_sbc_subbands_t)subbands;
    c.allocation_method = (avdtp_sbc_allocation_method_t)allocation_method;
    c.min_bitpool_value = min_bitpool;
    c.max_bitpool_value = max_bitpool;
    return a2dp_source_set_config_sbc(a2dp_cid, local_seid, remote_seid, &c);
}

uint8_t bluecaster_set_config_aac(uint16_t a2dp_cid, uint8_t local_seid, uint8_t remote_seid, int object_type,
                              uint32_t sampling_frequency, uint8_t channels, uint32_t bit_rate, uint8_t vbr) {
    avdtp_configuration_mpeg_aac_t c;
    memset(&c, 0, sizeof(c));
    c.object_type = (avdtp_aac_object_type_t)object_type;
    c.sampling_frequency = sampling_frequency;
    c.channels = channels;
    c.bit_rate = bit_rate;
    c.vbr = vbr;
    c.drc = false;
    return a2dp_source_set_config_mpeg_aac(a2dp_cid, local_seid, remote_seid, &c);
}

/* BTstack keeps the pointer of the codec information until the
 * configuration is done: a persistent copy */
static uint8_t other_config_info[BLUECASTER_CODEC_INFO_MAX];

uint8_t bluecaster_set_config_other(uint16_t a2dp_cid, uint8_t local_seid, uint8_t remote_seid, const uint8_t *info,
                                uint8_t info_len) {
    if (info_len > sizeof(other_config_info)) return ERROR_CODE_INVALID_HCI_COMMAND_PARAMETERS;
    memcpy(other_config_info, info, info_len);
    return a2dp_source_set_config_other(a2dp_cid, local_seid, remote_seid, other_config_info, info_len);
}

/* ======================================================================== */
/* Connection state queries                                                 */
/* ======================================================================== */

/* ACL link to addr: -1 none, 0 still paging, 1 open (handle set), 2 other */
int bluecaster_acl_state(const uint8_t *addr, uint16_t *con_handle) {
    hci_connection_t *c = hci_connection_for_bd_addr_and_type(addr, BD_ADDR_TYPE_ACL);
    if (c == NULL) return -1;
    if (c->state == SEND_CREATE_CONNECTION || c->state == SENT_CREATE_CONNECTION) return 0;
    if (c->state == OPEN) {
        if (con_handle) *con_handle = c->con_handle;
        return 1;
    }
    return 2;
}

static int avdtp_state(const avdtp_connection_t *c) {
    if (c == NULL) return -1;
    return (c->state == AVDTP_SIGNALING_CONNECTION_OPENED ||
            c->state == AVDTP_SIGNALING_CONNECTION_W4_L2CAP_DISCONNECTED) ? 1 : 2;
}

/* AVDTP connection: -1 none, 1 signaling channel open (a release follows a
 * disconnect), 2 not open yet (freed at once by a disconnect) */
int bluecaster_avdtp_state_for_addr(const uint8_t *addr, uint16_t *avdtp_cid) {
    bd_addr_t a;
    memcpy(a, addr, 6);
    avdtp_connection_t *c = avdtp_get_connection_for_bd_addr(a);
    if (c && avdtp_cid) *avdtp_cid = c->avdtp_cid;
    return avdtp_state(c);
}

int bluecaster_avdtp_state_for_cid(uint16_t avdtp_cid) {
    return avdtp_state(avdtp_get_connection_for_avdtp_cid(avdtp_cid));
}

/* ======================================================================== */
/* HCI commands                                                             */
/* ======================================================================== */

/* Not in BTstack's command table: HCI Set AFH Host Channel Classification
 * (10-byte map as 8 + 1 + 1 bytes) and HCI Read AFH Channel Map */
static const hci_cmd_t cmd_set_afh_host_channel_classification = {0x0C3F, "D11"};
static const hci_cmd_t cmd_read_afh_channel_map = {0x1406, "H"};

uint8_t bluecaster_cmd_set_afh_classification(const uint8_t *map) {
    return hci_send_cmd(&cmd_set_afh_host_channel_classification, map, map[8], map[9]);
}

uint8_t bluecaster_cmd_read_afh_channel_map(uint16_t con_handle) {
    return hci_send_cmd(&cmd_read_afh_channel_map, con_handle);
}

uint8_t bluecaster_cmd_read_rssi(uint16_t con_handle) {
    return hci_send_cmd(&hci_read_rssi, con_handle);
}

uint8_t bluecaster_cmd_read_bd_addr(void) {
    return hci_send_cmd(&hci_read_bd_addr);
}

uint8_t bluecaster_cmd_read_local_version(void) {
    return hci_send_cmd(&hci_read_local_version_information);
}

uint8_t bluecaster_cmd_create_connection_cancel(const uint8_t *addr) {
    bd_addr_t a;
    memcpy(a, addr, 6);
    return hci_send_cmd(&hci_create_connection_cancel, a);
}

uint8_t bluecaster_cmd_remote_name_request_cancel(const uint8_t *addr) {
    bd_addr_t a;
    memcpy(a, addr, 6);
    return hci_send_cmd(&hci_remote_name_request_cancel, a);
}

int bluecaster_hci_get_state(void) {
    return (int)hci_get_state();
}

int bluecaster_hci_can_send_command_now(void) {
    return hci_can_send_command_packet_now() ? 1 : 0;
}

/* ======================================================================== */
/* LE (experimental LE Audio): set up only by Stack::le_init                */
/* ======================================================================== */

/* ---- Per-connection storage BTstack keeps pointers to ---- */

#define BLUECASTER_LE_SLOTS 16
#define BLUECASTER_ATT_VALUE_MAX 512

typedef struct {
    hci_con_handle_t con;                        /* HCI_CON_HANDLE_INVALID: free */
    gatt_client_notification_t listener;         /* all notifications / indications of con */
    int listening;
    uint8_t write_buffer[BLUECASTER_ATT_VALUE_MAX];  /* the value of a running (long) write */
    btstack_context_callback_registration_t send_registration;  /* ATT server can-send-now */
} le_slot_t;

static le_slot_t le_slots[BLUECASTER_LE_SLOTS];

static void le_slots_reset(void) {
    memset(le_slots, 0, sizeof(le_slots));
    for (int i = 0; i < BLUECASTER_LE_SLOTS; i++) le_slots[i].con = HCI_CON_HANDLE_INVALID;
}

static le_slot_t *le_slot_for(hci_con_handle_t con, int create) {
    if (con == HCI_CON_HANDLE_INVALID) return NULL;
    le_slot_t *free_slot = NULL;
    for (int i = 0; i < BLUECASTER_LE_SLOTS; i++) {
        if (le_slots[i].con == con) return &le_slots[i];
        if (free_slot == NULL && le_slots[i].con == HCI_CON_HANDLE_INVALID) free_slot = &le_slots[i];
    }
    if (!create || free_slot == NULL) return NULL;
    memset(free_slot, 0, sizeof(*free_slot));
    free_slot->con = con;
    return free_slot;
}

/* The connection is gone: stop listening and free the slot. Its ATT send
 * registration was in a list of the freed hci_connection_t only. */
static void le_slot_release(hci_con_handle_t con) {
    le_slot_t *slot = le_slot_for(con, 0);
    if (slot == NULL) return;
    if (slot->listening) gatt_client_stop_listening_for_characteristic_value_updates(&slot->listener);
    slot->listening = 0;
    slot->con = HCI_CON_HANDLE_INVALID;
}

/* ---- CIG storage (gap_cig_create keeps both pointers until the CIG is
 * removed) ---- */

#define BLUECASTER_CIG_SLOTS 4

typedef struct {
    le_audio_cig_t cig;
    le_audio_cig_params_t params;
    int in_use;      /* possibly in hci.c's CIG list */
    uint8_t cig_id;
} cig_slot_t;

static cig_slot_t cig_slots[BLUECASTER_CIG_SLOTS];

static void cig_slot_release(uint8_t cig_id) {
    for (int i = 0; i < BLUECASTER_CIG_SLOTS; i++)
        if (cig_slots[i].in_use && cig_slots[i].cig_id == cig_id) cig_slots[i].in_use = 0;
}

static void cig_slots_reset(void) {
    memset(cig_slots, 0, sizeof(cig_slots));
}

/* ---- Housekeeping on HCI events (registered by bluecaster_le_init) ---- */

static btstack_packet_callback_registration_t le_hci_registration;

static void le_hci_handler(uint8_t packet_type, uint16_t channel, uint8_t *packet, uint16_t size) {
    (void)channel;
    if (packet_type != HCI_EVENT_PACKET || size < 2) return;
    switch (packet[0]) {
    case HCI_EVENT_DISCONNECTION_COMPLETE:
        if (size >= 6 && packet[2] == ERROR_CODE_SUCCESS) {
            hci_con_handle_t con = little_endian_read_16(packet, 3);
            le_slot_release(con);
            bluecaster_rs_le_disconnected(con);
        }
        break;
    case HCI_EVENT_META_GAP:
        /* A CIG that could not be created is not kept by hci.c */
        if (size >= 5 && packet[2] == GAP_SUBEVENT_CIG_CREATED && packet[3] != ERROR_CODE_SUCCESS)
            cig_slot_release(packet[4]);
        break;
    case BTSTACK_EVENT_STATE:
        /* HCI reset its state (power off): no CIG is kept */
        if (size >= 3 && packet[2] == HCI_STATE_OFF) cig_slots_reset();
        break;
    default:
        break;
    }
}

/* ---- Setup ---- */

static btstack_tlv_windows_t le_tlv_context;
static int le_tlv_open;
static char le_tlv_path[1024];
static btstack_packet_callback_registration_t sm_event_registration;

/* btstack_tlv_windows opens the file with CreateFileA: the path in the ANSI
 * code page, or with the folder's 8.3 name if it does not fit in it */
static int utf8_to_ansi_path(const char *in, char *out, int out_size) {
    wchar_t wide[1024];
    if (MultiByteToWideChar(CP_UTF8, 0, in, -1, wide, 1024) <= 0) return 0;
    if (GetACP() == CP_UTF8) {
        size_t n = strlen(in) + 1;
        if (n > (size_t)out_size) return 0;
        memcpy(out, in, n);
        return 1;
    }
    BOOL lossy = FALSE;
    if (WideCharToMultiByte(CP_ACP, 0, wide, -1, out, out_size, NULL, &lossy) > 0 && !lossy) return 1;
    wchar_t *sep = wcsrchr(wide, L'\\');
    wchar_t *sep2 = wcsrchr(wide, L'/');
    if (sep2 > sep) sep = sep2;
    if (sep == NULL) return 0;
    *sep = 0;
    wchar_t short_dir[1024];
    DWORD n = GetShortPathNameW(wide, short_dir, 1024);
    if (n == 0 || n >= 1024 || wcslen(short_dir) + 1 + wcslen(sep + 1) + 1 > 1024) return 0;
    wcscat(short_dir, L"\\");
    wcscat(short_dir, sep + 1);
    lossy = FALSE;
    return WideCharToMultiByte(CP_ACP, 0, short_dir, -1, out, out_size, NULL, &lossy) > 0 && !lossy;
}

/* LE bonds in a TLV file (le_device_db_tlv, as v1's le_audio_bonds.tlv),
 * SM with LE Secure Connections, Just Works and bonding, GATT client, ISO.
 * Returns 0 when the path cannot be used (LE is set up anyway, with a TLV
 * store that keeps nothing). */
int bluecaster_le_init(const char *tlv_path) {
    le_slots_reset();
    cig_slots_reset();
    int path_ok = utf8_to_ansi_path(tlv_path, le_tlv_path, (int)sizeof(le_tlv_path));
    if (!path_ok) strcpy(le_tlv_path, "NUL");
    memset(&le_tlv_context, 0, sizeof(le_tlv_context));
    const btstack_tlv_t *tlv = btstack_tlv_windows_init_instance(&le_tlv_context, le_tlv_path);
    le_tlv_open = 1;
    btstack_tlv_set_instance(tlv, &le_tlv_context);
    le_device_db_tlv_configure(tlv, &le_tlv_context);

    sm_init();
    sm_set_io_capabilities(IO_CAPABILITY_NO_INPUT_NO_OUTPUT);
    sm_set_authentication_requirements(SM_AUTHREQ_SECURE_CONNECTION | SM_AUTHREQ_BONDING);
    memset(&sm_event_registration, 0, sizeof(sm_event_registration));
    sm_event_registration.callback = &handler_sm;
    sm_add_event_handler(&sm_event_registration);

    gatt_client_init();

    memset(&le_hci_registration, 0, sizeof(le_hci_registration));
    le_hci_registration.callback = &le_hci_handler;
    hci_add_event_handler(&le_hci_registration);
    hci_register_iso_packet_handler(&handler_iso);
    return path_ok;
}

void bluecaster_le_deinit(int att_server_started) {
    for (int i = 0; i < BLUECASTER_LE_SLOTS; i++)
        if (le_slots[i].con != HCI_CON_HANDLE_INVALID) le_slot_release(le_slots[i].con);
    hci_remove_event_handler(&le_hci_registration);
    if (att_server_started) att_server_deinit();
    sm_deinit();
    btstack_tlv_set_instance(NULL, NULL);
    if (le_tlv_open) {
        btstack_tlv_windows_deinit(&le_tlv_context);
        /* btstack_tlv_windows_deinit frees the entries but leaves the file
         * open, without sharing: the next bluecaster_le_init (BTstack restarted
         * in this process) could then neither open nor re-create it, and the
         * LE bonds would be neither loaded nor saved */
        if (le_tlv_context.file != INVALID_HANDLE_VALUE) {
            CloseHandle(le_tlv_context.file);
            le_tlv_context.file = INVALID_HANDLE_VALUE;
        }
        le_tlv_open = 0;
    }
    cig_slots_reset();
}

/* ---- GAP ---- */

uint8_t bluecaster_gap_connect(const uint8_t *addr, uint8_t addr_type) {
    bd_addr_t a;
    memcpy(a, addr, 6);
    return gap_connect(a, (bd_addr_type_t)addr_type);
}

int bluecaster_gap_connection_type(uint16_t con) {
    return (int)gap_get_connection_type(con);
}

void bluecaster_gap_delete_bonding(uint8_t addr_type, const uint8_t *addr) {
    bd_addr_t a;
    memcpy(a, addr, 6);
    gap_delete_bonding((bd_addr_type_t)addr_type, a);
}

/* HCI Disconnect for any handle (a CIS: gap_disconnect only knows ACL / SCO) */
uint8_t bluecaster_cmd_disconnect(uint16_t handle, uint8_t reason) {
    return hci_send_cmd(&hci_disconnect, handle, reason);
}

/* ---- TLV (the store le_device_db and att_server use) ---- */

/* Length of the tag's value (copied into buffer if not NULL), -1 without a
 * store, 0 if the tag is not there */
int bluecaster_tlv_get(uint32_t tag, uint8_t *buffer, uint32_t size) {
    const btstack_tlv_t *tlv = NULL;
    void *context = NULL;
    btstack_tlv_get_instance(&tlv, &context);
    if (tlv == NULL) return -1;
    return tlv->get_tag(context, tag, buffer, size);
}

int bluecaster_tlv_store(uint32_t tag, const uint8_t *data, uint32_t size) {
    const btstack_tlv_t *tlv = NULL;
    void *context = NULL;
    btstack_tlv_get_instance(&tlv, &context);
    if (tlv == NULL) return -1;
    return tlv->store_tag(context, tag, data, size);
}

void bluecaster_tlv_delete(uint32_t tag) {
    const btstack_tlv_t *tlv = NULL;
    void *context = NULL;
    btstack_tlv_get_instance(&tlv, &context);
    if (tlv != NULL) tlv->delete_tag(context, tag);
}

/* ---- GATT client (events of queries and listeners: SOURCE_GATT_CLIENT) ---- */

uint8_t bluecaster_gatt_discover_primary_services(uint16_t con) {
    return gatt_client_discover_primary_services(&handler_gatt_client, con);
}

static void make_service(gatt_client_service_t *s, uint16_t start, uint16_t end) {
    memset(s, 0, sizeof(*s));
    s->start_group_handle = start;
    s->end_group_handle = end;
}

uint8_t bluecaster_gatt_find_included_services(uint16_t con, uint16_t start, uint16_t end) {
    gatt_client_service_t s;
    make_service(&s, start, end);
    return gatt_client_find_included_services_for_service(&handler_gatt_client, con, &s);
}

uint8_t bluecaster_gatt_discover_characteristics(uint16_t con, uint16_t start, uint16_t end) {
    gatt_client_service_t s;
    make_service(&s, start, end);
    return gatt_client_discover_characteristics_for_service(&handler_gatt_client, con, &s);
}

uint8_t bluecaster_gatt_read_long_value(uint16_t con, uint16_t value_handle) {
    return gatt_client_read_long_value_of_characteristic_using_value_handle(&handler_gatt_client, con, value_handle);
}

/* BTstack keeps the value pointer until the write is done: a copy in the
 * connection's slot, made only when no other query runs (gatt_client allows
 * one per connection) */
uint8_t bluecaster_gatt_write_value(uint16_t con, uint16_t value_handle, const uint8_t *value, uint16_t len,
                                int long_write) {
    if (len > BLUECASTER_ATT_VALUE_MAX) return ERROR_CODE_INVALID_HCI_COMMAND_PARAMETERS;
    if (!gatt_client_is_ready(con)) return GATT_CLIENT_IN_WRONG_STATE;
    le_slot_t *slot = le_slot_for(con, 1);
    if (slot == NULL) return BTSTACK_MEMORY_ALLOC_FAILED;
    if (len > 0) memcpy(slot->write_buffer, value, len);
    return long_write
        ? gatt_client_write_long_value_of_characteristic(&handler_gatt_client, con, value_handle, len,
                                                         slot->write_buffer)
        : gatt_client_write_value_of_characteristic(&handler_gatt_client, con, value_handle, len,
                                                    slot->write_buffer);
}

/* Copied into the outgoing buffer at once */
uint8_t bluecaster_gatt_write_without_response(uint16_t con, uint16_t value_handle, const uint8_t *value, uint16_t len) {
    return gatt_client_write_value_of_characteristic_without_response(con, value_handle, len, (uint8_t *)value);
}

uint8_t bluecaster_gatt_write_client_configuration(uint16_t con, uint16_t start_handle, uint16_t value_handle,
                                               uint16_t end_handle, uint16_t properties, uint16_t uuid16,
                                               const uint8_t *uuid128, uint16_t configuration) {
    gatt_client_characteristic_t c;
    memset(&c, 0, sizeof(c));
    c.start_handle = start_handle;
    c.value_handle = value_handle;
    c.end_handle = end_handle;
    c.properties = properties;
    c.uuid16 = uuid16;
    if (uuid128) memcpy(c.uuid128, uuid128, 16);
    return gatt_client_write_client_characteristic_configuration(&handler_gatt_client, con, &c, configuration);
}

/* All notifications and indications of con (GATT_EVENT_NOTIFICATION /
 * _INDICATION to SOURCE_GATT_CLIENT); the listener is dropped when the
 * connection goes down */
int bluecaster_gatt_listen(uint16_t con) {
    le_slot_t *slot = le_slot_for(con, 1);
    if (slot == NULL) return 0;
    if (!slot->listening) {
        gatt_client_listen_for_characteristic_value_updates(&slot->listener, &handler_gatt_client, con, NULL);
        slot->listening = 1;
    }
    return 1;
}

void bluecaster_gatt_stop_listening(uint16_t con) {
    le_slot_t *slot = le_slot_for(con, 0);
    if (slot == NULL || !slot->listening) return;
    gatt_client_stop_listening_for_characteristic_value_updates(&slot->listener);
    slot->listening = 0;
}

/* ---- ATT server: database from att_db_util, callbacks to Rust ---- */

static uint16_t att_read_callback(hci_con_handle_t con, uint16_t handle, uint16_t offset, uint8_t *buffer,
                                  uint16_t buffer_size) {
    return bluecaster_rs_att_read(con, handle, offset, buffer, buffer_size);
}

static int att_write_callback(hci_con_handle_t con, uint16_t handle, uint16_t mode, uint16_t offset,
                              uint8_t *buffer, uint16_t buffer_size) {
    return bluecaster_rs_att_write(con, handle, mode, offset, buffer, buffer_size);
}

/* The database att_db_util built: att_server keeps the pointer, so
 * att_db_util must not be used to change it afterwards */
void bluecaster_att_server_init(void) {
    att_server_init(att_db_util_get_address(), &att_read_callback, &att_write_callback);
    att_server_register_packet_handler(&handler_att_server);
}

static void att_send_callback(void *context) {
    bluecaster_rs_att_can_send((uint16_t)(uintptr_t)context);
}

/* One request at a time per connection (the registration is a list item):
 * Rust asks again after the callback */
uint8_t bluecaster_att_server_request_to_send(uint16_t con, int indication) {
    le_slot_t *slot = le_slot_for(con, 1);
    if (slot == NULL) return BTSTACK_MEMORY_ALLOC_FAILED;
    slot->send_registration.callback = &att_send_callback;
    slot->send_registration.context = (void *)(uintptr_t)con;
    return indication ? att_server_request_to_send_indication(&slot->send_registration, con)
                      : att_server_request_to_send_notification(&slot->send_registration, con);
}

/* ---- CIG / CIS / ISO ---- */

/* The plain layout Rust fills (btstack-sys src/ffi.rs CisParams) */
typedef struct {
    uint8_t cis_id;
    uint8_t phy_c_to_p;
    uint8_t phy_p_to_c;
    uint8_t rtn_c_to_p;
    uint8_t rtn_p_to_c;
    uint16_t max_sdu_c_to_p;
    uint16_t max_sdu_p_to_c;
} bluecaster_cis_params_t;

uint8_t bluecaster_cig_create(uint8_t cig_id, uint32_t sdu_interval_c_to_p, uint32_t sdu_interval_p_to_c,
                          uint8_t worst_case_sca, uint8_t packing, uint8_t framing,
                          uint16_t max_transport_latency_c_to_p, uint16_t max_transport_latency_p_to_c,
                          uint8_t num_cis, const bluecaster_cis_params_t *cis) {
    if (num_cis == 0 || num_cis > MAX_NR_CIS) return ERROR_CODE_INVALID_HCI_COMMAND_PARAMETERS;
    cig_slot_t *slot = NULL;
    for (int i = 0; i < BLUECASTER_CIG_SLOTS; i++) {
        if (cig_slots[i].in_use && cig_slots[i].cig_id == cig_id) return ERROR_CODE_ACL_CONNECTION_ALREADY_EXISTS;
        if (slot == NULL && !cig_slots[i].in_use) slot = &cig_slots[i];
    }
    if (slot == NULL) return ERROR_CODE_MEMORY_CAPACITY_EXCEEDED;
    memset(slot, 0, sizeof(*slot));
    le_audio_cig_params_t *p = &slot->params;
    p->cig_id = cig_id;
    p->sdu_interval_c_to_p = sdu_interval_c_to_p;
    p->sdu_interval_p_to_c = sdu_interval_p_to_c;
    p->worst_case_sca = worst_case_sca;
    p->packing = packing;
    p->framing = framing;
    p->max_transport_latency_c_to_p = max_transport_latency_c_to_p;
    p->max_transport_latency_p_to_c = max_transport_latency_p_to_c;
    p->num_cis = num_cis;
    for (int i = 0; i < num_cis; i++) {
        p->cis_params[i].cis_id = cis[i].cis_id;
        p->cis_params[i].max_sdu_c_to_p = cis[i].max_sdu_c_to_p;
        p->cis_params[i].max_sdu_p_to_c = cis[i].max_sdu_p_to_c;
        p->cis_params[i].phy_c_to_p = cis[i].phy_c_to_p;
        p->cis_params[i].phy_p_to_c = cis[i].phy_p_to_c;
        p->cis_params[i].rtn_c_to_p = cis[i].rtn_c_to_p;
        p->cis_params[i].rtn_p_to_c = cis[i].rtn_p_to_c;
    }
    uint8_t status = gap_cig_create(&slot->cig, p);
    if (status == ERROR_CODE_SUCCESS) {
        slot->in_use = 1;
        slot->cig_id = cig_id;
    }
    return status;
}

/* One HCI ISO data packet with a complete SDU: handle | PB 0b10, no time
 * stamp, SDU header (sequence number, length). For CIS can-send-now. */
uint8_t bluecaster_iso_send(uint16_t cis_handle, uint16_t sequence_number, const uint8_t *sdu, uint16_t len) {
    if ((uint32_t)len + 4u > HCI_ISO_PAYLOAD_SIZE) return ERROR_CODE_INVALID_HCI_COMMAND_PARAMETERS;
    if (hci_is_packet_buffer_reserved()) return BTSTACK_ACL_BUFFERS_FULL;
    hci_reserve_packet_buffer();
    uint8_t *buffer = hci_get_outgoing_packet_buffer();
    little_endian_store_16(buffer, 0, (uint16_t)((cis_handle & 0x0FFFu) | (2u << 12)));
    little_endian_store_16(buffer, 2, (uint16_t)(4u + len));
    little_endian_store_16(buffer, 4, sequence_number);
    little_endian_store_16(buffer, 6, len);
    if (len > 0) memcpy(buffer + 8, sdu, len);
    return hci_send_iso_packet_buffer((uint16_t)(8u + len));
}
