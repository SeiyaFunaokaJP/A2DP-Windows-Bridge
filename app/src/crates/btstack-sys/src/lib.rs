//! BTstack for BlueCaster: the C library built from extern/btstack, and a safe
//! layer over the parts BlueCaster uses.
//!
//! BTstack is not thread-safe. Everything that calls into it takes a
//! [`Stack`], a token that only exists on the thread running the BTstack run
//! loop ([`run`]) and cannot leave it (`!Send`). Other threads talk to the
//! stack through a [`Remote`], which only wakes the run loop up; the
//! application ([`App`]) then picks up its work there.
//!
//! The HCI transport ([`HciBackend`]) and the link key store
//! ([`LinkKeyStore`]) are implemented in Rust (crate hci-transport, bt-host)
//! and plugged into BTstack through function tables in csrc/shim.c.
//!
//! # LE (experimental LE Audio)
//!
//! LE is compiled in but hidden from BTstack by default (see
//! [`Stack::set_le_hidden`]): with LE Audio off nothing changes for A2DP.
//! With it on, the app calls, in the init closure of [`run`]:
//! `set_le_hidden(false)` before `hci_init`, then [`Stack::le_init`] (bond
//! TLV file, SM, GATT client, ISO), builds the PC's GATT database
//! ([`Stack::att_db_init`], `att_db_add_*`) and starts the ATT server
//! ([`Stack::att_server_init`]). LE events reach [`App::on_packet`] with the
//! sources [`Source::Sm`], [`Source::GattClient`], [`Source::AttServer`] and
//! [`Source::Iso`] (HCI LE events, LE Meta GAP events and advertising
//! reports with [`Source::Hci`]); [`events`] reads them. Reads and writes of
//! the GATT database come to [`App::on_att_read`] / [`App::on_att_write`],
//! synchronously, also while the app is busy in one of its own calls (they
//! are then put off until it returns, see crate::att).

mod att;
mod callbacks;
pub mod consts;
pub mod events;
mod ffi;
mod le;
mod stack;
mod timer_resolution;
mod transport;

pub use att::AttWriteRequest;
pub use le::{CigParams, CisParams, ConnectionType, LeDbEntry, LeEncryption};
pub use stack::{AclState, AvdtpState, HciState, Stack};
pub use timer_resolution::HighTimerResolution;
pub use transport::{
    HciBackend, LE_HIDDEN_COMMAND_BITS, LE_SUPPORTED_FEATURE, Upstream, expect_transport_loss,
    le_hidden_command_complete, le_hidden_intercept, le_hidden_patch_event,
};

use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};

/// Where a packet handed to [`App::on_packet`] comes from
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// HCI events (BTstack's own events included)
    Hci,
    /// A2DP source events (HCI_EVENT_A2DP_META)
    A2dp,
    /// AVDTP events emitted to both roles, seen through the sink callback
    /// (SEP discovery)
    AvdtpSink,
    Avrcp,
    AvrcpController,
    AvrcpTarget,
    /// LE Security Manager events (SM_EVENT_*), after [`Stack::le_init`]
    Sm,
    /// GATT client query results and notifications / indications
    /// (GATT_EVENT_*)
    GattClient,
    /// ATT server events (ATT_EVENT_*: MTU exchange, indication complete)
    AttServer,
    /// The ISO handler: HCI_EVENT_CIS_CAN_SEND_NOW (event packets, channel =
    /// CIS handle) and incoming ISO data (HCI_ISO_DATA_PACKET)
    Iso,
    /// HFP audio gateway events (HCI_EVENT_HFP_META), after
    /// [`Stack::hfp_ag_init`]
    Hfp,
    /// RFCOMM client channels ([`Stack::rfcomm_create_channel`]): their
    /// events, and their data (RFCOMM_DATA_PACKET, channel = RFCOMM cid)
    Rfcomm,
    /// Results of [`Stack::sdp_query_rfcomm`]
    SdpClient,
}

