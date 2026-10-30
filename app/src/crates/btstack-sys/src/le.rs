//! [`Stack`] for LE (experimental LE Audio): setup, GAP, SM, the LE device
//! DB and its TLV store, GATT client, the PC's GATT database and ATT server,
//! CIG / CIS and ISO data. Usable on the run loop thread only, like the rest
//! of [`Stack`]; see the crate docs for the order of the setup calls.

use std::cell::Cell;
use std::ffi::{CString, c_int};

use crate::consts::*;
use crate::events::Characteristic;
use crate::stack::{INIT_ATT, INIT_LE, addr_str, initialized, mark};
use crate::{Stack, att, ffi, transport};

/// gap_connection_type_t
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionType {
    Invalid,
    Acl,
    Sco,
    Le,
}

/// An LE bond (le_device_db_info)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeDbEntry {
    /// The identity address type (BD_ADDR_TYPE_LE_*)
    pub addr_type: u8,
    pub addr: [u8; 6],
    pub irk: [u8; 16],
}

/// The keys of an LE bond (le_device_db_encryption_get); the LTK most
/// significant octet first, as BTstack keeps it
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeEncryption {
    pub ediv: u16,
    pub rand: [u8; 8],
    pub ltk: [u8; 16],
    pub key_size: u8,
    pub authenticated: bool,
    pub authorized: bool,
    pub secure_connection: bool,
}

/// One CIS of [`CigParams`] (le_audio_cis_params_t)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CisParams {
    pub cis_id: u8,
    pub max_sdu_c_to_p: u16,
    pub max_sdu_p_to_c: u16,
    pub phy_c_to_p: u8,
    pub phy_p_to_c: u8,
    pub rtn_c_to_p: u8,
    pub rtn_p_to_c: u8,
}

/// LE Set CIG Parameters (le_audio_cig_params_t)
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CigParams {
    pub cig_id: u8,
    pub sdu_interval_c_to_p: u32,
    pub sdu_interval_p_to_c: u32,
    pub worst_case_sca: u8,
    pub packing: u8,
    pub framing: u8,
    pub max_transport_latency_c_to_p: u16,
    pub max_transport_latency_p_to_c: u16,
    /// 1..=4 (MAX_NR_CIS)
    pub cis: Vec<CisParams>,
}

thread_local! {
    /// att_db_util has a database (att_db_init was called) that may be
    /// changed (not lent out by att_db_with_mut)
    static ATT_DB: Cell<bool> = const { Cell::new(false) };
    /// att_db_with_mut runs: att_db_util must not reallocate its buffer
    static ATT_DB_LENT: Cell<bool> = const { Cell::new(false) };
}

pub(crate) fn reset() {
    ATT_DB.with(|d| d.set(false));
    ATT_DB_LENT.with(|d| d.set(false));
}

fn len16(data: &[u8]) -> Option<u16> {
    u16::try_from(data.len()).ok()
}

impl Stack {
    // ---- Setup ----

    /// Hides the controller's LE support from BTstack (the default of every
    /// run; see crate::transport). Takes effect only before
    /// [`Stack::hci_init`] / [`Stack::install_transport`]: false afterwards.
    pub fn set_le_hidden(&self, hidden: bool) -> bool {
        let ok = transport::set_le_hidden(hidden);
        if !ok {
            tracing::warn!(target: "hci", "LE hidden: can only be changed before hci_init");
        }
        ok
    }

    pub fn le_hidden(&self) -> bool {
        transport::le_hidden()
    }

    /// Whether the controller has the isochronous channels of LE Audio (LE
    /// Read Buffer Size v2 in its supported commands), from its answer
    /// during HCI init, also while LE is hidden. None before that answer.
    pub fn controller_iso(&self) -> Option<bool> {
        transport::controller_iso()
    }

    /// After hci_init (and l2cap_init): LE bonds in the TLV file `tlv_path`
    /// (le_device_db_tlv + btstack_tlv_windows, the format of v1's
    /// le_audio_bonds.tlv; the PC's GATT server keeps its bonded clients'
    /// configurations there too), SM (LE Secure Connections, no input / no
    /// output, bonding; events to [`crate::Source::Sm`]), the GATT client
    /// and the ISO handler ([`crate::Source::Iso`]). False when the path
    /// cannot be used: LE works, but bonds are not kept.
    pub fn le_init(&self, tlv_path: &str) -> bool {
        let path = CString::new(tlv_path.replace('\0', "")).unwrap_or_default();
        // SAFETY: on the BTstack thread after hci_init; the shim copies the
        // path into its own buffer.
        let ok = unsafe { ffi::bluecaster_le_init(path.as_ptr()) } != 0;
        mark(INIT_LE);
        tracing::debug!(target: "le", "le_init: bond store {tlv_path}, usable {ok}");
        if !ok {
            tracing::warn!(target: "le", "LE Audio: cannot use {tlv_path} for the bonds, keeping none");
        }
        ok
    }

