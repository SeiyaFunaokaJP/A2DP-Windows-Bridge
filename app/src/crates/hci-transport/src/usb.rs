//! HCI over USB ("H2") on a controller bound to WinUSB, through `nusb`.
//!
//! Interface 0 carries HCI: commands on the default control pipe (class
//! request to the device), events on the interrupt IN pipe, ACL data on the
//! bulk pipes. SCO (interface 1) is not used by an A2DP source. HCI ISO data
//! (LE Audio) shares the bulk pipes with ACL, as in v1 and Linux btusb: out
//! through the ACL slot; in, the ACL reader delivers it as ACL and BTstack
//! tells the two apart by the connection handle (a CIS handle).
//!
//! Recovery kept from v1's modified WinUSB transport:
//! - the event and ACL pipes are reset (clear halt) when the adapter is
//!   opened, so a halted endpoint or data left from a previous session does
//!   not keep the controller from answering HCI Reset
//! - a failed event read is retried (clear halt + new read), up to
//!   EVENT_IN_MAX_ERRORS in a row, instead of ending event delivery
//! - HCI commands time out (COMMAND_TIMEOUT), so a controller that stops
//!   accepting them does not block the stack
//! - close sends HCI Reset (bounded) and cancels everything, leaving the
//!   controller idle for the next session
//! - close waits for that reset's Command Complete (CLOSE_RESET_WAIT), so
//!   the next session's first command does not reach a controller that is
//!   still resetting: an AX211 that got HCI Reset while one from close was
//!   in progress answered one of the two and then nothing at all, until
//!   Windows restarted
//! - a failed ACL read is retried the same way (ACL_IN_MAX_ERRORS)
//! - ACL / ISO writes have no timeout, and a failed one only drops the packet
//!   (a stalled pipe also drops the writes behind it and is cleared)
//! - a transfer that fails for good (device gone, a lost command) is
//!   reported to BTstack's owner through [`Upstream::fail`]: BTstack itself
//!   cannot recover, it waits for the answer to a command that was lost
//! - an adapter that cannot be opened (another program holds it) is skipped
//!   for the next one the selector takes
//!
//! An Intel controller in its bootloader takes Secure Send (0xFC09) on the
//! bulk OUT pipe (the firmware loader sends it through the ACL slot) and
//! answers on the bulk IN pipe, as Linux btusb handles it: while
//! [`HciBackend::set_bootloader`] is on, the ACL reader delivers what it
//! reads as events, and close sends no HCI Reset: an AX211 bootloader
//! answered one with Unknown HCI Command and no command after it.
//!
//! BTstack treats this transport as asynchronous: `send` hands the packet to
//! a writer thread (one for commands, one for ACL / ISO) and returns. The
//! command writer reports each command as sent once it is out. ACL / ISO
//! packets are reported sent as soon as `send` has copied them, so BTstack
//! hands over the next one (the next fragment of a media packet) at once,
//! and the ACL writer keeps several bulk transfers in flight. v1 had one
//! ACL write in flight, finished on the run loop thread; v2 first waited for
//! each write on the writer thread and then for the run loop to pick up its
//! completion, so every ACL fragment took two thread hand-overs and the
//! pipe sat idle in between. The controller's ACL / ISO buffer count, which
//! BTstack keeps to, bounds how many packets are queued.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::thread::{JoinHandle, Thread};
use std::time::{Duration, Instant};

use btstack_sys::consts::{HCI_ACL_DATA_PACKET, HCI_COMMAND_DATA_PACKET, HCI_EVENT_PACKET, HCI_ISO_DATA_PACKET};
use btstack_sys::{ControlData, ControlKind, ControlRecipient, ControlRequest, HciBackend, Upstream, UsbDescriptors};
use nusb::descriptors::TransferType;
use nusb::transfer::{Buffer, Bulk, ControlIn, ControlOut, ControlType, In, Interrupt, Out, Recipient, TransferError};
use nusb::{Device, DeviceInfo, Endpoint, Interface, MaybeFuture};

use crate::reassembly::Reassembler;

/// HCI command control transfer timeout. A working controller accepts
/// commands at once, firmware download chunks included.
const COMMAND_TIMEOUT: Duration = Duration::from_millis(2000);
/// HCI Reset sent on close
const CLOSE_RESET_TIMEOUT: Duration = Duration::from_millis(500);
/// Waiting for the Command Complete of the HCI Reset sent on close (an
/// AX211 took 40 to 110 ms)
const CLOSE_RESET_WAIT: Duration = Duration::from_millis(1000);
/// Consecutive failed event reads before event delivery is given up
const EVENT_IN_MAX_ERRORS: u32 = 10;
/// Consecutive failed ACL reads before ACL delivery is given up
const ACL_IN_MAX_ERRORS: u32 = 10;
/// How often the I/O threads look at the stop flag
const POLL: Duration = Duration::from_millis(100);
/// Transfers kept in flight per IN pipe
const IN_TRANSFERS: usize = 3;
const EVENT_TRANSFER_SIZE: usize = 512;
const ACL_TRANSFER_SIZE: usize = 4096;
/// Bulk OUT transfers kept in flight
const ACL_OUT_TRANSFERS: usize = 8;

/// The writer slot of an outgoing packet: 0 commands (control pipe), 1 ACL
/// and ISO data (bulk OUT pipe). BTstack sends one command at a time; ACL
/// and ISO packets as the controller's buffers allow.
fn out_slot(packet_type: u8) -> Option<usize> {
    match packet_type {
        HCI_COMMAND_DATA_PACKET => Some(0),
        HCI_ACL_DATA_PACKET | HCI_ISO_DATA_PACKET => Some(1),
        _ => None,
    }
}

/// VID and PID of the adapter opened last (what a stack runs on)
static LAST_OPENED: Mutex<Option<(u16, u16)>> = Mutex::new(None);

/// ACL / ISO OUT writes since [`take_acl_out_timing`] last ran: the
/// longest from submit to completion (µs) and the most in flight at once
static ACL_OUT_LATENCY_US: AtomicU64 = AtomicU64::new(0);
static ACL_OUT_PENDING_MAX: AtomicU64 = AtomicU64::new(0);

/// How often the event reader logs the transfer counters (debug)
const STATS_EVERY: Duration = Duration::from_secs(10);

/// Transfer counters since the last debug summary
struct Stats {
    events_in: AtomicU64,
    events_in_bytes: AtomicU64,
    acl_in: AtomicU64,
    acl_in_bytes: AtomicU64,
    commands_out: AtomicU64,
    acl_out: AtomicU64,
    acl_out_bytes: AtomicU64,
    in_errors: AtomicU64,
    out_errors: AtomicU64,
    acl_out_pending_max: AtomicU64,
}

static STATS: Stats = Stats {
    events_in: AtomicU64::new(0),
    events_in_bytes: AtomicU64::new(0),
    acl_in: AtomicU64::new(0),
    acl_in_bytes: AtomicU64::new(0),
    commands_out: AtomicU64::new(0),
    acl_out: AtomicU64::new(0),
    acl_out_bytes: AtomicU64::new(0),
    in_errors: AtomicU64::new(0),
    out_errors: AtomicU64::new(0),
    acl_out_pending_max: AtomicU64::new(0),
};

