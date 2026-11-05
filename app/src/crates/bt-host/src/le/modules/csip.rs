//! CSIP Set Coordinator (v1 le_audio_csip.cpp).
//!
//! Earbuds that form a Coordinated Set (two devices, one per ear) stream
//! together. The user names one of them; this module finds and connects the
//! others, so the core streams to the whole set (one CIS per earbud,
//! channels by audio location - `plan_stream` does that for a list of
//! devices).
//!
//! - `on_gatt_ready`: CSIS (0x1846, primary or included by CAS) found: read
//!   the Set Identity Resolving Key (0x2B84; plaintext, or encrypted with
//!   the LTK of the link: sdf, CSIS 4.5), Coordinated Set Size (0x2B85), Set
//!   Member Lock (0x2B86) and Set Member Rank (0x2B87). Devices with the
//!   same SIRK form one set (an index into the module's sets, the
//!   `LeScanResult::set_id`). The SIRK of a bonded device is kept in
//!   `<bond_dir>\le_audio_csip.txt` for later sessions.
//! - `on_scan_result`: an advertised RSI (AD 0x2E) is checked against every
//!   known SIRK (sih, CSIS 4.7); a match sets `set_id` / `note` /
//!   `le_audio`.
//! - `prepare_group` (worker): waits for the CSIS reads of the devices of
//!   the stream, adds set members that are already connected, scans (up to
//!   `SCAN_TIMEOUT_MS`) for RSIs of the set and connects the members found
//!   one after the other (the core sets up one LE connection at a time),
//!   then orders each set's devices by rank. Members not found in time are
//!   left out with a warning; the stream goes to the members available. A
//!   member that disconnects later makes the core report the stream lost;
//!   the worker sets it up again and `prepare_group` looks for the member
//!   anew.
//! - Lock (CSIP 4.6 Lock Request / 4.7 Ordered Access): when every member of
//!   the set exposes Set Member Lock, `prepare_group` locks them in rank
//!   order before the stream is set up (the set-wide BAP procedures run
//!   under the lock) and `on_stream_state` releases them in reverse order
//!   once the stream runs or failed (at the latest after `LOCK_HOLD_MAX_MS`).
//!   Lock Denied (0x80, another coordinator holds a member) releases what
//!   was locked, waits a little and tries again; after `LOCK_ATTEMPTS`, or
//!   on any other error, the stream is set up without the lock (warning), as
//!   it is for a set whose members do not all support the lock (optional).
//!   Lock notifications (the member released it by its lock timeout) and
//!   disconnects (the member releases it) clear the module's lock state.
//! - The crypto is `le_audio::csip_crypto` (self-check at every stack init:
//!   if it fails the module does nothing).
//!
//! Not implemented: notifications of SIRK / size / rank changes, an
//! OOB-only SIRK (CSIS error 0x83, logged), more than one CSIS per device.
//!
//! Threads: the BTstack-thread state (`State`) lives in an
//! `Rc<RefCell<..>>` of the module that GATT / timer callbacks capture; the
//! worker reaches it through `BtHost::le_run` / `le_call` and
//! `LeCtx::module_mut::<CsipModule>()`. What both sides read (the crypto
//! check, the set the worker looks for) is in `Shared`.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use btstack_sys::Stack;
use le_audio::csip_crypto::{self, Key};
use le_audio::defs::UUID_SERVICE_CSIS;

use super::super::core::{LeCtx, LeDevice, LeDone, LeTimerId};
use super::super::gatt_client::{ATT_ERROR_HCI_DISCONNECT_RECEIVED, PROPERTY_NOTIFY};
use super::super::module::{LeGroupHooks, LeModule, LeModuleEntry, LeModuleSetup};
use super::super::{LeAddr, LeDeviceId, LeScanResult, LeStreamStatus};
use crate::BtHost;

const UUID_CHAR_SIRK: u16 = 0x2B84;
const UUID_CHAR_SET_SIZE: u16 = 0x2B85;
/// Set Member Lock
const UUID_CHAR_LOCK: u16 = 0x2B86;
/// Set Member Rank
const UUID_CHAR_RANK: u16 = 0x2B87;

const SIRK_ENCRYPTED: u8 = 0x00;
const SIRK_PLAINTEXT: u8 = 0x01;
/// Set Member Lock values
const LOCK_UNLOCKED: u8 = 0x01;
const LOCK_LOCKED: u8 = 0x02;
const ATT_DISCONNECTED: u8 = ATT_ERROR_HCI_DISCONNECT_RECEIVED;
/// No answer in LOCK_WRITE_TIMEOUT_MS (ours)
const LOCK_TIMEOUT: u8 = 0xFF;
/// CSIS application errors (CSIS 1.0.1 table 5.1)
const ATT_ERROR_LOCK_DENIED: u8 = 0x80;
const ATT_ERROR_LOCK_RELEASE_NOT_ALLOWED: u8 = 0x81;
const ATT_ERROR_INVALID_LOCK_VALUE: u8 = 0x82;
const ATT_ERROR_OOB_SIRK_ONLY: u8 = 0x83;
const ATT_ERROR_LOCK_ALREADY_GRANTED: u8 = 0x84;

/// CSIS reads of a connected device
const READ_TIMEOUT_MS: u64 = 6000;
/// Looking for the missing members
const SCAN_TIMEOUT_MS: u64 = 10000;
const CONNECT_TIMEOUT_MS: u64 = 30000;
/// RSIs remembered (resolved or not)
const RSI_CACHE_MAX: usize = 256;
const LOCK_WRITE_TIMEOUT_MS: u64 = 5000;
/// Lock Denied: tries before going on unlocked
const LOCK_ATTEMPTS: u32 = 5;
/// Times the attempt, plus up to 200 ms random
const LOCK_BACKOFF_MS: u32 = 300;
/// Released by then even without on_stream_state
const LOCK_HOLD_MAX_MS: u32 = 50000;
/// v1 Core::run's timeout
const RUN_TIMEOUT: Duration = Duration::from_secs(3);
/// v1 Core::scan looked at its stop condition this often
const SCAN_POLL: Duration = Duration::from_millis(50);

const FILE_NAME: &str = "le_audio_csip.txt";
const FILE_HEADER: &str =
    "# A2DPWB LE Audio: SIRK of bonded coordinated set members (identity address, type, SIRK, size, rank)\n";

// ============================================================================
// Pure helpers
// ============================================================================