    pub fn le_initialized(&self) -> bool {
        initialized(INIT_LE)
    }

    /// btstack_run_loop_get_time_ms
    pub fn time_ms(&self) -> u32 {
        // SAFETY: on the BTstack thread; reads the run loop's clock.
        unsafe { ffi::btstack_run_loop_get_time_ms() }
    }

    // ---- GAP ----

    /// Scan type active / passive, interval and window in 0.625 ms units
    pub fn le_set_scan_params(&self, active: bool, interval: u16, window: u16, filter_policy: u8) -> u8 {
        // SAFETY: on the BTstack thread.
        let status = unsafe { ffi::gap_set_scan_params(active as u8, interval, window, filter_policy) };
        tracing::debug!(target: "le", "scan params: active {active}, interval 0x{interval:04X}, window 0x{window:04X}, filter {filter_policy}: 0x{status:02X}");
        status
    }

    /// Advertising reports come as GAP_EVENT_(EXTENDED_)ADVERTISING_REPORT
    /// ([`crate::events::adv_report`])
    pub fn le_start_scan(&self) {
        tracing::debug!(target: "le", "LE scan start");
        // SAFETY: on the BTstack thread.
        unsafe { ffi::gap_start_scan() }
    }

    pub fn le_stop_scan(&self) {
        tracing::debug!(target: "le", "LE scan stop");
        // SAFETY: on the BTstack thread.
        unsafe { ffi::gap_stop_scan() }
    }

    /// Ends with GAP_SUBEVENT_LE_CONNECTION_COMPLETE
    pub fn le_connect(&self, addr: &[u8; 6], addr_type: u8) -> u8 {
        // SAFETY: on the BTstack thread; addr is 6 bytes (copied).
        let status = unsafe { ffi::bluecaster_gap_connect(addr.as_ptr(), addr_type) };
        tracing::debug!(target: "le", "LE connect {} (type {addr_type}): 0x{status:02X}", addr_str(addr));
        status
    }

    pub fn le_connect_cancel(&self) -> u8 {
        // SAFETY: on the BTstack thread.
        let status = unsafe { ffi::gap_connect_cancel() };
        tracing::debug!(target: "le", "LE connect cancel: 0x{status:02X}");
        status
    }

    pub fn connection_type(&self, con: u16) -> ConnectionType {
        // SAFETY: on the BTstack thread.
        match unsafe { ffi::bluecaster_gap_connection_type(con) } {
            1 => ConnectionType::Acl,
            2 => ConnectionType::Sco,
            3 => ConnectionType::Le,
            _ => ConnectionType::Invalid,
        }
    }

    /// Forgets the LE bond (GAP_SUBEVENT_BONDING_DELETED follows)
    pub fn le_delete_bonding(&self, addr_type: u8, addr: &[u8; 6]) {
        if !self.le_initialized() {
            return;
        }
        tracing::debug!(target: "le", "deleting the LE bond of {} (type {addr_type})", addr_str(addr));
        // SAFETY: on the BTstack thread after le_init; addr copied.
        unsafe { ffi::bluecaster_gap_delete_bonding(addr_type, addr.as_ptr()) }
    }

    /// HCI Disconnect for any connection handle, a CIS included (whose
    /// Disconnection Complete sets BTstack's ISO stream idle). Check
    /// [`Stack::can_send_command_now`] first.
    pub fn cmd_disconnect(&self, handle: u16, reason: u8) -> u8 {
        // SAFETY: on the BTstack thread.
        let status = unsafe { ffi::bluecaster_cmd_disconnect(handle, reason) };
        tracing::debug!(target: "le", "HCI disconnect 0x{handle:04X} (reason 0x{reason:02X}): 0x{status:02X}");
        status
    }

    // ---- SM (after le_init) ----

    pub fn sm_request_pairing(&self, con: u16) {
        if self.le_initialized() {
            tracing::debug!(target: "le", "SM: request pairing on 0x{con:04X}");
            // SAFETY: on the BTstack thread after le_init (sm_init).
            unsafe { ffi::sm_request_pairing(con) }
        }
    }

