//! Readers for the BTstack events the LE code uses (btstack_event.h getters
//! of the BTstack version in extern/btstack). Each takes the whole event
//! packet as an [`crate::App`] gets it and returns None for another event
//! or a packet too short for its fields.
//!
//! Addresses are in BTstack's byte order (most significant byte first, as
//! [`crate::Stack`] takes them); 128-bit UUIDs too, as BTstack keeps them
//! (gatt_client_service_t / gatt_client_characteristic_t).

use crate::consts::*;

fn u16_at(p: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*p.get(i)?, *p.get(i + 1)?]))
}

/// bd_addr_t from the 6 bytes at `i` (reversed: on air least significant
/// byte first)
fn addr_at(p: &[u8], i: usize) -> Option<[u8; 6]> {
    let b = p.get(i..i + 6)?;
    Some([b[5], b[4], b[3], b[2], b[1], b[0]])
}

fn event_is(p: &[u8], code: u8) -> bool {
    p.first() == Some(&code)
}

/// The event code (first byte)
pub fn event_type(packet: &[u8]) -> Option<u8> {
    packet.first().copied()
}

/// The subevent code of an HCI_EVENT_META_GAP event
pub fn gap_meta_subevent(packet: &[u8]) -> Option<u8> {
    if !event_is(packet, HCI_EVENT_META_GAP) {
        return None;
    }
    packet.get(2).copied()
}

/// The opcode of a Command Complete
pub fn command_complete_opcode(packet: &[u8]) -> Option<u16> {
    if !event_is(packet, HCI_EVENT_COMMAND_COMPLETE) {
        return None;
    }
    u16_at(packet, 3)
}

// ---- GAP ----

/// GAP_EVENT_ADVERTISING_REPORT or GAP_EVENT_EXTENDED_ADVERTISING_REPORT
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdvReport<'a> {
    /// Legacy: the 8-bit advertising event type; extended: the 16-bit one
    pub event_type: u16,
    /// As reported (extended reports: 0-3, bit 1 set for a resolved identity)
    pub addr_type: u8,
    pub addr: [u8; 6],
    pub rssi: i8,
    pub data: &'a [u8],
    pub extended: bool,
}

pub fn adv_report(packet: &[u8]) -> Option<AdvReport<'_>> {
    match *packet.first()? {
        GAP_EVENT_ADVERTISING_REPORT => {
            let len = *packet.get(11)? as usize;
            Some(AdvReport {
                event_type: *packet.get(2)? as u16,
                addr_type: *packet.get(3)?,
                addr: addr_at(packet, 4)?,
                rssi: *packet.get(10)? as i8,
                data: packet.get(12..12 + len)?,
                extended: false,
            })
        }
        GAP_EVENT_EXTENDED_ADVERTISING_REPORT => {
            let len = *packet.get(25)? as usize;
            Some(AdvReport {
                event_type: u16_at(packet, 2)?,
                addr_type: *packet.get(4)?,
                addr: addr_at(packet, 5)?,
                rssi: *packet.get(15)? as i8,
                data: packet.get(26..26 + len)?,
                extended: true,
            })
        }
        _ => None,
    }
}

/// GAP_SUBEVENT_LE_CONNECTION_COMPLETE (HCI_EVENT_META_GAP)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeConnectionComplete {
    pub status: u8,
    pub handle: u16,
    /// 0 central, 1 peripheral
    pub role: u8,
    pub peer_addr_type: u8,
    pub peer_addr: [u8; 6],
}

pub fn le_connection_complete(packet: &[u8]) -> Option<LeConnectionComplete> {
    if gap_meta_subevent(packet)? != GAP_SUBEVENT_LE_CONNECTION_COMPLETE {
        return None;
    }
    Some(LeConnectionComplete {
        status: *packet.get(3)?,
        handle: u16_at(packet, 4)?,
        role: *packet.get(6)?,
        peer_addr_type: *packet.get(7)?,
        peer_addr: addr_at(packet, 8)?,
    })
}

/// GAP_SUBEVENT_CIG_CREATED (HCI_EVENT_META_GAP)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CigCreated {
    pub status: u8,
    pub cig_id: u8,
    /// In the order of the CIS parameters of [`crate::Stack::cig_create`]
    pub cis_handles: Vec<u16>,
}

