//! The PC's GATT server (v1 le_audio::GattServer, le_audio_gatt_server.cpp).
//!
//! The PC is the GATT server for the services a headset reads from the
//! audio source: GAP and GATT (built by the core) and the modules' services
//! (GMCS, GTBS, TMAS, GMAS). The database is built with BTstack's
//! att_db_util while the stack starts ([`GattServerBuilder`], in
//! `LeModule::add_gatt_services` only); reads and writes are dispatched by
//! attribute handle to the module that added the characteristic
//! (`LeModule::on_att_read` / `on_att_write`), synchronously from BTstack's
//! att_server callbacks.
//!
//! The CCC descriptor of a NOTIFY / INDICATE characteristic is created at
//! value handle + 1 and handled here: [`GattServer::notify`] only sends to
//! clients that enabled it, and queues until ATT can send. Long reads
//! (offsets) and prepared writes are handled here; modules see whole
//! values. CCC values of bonded clients persist (BTstack's att_server in
//! the bond TLV, replayed on re-encryption); the GATT service has Service
//! Changed and a Database Hash ([`le_audio::db_hash`]); a database that
//! differs from the stored hash drops the stored CCC values and makes a
//! Service Changed indication due for clients that had it enabled (v1
//! check_stored_client_configs). Keep the first handles (GAP, then Service
//! Changed and its CCC) as they are.
//!
//! The builder records the database (att_db_util hands out handles in
//! order, so the handles are known at once) and [`GattServer::start`]
//! builds it in BTstack.

use std::collections::{BTreeMap, HashMap};

use btstack_sys::Stack;
use btstack_sys::consts::{
    ATT_ERROR_INVALID_ATTRIBUTE_VALUE_LENGTH, ATT_ERROR_INVALID_OFFSET, ATT_PROPERTY_DYNAMIC,
    ATT_TRANSACTION_MODE_ACTIVE, ATT_TRANSACTION_MODE_CANCEL, ATT_TRANSACTION_MODE_EXECUTE,
    ATT_TRANSACTION_MODE_VALIDATE, GATT_CLIENT_CHARACTERISTICS_CONFIGURATION_INDICATION,
    GATT_CLIENT_CHARACTERISTICS_CONFIGURATION_NOTIFICATION,
};
pub use le_audio::db_hash::AttUuid;
use le_audio::defs::{
    UUID_CHR_APPEARANCE, UUID_CHR_DATABASE_HASH, UUID_CHR_DEVICE_NAME, UUID_CHR_SERVICE_CHANGED, UUID_SERVICE_GAP,
    UUID_SERVICE_GATT,
};

/// ATT_PROPERTY_* for [`GattServerBuilder::add_characteristic`]
pub const ATT_PROPERTY_READ: u16 = 0x02;
pub const ATT_PROPERTY_WRITE_WITHOUT_RESPONSE: u16 = 0x04;
pub const ATT_PROPERTY_WRITE: u16 = 0x08;
pub const ATT_PROPERTY_NOTIFY: u16 = 0x10;
pub const ATT_PROPERTY_INDICATE: u16 = 0x20;

/// Security of a read / write (BTstack ATT_SECURITY_*)
pub const ATT_SECURITY_NONE: u8 = 0;
pub const ATT_SECURITY_ENCRYPTED: u8 = 1;

/// att_server.c's NVN_NUM_GATT_SERVER_CCC (btstack_config.h)
const NVN_NUM_GATT_SERVER_CCC: u32 = 256;

/// BTstack's persistent CCC store (att_server.c, private there): one TLV
/// tag 'BTC' + slot per CCC value of a bonded client
fn tag_ccc(slot: u32) -> u32 {
    (u32::from(b'B') << 24) | (u32::from(b'T') << 16) | (u32::from(b'C') << 8) | (slot & 0xFF)
}

/// 'BTDB': the Database Hash the stored CCC values were written for
const TAG_DB_HASH: u32 = (b'B' as u32) << 24 | (b'T' as u32) << 16 | (b'D' as u32) << 8 | b'B' as u32;