impl Source {
    fn from_raw(raw: u8) -> Source {
        match raw {
            1 => Source::A2dp,
            2 => Source::AvdtpSink,
            3 => Source::Avrcp,
            4 => Source::AvrcpController,
            5 => Source::AvrcpTarget,
            6 => Source::Sm,
            7 => Source::GattClient,
            8 => Source::AttServer,
            9 => Source::Iso,
            10 => Source::Hfp,
            11 => Source::Rfcomm,
            12 => Source::SdpClient,
            _ => Source::Hci,
        }
    }
}

/// The application on the BTstack thread. All of its methods run there, one
/// at a time. Events BTstack emits while one of them runs (e.g. a failed
/// connection reported from within a disconnect call) are delivered right
/// after it returns.
pub trait App {
    fn on_packet(&mut self, stack: &Stack, source: Source, packet_type: u8, channel: u16, packet: &[u8]);
    /// [`Remote::wake`] was called (once per batch of wakes)
    fn on_wake(&mut self, stack: &Stack);
    /// A timer started with [`Stack::timer_start`] fired
    fn on_timer(&mut self, stack: &Stack, id: u32);
    /// The Intel bootloader firmware download started with
    /// [`Stack::intel_download_firmware`] finished (0 = success)
    fn on_intel_firmware(&mut self, _stack: &Stack, _result: i32) {}
    /// The run loop has stopped; HCI is still initialized
    fn on_exit(&mut self, _stack: &Stack) {}
    /// A GATT client reads attribute `handle` of the PC's GATT database (a
    /// characteristic added with [`consts::ATT_PROPERTY_DYNAMIC`], or its
    /// client configuration descriptor): its whole value, or an ATT error.
    /// btstack-sys sends the part from `offset` (Read Blob) that fits.
    /// Called synchronously from BTstack; see the crate docs.
    fn on_att_read(&mut self, _stack: &Stack, _con: u16, _handle: u16, _offset: u16) -> Result<Vec<u8>, u8> {
        Err(consts::ATT_ERROR_READ_NOT_PERMITTED)
    }
    /// A GATT client writes the PC's GATT database: 0 or an ATT error
    /// (ignored for write commands and the execute of prepared writes)
    fn on_att_write(&mut self, _stack: &Stack, _write: &AttWriteRequest) -> u8 {
        consts::ATT_ERROR_WRITE_NOT_PERMITTED
    }
}

/// Stores BR/EDR link keys for BTstack (addresses in BTstack's byte order,
/// most significant byte first)
pub trait LinkKeyStore {
    fn get(&mut self, addr: &[u8; 6]) -> Option<([u8; 16], i32)>;
    fn put(&mut self, addr: &[u8; 6], key: &[u8; 16], key_type: i32);
    fn delete(&mut self, addr: &[u8; 6]);
    /// Entry `index` in storage order, for BTstack's iterator
    fn at(&self, index: usize) -> Option<([u8; 6], [u8; 16], i32)>;
}

/// Receives every HCI packet BTstack sends or receives (`incoming`), e.g. to
/// write an HCI capture
pub type PacketLogger = Box<dyn FnMut(u8, bool, &[u8])>;

#[derive(Debug)]
pub enum RunError {
    AlreadyRunning,
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("a BTstack run loop is already running in this process")
    }
}
impl std::error::Error for RunError {}

// ---- Process-wide state (one BTstack per process) ----

/// A run loop exists ([`run`] between start and end)
static RUNNING: AtomicBool = AtomicBool::new(false);
/// Wakes may be posted to the run loop; with WAKERS, keeps a wake from
/// reaching a run loop that is being torn down
static ALIVE: AtomicBool = AtomicBool::new(false);
static WAKERS: AtomicUsize = AtomicUsize::new(0);
/// Why the run loop was woken up
static WAKE_FLAGS: AtomicU32 = AtomicU32::new(0);
const WAKE_APP: u32 = 1;
const WAKE_EXIT: u32 = 2;
/// Identifies the current run: transports of an earlier run cannot deliver
static GENERATION: AtomicU64 = AtomicU64::new(0);
/// A callback panicked; the run loop is being stopped
static POISONED: AtomicBool = AtomicBool::new(false);

