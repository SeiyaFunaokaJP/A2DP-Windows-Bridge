//! Chip probe: what a controller answers to standard read requests,
//! recorded byte for byte (docs/chip-support.md)
//!
//! For adapters BlueCaster cannot bring up yet (MediaTek, Qualcomm) and for
//! reports of odd ones: whoever adds a loader sees what the chip says. The
//! probe only reads: no HCI Reset (a bootloader may not take it), no vendor
//! command. A step without an answer is recorded and the next one is sent;
//! a controller that answers nothing yields a report of timeouts, which says
//! something too.
//!
//! The steps: the standard HCI read commands, then over USB the device
//! descriptor and the manufacturer and product strings it names (standard
//! GET_DESCRIPTOR requests on the default pipe). The app adds the USB
//! descriptors Windows keeps ([`ProbeReport::usb`]).
//!
//! [`Probe`] is a [`ChipLoader`]: the app runs it on the raw transport in
//! place of HCI init ([`crate::BtHost::probe`]), so its packets are in the
//! HCI capture as well.

use std::collections::VecDeque;
use std::time::Instant;

use crate::bringup::{Action, ChipLoader, ControlRequest, Outcome, UsbDescriptors};
use crate::chips;

const EVENT_COMMAND_COMPLETE: u8 = 0x0E;
const EVENT_COMMAND_STATUS: u8 = 0x0F;

/// Answer to one HCI step
pub const STEP_TIMEOUT_MS: u32 = 1000;

/// Events kept that answer no step (a bootloader's vendor events)
const MAX_OTHER_EVENTS: usize = 32;

/// The HCI steps: read commands every controller should take (Core spec
/// Vol 4 Part E 7.4, 7.3.12, 7.8.2 / 7.8.3)
const HCI_STEPS: [(&str, u16); 8] = [
    ("Read Local Version Information", 0x1001),
    ("Read BD_ADDR", 0x1009),
    ("Read Local Supported Commands", 0x1002),
    ("Read Local Supported Features", 0x1003),
    ("Read Buffer Size", 0x1005),
    ("Read Local Name", 0x0C14),
    ("LE Read Local Supported Features", 0x2003),
    ("LE Read Buffer Size", 0x2002),
];

const USB_DEVICE_DESCRIPTOR: &str = "USB device descriptor";
const USB_MANUFACTURER: &str = "USB manufacturer string";
const USB_PRODUCT: &str = "USB product string";

/// How a step reached the adapter
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    /// An HCI command; the answer is its event
    Hci,
    /// A USB control transfer; `command` is its SETUP packet, the answer
    /// the bytes read
    Usb,
    /// A script's step that is no single request (crate::script): a wait,
    /// an awaited event (its answer), bootloader mode, an upload (its last
    /// command and answer)
    Other,
}

/// One request and what came back
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeStep {
    pub name: String,
    pub kind: StepKind,
    /// HCI: the command as sent (opcode, length, parameters); USB: the SETUP
    /// packet
    pub command: Vec<u8>,
    /// HCI: the event that answered it (Command Complete or Command Status)
    /// as received; USB: the bytes read. None: no answer (HCI: none within
    /// [`STEP_TIMEOUT_MS`]; USB: see `error`)
    pub answer: Option<Vec<u8>>,
    /// Why a USB transfer failed
    pub error: Option<String>,
    pub millis: u32,
}

impl ProbeStep {
    fn opcode(&self) -> Option<u16> {
        (self.kind == StepKind::Hci).then(|| u16::from_le_bytes([self.command[0], self.command[1]]))
    }

    /// The return parameters of a successful Command Complete
    fn ret(&self) -> Option<&[u8]> {
        match self.answer.as_deref()? {
            [EVENT_COMMAND_COMPLETE, _, _, _, _, 0, ret @ ..] => Some(ret),
            _ => None,
        }
    }
}

/// What the controller said; [`ProbeReport::summary`] reads the known parts
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProbeReport {
    pub steps: Vec<ProbeStep>,
    /// Events that answered no step, as received
    pub other_events: Vec<Vec<u8>>,
    pub millis: u32,
    /// Why the probe stopped early (the adapter could not be opened, a
    /// command could not be sent)
    pub error: Option<String>,
    /// The USB descriptors Windows keeps for the adapter (None over H4)
    pub usb: Option<UsbDescriptors>,
}