/// Ours: a Service Changed indication is due for this le_device_db index
fn tag_service_changed(index: usize) -> u32 {
    (u32::from(b'A') << 24) | (u32::from(b'W') << 16) | (u32::from(b'S') << 8) | (index as u32 & 0xFF)
}

/// A read of a module's characteristic
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttRead {
    /// Connection handle of the client
    pub con: u16,
    /// Value handle
    pub handle: u16,
    /// > 0: Read Blob, the rest of a long read
    pub offset: u16,
}

/// A write of a module's characteristic (the whole value)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttWrite<'a> {
    pub con: u16,
    /// Value handle
    pub handle: u16,
    pub value: &'a [u8],
    /// Write Without Response / Write Command
    pub without_response: bool,
}

/// Builds the modules' services (`LeModule::add_gatt_services`): the
/// characteristics added here belong to the module the builder was made
/// for
pub struct GattServerBuilder<'a> {
    pub(crate) server: &'a mut GattServer,
    /// Index of the module in the registry
    pub(crate) owner: usize,
}

impl GattServerBuilder<'_> {
    /// Starts a service (primary, or secondary to include); returns its
    /// start handle
    pub fn begin_service(&mut self, uuid: AttUuid, primary: bool) -> u16 {
        self.server.add_service(uuid, primary)
    }

    /// Adds a characteristic; returns its value handle (CCC: + 1 with
    /// NOTIFY / INDICATE). `properties`: ATT_PROPERTY_*; `read_security` /
    /// `write_security`: ATT_SECURITY_*.
    pub fn add_characteristic(&mut self, uuid: AttUuid, properties: u16, read_security: u8, write_security: u8) -> u16 {
        self.server.add_char(uuid, properties, read_security, write_security, Some(self.owner), None)
    }

    /// Ends the service; returns its end handle
    pub fn end_service(&mut self) -> u16 {
        self.server.last_handle
    }
}

/// One step of building the database (replayed into att_db_util)
#[derive(Debug, Clone)]
enum DbOp {
    Service { uuid: AttUuid, primary: bool, handle: u16 },
    Char { uuid: AttUuid, properties: u16, read_security: u8, write_security: u8, value: Option<Vec<u8>>, handle: u16 },
}

/// What a dynamic attribute is
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Attr {
    /// A characteristic value; owner None: the core's own (GAP, Service
    /// Changed)
    Value { owner: Option<usize> },
    /// The CCC descriptor of this value handle
    Ccc { value_handle: u16 },
}

/// What [`GattServer::write`] asks of the caller
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum WriteAction {
    /// Answered (0 or an ATT error)
    Done(u8),
    /// Whole values for modules: (owner, value handle, value) in handle
    /// order; the answer is the first error, else 0
    Modules(Vec<(usize, u16, Vec<u8>)>),
}

/// The server at run time (`ctx.gatt_server()`)
pub struct GattServer {
    /// Owner module per value handle
    owners: HashMap<u16, usize>,
    ops: Vec<DbOp>,
    next_handle: u16,
    last_handle: u16,
    attrs: HashMap<u16, Attr>,
    /// The core's own readable values (GAP)
    core_values: HashMap<u16, Vec<u8>>,
    /// (con, value handle) -> CCC
    ccc: BTreeMap<(u16, u16), u16>,
    /// Prepared (long) writes in progress: (con, handle) -> value
    prepared: BTreeMap<(u16, u16), Vec<u8>>,
    mtus: HashMap<u16, u16>,
    /// Notifications / indications for the core to send: (con, handle,
    /// value, indication)
    outbox: Vec<(u16, u16, Vec<u8>, bool)>,
    started: bool,
    service_changed: u16,
    db_hash_handle: u16,
    /// As sent: least significant octet first
    db_hash: [u8; 16],
}

impl GattServer {
    /// Sends to `con` if it enabled notifications of `value_handle` (cut to
    /// ATT_MTU - 3); false if it did not or the link is gone
    pub fn notify(&mut self, con: u16, value_handle: u16, value: &[u8]) -> bool {
        self.queue(con, value_handle, false, value)
    }

    /// As [`GattServer::notify`], for indications
    pub fn indicate(&mut self, con: u16, value_handle: u16, value: &[u8]) -> bool {
        self.queue(con, value_handle, true, value)
    }