impl Stats {
    fn add(counter: &AtomicU64, n: usize) {
        counter.fetch_add(n as u64, Ordering::Relaxed);
    }

    /// The counters, starting them again
    fn take(&self) -> [u64; 10] {
        [
            &self.events_in,
            &self.events_in_bytes,
            &self.acl_in,
            &self.acl_in_bytes,
            &self.commands_out,
            &self.acl_out,
            &self.acl_out_bytes,
            &self.in_errors,
            &self.out_errors,
            &self.acl_out_pending_max,
        ]
        .map(|c| c.swap(0, Ordering::Relaxed))
    }

    /// Logs the counters (debug) and starts them again
    fn log(&self, period: Duration) {
        let s = self.take();
        tracing::debug!(
            "USB: last {:.1} s: events in {} ({} B), ACL in {} ({} B), commands out {}, ACL/ISO out {} ({} B), \
             errors in {} out {}, ACL out max pending {}",
            period.as_secs_f32(),
            s[0],
            s[1],
            s[2],
            s[3],
            s[4],
            s[5],
            s[6],
            s[7],
            s[8],
            s[9]
        );
    }
}

/// The longest ACL / ISO OUT write (submit to completion) and the most
/// writes in flight since the last call (core's stream health log): how
/// long the controller held data back over USB
pub fn take_acl_out_timing() -> (Duration, usize) {
    let latency = ACL_OUT_LATENCY_US.swap(0, Ordering::Relaxed);
    let pending = ACL_OUT_PENDING_MAX.swap(0, Ordering::Relaxed);
    (Duration::from_micros(latency), pending as usize)
}

/// VID and PID of the adapter a [`UsbTransport`] opened last; None before
/// the first open
pub fn last_opened() -> Option<(u16, u16)> {
    *LAST_OPENED.lock().unwrap_or_else(|e| e.into_inner())
}

/// Which adapter to open
#[derive(Debug, Clone, Default)]
pub struct UsbSelector {
    /// Windows device instance ID (exact adapter), if chosen
    pub instance_id: Option<String>,
    /// VID:PID, if chosen
    pub vid_pid: Option<(u16, u16)>,
    /// VID:PID of controllers that are Bluetooth adapters even without the
    /// Bluetooth class code (vendor class FF/01/01, e.g. Realtek tables)
    pub known: Vec<(u16, u16)>,
    /// Instance ID of the adapter tried first when several are on WinUSB:
    /// the one the chipset setup (Realtek firmware) was made for
    pub preferred: Option<String>,
}