thread_local! {
    static APP: RefCell<Option<Box<dyn App>>> = const { RefCell::new(None) };
    /// Events that arrived while the app was busy
    static PENDING: RefCell<VecDeque<callbacks::Pending>> = const { RefCell::new(VecDeque::new()) };
    static LOGGER: RefCell<Option<PacketLogger>> = const { RefCell::new(None) };
    static LINK_KEYS: RefCell<Option<Box<dyn LinkKeyStore>>> = const { RefCell::new(None) };
}

/// Wakes the run loop from any thread. Cheap and lock-free: usable from the
/// audio threads.
#[derive(Clone, Copy, Debug, Default)]
pub struct Remote(());

impl Remote {
    /// [`App::on_wake`] runs on the BTstack thread soon. Wakes before that
    /// are merged into one.
    pub fn wake(&self) {
        WAKE_FLAGS.fetch_or(WAKE_APP, Ordering::AcqRel);
        post_wake();
    }

    /// Stops the run loop: [`run`] returns after shutting BTstack down
    pub fn request_exit(&self) {
        WAKE_FLAGS.fetch_or(WAKE_EXIT, Ordering::AcqRel);
        post_wake();
    }

    /// Whether the run loop still runs
    pub fn alive(&self) -> bool {
        ALIVE.load(Ordering::Acquire)
    }
}

fn post_wake() {
    WAKERS.fetch_add(1, Ordering::AcqRel);
    if ALIVE.load(Ordering::Acquire) {
        // SAFETY: ALIVE is set only between bluecaster_runloop_init() and the
        // run loop's teardown, which waits for WAKERS to drop to zero first;
        // bluecaster_wake() is thread-safe (mutex-protected callback list).
        unsafe { ffi::bluecaster_wake() };
    }
    WAKERS.fetch_sub(1, Ordering::AcqRel);
}

/// Runs BTstack on the calling thread until [`Remote::request_exit`]:
/// memory pools and run loop are set up, `init` registers the transport,
/// profiles and handlers and returns the app; afterwards every profile is
/// deinitialized and the transport closed, so a later `run` starts from
/// scratch.
pub fn run<F>(init: F) -> Result<(), RunError>
where
    F: FnOnce(&Stack, Remote) -> Box<dyn App>,
{
    if RUNNING.swap(true, Ordering::AcqRel) {
        return Err(RunError::AlreadyRunning);
    }
    GENERATION.fetch_add(1, Ordering::AcqRel);
    WAKE_FLAGS.store(0, Ordering::Release);
    POISONED.store(false, Ordering::Release);
    transport::reset();
    att::reset();
    le::reset();

    // SAFETY: first BTstack calls of this run, on this thread; nothing else
    // uses BTstack (RUNNING).
    unsafe {
        ffi::btstack_memory_init();
        ffi::bluecaster_runloop_init();
        ffi::bluecaster_hci_dump_init();
    }
    ALIVE.store(true, Ordering::Release);

    let stack = Stack::new();
    let app = init(&stack, Remote(()));
    APP.with(|a| *a.borrow_mut() = Some(app));
    callbacks::drain_pending(&stack);

    // SAFETY: the run loop was initialized above; returns after
    // btstack_run_loop_trigger_exit() on this thread.
    unsafe { ffi::btstack_run_loop_execute() };

    callbacks::with_app(&stack, |app, s| app.on_exit(s));
    // No more wakes: wait for any in progress on other threads
    ALIVE.store(false, Ordering::Release);
    while WAKERS.load(Ordering::Acquire) != 0 {
        std::thread::yield_now();
    }
    stack.deinit();
    APP.with(|a| a.borrow_mut().take());
    PENDING.with(|p| p.borrow_mut().clear());
    LOGGER.with(|l| l.borrow_mut().take());
    LINK_KEYS.with(|l| l.borrow_mut().take());
    // SAFETY: all profiles and HCI are deinitialized (stack.deinit()), the
    // run loop no longer runs; this resets BTstack's global state.
    unsafe {
        ffi::bluecaster_timers_clear();
        ffi::btstack_memory_deinit();
        ffi::btstack_run_loop_deinit();
    }
    transport::reset();
    att::reset();
    le::reset();
    RUNNING.store(false, Ordering::Release);
    Ok(())
}

