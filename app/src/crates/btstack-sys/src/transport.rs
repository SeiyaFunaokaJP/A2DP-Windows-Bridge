//! The HCI transport as BTstack sees it, implemented by a Rust backend
//! (crate hci-transport: WinUSB via nusb, H4 over TCP).
//!
//! BTstack calls open / close / send on its thread. The backend's I/O threads
//! hand received packets to an [`Upstream`], which queues them and wakes the
//! run loop; they reach BTstack there, with its pre-buffer in front.
//!
//! # LE hidden
//!
//! btstack_config.h compiles LE in (ENABLE_BLE, ...), and BTstack's HCI init
//! then sets up LE on every dual-mode controller: LE Read Buffer Size, Write
//! LE Host Supported, LE Set Event Mask, LE data length, LE Read White List
//! Size, LE Read Maximum Advertising Data Length, LE Set Host Feature, and the
//! Set Event Mask with the LE Meta event. Without LE Audio none of that is
//! wanted: a controller that never answers one of them stalls init, and Write
//! LE Host Supported makes the PC a dual-mode host to every headset. BTstack
//! gates each of those steps on the LE Supported (Controller) feature bit or
//! on a supported-commands bit, so clearing those bits in the controller's
//! answers, before BTstack sees them, gives the HCI init of a build without
//! LE (as v1's transport wrapper did).
//!
//! One LE command is sent anyway: hci_run() sends LE Set Scan Parameters once
//! HCI is working, with or without LE on the controller (ENABLE_LE_CENTRAL).
//! While hidden, LE commands are answered here as a controller without LE
//! would (Command Complete, Unknown HCI Command) and never reach the
//! controller. HCI captures (hci_dump, in hci.c) show the answers as BTstack
//! saw them, and the LE commands answered here.
//!
//! Hidden is the default of every run; [`crate::Stack::set_le_hidden`]
//! (before hci_init) turns it off when LE Audio is on.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};

use crate::consts::{
    ERROR_CODE_UNKNOWN_HCI_COMMAND, HCI_COMMAND_DATA_PACKET, HCI_EVENT_COMMAND_COMPLETE, HCI_EVENT_PACKET,
    HCI_OPCODE_READ_LOCAL_SUPPORTED_COMMANDS, HCI_OPCODE_READ_LOCAL_SUPPORTED_FEATURES,
    HCI_OPCODE_WRITE_LE_HOST_SUPPORTED, OGF_LE_CONTROLLER,
};
use crate::{GENERATION, WAKE_APP, WAKE_FLAGS, post_wake};

/// An HCI transport for BTstack
pub trait HciBackend: Send {
    /// Asynchronous backends return from [`send`](Self::send) before the
    /// packet is out and report it with [`Upstream::packet_sent`]; BTstack
    /// then asks [`can_send`](Self::can_send) before each packet.
    /// Synchronous ones (TCP) are done when `send` returns.
    fn is_async(&self) -> bool;
    /// Opens the device and starts receiving; packets go to `upstream`
    fn open(&mut self, upstream: Upstream) -> Result<(), String>;
    /// Stops receiving and releases the device. May be called more than
    /// once, and without a successful open.
    fn close(&mut self);
    /// Asynchronous backends: whether a packet of this type can be sent now
    fn can_send(&mut self, _packet_type: u8) -> bool {
        true
    }
    /// Sends one packet (H4 packet type, packet without type byte)
    fn send(&mut self, packet_type: u8, packet: &[u8]) -> bool;
}

/// The way back from a backend's I/O threads into BTstack. Belongs to one
/// run: after the stack it was opened for has stopped, it does nothing.
#[derive(Clone, Debug)]
pub struct Upstream {
    generation: u64,
}

struct RxQueue {
    packets: VecDeque<(u8, Vec<u8>)>,
    /// Buffers of delivered packets, reused
    free: Vec<Vec<u8>>,
}

static RX: Mutex<RxQueue> = Mutex::new(RxQueue { packets: VecDeque::new(), free: Vec::new() });
static SENT: AtomicU32 = AtomicU32::new(0);
static FAILED: AtomicBool = AtomicBool::new(false);
static LOSS_EXPECTED: AtomicBool = AtomicBool::new(false);
/// The controller's answer to Read Local Supported Commands of this run, as
/// it sent it: 0 not seen, 1 without, 2 with the isochronous commands
static CONTROLLER_ISO: AtomicU8 = AtomicU8::new(0);