impl UsbSelector {
    /// The adapter named on the command line (`-u`): a Windows device
    /// instance ID (`USB\VID_0BDA&PID_8771\00E04C000001`), a device
    /// interface path as v1 and SetupAPI list them
    /// (`\\?\usb#vid_0bda&pid_8771#00e04c000001#{a5dcbf10-...}`), or a
    /// VID:PID (`0bda:8771`, the first such adapter)
    pub fn from_arg(arg: &str) -> Result<UsbSelector, String> {
        let arg = arg.trim();
        let bad = || {
            format!(
                "'{arg}' is not a USB device: use its instance ID (USB\\VID_xxxx&PID_xxxx\\...), \
                 device path (\\\\?\\usb#vid_xxxx&pid_xxxx#...) or VID:PID"
            )
        };
        if let Some(vp) = parse_vid_pid_pair(arg) {
            return Ok(UsbSelector { vid_pid: Some(vp), ..Default::default() });
        }
        let id = if arg.starts_with(r"\\?\") || arg.starts_with(r"\\.\") {
            instance_id_from_device_path(arg).ok_or_else(bad)?
        } else {
            arg.to_string()
        };
        let mut parts = id.split('\\');
        let (Some(bus), Some(hw), Some(serial), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else {
            return Err(bad());
        };
        if !bus.eq_ignore_ascii_case("usb") || !hw.to_ascii_uppercase().starts_with("VID_") || serial.is_empty() {
            return Err(bad());
        }
        Ok(UsbSelector { instance_id: Some(id), ..Default::default() })
    }

    /// The selector picks this device: the named adapter, else any
    /// Bluetooth-class or known one
    fn takes(&self, c: &UsbCandidate) -> bool {
        if let Some(id) = &self.instance_id {
            c.instance_id.eq_ignore_ascii_case(id)
        } else if let Some(vp) = self.vid_pid {
            (c.vid, c.pid) == vp
        } else {
            c.bluetooth_class || c.known
        }
    }
}

/// `0bda:8771` (hex VID:PID)
fn parse_vid_pid_pair(s: &str) -> Option<(u16, u16)> {
    let (v, p) = s.split_once(':')?;
    if v.len() != 4 || p.len() != 4 {
        return None;
    }
    Some((u16::from_str_radix(v, 16).ok()?, u16::from_str_radix(p, 16).ok()?))
}

/// The device instance ID in a device interface path:
/// `\\?\usb#vid_0bda&pid_8771#00e04c000001#{guid}` is the interface of
/// `usb\vid_0bda&pid_8771\00e04c000001` (instance IDs compare without case)
pub fn instance_id_from_device_path(path: &str) -> Option<String> {
    let rest = path.strip_prefix(r"\\?\").or_else(|| path.strip_prefix(r"\\.\"))?;
    // The interface class GUID (and an optional reference string) follows
    // the last '#'
    let (id, guid) = rest.rsplit_once('#')?;
    if !guid.starts_with('{') || id.split('#').count() != 3 {
        return None;
    }
    Some(id.replace('#', "\\"))
}

/// A USB device that could be a Bluetooth controller
#[derive(Debug, Clone)]
pub struct UsbCandidate {
    pub vid: u16,
    pub pid: u16,
    pub instance_id: String,
    /// Driver service ("WinUSB", "BTHUSB", ...)
    pub driver: Option<String>,
    pub product: Option<String>,
    /// Bluetooth class code (E0/01/01) on the device or interface 0
    pub bluetooth_class: bool,
    /// In the selector's list of known controllers
    pub known: bool,
}

impl UsbCandidate {
    pub fn winusb(&self) -> bool {
        self.driver.as_deref().is_some_and(|d| d.eq_ignore_ascii_case("winusb"))
    }
}

fn is_bluetooth_class(class: u8, subclass: u8, protocol: u8) -> bool {
    class == 0xE0 && subclass == 0x01 && protocol == 0x01
}

/// VMware's virtual Bluetooth adapter (v1 skipped it as well)
fn is_vmware(info: &DeviceInfo) -> bool {
    info.vendor_id() == 0x0E0F
}

fn candidate(info: &DeviceInfo, known: &[(u16, u16)]) -> UsbCandidate {
    let bluetooth_class = is_bluetooth_class(info.class(), info.subclass(), info.protocol())
        || info
            .interfaces()
            .any(|i| i.interface_number() == 0 && is_bluetooth_class(i.class(), i.subclass(), i.protocol()));
    UsbCandidate {
        vid: info.vendor_id(),
        pid: info.product_id(),
        instance_id: info.instance_id().to_string_lossy().into_owned(),
        driver: info.driver().map(str::to_owned),
        product: info.product_string().map(str::to_owned),
        bluetooth_class,
        known: known.contains(&(info.vendor_id(), info.product_id())),
    }
}

/// Bluetooth controllers on USB (any driver), Bluetooth class or known VID:PID
pub fn list_candidates(known: &[(u16, u16)]) -> Vec<UsbCandidate> {
    let devices = match nusb::list_devices().wait() {
        Ok(d) => d,
        Err(e) => {
            tracing::debug!("USB: listing devices failed: {e}");
            return Vec::new();
        }
    };
    let list: Vec<UsbCandidate> = devices
        .filter(|d| !is_vmware(d))
        .map(|d| candidate(&d, known))
        .filter(|c| c.bluetooth_class || c.known)
        .collect();
    tracing::debug!("USB: {} candidate controller(s)", list.len());
    list
}

struct Opened {
    /// Kept for the control pipe (commands) and the endpoints' lifetime
    interface: Interface,
    /// The event pipe, read again on close for the reset's answer
    event_addr: u8,
    /// Its descriptors for a chip report
    device: Device,
    stop: Arc<AtomicBool>,
    /// The command writer; buffers of sent commands come back for reuse
    command_tx: mpsc::Sender<(u8, Vec<u8>)>,
    command_recycle_rx: mpsc::Receiver<Vec<u8>>,
    /// A command is with the writer (BTstack: one at a time)
    command_in_flight: Arc<AtomicBool>,
    /// The ACL / ISO writer, woken by unpark; transfer buffers come back
    acl_tx: mpsc::Sender<Buffer>,
    acl_recycle_rx: mpsc::Receiver<Buffer>,
    acl_writer: Thread,
    upstream: Upstream,
    /// The controller runs its bootloader: events on the bulk IN pipe, no
    /// HCI Reset on close
    bootloader: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
}

pub struct UsbTransport {
    selector: UsbSelector,
    opened: Option<Opened>,
}

impl UsbTransport {
    pub fn new(selector: UsbSelector) -> UsbTransport {
        UsbTransport { selector, opened: None }
    }

    /// The WinUSB adapters the selector takes, in enumeration order
    fn find(&self) -> Result<Vec<DeviceInfo>, String> {
        let devices: Vec<DeviceInfo> = nusb::list_devices()
            .wait()
            .map_err(|e| format!("cannot list USB devices: {e}"))?
            .filter(|d| !is_vmware(d))
            .collect();
        let mut found = Vec::new();
        let mut wrong_driver = None;
        for info in devices {
            let c = candidate(&info, &self.selector.known);
            let takes = self.selector.takes(&c);
            tracing::debug!(
                "USB: {:04X}:{:04X} {} driver {} bluetooth class {} known {} product {:?}: {}",
                c.vid,
                c.pid,
                c.instance_id,
                c.driver.as_deref().unwrap_or("(none)"),
                c.bluetooth_class,
                c.known,
                c.product,
                match (takes, c.winusb()) {
                    (false, _) => "not selected",
                    (true, true) => "selected",
                    (true, false) => "selected, not on WinUSB",
                }
            );
            if !takes {
                continue;
            }
            if c.winusb() {
                found.push(info);
            } else {
                wrong_driver.get_or_insert(c);
            }
        }
        if !found.is_empty() {
            if let Some(p) = &self.selector.preferred {
                // Stable: the others keep their enumeration order
                found.sort_by_key(|info| !info.instance_id().to_string_lossy().eq_ignore_ascii_case(p));
            }
            return Ok(found);
        }
        Err(match wrong_driver {
            Some(c) => format!(
                "adapter {:04X}:{:04X} uses driver {} instead of WinUSB",
                c.vid,
                c.pid,
                c.driver.as_deref().unwrap_or("(none)")
            ),
            None => "no Bluetooth adapter with the WinUSB driver found".into(),
        })
    }
}

/// Endpoint addresses of the HCI pipes on interface 0
fn hci_endpoints(interface: &Interface) -> Option<(u8, u8, u8)> {
    let desc = interface.descriptor()?;
    let (mut event_in, mut acl_in, mut acl_out) = (None, None, None);
    for ep in desc.endpoints() {
        let addr = ep.address();
        match (ep.transfer_type(), addr & 0x80 != 0) {
            (TransferType::Interrupt, true) => event_in = event_in.or(Some(addr)),
            (TransferType::Bulk, true) => acl_in = acl_in.or(Some(addr)),
            (TransferType::Bulk, false) => acl_out = acl_out.or(Some(addr)),
            _ => {}
        }
    }
    Some((event_in?, acl_in?, acl_out?))
}

/// The device's descriptors as the debug log shows them: speed, class and
/// each interface's alternate settings with their endpoints (interface 1,
/// SCO, has its isochronous alternate settings there)
fn log_layout(device: &Device) {
    if !tracing::enabled!(tracing::Level::DEBUG) {
        return;
    }
    let d = device.device_descriptor();
    tracing::debug!(
        "USB: device USB {:04X}, class {:02X}/{:02X}/{:02X}, ep0 max packet {}, version {:04X}, speed {:?}, {} configuration(s)",
        d.usb_version(),
        d.class(),
        d.subclass(),
        d.protocol(),
        d.max_packet_size_0(),
        d.device_version(),
        device.speed(),
        d.num_configurations()
    );
    let config = match device.active_configuration() {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!("USB: no active configuration: {e}");
            return;
        }
    };
    tracing::debug!(
        "USB: configuration {}, {} interface(s), attributes 0x{:02X}, max power {}",
        config.configuration_value(),
        config.num_interfaces(),
        config.attributes(),
        config.max_power()
    );
    for alt in config.interface_alt_settings() {
        tracing::debug!(
            "USB: interface {} alt {}: class {:02X}/{:02X}/{:02X}, {} endpoint(s)",
            alt.interface_number(),
            alt.alternate_setting(),
            alt.class(),
            alt.subclass(),
            alt.protocol(),
            alt.num_endpoints()
        );
        for ep in alt.endpoints() {
            tracing::debug!(
                "USB:   endpoint 0x{:02x} {:?} {:?}, max packet {} ({} per microframe), interval {}",
                ep.address(),
                ep.transfer_type(),
                ep.direction(),
                ep.max_packet_size(),
                ep.packets_per_microframe(),
                ep.interval()
            );
        }
    }
}

fn send_command(interface: &Interface, command: &[u8], timeout: Duration) -> Result<(), TransferError> {
    interface
        .control_out(
            ControlOut {
                control_type: ControlType::Class,
                recipient: Recipient::Device,
                request: 0,
                value: 0,
                index: 0,
                data: command,
            },
            timeout,
        )
        .wait()
}

/// Errors after which the device handle no longer reaches the adapter, as
/// v1's usb_device_gone: ERROR_DEVICE_NOT_CONNECTED, ERROR_NO_SUCH_DEVICE
/// and ERROR_FILE_NOT_FOUND (nusb's Disconnected), ERROR_BAD_COMMAND (22)
/// and ERROR_INVALID_HANDLE (6)
fn device_gone(e: &TransferError) -> bool {
    matches!(e, TransferError::Disconnected | TransferError::Unknown(22) | TransferError::Unknown(6))
}

/// The descriptors Windows keeps for the device (no I/O), with one line per
/// interface setting
fn descriptors(device: &Device) -> UsbDescriptors {
    let mut summary = Vec::new();
    let configurations = device
        .configurations()
        .map(|c| {
            for alt in c.interface_alt_settings() {
                let endpoints: Vec<String> = alt
                    .endpoints()
                    .map(|e| {
                        let kind = match e.transfer_type() {
                            TransferType::Control => "control",
                            TransferType::Isochronous => "isochronous",
                            TransferType::Bulk => "bulk",
                            TransferType::Interrupt => "interrupt",
                        };
                        format!("0x{:02X} {kind} {}", e.address(), e.max_packet_size())
                    })
                    .collect();
                summary.push(format!(
                    "interface {} alt {} class {:02X}/{:02X}/{:02X}: {}",
                    alt.interface_number(),
                    alt.alternate_setting(),
                    alt.class(),
                    alt.subclass(),
                    alt.protocol(),
                    if endpoints.is_empty() { "no endpoints".to_owned() } else { endpoints.join(", ") }
                ));
            }
            c.as_bytes().to_vec()
        })
        .collect();
    UsbDescriptors { device: device.device_descriptor().as_bytes().to_vec(), configurations, summary }
}

impl HciBackend for UsbTransport {
    fn is_async(&self) -> bool {
        true
    }