    /// The CCC value `con` wrote for `value_handle` (0 = none)
    pub fn client_config(&self, con: u16, value_handle: u16) -> u16 {
        self.ccc.get(&(con, value_handle)).copied().unwrap_or(0)
    }

    /// Connections that have written any CCC
    pub fn clients(&self) -> Vec<u16> {
        let mut out: Vec<u16> = Vec::new();
        for &(con, _) in self.ccc.keys() {
            if out.last() != Some(&con) {
                out.push(con);
            }
        }
        out
    }

    /// ATT_MTU of a connection (23 until exchanged)
    pub fn mtu(&self, con: u16) -> u16 {
        self.mtus.get(&con).copied().unwrap_or(btstack_sys::consts::ATT_DEFAULT_MTU)
    }

    // ---- core only ----

    /// A new session: empty database
    pub(crate) fn new() -> GattServer {
        GattServer {
            owners: HashMap::new(),
            ops: Vec::new(),
            next_handle: 1,
            last_handle: 0,
            attrs: HashMap::new(),
            core_values: HashMap::new(),
            ccc: BTreeMap::new(),
            prepared: BTreeMap::new(),
            mtus: HashMap::new(),
            outbox: Vec::new(),
            started: false,
            service_changed: 0,
            db_hash_handle: 0,
            db_hash: [0; 16],
        }
    }

    /// GAP and GATT services (Service Changed, Database Hash)
    pub(crate) fn build_base(&mut self) {
        // GAP: device name and appearance (Generic Computer)
        self.add_service(AttUuid::Uuid16(UUID_SERVICE_GAP), true);
        let name = self.add_char(AttUuid::Uuid16(UUID_CHR_DEVICE_NAME), ATT_PROPERTY_READ, 0, 0, None, None);
        self.core_values.insert(name, b"BlueCaster".to_vec());
        let appearance = self.add_char(AttUuid::Uuid16(UUID_CHR_APPEARANCE), ATT_PROPERTY_READ, 0, 0, None, None);
        self.core_values.insert(appearance, vec![0x80, 0x00]);
        // GATT: Service Changed (the DB never changes while connected;
        // indicated to bonded clients after an BlueCaster update changed it),
        // then the Database Hash (static value, filled in by start()). Keep
        // this layout: the handles up to the Service Changed CCC must stay
        // the same in every version, check_stored_client_configs() relies
        // on them.
        self.add_service(AttUuid::Uuid16(UUID_SERVICE_GATT), true);
        self.service_changed =
            self.add_char(AttUuid::Uuid16(UUID_CHR_SERVICE_CHANGED), ATT_PROPERTY_INDICATE, 0, 0, None, None);
        self.db_hash_handle = self.add_char(
            AttUuid::Uuid16(UUID_CHR_DATABASE_HASH),
            ATT_PROPERTY_READ,
            ATT_SECURITY_NONE,
            ATT_SECURITY_NONE,
            None,
            Some(vec![0; 16]),
        );
    }

