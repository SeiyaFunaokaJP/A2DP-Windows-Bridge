//! The LE core on the BTstack thread (v1 le_audio::Core, le_audio_core.cpp):
//! stack init / deinit of the LE part, LE scan (extended scan fallback),
//! connect with RPA / identity merge, SM LE Secure Connections Just Works
//! bonding (a bond the device lost is deleted and pairing repeated once),
//! bonds in <bond_dir>\le_audio_bonds.tlv, GATT discovery (included /
//! secondary services too), PACS / ASCS reads, refusing a stream without
//! ISO buffers ("adapter does not support LE Audio (no ISO buffers)"), the
//! BAP stream, and the module dispatch.
//!
//! [`LeCtx`] is what module hooks and GATT / timer callbacks get: the core's
//! BTstack-thread state for one call. Every entry point (an HCI / SM / GATT
//! / ATT event, a timer, a job of the worker API) runs through
//! [`LeCore::run`], which afterwards starts the queued GATT operations, runs
//! the callbacks of finished ones, hands notifications to att_server and
//! arms the LE timer.

use std::any::Any;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, mpsc};

use btstack_sys::consts::*;
use btstack_sys::{AttWriteRequest, CigParams, ConnectionType, Remote, Stack, events};
use le_audio::defs::{
    AD_APPEARANCE, AD_NAME_COMPLETE, AD_NAME_SHORT, AD_RSI, AD_SERVICE_DATA_UUID16, AD_UUID16_COMPLETE,
    AD_UUID16_INCOMPLETE, UUID_SERVICE_ASCS, UUID_SERVICE_CAS, UUID_SERVICE_GMAS, UUID_SERVICE_PACS, UUID_SERVICE_TMAS,
};
use le_audio::pacs_ascs::{Ase, PacRecord};
use le_audio::plan::{IsoLimits, PlanDevice, StreamPlan, plan_stream};
use le_audio::presets::PresetTable;
use le_audio::sdu_queue::SduQueue;

use super::bap::{self, Unicast, UnicastEnv, UnicastTimer};
use super::gatt_client::{GattChar, GattClient, GattService};
use super::gatt_server::{AttRead, AttWrite, GattServer, WriteAction};
use super::iso_send::IsoSender;
use super::module::{LeGroupHooks, LeModule, LeModuleSetup};
use super::modules::tmap_gmap::TmapGmapModule;
use super::{LeAddr, LeConfig, LeDeviceId, LeDeviceInfo, LeScanResult, LeStreamRequest, LeStreamStats, LeStreamStatus};
use crate::Controls;
use crate::app::{HostApp, Job};

/// hci_con_handle_t of no connection
pub const CON_HANDLE_INVALID: u16 = 0xFFFF;

/// Addresses resolved / looked up are used for connecting at most this long
/// after they were seen (RPAs change every ~15 min); at most this many kept
const RPA_FRESH_MS: u32 = 5 * 60 * 1000;
const RPA_CACHE_MAX: usize = 512;

/// One LE Audio peer (e.g. a headset, or one earbud of a pair), owned by
/// the core, only touched on the BTstack thread (v1 LeAudioDevice). Profile
/// modules read it and keep their own state keyed by `id`.
#[derive(Debug, Clone)]
pub struct LeDevice {
    pub id: LeDeviceId,
    /// The identity address (public or static random) once SM learned it
    /// (pairing, or an RPA resolved with the stored IRK); before that the
    /// address it was first connected with. Devices are keyed by it.
    pub addr: LeAddr,
    /// `addr` is the identity from SM
    pub identity_known: bool,
    /// Address of the current / last link (maybe an RPA)
    pub peer_addr: LeAddr,
    pub name: String,

    // ---- link ----
    /// ACL handle, CON_HANDLE_INVALID when down
    pub acl: u16,
    pub encrypted: bool,
    /// SM pairing in progress
    pub pairing: bool,
    pub pair_attempts: u32,
    /// Security done, discovery started
    pub secured: bool,
    /// GATT cache + PACS + ASCS read
    pub gatt_ready: bool,
    /// `le_connect` may return
    pub ready: bool,
    /// Why it went down / failed (HCI / ATT / SM)
    pub last_error: u8,

    // ---- GATT cache (filled once per connection) ----
    pub services: Vec<GattService>,

    // ---- PACS (sink direction: we are the audio source) ----
    pub sink_pacs: Vec<PacRecord>,
    pub sink_locations: u32,
    pub available_sink_contexts: u16,
    pub available_source_contexts: u16,
    pub supported_sink_contexts: u16,
    pub supported_source_contexts: u16,
    pub has_pacs: bool,
    pub has_ascs: bool,

    // ---- ASCS ----
    pub ases: Vec<Ase>,
    pub ase_cp_handle: u16,
}

impl LeDevice {
    pub(crate) fn new(id: LeDeviceId, addr: LeAddr) -> LeDevice {
        LeDevice {
            id,
            addr,
            identity_known: false,
            peer_addr: addr,
            name: String::new(),
            acl: CON_HANDLE_INVALID,
            encrypted: false,
            pairing: false,
            pair_attempts: 0,
            secured: false,
            gatt_ready: false,
            ready: false,
            last_error: 0,
            services: Vec::new(),
            sink_pacs: Vec::new(),
            sink_locations: 0,
            available_sink_contexts: 0,
            available_source_contexts: 0,
            supported_sink_contexts: 0,
            supported_source_contexts: 0,
            has_pacs: false,
            has_ascs: false,
            ases: Vec::new(),
            ase_cp_handle: 0,
        }
    }

    pub fn connected(&self) -> bool {
        self.acl != CON_HANDLE_INVALID
    }

    /// The first service with this UUID (primary or secondary)
    pub fn find_service(&self, uuid16: u16) -> Option<&GattService> {
        self.services.iter().find(|s| s.uuid16 == uuid16)
    }

    /// All characteristics with this UUID in all services with
    /// `service_uuid16` (0 = any service)
    pub fn find_chars(&self, service_uuid16: u16, char_uuid16: u16) -> Vec<&GattChar> {
        self.services
            .iter()
            .filter(|s| service_uuid16 == 0 || s.uuid16 == service_uuid16)
            .flat_map(|s| s.chars.iter().filter(|c| c.uuid16 == char_uuid16))
            .collect()
    }

    pub fn find_char(&self, service_uuid16: u16, char_uuid16: u16) -> Option<&GattChar> {
        self.find_chars(service_uuid16, char_uuid16).into_iter().next()
    }

    pub fn ase(&self, ase_id: u8) -> Option<&Ase> {
        self.ases.iter().find(|a| a.id == ase_id)
    }

    #[cfg(test)]
    pub fn ase_mut(&mut self, ase_id: u8) -> Option<&mut Ase> {
        self.ases.iter_mut().find(|a| a.id == ase_id)
    }

    pub fn sink_ase_count(&self) -> usize {
        self.ases.iter().filter(|a| a.sink).count()
    }
}

/// A timer of [`LeCtx::run_after`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LeTimerId(pub u32);