/// A SIRK value as sent (least significant octet first) to a [`Key`]
fn key_from_air(v: &[u8]) -> Key {
    let mut k = [0u8; 16];
    for (i, b) in k.iter_mut().enumerate() {
        *b = v[15 - i];
    }
    k
}

/// 32 lowercase hex digits, most significant octet first
fn key_hex(k: &Key) -> String {
    k.iter().map(|b| format!("{b:02x}")).collect()
}

/// 32 hex digits, most significant octet first; None if malformed
fn parse_key(hex: &str) -> Option<Key> {
    if hex.len() != 32 || !hex.is_ascii() {
        return None;
    }
    let mut k = [0u8; 16];
    for (i, b) in k.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(k)
}

/// A SIRK kept for a bonded device (le_audio_csip.txt)
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stored {
    /// Identity address
    addr: LeAddr,
    sirk: Key,
    size: u8,
    rank: u8,
}

/// One line of le_audio_csip.txt: "<identity> <type> <SIRK hex> <size>
/// <rank>"; None for a comment or a malformed line
fn parse_stored_line(line: &str) -> Option<Stored> {
    if line.starts_with('#') {
        return None;
    }
    let mut it = line.split_whitespace();
    let addr = crate::parse_addr(it.next()?)?;
    let addr_type: u8 = it.next()?.parse().ok()?;
    let sirk = parse_key(it.next()?)?;
    let size: u8 = it.next()?.parse().ok()?;
    let rank: u8 = it.next()?.parse().ok()?;
    Some(Stored { addr: LeAddr { addr, addr_type }, sirk, size, rank })
}

fn format_stored(stored: &[Stored]) -> String {
    let mut s = String::from(FILE_HEADER);
    for x in stored {
        s.push_str(&format!("{} {} {} {} {}\n", x.addr, x.addr.addr_type, key_hex(&x.sirk), x.size, x.rank));
    }
    s
}

/// What an ATT status of a lock write means (v1 lock_error)
fn lock_error(status: u8) -> &'static str {
    match status {
        ATT_ERROR_LOCK_DENIED => "Lock Denied",
        ATT_ERROR_LOCK_RELEASE_NOT_ALLOWED => "Lock Release Not Allowed",
        ATT_ERROR_INVALID_LOCK_VALUE => "Invalid Lock Value",
        ATT_ERROR_LOCK_ALREADY_GRANTED => "Lock Already Granted",
        ATT_DISCONNECTED => "disconnected",
        LOCK_TIMEOUT => "no answer",
        _ => "error",
    }
}

/// The device an advertiser is: the identity its RPA resolved to (SM, a
/// bonded device), else the advertised address
fn scan_identity(r: &LeScanResult) -> LeAddr {
    r.identity.unwrap_or(r.addr)
}

/// An advertiser is this device: its identity (`LeDevice::addr` once SM
/// knows it) or the address of its current link
fn is_device(r: &LeScanResult, d: &LeDevice) -> bool {
    scan_identity(r).addr == d.addr.addr || r.addr.addr == d.addr.addr || r.addr.addr == d.peer_addr.addr
}

/// A member as the worker sees it
#[derive(Debug, Clone, PartialEq, Eq)]
struct Snap {
    id: LeDeviceId,
    set: usize,
    size: u8,
    rank: u8,
    addr: LeAddr,
}

/// Rank order: rank 1 first, no rank (0) last, then by device id
fn sort_by_rank(members: &mut [Snap]) {
    members.sort_by_key(|s| (if s.rank > 0 { u32::from(s.rank) } else { u32::MAX }, s.id));
}

/// The RSI cache keeps the last RSI_CACHE_MAX advertised RSIs: every device
/// around advertises its own and they rotate (RPA-like, about every 15
/// min), so an unbounded cache would grow for as long as A2DPWB scans
#[derive(Default)]
struct RsiCache {
    /// RSI -> set index, None: no known set
    map: HashMap<u64, Option<usize>>,
    /// Keys of map, oldest first
    order: VecDeque<u64>,
}

impl RsiCache {
    fn key(rsi: &[u8; 6]) -> u64 {
        rsi.iter().enumerate().fold(0u64, |k, (i, b)| k | u64::from(*b) << (8 * i))
    }

    fn get(&self, rsi: &[u8; 6]) -> Option<Option<usize>> {
        self.map.get(&RsiCache::key(rsi)).copied()
    }

    fn insert(&mut self, rsi: &[u8; 6], set: Option<usize>) {
        let key = RsiCache::key(rsi);
        if self.map.insert(key, set).is_none() {
            self.order.push_back(key);
        }
        while self.order.len() > RSI_CACHE_MAX {
            if let Some(old) = self.order.pop_front() {
                self.map.remove(&old);
            }
        }
    }

    /// RSIs that did not resolve may belong to a new set
    fn forget_unresolved(&mut self) {
        self.map.retain(|_, s| s.is_some());
        let map = &self.map;
        self.order.retain(|k| map.contains_key(k));
    }

    fn clear(&mut self) {
        self.map.clear();
        self.order.clear();
    }
}

// ============================================================================
// BTstack-thread state
// ============================================================================

/// Which optional CSIS characteristic a read fills
#[derive(Debug, Clone, Copy)]
enum Field {
    Size,
    Rank,
    Lock,
}

/// CSIS of one connected device (per LeDeviceId)
struct Member {
    con: u16,
    /// Reads in flight
    pending: usize,
    /// Reads finished (or no CSIS)
    done: bool,
    /// SIRK known
    ok: bool,
    encrypted_sirk: bool,
    sirk: Key,
    /// 0: not exposed
    size: u8,
    /// 0: not exposed
    rank: u8,
    /// 0: not exposed, 1 unlocked, 2 locked
    lock: u8,
    /// Set Member Lock value handle, 0: none
    lock_handle: u16,
    /// A2DPWB holds this member's lock
    we_locked: bool,
    /// Index into State::sets
    set: Option<usize>,
    /// Workers waiting for the reads (wait_member)
    waiters: Vec<LeDone>,
}

impl Member {
    fn new(con: u16) -> Member {
        Member {
            con,
            pending: 0,
            done: false,
            ok: false,
            encrypted_sirk: false,
            sirk: [0; 16],
            size: 0,
            rank: 0,
            lock: 0,
            lock_handle: 0,
            we_locked: false,
            set: None,
            waiters: Vec::new(),
        }
    }
}

struct KnownSet {
    sirk: Key,
    size: u8,
}