pub fn cig_created(packet: &[u8]) -> Option<CigCreated> {
    if gap_meta_subevent(packet)? != GAP_SUBEVENT_CIG_CREATED {
        return None;
    }
    let n = *packet.get(5)? as usize;
    let cis_handles = (0..n).map(|i| u16_at(packet, 6 + 2 * i)).collect::<Option<Vec<u16>>>()?;
    Some(CigCreated { status: *packet.get(3)?, cig_id: *packet.get(4)?, cis_handles })
}

/// GAP_SUBEVENT_BIG_CREATED (HCI_EVENT_META_GAP): after LE Create BIG
/// and the ISO data paths of all BIS
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BigCreated {
    pub status: u8,
    pub big_handle: u8,
    /// In BIS index order
    pub bis_handles: Vec<u16>,
}

pub fn big_created(packet: &[u8]) -> Option<BigCreated> {
    if gap_meta_subevent(packet)? != GAP_SUBEVENT_BIG_CREATED {
        return None;
    }
    let n = *packet.get(5)? as usize;
    let bis_handles = (0..n).map(|i| u16_at(packet, 6 + 2 * i)).collect::<Option<Vec<u16>>>()?;
    Some(BigCreated { status: *packet.get(3)?, big_handle: *packet.get(4)?, bis_handles })
}

/// GAP_SUBEVENT_BIG_TERMINATED (HCI_EVENT_META_GAP): the BIG handle
pub fn big_terminated(packet: &[u8]) -> Option<u8> {
    if gap_meta_subevent(packet)? != GAP_SUBEVENT_BIG_TERMINATED {
        return None;
    }
    packet.get(3).copied()
}

/// GAP_SUBEVENT_CIS_CREATED (HCI_EVENT_META_GAP)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CisCreated {
    pub status: u8,
    pub cig_id: u8,
    pub cis_id: u8,
    pub cis_handle: u16,
    pub acl_handle: u16,
    pub iso_interval_1250us: u16,
}

pub fn cis_created(packet: &[u8]) -> Option<CisCreated> {
    if gap_meta_subevent(packet)? != GAP_SUBEVENT_CIS_CREATED {
        return None;
    }
    Some(CisCreated {
        status: *packet.get(3)?,
        cig_id: *packet.get(4)?,
        cis_id: *packet.get(5)?,
        cis_handle: u16_at(packet, 6)?,
        acl_handle: u16_at(packet, 8)?,
        iso_interval_1250us: u16_at(packet, 10)?,
    })
}

/// GAP_SUBEVENT_BONDING_DELETED (HCI_EVENT_META_GAP)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BondingDeleted {
    pub addr_type: u8,
    pub addr: [u8; 6],
    /// le_device_db index
    pub index: u8,
    pub handle: u16,
}

pub fn bonding_deleted(packet: &[u8]) -> Option<BondingDeleted> {
    if gap_meta_subevent(packet)? != GAP_SUBEVENT_BONDING_DELETED {
        return None;
    }
    Some(BondingDeleted {
        addr_type: *packet.get(3)?,
        addr: addr_at(packet, 4)?,
        index: *packet.get(10)?,
        handle: u16_at(packet, 11)?,
    })
}

// ---- HCI ----

/// HCI_EVENT_ENCRYPTION_CHANGE or _V2
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncryptionChange {
    pub status: u8,
    pub handle: u16,
    pub enabled: bool,
}

pub fn encryption_change(packet: &[u8]) -> Option<EncryptionChange> {
    match *packet.first()? {
        HCI_EVENT_ENCRYPTION_CHANGE | HCI_EVENT_ENCRYPTION_CHANGE_V2 => Some(EncryptionChange {
            status: *packet.get(2)?,
            handle: u16_at(packet, 3)?,
            enabled: *packet.get(5)? != 0,
        }),
        _ => None,
    }
}

/// HCI_EVENT_DISCONNECTION_COMPLETE: (status, handle, reason)
pub fn disconnection_complete(packet: &[u8]) -> Option<(u8, u16, u8)> {
    if !event_is(packet, HCI_EVENT_DISCONNECTION_COMPLETE) {
        return None;
    }
    Some((*packet.get(2)?, u16_at(packet, 3)?, *packet.get(5)?))
}

/// The answer to LE Read Buffer Size v2 (Command Complete)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeBufferSizeV2 {
    pub status: u8,
    pub acl_packet_length: u16,
    pub acl_packets: u8,
    pub iso_packet_length: u16,
    pub iso_packets: u8,
}