/// The decoded fields of a [`ProbeReport`]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProbeSummary {
    pub hci_version: Option<u8>,
    pub hci_revision: Option<u16>,
    pub lmp_version: Option<u8>,
    pub lmp_subversion: Option<u16>,
    pub company: Option<u16>,
    pub company_name: Option<&'static str>,
    /// "00:1A:7D:DA:71:13"
    pub address: Option<String>,
    pub name: Option<String>,
    /// ACL data packet length and count
    pub acl_buffers: Option<(u16, u16)>,
    pub le_acl_buffers: Option<(u16, u8)>,
    /// From the USB device descriptor
    pub usb_ids: Option<(u16, u16)>,
    pub usb_manufacturer: Option<String>,
    pub usb_product: Option<String>,
    pub answered: usize,
    pub steps: usize,
}

/// A USB string descriptor's text (UTF-16LE after the 2-byte header)
fn usb_string(d: &[u8]) -> Option<String> {
    let len = (*d.first()? as usize).min(d.len());
    if len < 2 || d.get(1) != Some(&0x03) {
        return None;
    }
    let (pairs, _) = d[2..len].as_chunks::<2>();
    let units: Vec<u16> = pairs.iter().map(|&c| u16::from_le_bytes(c)).collect();
    Some(String::from_utf16_lossy(&units))
}

/// iManufacturer and iProduct of a device descriptor
fn string_indexes(d: &[u8]) -> Option<(u8, u8)> {
    (d.len() >= 18 && d[1] == 0x01).then(|| (d[14], d[15]))
}

impl ProbeReport {
    fn ret(&self, opcode: u16) -> Option<&[u8]> {
        self.steps.iter().find(|s| s.opcode() == Some(opcode)).and_then(ProbeStep::ret)
    }

    fn usb_answer(&self, name: &str) -> Option<&[u8]> {
        self.steps.iter().find(|s| s.kind == StepKind::Usb && s.name == name).and_then(|s| s.answer.as_deref())
    }

    pub fn summary(&self) -> ProbeSummary {
        let le16 = |b: &[u8], at: usize| b.get(at..at + 2).map(|v| u16::from_le_bytes([v[0], v[1]]));
        let mut s = ProbeSummary {
            answered: self.steps.iter().filter(|s| s.answer.is_some()).count(),
            steps: self.steps.len(),
            ..ProbeSummary::default()
        };
        if let Some(v) = self.ret(0x1001).filter(|v| v.len() >= 8) {
            s.hci_version = Some(v[0]);
            s.hci_revision = le16(v, 1);
            s.lmp_version = Some(v[3]);
            s.company = le16(v, 4);
            s.lmp_subversion = le16(v, 6);
            s.company_name = s.company.and_then(chips::company_name);
        }
        if let Some(a) = self.ret(0x1009).and_then(|v| v.get(..6)) {
            s.address = Some(crate::addr_str(&[a[5], a[4], a[3], a[2], a[1], a[0]]));
        }
        if let Some(n) = self.ret(0x0C14) {
            let end = n.iter().position(|&b| b == 0).unwrap_or(n.len());
            s.name = Some(String::from_utf8_lossy(&n[..end]).into_owned());
        }
        if let Some(b) = self.ret(0x1005).filter(|b| b.len() >= 7) {
            s.acl_buffers = le16(b, 0).zip(le16(b, 3));
        }
        if let Some(b) = self.ret(0x2002).filter(|b| b.len() >= 3) {
            s.le_acl_buffers = le16(b, 0).map(|len| (len, b[2]));
        }
        if let Some(d) = self.usb_answer(USB_DEVICE_DESCRIPTOR).filter(|d| string_indexes(d).is_some()) {
            s.usb_ids = le16(d, 8).zip(le16(d, 10));
        }
        s.usb_manufacturer = self.usb_answer(USB_MANUFACTURER).and_then(usb_string);
        s.usb_product = self.usb_answer(USB_PRODUCT).and_then(usb_string);
        s
    }
}