    /// The builder for module `owner`'s add_gatt_services
    pub(crate) fn builder(&mut self, owner: usize) -> GattServerBuilder<'_> {
        GattServerBuilder { server: self, owner }
    }

    fn add_service(&mut self, uuid: AttUuid, primary: bool) -> u16 {
        let handle = self.take_handles(1);
        self.ops.push(DbOp::Service { uuid, primary, handle });
        handle
    }

    /// A characteristic: declaration, value (dynamic unless `value` is a
    /// static one), CCC for NOTIFY / INDICATE. Returns the value handle.
    fn add_char(
        &mut self,
        uuid: AttUuid,
        properties: u16,
        read_security: u8,
        write_security: u8,
        owner: Option<usize>,
        value: Option<Vec<u8>>,
    ) -> u16 {
        let decl = self.take_handles(2);
        let handle = decl + 1;
        if value.is_none() {
            self.attrs.insert(handle, Attr::Value { owner });
        }
        if let Some(o) = owner {
            self.owners.insert(handle, o);
        }
        if properties & (ATT_PROPERTY_NOTIFY | ATT_PROPERTY_INDICATE) != 0 {
            let ccc = self.take_handles(1);
            self.attrs.insert(ccc, Attr::Ccc { value_handle: handle });
        }
        self.ops.push(DbOp::Char { uuid, properties, read_security, write_security, value, handle });
        handle
    }

    fn take_handles(&mut self, n: u16) -> u16 {
        let first = self.next_handle;
        self.next_handle += n;
        self.last_handle = self.next_handle - 1;
        first
    }

    /// Builds the database in att_db_util, computes the Database Hash,
    /// checks the stored client configurations and starts att_server
    pub(crate) fn start(&mut self, stack: &Stack) {
        stack.att_db_init();
        for op in &self.ops {
            let (want, got) = match op {
                DbOp::Service { uuid: AttUuid::Uuid16(u), primary, handle } => {
                    (*handle, stack.att_db_add_service_uuid16(*u, *primary))
                }
                DbOp::Service { uuid: AttUuid::Uuid128(u), primary, handle } => {
                    (*handle, stack.att_db_add_service_uuid128(&msb_first(u), *primary))
                }
                DbOp::Char { uuid, properties, read_security, write_security, value, handle } => {
                    let flags = if value.is_some() { *properties } else { *properties | ATT_PROPERTY_DYNAMIC };
                    let v = value.as_deref().unwrap_or(&[]);
                    let got = match uuid {
                        AttUuid::Uuid16(u) => {
                            stack.att_db_add_characteristic_uuid16(*u, flags, *read_security, *write_security, v)
                        }
                        AttUuid::Uuid128(u) => stack.att_db_add_characteristic_uuid128(
                            &msb_first(u),
                            flags,
                            *read_security,
                            *write_security,
                            v,
                        ),
                    };
                    (*handle, got)
                }
            };
            if want != got {
                tracing::error!("LE Audio: GATT database: attribute at handle 0x{got:04x}, expected 0x{want:04x}");
            }
        }
        self.compute_db_hash(stack);
        self.check_stored_client_configs(stack);
        if !stack.att_server_init() {
            tracing::error!("LE Audio: the GATT server could not start");
            return;
        }
        self.started = true;
        tracing::info!(
            "LE Audio: GATT server started ({} attributes with callbacks, {} bytes DB)",
            self.attrs.len(),
            stack.att_db_size()
        );
    }

    /// Database Hash (Core Vol 3 Part G 7.3): AES-CMAC with a zero key over
    /// the attributes att_db_util selects, written into the static value in
    /// the DB, where BTstack's att_server also looks for it
    fn compute_db_hash(&mut self, stack: &Stack) {
        self.db_hash = le_audio::db_hash::database_hash(&stack.att_db_hash_input());
        let (handle, hash) = (self.db_hash_handle, self.db_hash);
        if stack.att_db_with_mut(|db| le_audio::db_hash::patch_att_db(db, handle, &hash)) != Some(true) {
            tracing::warn!("LE Audio: GATT: the Database Hash could not be stored in the database");
        }
    }

    /// Before att_server_init(): the CCC values BTstack stored for bonded
    /// clients belong to the database with the stored hash. Another
    /// database (an BlueCaster update added or moved characteristics, or a
    /// version from before the Database Hash, which stored no hash): drop
    /// them - BTstack would replay them on the old handles - and remember
    /// which clients had Service Changed indications on, to tell them on
    /// their next encrypted link.
    fn check_stored_client_configs(&mut self, stack: &Stack) {
        if !stack.le_initialized() {
            return;
        }
        let stored = stack.tlv_get(TAG_DB_HASH);
        tracing::debug!("LE Audio: GATT database hash {:02X?}, stored {:02X?}", self.db_hash, stored.as_deref());
        if stored.as_deref() == Some(&self.db_hash[..]) {
            return;
        }
        let mut clients = std::collections::BTreeSet::new();
        let mut notify = std::collections::BTreeSet::new();
        for slot in 0..NVN_NUM_GATT_SERVER_CCC {
            let Some(e) = stack.tlv_get(tag_ccc(slot)) else { continue };
            if e.len() != 8 {
                continue;
            }
            // persistent_ccc_entry_t: seq_nr u32, att_handle u16, value u8,
            // device_index u8
            let (att_handle, value, device_index) = (u16::from_le_bytes([e[4], e[5]]), e[6], e[7]);
            clients.insert(device_index);
            if att_handle == self.service_changed + 1
                && u16::from(value) & GATT_CLIENT_CHARACTERISTICS_CONFIGURATION_INDICATION != 0
            {
                notify.insert(device_index);
            }
            stack.tlv_delete(tag_ccc(slot));
        }
        stack.tlv_store(TAG_DB_HASH, &self.db_hash);
        for &index in &notify {
            stack.tlv_store(tag_service_changed(index as usize), &[1]);
        }
        if !clients.is_empty() {
            tracing::info!(
                "LE Audio: GATT database changed since the last session ({}): client configurations of {} bonded \
                 client(s) dropped, Service Changed due for {}",
                if stored.is_some() { "other hash" } else { "no hash stored" },
                clients.len(),
                notify.len()
            );
        }
    }

    /// An LE link is encrypted: a bonded client (le_device_db `index`) that
    /// saw an older database gets the Service Changed indication now
    pub(crate) fn on_encrypted(&mut self, stack: &Stack, con: u16, index: usize) {
        let tag = tag_service_changed(index);
        if stack.tlv_get(tag).is_none() {
            return;
        }
        stack.tlv_delete(tag);
        // The client had indications on (its CCC was dropped with the old
        // database): the whole range changed
        *self.ccc.entry((con, self.service_changed)).or_default() |=
            GATT_CLIENT_CHARACTERISTICS_CONFIGURATION_INDICATION;
        tracing::info!("LE Audio: GATT database changed since bonded client 0x{con:04x} saw it: Service Changed");
        let sc = self.service_changed;
        self.indicate(con, sc, &[0x01, 0x00, 0xFF, 0xFF]);
    }

    /// A bond was deleted: no Service Changed due for it any more
    pub(crate) fn on_bonding_deleted(&mut self, stack: &Stack, index: usize) {
        tracing::debug!("LE Audio: GATT server: bond {index} deleted, no Service Changed due for it");
        stack.tlv_delete(tag_service_changed(index));
    }

    pub(crate) fn on_mtu(&mut self, con: u16, mtu: u16) {
        tracing::trace!("LE Audio: GATT server: MTU {mtu} on 0x{con:04X}");
        self.mtus.insert(con, mtu);
    }

    /// The module owning `value_handle`, None for the core's own
    /// attributes
    pub(crate) fn owner(&self, value_handle: u16) -> Option<usize> {
        self.owners.get(&value_handle).copied()
    }

    #[cfg(test)]
    pub(crate) fn attr(&self, handle: u16) -> Option<Attr> {
        self.attrs.get(&handle).copied()
    }

    /// A read of the core's own attributes and of CCCs (module values:
    /// None, the core asks the owner)
    pub(crate) fn read_own(&self, con: u16, handle: u16) -> Option<Vec<u8>> {
        match self.attrs.get(&handle)? {
            Attr::Ccc { value_handle } => Some(self.client_config(con, *value_handle).to_le_bytes().to_vec()),
            Attr::Value { owner: None } => Some(self.core_values.get(&handle).cloned().unwrap_or_default()),
            Attr::Value { owner: Some(_) } => None,
        }
    }

    /// A write from BTstack's att_server (v1 write_cb): prepared writes are
    /// assembled per (connection, handle), CCC values kept; whole values of
    /// module characteristics go back to the caller
    pub(crate) fn write(&mut self, con: u16, handle: u16, mode: u16, offset: u16, value: &[u8]) -> WriteAction {
        match mode {
            ATT_TRANSACTION_MODE_ACTIVE => {
                if !self.attrs.contains_key(&handle) {
                    return WriteAction::Done(0);
                }
                let v = self.prepared.entry((con, handle)).or_default();
                let end = offset as usize + value.len();
                if v.len() < end {
                    v.resize(end, 0);
                }
                v[offset as usize..end].copy_from_slice(value);
                return WriteAction::Done(0);
            }
            ATT_TRANSACTION_MODE_VALIDATE => return WriteAction::Done(0),
            ATT_TRANSACTION_MODE_CANCEL => {
                self.prepared.retain(|(c, _), _| *c != con);
                return WriteAction::Done(0);
            }
            ATT_TRANSACTION_MODE_EXECUTE => {
                // BTstack passes handle 0: every prepared value of the
                // connection
                let keys: Vec<(u16, u16)> = self.prepared.keys().filter(|(c, _)| *c == con).copied().collect();
                let mut modules = Vec::new();
                let mut status = 0;
                for key in keys {
                    let Some(v) = self.prepared.remove(&key) else { continue };
                    match self.write_whole(con, key.1, &v) {
                        WriteAction::Done(0) => {}
                        WriteAction::Done(e) => {
                            if status == 0 {
                                status = e;
                            }
                        }
                        WriteAction::Modules(m) => modules.extend(m),
                    }
                }
                return if status != 0 || modules.is_empty() {
                    WriteAction::Done(status)
                } else {
                    WriteAction::Modules(modules)
                };
            }
            _ => {}
        }
        if offset != 0 {
            return WriteAction::Done(ATT_ERROR_INVALID_OFFSET);
        }
        self.write_whole(con, handle, value)
    }

    fn write_whole(&mut self, con: u16, handle: u16, value: &[u8]) -> WriteAction {
        match self.attrs.get(&handle) {
            None => WriteAction::Done(0),
            Some(Attr::Ccc { value_handle }) => {
                if value.len() < 2 {
                    return WriteAction::Done(ATT_ERROR_INVALID_ATTRIBUTE_VALUE_LENGTH);
                }
                let cfg = u16::from_le_bytes([value[0], value[1]]);
                tracing::debug!("LE Audio: GATT server: client 0x{con:04X} CCC of 0x{value_handle:04X} = 0x{cfg:04X}");
                self.ccc.insert((con, *value_handle), cfg);
                WriteAction::Done(0)
            }
            Some(Attr::Value { owner: None }) => WriteAction::Done(btstack_sys::consts::ATT_ERROR_WRITE_NOT_PERMITTED),
            Some(Attr::Value { owner: Some(o) }) => WriteAction::Modules(vec![(*o, handle, value.to_vec())]),
        }
    }

    fn queue(&mut self, con: u16, handle: u16, indication: bool, value: &[u8]) -> bool {
        if !self.started {
            return false;
        }
        let bit = if indication {
            GATT_CLIENT_CHARACTERISTICS_CONFIGURATION_INDICATION
        } else {
            GATT_CLIENT_CHARACTERISTICS_CONFIGURATION_NOTIFICATION
        };
        if self.client_config(con, handle) & bit == 0 {
            tracing::trace!("LE Audio: GATT server: 0x{handle:04X} not enabled by 0x{con:04X}, not sent");
            return false;
        }
        tracing::trace!(
            "LE Audio: GATT server: {} 0x{handle:04X} to 0x{con:04X}: {value:02X?}",
            if indication { "indication" } else { "notification" }
        );
        self.outbox.push((con, handle, value.to_vec(), indication));
        true
    }

    /// Notifications / indications to hand to att_server (it queues them
    /// until it can send and cuts them to ATT_MTU - 3)
    pub(crate) fn take_outbox(&mut self) -> Vec<(u16, u16, Vec<u8>, bool)> {
        std::mem::take(&mut self.outbox)
    }

    pub(crate) fn has_outbox(&self) -> bool {
        !self.outbox.is_empty()
    }

    pub(crate) fn on_disconnected(&mut self, con: u16) {
        tracing::debug!(
            "LE Audio: GATT server: client 0x{con:04X} gone ({} CCC value(s), {} queued value(s) dropped)",
            self.ccc.keys().filter(|(c, _)| *c == con).count(),
            self.outbox.iter().filter(|(c, ..)| *c == con).count()
        );
        // The live values of this link go; a bonded client gets its CCC
        // values back from BTstack's persistent store when it encrypts
        // again (they are replayed through the write callback), an unbonded
        // one starts from none
        self.ccc.retain(|(c, _), _| *c != con);
        self.prepared.retain(|(c, _), _| *c != con);
        self.outbox.retain(|(c, ..)| *c != con);
        self.mtus.remove(&con);
    }
}