    pub fn sm_just_works_confirm(&self, con: u16) {
        if self.le_initialized() {
            tracing::debug!(target: "le", "SM: just works confirm on 0x{con:04X}");
            // SAFETY: as above.
            unsafe { ffi::sm_just_works_confirm(con) }
        }
    }

    pub fn sm_numeric_comparison_confirm(&self, con: u16) {
        if self.le_initialized() {
            tracing::debug!(target: "le", "SM: numeric comparison confirm on 0x{con:04X}");
            // SAFETY: as above.
            unsafe { ffi::sm_numeric_comparison_confirm(con) }
        }
    }

    pub fn sm_bonding_decline(&self, con: u16) {
        if self.le_initialized() {
            tracing::debug!(target: "le", "SM: bonding decline on 0x{con:04X}");
            // SAFETY: as above.
            unsafe { ffi::sm_bonding_decline(con) }
        }
    }

    /// Resolves a random address against the IRKs of all bonds; the answer
    /// is SM_EVENT_IDENTITY_RESOLVING_SUCCEEDED / _FAILED. 0, or BTSTACK_BUSY
    /// (queued anyway) or an error.
    pub fn sm_address_resolution_lookup(&self, addr_type: u8, addr: &[u8; 6]) -> i32 {
        if !self.le_initialized() {
            return ERROR_CODE_COMMAND_DISALLOWED as i32;
        }
        let mut a = *addr;
        // SAFETY: on the BTstack thread after le_init; a is a writable copy.
        let r = unsafe { ffi::sm_address_resolution_lookup(addr_type, a.as_mut_ptr()) };
        tracing::debug!(target: "le", "SM: address resolution lookup {} (type {addr_type}): {r}", addr_str(addr));
        r
    }

    /// The le_device_db index of the bond of the connection, None if not
    /// bonded
    pub fn sm_le_device_index(&self, con: u16) -> Option<usize> {
        if !self.le_initialized() {
            return None;
        }
        // SAFETY: on the BTstack thread after le_init.
        let i = unsafe { ffi::sm_le_device_index(con) };
        usize::try_from(i).ok()
    }

    // ---- LE device DB (after le_init) ----

    pub fn le_device_db_max_count(&self) -> usize {
        if !self.le_initialized() {
            return 0;
        }
        // SAFETY: on the BTstack thread after le_init.
        usize::try_from(unsafe { ffi::le_device_db_max_count() }).unwrap_or(0)
    }

    /// The bond in slot `index`, None for an empty slot
    pub fn le_device_db_info(&self, index: usize) -> Option<LeDbEntry> {
        if index >= self.le_device_db_max_count() {
            return None;
        }
        let mut addr_type: c_int = BD_ADDR_TYPE_UNKNOWN as c_int;
        let mut addr = [0u8; 6];
        let mut irk = [0u8; 16];
        // SAFETY: on the BTstack thread after le_init; index in range; the
        // pointers have the sizes BTstack writes.
        unsafe { ffi::le_device_db_info(index as c_int, &mut addr_type, addr.as_mut_ptr(), irk.as_mut_ptr()) };
        let addr_type = u8::try_from(addr_type).ok().filter(|t| *t != BD_ADDR_TYPE_UNKNOWN)?;
        Some(LeDbEntry { addr_type, addr, irk })
    }

    pub fn le_device_db_encryption(&self, index: usize) -> Option<LeEncryption> {
        self.le_device_db_info(index)?;
        let (mut ediv, mut rand, mut ltk) = (0u16, [0u8; 8], [0u8; 16]);
        let (mut key_size, mut authenticated, mut authorized, mut secure) = (0 as c_int, 0 as c_int, 0, 0);
        // SAFETY: on the BTstack thread after le_init; index holds a bond;
        // the pointers have the sizes BTstack writes.
        unsafe {
            ffi::le_device_db_encryption_get(
                index as c_int,
                &mut ediv,
                rand.as_mut_ptr(),
                ltk.as_mut_ptr(),
                &mut key_size,
                &mut authenticated,
                &mut authorized,
                &mut secure,
            )
        };
        Some(LeEncryption {
            ediv,
            rand,
            ltk,
            key_size: key_size as u8,
            authenticated: authenticated != 0,
            authorized: authorized != 0,
            secure_connection: secure != 0,
        })
    }

    // ---- TLV store of the bonds (after le_init) ----