enum Planned {
    Hci(&'static str, u16),
    Usb(&'static str, ControlRequest),
}

/// The probe as a [`ChipLoader`]: one step at a time, each answered, timed
/// out or failed, then [`Outcome::Probed`]; the report from
/// [`ChipLoader::take_report`]
pub struct Probe {
    plan: VecDeque<Planned>,
    sent: Option<Instant>,
    report: ProbeReport,
    started: Instant,
    done: bool,
}

impl Default for Probe {
    fn default() -> Self {
        Probe::new()
    }
}

impl Probe {
    pub fn new() -> Probe {
        let mut plan: VecDeque<Planned> = HCI_STEPS.iter().map(|&(name, op)| Planned::Hci(name, op)).collect();
        plan.push_back(Planned::Usb(USB_DEVICE_DESCRIPTOR, ControlRequest::device_descriptor()));
        Probe { plan, sent: None, report: ProbeReport::default(), started: Instant::now(), done: false }
    }

    pub fn report(&self) -> &ProbeReport {
        &self.report
    }

    fn send_next(&mut self) -> Vec<Action> {
        let Some(step) = self.plan.pop_front() else {
            self.done = true;
            self.sent = None;
            self.report.millis = self.started.elapsed().as_millis() as u32;
            tracing::info!(
                "chip probe: {} of {} requests answered in {} ms",
                self.report.steps.iter().filter(|s| s.answer.is_some()).count(),
                self.report.steps.len(),
                self.report.millis
            );
            return vec![Action::Done(Ok(Outcome::Probed))];
        };
        self.sent = Some(Instant::now());
        match step {
            Planned::Hci(name, opcode) => {
                let [lo, hi] = opcode.to_le_bytes();
                let command = vec![lo, hi, 0];
                self.report.steps.push(ProbeStep {
                    name: name.to_owned(),
                    kind: StepKind::Hci,
                    command: command.clone(),
                    answer: None,
                    error: None,
                    millis: 0,
                });
                vec![Action::Command(command), Action::Timer(STEP_TIMEOUT_MS)]
            }
            Planned::Usb(name, request) => {
                self.report.steps.push(ProbeStep {
                    name: name.to_owned(),
                    kind: StepKind::Usb,
                    command: request.setup_packet().to_vec(),
                    answer: None,
                    error: None,
                    millis: 0,
                });
                vec![Action::UsbControl(request)]
            }
        }
    }

    /// The opcode an event answers (Command Complete / Status), if any
    fn answers(event: &[u8]) -> Option<u16> {
        match event {
            [EVENT_COMMAND_COMPLETE, _, _, lo, hi, ..] => Some(u16::from_le_bytes([*lo, *hi])),
            [EVENT_COMMAND_STATUS, _, _, _, lo, hi, ..] => Some(u16::from_le_bytes([*lo, *hi])),
            _ => None,
        }
    }

    fn elapsed(&self) -> u32 {
        self.sent.map_or(0, |t| t.elapsed().as_millis() as u32)
    }
}

impl ChipLoader for Probe {
    fn vendor(&self) -> &'static str {
        "Chip probe"
    }

    fn start(&mut self) -> Vec<Action> {
        *self = Probe::new();
        tracing::info!("chip probe: {} HCI read commands and the USB descriptors, no HCI Reset", HCI_STEPS.len());
        // Bootloader mode: closing the adapter afterwards sends no HCI Reset
        // either, and events on the bulk IN pipe (a bootloader's) count too
        let mut actions = vec![Action::Bootloader(true)];
        actions.extend(self.send_next());
        actions
    }

    fn on_event(&mut self, event: &[u8]) -> Vec<Action> {
        if self.done {
            return Vec::new();
        }
        let millis = self.elapsed();
        let current = self.sent.and_then(|_| self.report.steps.last_mut()).filter(|s| s.kind == StepKind::Hci);
        match current {
            Some(step) if Probe::answers(event) == step.opcode() => {
                step.millis = millis;
                step.answer = Some(event.to_vec());
                tracing::debug!("chip probe: {} answered in {millis} ms", step.name);
                self.send_next()
            }
            _ => {
                if self.report.other_events.len() < MAX_OTHER_EVENTS {
                    self.report.other_events.push(event.to_vec());
                }
                Vec::new()
            }
        }
    }

    fn on_timeout(&mut self) -> Vec<Action> {
        if self.done {
            return Vec::new();
        }
        if let Some(step) = self.report.steps.last_mut() {
            step.millis = STEP_TIMEOUT_MS;
            tracing::info!("chip probe: no answer to {}", step.name);
        }
        self.send_next()
    }

    fn on_control(&mut self, result: Result<Vec<u8>, String>) -> Vec<Action> {
        if self.done {
            return Vec::new();
        }
        let millis = self.elapsed();
        let Some(step) = self.report.steps.last_mut().filter(|s| s.kind == StepKind::Usb) else {
            return Vec::new();
        };
        step.millis = millis;
        match result {
            Ok(data) => {
                // The device descriptor names its strings
                if step.name == USB_DEVICE_DESCRIPTOR
                    && let Some((manufacturer, product)) = string_indexes(&data)
                {
                    for (name, index) in [(USB_MANUFACTURER, manufacturer), (USB_PRODUCT, product)] {
                        if index != 0 {
                            self.plan.push_back(Planned::Usb(name, ControlRequest::string_descriptor(index)));
                        }
                    }
                }
                step.answer = Some(data);
            }
            Err(e) => {
                tracing::info!("chip probe: {}: {e}", step.name);
                step.error = Some(e);
            }
        }
        self.send_next()
    }

    fn take_report(&mut self) -> Option<ProbeReport> {
        Some(std::mem::take(&mut self.report))
    }
}