/// AttUuid::Uuid128 is on-air order (least significant octet first);
/// att_db_util takes the most significant octet first
fn msb_first(uuid: &[u8; 16]) -> [u8; 16] {
    let mut u = *uuid;
    u.reverse();
    u
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_follow_att_db_util() {
        let mut s = GattServer::new();
        s.build_base();
        // GAP 1, name 2/3, appearance 4/5, GATT 6, Service Changed 7/8 +
        // CCC 9, Database Hash 10/11
        assert_eq!(s.service_changed, 8);
        assert_eq!(s.attr(9), Some(Attr::Ccc { value_handle: 8 }));
        assert_eq!(s.db_hash_handle, 11);
        assert_eq!(s.attr(11), None, "the hash is a static value");
        let mut b = s.builder(3);
        assert_eq!(b.begin_service(AttUuid::Uuid16(0x1849), true), 12);
        assert_eq!(b.add_characteristic(AttUuid::Uuid16(0x2B93), ATT_PROPERTY_READ, 1, 1), 14);
        assert_eq!(b.add_characteristic(AttUuid::Uuid16(0x2BA3), ATT_PROPERTY_READ | ATT_PROPERTY_NOTIFY, 1, 1), 16);
        assert_eq!(b.end_service(), 17);
        assert_eq!(s.owner(16), Some(3));
        assert_eq!(s.owner(8), None);
        assert_eq!(s.read_own(0x40, 3), Some(b"BlueCaster".to_vec()));
        assert_eq!(s.read_own(0x40, 16), None);
    }

    #[test]
    fn ccc_and_prepared_writes() {
        let mut s = GattServer::new();
        s.build_base();
        let h =
            s.builder(0).add_characteristic(AttUuid::Uuid16(0x2BA4), ATT_PROPERTY_WRITE | ATT_PROPERTY_NOTIFY, 1, 1);
        s.started = true;
        assert!(!s.notify(0x40, h, &[1]));
        assert_eq!(s.write(0x40, h + 1, 0, 0, &[1, 0]), WriteAction::Done(0));
        assert_eq!(s.client_config(0x40, h), 1);
        assert_eq!(s.read_own(0x40, h + 1), Some(vec![1, 0]));
        assert!(s.notify(0x40, h, &[1]));
        assert!(!s.indicate(0x40, h, &[1]));
        assert_eq!(s.clients(), [0x40]);
        assert_eq!(s.take_outbox(), [(0x40, h, vec![1], false)]);
        assert_eq!(s.write(0x40, h + 1, 0, 0, &[1]), WriteAction::Done(ATT_ERROR_INVALID_ATTRIBUTE_VALUE_LENGTH));
        // A long write in two pieces, executed with handle 0
        assert_eq!(s.write(0x40, h, ATT_TRANSACTION_MODE_ACTIVE, 0, &[1, 2]), WriteAction::Done(0));
        assert_eq!(s.write(0x40, h, ATT_TRANSACTION_MODE_ACTIVE, 2, &[3]), WriteAction::Done(0));
        assert_eq!(s.write(0x40, 0, ATT_TRANSACTION_MODE_VALIDATE, 0, &[]), WriteAction::Done(0));
        assert_eq!(
            s.write(0x40, 0, ATT_TRANSACTION_MODE_EXECUTE, 0, &[]),
            WriteAction::Modules(vec![(0, h, vec![1, 2, 3])])
        );
        assert_eq!(s.write(0x40, h, 0, 0, &[9]), WriteAction::Modules(vec![(0, h, vec![9])]));
        assert_eq!(s.write(0x40, h, 0, 1, &[9]), WriteAction::Done(ATT_ERROR_INVALID_OFFSET));
        assert_eq!(s.write(0x40, 3, 0, 0, &[9]), WriteAction::Done(btstack_sys::consts::ATT_ERROR_WRITE_NOT_PERMITTED));
        s.on_disconnected(0x40);
        assert_eq!(s.client_config(0x40, h), 0);
        assert!(s.clients().is_empty());
    }
}