    pub fn tlv_get(&self, tag: u32) -> Option<Vec<u8>> {
        // SAFETY: on the BTstack thread; a NULL buffer asks for the length.
        let len = unsafe { ffi::bluecaster_tlv_get(tag, std::ptr::null_mut(), 0) };
        let len = usize::try_from(len).ok().filter(|n| *n > 0)?;
        let mut buf = vec![0u8; len];
        // SAFETY: buf has len bytes.
        let n = unsafe { ffi::bluecaster_tlv_get(tag, buf.as_mut_ptr(), len as u32) };
        buf.truncate(usize::try_from(n).ok()?);
        Some(buf)
    }

    pub fn tlv_store(&self, tag: u32, value: &[u8]) -> bool {
        // SAFETY: on the BTstack thread; value valid for its length (copied).
        let ok = unsafe { ffi::bluecaster_tlv_store(tag, value.as_ptr(), value.len() as u32) == 0 };
        tracing::debug!(target: "le", "TLV store tag 0x{tag:08X}, {} bytes: {ok}", value.len());
        ok
    }

    pub fn tlv_delete(&self, tag: u32) {
        tracing::debug!(target: "le", "TLV delete tag 0x{tag:08X}");
        // SAFETY: on the BTstack thread.
        unsafe { ffi::bluecaster_tlv_delete(tag) }
    }

    // ---- GATT client (after le_init; results to Source::GattClient) ----

    /// GATT_EVENT_SERVICE_QUERY_RESULT for each, then GATT_EVENT_QUERY_COMPLETE.
    /// One query per connection at a time (else GATT_CLIENT_IN_WRONG_STATE /
    /// GATT_CLIENT_BUSY: try again later).
    pub fn gatt_discover_primary_services(&self, con: u16) -> u8 {
        if !self.le_initialized() {
            return ERROR_CODE_COMMAND_DISALLOWED;
        }
        // SAFETY: on the BTstack thread after le_init (gatt_client_init).
        let status = unsafe { ffi::bluecaster_gatt_discover_primary_services(con) };
        tracing::debug!(target: "le", "GATT discover primary services on 0x{con:04X}: 0x{status:02X}");
        status
    }

    /// GATT_EVENT_INCLUDED_SERVICE_QUERY_RESULT for each
    pub fn gatt_find_included_services(&self, con: u16, start_handle: u16, end_handle: u16) -> u8 {
        if !self.le_initialized() {
            return ERROR_CODE_COMMAND_DISALLOWED;
        }
        // SAFETY: as above; the service is copied by BTstack.
        let status = unsafe { ffi::bluecaster_gatt_find_included_services(con, start_handle, end_handle) };
        tracing::debug!(target: "le", "GATT find included services 0x{start_handle:04X}-0x{end_handle:04X} on 0x{con:04X}: 0x{status:02X}");
        status
    }

    /// GATT_EVENT_CHARACTERISTIC_QUERY_RESULT for each
    pub fn gatt_discover_characteristics(&self, con: u16, start_handle: u16, end_handle: u16) -> u8 {
        if !self.le_initialized() {
            return ERROR_CODE_COMMAND_DISALLOWED;
        }
        // SAFETY: as above.
        let status = unsafe { ffi::bluecaster_gatt_discover_characteristics(con, start_handle, end_handle) };
        tracing::debug!(target: "le", "GATT discover characteristics 0x{start_handle:04X}-0x{end_handle:04X} on 0x{con:04X}: 0x{status:02X}");
        status
    }

    /// GATT_EVENT_LONG_CHARACTERISTIC_VALUE_QUERY_RESULT for each piece
    pub fn gatt_read_long_value(&self, con: u16, value_handle: u16) -> u8 {
        if !self.le_initialized() {
            return ERROR_CODE_COMMAND_DISALLOWED;
        }
        // SAFETY: as above.
        let status = unsafe { ffi::bluecaster_gatt_read_long_value(con, value_handle) };
        tracing::debug!(target: "le", "GATT read 0x{value_handle:04X} on 0x{con:04X}: 0x{status:02X}");
        status
    }