    fn open(&mut self, upstream: Upstream) -> Result<(), String> {
        self.close();
        // The first adapter that opens: one another program holds is
        // skipped, as v1's usb_open tried every device
        let mut last_error = String::new();
        let mut opened = None;
        tracing::debug!("USB: opening an adapter ({:?})", self.selector);
        for info in self.find()? {
            let (vid, pid) = (info.vendor_id(), info.product_id());
            tracing::debug!(
                "USB: trying {vid:04X}:{pid:04X} {} (bus {:?}, port chain {:?}, driver {:?})",
                info.instance_id().to_string_lossy(),
                info.bus_id(),
                info.port_chain(),
                info.driver()
            );
            let attempt =
                info.open().wait().map_err(|e| format!("cannot open {vid:04X}:{pid:04X}: {e}")).and_then(|device| {
                    log_layout(&device);
                    let interface = device
                        .claim_interface(0)
                        .wait()
                        .map_err(|e| {
                            // Seen right after WinUSB was assigned while
                            // Windows still had the old driver loaded (issue
                            // #81: "failed to initialize WinUSB (error 50)")
                            format!(
                                "cannot claim interface 0 of {vid:04X}:{pid:04X}: {e} (if WinUSB was just assigned: replug the adapter or restart Windows)"
                            )
                        })?;
                    let endpoints = hci_endpoints(&interface)
                        .ok_or_else(|| format!("{vid:04X}:{pid:04X} lacks the HCI event / ACL endpoints"))?;
                    Ok((device, interface, endpoints))
                });
            match attempt {
                Ok(o) => {
                    opened = Some((info, o));
                    break;
                }
                Err(e) => {
                    tracing::warn!(target: "hci", "USB: {e}; trying the next adapter, if any");
                    last_error = e;
                }
            }
        }
        let Some((info, (device, interface, (event_addr, acl_in_addr, acl_out_addr)))) = opened else {
            return Err(last_error);
        };
        let (vid, pid) = (info.vendor_id(), info.product_id());
        *LAST_OPENED.lock().unwrap_or_else(|e| e.into_inner()) = Some((vid, pid));
        let mut event_in = interface.endpoint::<Interrupt, In>(event_addr).map_err(|e| e.to_string())?;
        let mut acl_in = interface.endpoint::<Bulk, In>(acl_in_addr).map_err(|e| e.to_string())?;
        let mut acl_out = interface.endpoint::<Bulk, Out>(acl_out_addr).map_err(|e| e.to_string())?;
        tracing::debug!(
            "USB: interface 0 alt {}: event in 0x{event_addr:02x} (max packet {}), ACL in 0x{acl_in_addr:02x} (max packet {}), \
             ACL out 0x{acl_out_addr:02x} (max packet {}); pipe policies: nusb's (RAW_IO on IN pipes, no control pipe timeout)",
            interface.get_alt_setting(),
            event_in.max_packet_size(),
            acl_in.max_packet_size(),
            acl_out.max_packet_size()
        );
        // A halted endpoint or a data toggle left from an earlier session
        // keeps the controller from answering HCI Reset
        for (name, r) in [
            ("event", event_in.clear_halt().wait()),
            ("ACL in", acl_in.clear_halt().wait()),
            ("ACL out", acl_out.clear_halt().wait()),
        ] {
            if let Err(e) = r {
                tracing::warn!(target: "hci", "USB: clearing the {name} pipe failed: {e}");
                tracing::debug!("USB: clear halt on the {name} pipe: {e:?}");
            } else {
                tracing::debug!("USB: {name} pipe cleared");
            }
        }
        STATS.take();
        tracing::info!(
            target: "hci",
            "USB: opened {vid:04X}:{pid:04X} {} (event 0x{event_addr:02x}, ACL 0x{acl_in_addr:02x}/0x{acl_out_addr:02x})",
            info.instance_id().to_string_lossy()
        );

        let stop = Arc::new(AtomicBool::new(false));
        let bootloader = Arc::new(AtomicBool::new(false));
        let command_in_flight = Arc::new(AtomicBool::new(false));
        let (command_tx, command_rx) = mpsc::channel::<(u8, Vec<u8>)>();
        let (command_recycle_tx, command_recycle_rx) = mpsc::channel::<Vec<u8>>();
        let (acl_tx, acl_rx) = mpsc::channel::<Buffer>();
        let (acl_recycle_tx, acl_recycle_rx) = mpsc::channel::<Buffer>();
        let mut threads = Vec::new();
        let spawn = |name: &str, f: Box<dyn FnOnce() + Send>| {
            std::thread::Builder::new().name(name.into()).spawn(f).map_err(|e| e.to_string())
        };
        {
            let (stop, upstream) = (stop.clone(), upstream.clone());
            threads.push(spawn("usb-event-in", Box::new(move || event_loop(&mut event_in, &stop, &upstream)))?);
        }
        {
            let (stop, upstream, bootloader) = (stop.clone(), upstream.clone(), bootloader.clone());
            threads
                .push(spawn("usb-acl-in", Box::new(move || acl_in_loop(&mut acl_in, &stop, &bootloader, &upstream)))?);
        }
        {
            let (stop, interface, in_flight) = (stop.clone(), interface.clone(), command_in_flight.clone());
            let upstream = upstream.clone();
            threads.push(spawn(
                "usb-cmd-out",
                Box::new(move || {
                    let write = |_: u8, p: &[u8]| send_command(&interface, p, COMMAND_TIMEOUT);
                    writer_loop(write, command_rx, command_recycle_tx, &in_flight, &stop, &upstream)
                }),
            )?);
        }
        let acl_writer = {
            let (stop, upstream) = (stop.clone(), upstream.clone());
            let t = spawn(
                "usb-acl-out",
                Box::new(move || {
                    // Media data has a deadline: a busy PC must not hold
                    // the writer back
                    let _priority = crate::acl_out_priority();
                    acl_writer_loop(&mut acl_out, acl_rx, acl_recycle_tx, &stop, &upstream)
                }),
            )?;
            let thread = t.thread().clone();
            threads.push(t);
            thread
        };
        self.opened = Some(Opened {
            interface,
            event_addr,
            device,
            stop,
            command_tx,
            command_recycle_rx,
            command_in_flight,
            acl_tx,
            acl_recycle_rx,
            acl_writer,
            upstream,
            bootloader,
            threads,
        });
        Ok(())
    }

