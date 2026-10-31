//! Raw declarations: BTstack functions with plain signatures, and the glue in
//! csrc/shim.c for everything that involves BTstack's structs.
//!
//! Return types follow the C prototypes exactly (an `int` read as `u8`, or the
//! other way round, would be undefined).

use std::ffi::{c_char, c_int};

unsafe extern "C" {
    // ---- shim.c: run loop ----
    pub fn bluecaster_runloop_init();
    pub fn bluecaster_wake();
    pub fn bluecaster_timer_start(id: u32, timeout_ms: u32) -> c_int;
    pub fn bluecaster_timer_stop(id: u32);
    pub fn bluecaster_timers_clear();

    // ---- shim.c: transport, dump, link keys, handlers ----
    pub fn bluecaster_hci_init(is_async: c_int);
    pub fn bluecaster_transport_deliver(packet_type: u8, packet: *const u8, size: u16) -> c_int;
    pub fn bluecaster_transport_packet_sent();
    pub fn bluecaster_intel_download_firmware(is_async: c_int, firmware_path: *const c_char);
    pub fn bluecaster_hci_dump_init();
    pub fn bluecaster_set_link_key_db(rust_store: c_int);
    pub fn bluecaster_add_hci_event_handler();
    pub fn bluecaster_a2dp_source_init();
    pub fn bluecaster_avrcp_init();
    pub fn bluecaster_hfp_ag_init();
    pub fn bluecaster_hfp_ag_connect(addr: *const u8) -> u8;
    pub fn bluecaster_hfp_ag_release(acl_handle: u16) -> u8;
    pub fn bluecaster_hfp_ag_deinit();
    pub fn bluecaster_rfcomm_init();
    pub fn bluecaster_rfcomm_deinit();
    pub fn bluecaster_rfcomm_create_channel(addr: *const u8, server_channel: u8, out_cid: *mut u16) -> u8;
    pub fn bluecaster_sdp_query_rfcomm(addr: *const u8, uuid128: *const u8) -> u8;

    // ---- shim.c: GAP, SDP, endpoints, configuration ----
    pub fn bluecaster_gap_setup(class_of_device: u32, page_timeout: u16);
    pub fn bluecaster_sdp_register_records(controller_features: u16, target_features: u16);
    pub fn bluecaster_create_stream_endpoint(
        slot: c_int,
        media_codec_type: u8,
        caps: *const u8,
        caps_len: u16,
        config: *const u8,
        config_len: u16,
    ) -> u8;
    pub fn bluecaster_set_config_sbc(
        a2dp_cid: u16,
        local_seid: u8,
        remote_seid: u8,
        sampling_frequency: u16,
        channel_mode: c_int,
        block_length: c_int,
        subbands: c_int,
        allocation_method: c_int,
        min_bitpool: u8,
        max_bitpool: u8,
    ) -> u8;
    pub fn bluecaster_set_config_aac(
        a2dp_cid: u16,
        local_seid: u8,
        remote_seid: u8,
        object_type: c_int,
        sampling_frequency: u32,
        channels: u8,
        bit_rate: u32,
        vbr: u8,
    ) -> u8;
    pub fn bluecaster_set_config_other(
        a2dp_cid: u16,
        local_seid: u8,
        remote_seid: u8,
        info: *const u8,
        info_len: u8,
    ) -> u8;

    // ---- shim.c: state queries, HCI commands ----
    pub fn bluecaster_acl_state(addr: *const u8, con_handle: *mut u16) -> c_int;
    pub fn bluecaster_cmd_create_connection_cancel(addr: *const u8) -> u8;
    pub fn bluecaster_avdtp_state_for_addr(addr: *const u8, avdtp_cid: *mut u16) -> c_int;
    pub fn bluecaster_avdtp_state_for_cid(avdtp_cid: u16) -> c_int;
    pub fn bluecaster_cmd_set_afh_classification(map: *const u8) -> u8;
    pub fn bluecaster_cmd_read_afh_channel_map(con_handle: u16) -> u8;
    pub fn bluecaster_cmd_read_rssi(con_handle: u16) -> u8;
    pub fn bluecaster_cmd_read_bd_addr() -> u8;
    pub fn bluecaster_cmd_read_local_version() -> u8;
    pub fn bluecaster_cmd_remote_name_request_cancel(addr: *const u8) -> u8;
    pub fn bluecaster_hci_get_state() -> c_int;
    pub fn bluecaster_hci_can_send_command_now() -> c_int;

    // ---- BTstack: memory, run loop ----
    pub fn btstack_memory_init();
    pub fn btstack_memory_deinit();
    pub fn btstack_run_loop_execute();
    pub fn btstack_run_loop_trigger_exit();
    pub fn btstack_run_loop_deinit();

    // ---- BTstack: HCI, L2CAP, SDP ----
    pub fn hci_power_control(mode: c_int) -> c_int;
    pub fn hci_enable_custom_pre_init();
    pub fn hci_deinit();
    pub fn l2cap_init();
    pub fn l2cap_deinit();
    pub fn sdp_init();
    pub fn sdp_deinit();
    pub fn sdp_client_deinit();

    // ---- BTstack: GAP ----
    pub fn gap_inquiry_start(duration_in_1280ms_units: u8) -> c_int;
    pub fn gap_inquiry_stop() -> c_int;
    pub fn gap_remote_name_request(addr: *const u8, page_scan_repetition_mode: u8, clock_offset: u16) -> c_int;
    pub fn gap_ssp_confirmation_response(addr: *const u8) -> c_int;
    pub fn gap_pin_code_response(addr: *const u8, pin: *const c_char) -> c_int;
    pub fn gap_disconnect(handle: u16) -> u8;
    pub fn gap_drop_link_key_for_bd_addr(addr: *mut u8);

    // ---- BTstack: A2DP source ----
    pub fn a2dp_source_establish_stream(remote_addr: *mut u8, avdtp_cid: *mut u16) -> u8;
    pub fn a2dp_source_start_stream(a2dp_cid: u16, local_seid: u8) -> u8;
    pub fn a2dp_source_pause_stream(a2dp_cid: u16, local_seid: u8) -> u8;
    pub fn a2dp_source_disconnect(a2dp_cid: u16) -> u8;
    pub fn a2dp_source_stream_endpoint_request_can_send_now(a2dp_cid: u16, local_seid: u8);
    pub fn a2dp_max_media_payload_size(a2dp_cid: u16, local_seid: u8) -> c_int;
    pub fn a2dp_source_stream_send_media_payload_rtp(
        a2dp_cid: u16,
        local_seid: u8,
        marker: u8,
        timestamp: u32,
        payload: *const u8,
        payload_size: u16,
    ) -> u8;
    pub fn a2dp_source_stream_send_media_packet(a2dp_cid: u16, local_seid: u8, packet: *const u8, size: u16) -> u8;
    pub fn a2dp_source_deinit();

    // ---- BTstack: AVRCP ----
    pub fn avrcp_connect(remote_addr: *mut u8, avrcp_cid: *mut u16) -> u8;
    pub fn avrcp_disconnect(avrcp_cid: u16) -> u8;
    pub fn avrcp_controller_set_absolute_volume(avrcp_cid: u16, volume: u8) -> u8;
    pub fn avrcp_controller_enable_notification(avrcp_cid: u16, event_id: c_int) -> u8;
    pub fn avrcp_target_support_event(avrcp_cid: u16, event_id: c_int) -> u8;
    pub fn avrcp_target_set_playback_status(avrcp_cid: u16, playback_status: c_int) -> u8;
    pub fn avrcp_target_play_status(avrcp_cid: u16, song_length_ms: u32, song_position_ms: u32, status: c_int) -> u8;
    pub fn avrcp_target_deinit();
    pub fn avrcp_controller_deinit();
    pub fn avrcp_deinit();

    // ---- BTstack: RFCOMM ----
    pub fn rfcomm_disconnect(rfcomm_cid: u16) -> u8;
    pub fn rfcomm_request_can_send_now_event(rfcomm_cid: u16) -> u8;
    pub fn rfcomm_send(rfcomm_cid: u16, data: *mut u8, len: u16) -> u8;

    // ---- BTstack: chipsets ----
    pub fn btstack_chipset_realtek_get_num_usb_controllers() -> u16;
    pub fn btstack_chipset_realtek_get_vendor_product_id(index: u16, vendor_id: *mut u16, product_id: *mut u16);
    /// csrc/realtek.c
    pub fn bluecaster_realtek_usb_fw_name(index: u16) -> *const c_char;
    pub fn btstack_chipset_realtek_set_product_id(id: u16);
    pub fn btstack_chipset_realtek_set_firmware_folder_path(path: *const c_char);
    pub fn btstack_chipset_realtek_set_config_folder_path(path: *const c_char);
    /// csrc/realtek.c: the Realtek chipset opening the .bin files
    pub fn bluecaster_realtek_instance() -> *const core::ffi::c_void;
    pub fn btstack_chipset_bcm_set_hcd_file_path(path: *const c_char);
    pub fn btstack_chipset_bcm_instance() -> *const core::ffi::c_void;
    pub fn hci_set_chipset(chipset: *const core::ffi::c_void);
}