pub fn le_read_buffer_size_v2(packet: &[u8]) -> Option<LeBufferSizeV2> {
    if command_complete_opcode(packet)? != HCI_OPCODE_LE_READ_BUFFER_SIZE_V2 {
        return None;
    }
    let status = *packet.get(5)?;
    if status != 0 {
        return Some(LeBufferSizeV2 {
            status,
            acl_packet_length: 0,
            acl_packets: 0,
            iso_packet_length: 0,
            iso_packets: 0,
        });
    }
    Some(LeBufferSizeV2 {
        status,
        acl_packet_length: u16_at(packet, 6)?,
        acl_packets: *packet.get(8)?,
        iso_packet_length: u16_at(packet, 9)?,
        iso_packets: *packet.get(11)?,
    })
}

/// HCI_EVENT_CIS_CAN_SEND_NOW (to the ISO handler)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CisCanSendNow {
    pub cig_id: u8,
    pub cis_id: u8,
    pub cis_handle: u16,
    pub stream_index: u8,
    pub group_complete: bool,
}

pub fn cis_can_send_now(packet: &[u8]) -> Option<CisCanSendNow> {
    if !event_is(packet, HCI_EVENT_CIS_CAN_SEND_NOW) {
        return None;
    }
    Some(CisCanSendNow {
        cig_id: *packet.get(2)?,
        cis_id: *packet.get(3)?,
        cis_handle: u16_at(packet, 4)?,
        stream_index: *packet.get(6)?,
        group_complete: *packet.get(7)? != 0,
    })
}

/// HCI_EVENT_BIS_CAN_SEND_NOW (to the ISO handler): one per BIS of the
/// BIG, in BIS order, once every BIS has room. BTstack 1.8.1 emits it
/// with a length of 4: the BIS handle it stores after the index is cut off
/// (`hci_emit_bis_can_send_now` does not advance past it), so it is None
/// then and the BIS is known by its index only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BisCanSendNow {
    pub big_handle: u8,
    pub bis_index: u8,
    pub bis_handle: Option<u16>,
}

pub fn bis_can_send_now(packet: &[u8]) -> Option<BisCanSendNow> {
    if !event_is(packet, HCI_EVENT_BIS_CAN_SEND_NOW) {
        return None;
    }
    Some(BisCanSendNow { big_handle: *packet.get(2)?, bis_index: *packet.get(3)?, bis_handle: u16_at(packet, 4) })
}

// ---- SM ----

/// The connection handle of an SM event (all of them have it first)
pub fn sm_handle(packet: &[u8]) -> Option<u16> {
    match *packet.first()? {
        SM_EVENT_JUST_WORKS_REQUEST..=SM_EVENT_REENCRYPTION_COMPLETE => u16_at(packet, 2),
        _ => None,
    }
}

/// SM_EVENT_IDENTITY_CREATED or SM_EVENT_IDENTITY_RESOLVING_SUCCEEDED
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmIdentity {
    pub handle: u16,
    pub addr_type: u8,
    pub addr: [u8; 6],
    pub identity_addr_type: u8,
    pub identity_addr: [u8; 6],
    /// le_device_db index
    pub index: u16,
}

pub fn sm_identity(packet: &[u8]) -> Option<SmIdentity> {
    match *packet.first()? {
        SM_EVENT_IDENTITY_CREATED | SM_EVENT_IDENTITY_RESOLVING_SUCCEEDED => Some(SmIdentity {
            handle: u16_at(packet, 2)?,
            addr_type: *packet.get(4)?,
            addr: addr_at(packet, 5)?,
            identity_addr_type: *packet.get(11)?,
            identity_addr: addr_at(packet, 12)?,
            index: u16_at(packet, 18)?,
        }),
        _ => None,
    }
}

/// SM_EVENT_IDENTITY_RESOLVING_FAILED: (handle, address type, address)
pub fn sm_identity_resolving_failed(packet: &[u8]) -> Option<(u16, u8, [u8; 6])> {
    if !event_is(packet, SM_EVENT_IDENTITY_RESOLVING_FAILED) {
        return None;
    }
    Some((u16_at(packet, 2)?, *packet.get(4)?, addr_at(packet, 5)?))
}

/// SM_EVENT_PAIRING_COMPLETE
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmPairingComplete {
    pub handle: u16,
    pub addr_type: u8,
    pub addr: [u8; 6],
    pub status: u8,
    pub reason: u8,
}