    /// Write with response; a long write when the value does not fit the
    /// ATT MTU (as v1). At most [`ATT_VALUE_MAX`] bytes; the shim keeps a
    /// copy until GATT_EVENT_QUERY_COMPLETE.
    pub fn gatt_write_value(&self, con: u16, value_handle: u16, value: &[u8]) -> u8 {
        if !self.le_initialized() {
            return ERROR_CODE_COMMAND_DISALLOWED;
        }
        if value.len() > ATT_VALUE_MAX {
            tracing::debug!(target: "le", "GATT write 0x{value_handle:04X}: {} bytes over the maximum", value.len());
            return ERROR_CODE_INVALID_HCI_COMMAND_PARAMETERS;
        }
        let mtu = self.gatt_mtu(con).unwrap_or(ATT_DEFAULT_MTU);
        let long = value.len() + 3 > mtu as usize;
        // SAFETY: on the BTstack thread after le_init; value valid for its
        // length (copied into the connection's slot).
        let status = unsafe {
            ffi::bluecaster_gatt_write_value(con, value_handle, value.as_ptr(), value.len() as u16, long as c_int)
        };
        tracing::debug!(target: "le", "GATT write 0x{value_handle:04X} on 0x{con:04X} (mtu {mtu}, long {long}) {value:02X?}: 0x{status:02X}");
        status
    }

    /// Write command (no response, no completion event)
    pub fn gatt_write_without_response(&self, con: u16, value_handle: u16, value: &[u8]) -> u8 {
        if !self.le_initialized() {
            return ERROR_CODE_COMMAND_DISALLOWED;
        }
        let Some(len) = len16(value) else { return ERROR_CODE_INVALID_HCI_COMMAND_PARAMETERS };
        // SAFETY: as above; BTstack copies the value into its buffer at once.
        let status = unsafe { ffi::bluecaster_gatt_write_without_response(con, value_handle, value.as_ptr(), len) };
        tracing::debug!(target: "le", "GATT write command 0x{value_handle:04X} on 0x{con:04X} {value:02X?}: 0x{status:02X}");
        status
    }

    /// Writes the characteristic's client configuration descriptor
    /// ([`GATT_CLIENT_CHARACTERISTICS_CONFIGURATION_NOTIFICATION`] /
    /// `_INDICATION`)
    pub fn gatt_write_client_configuration(&self, con: u16, chr: &Characteristic, configuration: u16) -> u8 {
        if !self.le_initialized() {
            return ERROR_CODE_COMMAND_DISALLOWED;
        }
        // SAFETY: on the BTstack thread after le_init; the shim copies the
        // characteristic.
        let status = unsafe {
            ffi::bluecaster_gatt_write_client_configuration(
                con,
                chr.start_handle,
                chr.value_handle,
                chr.end_handle,
                chr.properties,
                chr.uuid16,
                chr.uuid128.as_ptr(),
                configuration,
            )
        };
        tracing::debug!(target: "le", "GATT client configuration 0x{configuration:04X} of 0x{:04X} (uuid16 0x{:04X}) on 0x{con:04X}: 0x{status:02X}",
            chr.value_handle,
            chr.uuid16
        );
        status
    }

    /// The connection's ATT MTU as the GATT client has it
    pub fn gatt_mtu(&self, con: u16) -> Option<u16> {
        if !self.le_initialized() {
            return None;
        }
        let mut mtu = 0u16;
        // SAFETY: on the BTstack thread after le_init; mtu writable.
        let status = unsafe { ffi::gatt_client_get_mtu(con, &mut mtu) };
        (status == ERROR_CODE_SUCCESS).then_some(mtu)
    }

    /// Every notification / indication of `con` goes to
    /// [`crate::Source::GattClient`] (GATT_EVENT_NOTIFICATION / _INDICATION)
    /// until the connection goes down. False when all 16 slots are in use.
    pub fn gatt_listen(&self, con: u16) -> bool {
        // SAFETY: on the BTstack thread after le_init; the listener is static
        // in C.
        let ok = self.le_initialized() && unsafe { ffi::bluecaster_gatt_listen(con) } != 0;
        tracing::debug!(target: "le", "GATT listen on 0x{con:04X}: {ok}");
        ok
    }

    pub fn gatt_stop_listening(&self, con: u16) {
        if self.le_initialized() {
            // SAFETY: on the BTstack thread.
            unsafe { ffi::bluecaster_gatt_stop_listening(con) }
        }
    }

    // ---- The PC's GATT database (att_db_util), before att_server_init ----

    /// Starts a new database (attribute handles from 1)
    pub fn att_db_init(&self) {
        if ATT_DB_LENT.with(|d| d.get()) {
            return;
        }
        // SAFETY: on the BTstack thread; att_db_util allocates its buffer.
        unsafe { ffi::att_db_util_init() };
        ATT_DB.with(|d| d.set(true));
    }

