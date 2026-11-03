//! Chip probe: what a controller answers to the standard read commands,
//! recorded byte for byte (docs/chip-bringup-dev.md)
//!
//! For adapters A2DPWB cannot bring up yet (MediaTek, Qualcomm) and for
//! reports of odd ones: whoever adds a loader sees what the chip says. The
//! probe only reads: no HCI Reset (a bootloader may not take it), no vendor
//! command. A step without an answer is recorded and the next one is sent;
//! a controller that answers nothing yields a report of timeouts, which says
//! something too.
//!
//! [`Probe`] is a [`ChipLoader`]: the app runs it on the raw transport in
//! place of HCI init ([`crate::BtHost::probe`]), so its packets are in the
//! HCI capture as well.

use std::time::Instant;

use crate::bringup::{Action, ChipLoader, Outcome};
use crate::chips;

const EVENT_COMMAND_COMPLETE: u8 = 0x0E;
const EVENT_COMMAND_STATUS: u8 = 0x0F;

/// Answer to one step
pub const STEP_TIMEOUT_MS: u32 = 1000;

/// Events kept that answer no step (a bootloader's vendor events)
const MAX_OTHER_EVENTS: usize = 32;

/// The steps: read commands every controller should take (Core spec Vol 4
/// Part E 7.4, 7.3.12, 7.8.2 / 7.8.3)
const STEPS: [(&str, u16); 8] = [
    ("Read Local Version Information", 0x1001),
    ("Read BD_ADDR", 0x1009),
    ("Read Local Supported Commands", 0x1002),
    ("Read Local Supported Features", 0x1003),
    ("Read Buffer Size", 0x1005),
    ("Read Local Name", 0x0C14),
    ("LE Read Local Supported Features", 0x2003),
    ("LE Read Buffer Size", 0x2002),
];

/// One command and what came back
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeStep {
    pub name: &'static str,
    /// The HCI command as sent (opcode, length, parameters)
    pub command: Vec<u8>,
    /// The event that answered it (Command Complete or Command Status), as
    /// received; None: no answer within [`STEP_TIMEOUT_MS`]
    pub answer: Option<Vec<u8>>,
    pub millis: u32,
}

impl ProbeStep {
    fn opcode(&self) -> u16 {
        u16::from_le_bytes([self.command[0], self.command[1]])
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
    pub answered: usize,
    pub steps: usize,
}

impl ProbeReport {
    fn ret(&self, opcode: u16) -> Option<&[u8]> {
        self.steps.iter().find(|s| s.opcode() == opcode).and_then(ProbeStep::ret)
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
        s
    }
}

/// The probe as a [`ChipLoader`]: one step at a time, each answered or timed
/// out, then [`Outcome::Probed`]; the report from [`Probe::report`]
pub struct Probe {
    next: usize,
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
        Probe { next: 0, sent: None, report: ProbeReport::default(), started: Instant::now(), done: false }
    }

    pub fn report(&self) -> &ProbeReport {
        &self.report
    }

    fn send_next(&mut self) -> Vec<Action> {
        let Some(&(name, opcode)) = STEPS.get(self.next) else {
            self.done = true;
            self.sent = None;
            self.report.millis = self.started.elapsed().as_millis() as u32;
            tracing::info!(
                "chip probe: {} of {} commands answered in {} ms",
                self.report.steps.iter().filter(|s| s.answer.is_some()).count(),
                self.report.steps.len(),
                self.report.millis
            );
            return vec![Action::Done(Ok(Outcome::Probed))];
        };
        self.next += 1;
        let [lo, hi] = opcode.to_le_bytes();
        let command = vec![lo, hi, 0];
        self.report.steps.push(ProbeStep { name, command: command.clone(), answer: None, millis: 0 });
        self.sent = Some(Instant::now());
        vec![Action::Command(command), Action::Timer(STEP_TIMEOUT_MS)]
    }

    /// The opcode an event answers (Command Complete / Status), if any
    fn answers(event: &[u8]) -> Option<u16> {
        match event {
            [EVENT_COMMAND_COMPLETE, _, _, lo, hi, ..] => Some(u16::from_le_bytes([*lo, *hi])),
            [EVENT_COMMAND_STATUS, _, _, _, lo, hi, ..] => Some(u16::from_le_bytes([*lo, *hi])),
            _ => None,
        }
    }
}

impl ChipLoader for Probe {
    fn vendor(&self) -> &'static str {
        "Chip probe"
    }

    fn start(&mut self) -> Vec<Action> {
        tracing::info!("chip probe: {} read commands, no HCI Reset", STEPS.len());
        *self = Probe::new();
        self.send_next()
    }

    fn on_event(&mut self, event: &[u8]) -> Vec<Action> {
        if self.done {
            return Vec::new();
        }
        let current = self.sent.and_then(|_| self.report.steps.last_mut());
        match current {
            Some(step) if Probe::answers(event) == Some(step.opcode()) => {
                step.millis = self.sent.map_or(0, |t| t.elapsed().as_millis() as u32);
                step.answer = Some(event.to_vec());
                tracing::debug!("chip probe: {} answered in {} ms", step.name, step.millis);
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

    fn take_report(&mut self) -> Option<ProbeReport> {
        Some(std::mem::take(&mut self.report))
    }
}