/// The module's BTstack-thread state
struct State {
    members: BTreeMap<LeDeviceId, Member>,
    sets: Vec<KnownSet>,
    stored: Vec<Stored>,
    rsi: RsiCache,
    /// Locked members (id, con) in rank order
    locked: Vec<(LeDeviceId, u16)>,
    lock_timer: Option<LeTimerId>,
    /// <bond_dir>\le_audio_csip.txt; None: in memory only
    file: Option<PathBuf>,
}

impl State {
    fn new(file: Option<PathBuf>) -> State {
        State {
            members: BTreeMap::new(),
            sets: Vec::new(),
            stored: Vec::new(),
            rsi: RsiCache::default(),
            locked: Vec::new(),
            lock_timer: None,
            file,
        }
    }

    /// The member if it is still on the link `con`
    fn member(&mut self, id: LeDeviceId, con: u16) -> Option<&mut Member> {
        self.members.get_mut(&id).filter(|m| m.con == con)
    }

    fn add_set(&mut self, sirk: &Key, size: u8) -> usize {
        if let Some(i) = self.sets.iter().position(|s| s.sirk == *sirk) {
            self.sets[i].size = self.sets[i].size.max(size);
            return i;
        }
        self.sets.push(KnownSet { sirk: *sirk, size });
        // RSIs that did not resolve may belong to the new set
        self.rsi.forget_unresolved();
        self.sets.len() - 1
    }

    /// The known set an RSI belongs to (cached)
    fn resolve_rsi(&mut self, rsi: &[u8; 6]) -> Option<usize> {
        if let Some(set) = self.rsi.get(rsi) {
            return set;
        }
        let set = self.sets.iter().position(|s| csip_crypto::rsi_matches(&s.sirk, rsi));
        self.rsi.insert(rsi, set);
        set
    }

    fn save_stored(&self) {
        let Some(path) = &self.file else { return };
        if std::fs::write(path, format_stored(&self.stored)).is_err() {
            tracing::warn!("CSIP: cannot write {}", path.display());
        }
    }

    fn load_stored(&mut self, stack: &Stack) {
        self.stored.clear();
        let Some(path) = &self.file else { return };
        let Ok(text) = std::fs::read_to_string(path) else { return };
        let mut dropped = 0;
        for s in text.lines().filter_map(parse_stored_line) {
            if !bonded(stack, &s.addr) {
                // The bond was removed: forget its SIRK too
                dropped += 1;
                continue;
            }
            self.add_set(&s.sirk, s.size);
            self.stored.push(s);
        }
        if dropped > 0 {
            self.save_stored();
        }
        if !self.stored.is_empty() {
            tracing::info!(
                "CSIP: {} known coordinated set(s) of {} bonded device(s)",
                self.sets.len(),
                self.stored.len()
            );
        }
    }

    /// Keeps the SIRK of a bonded member for later sessions
    fn store(&mut self, stack: &Stack, dev_addr: LeAddr, id: LeDeviceId) {
        let Some(m) = self.members.get(&id) else { return };
        // Not bonded: this session only
        let Some(index) = stack.sm_le_device_index(m.con) else { return };
        let addr = match stack.le_device_db_info(index) {
            Some(e) => LeAddr { addr: e.addr, addr_type: e.addr_type },
            None => dev_addr,
        };
        let s = Stored { addr, sirk: m.sirk, size: m.size, rank: m.rank };
        if self.stored.contains(&s) {
            return;
        }
        self.stored.retain(|x| x.addr != s.addr);
        self.stored.push(s);
        self.save_stored();
    }
}

/// A bond for this identity address is in le_device_db
fn bonded(stack: &Stack, addr: &LeAddr) -> bool {
    (0..stack.le_device_db_max_count()).any(|i| stack.le_device_db_info(i).is_some_and(|e| e.addr == addr.addr))
}

fn addr_of(ctx: &LeCtx<'_>, id: LeDeviceId) -> String {
    ctx.device(id).map_or_else(|| "?".to_string(), |d| d.addr.to_string())
}

/// The module's state, from a callback or a worker closure (not from the
/// module's own hooks: they use `self.st`)
fn state(ctx: &mut LeCtx<'_>) -> Option<Rc<RefCell<State>>> {
    ctx.module_mut::<CsipModule>().map(|m| m.st.clone())
}

/// The SIRK read finished (v1 on_sirk): m.sirk / m.ok
fn on_sirk(stack: &Stack, addr: &str, m: &mut Member, status: u8, v: &[u8]) {
    if status != 0 {
        tracing::warn!(
            "CSIP: {addr}: reading the SIRK failed (0x{status:02x}{})",
            if status == ATT_ERROR_OOB_SIRK_ONLY { ", OOB SIRK only: not supported" } else { "" }
        );
        return;
    }
    if v.len() < 17 {
        tracing::warn!("CSIP: {addr}: SIRK value of {} bytes", v.len());
        return;
    }
    tracing::debug!("CSIP: {addr}: SIRK type 0x{:02X} read", v[0]);
    let value = key_from_air(&v[1..17]);
    match v[0] {
        SIRK_PLAINTEXT => m.sirk = value,
        SIRK_ENCRYPTED => {
            // sdf(K, EncSIRK) with K = the LTK of the link (BTstack keeps it
            // most significant octet first, as the spec writes it)
            let Some(index) = stack.sm_le_device_index(m.con) else {
                tracing::warn!("CSIP: {addr}: encrypted SIRK, but no bond (LTK) to decrypt it");
                return;
            };
            let k = stack.le_device_db_encryption(index).map(|e| e.ltk).unwrap_or_default();
            if k == [0u8; 16] {
                tracing::warn!("CSIP: {addr}: encrypted SIRK, but the LTK of the bond is not available");
                return;
            }
            m.sirk = csip_crypto::sdf(&k, &value);
            m.encrypted_sirk = true;
        }
        other => {
            tracing::warn!("CSIP: {addr}: SIRK type 0x{other:02x} unknown");
            return;
        }
    }
    m.ok = true;
}