/// csrc/shim.c bluecaster_cis_params_t (a plain layout of BlueCaster's own, copied
/// into BTstack's le_audio_cis_params_t by the shim)
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CisParamsC {
    pub cis_id: u8,
    pub phy_c_to_p: u8,
    pub phy_p_to_c: u8,
    pub rtn_c_to_p: u8,
    pub rtn_p_to_c: u8,
    pub max_sdu_c_to_p: u16,
    pub max_sdu_p_to_c: u16,
}

// LE (experimental LE Audio)
unsafe extern "C" {
    // ---- shim.c: setup ----
    pub fn bluecaster_le_init(tlv_path: *const c_char) -> c_int;
    pub fn bluecaster_le_deinit(att_server_started: c_int);

    // ---- shim.c: GAP, HCI ----
    pub fn bluecaster_gap_connect(addr: *const u8, addr_type: u8) -> u8;
    pub fn bluecaster_gap_connection_type(con: u16) -> c_int;
    pub fn bluecaster_gap_delete_bonding(addr_type: u8, addr: *const u8);
    pub fn bluecaster_cmd_disconnect(handle: u16, reason: u8) -> u8;

    // ---- shim.c: TLV ----
    pub fn bluecaster_tlv_get(tag: u32, buffer: *mut u8, size: u32) -> c_int;
    pub fn bluecaster_tlv_store(tag: u32, data: *const u8, size: u32) -> c_int;
    pub fn bluecaster_tlv_delete(tag: u32);

    // ---- shim.c: GATT client ----
    pub fn bluecaster_gatt_discover_primary_services(con: u16) -> u8;
    pub fn bluecaster_gatt_find_included_services(con: u16, start: u16, end: u16) -> u8;
    pub fn bluecaster_gatt_discover_characteristics(con: u16, start: u16, end: u16) -> u8;
    pub fn bluecaster_gatt_read_long_value(con: u16, value_handle: u16) -> u8;
    pub fn bluecaster_gatt_write_value(
        con: u16,
        value_handle: u16,
        value: *const u8,
        len: u16,
        long_write: c_int,
    ) -> u8;
    pub fn bluecaster_gatt_write_without_response(con: u16, value_handle: u16, value: *const u8, len: u16) -> u8;
    #[allow(clippy::too_many_arguments)]
    pub fn bluecaster_gatt_write_client_configuration(
        con: u16,
        start_handle: u16,
        value_handle: u16,
        end_handle: u16,
        properties: u16,
        uuid16: u16,
        uuid128: *const u8,
        configuration: u16,
    ) -> u8;
    pub fn bluecaster_gatt_listen(con: u16) -> c_int;
    pub fn bluecaster_gatt_stop_listening(con: u16);

    // ---- shim.c: ATT server ----
    pub fn bluecaster_att_server_init();
    pub fn bluecaster_att_server_request_to_send(con: u16, indication: c_int) -> u8;

    // ---- shim.c: CIG / ISO ----
    #[allow(clippy::too_many_arguments)]
    pub fn bluecaster_cig_create(
        cig_id: u8,
        sdu_interval_c_to_p: u32,
        sdu_interval_p_to_c: u32,
        worst_case_sca: u8,
        packing: u8,
        framing: u8,
        max_transport_latency_c_to_p: u16,
        max_transport_latency_p_to_c: u16,
        num_cis: u8,
        cis: *const CisParamsC,
    ) -> u8;
    pub fn bluecaster_iso_send(cis_handle: u16, sequence_number: u16, sdu: *const u8, len: u16) -> u8;

    // ---- BTstack: GAP (LE) ----
    pub fn gap_set_scan_params(scan_type: u8, scan_interval: u16, scan_window: u16, filter_policy: u8) -> u8;
    pub fn gap_start_scan();
    pub fn gap_stop_scan();
    pub fn gap_connect_cancel() -> u8;
    pub fn gap_cig_remove(cig_id: u8) -> u8;
    pub fn gap_cis_create(cig_id: u8, cis_con_handles: *mut u16, acl_con_handles: *mut u16) -> u8;

    // ---- BTstack: HCI ISO ----
    pub fn hci_set_num_iso_packets_to_queue(num_packets: u8);
    pub fn hci_request_cis_can_send_now_events(cis_con_handle: u16) -> u8;

    // ---- BTstack: SM ----
    pub fn sm_request_pairing(con: u16);
    pub fn sm_just_works_confirm(con: u16);
    pub fn sm_numeric_comparison_confirm(con: u16);
    pub fn sm_bonding_decline(con: u16);
    pub fn sm_address_resolution_lookup(address_type: u8, address: *mut u8) -> c_int;
    pub fn sm_le_device_index(con: u16) -> c_int;

    // ---- BTstack: LE device DB ----
    pub fn le_device_db_max_count() -> c_int;
    pub fn le_device_db_info(index: c_int, addr_type: *mut c_int, addr: *mut u8, irk: *mut u8);
    #[allow(clippy::too_many_arguments)]
    pub fn le_device_db_encryption_get(
        index: c_int,
        ediv: *mut u16,
        rand: *mut u8,
        ltk: *mut u8,
        key_size: *mut c_int,
        authenticated: *mut c_int,
        authorized: *mut c_int,
        secure_connection: *mut c_int,
    );

    // ---- BTstack: GATT client ----
    pub fn gatt_client_get_mtu(con: u16, mtu: *mut u16) -> u8;

    // ---- BTstack: ATT DB builder, ATT server ----
    pub fn att_db_util_init();
    pub fn att_db_util_add_service_uuid16(uuid16: u16) -> u16;
    pub fn att_db_util_add_service_uuid128(uuid128: *const u8) -> u16;
    pub fn att_db_util_add_secondary_service_uuid16(uuid16: u16) -> u16;
    pub fn att_db_util_add_secondary_service_uuid128(uuid128: *const u8) -> u16;
    pub fn att_db_util_add_included_service_uuid16(start_group_handle: u16, end_group_handle: u16, uuid16: u16) -> u16;
    pub fn att_db_util_add_characteristic_uuid16(
        uuid16: u16,
        properties: u16,
        read_permission: u8,
        write_permission: u8,
        data: *mut u8,
        data_len: u16,
    ) -> u16;
    pub fn att_db_util_add_characteristic_uuid128(
        uuid128: *const u8,
        properties: u16,
        read_permission: u8,
        write_permission: u8,
        data: *mut u8,
        data_len: u16,
    ) -> u16;
    pub fn att_db_util_get_address() -> *mut u8;
    pub fn att_db_util_get_size() -> u16;
    pub fn att_db_util_hash_len() -> u16;
    pub fn att_db_util_hash_init();
    pub fn att_db_util_hash_get_next() -> u8;
    pub fn att_server_notify(con: u16, attribute_handle: u16, value: *const u8, value_len: u16) -> u8;
    pub fn att_server_indicate(con: u16, attribute_handle: u16, value: *const u8, value_len: u16) -> u8;
    pub fn att_server_get_mtu(con: u16) -> u16;
    pub fn att_server_response_ready(con: u16) -> u8;

    // ---- BTstack: run loop ----
    pub fn btstack_run_loop_get_time_ms() -> u32;
}

#[cfg(feature = "sbc-reference")]
unsafe extern "C" {
    pub fn bluecaster_sbc_ref_new(
        sample_rate: u16,
        channel_mode: c_int,
        blocks: c_int,
        subbands: c_int,
        allocation: c_int,
        bitpool: c_int,
    ) -> *mut core::ffi::c_void;
    pub fn bluecaster_sbc_ref_encode(
        state: *mut core::ffi::c_void,
        pcm: *const i16,
        out: *mut u8,
        out_cap: c_int,
    ) -> c_int;
    pub fn bluecaster_sbc_ref_free(state: *mut core::ffi::c_void);
}