    fn close(&mut self) {
        let Some(opened) = self.opened.take() else { return };
        tracing::debug!("USB: closing; stopping {} I/O thread(s)", opened.threads.len());
        let started = Instant::now();
        opened.stop.store(true, Ordering::Release);
        // The command writer finishes the command it has (bounded by its
        // timeout); the ACL writer cancels its transfers
        drop(opened.command_tx);
        drop(opened.acl_tx);
        opened.acl_writer.unpark();
        for t in opened.threads {
            let name = t.thread().name().unwrap_or("?").to_owned();
            if t.join().is_err() {
                tracing::debug!("USB: thread {name} panicked");
            }
        }
        tracing::debug!("USB: I/O threads stopped in {:?}", started.elapsed());
        // Leave the controller idle (no links, no scans, no queued events)
        // for the next session, as Linux does with HCI_QUIRK_RESET_ON_CLOSE;
        // not a bootloader (module docs)
        if opened.bootloader.load(Ordering::Acquire) {
            tracing::info!(target: "hci", "USB: controller in its bootloader, no HCI Reset on close");
        } else {
            match send_command(&opened.interface, &[0x03, 0x0C, 0x00], CLOSE_RESET_TIMEOUT) {
                Ok(()) => {
                    let at = Instant::now();
                    match reset_completed(&opened.interface, opened.event_addr, CLOSE_RESET_WAIT) {
                        Ok(()) => tracing::info!(
                            target: "hci",
                            "USB: HCI Reset on close done in {} ms",
                            at.elapsed().as_millis()
                        ),
                        Err(e) => tracing::warn!(target: "hci", "USB: HCI Reset sent on close, but {e}"),
                    }
                }
                Err(e) => tracing::warn!(target: "hci", "USB: HCI Reset on close failed: {e}"),
            }
        }
        tracing::info!(target: "hci", "USB: closed");
    }

    fn set_bootloader(&mut self, on: bool) {
        if let Some(o) = &self.opened {
            tracing::debug!(
                "USB: bootloader {on}: the bulk IN pipe carries {}",
                if on { "events" } else { "ACL data" }
            );
            o.bootloader.store(on, Ordering::Release);
        }
    }

    fn control(&mut self, request: &ControlRequest) -> Result<Vec<u8>, String> {
        let Some(o) = &self.opened else { return Err("the adapter is not open".into()) };
        let control_type = match request.kind {
            ControlKind::Standard => ControlType::Standard,
            ControlKind::Class => ControlType::Class,
            ControlKind::Vendor => ControlType::Vendor,
        };
        let recipient = match request.recipient {
            ControlRecipient::Device => Recipient::Device,
            ControlRecipient::Interface => Recipient::Interface,
            ControlRecipient::Endpoint => Recipient::Endpoint,
            ControlRecipient::Other => Recipient::Other,
        };
        let result = match &request.data {
            ControlData::In(length) => o
                .interface
                .control_in(
                    ControlIn {
                        control_type,
                        recipient,
                        request: request.request,
                        value: request.value,
                        index: request.index,
                        length: *length,
                    },
                    COMMAND_TIMEOUT,
                )
                .wait(),
            ControlData::Out(data) => o
                .interface
                .control_out(
                    ControlOut {
                        control_type,
                        recipient,
                        request: request.request,
                        value: request.value,
                        index: request.index,
                        data,
                    },
                    COMMAND_TIMEOUT,
                )
                .wait()
                .map(|()| Vec::new()),
        };
        result.map_err(|e| format!("{e:?}"))
    }

    fn usb_descriptors(&mut self) -> Option<UsbDescriptors> {
        let o = self.opened.as_ref()?;
        Some(descriptors(&o.device))
    }

    fn can_send(&mut self, packet_type: u8) -> bool {
        let Some(o) = &self.opened else { return false };
        match out_slot(packet_type) {
            Some(0) => !o.command_in_flight.load(Ordering::Acquire),
            Some(_) => true,
            None => false,
        }
    }

    fn send(&mut self, packet_type: u8, packet: &[u8]) -> bool {
        let Some(o) = &self.opened else { return false };
        match out_slot(packet_type) {
            Some(0) => {
                tracing::trace!("USB: command 0x{:04X} out, {} bytes", le16(packet), packet.len());
                let mut buf = o.command_recycle_rx.try_recv().unwrap_or_default();
                buf.clear();
                buf.extend_from_slice(packet);
                o.command_in_flight.store(true, Ordering::Release);
                if o.command_tx.send((packet_type, buf)).is_err() {
                    o.command_in_flight.store(false, Ordering::Release);
                    return false;
                }
                true
            }
            Some(_) => {
                tracing::trace!(
                    "USB: {} out, handle 0x{:03X}, {} bytes",
                    if packet_type == HCI_ISO_DATA_PACKET { "ISO" } else { "ACL" },
                    le16(packet) & 0x0FFF,
                    packet.len()
                );
                let mut buf = o
                    .acl_recycle_rx
                    .try_recv()
                    .ok()
                    .filter(|b| b.capacity() >= packet.len())
                    .unwrap_or_else(|| Buffer::new(packet.len().max(ACL_TRANSFER_SIZE)));
                buf.clear();
                buf.extend_from_slice(packet);
                if o.acl_tx.send(buf).is_err() {
                    return false;
                }
                o.acl_writer.unpark();
                // Copied: BTstack may reuse its buffer and send the next one
                o.upstream.packet_sent();
                true
            }
            None => false,
        }
    }
}

impl Drop for UsbTransport {
    fn drop(&mut self) {
        self.close();
    }
}

/// The first two bytes little-endian (a command's opcode, an ACL / ISO
/// packet's handle and flags); 0 if shorter
fn le16(p: &[u8]) -> u16 {
    match p {
        [a, b, ..] => u16::from_le_bytes([*a, *b]),
        _ => 0,
    }
}

/// Reads the event pipe (the event reader has stopped) until the Command
/// Complete of HCI Reset; other events are dropped, the session is over.
/// Err: why it was not seen
fn reset_completed(interface: &Interface, event_addr: u8, wait: Duration) -> Result<(), String> {
    let mut ep = interface
        .endpoint::<Interrupt, In>(event_addr)
        .map_err(|e| format!("the event pipe 0x{event_addr:02x} is not available: {e}"))?;
    let deadline = Instant::now() + wait;
    let mut events = Reassembler::new(HCI_EVENT_PACKET);
    let mut result = Err(format!("no Command Complete within {} ms", wait.as_millis()));
    submit_in(&mut ep, EVENT_TRANSFER_SIZE);
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        let Some(c) = ep.wait_next_complete(left) else { break };
        if let Err(e) = c.status {
            result = Err(format!("the event pipe failed: {e:?}"));
            break;
        }
        // Command Complete: 0x0E, length, command credits, opcode 0x0C03
        let mut done = false;
        events.push(&c.buffer[..c.actual_len], |p| done |= p.len() >= 5 && p[0] == 0x0E && p[3..5] == [0x03, 0x0C]);
        if done {
            result = Ok(());
            break;
        }
        submit_in(&mut ep, EVENT_TRANSFER_SIZE);
    }
    drain(&mut ep);
    result
}