/// One CSIS read finished; after the last: the set, the log line, the
/// stored SIRK and the waiting workers
fn read_done(ctx: &mut LeCtx<'_>, st: &mut State, id: LeDeviceId) {
    let Some(m) = st.members.get_mut(&id) else { return };
    m.pending = m.pending.saturating_sub(1);
    if m.pending > 0 {
        return;
    }
    m.done = true;
    if m.ok {
        let (sirk, size) = (m.sirk, m.size);
        let set = st.add_set(&sirk, size);
        let Some(m) = st.members.get_mut(&id) else { return };
        m.set = Some(set);
        let lock = match m.lock {
            0 => "no lock",
            1 => "unlocked",
            2 => "locked",
            _ => "lock ?",
        };
        tracing::info!(
            "CSIP: {}: member of coordinated set {set}, rank {} of {} ({} SIRK, {lock})",
            addr_of(ctx, id),
            m.rank,
            m.size,
            if m.encrypted_sirk { "encrypted" } else { "plaintext" }
        );
        if let Some(addr) = ctx.device(id).map(|d| d.addr) {
            st.store(ctx.stack(), addr, id);
        }
    }
    let Some(m) = st.members.get_mut(&id) else { return };
    let ok = m.ok;
    for w in std::mem::take(&mut m.waiters) {
        w(ok);
    }
}

// ---- Lock (CSIP 4.6 / 4.7), BTstack thread ----

/// Release every lock A2DPWB holds, highest rank first
fn release_locks(ctx: &mut LeCtx<'_>, rc: &Rc<RefCell<State>>, done: Option<LeDone>) {
    let order: Vec<(LeDeviceId, u16)> = {
        let mut st = rc.borrow_mut();
        if let Some(t) = st.lock_timer.take() {
            ctx.cancel_timer(t);
        }
        let order = st.locked.iter().rev().copied().collect();
        st.locked.clear();
        order
    };
    release_next(ctx, rc.clone(), Rc::new(order), 0, done);
}

fn release_next(
    ctx: &mut LeCtx<'_>,
    rc: Rc<RefCell<State>>,
    order: Rc<Vec<(LeDeviceId, u16)>>,
    mut i: usize,
    done: Option<LeDone>,
) {
    while i < order.len() {
        let (id, con) = order[i];
        let handle =
            rc.borrow_mut().member(id, con).filter(|m| m.we_locked && m.lock_handle != 0).map(|m| m.lock_handle);
        // Gone, or released by the member
        let Some(handle) = handle else {
            i += 1;
            continue;
        };
        ctx.gatt().write(
            con,
            handle,
            vec![LOCK_UNLOCKED],
            Some(Box::new(move |ctx, status| {
                let addr = addr_of(ctx, id);
                if let Some(x) = rc.borrow_mut().member(id, con) {
                    x.lock = LOCK_UNLOCKED;
                    x.we_locked = false;
                }
                if status == 0 {
                    tracing::info!("CSIP: {addr}: lock released");
                } else if status != ATT_DISCONNECTED {
                    tracing::warn!("CSIP: {addr}: releasing the lock failed: {} (0x{status:02x})", lock_error(status));
                }
                release_next(ctx, rc, order, i + 1, done);
            })),
        );
        return;
    }
    if let Some(done) = done {
        done(true);
    }
}

fn start_lock_timer(ctx: &mut LeCtx<'_>, rc: &Rc<RefCell<State>>) {
    if let Some(t) = rc.borrow_mut().lock_timer.take() {
        ctx.cancel_timer(t);
    }
    let r = rc.clone();
    let timer = ctx.run_after(
        LOCK_HOLD_MAX_MS,
        Box::new(move |ctx| {
            let still_locked = {
                let mut st = r.borrow_mut();
                st.lock_timer = None;
                !st.locked.is_empty()
            };
            if !still_locked {
                return;
            }
            tracing::warn!("CSIP: the set is still locked {} s after locking it: releasing", LOCK_HOLD_MAX_MS / 1000);
            release_locks(ctx, &r, None);
        }),
    );
    rc.borrow_mut().lock_timer = Some(timer);
}

// ============================================================================
// The module (BTstack thread)
// ============================================================================

/// What the BTstack thread and the worker both look at
struct Shared {
    /// csip_crypto::self_test passed at the last stack init
    crypto_ok: AtomicBool,
    /// find_members (worker) <-> on_scan_result (BTstack thread): the set
    /// looked for, -1: none
    wanted_set: AtomicI64,
    /// on_scan_result saw a member of wanted_set not connected or tried yet
    member_seen: AtomicBool,
    /// Connect attempts of this search (identities)
    tried: Mutex<Vec<[u8; 6]>>,
}

impl Shared {
    fn was_tried(&self, a: &LeAddr) -> bool {
        self.tried.lock().unwrap_or_else(|e| e.into_inner()).contains(&a.addr)
    }
}

/// BTstack-thread part
pub struct CsipModule {
    st: Rc<RefCell<State>>,
    shared: Arc<Shared>,
}