    /// A primary or secondary service; returns its handle
    pub fn att_db_add_service_uuid16(&self, uuid16: u16, primary: bool) -> u16 {
        if !ATT_DB.with(|d| d.get()) {
            return 0;
        }
        // SAFETY: on the BTstack thread after att_db_init.
        unsafe {
            if primary {
                ffi::att_db_util_add_service_uuid16(uuid16)
            } else {
                ffi::att_db_util_add_secondary_service_uuid16(uuid16)
            }
        }
    }

    /// `uuid128` most significant octet first
    pub fn att_db_add_service_uuid128(&self, uuid128: &[u8; 16], primary: bool) -> u16 {
        if !ATT_DB.with(|d| d.get()) {
            return 0;
        }
        // SAFETY: on the BTstack thread after att_db_init; 16 bytes read.
        unsafe {
            if primary {
                ffi::att_db_util_add_service_uuid128(uuid128.as_ptr())
            } else {
                ffi::att_db_util_add_secondary_service_uuid128(uuid128.as_ptr())
            }
        }
    }

    pub fn att_db_add_included_service(&self, start_handle: u16, end_handle: u16, uuid16: u16) -> u16 {
        if !ATT_DB.with(|d| d.get()) {
            return 0;
        }
        // SAFETY: on the BTstack thread after att_db_init.
        unsafe { ffi::att_db_util_add_included_service_uuid16(start_handle, end_handle, uuid16) }
    }

    /// A characteristic; returns its value handle (its client configuration
    /// descriptor, added for NOTIFY / INDICATE, is the next handle). With
    /// [`ATT_PROPERTY_DYNAMIC`] the value is served by
    /// [`crate::App::on_att_read`] / `on_att_write`, else `value` is stored.
    pub fn att_db_add_characteristic_uuid16(
        &self,
        uuid16: u16,
        properties: u16,
        read_security: u8,
        write_security: u8,
        value: &[u8],
    ) -> u16 {
        let Some(len) = len16(value) else { return 0 };
        if !ATT_DB.with(|d| d.get()) {
            return 0;
        }
        // SAFETY: on the BTstack thread after att_db_init; att_db_util
        // copies the value (it does not write through the pointer).
        unsafe {
            ffi::att_db_util_add_characteristic_uuid16(
                uuid16,
                properties,
                read_security,
                write_security,
                value.as_ptr() as *mut u8,
                len,
            )
        }
    }

    /// As [`Stack::att_db_add_characteristic_uuid16`], `uuid128` most
    /// significant octet first
    pub fn att_db_add_characteristic_uuid128(
        &self,
        uuid128: &[u8; 16],
        properties: u16,
        read_security: u8,
        write_security: u8,
        value: &[u8],
    ) -> u16 {
        let Some(len) = len16(value) else { return 0 };
        if !ATT_DB.with(|d| d.get()) {
            return 0;
        }
        // SAFETY: as above; 16 bytes of UUID read.
        unsafe {
            ffi::att_db_util_add_characteristic_uuid128(
                uuid128.as_ptr(),
                properties,
                read_security,
                write_security,
                value.as_ptr() as *mut u8,
                len,
            )
        }
    }

    /// Size of the database in bytes
    pub fn att_db_size(&self) -> usize {
        if !ATT_DB.with(|d| d.get()) {
            return 0;
        }
        // SAFETY: on the BTstack thread after att_db_init.
        unsafe { ffi::att_db_util_get_size() as usize }
    }

    /// The bytes the Database Hash is computed over (att_db_util_hash_*:
    /// AES-CMAC with a zero key over them, Core Vol 3 Part G 7.3)
    pub fn att_db_hash_input(&self) -> Vec<u8> {
        if !ATT_DB.with(|d| d.get()) {
            return Vec::new();
        }
        // SAFETY: on the BTstack thread after att_db_init; the iterator
        // reads the database att_db_util holds.
        unsafe {
            let n = ffi::att_db_util_hash_len() as usize;
            ffi::att_db_util_hash_init();
            (0..n).map(|_| ffi::att_db_util_hash_get_next()).collect()
        }
    }