thread_local! {
    static BACKEND: RefCell<Option<Box<dyn HciBackend>>> = const { RefCell::new(None) };
    static OPEN: Cell<bool> = const { Cell::new(false) };
    /// The controller's LE support is hidden from BTstack (module docs)
    static LE_HIDDEN: Cell<bool> = const { Cell::new(true) };
}

// ---- LE hidden: pure functions ----

/// The supported-commands bits (octet, bit) of the LE commands BTstack looks
/// up in hci.c (SUPPORTED_HCI_COMMANDS): Write LE Host Supported, LE Read
/// Remote Features, LE Write Suggested Default Data Length, LE Set Address
/// Resolution Enable, LE Read Maximum Data Length, LE Set Default PHY, LE Set
/// Extended Advertising Enable (checked without the LE feature bit), LE Read
/// Buffer Size v2, LE Set Host Feature, LE Read Minimum Supported Connection
/// Interval
pub const LE_HIDDEN_COMMAND_BITS: [(usize, u8); 10] =
    [(24, 6), (27, 5), (34, 0), (35, 1), (35, 3), (35, 5), (36, 5), (41, 5), (44, 1), (48, 7)];

/// LMP feature bit 38, LE Supported (Controller): octet 4, bit 6
pub const LE_SUPPORTED_FEATURE: (usize, u8) = (4, 6);

/// LE Read Buffer Size v2 in the supported commands: the controller has the
/// isochronous channels LE Audio streams over (Bluetooth 5.2)
pub const ISO_COMMAND_BIT: (usize, u8) = (41, 5);

/// Whether a successful Read Local Supported Commands answer lists the
/// isochronous commands; None for every other packet
pub fn iso_commands_in(packet: &[u8]) -> Option<bool> {
    if packet.len() < 6 + 64 || packet[0] != HCI_EVENT_COMMAND_COMPLETE || packet[5] != 0 {
        return None;
    }
    let opcode = u16::from_le_bytes([packet[3], packet[4]]);
    let (octet, bit) = ISO_COMMAND_BIT;
    (opcode == HCI_OPCODE_READ_LOCAL_SUPPORTED_COMMANDS).then(|| packet[6 + octet] & (1 << bit) != 0)
}

/// The opcode of a command that is answered locally while LE is hidden (an
/// LE controller command, OGF 0x08, or Write LE Host Supported), None for
/// every other packet
pub fn le_hidden_intercept(packet_type: u8, packet: &[u8]) -> Option<u16> {
    if packet_type != HCI_COMMAND_DATA_PACKET || packet.len() < 3 {
        return None;
    }
    let opcode = u16::from_le_bytes([packet[0], packet[1]]);
    (opcode >> 10 == OGF_LE_CONTROLLER || opcode == HCI_OPCODE_WRITE_LE_HOST_SUPPORTED).then_some(opcode)
}

/// The answer of a controller without LE to `opcode`: Command Complete, one
/// command allowed, Unknown HCI Command
pub fn le_hidden_command_complete(opcode: u16) -> [u8; 6] {
    let [lo, hi] = opcode.to_le_bytes();
    [HCI_EVENT_COMMAND_COMPLETE, 4, 1, lo, hi, ERROR_CODE_UNKNOWN_HCI_COMMAND]
}

/// Clears the controller's LE support in an event before BTstack sees it:
/// the LE feature bit in Read Local Supported Features, the LE command bits
/// in Read Local Supported Commands. Other events are left alone.
pub fn le_hidden_patch_event(packet: &mut [u8]) {
    // Command Complete: event code, length, num packets, opcode (2), status,
    // return parameters
    if packet.len() < 6 || packet[0] != HCI_EVENT_COMMAND_COMPLETE {
        return;
    }
    let opcode = u16::from_le_bytes([packet[3], packet[4]]);
    let params = &mut packet[6..];
    if opcode == HCI_OPCODE_READ_LOCAL_SUPPORTED_FEATURES && params.len() >= 8 {
        let (octet, bit) = LE_SUPPORTED_FEATURE;
        params[octet] &= !(1u8 << bit);
    } else if opcode == HCI_OPCODE_READ_LOCAL_SUPPORTED_COMMANDS && params.len() >= 64 {
        for (octet, bit) in LE_HIDDEN_COMMAND_BITS {
            params[octet] &= !(1u8 << bit);
        }
    }
}

/// See [`crate::Stack::set_le_hidden`]. False (and no change) once a backend
/// is installed.
pub(crate) fn set_le_hidden(hidden: bool) -> bool {
    if BACKEND.with(|b| b.borrow().is_some()) {
        return false;
    }
    LE_HIDDEN.with(|h| h.set(hidden));
    true
}