impl LeModule for CsipModule {
    fn name(&self) -> &'static str {
        "CSIP"
    }

    fn on_stack_init(&mut self, ctx: &mut LeCtx<'_>) {
        let mut st = self.st.borrow_mut();
        st.members.clear();
        st.sets.clear();
        st.rsi.clear();
        let ok = csip_crypto::self_test();
        self.shared.crypto_ok.store(ok, Ordering::Release);
        if ok {
            tracing::info!("CSIP: crypto self-check passed (sih, sef against the CSIS sample data)");
        } else {
            tracing::error!("CSIP: crypto self-check FAILED: coordinated sets are not used");
        }
        st.load_stored(ctx.stack());
    }

    fn on_stack_deinit(&mut self) {
        let mut st = self.st.borrow_mut();
        st.members.clear();
        st.sets.clear();
        st.rsi.clear();
        st.locked.clear();
        // The run loop has stopped: its timers are gone
        st.lock_timer = None;
    }

    /// The stream runs (or its set-up ended): the set-wide procedures are
    /// done, release the lock
    fn on_stream_state(&mut self, ctx: &mut LeCtx<'_>, _status: &LeStreamStatus, _streaming: bool) {
        release_locks(ctx, &self.st, None);
    }

    fn on_scan_result(&mut self, ctx: &mut LeCtx<'_>, result: &mut LeScanResult, _ad: &[u8]) {
        let Some(rsi) = result.rsi else { return };
        if !self.shared.crypto_ok.load(Ordering::Acquire) {
            return;
        }
        let (set, size) = {
            let mut st = self.st.borrow_mut();
            if st.sets.is_empty() {
                return;
            }
            let Some(set) = st.resolve_rsi(&rsi) else { return };
            (set, st.sets[set].size)
        };
        tracing::debug!("CSIP: {} RSI {rsi:02X?} resolves to coordinated set {set}", result.addr);
        result.set_id = Some(set as u32);
        result.le_audio = true;
        result.note = if size > 0 {
            format!("member of coordinated set {set} ({size} devices)")
        } else {
            format!("member of coordinated set {set}")
        };
        // find_members waits for a member it has not connected yet
        if set as i64 == self.shared.wanted_set.load(Ordering::Acquire)
            && result.connectable
            && !self.shared.member_seen.load(Ordering::Acquire)
        {
            let connected = ctx
                .device_ids()
                .into_iter()
                .any(|id| ctx.device(id).is_some_and(|d| d.connected() && is_device(result, d)));
            if connected || self.shared.was_tried(&scan_identity(result)) {
                return;
            }
            self.shared.member_seen.store(true, Ordering::Release);
        }
    }

    fn on_gatt_ready(&mut self, ctx: &mut LeCtx<'_>, dev: LeDeviceId) {
        let Some(d) = ctx.device(dev) else { return };
        let con = d.acl;
        let sirk = d.find_char(UUID_SERVICE_CSIS, UUID_CHAR_SIRK).map(|c| c.value_handle);
        let reads: Vec<(u16, Field)> =
            [(UUID_CHAR_SET_SIZE, Field::Size), (UUID_CHAR_RANK, Field::Rank), (UUID_CHAR_LOCK, Field::Lock)]
                .into_iter()
                .filter_map(|(uuid, f)| d.find_char(UUID_SERVICE_CSIS, uuid).map(|c| (c.value_handle, f)))
                .collect();
        let lock = d.find_char(UUID_SERVICE_CSIS, UUID_CHAR_LOCK).cloned();
        tracing::debug!(
            "CSIP: {}: CSIS SIRK {:04X?}, reads {:?}, lock {:04X?}",
            d.addr,
            sirk,
            reads.iter().map(|(h, f)| format!("{f:?}@0x{h:04X}")).collect::<Vec<_>>(),
            lock.as_ref().map(|l| l.value_handle)
        );

        let mut st = self.st.borrow_mut();
        // Workers waiting for an earlier link of this device wait for this one
        let waiters = st.members.remove(&dev).map(|m| m.waiters).unwrap_or_default();
        let mut m = Member::new(con);
        m.waiters = waiters;
        let Some(sirk) = sirk else {
            m.done = true;
            for w in std::mem::take(&mut m.waiters) {
                w(false);
            }
            st.members.insert(dev, m);
            return;
        };
        m.pending = 1 + reads.len();
        if let Some(lock) = &lock {
            m.lock_handle = lock.value_handle;
        }
        st.members.insert(dev, m);
        drop(st);

        if let Some(lock) = lock.filter(|l| l.properties & PROPERTY_NOTIFY != 0) {
            // The member tells when its lock changes: e.g. released by its
            // lock timeout while A2DPWB thought it held it
            let rc = self.st.clone();
            ctx.gatt().listen(
                con,
                lock.value_handle,
                Box::new(move |ctx, v| {
                    let Some(&value) = v.first() else { return };
                    tracing::debug!("CSIP: device {}: Set Member Lock notified {value}", dev.0);
                    let released = {
                        let mut st = rc.borrow_mut();
                        let Some(x) = st.member(dev, con) else { return };
                        x.lock = value;
                        let released = value == LOCK_UNLOCKED && x.we_locked;
                        if released {
                            x.we_locked = false;
                        }
                        released
                    };
                    if released {
                        tracing::info!("CSIP: {} released the lock A2DPWB held (lock timeout)", addr_of(ctx, dev));
                    }
                }),
            );
            ctx.gatt().subscribe(con, &lock, false, None);
        }
        let rc = self.st.clone();
        ctx.gatt().read(
            con,
            sirk,
            Box::new(move |ctx, status, v| {
                let addr = addr_of(ctx, dev);
                let mut st = rc.borrow_mut();
                let Some(x) = st.member(dev, con) else { return };
                on_sirk(ctx.stack(), &addr, x, status, v);
                read_done(ctx, &mut st, dev);
            }),
        );
        for (handle, field) in reads {
            let rc = self.st.clone();
            ctx.gatt().read(
                con,
                handle,
                Box::new(move |ctx, status, v| {
                    let mut st = rc.borrow_mut();
                    let Some(x) = st.member(dev, con) else { return };
                    tracing::debug!("CSIP: device {}: {field:?} read (0x{status:02X}): {v:02X?}", dev.0);
                    if let (0, Some(&b)) = (status, v.first()) {
                        match field {
                            Field::Size => x.size = b,
                            Field::Rank => x.rank = b,
                            Field::Lock => x.lock = b,
                        }
                    }
                    read_done(ctx, &mut st, dev);
                }),
            );
        }
    }

    fn on_disconnected(&mut self, ctx: &mut LeCtx<'_>, dev: LeDeviceId, reason: u8) {
        // A member releases the lock of a client that disconnects
        let Some(m) = self.st.borrow_mut().members.remove(&dev) else { return };
        if let Some(set) = m.set {
            tracing::info!(
                "CSIP: {} (rank {} of coordinated set {set}) disconnected (0x{reason:02x})",
                addr_of(ctx, dev),
                m.rank
            );
        }
        for w in m.waiters {
            w(false);
        }
    }
}

// ============================================================================
// The worker part (prepare_group)
// ============================================================================

/// Worker part (prepare_group)
pub struct CsipHooks {
    shared: Arc<Shared>,
}