pub fn sm_pairing_complete(packet: &[u8]) -> Option<SmPairingComplete> {
    if !event_is(packet, SM_EVENT_PAIRING_COMPLETE) {
        return None;
    }
    Some(SmPairingComplete {
        handle: u16_at(packet, 2)?,
        addr_type: *packet.get(4)?,
        addr: addr_at(packet, 5)?,
        status: *packet.get(11)?,
        reason: *packet.get(12)?,
    })
}

/// SM_EVENT_REENCRYPTION_COMPLETE: (handle, address type, address, status)
pub fn sm_reencryption_complete(packet: &[u8]) -> Option<(u16, u8, [u8; 6], u8)> {
    if !event_is(packet, SM_EVENT_REENCRYPTION_COMPLETE) {
        return None;
    }
    Some((u16_at(packet, 2)?, *packet.get(4)?, addr_at(packet, 5)?, *packet.get(11)?))
}

// ---- GATT client ----

/// gatt_client_service_t
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Service {
    pub start_handle: u16,
    pub end_handle: u16,
    /// 0 for a UUID outside the Bluetooth base
    pub uuid16: u16,
    pub uuid128: [u8; 16],
}

/// gatt_client_characteristic_t
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Characteristic {
    pub start_handle: u16,
    pub value_handle: u16,
    pub end_handle: u16,
    pub properties: u16,
    pub uuid16: u16,
    pub uuid128: [u8; 16],
}

/// The Bluetooth base UUID 0000xxxx-0000-1000-8000-00805F9B34FB, most
/// significant octet first
const BLUETOOTH_BASE_UUID: [u8; 16] =
    [0, 0, 0, 0, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0x80, 0x5F, 0x9B, 0x34, 0xFB];

/// The 128-bit UUID at `i` (on air least significant octet first) as BTstack
/// keeps it, and its 16-bit form (0 outside the Bluetooth base)
fn uuid_at(p: &[u8], i: usize) -> Option<(u16, [u8; 16])> {
    let raw = p.get(i..i + 16)?;
    let mut uuid128 = [0u8; 16];
    for (k, b) in raw.iter().rev().enumerate() {
        uuid128[k] = *b;
    }
    let uuid16 =
        if uuid128[4..] == BLUETOOTH_BASE_UUID[4..] { u16::from_be_bytes([uuid128[2], uuid128[3]]) } else { 0 };
    Some((uuid16, uuid128))
}

fn service_at(p: &[u8], i: usize) -> Option<Service> {
    let (uuid16, uuid128) = uuid_at(p, i + 4)?;
    Some(Service { start_handle: u16_at(p, i)?, end_handle: u16_at(p, i + 2)?, uuid16, uuid128 })
}

/// GATT_EVENT_SERVICE_QUERY_RESULT: (connection, service)
pub fn gatt_service_query_result(packet: &[u8]) -> Option<(u16, Service)> {
    if !event_is(packet, GATT_EVENT_SERVICE_QUERY_RESULT) {
        return None;
    }
    Some((u16_at(packet, 2)?, service_at(packet, 8)?))
}

/// GATT_EVENT_INCLUDED_SERVICE_QUERY_RESULT: (connection, handle of the
/// include declaration, included service)
pub fn gatt_included_service_query_result(packet: &[u8]) -> Option<(u16, u16, Service)> {
    if !event_is(packet, GATT_EVENT_INCLUDED_SERVICE_QUERY_RESULT) {
        return None;
    }
    Some((u16_at(packet, 2)?, u16_at(packet, 8)?, service_at(packet, 10)?))
}

/// GATT_EVENT_CHARACTERISTIC_QUERY_RESULT: (connection, characteristic)
pub fn gatt_characteristic_query_result(packet: &[u8]) -> Option<(u16, Characteristic)> {
    if !event_is(packet, GATT_EVENT_CHARACTERISTIC_QUERY_RESULT) {
        return None;
    }
    let (uuid16, uuid128) = uuid_at(packet, 16)?;
    Some((
        u16_at(packet, 2)?,
        Characteristic {
            start_handle: u16_at(packet, 8)?,
            value_handle: u16_at(packet, 10)?,
            end_handle: u16_at(packet, 12)?,
            properties: u16_at(packet, 14)?,
            uuid16,
            uuid128,
        },
    ))
}