pub(crate) fn le_hidden() -> bool {
    LE_HIDDEN.with(|h| h.get())
}

/// See [`crate::Stack::controller_iso`]
pub(crate) fn controller_iso() -> Option<bool> {
    match CONTROLLER_ISO.load(Ordering::Acquire) {
        0 => None,
        v => Some(v == 2),
    }
}

impl Upstream {
    fn current(&self) -> bool {
        self.generation == GENERATION.load(Ordering::Acquire)
    }

    /// A packet from the controller (H4 packet type, packet without it)
    pub fn deliver(&self, packet_type: u8, packet: &[u8]) {
        if !self.current() {
            return;
        }
        {
            let mut q = RX.lock().unwrap_or_else(|e| e.into_inner());
            let mut buf = q.free.pop().unwrap_or_default();
            buf.clear();
            buf.extend_from_slice(packet);
            q.packets.push_back((packet_type, buf));
        }
        post_wake();
    }

    /// Asynchronous backends: the packet of the last `send` is out
    pub fn packet_sent(&self) {
        if !self.current() {
            return;
        }
        SENT.fetch_add(1, Ordering::AcqRel);
        post_wake();
    }

    /// The device stopped working for good (unplugged, I/O errors); see
    /// [`crate::Stack::transport_failed`]. The app is woken up.
    pub fn fail(&self, why: &str) {
        if !self.current() {
            return;
        }
        if !FAILED.swap(true, Ordering::AcqRel) {
            if loss_expected() {
                tracing::info!(target: "hci", "HCI transport stopped as expected: {why}");
            } else {
                tracing::error!(target: "hci", "HCI transport failed: {why}");
            }
        }
        WAKE_FLAGS.fetch_or(WAKE_APP, Ordering::AcqRel);
        post_wake();
    }
}

/// The adapter may go away now (the PC is going to sleep): a transport
/// failure is then reported as information, not as an error. Stays set,
/// also across runs, until cleared.
pub fn expect_transport_loss(expected: bool) {
    LOSS_EXPECTED.store(expected, Ordering::Release);
}

pub(crate) fn loss_expected() -> bool {
    LOSS_EXPECTED.load(Ordering::Acquire)
}

/// Clears what a previous run left
pub(crate) fn reset() {
    let mut q = RX.lock().unwrap_or_else(|e| e.into_inner());
    if !q.packets.is_empty() {
        tracing::debug!(target: "hci", "transport reset: {} queued packet(s) dropped", q.packets.len());
    }
    q.packets.clear();
    SENT.store(0, Ordering::Release);
    FAILED.store(false, Ordering::Release);
    CONTROLLER_ISO.store(0, Ordering::Release);
    LE_HIDDEN.with(|h| h.set(true));
}

pub(crate) fn failed() -> bool {
    FAILED.load(Ordering::Acquire)
}

pub(crate) fn install(backend: Box<dyn HciBackend>) -> bool {
    let is_async = backend.is_async();
    tracing::debug!(target: "hci", "transport installed (async {is_async})");
    BACKEND.with(|b| *b.borrow_mut() = Some(backend));
    is_async
}

pub(crate) fn is_async() -> bool {
    BACKEND.with(|b| b.borrow().as_ref().is_some_and(|b| b.is_async()))
}

pub(crate) fn open() -> bool {
    let upstream = Upstream { generation: GENERATION.load(Ordering::Acquire) };
    let result = BACKEND.with(|b| match b.borrow_mut().as_mut() {
        Some(backend) => backend.open(upstream),
        None => Err("no HCI backend installed".into()),
    });
    match result {
        Ok(()) => {
            tracing::debug!(target: "hci", "HCI transport opened");
            OPEN.with(|o| o.set(true));
            true
        }
        Err(e) => {
            tracing::error!(target: "hci", "cannot open the HCI transport: {e}");
            false
        }
    }
}

pub(crate) fn close() {
    tracing::debug!(target: "hci", "HCI transport closing (open {})", OPEN.with(|o| o.get()));
    OPEN.with(|o| o.set(false));
    BACKEND.with(|b| {
        if let Some(backend) = b.borrow_mut().as_mut() {
            backend.close();
        }
    });
}

/// Closes and drops the backend (end of a run)
pub(crate) fn uninstall() {
    close();
    BACKEND.with(|b| b.borrow_mut().take());
}

pub(crate) fn can_send(packet_type: u8) -> bool {
    BACKEND.with(|b| b.borrow_mut().as_mut().is_some_and(|b| b.can_send(packet_type)))
}