impl LeGroupHooks for CsipHooks {
    fn name(&self) -> &'static str {
        "CSIP"
    }

    fn prepare_group(&self, host: &BtHost, ids: &mut Vec<LeDeviceId>, cancelled: &dyn Fn() -> bool) {
        if ids.is_empty() || !self.shared.crypto_ok.load(Ordering::Acquire) {
            return;
        }
        // A lock left from a set-up that never reached on_stream_state (e.g.
        // planning failed)
        host.le_call(Duration::from_millis(LOCK_WRITE_TIMEOUT_MS * 2), |ctx, done| match state(ctx) {
            Some(rc) => release_locks(ctx, &rc, Some(done)),
            None => done(true),
        });
        for &id in ids.iter() {
            wait_member(host, id);
        }
        let mut out: Vec<LeDeviceId> = Vec::new();
        let mut sets_done: Vec<usize> = Vec::new();
        let add = |out: &mut Vec<LeDeviceId>, id: LeDeviceId| {
            if !out.contains(&id) {
                out.push(id);
            }
        };
        for &id in ids.iter() {
            let Some(me) = snapshot(host, id) else {
                add(&mut out, id);
                continue;
            };
            if sets_done.contains(&me.set) {
                continue;
            }
            sets_done.push(me.set);
            let mut have = set_members(host, me.set);
            if !have.iter().any(|s| s.id == me.id) {
                have.push(me.clone());
            }
            let size = have.iter().map(|s| s.size).max().unwrap_or(0).max(me.size) as usize;
            if have.len() < size && !cancelled() {
                self.find_members(host, me.set, size, &mut have, cancelled);
            }
            // A member that went down meanwhile is left out (the device named
            // by the caller stays: the core reports it)
            let up = set_members(host, me.set);
            have.retain(|s| {
                let gone = s.id != me.id && !up.iter().any(|u| u.id == s.id);
                if gone {
                    tracing::warn!("CSIP: set member {} went down, left out", s.addr);
                }
                !gone
            });
            sort_by_rank(&mut have);
            let list = have.iter().map(|s| format!("{} rank {}", s.addr, s.rank)).collect::<Vec<_>>().join(", ");
            for s in &have {
                add(&mut out, s.id);
            }
            if have.len() < size {
                tracing::warn!(
                    "CSIP: only {} of {size} members of coordinated set {} found within {} s: streaming to what is \
                     available",
                    have.len(),
                    me.set,
                    SCAN_TIMEOUT_MS / 1000
                );
            }
            tracing::info!("CSIP: streaming to coordinated set {}, {} of {size} members: {list}", me.set, have.len());
            if !cancelled() {
                lock_set(host, me.set, &have, cancelled);
            }
        }
        *ids = out;
    }
}

impl CsipHooks {
    /// Scan for RSIs of the set and connect the members found, one at a time
    fn find_members(&self, host: &BtHost, set: usize, size: usize, have: &mut Vec<Snap>, cancelled: &dyn Fn() -> bool) {
        tracing::info!(
            "CSIP: coordinated set {set}: {} of {size} members connected, looking for the others (up to {} s)",
            have.len(),
            SCAN_TIMEOUT_MS / 1000
        );
        let shared = &self.shared;
        shared.tried.lock().unwrap_or_else(|e| e.into_inner()).clear();
        shared.wanted_set.store(set as i64, Ordering::Release);
        // Scan until on_scan_result saw a member not tried yet (or the time is
        // up)
        let stop = || shared.member_seen.load(Ordering::Acquire) || cancelled();
        let deadline = Instant::now() + Duration::from_millis(SCAN_TIMEOUT_MS);
        while have.len() < size && !cancelled() {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.as_millis() == 0 {
                break;
            }
            shared.member_seen.store(false, Ordering::Release);
            let results = scan(host, left, &stop);
            for r in results {
                if have.len() >= size || cancelled() {
                    break;
                }
                if r.set_id != Some(set as u32) || !r.connectable || known(host, have, &r) {
                    continue;
                }
                let identity = scan_identity(&r);
                if shared.was_tried(&identity) {
                    continue;
                }
                shared.tried.lock().unwrap_or_else(|e| e.into_inner()).push(identity.addr);
                tracing::info!(
                    "CSIP: {} \"{}\" resolves to coordinated set {set} (RSI): connecting it",
                    r.addr,
                    r.name
                );
                let id = match host.le_connect(r.addr, &r.name, Duration::from_millis(CONNECT_TIMEOUT_MS)) {
                    Ok(id) => id,
                    Err(error) => {
                        tracing::warn!("CSIP: set member {}: {error}", r.addr);
                        continue;
                    }
                };
                let snap = if wait_member(host, id) { snapshot(host, id).filter(|s| s.set == set) } else { None };
                let Some(s) = snap else {
                    tracing::warn!("CSIP: {} is not a member of coordinated set {set} after all: not used", r.addr);
                    host.le_disconnect(id);
                    continue;
                };
                have.push(s);
            }
        }
        shared.wanted_set.store(-1, Ordering::Release);
    }
}

/// v1 Core::scan with a stop condition: LE scan for up to `duration`
fn scan(host: &BtHost, duration: Duration, stop: &dyn Fn() -> bool) -> Vec<LeScanResult> {
    if !host.le_ready() {
        return Vec::new();
    }
    tracing::info!("LE Audio: scanning for {} ms", duration.as_millis());
    host.le_run(RUN_TIMEOUT, |ctx| ctx.start_scan());
    let end = Instant::now() + duration;
    while Instant::now() < end && !stop() {
        std::thread::sleep(SCAN_POLL.min(end.saturating_duration_since(Instant::now())));
    }
    host.le_run(RUN_TIMEOUT, |ctx| ctx.stop_scan()).unwrap_or_default()
}

/// Waits until the CSIS reads of device id finished; true if it has a SIRK
fn wait_member(host: &BtHost, id: LeDeviceId) -> bool {
    host.le_call(Duration::from_millis(READ_TIMEOUT_MS), move |ctx, done| {
        let Some(rc) = state(ctx) else {
            done(false);
            return;
        };
        let mut st = rc.borrow_mut();
        match st.members.get_mut(&id) {
            None => done(false),
            Some(m) if m.done => done(m.ok),
            Some(m) => m.waiters.push(done),
        }
    })
}

fn snapshot(host: &BtHost, id: LeDeviceId) -> Option<Snap> {
    host.le_run(RUN_TIMEOUT, move |ctx| {
        let addr = ctx.device(id)?.addr;
        let rc = state(ctx)?;
        let st = rc.borrow();
        let m = st.members.get(&id).filter(|m| m.ok)?;
        Some(Snap { id, set: m.set?, size: m.size, rank: m.rank, addr })
    })
    .flatten()
}

/// Connected, ready devices of a set
fn set_members(host: &BtHost, set: usize) -> Vec<Snap> {
    host.le_run(RUN_TIMEOUT, move |ctx| {
        let Some(rc) = state(ctx) else { return Vec::new() };
        let st = rc.borrow();
        ctx.device_ids()
            .into_iter()
            .filter_map(|id| {
                let d = ctx.device(id).filter(|d| d.ready)?;
                let m = st.members.get(&id).filter(|m| m.ok && m.set == Some(set))?;
                Some(Snap { id, set, size: m.size, rank: m.rank, addr: d.addr })
            })
            .collect()
    })
    .unwrap_or_default()
}

/// A member already in `have` (an RPA of a connected member resolves to its
/// identity, or is its current link address)
fn known(host: &BtHost, have: &[Snap], r: &LeScanResult) -> bool {
    let ids: Vec<LeDeviceId> = have.iter().map(|s| s.id).collect();
    let r = r.clone();
    host.le_run(RUN_TIMEOUT, move |ctx| ids.iter().any(|id| ctx.device(*id).is_some_and(|d| is_device(&r, d))))
        .unwrap_or(false)
}