/// A piece of a value (GATT_EVENT_LONG_CHARACTERISTIC_VALUE_QUERY_RESULT)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LongValue<'a> {
    pub con: u16,
    pub value_handle: u16,
    pub offset: u16,
    pub value: &'a [u8],
}

pub fn gatt_long_value_query_result(packet: &[u8]) -> Option<LongValue<'_>> {
    if !event_is(packet, GATT_EVENT_LONG_CHARACTERISTIC_VALUE_QUERY_RESULT) {
        return None;
    }
    let len = u16_at(packet, 12)? as usize;
    Some(LongValue {
        con: u16_at(packet, 2)?,
        value_handle: u16_at(packet, 8)?,
        offset: u16_at(packet, 10)?,
        value: packet.get(14..14 + len)?,
    })
}

/// GATT_EVENT_NOTIFICATION or GATT_EVENT_INDICATION
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValueUpdate<'a> {
    pub con: u16,
    pub value_handle: u16,
    pub value: &'a [u8],
    pub indication: bool,
}

pub fn gatt_value_update(packet: &[u8]) -> Option<ValueUpdate<'_>> {
    let indication = match *packet.first()? {
        GATT_EVENT_NOTIFICATION => false,
        GATT_EVENT_INDICATION => true,
        _ => return None,
    };
    let len = u16_at(packet, 10)? as usize;
    Some(ValueUpdate {
        con: u16_at(packet, 2)?,
        value_handle: u16_at(packet, 8)?,
        value: packet.get(12..12 + len)?,
        indication,
    })
}

/// GATT_EVENT_QUERY_COMPLETE: (connection, ATT status)
pub fn gatt_query_complete(packet: &[u8]) -> Option<(u16, u8)> {
    if !event_is(packet, GATT_EVENT_QUERY_COMPLETE) {
        return None;
    }
    Some((u16_at(packet, 2)?, *packet.get(8)?))
}

/// The connection handle of a GATT client event (all have it first)
pub fn gatt_con(packet: &[u8]) -> Option<u16> {
    match *packet.first()? {
        GATT_EVENT_QUERY_COMPLETE..=GATT_EVENT_INDICATION => u16_at(packet, 2),
        _ => None,
    }
}

// ---- ATT server ----

/// ATT_EVENT_MTU_EXCHANGE_COMPLETE: (connection, MTU)
pub fn att_mtu_exchange_complete(packet: &[u8]) -> Option<(u16, u16)> {
    if !event_is(packet, ATT_EVENT_MTU_EXCHANGE_COMPLETE) {
        return None;
    }
    Some((u16_at(packet, 2)?, u16_at(packet, 4)?))
}