pub(crate) fn send(packet_type: u8, packet: &[u8]) -> bool {
    if le_hidden()
        && let Some(opcode) = le_hidden_intercept(packet_type, packet)
    {
        answer_locally(opcode);
        return true;
    }
    let ok = BACKEND.with(|b| b.borrow_mut().as_mut().is_some_and(|b| b.send(packet_type, packet)));
    tracing::trace!(target: "hci", "send type 0x{packet_type:02X}, {} bytes: {ok}", packet.len());
    ok
}

/// LE hidden: the command is not sent; from the run loop, as after any
/// command, BTstack gets "packet sent" (asynchronous backends: it holds its
/// packet buffer until then) and the Command Complete
fn answer_locally(opcode: u16) {
    tracing::debug!(target: "hci", "LE hidden: answering HCI command 0x{opcode:04x} locally");
    if is_async() {
        SENT.fetch_add(1, Ordering::AcqRel);
    }
    {
        let mut q = RX.lock().unwrap_or_else(|e| e.into_inner());
        let mut buf = q.free.pop().unwrap_or_default();
        buf.clear();
        buf.extend_from_slice(&le_hidden_command_complete(opcode));
        q.packets.push_back((HCI_EVENT_PACKET, buf));
    }
    post_wake();
}

/// On the run loop: hands queued completions and packets to BTstack
pub(crate) fn pump() {
    let sent = SENT.swap(0, Ordering::AcqRel);
    if sent > 0 {
        tracing::trace!(target: "hci", "{sent} packet(s) sent");
    }
    for _ in 0..sent {
        // SAFETY: on the run loop thread; the shim checks for a handler.
        unsafe { crate::ffi::bluecaster_transport_packet_sent() };
    }
    let hidden = le_hidden();
    loop {
        let next = RX.lock().unwrap_or_else(|e| e.into_inner()).packets.pop_front();
        let Some((packet_type, mut buf)) = next else { break };
        if packet_type == HCI_EVENT_PACKET {
            // As the controller sent it, before LE hidden clears its LE bits
            if let Some(iso) = iso_commands_in(&buf) {
                tracing::debug!(target: "hci", "supported commands: ISO {}", if iso { "yes" } else { "no" });
                CONTROLLER_ISO.store(if iso { 2 } else { 1 }, Ordering::Release);
            }
            if hidden {
                le_hidden_patch_event(&mut buf);
            }
        }
        tracing::trace!(target: "hci", "deliver type 0x{packet_type:02X}, {} bytes", buf.len());
        if buf.len() <= u16::MAX as usize {
            // SAFETY: on the run loop thread; buf is valid for its length;
            // the shim copies it and checks the size.
            let ok = unsafe { crate::ffi::bluecaster_transport_deliver(packet_type, buf.as_ptr(), buf.len() as u16) };
            if ok == 0 {
                tracing::warn!(target: "hci", "dropped an incoming HCI packet of {} bytes", buf.len());
            }
        }
        let mut q = RX.lock().unwrap_or_else(|e| e.into_inner());
        if q.free.len() < 64 {
            q.free.push(buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consts::{HCI_ACL_DATA_PACKET, HCI_ISO_DATA_PACKET};

    #[test]
    fn le_commands_are_intercepted() {
        // LE Set Scan Parameters (0x200B), LE Read Buffer Size (0x2002), LE
        // Set Event Mask (0x2001), LE Read Buffer Size v2 (0x2060)
        for opcode in [0x200Bu16, 0x2002, 0x2001, 0x2060, 0x2065, 0x23FF] {
            let [lo, hi] = opcode.to_le_bytes();
            assert_eq!(le_hidden_intercept(HCI_COMMAND_DATA_PACKET, &[lo, hi, 0]), Some(opcode), "{opcode:04x}");
        }
        // Write LE Host Supported 01 00
        assert_eq!(le_hidden_intercept(HCI_COMMAND_DATA_PACKET, &[0x6D, 0x0C, 2, 1, 0]), Some(0x0C6D));
    }

    #[test]
    fn other_packets_pass() {
        // HCI Reset, Read Local Supported Features / Commands, Set Event
        // Mask, Write Simple Pairing Mode, a vendor command, Disconnect
        for opcode in [0x0C03u16, 0x1003, 0x1002, 0x0C01, 0x0C56, 0xFC61, 0x0406, 0x0C6C, 0x0C6E] {
            let [lo, hi] = opcode.to_le_bytes();
            assert_eq!(le_hidden_intercept(HCI_COMMAND_DATA_PACKET, &[lo, hi, 0]), None, "{opcode:04x}");
        }
        // Data packets whose first bytes look like an LE opcode
        assert_eq!(le_hidden_intercept(HCI_ACL_DATA_PACKET, &[0x0B, 0x20, 4, 0, 1, 2, 3, 4]), None);
        assert_eq!(le_hidden_intercept(HCI_ISO_DATA_PACKET, &[0x0B, 0x20, 4, 0, 1, 2, 3, 4]), None);
        // Too short to be a command
        assert_eq!(le_hidden_intercept(HCI_COMMAND_DATA_PACKET, &[0x0B, 0x20]), None);
    }

    #[test]
    fn local_answer_is_unknown_command() {
        assert_eq!(le_hidden_command_complete(0x200B), [0x0E, 4, 1, 0x0B, 0x20, 0x01]);
        assert_eq!(le_hidden_command_complete(0x0C6D), [0x0E, 4, 1, 0x6D, 0x0C, 0x01]);
    }

    fn command_complete(opcode: u16, params: &[u8]) -> Vec<u8> {
        let [lo, hi] = opcode.to_le_bytes();
        let mut p = vec![0x0E, (4 + params.len()) as u8, 1, lo, hi, 0];
        p.extend_from_slice(params);
        p
    }

    #[test]
    fn le_feature_bit_cleared() {
        // A dual-mode controller: LMP features with bit 38 (octet 4 bit 6)
        let features = [0xFF, 0xFF, 0x8F, 0xFE, 0xDB, 0xFF, 0x5B, 0x87];
        let mut p = command_complete(0x1003, &features);
        le_hidden_patch_event(&mut p);
        let mut expected = features;
        expected[4] &= !0x40;
        assert_eq!(&p[6..], &expected);
        assert_eq!(p[10], 0x9B);
        // Header untouched
        assert_eq!(&p[..6], &[0x0E, 12, 1, 0x03, 0x10, 0]);
    }

    #[test]
    fn le_command_bits_cleared() {
        let mut p = command_complete(0x1002, &[0xFF; 64]);
        le_hidden_patch_event(&mut p);
        let commands = &p[6..];
        for (octet, byte) in commands.iter().enumerate() {
            let cleared: u8 =
                LE_HIDDEN_COMMAND_BITS.iter().filter(|(o, _)| *o == octet).fold(0, |m, (_, b)| m | (1 << b));
            assert_eq!(*byte, !cleared, "octet {octet}");
        }
        // Exactly the ten bits of v1
        assert_eq!(commands.iter().map(|b| b.count_zeros()).sum::<u32>(), 10);
        assert_eq!(commands[24], 0xBF);
        assert_eq!(commands[35], 0xD5);
        assert_eq!(commands[48], 0x7F);
    }

    #[test]
    fn iso_commands_from_supported_commands() {
        let mut commands = [0u8; 64];
        assert_eq!(iso_commands_in(&command_complete(0x1002, &commands)), Some(false));
        commands[41] = 1 << 5;
        assert_eq!(iso_commands_in(&command_complete(0x1002, &commands)), Some(true));
        // Other commands, a failed answer, a short one
        assert_eq!(iso_commands_in(&command_complete(0x1003, &[0xFF; 64])), None);
        let mut failed = command_complete(0x1002, &[0xFF; 64]);
        failed[5] = 1;
        assert_eq!(iso_commands_in(&failed), None);
        assert_eq!(iso_commands_in(&command_complete(0x1002, &[0xFF; 8])), None);
    }

    #[test]
    fn other_events_untouched() {
        // Read Local Version, Read BD_ADDR: same layout, other opcodes
        for opcode in [0x1001u16, 0x1009, 0x1004] {
            let mut p = command_complete(opcode, &[0xFF; 64]);
            let before = p.clone();
            le_hidden_patch_event(&mut p);
            assert_eq!(p, before);
        }
        // Command Status, a Connection Complete, short packets
        for packet in [vec![0x0F, 4, 0, 1, 0x03, 0x10], vec![0x03, 11, 0, 1, 0, 1, 2, 3, 4, 5, 6, 1, 0], vec![0x0E, 4]]
        {
            let mut p = packet.clone();
            le_hidden_patch_event(&mut p);
            assert_eq!(p, packet);
        }
        // A Read Local Supported Features that failed (no features): left alone
        let mut p = vec![0x0E, 4, 1, 0x03, 0x10, 0x01];
        le_hidden_patch_event(&mut p);
        assert_eq!(p, vec![0x0E, 4, 1, 0x03, 0x10, 0x01]);
    }
}