    /// `f` on the database bytes (e.g. to write a static value such as the
    /// Database Hash: le_audio::db_hash::patch_att_db). None without a
    /// database.
    pub fn att_db_with_mut<R>(&self, f: impl FnOnce(&mut [u8]) -> R) -> Option<R> {
        if !ATT_DB.with(|d| d.get()) {
            return None;
        }
        // SAFETY: on the BTstack thread after att_db_init: att_db_util's
        // buffer holds att_db_util_get_size() bytes. It is only reallocated
        // by att_db_util calls, which the Stack methods do not make while it
        // is lent (ATT_DB false, ATT_DB_LENT true); the slice does not
        // outlive f.
        let (p, len) = unsafe { (ffi::att_db_util_get_address(), ffi::att_db_util_get_size() as usize) };
        if p.is_null() {
            return None;
        }
        ATT_DB.with(|d| d.set(false));
        ATT_DB_LENT.with(|d| d.set(true));
        struct Restore;
        impl Drop for Restore {
            fn drop(&mut self) {
                ATT_DB.with(|d| d.set(true));
                ATT_DB_LENT.with(|d| d.set(false));
            }
        }
        let _restore = Restore;
        // SAFETY: see above.
        Some(f(unsafe { std::slice::from_raw_parts_mut(p, len) }))
    }

    /// Starts the ATT server on the database built (BTstack keeps a pointer
    /// to it: no att_db_add_* afterwards). Events to [`crate::Source::AttServer`],
    /// reads / writes to [`crate::App::on_att_read`] / `on_att_write`.
    pub fn att_server_init(&self) -> bool {
        if !self.le_initialized() || !ATT_DB.with(|d| d.get()) {
            return false;
        }
        tracing::debug!(target: "le", "ATT server init ({} bytes of database)", self.att_db_size());
        // SAFETY: on the BTstack thread after le_init and att_db_init.
        unsafe { ffi::bluecaster_att_server_init() };
        mark(INIT_ATT);
        true
    }

    /// Queues a notification (or indication) of `value_handle` to `con`,
    /// sent when BTstack can, cut to ATT_MTU - 3. Whether the client
    /// subscribed is the caller's business.
    pub fn att_server_send(&self, con: u16, value_handle: u16, value: &[u8], indication: bool) {
        if initialized(INIT_ATT) {
            tracing::trace!(target: "le", "ATT {} 0x{value_handle:04X} on 0x{con:04X}: {value:02X?}", if indication { "indicate" } else { "notify" });
            att::send(con, value_handle, value, indication);
        }
    }

    pub fn att_server_mtu(&self, con: u16) -> u16 {
        if !initialized(INIT_ATT) {
            return ATT_DEFAULT_MTU;
        }
        // SAFETY: on the BTstack thread after att_server_init.
        unsafe { ffi::att_server_get_mtu(con) }
    }

    // ---- CIG / CIS / ISO (after le_init) ----

    /// LE Set CIG Parameters (GAP_SUBEVENT_CIG_CREATED follows). The shim
    /// keeps the CIG's storage until BTstack removes it; at most 4 CIGs.
    pub fn cig_create(&self, params: &CigParams) -> u8 {
        if !self.le_initialized() {
            return ERROR_CODE_COMMAND_DISALLOWED;
        }
        if params.cis.is_empty() || params.cis.len() > 4 {
            tracing::debug!(target: "le", "CIG create: {} CIS, not 1-4", params.cis.len());
            return ERROR_CODE_INVALID_HCI_COMMAND_PARAMETERS;
        }
        let cis: Vec<ffi::CisParamsC> = params
            .cis
            .iter()
            .map(|c| ffi::CisParamsC {
                cis_id: c.cis_id,
                phy_c_to_p: c.phy_c_to_p,
                phy_p_to_c: c.phy_p_to_c,
                rtn_c_to_p: c.rtn_c_to_p,
                rtn_p_to_c: c.rtn_p_to_c,
                max_sdu_c_to_p: c.max_sdu_c_to_p,
                max_sdu_p_to_c: c.max_sdu_p_to_c,
            })
            .collect();
        // SAFETY: on the BTstack thread after le_init; cis has num_cis
        // entries, copied by the shim.
        let status = unsafe {
            ffi::bluecaster_cig_create(
                params.cig_id,
                params.sdu_interval_c_to_p,
                params.sdu_interval_p_to_c,
                params.worst_case_sca,
                params.packing,
                params.framing,
                params.max_transport_latency_c_to_p,
                params.max_transport_latency_p_to_c,
                cis.len() as u8,
                cis.as_ptr(),
            )
        };
        tracing::debug!(target: "le", "CIG create {params:?}: 0x{status:02X}");
        status
    }

    /// Closes the CIG's CIS and removes it (LE Remove CIG)
    pub fn cig_remove(&self, cig_id: u8) -> u8 {
        if !self.le_initialized() {
            return ERROR_CODE_COMMAND_DISALLOWED;
        }
        // SAFETY: on the BTstack thread after le_init.
        let status = unsafe { ffi::gap_cig_remove(cig_id) };
        tracing::debug!(target: "le", "CIG remove {cig_id}: 0x{status:02X}");
        status
    }