/// ATT_EVENT_HANDLE_VALUE_INDICATION_COMPLETE: (status, connection,
/// attribute handle)
pub fn att_indication_complete(packet: &[u8]) -> Option<(u8, u16, u16)> {
    if !event_is(packet, ATT_EVENT_HANDLE_VALUE_INDICATION_COMPLETE) {
        return None;
    }
    Some((*packet.get(2)?, u16_at(packet, 3)?, u16_at(packet, 5)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_adv_report() {
        // event type 0 (ADV_IND), random, C0:11:22:33:44:55 on air reversed,
        // RSSI -60, 3 bytes of data
        let p = [0xDA, 15, 0x00, 0x01, 0x55, 0x44, 0x33, 0x22, 0x11, 0xC0, 0xC4, 3, 2, 1, 6];
        let r = adv_report(&p).unwrap();
        assert_eq!(r.event_type, 0);
        assert_eq!(r.addr_type, 1);
        assert_eq!(r.addr, [0xC0, 0x11, 0x22, 0x33, 0x44, 0x55]);
        assert_eq!(r.rssi, -60);
        assert_eq!(r.data, &[2, 1, 6]);
        assert!(!r.extended);
        // Data length beyond the packet
        let mut short = p;
        short[11] = 9;
        assert!(adv_report(&short).is_none());
    }

    #[test]
    fn extended_adv_report() {
        let mut p = vec![0xDB, 0, 0x13, 0x00, 0x00, 6, 5, 4, 3, 2, 1, 1, 0, 0, 0x7F, 0xB0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        p.push(2);
        p.extend_from_slice(&[0x01, 0x06]);
        p[1] = (p.len() - 2) as u8;
        let r = adv_report(&p).unwrap();
        assert_eq!(r.event_type, 0x0013);
        assert_eq!(r.addr, [1, 2, 3, 4, 5, 6]);
        assert_eq!(r.rssi, -80);
        assert_eq!(r.data, &[1, 6]);
        assert!(r.extended);
    }

    #[test]
    fn meta_gap_events() {
        let mut p = vec![0xE7, 0, GAP_SUBEVENT_LE_CONNECTION_COMPLETE, 0, 0x40, 0x00, 0, 1, 6, 5, 4, 3, 2, 1];
        p.resize(36, 0);
        let c = le_connection_complete(&p).unwrap();
        assert_eq!((c.status, c.handle, c.role, c.peer_addr_type), (0, 0x0040, 0, 1));
        assert_eq!(c.peer_addr, [1, 2, 3, 4, 5, 6]);
        assert!(cig_created(&p).is_none());

        let p = [0xE7, 0, GAP_SUBEVENT_CIG_CREATED, 0, 1, 2, 0x60, 0x00, 0x61, 0x00];
        assert_eq!(cig_created(&p), Some(CigCreated { status: 0, cig_id: 1, cis_handles: vec![0x60, 0x61] }));
        assert!(cig_created(&p[..8]).is_none());

        let p = [0xE7, 0, GAP_SUBEVENT_CIS_CREATED, 0, 1, 2, 0x60, 0x00, 0x40, 0x00, 8, 0, 1, 1, 1, 1, 1];
        let c = cis_created(&p).unwrap();
        assert_eq!((c.cig_id, c.cis_id, c.cis_handle, c.acl_handle, c.iso_interval_1250us), (1, 2, 0x60, 0x40, 8));

        let p = [0xE7, 6, GAP_SUBEVENT_BIG_CREATED, 0, 0, 2, 0x10, 0x00, 0x11, 0x00];
        assert_eq!(big_created(&p), Some(BigCreated { status: 0, big_handle: 0, bis_handles: vec![0x10, 0x11] }));
        assert!(big_created(&p[..9]).is_none());
        assert_eq!(big_terminated(&[0xE7, 2, GAP_SUBEVENT_BIG_TERMINATED, 0]), Some(0));
        assert!(big_terminated(&p).is_none());
        let p = [HCI_EVENT_BIS_CAN_SEND_NOW, 4, 0, 1, 0x11, 0x00];
        assert_eq!(bis_can_send_now(&p), Some(BisCanSendNow { big_handle: 0, bis_index: 1, bis_handle: Some(0x11) }));
        // As BTstack 1.8.1 emits it
        assert_eq!(bis_can_send_now(&p[..4]), Some(BisCanSendNow { big_handle: 0, bis_index: 1, bis_handle: None }));

        let p = [0xE7, 0, GAP_SUBEVENT_BONDING_DELETED, 0, 6, 5, 4, 3, 2, 1, 3, 0x40, 0x00];
        let b = bonding_deleted(&p).unwrap();
        assert_eq!((b.addr, b.index, b.handle), ([1, 2, 3, 4, 5, 6], 3, 0x40));
    }

    #[test]
    fn buffer_size_v2() {
        // LE ACL 251 x 8, ISO 310 x 6
        let p = [0x0E, 10, 1, 0x60, 0x20, 0, 0xFB, 0, 8, 0x36, 0x01, 6];
        let b = le_read_buffer_size_v2(&p).unwrap();
        assert_eq!((b.acl_packet_length, b.acl_packets, b.iso_packet_length, b.iso_packets), (251, 8, 310, 6));
        // Unknown HCI Command (LE hidden, or no ISO support)
        let b = le_read_buffer_size_v2(&[0x0E, 4, 1, 0x60, 0x20, 1]).unwrap();
        assert_eq!((b.status, b.iso_packets), (1, 0));
        assert!(le_read_buffer_size_v2(&[0x0E, 4, 1, 0x02, 0x20, 0]).is_none());
    }

    #[test]
    fn hci_events() {
        let e = encryption_change(&[0x08, 4, 0, 0x40, 0, 1]).unwrap();
        assert_eq!(e, EncryptionChange { status: 0, handle: 0x40, enabled: true });
        let e = encryption_change(&[0x59, 5, 0, 0x41, 0, 0, 16]).unwrap();
        assert!(!e.enabled);
        assert_eq!(disconnection_complete(&[0x05, 4, 0, 0x40, 0, 0x13]), Some((0, 0x40, 0x13)));
        let c = cis_can_send_now(&[0x6C, 6, 1, 2, 0x60, 0, 1, 1]).unwrap();
        assert_eq!((c.cig_id, c.cis_id, c.cis_handle, c.stream_index, c.group_complete), (1, 2, 0x60, 1, true));
    }

    #[test]
    fn sm_events() {
        let mut p = vec![SM_EVENT_IDENTITY_RESOLVING_SUCCEEDED, 0, 0x40, 0, 1, 6, 5, 4, 3, 2, 0x41, 0];
        p.extend_from_slice(&[0xF6, 0xE5, 0xD4, 0xC3, 0xB2, 0xA1, 2, 0]);
        let i = sm_identity(&p).unwrap();
        assert_eq!(i.addr, [0x41, 2, 3, 4, 5, 6]);
        assert_eq!((i.identity_addr_type, i.identity_addr, i.index), (0, [0xA1, 0xB2, 0xC3, 0xD4, 0xE5, 0xF6], 2));
        assert_eq!(sm_handle(&p), Some(0x40));

        let p = [SM_EVENT_PAIRING_COMPLETE, 0, 0x40, 0, 0, 6, 5, 4, 3, 2, 1, 0x02, 0x05, 0];
        let c = sm_pairing_complete(&p).unwrap();
        assert_eq!((c.handle, c.status, c.reason), (0x40, 2, 5));
        assert_eq!(
            sm_reencryption_complete(&[SM_EVENT_REENCRYPTION_COMPLETE, 0, 0x40, 0, 0, 6, 5, 4, 3, 2, 1, 0]).unwrap().3,
            0
        );
        assert!(sm_handle(&[0xB3, 0, 0x40, 0]).is_none());
    }

    #[test]
    fn gatt_events() {
        // PACS (0x1850) service 0x0010..0x0020
        let mut p = vec![GATT_EVENT_SERVICE_QUERY_RESULT, 0, 0x40, 0, 0, 0, 0, 0, 0x10, 0, 0x20, 0];
        let mut uuid = BLUETOOTH_BASE_UUID;
        uuid[2..4].copy_from_slice(&[0x18, 0x50]);
        p.extend(uuid.iter().rev());
        let (con, s) = gatt_service_query_result(&p).unwrap();
        assert_eq!((con, s.start_handle, s.end_handle, s.uuid16), (0x40, 0x10, 0x20, 0x1850));
        assert_eq!(s.uuid128, uuid);

        // A vendor characteristic: no 16-bit UUID
        let mut p = vec![GATT_EVENT_CHARACTERISTIC_QUERY_RESULT, 0, 0x40, 0, 0, 0, 0, 0];
        p.extend_from_slice(&[0x11, 0, 0x12, 0, 0x13, 0, 0x1A, 0]);
        p.extend((1..=16u8).rev());
        let (_, c) = gatt_characteristic_query_result(&p).unwrap();
        assert_eq!((c.start_handle, c.value_handle, c.end_handle, c.properties, c.uuid16), (0x11, 0x12, 0x13, 0x1A, 0));
        assert_eq!(c.uuid128[0], 1);

        let mut p = vec![GATT_EVENT_INCLUDED_SERVICE_QUERY_RESULT, 0, 0x40, 0, 0, 0, 0, 0, 0x15, 0, 0x30, 0, 0x38, 0];
        p.extend(uuid.iter().rev());
        let (_, include, s) = gatt_included_service_query_result(&p).unwrap();
        assert_eq!((include, s.start_handle, s.end_handle), (0x15, 0x30, 0x38));

        let p = [GATT_EVENT_LONG_CHARACTERISTIC_VALUE_QUERY_RESULT, 0, 0x40, 0, 0, 0, 0, 0, 0x12, 0, 22, 0, 2, 0, 7, 8];
        let v = gatt_long_value_query_result(&p).unwrap();
        assert_eq!((v.value_handle, v.offset, v.value), (0x12, 22, &[7u8, 8][..]));

        let p = [GATT_EVENT_INDICATION, 0, 0x40, 0, 0, 0, 0, 0, 0x12, 0, 1, 0, 9];
        let u = gatt_value_update(&p).unwrap();
        assert!(u.indication);
        assert_eq!(u.value, &[9]);
        assert_eq!(gatt_query_complete(&[GATT_EVENT_QUERY_COMPLETE, 0, 0x40, 0, 0, 0, 0, 0, 0x1F]), Some((0x40, 0x1F)));
        assert_eq!(gatt_con(&p), Some(0x40));
    }
}