/// Write Set Member Lock of member id; the ATT status, LOCK_TIMEOUT without
/// an answer
fn write_lock(host: &BtHost, id: LeDeviceId, value: u8) -> u8 {
    let status = Arc::new(AtomicU8::new(LOCK_TIMEOUT));
    let s = status.clone();
    tracing::debug!("CSIP: device {}: writing Set Member Lock {value}", id.0);
    host.le_call(Duration::from_millis(LOCK_WRITE_TIMEOUT_MS), move |ctx, done| {
        let Some(rc) = state(ctx) else {
            s.store(ATT_DISCONNECTED, Ordering::Release);
            done(true);
            return;
        };
        let target = rc.borrow().members.get(&id).map(|m| (m.con, m.lock_handle));
        let target = target.filter(|&(con, handle)| handle != 0 && ctx.device(id).is_some_and(|d| d.acl == con));
        let Some((con, handle)) = target else {
            s.store(ATT_DISCONNECTED, Ordering::Release);
            done(true);
            return;
        };
        ctx.gatt().write(
            con,
            handle,
            vec![value],
            Some(Box::new(move |_ctx, st| {
                s.store(st, Ordering::Release);
                let mut state = rc.borrow_mut();
                let mut now_locked = false;
                if let Some(m) = state.member(id, con) {
                    if value == LOCK_LOCKED && (st == 0 || st == ATT_ERROR_LOCK_ALREADY_GRANTED) {
                        m.we_locked = true;
                        m.lock = LOCK_LOCKED;
                        now_locked = true;
                    } else if value == LOCK_UNLOCKED && st == 0 {
                        m.we_locked = false;
                        m.lock = LOCK_UNLOCKED;
                    }
                }
                if now_locked {
                    state.locked.push((id, con));
                }
                done(true);
            })),
        );
    });
    let status = status.load(Ordering::Acquire);
    tracing::debug!("CSIP: device {}: Set Member Lock {value} -> 0x{status:02X} ({})", id.0, lock_error(status));
    status
}

/// Lock the members of a set (`have`: in rank order). Lock Denied: release,
/// back off, try again. Without the lock (not supported by every member, or
/// not granted) the stream is set up all the same.
fn lock_set(host: &BtHost, set: usize, have: &[Snap], cancelled: &dyn Fn() -> bool) {
    let ids: Vec<LeDeviceId> = have.iter().map(|s| s.id).collect();
    let handles = host
        .le_run(RUN_TIMEOUT, move |ctx| {
            let Some(rc) = state(ctx) else { return Vec::new() };
            let st = rc.borrow();
            ids.iter().map(|id| st.members.get(id).map_or(0, |m| m.lock_handle)).collect::<Vec<u16>>()
        })
        .unwrap_or_default();
    tracing::debug!("CSIP: coordinated set {set}: lock handles {handles:04X?}");
    if handles.len() != have.len() || handles.contains(&0) {
        tracing::info!(
            "CSIP: coordinated set {set}: not every member has a Set Member Lock: ordered access without locking"
        );
        return;
    }
    let mut attempt = 1;
    while attempt <= LOCK_ATTEMPTS && !cancelled() {
        let mut got: Vec<LeDeviceId> = Vec::new();
        let mut refused: Option<(&Snap, u8)> = None;
        for s in have {
            let st = write_lock(host, s.id, LOCK_LOCKED);
            if st == 0 || st == ATT_ERROR_LOCK_ALREADY_GRANTED {
                got.push(s.id);
                continue;
            }
            refused = Some((s, st));
            break;
        }
        let Some((refused, error)) = refused else {
            host.le_run(RUN_TIMEOUT, |ctx| {
                if let Some(rc) = state(ctx) {
                    start_lock_timer(ctx, &rc);
                }
            });
            tracing::info!("CSIP: coordinated set {set} locked ({} member(s), in rank order)", have.len());
            return;
        };
        // Release what was locked, highest rank first
        for &id in got.iter().rev() {
            write_lock(host, id, LOCK_UNLOCKED);
        }
        host.le_run(RUN_TIMEOUT, |ctx| {
            if let Some(rc) = state(ctx) {
                let mut st = rc.borrow_mut();
                let State { locked, members, .. } = &mut *st;
                locked.retain(|(id, con)| members.get(id).is_some_and(|m| m.con == *con && m.we_locked));
            }
        });
        if error != ATT_ERROR_LOCK_DENIED {
            tracing::warn!(
                "CSIP: coordinated set {set}: locking {} failed: {} (0x{error:02x}): streaming without the lock",
                refused.addr,
                lock_error(error)
            );
            return;
        }
        if attempt == LOCK_ATTEMPTS {
            break;
        }
        let wait = LOCK_BACKOFF_MS * attempt + jitter_ms();
        tracing::info!(
            "CSIP: coordinated set {set}: Lock Denied by {} (another coordinator holds it): released, trying again in \
             {wait} ms",
            refused.addr
        );
        let mut t = 0;
        while t < wait && !cancelled() {
            std::thread::sleep(Duration::from_millis(50));
            t += 50;
        }
        attempt += 1;
    }
    tracing::warn!(
        "CSIP: coordinated set {set}: lock not granted after {LOCK_ATTEMPTS} attempts: streaming without the lock"
    );
}

/// Up to 200 ms of randomness for the lock back-off (v1 GetTickCount() %
/// 200)
fn jitter_ms() -> u32 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.subsec_millis() % 200)
}