/// Whether a callback panicked during the last run (the run loop was stopped)
pub fn poisoned() -> bool {
    POISONED.load(Ordering::Acquire)
}

/// USB VID:PID pairs of the Realtek controllers BTstack's Realtek driver
/// knows (its firmware table). A static table: callable from any thread.
pub fn realtek_usb_ids() -> Vec<(u16, u16)> {
    realtek_usb_firmware().iter().map(|&(vid, pid, _)| (vid, pid)).collect()
}

/// BTstack's Realtek firmware table: VID, PID and the firmware name without
/// extension ("rtl8852bu_fw"), which tells the chip family. A static table:
/// callable from any thread.
pub fn realtek_usb_firmware() -> &'static [(u16, u16, &'static str)] {
    static TABLE: std::sync::OnceLock<Vec<(u16, u16, &'static str)>> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        // SAFETY: reads BTstack's constant firmware table; no state involved.
        let n = unsafe { ffi::btstack_chipset_realtek_get_num_usb_controllers() };
        (0..n)
            .map(|i| {
                let (mut vid, mut pid) = (0u16, 0u16);
                // SAFETY: i < n; both pointers are valid for writes.
                unsafe { ffi::btstack_chipset_realtek_get_vendor_product_id(i, &mut vid, &mut pid) };
                // SAFETY: i < n; the name is a string literal of the table
                // (static), or NULL
                let name = unsafe { ffi::bluecaster_realtek_usb_fw_name(i) };
                let name = if name.is_null() {
                    ""
                } else {
                    // SAFETY: a NUL-terminated literal that lives forever
                    unsafe { std::ffi::CStr::from_ptr(name) }.to_str().unwrap_or("")
                };
                (vid, pid, name)
            })
            .collect()
    })
}

#[cfg(feature = "sbc-reference")]
pub mod sbc_reference {
    //! The Bluedroid SBC encoder (C), for comparing the Rust port against it

    use crate::ffi;

    pub struct Encoder(*mut core::ffi::c_void);

    impl Encoder {
        /// channel_mode / allocation: Bluedroid's values (SBC_MONO 0 ..
        /// SBC_JOINT_STEREO 3; SBC_LOUDNESS 0, SBC_SNR 1)
        pub fn new(
            sample_rate: u16,
            channel_mode: i32,
            blocks: i32,
            subbands: i32,
            allocation: i32,
            bitpool: i32,
        ) -> Encoder {
            // SAFETY: plain values; returns an owned, heap-allocated state.
            let p = unsafe {
                ffi::bluecaster_sbc_ref_new(sample_rate, channel_mode, blocks, subbands, allocation, bitpool)
            };
            assert!(!p.is_null());
            Encoder(p)
        }

        /// One frame from `pcm` (blocks * subbands samples per channel,
        /// interleaved)
        pub fn encode(&mut self, pcm: &[i16]) -> Vec<u8> {
            let mut out = vec![0u8; 1024];
            // SAFETY: self.0 is a live state; the encoder reads blocks *
            // subbands * channels samples, which the caller provides; out has
            // out.len() bytes.
            let n = unsafe { ffi::bluecaster_sbc_ref_encode(self.0, pcm.as_ptr(), out.as_mut_ptr(), out.len() as i32) };
            assert!(n >= 0);
            out.truncate(n as usize);
            out
        }
    }

    impl Drop for Encoder {
        fn drop(&mut self) {
            // SAFETY: allocated by bluecaster_sbc_ref_new, freed once.
            unsafe { ffi::bluecaster_sbc_ref_free(self.0) };
        }
    }
}