/// A timer / posted callback
pub type LeCallback = Box<dyn FnOnce(&mut LeCtx<'_>)>;

/// Completion of a `BtHost::le_call` operation: true = success
pub type LeDone = Box<dyn FnOnce(bool) + Send>;

/// What the worker API reads without a round trip to the BTstack thread
#[derive(Default)]
pub(crate) struct LeShared {
    /// stack_init ran (v1 initialized_)
    pub initialized: AtomicBool,
    /// HCI working (v1 hci_working_)
    pub hci_working: AtomicBool,
    /// LE Read Buffer Size v2: ISO_Data_Packet_Length, Total_Num_ISO_Data_Packets
    pub iso_len: AtomicU32,
    pub iso_count: AtomicU32,
    /// "Low latency (GMAP)"
    pub low_latency: AtomicBool,
    /// Set once when a device or CIS of the running stream was lost
    pub stream_lost: AtomicBool,
    /// Counters of the running (or last) stream, published by the sender
    pub stats: Arc<Mutex<LeStreamStats>>,
    /// The modules' worker / any-thread parts (registry order)
    pub hooks: Mutex<Vec<Arc<dyn LeGroupHooks>>>,
}

impl LeShared {
    pub fn iso_buffers(&self) -> (u16, u8) {
        (self.iso_len.load(Ordering::Acquire) as u16, self.iso_count.load(Ordering::Acquire) as u8)
    }

    pub fn hooks(&self) -> Vec<Arc<dyn LeGroupHooks>> {
        self.hooks.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

/// RPA resolution (BTstack SM, IRKs of le_device_db)
#[derive(Debug, Clone, Copy, Default)]
struct RpaEntry {
    done: bool,
    ok: bool,
    identity: LeAddr,
    /// Last advertising report (run loop ms), None: never seen
    seen_ms: Option<u32>,
}

/// The result of a connection attempt for the worker (the id moves when
/// on_identity merges the link into a known device)
#[derive(Debug, Default)]
pub(crate) struct ConnectResult {
    pub id: Option<u32>,
    pub error: String,
}

/// The gap_connect in progress
struct ConnectAttempt {
    device: LeDeviceId,
    /// The address gap_connect was given
    target: LeAddr,
    result: Arc<Mutex<ConnectResult>>,
    done: LeDone,
    error: String,
}

struct Timer {
    id: u32,
    due: u32,
    cb: LeCallback,
}

/// The LE core's BTstack-thread state (devices, scan results, RPA cache,
/// GATT client and server, presets, unicast, ISO sender, modules, timers).
/// Lives in HostApp while LE Audio is on.
pub struct LeCore {
    pub(crate) shared: Arc<LeShared>,
    controls: Arc<Controls>,
    poster: LePoster,
    events: std::sync::mpsc::Sender<crate::HostEvent>,
    battery: bool,
    /// Taken at stack init (the modules get it)
    config: Option<LeConfig>,
    initialized: bool,
    /// The modules in registry order (None while one of its hooks runs)
    modules: Vec<Option<Box<dyn LeModule>>>,
    devices: Vec<LeDevice>,
    scan_results: Vec<LeScanResult>,
    scanning: bool,
    connect: Option<ConnectAttempt>,
    /// By RPA
    rpa_cache: HashMap<[u8; 6], RpaEntry>,
    pub(crate) gatt: GattClient,
    server: GattServer,
    presets: PresetTable,
    /// None while one of its calls runs, and after stack deinit
    unicast: Option<Unicast>,
    unicast_timers: [Option<LeTimerId>; UnicastTimer::ALL],
    iso: IsoSender,
    queue: Option<SharedSduQueue>,
    status: Option<LeStreamStatus>,
    timers: Vec<Timer>,
    next_timer: u32,
    timers_changed: bool,
    gatt_retry: Option<LeTimerId>,
    /// Sink PAC records per characteristic (con, value handle): a device may
    /// have several
    pub(crate) sink_pacs_by_char: BTreeMap<(u16, u16), Vec<PacRecord>>,
}

/// The LE core for one hook or callback (BTstack thread)
pub struct LeCtx<'a> {
    pub(crate) stack: &'a Stack,
    pub(crate) core: &'a mut LeCore,
}

impl<'a> LeCtx<'a> {
    pub fn stack(&self) -> &Stack {
        self.stack
    }

    pub fn device(&self, id: LeDeviceId) -> Option<&LeDevice> {
        self.core.devices.get(id.0 as usize)
    }

    pub fn device_mut(&mut self, id: LeDeviceId) -> Option<&mut LeDevice> {
        self.core.devices.get_mut(id.0 as usize)
    }

    /// The device with this ACL handle (connected)
    pub fn device_by_handle(&self, acl: u16) -> Option<&LeDevice> {
        if acl == CON_HANDLE_INVALID {
            return None;
        }
        self.core.devices.iter().find(|d| d.acl == acl)
    }

    /// All devices the core knows (connected or not)
    pub fn device_ids(&self) -> Vec<LeDeviceId> {
        self.core.devices.iter().map(|d| d.id).collect()
    }

    /// The GATT client queue (never call BTstack's gatt_client directly)
    pub fn gatt(&mut self) -> &mut GattClient {
        &mut self.core.gatt
    }

    /// The PC's GATT server at run time (notify / indicate / CCC state)
    pub fn gatt_server(&mut self) -> &mut GattServer {
        &mut self.core.server
    }

    /// The stream presets of this session (v1 add_stream_preset /
    /// find_stream_preset)
    pub fn presets(&self) -> &PresetTable {
        &self.core.presets
    }

    /// ISO buffers of the controller (LE Read Buffer Size v2):
    /// (ISO_Data_Packet_Length, Total_Num_ISO_Data_Packets), (0, 0) without
    /// ISO support
    pub fn iso_buffers(&self) -> (u16, u8) {
        self.core.shared.iso_buffers()
    }

    /// The running stream, None when idle
    pub fn stream_status(&self) -> Option<LeStreamStatus> {
        self.core.status.clone()
    }

    /// The "media keys" setting (MCP refuses commands while it is off, as
    /// AVRCP buttons are ignored then)
    pub fn media_keys(&self) -> bool {
        self.core.controls.media_keys.load(Ordering::Acquire)
    }

    /// Another module (or this one, from a GATT / timer callback); None
    /// while its own hook runs or when it is not registered
    pub fn module_mut<T: LeModule>(&mut self) -> Option<&mut T> {
        self.core.modules.iter_mut().flatten().find_map(|m| {
            let any: &mut dyn Any = m.as_mut();
            any.downcast_mut::<T>()
        })
    }

    /// Runs `f` on the BTstack thread after `ms` (v1 btstack timers of the
    /// modules); dropped when the session ends
    pub fn run_after(&mut self, ms: u32, f: LeCallback) -> LeTimerId {
        let id = self.core.next_timer;
        self.core.next_timer = self.core.next_timer.wrapping_add(1);
        let due = self.stack.time_ms().wrapping_add(ms);
        self.core.timers.push(Timer { id, due, cb: f });
        self.core.timers_changed = true;
        LeTimerId(id)
    }

    pub fn cancel_timer(&mut self, timer: LeTimerId) {
        let before = self.core.timers.len();
        self.core.timers.retain(|t| t.id != timer.0);
        if self.core.timers.len() != before {
            self.core.timers_changed = true;
        }
    }

    // ======================================================================
    // Crate-internal helpers
    // ======================================================================

    /// The device if it is still on the link `con` (for callbacks)
    pub(crate) fn device_on(&self, id: LeDeviceId, con: u16) -> Option<&LeDevice> {
        self.device(id).filter(|d| d.acl == con && con != CON_HANDLE_INVALID)
    }

    pub(crate) fn device_on_mut(&mut self, id: LeDeviceId, con: u16) -> Option<&mut LeDevice> {
        self.device_mut(id).filter(|d| d.acl == con && con != CON_HANDLE_INVALID)
    }

    /// Calls `f` on every module in registry order (each taken out of the
    /// registry while its hook runs)
    pub(crate) fn each_module(&mut self, mut f: impl FnMut(&mut dyn LeModule, &mut LeCtx<'_>)) {
        for i in 0..self.core.modules.len() {
            let Some(mut m) = self.core.modules[i].take() else { continue };
            f(m.as_mut(), self);
            self.core.modules[i] = Some(m);
        }
    }

    fn with_module<R>(&mut self, i: usize, f: impl FnOnce(&mut dyn LeModule, &mut LeCtx<'_>) -> R) -> Option<R> {
        let mut m = self.core.modules.get_mut(i)?.take()?;
        let r = f(m.as_mut(), self);
        self.core.modules[i] = Some(m);
        Some(r)
    }

    /// Calls `f` with the unicast state machine (taken out of the core while
    /// it runs; nothing happens when it is busy already or gone)
    pub(crate) fn with_unicast(&mut self, f: impl FnOnce(&mut Unicast, &mut LeCtx<'_>)) {
        let Some(mut u) = self.core.unicast.take() else { return };
        f(&mut u, self);
        if self.core.unicast.is_none() && self.core.initialized {
            self.core.unicast = Some(u);
        }
    }

    /// After an entry point: start queued GATT operations, run finished
    /// callbacks (which may queue more), send notifications, arm the timer
    fn settle(&mut self) {
        loop {
            self.core.gatt.pump_all(self.stack);
            let ready = self.core.gatt.take_ready();
            if self.core.server.has_outbox() {
                for (con, handle, value, indication) in self.core.server.take_outbox() {
                    self.stack.att_server_send(con, handle, &value, indication);
                }
            }
            if ready.is_empty() {
                break;
            }
            for c in ready {
                c.run(self);
            }
        }
        if self.core.gatt.wants_retry() && self.core.gatt_retry.is_none() {
            // gatt_client was busy (e.g. with the MTU exchange): try shortly
            let id = self.run_after(5, Box::new(|ctx| ctx.core.gatt_retry = None));
            self.core.gatt_retry = Some(id);
        }
        if self.core.timers_changed {
            self.core.timers_changed = false;
            match self.core.timers.iter().map(|t| t.due.wrapping_sub(self.stack.time_ms()) as i32).min() {
                Some(wait) => {
                    self.stack.timer_start(TIMER_LE, wait.max(0) as u32);
                }
                None => self.stack.timer_stop(TIMER_LE),
            }
        }
    }

    // ======================================================================
    // Addresses
    // ======================================================================

    fn lookup_rpa(&mut self, addr: LeAddr, seen: bool) {
        if !addr.is_rpa() {
            return;
        }
        if !self.core.rpa_cache.contains_key(&addr.addr) {
            if self.core.rpa_cache.len() >= RPA_CACHE_MAX {
                // Forget the addresses that resolved to nothing
                self.core.rpa_cache.retain(|_, e| !(e.done && !e.ok));
                if self.core.rpa_cache.len() >= RPA_CACHE_MAX {
                    return;
                }
            }
            let mut e = RpaEntry::default();
            // SM checks it against the IRKs of all bonds (software AES); the
            // answer is SM_EVENT_IDENTITY_RESOLVING_SUCCEEDED / _FAILED
            let status = self.stack.sm_address_resolution_lookup(BD_ADDR_TYPE_LE_RANDOM, &addr.addr);
            tracing::trace!("LE Audio: RPA {addr} resolution lookup ({status})");
            if status != 0 && status != BTSTACK_BUSY as i32 {
                e.done = true;
            }
            self.core.rpa_cache.insert(addr.addr, e);
        }
        if seen {
            let now = self.stack.time_ms();
            if let Some(e) = self.core.rpa_cache.get_mut(&addr.addr) {
                e.seen_ms = Some(now);
            }
        }
    }

    fn resolved_identity(&self, addr: &[u8; 6]) -> Option<LeAddr> {
        self.core.rpa_cache.get(addr).filter(|e| e.ok).map(|e| e.identity)
    }

    /// The RPA the device advertised most recently (scan / find just
    /// before): a saved RPA or the identity is not what it answers to now
    fn connect_address(&self, dev: LeDeviceId, given: LeAddr) -> LeAddr {
        let Some(d) = self.device(dev) else { return given };
        let now = self.stack.time_ms();
        let best = self
            .core
            .rpa_cache
            .iter()
            .filter_map(|(rpa, e)| {
                let seen = e.seen_ms?;
                (e.ok && now.wrapping_sub(seen) <= RPA_FRESH_MS && e.identity.addr == d.addr.addr)
                    .then_some((now.wrapping_sub(seen), *rpa))
            })
            .min_by_key(|(age, _)| *age);
        tracing::debug!(
            "LE Audio: connect address for device {dev:?}: given {given}, freshest resolved RPA {:?}",
            best.map(|(age, rpa)| (crate::addr_str(&rpa), age))
        );
        match best {
            Some((_, rpa)) => LeAddr { addr: rpa, addr_type: LeAddr::RANDOM },
            None => given,
        }
    }

    /// The device of an address (an RPA SM resolved stands for its
    /// identity), created if asked
    fn device_for_addr(&mut self, addr: LeAddr, create: bool) -> Option<LeDeviceId> {
        let resolved = self.resolved_identity(&addr.addr);
        let key = resolved.unwrap_or(addr);
        if let Some(d) = self.core.devices.iter_mut().find(|d| d.addr.addr == key.addr || d.peer_addr.addr == addr.addr)
        {
            if !d.identity_known && resolved.is_none() {
                d.addr.addr_type = addr.addr_type;
            }
            return Some(d.id);
        }
        if !create {
            return None;
        }
        let id = LeDeviceId(self.core.devices.len() as u32);
        tracing::debug!("LE Audio: new device {} for {addr} (identity {:?})", id.0, resolved.map(|r| r.to_string()));
        let mut d = LeDevice::new(id, key);
        d.identity_known = resolved.is_some();
        d.peer_addr = addr;
        self.core.devices.push(d);
        Some(id)
    }

    /// SM told the identity of the device on a link
    fn on_identity(&mut self, dev: LeDeviceId, identity: LeAddr) {
        let Some(d) = self.device(dev) else { return };
        if d.identity_known && d.addr == identity {
            return;
        }
        let other = self.core.devices.iter().find(|o| o.id != dev && o.addr.addr == identity.addr).map(|o| o.id);
        let other_connected = other.and_then(|o| self.device(o)).is_some_and(|o| o.connected());
        let Some(other) = other.filter(|_| !other_connected && !d.secured) else {
            let Some(d) = self.device_mut(dev) else { return };
            if d.addr != identity {
                tracing::info!(
                    "LE Audio: {} has the identity address {identity} (type {})",
                    d.addr,
                    identity.addr_type
                );
            }
            d.addr = identity;
            d.identity_known = true;
            return;
        };
        // A device known under its identity came back with an address that
        // was not resolved yet: the link belongs to the known device (its id,
        // which CSIP and the worker hold, survives). Nothing but on_connected
        // has run for the link yet (security is not done).
        let from = d.clone();
        tracing::info!("LE Audio: {} is {identity} (device {}): link moved to it", from.peer_addr, other);
        if let Some(o) = self.device_mut(other) {
            o.addr = identity;
            o.identity_known = true;
            o.peer_addr = from.peer_addr;
            o.acl = from.acl;
            o.encrypted = from.encrypted;
            o.pairing = from.pairing;
            o.pair_attempts = from.pair_attempts;
            o.secured = false;
            o.gatt_ready = false;
            o.ready = false;
            o.last_error = 0;
            o.services.clear();
            if o.name.is_empty() {
                o.name = from.name.clone();
            }
        }
        if let Some(d) = self.device_mut(dev) {
            d.acl = CON_HANDLE_INVALID;
            d.encrypted = false;
            d.pairing = false;
            d.secured = false;
            d.gatt_ready = false;
            d.ready = false;
        }
        if let Some(c) = self.core.connect.as_mut()
            && c.device == dev
        {
            c.device = other;
            c.result.lock().unwrap_or_else(|e| e.into_inner()).id = Some(other.0);
        }
        self.each_module(|m, ctx| m.on_connected(ctx, other));
    }

    // ======================================================================
    // HCI events
    // ======================================================================

    pub(crate) fn on_hci(&mut self, p: &[u8]) {
        let Some(&ev) = p.first() else { return };
        match ev {
            BTSTACK_EVENT_STATE => {
                if p.get(2) == Some(&2) {
                    self.core.shared.hci_working.store(true, Ordering::Release);
                    let (len, count) = self.iso_buffers();
                    tracing::info!("LE Audio: ready (controller ISO buffers: {count} x {len} bytes)");
                    if count == 0 || len == 0 {
                        tracing::warn!(
                            "LE Audio: the controller reports no ISO buffers: it does not support LE Audio streams \
                             (CIS); streaming will be refused"
                        );
                    }
                } else {
                    self.core.shared.hci_working.store(false, Ordering::Release);
                }
            }
            HCI_EVENT_COMMAND_COMPLETE => {
                if let Some(b) = events::le_read_buffer_size_v2(p) {
                    tracing::debug!(
                        "LE Audio: LE Read Buffer Size v2 status 0x{:02X}: ISO {} x {} bytes",
                        b.status,
                        b.iso_packets,
                        b.iso_packet_length
                    );
                    if b.status == 0 {
                        self.core.shared.iso_len.store(b.iso_packet_length as u32, Ordering::Release);
                        self.core.shared.iso_count.store(b.iso_packets as u32, Ordering::Release);
                    }
                } else if events::command_complete_opcode(p) == Some(HCI_OPCODE_LE_REMOVE_CIG) {
                    tracing::debug!("LE Audio: LE Remove CIG complete: {:02X?}", p.get(5..));
                    self.with_unicast(|u, env| u.on_cig_removed(env));
                }
            }
            GAP_EVENT_ADVERTISING_REPORT | GAP_EVENT_EXTENDED_ADVERTISING_REPORT => {
                if let Some(r) = events::adv_report(p) {
                    self.on_adv_report(&r);
                }
            }
            HCI_EVENT_META_GAP => {
                if let Some(c) = events::le_connection_complete(p) {
                    let addr = LeAddr { addr: c.peer_addr, addr_type: c.peer_addr_type };
                    self.on_le_connected(c.status, c.handle, addr, c.role);
                } else if events::cig_created(p).is_some() {
                    self.with_unicast(|u, env| u.on_cig_created(env, p));
                } else if events::cis_created(p).is_some() {
                    self.with_unicast(|u, env| u.on_cis_created(env, p));
                } else if let Some(b) = events::bonding_deleted(p) {
                    tracing::debug!("LE Audio: bonding deleted (le_device_db index {})", b.index);
                    self.core.server.on_bonding_deleted(self.stack, b.index as usize);
                }
            }
            HCI_EVENT_ENCRYPTION_CHANGE | HCI_EVENT_ENCRYPTION_CHANGE_V2 => {
                if let Some(e) = events::encryption_change(p) {
                    self.on_encryption(e.handle, e.status, e.enabled);
                    if e.status == 0
                        && e.enabled
                        && self.stack.connection_type(e.handle) == ConnectionType::Le
                        && let Some(index) = self.stack.sm_le_device_index(e.handle)
                    {
                        self.core.server.on_encrypted(self.stack, e.handle, index);
                    }
                }
            }
            HCI_EVENT_DISCONNECTION_COMPLETE => {
                if let Some((_, handle, reason)) = events::disconnection_complete(p) {
                    tracing::debug!(
                        "LE Audio: disconnection complete 0x{handle:04X} (reason 0x{reason:02X}), LE device {:?}",
                        self.device_by_handle(handle).map(|d| d.id)
                    );
                    self.on_disconnected(handle, reason);
                }
            }
            _ => {}
        }
    }

    // ======================================================================
    // Scanning
    // ======================================================================

    pub(crate) fn start_scan(&mut self) {
        tracing::debug!("LE Audio: scan start (active, interval / window 0x0030)");
        self.core.scan_results.clear();
        self.core.scanning = true;
        self.stack.le_set_scan_params(true, 0x0030, 0x0030, 0);
        self.stack.le_start_scan();
    }

    pub(crate) fn stop_scan(&mut self) -> Vec<LeScanResult> {
        self.stack.le_stop_scan();
        tracing::debug!(
            "LE Audio: scan stop, {} advertiser(s), {} LE Audio",
            self.core.scan_results.len(),
            self.core.scan_results.iter().filter(|r| r.le_audio).count()
        );
        self.core.scanning = false;
        self.core.scan_results.clone()
    }

    fn on_adv_report(&mut self, r: &events::AdvReport<'_>) {
        if !self.core.scanning {
            return;
        }
        let addr = LeAddr { addr: r.addr, addr_type: if r.extended { r.addr_type & 0x01 } else { r.addr_type } };
        let (idx, fresh) = match self.core.scan_results.iter().position(|e| e.addr == addr) {
            Some(i) => (i, false),
            None => {
                self.core.scan_results.push(LeScanResult { addr, connectable: true, ..LeScanResult::default() });
                (self.core.scan_results.len() - 1, true)
            }
        };
        self.lookup_rpa(addr, true);
        let identity = self.resolved_identity(&addr.addr);
        let mut res = std::mem::take(&mut self.core.scan_results[idx]);
        if res.identity.is_none() {
            res.identity = identity;
        }
        let (scan_rsp, connectable) = if r.extended {
            (r.event_type & 0x08 != 0, r.event_type & 0x01 != 0)
        } else {
            (r.event_type == 4, r.event_type == 0 || r.event_type == 1)
        };
        if scan_rsp {
            res.scan_rsp = r.data.to_vec();
        } else {
            res.adv = r.data.to_vec();
            res.connectable = connectable;
        }
        res.rssi = r.rssi;
        let was_le_audio = res.le_audio;
        let (adv, rsp) = (res.adv.clone(), res.scan_rsp.clone());
        parse_ad(&mut res, &adv);
        parse_ad(&mut res, &rsp);
        self.each_module(|m, ctx| m.on_scan_result(ctx, &mut res, r.data));
        if (fresh || !was_le_audio) && res.le_audio {
            tracing::info!(
                "LE Audio: found {} ({}, type {}) RSSI {}{}{}{}",
                if res.name.is_empty() { "?" } else { res.name.as_str() },
                res.addr,
                res.addr.addr_type,
                res.rssi,
                res.identity.map(|i| format!(", identity {i}")).unwrap_or_default(),
                if res.rsi.is_some() { ", set member (RSI)" } else { "" },
                if res.note.is_empty() { String::new() } else { format!(", {}", res.note) }
            );
        }
        if let Some(slot) = self.core.scan_results.get_mut(idx) {
            *slot = res;
        }
    }

    /// le_find: a saved RPA of a bonded device is looked for by its identity
    pub(crate) fn start_find(&mut self, want: LeAddr) {
        self.lookup_rpa(want, false);
        self.start_scan();
    }

    /// le_find: an advertiser matching `want` (the address itself, one
    /// whose RPA resolves to it, or to the identity `want` resolves to)
    pub(crate) fn find_match(&mut self, want: LeAddr) -> Option<LeScanResult> {
        let want_identity = self.resolved_identity(&want.addr);
        let mut found = None;
        for i in 0..self.core.scan_results.len() {
            if self.core.scan_results[i].identity.is_none() {
                let a = self.core.scan_results[i].addr.addr;
                self.core.scan_results[i].identity = self.resolved_identity(&a);
            }
            let r = &self.core.scan_results[i];
            let matches = r.addr.addr == want.addr
                || r.identity
                    .is_some_and(|id| id.addr == want.addr || want_identity.is_some_and(|w| w.addr == id.addr));
            if matches && (!r.name.is_empty() || !r.scan_rsp.is_empty() || r.le_audio) {
                if r.addr.addr != want.addr {
                    tracing::info!(
                        "LE Audio: {want} advertises as {} now (identity {})",
                        r.addr,
                        r.identity.map(|i| i.to_string()).unwrap_or_default()
                    );
                }
                found = Some(r.clone());
            }
        }
        found
    }

    // ======================================================================
    // Connection
    // ======================================================================

    /// le_connect's work: `done(true)` once the device is ready
    pub(crate) fn start_connect(
        &mut self,
        addr: LeAddr,
        name: String,
        result: Arc<Mutex<ConnectResult>>,
        done: LeDone,
    ) {
        tracing::debug!("LE Audio: connect request {addr} (type {}) name {name:?}", addr.addr_type);
        if self.core.connect.is_some() {
            tracing::debug!("LE Audio: connect refused, another attempt is in progress");
            result.lock().unwrap_or_else(|e| e.into_inner()).error = "another LE connection is being set up".into();
            done(false);
            return;
        }
        let Some(id) = self.device_for_addr(addr, true) else {
            done(false);
            return;
        };
        if !name.is_empty()
            && let Some(d) = self.device_mut(id)
        {
            d.name = name;
        }
        result.lock().unwrap_or_else(|e| e.into_inner()).id = Some(id.0);
        let target = self.connect_address(id, addr);
        let Some(d) = self.device_mut(id) else { return };
        if d.connected() && d.ready {
            tracing::debug!("LE Audio: device {} already connected and ready (0x{:04X})", id.0, d.acl);
            done(true);
            return;
        }
        let in_progress = d.connected();
        tracing::debug!(
            "LE Audio: device {} connect: target {target}, link {} (acl 0x{:04X}, encrypted {}, secured {})",
            id.0,
            if in_progress { "in setup" } else { "down" },
            d.acl,
            d.encrypted,
            d.secured
        );
        if !in_progress {
            d.pair_attempts = 0;
            d.last_error = 0;
            if target != addr || d.addr != addr {
                tracing::info!(
                    "LE Audio: device {id} ({}): connecting with {target} (address type {})",
                    d.addr,
                    target.addr_type
                );
            }
        }
        self.core.connect =
            Some(ConnectAttempt { device: id, target, result: result.clone(), done, error: String::new() });
        if in_progress {
            // Set up in progress: wait for it
            return;
        }
        let status = self.stack.le_connect(&target.addr, target.addr_type);
        tracing::debug!("LE Audio: gap_connect {target} -> 0x{status:02X}");
        if status != ERROR_CODE_SUCCESS
            && let Some(c) = self.core.connect.take()
        {
            result.lock().unwrap_or_else(|e| e.into_inner()).error =
                format!("LE connect could not start (0x{status:02x})");
            (c.done)(false);
        }
    }

    /// The worker gave up on a connection attempt (timeout / cancel): cancel
    /// the connect or drop the link. A link that comes up after this is
    /// dropped by on_le_connected.
    pub(crate) fn abandon_connect(&mut self, id: Option<u32>) {
        tracing::debug!(
            "LE Audio: connect abandoned (device {id:?}, link {:?})",
            id.and_then(|i| self.device(LeDeviceId(i))).map(|d| d.acl)
        );
        let Some(id) = id.map(LeDeviceId) else {
            self.stack.le_connect_cancel();
            return;
        };
        if self.core.connect.as_ref().is_some_and(|c| c.device == id) {
            self.core.connect = None;
        }
        match self.device(id) {
            Some(d) if d.connected() => {
                self.stack.disconnect(d.acl);
            }
            _ => {
                self.stack.le_connect_cancel();
            }
        }
    }

    fn on_le_connected(&mut self, status: u8, handle: u16, addr: LeAddr, role: u8) {
        let attempt = self.core.connect.as_ref().map(|c| (c.device, c.target));
        tracing::debug!(
            "LE Audio: LE connection complete 0x{status:02X}: handle 0x{handle:04X}, {addr} (type {}), role {role}, attempt {:?}",
            addr.addr_type,
            attempt.map(|(d, t)| (d.0, t.to_string()))
        );
        let stray = match attempt {
            None => true,
            Some((dev, target)) => {
                status == ERROR_CODE_SUCCESS
                    && (self.device(dev).is_some_and(|d| d.connected()) || target.addr != addr.addr)
            }
        };
        if stray {
            // A2DPWB never advertises: this is the connect of an attempt
            // given up (timeout / cancel) that completed before the cancel
            // did. Nobody manages the link: drop it.
            if status == ERROR_CODE_SUCCESS {
                tracing::warn!(
                    "LE Audio: unexpected LE connection 0x{handle:04x} with {addr} (role {role}): disconnecting"
                );
                self.stack.disconnect(handle);
            }
            return;
        }
        let Some((dev, _)) = attempt else { return };
        if status != ERROR_CODE_SUCCESS {
            let what = if status == ERROR_CODE_UNKNOWN_CONNECTION_IDENTIFIER {
                "LE connection cancelled"
            } else {
                "LE connection failed"
            };
            self.fail_connect(dev, format!("{what} (0x{status:02x})"));
            return;
        }
        let Some(d) = self.device_mut(dev) else { return };
        d.acl = handle;
        d.peer_addr = addr;
        d.encrypted = false;
        d.pairing = false;
        d.secured = false;
        d.gatt_ready = false;
        d.ready = false;
        d.services.clear();
        tracing::info!("LE Audio: LE link to {} up (handle 0x{handle:04x}), securing it", d.addr);
        self.core.gatt.attach(handle);
        if !self.stack.gatt_listen(handle) {
            tracing::warn!("LE Audio: cannot listen for notifications on 0x{handle:04x} (all slots in use)");
        }
        self.each_module(|m, ctx| m.on_connected(ctx, dev));
        tracing::debug!("LE Audio: requesting security on 0x{handle:04X}");
        self.stack.sm_request_pairing(handle);
    }

    fn fail_connect(&mut self, dev: LeDeviceId, why: String) {
        if let Some(d) = self.device(dev) {
            tracing::warn!("LE Audio: {}: {why}", d.addr);
            if d.connected() {
                self.stack.disconnect(d.acl);
            }
        }
        if let Some(c) = self.core.connect.as_mut() {
            c.error = why;
        }
        self.finish_connect(dev, false);
    }

    fn finish_connect(&mut self, dev: LeDeviceId, ok: bool) {
        if self.core.connect.as_ref().is_none_or(|c| c.device != dev) {
            return;
        }
        let Some(c) = self.core.connect.take() else { return };
        tracing::debug!("LE Audio: connect of device {} finished: ok {ok} {:?}", dev.0, c.error);
        {
            let mut r = c.result.lock().unwrap_or_else(|e| e.into_inner());
            r.id = Some(dev.0);
            if !ok {
                r.error = c.error;
            }
        }
        (c.done)(ok);
    }

    fn on_encryption(&mut self, handle: u16, status: u8, enabled: bool) {
        let Some(id) = self.device_by_handle(handle).map(|d| d.id) else { return };
        if let Some(d) = self.device_mut(id) {
            d.encrypted = status == ERROR_CODE_SUCCESS && enabled;
            tracing::info!(
                "LE Audio: {} encryption {} (0x{status:02x})",
                d.addr,
                if d.encrypted { "on" } else { "off" }
            );
        }
        self.maybe_secured(id);
    }

    fn maybe_secured(&mut self, id: LeDeviceId) {
        let Some(d) = self.device_mut(id) else { return };
        if !d.encrypted || d.pairing || d.secured || !d.connected() {
            tracing::debug!(
                "LE Audio: device {} not secured yet (encrypted {}, pairing {}, secured {}, acl 0x{:04X})",
                id.0,
                d.encrypted,
                d.pairing,
                d.secured,
                d.acl
            );
            return;
        }
        d.secured = true;
        let con = d.acl;
        tracing::debug!("LE Audio: device {} secured, GATT discovery on 0x{con:04X}", id.0);
        self.each_module(|m, ctx| m.on_encrypted(ctx, id));
        self.core.gatt.discover_all(
            con,
            Box::new(move |ctx, status, services| {
                if ctx.device_on(id, con).is_none() {
                    return;
                }
                if status != 0 {
                    ctx.fail_connect(id, format!("GATT discovery failed (0x{status:02x})"));
                    return;
                }
                for s in &services {
                    tracing::debug!(
                        "LE Audio: service 0x{:04X} {} 0x{:04X}-0x{:04X}{}: {}",
                        s.uuid16,
                        if s.primary { "primary" } else { "secondary" },
                        s.start_handle,
                        s.end_handle,
                        if s.included_by != 0 { format!(" (included by 0x{:04X})", s.included_by) } else { String::new() },
                        s.chars
                            .iter()
                            .map(|c| format!("0x{:04X}@0x{:04X}", c.uuid16, c.value_handle))
                            .collect::<Vec<_>>()
                            .join(" ")
                    );
                }
                let Some(d) = ctx.device_on_mut(id, con) else { return };
                d.services = services;
                let chars: usize = d.services.iter().map(|s| s.chars.len()).sum();
                tracing::info!("LE Audio: {}: {} services, {chars} characteristics", d.addr, d.services.len());
                bap::read_device(
                    ctx,
                    id,
                    Box::new(move |ctx, _| {
                        let Some(d) = ctx.device_on_mut(id, con) else { return };
                        tracing::debug!(
                            "LE Audio: device {} GATT ready: pacs {}, ascs {}, {} sink PAC(s), {} ASE(s), locations 0x{:08X}",
                            id.0,
                            d.has_pacs,
                            d.has_ascs,
                            d.sink_pacs.len(),
                            d.ases.len(),
                            d.sink_locations
                        );
                        d.gatt_ready = true;
                        ctx.each_module(|m, ctx| m.on_gatt_ready(ctx, id));
                        let Some(d) = ctx.device_on_mut(id, con) else { return };
                        d.ready = true;
                        ctx.finish_connect(id, true);
                    }),
                );
            }),
        );
    }

    fn on_disconnected(&mut self, handle: u16, reason: u8) {
        if let Some(id) = self.device_by_handle(handle).map(|d| d.id) {
            self.core.gatt.detach(handle);
            self.core.server.on_disconnected(handle);
            self.core.sink_pacs_by_char.retain(|(c, _), _| *c != handle);
            if let Some(d) = self.device_mut(id) {
                tracing::info!("LE Audio: {} disconnected (0x{reason:02x})", d.addr);
                d.acl = CON_HANDLE_INVALID;
                d.encrypted = false;
                d.pairing = false;
                d.secured = false;
                d.gatt_ready = false;
                d.ready = false;
                d.last_error = reason;
                for a in d.ases.iter_mut() {
                    a.state = le_audio::defs::AseState::Unknown;
                }
            }
            self.each_module(|m, ctx| m.on_disconnected(ctx, id, reason));
            if let Some(c) = self.core.connect.as_mut()
                && c.device == id
            {
                if c.error.is_empty() {
                    c.error = format!("the link went down during setup (0x{reason:02x})");
                }
                self.finish_connect(id, false);
            }
        }
        self.with_unicast(|u, env| u.on_link_lost(env, handle));
    }

    /// le_disconnect's work: HCI Disconnect, then wait (polling) until the
    /// link is down
    pub(crate) fn disconnect_device(&mut self, id: LeDeviceId, done: LeDone) {
        let Some(d) = self.device(id).filter(|d| d.connected()) else {
            tracing::debug!("LE Audio: disconnect device {}: not connected", id.0);
            done(true);
            return;
        };
        let status = self.stack.disconnect(d.acl);
        tracing::debug!("LE Audio: disconnect device {} (0x{:04X}) -> 0x{status:02X}", id.0, d.acl);
        if status != ERROR_CODE_SUCCESS {
            done(false);
            return;
        }
        // Disconnection Complete arrives later
        poll_disconnected(self, id, 0, done);
    }

    // ======================================================================
    // SM events
    // ======================================================================

    pub(crate) fn on_sm(&mut self, p: &[u8]) {
        let Some(&ty) = p.first() else { return };
        let Some(handle) = events::sm_handle(p) else { return };
        tracing::debug!("LE Audio: SM event 0x{ty:02X} on 0x{handle:04X}");
        match ty {
            SM_EVENT_IDENTITY_RESOLVING_SUCCEEDED | SM_EVENT_IDENTITY_CREATED => {
                // An address resolved with a bond's IRK (an RPA seen while
                // scanning, or a link), or the identity a new pairing
                // distributed
                let Some(i) = events::sm_identity(p) else { return };
                let a = LeAddr { addr: i.addr, addr_type: i.addr_type };
                let identity = LeAddr { addr: i.identity_addr, addr_type: i.identity_addr_type & 0x01 };
                if ty == SM_EVENT_IDENTITY_CREATED {
                    // A new IRK: addresses that resolved to nothing may
                    // resolve now
                    self.core.rpa_cache.retain(|_, e| !(e.done && !e.ok));
                }
                if a.is_rpa() {
                    let e = self.core.rpa_cache.entry(a.addr).or_default();
                    if !e.ok {
                        tracing::info!("LE Audio: {a} resolves to {identity}");
                    }
                    e.done = true;
                    e.ok = true;
                    e.identity = identity;
                    for r in self.core.scan_results.iter_mut().filter(|r| r.addr.addr == a.addr) {
                        r.identity = Some(identity);
                    }
                }
                if handle != CON_HANDLE_INVALID
                    && let Some(id) = self.device_by_handle(handle).map(|d| d.id)
                {
                    self.on_identity(id, identity);
                }
                return;
            }
            SM_EVENT_IDENTITY_RESOLVING_FAILED => {
                if let Some((_, _, a)) = events::sm_identity_resolving_failed(p)
                    && let Some(e) = self.core.rpa_cache.get_mut(&a)
                    && !e.ok
                {
                    e.done = true;
                }
                return;
            }
            _ => {}
        }
        let Some(id) = self.device_by_handle(handle).map(|d| d.id) else { return };
        let addr = self.device(id).map(|d| d.addr).unwrap_or_default();
        match ty {
            SM_EVENT_JUST_WORKS_REQUEST => {
                tracing::info!("LE Audio: {addr}: pairing (Just Works)");
                self.stack.sm_just_works_confirm(handle);
            }
            SM_EVENT_NUMERIC_COMPARISON_REQUEST => {
                tracing::info!("LE Audio: {addr}: pairing (numeric comparison, accepted)");
                self.stack.sm_numeric_comparison_confirm(handle);
            }
            SM_EVENT_PASSKEY_INPUT_NUMBER => {
                // No keyboard: such a device cannot pair with A2DPWB
                tracing::warn!("LE Audio: {addr} asks for a passkey, which A2DPWB cannot enter");
                self.stack.sm_bonding_decline(handle);
            }
            SM_EVENT_PAIRING_STARTED => {
                if let Some(d) = self.device_mut(id) {
                    d.pairing = true;
                }
            }
            SM_EVENT_PAIRING_COMPLETE => {
                let Some(c) = events::sm_pairing_complete(p) else { return };
                let Some(d) = self.device_mut(id) else { return };
                tracing::debug!(
                    "LE Audio: {addr}: pairing complete status 0x{:02X} reason 0x{:02X} (attempts {})",
                    c.status,
                    c.reason,
                    d.pair_attempts
                );
                d.pairing = false;
                if c.status == ERROR_CODE_SUCCESS {
                    tracing::info!("LE Audio: {addr} paired and bonded");
                    self.maybe_secured(id);
                } else {
                    d.pair_attempts += 1;
                    if d.pair_attempts < 2 {
                        tracing::warn!(
                            "LE Audio: {addr}: pairing failed (0x{:02x}, reason 0x{:02x}), trying again",
                            c.status,
                            c.reason
                        );
                        self.stack.sm_request_pairing(handle);
                    } else {
                        self.fail_connect(id, format!("pairing failed (0x{:02x})", c.status));
                    }
                }
            }
            SM_EVENT_REENCRYPTION_COMPLETE => {
                let Some((_, _, _, status)) = events::sm_reencryption_complete(p) else { return };
                tracing::debug!("LE Audio: {addr}: re-encryption complete 0x{status:02X}");
                if status != ERROR_CODE_SUCCESS {
                    // The device lost its bond (reset or paired elsewhere):
                    // pair anew
                    tracing::warn!(
                        "LE Audio: {addr}: encryption with the stored key failed (0x{status:02x}), pairing again"
                    );
                    self.stack.le_delete_bonding(addr.addr_type, &addr.addr);
                    let Some(d) = self.device_mut(id) else { return };
                    d.pair_attempts += 1;
                    if d.pair_attempts < 2 && d.connected() {
                        self.stack.sm_request_pairing(handle);
                    } else {
                        self.fail_connect(id, format!("encryption failed (0x{status:02x})"));
                    }
                }
            }
            _ => {}
        }
    }

    // ======================================================================
    // Streaming
    // ======================================================================

    /// The plan for these devices (connected and ready)
    pub(crate) fn plan_for(&self, ids: &[LeDeviceId], request: &LeStreamRequest) -> Result<StreamPlan, String> {
        let mut devs = Vec::new();
        for &id in ids {
            let Some(d) = self.device(id).filter(|d| d.ready) else {
                return Err("a device of the group is not connected".into());
            };
            devs.push(PlanDevice {
                id: d.id.0,
                name: if d.name.is_empty() { d.addr.to_string() } else { d.name.clone() },
                acl: d.acl,
                sink_pacs: d.sink_pacs.clone(),
                sink_locations: d.sink_locations,
                available_sink_contexts: d.available_sink_contexts,
                ases: d.ases.clone(),
            });
        }
        let (packet_length, packet_count) = self.iso_buffers();
        let plan = plan_stream(&devs, request, self.presets(), IsoLimits { packet_length, packet_count });
        tracing::debug!(
            "LE Audio: plan for devices {:?} ({request:?}, ISO {packet_count} x {packet_length}): {plan:?}",
            ids.iter().map(|i| i.0).collect::<Vec<_>>()
        );
        plan
    }

    /// le_start_stream's BAP part: `done(true)` once audio may flow
    pub(crate) fn start_unicast(
        &mut self,
        plan: StreamPlan,
        queue: SharedSduQueue,
        status: LeStreamStatus,
        why: Arc<Mutex<String>>,
        done: LeDone,
    ) {
        let set_why = |why: &Arc<Mutex<String>>, s: &str| *why.lock().unwrap_or_else(|e| e.into_inner()) = s.into();
        let Some(u) = self.core.unicast.as_ref() else {
            // stack deinit ran
            set_why(&why, "LE Audio stopped");
            done(false);
            return;
        };
        if u.busy() {
            tracing::debug!("LE Audio: stream start refused, unicast busy");
            set_why(&why, "a stream is still being stopped");
            done(false);
            return;
        }
        tracing::debug!("LE Audio: unicast start: {}", plan.describe());
        self.core.queue = Some(queue);
        self.core.status = Some(status);
        self.with_unicast(|u, env| {
            u.start(
                env,
                plan,
                Box::new(move |r| match r {
                    Ok(()) => done(true),
                    Err(e) => {
                        set_why(&why, &e);
                        done(false);
                    }
                }),
            )
        });
    }

    /// le_stop_stream's work: stop sending, Disable / Release, remove the
    /// CIG
    pub(crate) fn stop_unicast(&mut self, done: LeDone) {
        tracing::debug!("LE Audio: unicast stop (state machine present {})", self.core.unicast.is_some());
        self.core.iso.stop(self.stack);
        if self.core.unicast.is_none() {
            done(true);
            return;
        }
        self.with_unicast(|u, env| u.stop(env, Box::new(move || done(true))));
    }

    /// For BtHost::le_devices (v1 Core::devices)
    pub(crate) fn device_infos(&mut self) -> Vec<LeDeviceInfo> {
        let mut list: Vec<LeDeviceInfo> = self
            .core
            .devices
            .iter()
            .map(|d| LeDeviceInfo {
                id: d.id,
                addr: d.addr,
                name: d.name.clone(),
                connected: d.connected(),
                ready: d.ready,
                last_error: d.last_error,
                sink_locations: d.sink_locations,
                sink_ases: d.sink_ase_count(),
                pacs: d
                    .sink_pacs
                    .iter()
                    .filter(|r| r.lc3)
                    .map(|r| {
                        format!(
                            "LC3 rates 0x{:04x} ch 0x{:02x} octets {}-{}",
                            r.caps.sampling_frequencies, r.caps.channel_counts, r.caps.min_octets, r.caps.max_octets
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; "),
                roles: String::new(),
            })
            .collect();
        if let Some(m) = self.module_mut::<TmapGmapModule>() {
            for d in &mut list {
                d.roles = m.info(d.id).map(|i| i.summary()).unwrap_or_default();
            }
        }
        list
    }

    // ======================================================================
    // GATT server (att_server callbacks)
    // ======================================================================

    fn att_read(&mut self, con: u16, handle: u16, offset: u16) -> Result<Vec<u8>, u8> {
        if let Some(v) = self.core.server.read_own(con, handle) {
            return Ok(v);
        }
        let Some(owner) = self.core.server.owner(handle) else { return Ok(Vec::new()) };
        let r = self
            .with_module(owner, |m, ctx| m.on_att_read(ctx, &AttRead { con, handle, offset }))
            .unwrap_or(Err(ATT_ERROR_UNLIKELY_ERROR));
        tracing::debug!(
            "LE Audio: ATT read 0x{handle:04X} offset {offset} on 0x{con:04X} (module {owner}): {:?}",
            r.as_ref().map(|v| v.len())
        );
        r
    }

    fn att_write(&mut self, w: &AttWriteRequest<'_>) -> u8 {
        tracing::debug!(
            "LE Audio: ATT write 0x{:04X} on 0x{:04X} (mode {}, offset {}): {:02X?}",
            w.handle,
            w.con,
            w.mode,
            w.offset,
            w.value
        );
        match self.core.server.write(w.con, w.handle, w.mode, w.offset, w.value) {
            WriteAction::Done(status) => status,
            WriteAction::Modules(list) => {
                let mut status = 0;
                for (owner, handle, value) in list {
                    let r = self
                        .with_module(owner, |m, ctx| {
                            m.on_att_write(
                                ctx,
                                &AttWrite { con: w.con, handle, value: &value, without_response: false },
                            )
                        })
                        .unwrap_or(Err(ATT_ERROR_UNLIKELY_ERROR));
                    if let Err(e) = r
                        && status == 0
                    {
                        status = e;
                    }
                }
                if status != 0 {
                    tracing::debug!("LE Audio: ATT write 0x{:04X} refused (0x{status:02X})", w.handle);
                }
                status
            }
        }
    }
}

/// Polls every 20 ms (at most 2 s) until the device's link is down
fn poll_disconnected(ctx: &mut LeCtx<'_>, id: LeDeviceId, ticks: u32, done: LeDone) {
    if ticks > 100 || ctx.device(id).is_none_or(|d| !d.connected()) {
        done(true);
        return;
    }
    ctx.run_after(20, Box::new(move |ctx| poll_disconnected(ctx, id, ticks + 1, done)));
}

/// Fills the parsed fields of `r` from AD data (v1 parse_ad)
fn parse_ad(r: &mut LeScanResult, data: &[u8]) {
    let add_uuid = |r: &mut LeScanResult, u: u16| {
        if !r.uuids16.contains(&u) {
            r.uuids16.push(u);
        }
    };
    let mut i = 0usize;
    while i < data.len() {
        let len = data[i] as usize;
        if len == 0 {
            i += 1;
            continue;
        }
        let Some(item) = data.get(i + 1..i + 1 + len) else { break };
        i += 1 + len;
        let (ty, v) = (item[0], &item[1..]);
        let rd16 = |p: &[u8]| u16::from_le_bytes([p[0], p[1]]);
        match ty {
            AD_NAME_SHORT | AD_NAME_COMPLETE => {
                if ty == AD_NAME_COMPLETE || r.name.is_empty() {
                    r.name = String::from_utf8_lossy(v).into_owned();
                }
            }
            AD_APPEARANCE if v.len() >= 2 => r.appearance = rd16(v),
            AD_UUID16_INCOMPLETE | AD_UUID16_COMPLETE => {
                for u in v.as_chunks::<2>().0 {
                    add_uuid(r, u16::from_le_bytes(*u));
                }
            }
            AD_SERVICE_DATA_UUID16 if v.len() >= 2 => {
                let u = rd16(v);
                add_uuid(r, u);
                if u == UUID_SERVICE_ASCS && v.len() >= 5 {
                    r.targeted = v[2] == 0x01;
                    r.ascs_sink_contexts = rd16(&v[3..]);
                } else if u == UUID_SERVICE_CAS && v.len() >= 3 {
                    r.targeted |= v[2] == 0x01;
                }
            }
            AD_RSI if v.len() >= 6 => {
                let mut rsi = [0u8; 6];
                rsi.copy_from_slice(&v[..6]);
                r.rsi = Some(rsi);
            }
            _ => {}
        }
    }
    if r.uuids16.iter().any(|u| {
        matches!(*u, UUID_SERVICE_ASCS | UUID_SERVICE_PACS | UUID_SERVICE_CAS | UUID_SERVICE_TMAS | UUID_SERVICE_GMAS)
    }) {
        r.le_audio = true;
    }
    // No LE Audio UUID, but an audio appearance (Audio Sink 0x0840-0x087F,
    // Wearable Audio Device 0x0940-0x097F, e.g. earbuds): list it as a
    // candidate; connecting tells (no ASCS: "no sink ASE")
    let category = r.appearance >> 6;
    if !r.le_audio && r.connectable && (category == 0x021 || category == 0x025) {
        r.le_audio = true;
        if r.note.is_empty() {
            r.note = "audio appearance, LE Audio not advertised".into();
        }
    }
}

// ============================================================================
// The unicast state machine's view of the core
// ============================================================================

impl UnicastEnv for LeCtx<'_> {
    fn device(&self, id: u32) -> Option<&LeDevice> {
        self.core.devices.get(id as usize)
    }

    fn cp_write(&mut self, device: u32, value: Vec<u8>, what: &'static str) {
        let Some(d) = self.core.devices.get(device as usize) else { return };
        if d.ase_cp_handle == 0 {
            tracing::error!("LE Audio: {} has no ASE control point", d.addr);
            return;
        }
        let (addr, acl, cp) = (d.addr, d.acl, d.ase_cp_handle);
        tracing::debug!("LE Audio: {addr}: ASE control point ({what}) 0x{cp:04X} <- {value:02X?}");
        self.core.gatt.write(
            acl,
            cp,
            value,
            Some(Box::new(move |_, status| {
                if status != 0 {
                    tracing::warn!("LE Audio: {addr}: ASE control point write ({what}) failed (ATT 0x{status:02x})");
                }
            })),
        );
    }

    fn ccids(&mut self, contexts: u16) -> Vec<u8> {
        let mut ccids = Vec::new();
        for m in self.core.modules.iter().flatten() {
            m.add_ccids(contexts, &mut ccids);
        }
        ccids
    }

    fn cig_create(&mut self, params: &CigParams) -> u8 {
        let status = self.stack.cig_create(params);
        tracing::debug!("LE Audio: LE Set CIG Parameters {params:?} -> 0x{status:02X}");
        status
    }

    fn cig_remove(&mut self, cig_id: u8) -> u8 {
        let status = self.stack.cig_remove(cig_id);
        tracing::debug!("LE Audio: LE Remove CIG {cig_id} -> 0x{status:02X}");
        status
    }

    fn cis_create(&mut self, cig_id: u8, cis_handles: &[u16], acl_handles: &[u16]) -> u8 {
        let status = self.stack.cis_create(cig_id, cis_handles, acl_handles);
        tracing::debug!(
            "LE Audio: LE Create CIS (CIG {cig_id}, CIS {cis_handles:04X?}, ACL {acl_handles:04X?}) -> 0x{status:02X}"
        );
        status
    }

    fn can_send_command(&self) -> bool {
        self.stack.can_send_command_now()
    }

    fn disconnect_cis(&mut self, handle: u16) -> u8 {
        let status = self.stack.cmd_disconnect(handle, ERROR_CODE_REMOTE_USER_TERMINATED_CONNECTION);
        tracing::debug!("LE Audio: disconnect CIS 0x{handle:04X} -> 0x{status:02X}");
        status
    }

    fn iso_packet_count(&self) -> u8 {
        self.core.shared.iso_buffers().1
    }

    fn start_sending(&mut self, cis_handles: &[u16], queue_depth: u8, frame_us: u32) {
        let queue = self.core.queue.get_or_insert_with(|| Arc::new(Mutex::new(SduQueue::new()))).clone();
        tracing::debug!(
            "LE Audio: start sending on CIS {cis_handles:04X?}, queue depth {queue_depth}, frame {frame_us} us"
        );
        self.core.iso.start(self.stack, cis_handles, queue, frame_us, queue_depth);
    }

    fn stop_sending(&mut self) {
        tracing::debug!("LE Audio: stop sending");
        self.core.iso.stop(self.stack);
    }

    fn cis_down(&mut self, handle: u16) {
        tracing::debug!("LE Audio: CIS 0x{handle:04X} down");
        self.core.iso.on_cis_down(self.stack, handle);
    }

    fn stream_lost(&mut self, why: &str) {
        if !self.core.shared.stream_lost.swap(true, Ordering::AcqRel) {
            tracing::warn!("LE Audio: stream lost: {why}");
        }
        self.core.iso.stop(self.stack);
    }

    fn stream_state(&mut self, plan: &StreamPlan, streaming: bool) {
        tracing::debug!("LE Audio: stream state: streaming {streaming} ({})", plan.describe());
        let status = match self.core.status.as_mut() {
            Some(s) => {
                s.active = streaming;
                s.plan = plan.clone();
                s.summary = plan.describe();
                s.clone()
            }
            None => {
                LeStreamStatus { active: streaming, plan: plan.clone(), devices: Vec::new(), summary: plan.describe() }
            }
        };
        self.each_module(|m, ctx| m.on_stream_state(ctx, &status, streaming));
        if !streaming {
            self.core.status = None;
        }
    }

    fn arm(&mut self, timer: UnicastTimer, ms: u32) {
        self.disarm(timer);
        tracing::trace!("LE Audio: unicast timer {timer:?} in {ms} ms");
        let slot = timer as usize;
        let id = self.run_after(
            ms,
            Box::new(move |ctx| {
                ctx.core.unicast_timers[slot] = None;
                ctx.with_unicast(|u, env| match timer {
                    UnicastTimer::Step => u.on_timeout(env),
                    UnicastTimer::Drop => u.drop_cis(env),
                    UnicastTimer::Refresh => u.on_refresh(env),
                });
            }),
        );
        self.core.unicast_timers[slot] = Some(id);
    }

    fn disarm(&mut self, timer: UnicastTimer) {
        if let Some(id) = self.core.unicast_timers[timer as usize].take() {
            self.cancel_timer(id);
        }
    }

    fn read_ase(&mut self, device: u32, ase_id: u8) {
        let Some(d) = self.core.devices.get(device as usize) else { return };
        let Some(h) = d.ase(ase_id).map(|a| a.value_handle) else { return };
        let (dev, con) = (d.id, d.acl);
        tracing::debug!("LE Audio: device {device}: reading ASE {ase_id} (0x{h:04X})");
        self.core.gatt.read(
            con,
            h,
            Box::new(move |ctx, status, v| {
                if status == 0 {
                    bap::on_ase_value(ctx, dev, con, h, v);
                } else {
                    tracing::debug!("LE Audio: ASE 0x{h:04X} read failed (ATT 0x{status:02X})");
                }
            }),
        );
    }
}

// ============================================================================
// LeCore: the entry points (HostApp)
// ============================================================================

impl LeCore {
    pub(crate) fn new(
        config: LeConfig,
        shared: Arc<LeShared>,
        controls: Arc<Controls>,
        poster: LePoster,
        events: std::sync::mpsc::Sender<crate::HostEvent>,
        battery: bool,
    ) -> LeCore {
        LeCore {
            events,
            battery,
            iso: IsoSender::new(shared.stats.clone()),
            shared,
            controls,
            poster,
            config: Some(config),
            initialized: false,
            modules: Vec::new(),
            devices: Vec::new(),
            scan_results: Vec::new(),
            scanning: false,
            connect: None,
            rpa_cache: HashMap::new(),
            gatt: GattClient::new(),
            server: GattServer::new(),
            presets: PresetTable::new(),
            unicast: None,
            unicast_timers: [None; UnicastTimer::ALL],
            queue: None,
            status: None,
            timers: Vec::new(),
            next_timer: 1,
            timers_changed: false,
            gatt_retry: None,
            sink_pacs_by_char: BTreeMap::new(),
        }
    }

    pub(crate) fn initialized(&self) -> bool {
        self.initialized
    }

    /// Runs `f` with an LeCtx, then settles (GATT queue, callbacks,
    /// notifications, timer)
    pub(crate) fn run<R>(&mut self, stack: &Stack, f: impl FnOnce(&mut LeCtx<'_>) -> R) -> R {
        let mut ctx = LeCtx { stack, core: self };
        let r = f(&mut ctx);
        ctx.settle();
        r
    }

    /// After hci_init / l2cap_init, before HCI power on (v1
    /// Core::stack_init): bonds TLV, SM, GATT client, ISO, the modules'
    /// on_stack_init, the GATT server with their services
    pub(crate) fn stack_init(&mut self, stack: &Stack) {
        if self.initialized {
            return;
        }
        tracing::info!("LE Audio (experimental): initializing (SM, GATT client / server, ISO)");
        let dir: PathBuf = self.config.as_ref().and_then(|c| c.bond_dir.clone()).unwrap_or_else(std::env::temp_dir);
        let path = dir.join("le_audio_bonds.tlv");
        stack.le_init(&path.to_string_lossy());
        tracing::info!("LE Audio: bonds in {}", path.display());

        // The modules (once per BtHost)
        if let Some(cfg) = self.config.take() {
            let mut setup = LeModuleSetup {
                bond_dir: cfg.bond_dir,
                media: cfg.media,
                ccp_sim: cfg.ccp_sim,
                ccp_console: cfg.ccp_console,
                poster: self.poster.clone(),
                events: self.events.clone(),
                battery: self.battery,
            };
            let mut hooks = Vec::new();
            let mut names = Vec::new();
            for e in super::modules::create_all(&mut setup) {
                names.push(e.module.name());
                self.modules.push(Some(e.module));
                hooks.extend(e.hooks);
            }
            tracing::debug!("LE Audio: modules {}", names.join(", "));
            *self.shared.hooks.lock().unwrap_or_else(|e| e.into_inner()) = hooks;
        }

        self.devices.clear();
        self.scan_results.clear();
        self.scanning = false;
        self.connect = None;
        self.rpa_cache.clear();
        self.gatt = GattClient::new();
        self.sink_pacs_by_char.clear();
        self.shared.iso_len.store(0, Ordering::Release);
        self.shared.iso_count.store(0, Ordering::Release);
        self.unicast = Some(Unicast::default());
        self.status = None;
        self.initialized = true;

        self.run(stack, |ctx| ctx.each_module(|m, ctx| m.on_stack_init(ctx)));

        self.server = GattServer::new();
        self.server.build_base();
        for i in 0..self.modules.len() {
            if let Some(mut m) = self.modules[i].take() {
                m.add_gatt_services(&mut self.server.builder(i));
                self.modules[i] = Some(m);
            }
        }
        self.server.start(stack);
        self.shared.initialized.store(true, Ordering::Release);
    }

    /// The run loop has stopped (v1 Core::stack_deinit): the modules'
    /// on_stack_deinit, a connect waiting gets "Bluetooth stopped", all
    /// session state goes
    pub(crate) fn stack_deinit(&mut self, stack: &Stack) {
        if !self.initialized {
            return;
        }
        self.shared.initialized.store(false, Ordering::Release);
        self.shared.hci_working.store(false, Ordering::Release);
        tracing::debug!(
            "LE Audio: deinit ({} device(s), connect pending {}, {} timer(s))",
            self.devices.len(),
            self.connect.is_some(),
            self.timers.len()
        );
        for m in self.modules.iter_mut().flatten() {
            m.on_stack_deinit();
        }
        self.iso.stop(stack);
        if let Some(c) = self.connect.take() {
            c.result.lock().unwrap_or_else(|e| e.into_inner()).error = "Bluetooth stopped".into();
            (c.done)(false);
        }
        self.initialized = false;
        self.unicast = None;
        self.gatt.detach_all();
        // Callbacks of operations cut short are dropped: no BTstack calls
        // any more
        drop(self.gatt.take_ready());
        self.timers.clear();
        self.devices.clear();
        self.scan_results.clear();
        self.rpa_cache.clear();
        self.queue = None;
        self.status = None;
        tracing::info!("LE Audio: stopped");
    }

    pub(crate) fn on_hci(&mut self, stack: &Stack, packet: &[u8]) {
        if self.initialized {
            self.run(stack, |ctx| ctx.on_hci(packet));
        }
    }

    pub(crate) fn on_sm(&mut self, stack: &Stack, packet: &[u8]) {
        if self.initialized {
            self.run(stack, |ctx| ctx.on_sm(packet));
        }
    }

    pub(crate) fn on_gatt(&mut self, stack: &Stack, packet: &[u8]) {
        // GATT_EVENT_MTU: con, MTU (the bearer's, shared with att_server)
        if packet.first() == Some(&GATT_EVENT_MTU)
            && let Some(&[_, _, c0, c1, m0, m1]) = packet.get(..6)
        {
            tracing::debug!(
                "LE Audio: GATT MTU {} on 0x{:04X}",
                u16::from_le_bytes([m0, m1]),
                u16::from_le_bytes([c0, c1])
            );
            self.server.on_mtu(u16::from_le_bytes([c0, c1]), u16::from_le_bytes([m0, m1]));
        }
        if self.initialized {
            self.run(stack, |ctx| ctx.core.gatt.on_event(stack, packet));
        }
    }

    pub(crate) fn on_att_event(&mut self, packet: &[u8]) {
        if let Some((con, mtu)) = events::att_mtu_exchange_complete(packet) {
            tracing::debug!("LE Audio: ATT server MTU {mtu} on 0x{con:04X}");
            self.server.on_mtu(con, mtu);
        }
    }

    /// The ISO handler: HCI_EVENT_CIS_CAN_SEND_NOW (incoming ISO data of
    /// source ASEs is not used)
    pub(crate) fn on_iso(&mut self, stack: &Stack, packet: &[u8]) {
        if self.initialized {
            self.iso.on_cis_can_send_now(stack, packet);
        }
    }

    pub(crate) fn on_iso_timer(&mut self, stack: &Stack) {
        self.iso.on_timer(stack);
    }

    /// TIMER_LE: the due timers of run_after
    pub(crate) fn on_timer(&mut self, stack: &Stack) {
        self.timers_changed = true;
        self.run(stack, |ctx| {
            let now = ctx.stack.time_ms();
            let mut due: Vec<(u32, u32)> = ctx
                .core
                .timers
                .iter()
                .filter(|t| now.wrapping_sub(t.due) as i32 >= 0)
                .map(|t| (t.due.wrapping_sub(now), t.id))
                .collect();
            tracing::trace!("LE Audio: timer tick, {} due of {}", due.len(), ctx.core.timers.len());
            // Earliest first
            due.sort_by_key(|(d, _)| *d as i32);
            for (_, id) in due {
                let Some(pos) = ctx.core.timers.iter().position(|t| t.id == id) else { continue };
                let t = ctx.core.timers.remove(pos);
                ctx.core.timers_changed = true;
                (t.cb)(ctx);
            }
        });
    }

    pub(crate) fn att_read(&mut self, stack: &Stack, con: u16, handle: u16, offset: u16) -> Result<Vec<u8>, u8> {
        if !self.initialized {
            return Err(ATT_ERROR_UNLIKELY_ERROR);
        }
        self.run(stack, |ctx| ctx.att_read(con, handle, offset))
    }

    pub(crate) fn att_write(&mut self, stack: &Stack, write: &AttWriteRequest<'_>) -> u8 {
        if !self.initialized {
            return ATT_ERROR_UNLIKELY_ERROR;
        }
        self.run(stack, |ctx| ctx.att_write(write))
    }
}

/// Queues closures for the BTstack thread from any thread without waiting
/// (v1 Core::post). A closure still queued when the session ends never
/// runs.
#[derive(Clone)]
pub struct LePoster {
    jobs: mpsc::Sender<Job>,
    remote: Option<Remote>,
}

impl LePoster {
    pub(crate) fn new(jobs: mpsc::Sender<Job>, remote: Remote) -> LePoster {
        LePoster { jobs, remote: Some(remote) }
    }

    /// A poster whose closures never run (tests of modules)
    #[cfg(test)]
    pub(crate) fn detached() -> LePoster {
        let (jobs, _) = mpsc::channel();
        LePoster { jobs, remote: None }
    }

    pub fn post(&self, f: impl FnOnce(&mut LeCtx<'_>) + Send + 'static) {
        let job: Job = Box::new(move |app: &mut HostApp, stack: &Stack| {
            if let Some(le) = app.le.as_mut()
                && le.initialized
            {
                le.run(stack, f);
            }
        });
        if self.jobs.send(job).is_ok()
            && let Some(r) = self.remote
        {
            r.wake();
        }
    }
}

/// The stream's jitter buffer, shared by [`super::LeSduSink`] (encode
/// thread) and the ISO sender (BTstack thread)
pub type SharedSduQueue = Arc<Mutex<SduQueue>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advertising_data_is_parsed_as_v1() {
        let mut r = LeScanResult { connectable: true, ..LeScanResult::default() };
        // Flags, ASCS service data (targeted, sink contexts 0x0006), CAS
        // service data, RSI, short name
        let mut ad = vec![2, 0x01, 0x06, 8, 0x16, 0x4E, 0x18, 0x01, 0x06, 0x00, 0x00, 0x00];
        ad.extend_from_slice(&[4, 0x16, 0x53, 0x18, 0x00]);
        ad.extend_from_slice(&[7, 0x2E, 1, 2, 3, 4, 5, 6]);
        ad.extend_from_slice(&[4, 0x08, b'B', b'u', b'd']);
        parse_ad(&mut r, &ad);
        assert!(r.le_audio && r.targeted);
        assert_eq!(r.uuids16, [0x184E, 0x1853]);
        assert_eq!(r.ascs_sink_contexts, 0x0006);
        assert_eq!(r.rsi, Some([1, 2, 3, 4, 5, 6]));
        assert_eq!(r.name, "Bud");
        // The complete name wins over the short one
        parse_ad(&mut r, &[5, 0x09, b'B', b'u', b'd', b's']);
        parse_ad(&mut r, &[3, 0x08, b'X', b'X']);
        assert_eq!(r.name, "Buds");
        // An earbud appearance without LE Audio UUIDs is a candidate
        let mut e = LeScanResult { connectable: true, ..LeScanResult::default() };
        parse_ad(&mut e, &[3, 0x19, 0x41, 0x09, 3, 0x03, 0x0F, 0x18]);
        assert!(e.le_audio);
        assert_eq!(e.note, "audio appearance, LE Audio not advertised");
        // Malformed: a length beyond the data ends the parsing
        let mut m = LeScanResult::default();
        parse_ad(&mut m, &[9, 0x09, b'A']);
        assert!(m.name.is_empty() && !m.le_audio);
    }

    #[test]
    fn device_lookups() {
        let mut d = LeDevice::new(LeDeviceId(0), LeAddr::default());
        d.services.push(GattService {
            uuid16: 0x1850,
            uuid128: [0; 16],
            start_handle: 1,
            end_handle: 9,
            primary: true,
            included_by: 0,
            chars: vec![
                GattChar { uuid16: 0x2BC9, value_handle: 3, ..GattChar::default() },
                GattChar { uuid16: 0x2BC9, value_handle: 5, ..GattChar::default() },
            ],
        });
        assert_eq!(d.find_chars(0x1850, 0x2BC9).len(), 2);
        assert_eq!(d.find_chars(0, 0x2BC9).len(), 2);
        assert_eq!(d.find_char(0x184E, 0x2BC9), None);
        assert_eq!(d.find_char(0x1850, 0x2BC9).map(|c| c.value_handle), Some(3));
        d.ases.push(Ase { id: 2, sink: true, ..Ase::default() });
        d.ases.push(Ase { id: 3, sink: false, ..Ase::default() });
        assert_eq!(d.sink_ase_count(), 1);
        assert!(d.ase(3).is_some_and(|a| !a.sink));
        assert!(!d.connected());
        assert!(LeAddr { addr: [0x5A, 1, 2, 3, 4, 5], addr_type: LeAddr::RANDOM }.is_rpa());
        assert!(!LeAddr { addr: [0xDA, 1, 2, 3, 4, 5], addr_type: LeAddr::RANDOM }.is_rpa());
        assert!(!LeAddr { addr: [0x5A, 1, 2, 3, 4, 5], addr_type: LeAddr::PUBLIC }.is_rpa());
    }
}