    /// LE Create CIS for the CIG's CIS (one per CIS of [`CigParams`], with
    /// the ACL of each); GAP_SUBEVENT_CIS_CREATED for each
    pub fn cis_create(&self, cig_id: u8, cis_handles: &[u16], acl_handles: &[u16]) -> u8 {
        if !self.le_initialized() {
            return ERROR_CODE_COMMAND_DISALLOWED;
        }
        if cis_handles.len() != acl_handles.len() || cis_handles.is_empty() || cis_handles.len() > 4 {
            tracing::debug!(target: "le", "CIS create: CIS {cis_handles:04X?} / ACL {acl_handles:04X?} do not pair up");
            return ERROR_CODE_INVALID_HCI_COMMAND_PARAMETERS;
        }
        let mut cis = [HCI_CON_HANDLE_INVALID; 4];
        let mut acl = [HCI_CON_HANDLE_INVALID; 4];
        cis[..cis_handles.len()].copy_from_slice(cis_handles);
        acl[..acl_handles.len()].copy_from_slice(acl_handles);
        // SAFETY: on the BTstack thread after le_init; BTstack reads one
        // entry per CIS of the CIG (at most 4) and copies them.
        let status = unsafe { ffi::gap_cis_create(cig_id, cis.as_mut_ptr(), acl.as_mut_ptr()) };
        tracing::debug!(target: "le", "CIS create CIG {cig_id}: CIS {cis_handles:04X?}, ACL {acl_handles:04X?}: 0x{status:02X}");
        status
    }

    /// SDUs BTstack queues per CIS before it stops asking (CIS can-send-now)
    pub fn set_num_iso_packets_to_queue(&self, packets: u8) {
        tracing::debug!(target: "le", "ISO packets to queue per CIS: {packets}");
        // SAFETY: on the BTstack thread; sets a field.
        unsafe { ffi::hci_set_num_iso_packets_to_queue(packets) }
    }

    /// HCI_EVENT_CIS_CAN_SEND_NOW for every outgoing CIS of the CIG of
    /// `cis_handle` ([`crate::Source::Iso`])
    pub fn request_cis_can_send_now(&self, cis_handle: u16) -> u8 {
        if !self.le_initialized() {
            return ERROR_CODE_COMMAND_DISALLOWED;
        }
        // SAFETY: on the BTstack thread after le_init.
        let status = unsafe { ffi::hci_request_cis_can_send_now_events(cis_handle) };
        tracing::trace!(target: "le", "CIS can send now request 0x{cis_handle:04X}: 0x{status:02X}");
        status
    }

    /// One complete SDU as an HCI ISO data packet (no time stamp), at most
    /// [`ISO_SDU_MAX`] bytes; call it on CIS can-send-now
    pub fn iso_send(&self, cis_handle: u16, sequence_number: u16, sdu: &[u8]) -> u8 {
        if !self.le_initialized() {
            return ERROR_CODE_COMMAND_DISALLOWED;
        }
        if sdu.len() > ISO_SDU_MAX {
            tracing::debug!(target: "le", "ISO SDU of {} bytes over the maximum on 0x{cis_handle:04X}", sdu.len());
            return ERROR_CODE_INVALID_HCI_COMMAND_PARAMETERS;
        }
        // SAFETY: on the BTstack thread after le_init; the shim checks the
        // packet buffer and copies the SDU.
        let status = unsafe { ffi::bluecaster_iso_send(cis_handle, sequence_number, sdu.as_ptr(), sdu.len() as u16) };
        tracing::trace!(target: "le", "ISO send 0x{cis_handle:04X} seq {sequence_number}, {} bytes: 0x{status:02X}", sdu.len());
        status
    }

    /// Tests only: hands the packets received so far to BTstack now, from
    /// within the app's call (as BTstack handling a request inside an app
    /// call would)
    #[doc(hidden)]
    pub fn test_process_incoming_now(&self) {
        transport::pump();
    }

    /// LE teardown (crate::run, through Stack::deinit)
    pub(crate) fn le_deinit(&self, att_server_started: bool) {
        tracing::debug!(target: "le", "LE deinit (ATT server {att_server_started})");
        // SAFETY: on the BTstack thread after the run loop stopped; le_init
        // ran in this run.
        unsafe { ffi::bluecaster_le_deinit(att_server_started as c_int) };
        reset();
    }
}