fn submit_in<T: nusb::transfer::BulkOrInterrupt>(ep: &mut Endpoint<T, In>, size: usize) {
    // IN transfers must be a multiple of the max packet size
    let mps = ep.max_packet_size().max(1);
    let len = size.div_ceil(mps) * mps;
    ep.submit(Buffer::new(len));
}

fn drain<T: nusb::transfer::BulkOrInterrupt>(ep: &mut Endpoint<T, In>) {
    tracing::debug!("USB: cancelling {} IN transfer(s) on 0x{:02x}", ep.pending(), ep.endpoint_address());
    ep.cancel_all();
    while ep.pending() > 0 {
        if ep.wait_next_complete(Duration::from_secs(1)).is_none() {
            tracing::warn!(target: "hci", "USB: a cancelled IN transfer did not return");
            break;
        }
    }
}

fn event_loop(ep: &mut Endpoint<Interrupt, In>, stop: &AtomicBool, upstream: &Upstream) {
    let mut events = Reassembler::new(HCI_EVENT_PACKET);
    let mut errors = 0u32;
    let addr = ep.endpoint_address();
    tracing::debug!("USB: event reader started (0x{addr:02x}, {IN_TRANSFERS} x {EVENT_TRANSFER_SIZE} B)");
    let mut stats_at = Instant::now();
    for _ in 0..IN_TRANSFERS {
        submit_in(ep, EVENT_TRANSFER_SIZE);
    }
    while !stop.load(Ordering::Acquire) {
        if stats_at.elapsed() >= STATS_EVERY {
            STATS.log(stats_at.elapsed());
            stats_at = Instant::now();
        }
        let Some(c) = ep.wait_next_complete(POLL) else { continue };
        match c.status {
            Ok(()) => {
                errors = 0;
                events.push(&c.buffer[..c.actual_len], |p| {
                    tracing::trace!(
                        "USB: event 0x{:02X} in on 0x{addr:02x}, {} bytes",
                        p.first().copied().unwrap_or(0),
                        p.len()
                    );
                    Stats::add(&STATS.events_in, 1);
                    Stats::add(&STATS.events_in_bytes, p.len());
                    upstream.deliver(HCI_EVENT_PACKET, p)
                });
            }
            Err(TransferError::Cancelled) if stop.load(Ordering::Acquire) => break,
            Err(e) if device_gone(&e) => {
                tracing::debug!("USB: event read on 0x{addr:02x}: {e:?}, the adapter is gone");
                upstream.fail(&format!("HCI event read: {e}"));
                break;
            }
            Err(e) => {
                // One bad read must not end event delivery for the session
                errors += 1;
                Stats::add(&STATS.in_errors, 1);
                tracing::debug!("USB: event read on 0x{addr:02x} failed: {e:?} ({} bytes)", c.actual_len);
                events.reset();
                if errors > EVENT_IN_MAX_ERRORS {
                    upstream.fail(&format!("HCI event read failed {errors} times in a row: {e}"));
                    break;
                }
                tracing::warn!(target: "hci", "USB: HCI event read failed ({e}), resetting the pipe ({errors}/{EVENT_IN_MAX_ERRORS})");
                drain(ep);
                if let Err(e) = ep.clear_halt().wait() {
                    tracing::warn!(target: "hci", "USB: clearing the event pipe failed: {e}");
                } else {
                    tracing::debug!("USB: event pipe 0x{addr:02x} cleared, reading again");
                }
                for _ in 0..IN_TRANSFERS {
                    submit_in(ep, EVENT_TRANSFER_SIZE);
                }
                continue;
            }
        }
        submit_in(ep, EVENT_TRANSFER_SIZE);
    }
    drain(ep);
    STATS.log(stats_at.elapsed());
    tracing::debug!("USB: event reader stopped (stop flag {})", stop.load(Ordering::Acquire));
}

/// `bootloader`: what comes in is HCI events ([`HciBackend::set_bootloader`])
fn acl_in_loop(ep: &mut Endpoint<Bulk, In>, stop: &AtomicBool, bootloader: &AtomicBool, upstream: &Upstream) {
    let mut acl = Reassembler::new(HCI_ACL_DATA_PACKET);
    let mut events = Reassembler::new(HCI_EVENT_PACKET);
    let mut events_before = false;
    let mut errors = 0u32;
    let addr = ep.endpoint_address();
    tracing::debug!("USB: ACL reader started (0x{addr:02x}, {IN_TRANSFERS} x {ACL_TRANSFER_SIZE} B)");
    for _ in 0..IN_TRANSFERS {
        submit_in(ep, ACL_TRANSFER_SIZE);
    }
    while !stop.load(Ordering::Acquire) {
        let Some(c) = ep.wait_next_complete(POLL) else { continue };
        match c.status {
            Ok(()) if bootloader.load(Ordering::Acquire) => {
                errors = 0;
                if !events_before {
                    acl.reset();
                    events_before = true;
                }
                events.push(&c.buffer[..c.actual_len], |p| {
                    tracing::trace!("USB: event 0x{:02X} on the bulk IN pipe 0x{addr:02x}, {} bytes", p[0], p.len());
                    upstream.deliver(HCI_EVENT_PACKET, p)
                });
            }
            Ok(()) => {
                errors = 0;
                if events_before {
                    events.reset();
                    events_before = false;
                }
                acl.push(&c.buffer[..c.actual_len], |p| {
                    tracing::trace!(
                        "USB: ACL in on 0x{addr:02x}, handle 0x{:03X}, {} bytes",
                        le16(p) & 0x0FFF,
                        p.len()
                    );
                    Stats::add(&STATS.acl_in, 1);
                    Stats::add(&STATS.acl_in_bytes, p.len());
                    upstream.deliver(HCI_ACL_DATA_PACKET, p)
                });
            }
            Err(TransferError::Cancelled) if stop.load(Ordering::Acquire) => break,
            Err(e) if device_gone(&e) => {
                tracing::debug!("USB: ACL read on 0x{addr:02x}: {e:?}, the adapter is gone");
                upstream.fail(&format!("ACL read: {e}"));
                break;
            }
            Err(e) => {
                // As v1: reset the pipe and read on (a stall, a bad
                // transfer); only a run of failures gives up
                errors += 1;
                Stats::add(&STATS.in_errors, 1);
                tracing::debug!("USB: ACL read on 0x{addr:02x} failed: {e:?} ({} bytes)", c.actual_len);
                acl.reset();
                events.reset();
                if errors > ACL_IN_MAX_ERRORS {
                    upstream.fail(&format!("ACL read failed {errors} times in a row: {e}"));
                    break;
                }
                tracing::warn!(target: "hci", "USB: ACL read failed ({e}), resetting the pipe ({errors}/{ACL_IN_MAX_ERRORS})");
                drain(ep);
                if let Err(e) = ep.clear_halt().wait() {
                    tracing::warn!(target: "hci", "USB: clearing the ACL in pipe failed: {e}");
                } else {
                    tracing::debug!("USB: ACL in pipe 0x{addr:02x} cleared, reading again");
                }
                for _ in 0..IN_TRANSFERS {
                    submit_in(ep, ACL_TRANSFER_SIZE);
                }
                continue;
            }
        }
        submit_in(ep, ACL_TRANSFER_SIZE);
    }
    drain(ep);
    tracing::debug!("USB: ACL reader stopped (stop flag {})", stop.load(Ordering::Acquire));
}