pub fn create(setup: &mut LeModuleSetup) -> LeModuleEntry {
    let shared = Arc::new(Shared {
        crypto_ok: AtomicBool::new(false),
        wanted_set: AtomicI64::new(-1),
        member_seen: AtomicBool::new(false),
        tried: Mutex::new(Vec::new()),
    });
    let file = setup.bond_dir.as_ref().map(|d| d.join(FILE_NAME));
    LeModuleEntry {
        module: Box::new(CsipModule { st: Rc::new(RefCell::new(State::new(file))), shared: shared.clone() }),
        hooks: Some(Arc::new(CsipHooks { shared })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CSIS 1.0.1 appendix A.1: SIRK and an RSI generated with it
    const SAMPLE_SIRK: Key =
        [0x45, 0x7d, 0x7d, 0x09, 0x21, 0xa1, 0xfd, 0x22, 0xce, 0xcd, 0x8c, 0x86, 0xdd, 0x72, 0xcc, 0xcd];
    const SAMPLE_RSI: [u8; 6] = [0xda, 0x48, 0x19, 0x63, 0xf5, 0x69];

    fn snap(id: u32, rank: u8) -> Snap {
        Snap { id: LeDeviceId(id), set: 0, size: 2, rank, addr: LeAddr::default() }
    }

    #[test]
    fn keys_from_the_air_and_hex() {
        let mut air: Vec<u8> = SAMPLE_SIRK.to_vec();
        air.reverse();
        assert_eq!(key_from_air(&air), SAMPLE_SIRK);
        assert_eq!(key_hex(&SAMPLE_SIRK), "457d7d0921a1fd22cecd8c86dd72cccd");
        assert_eq!(parse_key("457d7d0921a1fd22cecd8c86dd72cccd"), Some(SAMPLE_SIRK));
        assert_eq!(parse_key("457D7D0921A1FD22CECD8C86DD72CCCD"), Some(SAMPLE_SIRK));
        assert_eq!(parse_key("457d7d0921a1fd22cecd8c86dd72ccc"), None);
        assert_eq!(parse_key("457d7d0921a1fd22cecd8c86dd72cccd0"), None);
        assert_eq!(parse_key("x57d7d0921a1fd22cecd8c86dd72cccd"), None);
    }

    #[test]
    fn stored_file_round_trip() {
        let s = Stored {
            addr: LeAddr { addr: [0xC0, 0x01, 0x02, 0x03, 0x04, 0x05], addr_type: 1 },
            sirk: SAMPLE_SIRK,
            size: 2,
            rank: 1,
        };
        let text = format_stored(std::slice::from_ref(&s));
        assert_eq!(
            text,
            "# A2DPWB LE Audio: SIRK of bonded coordinated set members (identity address, type, SIRK, size, rank)\n\
             C0:01:02:03:04:05 1 457d7d0921a1fd22cecd8c86dd72cccd 2 1\n"
        );
        let back: Vec<Stored> = text.lines().filter_map(parse_stored_line).collect();
        assert_eq!(back, vec![s]);
        assert_eq!(parse_stored_line("C0:01:02:03:04:05 1 457d7d0921a1fd22cecd8c86dd72cccd 2"), None);
        assert_eq!(parse_stored_line("C0:01:02:03:04 1 457d7d0921a1fd22cecd8c86dd72cccd 2 1"), None);
        assert_eq!(parse_stored_line("C0:01:02:03:04:05 1 nothex 2 1"), None);
        assert_eq!(parse_stored_line(""), None);
    }

    #[test]
    fn members_are_ordered_by_rank() {
        let mut v = vec![snap(0, 2), snap(1, 0), snap(2, 1), snap(3, 0)];
        sort_by_rank(&mut v);
        let order: Vec<(u32, u8)> = v.iter().map(|s| (s.id.0, s.rank)).collect();
        assert_eq!(order, vec![(2, 1), (0, 2), (1, 0), (3, 0)]);
    }

    #[test]
    fn rsi_resolves_to_a_known_set() {
        let mut st = State::new(None);
        // No set yet: cached as unresolved
        assert_eq!(st.resolve_rsi(&SAMPLE_RSI), None);
        assert_eq!(st.rsi.get(&SAMPLE_RSI), Some(None));
        // Another set first, then the sample set: the unresolved RSI is
        // checked again
        assert_eq!(st.add_set(&[1; 16], 2), 0);
        assert_eq!(st.add_set(&SAMPLE_SIRK, 2), 1);
        assert_eq!(st.rsi.get(&SAMPLE_RSI), None);
        assert_eq!(st.resolve_rsi(&SAMPLE_RSI), Some(1));
        assert_eq!(st.rsi.get(&SAMPLE_RSI), Some(Some(1)));
        // The same SIRK again is the same set; the size grows
        assert_eq!(st.add_set(&SAMPLE_SIRK, 3), 1);
        assert_eq!(st.sets[1].size, 3);
        // A changed hash does not resolve
        let mut other = SAMPLE_RSI;
        other[0] ^= 1;
        assert_eq!(st.resolve_rsi(&other), None);
    }

    #[test]
    fn rsi_cache_is_bounded() {
        let mut c = RsiCache::default();
        for i in 0..(RSI_CACHE_MAX as u32 + 10) {
            let b = i.to_le_bytes();
            c.insert(&[b[0], b[1], b[2], b[3], 0, 0], if i % 2 == 0 { Some(0) } else { None });
        }
        assert_eq!(c.map.len(), RSI_CACHE_MAX);
        assert_eq!(c.get(&[0, 0, 0, 0, 0, 0]), None);
        assert_eq!(c.get(&[10, 0, 0, 0, 0, 0]), Some(Some(0)));
        c.forget_unresolved();
        assert_eq!(c.map.len(), RSI_CACHE_MAX / 2);
        assert_eq!(c.order.len(), RSI_CACHE_MAX / 2);
        c.clear();
        assert!(c.map.is_empty() && c.order.is_empty());
    }

    #[test]
    fn lock_errors_are_named() {
        assert_eq!(lock_error(0x80), "Lock Denied");
        assert_eq!(lock_error(0x81), "Lock Release Not Allowed");
        assert_eq!(lock_error(0x82), "Invalid Lock Value");
        assert_eq!(lock_error(0x84), "Lock Already Granted");
        assert_eq!(lock_error(0x1F), "disconnected");
        assert_eq!(lock_error(0xFF), "no answer");
        assert_eq!(lock_error(0x0E), "error");
    }

    #[test]
    fn advertisers_match_devices() {
        let mut d = LeDevice::new(LeDeviceId(0), LeAddr { addr: [1, 2, 3, 4, 5, 6], addr_type: 0 });
        d.peer_addr = LeAddr { addr: [0x40, 9, 9, 9, 9, 9], addr_type: 1 };
        let mut r = LeScanResult { addr: LeAddr { addr: [0x41, 7, 7, 7, 7, 7], addr_type: 1 }, ..Default::default() };
        assert!(!is_device(&r, &d));
        r.identity = Some(LeAddr { addr: [1, 2, 3, 4, 5, 6], addr_type: 0 });
        assert!(is_device(&r, &d));
        r.identity = None;
        r.addr = d.peer_addr;
        assert!(is_device(&r, &d));
        assert_eq!(scan_identity(&r), d.peer_addr);
    }
}