/// What becomes of a failed write
#[derive(Debug, PartialEq, Eq)]
enum WriteFailure {
    /// The stack has to be restarted
    Fatal,
    /// Logged; the packet is lost and the stack goes on
    Dropped,
}

/// A lost command leaves BTstack waiting for its answer for good, so any
/// command failure is fatal. A lost ACL / ISO packet is not (L2CAP and the
/// media stream go on, as in v1): only a gone adapter is.
fn write_failure(packet_type: u8, e: &TransferError) -> WriteFailure {
    if packet_type == HCI_COMMAND_DATA_PACKET || device_gone(e) { WriteFailure::Fatal } else { WriteFailure::Dropped }
}

/// Wakes the ACL writer when one of its transfers completes
struct ThreadWaker(Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

/// Writes ACL / ISO packets in the order BTstack hands them over, up to
/// [`ACL_OUT_TRANSFERS`] at a time. BTstack already had them as sent
/// ([`UsbTransport::send`]). Writes have no timeout, as v1's: a controller
/// may hold data for a while (a link going down, the air busy), which is not
/// a failure. The thread sleeps until a packet comes ([`UsbTransport::send`]
/// unparks it) or a transfer completes (the waker).
fn acl_writer_loop(
    ep: &mut Endpoint<Bulk, Out>,
    rx: mpsc::Receiver<Buffer>,
    recycle: mpsc::Sender<Buffer>,
    stop: &AtomicBool,
    upstream: &Upstream,
) {
    let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut open = true;
    // Submit times of the transfers in flight (they complete in order)
    let mut submitted: VecDeque<Instant> = VecDeque::with_capacity(ACL_OUT_TRANSFERS);
    let addr = ep.endpoint_address();
    tracing::debug!("USB: ACL writer started (0x{addr:02x}, up to {ACL_OUT_TRANSFERS} in flight)");
    while !stop.load(Ordering::Acquire) {
        while open && ep.pending() < ACL_OUT_TRANSFERS {
            match rx.try_recv() {
                Ok(buf) => {
                    ep.submit(buf);
                    submitted.push_back(Instant::now());
                    ACL_OUT_PENDING_MAX.fetch_max(ep.pending() as u64, Ordering::Relaxed);
                    STATS.acl_out_pending_max.fetch_max(ep.pending() as u64, Ordering::Relaxed);
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => open = false,
            }
        }
        if ep.pending() == 0 {
            if !open {
                break;
            }
            std::thread::park_timeout(POLL);
            continue;
        }
        let Poll::Ready(c) = ep.poll_next_complete(&mut cx) else {
            std::thread::park_timeout(POLL);
            continue;
        };
        let len = c.actual_len;
        let _ = recycle.send(c.buffer);
        if let Some(at) = submitted.pop_front() {
            ACL_OUT_LATENCY_US.fetch_max(at.elapsed().as_micros() as u64, Ordering::Relaxed);
            tracing::trace!("USB: ACL out on 0x{addr:02x} done, {len} bytes, {:?}, {:?}", at.elapsed(), c.status);
        }
        let Err(e) = c.status else {
            Stats::add(&STATS.acl_out, 1);
            Stats::add(&STATS.acl_out_bytes, len);
            continue;
        };
        Stats::add(&STATS.out_errors, 1);
        if stop.load(Ordering::Acquire) {
            break;
        }
        tracing::debug!("USB: ACL write on 0x{addr:02x} failed: {e:?} ({len} bytes, {} more pending)", ep.pending());
        if write_failure(HCI_ACL_DATA_PACKET, &e) == WriteFailure::Fatal {
            upstream.fail(&format!("ACL write: {e}"));
            break;
        }
        if matches!(e, TransferError::Stall) {
            // The writes behind it fail as well, and every later one until
            // the pipe is cleared
            let behind = cancel_out(ep, &recycle);
            submitted.clear();
            tracing::warn!(target: "hci", "USB: ACL write failed ({e}), {} packet(s) dropped", 1 + behind);
            if let Err(e) = ep.clear_halt().wait() {
                tracing::warn!(target: "hci", "USB: clearing the ACL out pipe failed: {e}");
            } else {
                tracing::debug!("USB: ACL out pipe 0x{addr:02x} cleared after a stall");
            }
        } else {
            tracing::warn!(target: "hci", "USB: ACL write failed ({e}), packet dropped");
        }
    }
    let cancelled = cancel_out(ep, &recycle);
    tracing::debug!("USB: ACL writer stopped ({cancelled} write(s) cancelled)");
}

/// Cancels the pending OUT transfers and waits for them; returns how many
/// were pending
fn cancel_out(ep: &mut Endpoint<Bulk, Out>, recycle: &mpsc::Sender<Buffer>) -> usize {
    let pending = ep.pending();
    ep.cancel_all();
    while ep.pending() > 0 {
        match ep.wait_next_complete(Duration::from_secs(1)) {
            Some(c) => {
                let _ = recycle.send(c.buffer);
            }
            None => {
                tracing::warn!(target: "hci", "USB: a cancelled ACL write did not return");
                break;
            }
        }
    }
    pending
}

/// Sends the commands in the order BTstack hands them over and reports each
/// as sent. Commands (control pipe) and ACL / ISO data (bulk OUT) have a
/// thread each, so data the controller holds back does not delay commands,
/// as in v1 where both were separate overlapped transfers.
fn writer_loop(
    mut write: impl FnMut(u8, &[u8]) -> Result<(), TransferError>,
    rx: mpsc::Receiver<(u8, Vec<u8>)>,
    recycle: mpsc::Sender<Vec<u8>>,
    in_flight: &AtomicBool,
    stop: &AtomicBool,
    upstream: &Upstream,
) {
    tracing::debug!("USB: command writer started");
    loop {
        let (packet_type, packet) = match rx.recv_timeout(POLL) {
            Ok(p) => p,
            Err(mpsc::RecvTimeoutError::Timeout) if !stop.load(Ordering::Acquire) => continue,
            Err(_) => break,
        };
        let (head, len, started) = (le16(&packet), packet.len(), Instant::now());
        let result = write(packet_type, &packet);
        in_flight.store(false, Ordering::Release);
        let _ = recycle.send(packet);
        match result {
            Ok(()) => {
                tracing::trace!("USB: command 0x{head:04X} written, {len} bytes, {:?}", started.elapsed());
                Stats::add(&STATS.commands_out, 1);
                upstream.packet_sent()
            }
            Err(e) => {
                let what = match packet_type {
                    HCI_COMMAND_DATA_PACKET => "HCI command",
                    HCI_ISO_DATA_PACKET => "ISO write",
                    _ => "ACL write",
                };
                Stats::add(&STATS.out_errors, 1);
                tracing::debug!(
                    "USB: {what} 0x{head:04X} ({len} bytes) on the control pipe failed after {:?}: {e:?}{}",
                    started.elapsed(),
                    if stop.load(Ordering::Acquire) { " (closing)" } else { "" }
                );
                if stop.load(Ordering::Acquire) {
                    break;
                }
                match write_failure(packet_type, &e) {
                    WriteFailure::Fatal => {
                        upstream.fail(&format!("{what}: {e}"));
                        if device_gone(&e) {
                            break;
                        }
                    }
                    WriteFailure::Dropped => {
                        tracing::warn!(target: "hci", "USB: {what} failed ({e}), packet dropped");
                    }
                }
                // Free BTstack's packet buffer anyway
                upstream.packet_sent();
            }
        }
    }
    tracing::debug!("USB: command writer stopped");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_goes_through_the_acl_slot() {
        assert_eq!(out_slot(HCI_COMMAND_DATA_PACKET), Some(0));
        assert_eq!(out_slot(HCI_ACL_DATA_PACKET), Some(1));
        assert_eq!(out_slot(HCI_ISO_DATA_PACKET), Some(1));
        // SCO and events are not sent on this transport
        assert_eq!(out_slot(btstack_sys::consts::HCI_SCO_DATA_PACKET), None);
        assert_eq!(out_slot(HCI_EVENT_PACKET), None);
    }

    #[test]
    fn only_a_gone_adapter_or_a_lost_command_is_fatal() {
        use TransferError::*;
        for data in [HCI_ACL_DATA_PACKET, HCI_ISO_DATA_PACKET] {
            for e in [Cancelled, Stall, Fault, InvalidArgument, Unknown(31)] {
                assert_eq!(write_failure(data, &e), WriteFailure::Dropped, "{data} {e:?}");
            }
            for e in [Disconnected, Unknown(22), Unknown(6)] {
                assert_eq!(write_failure(data, &e), WriteFailure::Fatal, "{data} {e:?}");
            }
        }
        for e in [Cancelled, Stall, Disconnected] {
            assert_eq!(write_failure(HCI_COMMAND_DATA_PACKET, &e), WriteFailure::Fatal);
        }
        assert!(device_gone(&Disconnected));
        assert!(!device_gone(&Stall));
        assert!(!device_gone(&Cancelled));
    }

    fn usb_candidate(vid: u16, pid: u16, bluetooth_class: bool, known: bool) -> UsbCandidate {
        UsbCandidate {
            vid,
            pid,
            instance_id: format!(r"USB\VID_{vid:04X}&PID_{pid:04X}\1"),
            driver: Some("WinUSB".into()),
            product: None,
            bluetooth_class,
            known,
        }
    }

    #[test]
    fn selector_takes() {
        let any = UsbSelector::default();
        assert!(any.takes(&usb_candidate(0x0BDA, 0x8771, true, false)));
        // A vendor-class adapter (ASUS BT400) once its VID:PID is known
        assert!(any.takes(&usb_candidate(0x0B05, 0x17CB, false, true)));
        assert!(!any.takes(&usb_candidate(0x0B05, 0x17CB, false, false)));
        let vp = UsbSelector { vid_pid: Some((0x0B05, 0x17CB)), ..Default::default() };
        assert!(vp.takes(&usb_candidate(0x0B05, 0x17CB, false, false)));
        assert!(!vp.takes(&usb_candidate(0x0BDA, 0x8771, true, false)));
        let id = UsbSelector::from_arg(r"usb\vid_0bda&pid_8771\1").unwrap();
        assert!(id.takes(&usb_candidate(0x0BDA, 0x8771, true, false)));
        assert!(!id.takes(&usb_candidate(0x0BDA, 0x8772, true, false)));
    }

    #[test]
    fn selector_from_instance_id() {
        let s = UsbSelector::from_arg(r"USB\VID_0BDA&PID_8771\00E04C000001").unwrap();
        assert_eq!(s.instance_id.as_deref(), Some(r"USB\VID_0BDA&PID_8771\00E04C000001"));
        assert!(s.vid_pid.is_none());
        let s = UsbSelector::from_arg(r" usb\vid_0bda&pid_8771\5&1a2b3c&0&4 ").unwrap();
        assert_eq!(s.instance_id.as_deref(), Some(r"usb\vid_0bda&pid_8771\5&1a2b3c&0&4"));
    }

    #[test]
    fn selector_from_device_path() {
        let s = UsbSelector::from_arg(r"\\?\usb#vid_0bda&pid_8771#00e04c000001#{a5dcbf10-6530-11d2-901f-00c04fb951ed}")
            .unwrap();
        assert_eq!(s.instance_id.as_deref(), Some(r"usb\vid_0bda&pid_8771\00e04c000001"));
        assert!(
            s.instance_id.unwrap().eq_ignore_ascii_case(r"USB\VID_0BDA&PID_8771\00E04C000001"),
            "matches the instance ID nusb reports"
        );
        assert_eq!(
            instance_id_from_device_path(
                r"\\.\USB#VID_0A12&PID_0001#6&2C3D&0&1#{dee824ef-729b-4a0e-9c14-b7117d33a817}"
            )
            .as_deref(),
            Some(r"USB\VID_0A12&PID_0001\6&2C3D&0&1")
        );
        assert!(instance_id_from_device_path(r"\\?\usb#vid_0bda&pid_8771#00e04c000001").is_none());
        assert!(instance_id_from_device_path(r"usb#vid_0bda&pid_8771#x#{guid}").is_none());
    }

    #[test]
    fn selector_from_vid_pid() {
        let s = UsbSelector::from_arg("0bda:8771").unwrap();
        assert_eq!(s.vid_pid, Some((0x0BDA, 0x8771)));
        assert!(s.instance_id.is_none());
    }

    #[test]
    fn bad_selectors() {
        for bad in [
            "",
            "hello",
            "0bda:87",
            r"PCI\VEN_8086&DEV_A0F0\3&11583659&0&A3",
            r"USB\VID_0BDA&PID_8771",
            r"\\?\hid#x#{g}",
        ] {
            assert!(UsbSelector::from_arg(bad).is_err(), "{bad}");
        }
    }
}
