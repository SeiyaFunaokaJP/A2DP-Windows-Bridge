//! Chip scripts: requests to an adapter written as text, for the chip
//! console and step files (developer mode, docs/chip-support.md)
//!
//! A contributor tries a sequence on a reporter's adapter without a new
//! build: the reporter runs the script and sends the report back. One
//! request per line; `#` starts a comment. Numbers are hexadecimal (`0x`
//! optional) except `wait` and the `timeout=`, `chunk=` options.
//!
//! ```text
//! hci 1001                     # an HCI command: opcode, then parameter bytes
//! hci fc05 ff
//! bulk fc09 00 01 02           # the same on the bulk OUT pipe
//! usb in vendor device 63 0 0 4      # control IN: type, recipient, request, value, index, length
//! usb out vendor device 64 0 0 01    # control OUT: ..., then the data bytes
//! wait 100                     # milliseconds
//! expect e4 02 01              # an event: its code and the first parameter bytes
//! bootloader off               # see Action::Bootloader (on at the start)
//! upload patch.bin fc6f 01 01 chunk=240 ack=event:e4:02,01
//! ```
//!
//! `upload` sends a file of the firmware folder in commands of the opcode,
//! each with the header bytes and up to `chunk` bytes of the file, waiting
//! for each acknowledgment: `ack=complete` (default; a Command Complete or a
//! Command Status of the opcode with status 0) or `ack=event:<code>[:<bytes>]`.
//!
//! Safety: only requests known to read ([`Step::reads_only`]) run without
//! permission; any other script needs it, and opcodes on the deny list
//! never run. The script stops at the first failed step. Bootloader mode is
//! on from the start, so closing the adapter sends no HCI Reset.

use std::time::Instant;

use crate::bringup::{
    Action, ChipLoader, ControlData, ControlKind, ControlRecipient, ControlRequest, FileReader, HciEvent, Outcome,
    hci_command, parse_event,
};
use crate::probe::{ProbeReport, ProbeStep, StepKind};

/// Default wait for an answer or an event
pub const STEP_TIMEOUT_MS: u32 = 2000;
/// Upper bound on a script's waits and timeouts
const MAX_MS: u32 = 60_000;
/// Events kept that answer no step
const MAX_OTHER_EVENTS: usize = 64;

/// How an upload's commands are acknowledged
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ack {
    /// A Command Complete or Command Status of the opcode, status 0
    Complete,
    /// An event of this code whose parameters start with these bytes
    Event { code: u8, prefix: Vec<u8> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    Hci { opcode: u16, params: Vec<u8>, bulk: bool },
    Usb(ControlRequest),
    Wait(u32),
    Expect { code: u8, prefix: Vec<u8>, timeout: u32 },
    Bootloader(bool),
    Upload { file: String, opcode: u16, header: Vec<u8>, chunk: usize, ack: Ack, bulk: bool, timeout: u32 },
}

/// HCI commands that only read (Core spec Vol 4 Part E): all of OGF 0x04
/// (informational parameters), OGF 0x05 but Reset Failed Contact Counter,
/// and these reads of OGF 0x03 and 0x08
const READ_OPCODES: [u16; 12] =
    [0x0C14, 0x0C09, 0x0C0F, 0x0C2D, 0x0C38, 0x0C45, 0x2002, 0x2003, 0x2007, 0x200F, 0x201C, 0x202F];

impl Step {
    /// Known not to change the controller: such a script runs without
    /// permission
    pub fn reads_only(&self) -> bool {
        match self {
            Step::Hci { opcode, bulk: false, .. } => {
                let ogf = opcode >> 10;
                ogf == 0x04 || (ogf == 0x05 && *opcode != 0x1402) || READ_OPCODES.contains(opcode)
            }
            // Standard GET_STATUS, GET_DESCRIPTOR, GET_CONFIGURATION,
            // GET_INTERFACE
            Step::Usb(r) => {
                r.kind == ControlKind::Standard
                    && matches!(r.data, ControlData::In(_))
                    && matches!(r.request, 0x00 | 0x06 | 0x08 | 0x0A)
            }
            Step::Wait(_) | Step::Expect { .. } | Step::Bootloader(_) => true,
            Step::Hci { bulk: true, .. } | Step::Upload { .. } => false,
        }
    }

    /// The opcodes the step sends
    fn opcode(&self) -> Option<u16> {
        match self {
            Step::Hci { opcode, .. } | Step::Upload { opcode, .. } => Some(*opcode),
            _ => None,
        }
    }
}

/// A script line and what it asks for
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptLine {
    /// 1-based, in the text
    pub line: usize,
    pub text: String,
    pub step: Step,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Script {
    pub lines: Vec<ScriptLine>,
}

impl Script {
    /// The lines that are not known to only read
    pub fn writes(&self) -> Vec<&ScriptLine> {
        self.lines.iter().filter(|l| !l.step.reads_only()).collect()
    }

    /// The lines that send an opcode of `deny`
    pub fn denied(&self, deny: &[u16]) -> Vec<&ScriptLine> {
        self.lines.iter().filter(|l| l.step.opcode().is_some_and(|o| deny.contains(&o))).collect()
    }
}

fn hex(token: &str) -> Option<u32> {
    let t = token.trim_start_matches("0x").trim_start_matches("0X");
    if t.is_empty() {
        return None;
    }
    u32::from_str_radix(t, 16).ok()
}

fn byte(token: &str) -> Result<u8, String> {
    hex(token).filter(|v| *v <= 0xFF).map(|v| v as u8).ok_or_else(|| format!("'{token}' is not a byte (hex)"))
}

fn word(token: &str) -> Result<u16, String> {
    hex(token)
        .filter(|v| *v <= 0xFFFF)
        .map(|v| v as u16)
        .ok_or_else(|| format!("'{token}' is not a 16-bit value (hex)"))
}

fn bytes(tokens: &[&str]) -> Result<Vec<u8>, String> {
    tokens.iter().map(|t| byte(t)).collect()
}

fn ms(token: &str) -> Result<u32, String> {
    token.parse::<u32>().ok().filter(|v| *v <= MAX_MS).ok_or_else(|| format!("'{token}' is not 0 to {MAX_MS} ms"))
}

/// Splits `key=value` options off the end of a line
fn options<'a>(tokens: &[&'a str]) -> (Vec<&'a str>, Vec<(&'a str, &'a str)>) {
    let mut plain = Vec::new();
    let mut opts = Vec::new();
    for t in tokens {
        match t.split_once('=') {
            Some((k, v)) => opts.push((k, v)),
            None => plain.push(*t),
        }
    }
    (plain, opts)
}

fn parse_ack(v: &str) -> Result<Ack, String> {
    if v == "complete" {
        return Ok(Ack::Complete);
    }
    let rest = v.strip_prefix("event:").ok_or_else(|| format!("ack '{v}': use complete or event:<code>[:<bytes>]"))?;
    let (code, prefix) = rest.split_once(':').unwrap_or((rest, ""));
    let prefix = prefix.split(',').filter(|s| !s.is_empty()).map(byte).collect::<Result<Vec<u8>, String>>()?;
    Ok(Ack::Event { code: byte(code)?, prefix })
}

fn parse_step(tokens: &[&str]) -> Result<Step, String> {
    let (plain, opts) = options(tokens);
    let opt = |name: &str| opts.iter().find(|(k, _)| *k == name).map(|(_, v)| *v);
    for (k, _) in &opts {
        if !matches!(*k, "timeout" | "chunk" | "ack") {
            return Err(format!("unknown option '{k}'"));
        }
    }
    let timeout = opt("timeout").map(ms).transpose()?.unwrap_or(STEP_TIMEOUT_MS);
    match plain.as_slice() {
        ["hci" | "bulk", op, rest @ ..] => {
            Ok(Step::Hci { opcode: word(op)?, params: bytes(rest)?, bulk: plain[0] == "bulk" })
        }
        ["usb", dir @ ("in" | "out"), kind, recipient, request, value, index, rest @ ..] => {
            let kind = match *kind {
                "standard" => ControlKind::Standard,
                "class" => ControlKind::Class,
                "vendor" => ControlKind::Vendor,
                k => return Err(format!("'{k}': use standard, class or vendor")),
            };
            let recipient = match *recipient {
                "device" => ControlRecipient::Device,
                "interface" => ControlRecipient::Interface,
                "endpoint" => ControlRecipient::Endpoint,
                "other" => ControlRecipient::Other,
                r => return Err(format!("'{r}': use device, interface, endpoint or other")),
            };
            let data = if *dir == "in" {
                match rest {
                    [len] => ControlData::In(word(len)?),
                    _ => return Err("usb in: give the length to read last".into()),
                }
            } else {
                ControlData::Out(bytes(rest)?)
            };
            Ok(Step::Usb(ControlRequest {
                kind,
                recipient,
                request: byte(request)?,
                value: word(value)?,
                index: word(index)?,
                data,
            }))
        }
        ["wait", n] => Ok(Step::Wait(ms(n)?)),
        ["expect", code, prefix @ ..] => Ok(Step::Expect { code: byte(code)?, prefix: bytes(prefix)?, timeout }),
        ["bootloader", "on"] => Ok(Step::Bootloader(true)),
        ["bootloader", "off"] => Ok(Step::Bootloader(false)),
        ["upload", file, op, header @ ..] => {
            let chunk = match opt("chunk") {
                Some(c) => c.parse::<usize>().ok().filter(|c| (1..=255).contains(c)).ok_or("chunk: 1 to 255")?,
                None => 240,
            };
            let header = bytes(header)?;
            if header.len() + chunk > 255 {
                return Err("header and chunk are more than 255 bytes".into());
            }
            let ack = opt("ack").map(parse_ack).transpose()?.unwrap_or(Ack::Complete);
            // In the firmware folder only
            if file.contains("..") || file.contains(':') || file.starts_with(['/', '\\']) {
                return Err(format!("'{file}': a file name in the firmware folder"));
            }
            Ok(Step::Upload { file: (*file).to_owned(), opcode: word(op)?, header, chunk, ack, bulk: false, timeout })
        }
        ["upload-bulk", ..] => {
            let mut s = parse_step(&[&["upload"], &tokens[1..]].concat())?;
            if let Step::Upload { bulk, .. } = &mut s {
                *bulk = true;
            }
            Ok(s)
        }
        [] => Err("empty".into()),
        [word, ..] => Err(format!(
            "'{word}' is not a request: hci, bulk, usb in, usb out, wait, expect, bootloader, upload, upload-bulk"
        )),
    }
}

/// Reads a script; errors name their lines ("line 3: ...")
pub fn parse(text: &str) -> Result<Script, Vec<String>> {
    let mut lines = Vec::new();
    let mut errors = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let code = raw.split('#').next().unwrap_or("").trim();
        if code.is_empty() {
            continue;
        }
        let tokens: Vec<&str> = code.split_whitespace().collect();
        match parse_step(&tokens) {
            Ok(step) => lines.push(ScriptLine { line: i + 1, text: code.to_owned(), step }),
            Err(e) => errors.push(format!("line {}: {e}", i + 1)),
        }
    }
    if lines.is_empty() && errors.is_empty() {
        errors.push("no requests".into());
    }
    if errors.is_empty() { Ok(Script { lines }) } else { Err(errors) }
}

/// What the running step waits for
enum Wait {
    None,
    Answer { opcode: u16 },
    Pause,
    Event { code: u8, prefix: Vec<u8> },
    Upload { commands: Vec<Vec<u8>>, next: usize, opcode: u16, ack: Ack, bulk: bool, timeout: u32 },
}

/// Runs a [`Script`] as a [`ChipLoader`]; the report has a step per line
pub struct ScriptLoader {
    script: Script,
    read_file: FileReader,
    next: usize,
    wait: Wait,
    sent: Instant,
    started: Instant,
    report: ProbeReport,
    done: bool,
}

fn hex_text(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

impl ScriptLoader {
    pub fn new(script: Script, read_file: FileReader) -> ScriptLoader {
        ScriptLoader {
            script,
            read_file,
            next: 0,
            wait: Wait::None,
            sent: Instant::now(),
            started: Instant::now(),
            report: ProbeReport::default(),
            done: false,
        }
    }

    fn step(&mut self) -> &mut ProbeStep {
        self.report.steps.last_mut().expect("a step runs")
    }

    fn finish(&mut self, error: Option<String>) -> Vec<Action> {
        self.done = true;
        self.wait = Wait::None;
        self.report.millis = self.started.elapsed().as_millis() as u32;
        if let Some(e) = &error {
            tracing::info!("chip script stopped: {e}");
        } else {
            tracing::info!("chip script: {} steps in {} ms", self.report.steps.len(), self.report.millis);
        }
        self.report.error = error;
        vec![Action::Done(Ok(Outcome::Probed))]
    }

    /// The running step failed: the script stops
    fn fail(&mut self, why: String) -> Vec<Action> {
        let line = self.script.lines[self.next - 1].line;
        self.step().error = Some(why.clone());
        self.step().millis = self.sent.elapsed().as_millis() as u32;
        self.finish(Some(format!("line {line}: {why}")))
    }

    fn succeed(&mut self, answer: Option<Vec<u8>>) -> Vec<Action> {
        let millis = self.sent.elapsed().as_millis() as u32;
        let step = self.step();
        step.millis = millis;
        if answer.is_some() {
            step.answer = answer;
        }
        self.wait = Wait::None;
        self.run_next()
    }

    fn run_next(&mut self) -> Vec<Action> {
        let Some(line) = self.script.lines.get(self.next).cloned() else { return self.finish(None) };
        self.next += 1;
        self.sent = Instant::now();
        let name = format!("line {}: {}", line.line, line.text);
        let mut push = |kind, command: Vec<u8>| {
            self.report.steps.push(ProbeStep {
                name: name.clone(),
                kind,
                command,
                answer: None,
                error: None,
                millis: 0,
            })
        };
        match line.step {
            Step::Hci { opcode, params, bulk } => {
                let c = hci_command(opcode, &params);
                push(StepKind::Hci, c.clone());
                self.wait = Wait::Answer { opcode };
                let send = if bulk { Action::BulkCommand(c) } else { Action::Command(c) };
                vec![send, Action::Timer(STEP_TIMEOUT_MS)]
            }
            Step::Usb(r) => {
                push(StepKind::Usb, r.setup_packet().to_vec());
                vec![Action::UsbControl(r)]
            }
            Step::Wait(ms) => {
                push(StepKind::Other, Vec::new());
                self.wait = Wait::Pause;
                vec![Action::Timer(ms)]
            }
            Step::Expect { code, prefix, timeout } => {
                push(StepKind::Other, Vec::new());
                self.wait = Wait::Event { code, prefix };
                vec![Action::Timer(timeout)]
            }
            Step::Bootloader(on) => {
                push(StepKind::Other, Vec::new());
                let mut actions = vec![Action::Bootloader(on)];
                actions.extend(self.succeed(None));
                actions
            }
            Step::Upload { file, opcode, header, chunk, ack, bulk, timeout } => {
                push(StepKind::Other, Vec::new());
                let Some(data) = (self.read_file)(&file) else {
                    return self.fail(format!("{file} is not in the firmware folder"));
                };
                if data.is_empty() {
                    return self.fail(format!("{file} is empty"));
                }
                let commands: Vec<Vec<u8>> = data
                    .chunks(chunk)
                    .map(|part| {
                        let mut params = header.clone();
                        params.extend_from_slice(part);
                        hci_command(opcode, &params)
                    })
                    .collect();
                tracing::info!("chip script: {file}, {} bytes in {} commands", data.len(), commands.len());
                let first = commands[0].clone();
                self.step().command = first.clone();
                self.wait = Wait::Upload { commands, next: 1, opcode, ack, bulk, timeout };
                let send = if bulk { Action::BulkCommand(first) } else { Action::Command(first) };
                vec![send, Action::Timer(timeout)]
            }
        }
    }

    /// The upload's next command, or the step done
    fn upload_acked(&mut self, event: &[u8]) -> Vec<Action> {
        let Wait::Upload { commands, next, opcode, ack, bulk, timeout } = std::mem::replace(&mut self.wait, Wait::None)
        else {
            return Vec::new();
        };
        self.step().answer = Some(event.to_vec());
        match commands.get(next) {
            Some(c) => {
                let c = c.clone();
                self.step().command = c.clone();
                let send = if bulk { Action::BulkCommand(c) } else { Action::Command(c) };
                self.wait = Wait::Upload { commands, next: next + 1, opcode, ack, bulk, timeout };
                vec![send, Action::Timer(timeout)]
            }
            None => {
                let n = commands.len();
                let step = self.step();
                step.name = format!("{} ({n} commands)", step.name);
                self.succeed(None)
            }
        }
    }

    fn other_event(&mut self, event: &[u8]) -> Vec<Action> {
        if self.report.other_events.len() < MAX_OTHER_EVENTS {
            self.report.other_events.push(event.to_vec());
        }
        Vec::new()
    }
}

impl ChipLoader for ScriptLoader {
    fn vendor(&self) -> &'static str {
        "Chip script"
    }

    fn start(&mut self) -> Vec<Action> {
        tracing::info!(
            "chip script: {} steps, {} that may change the chip",
            self.script.lines.len(),
            self.script.writes().len()
        );
        self.started = Instant::now();
        let mut actions = vec![Action::Bootloader(true)];
        actions.extend(self.run_next());
        actions
    }

    fn on_event(&mut self, event: &[u8]) -> Vec<Action> {
        if self.done {
            return Vec::new();
        }
        /// What the event is to the running step
        enum Is {
            Answer,
            UploadFailed(u16, u8),
            UploadAck,
            /// A status-0 answer of an upload acknowledged by an event
            Ignored,
            Other,
        }
        let matches = |code: u8, prefix: &[u8]| {
            event.first() == Some(&code) && event.get(2..).is_some_and(|p| p.starts_with(prefix))
        };
        let is = match (&self.wait, parse_event(event)) {
            // The answer is recorded whatever its status: the script goes
            // on, as a probe would
            (Wait::Answer { opcode }, HciEvent::Complete { opcode: o, .. } | HciEvent::Status { opcode: o, .. })
                if *opcode == o =>
            {
                Is::Answer
            }
            (Wait::Event { code, prefix }, _) if matches(*code, prefix) => Is::Answer,
            (
                Wait::Upload { opcode, ack, .. },
                HciEvent::Complete { opcode: o, status, .. } | HciEvent::Status { opcode: o, status },
            ) if *opcode == o => match (status, ack) {
                (0, Ack::Complete) => Is::UploadAck,
                (0, Ack::Event { .. }) => Is::Ignored,
                (s, _) => Is::UploadFailed(o, s),
            },
            (Wait::Upload { ack: Ack::Event { code, prefix }, .. }, _) if matches(*code, prefix) => Is::UploadAck,
            _ => Is::Other,
        };
        match is {
            Is::Answer => self.succeed(Some(event.to_vec())),
            Is::UploadAck => self.upload_acked(event),
            Is::UploadFailed(o, s) => {
                self.step().answer = Some(event.to_vec());
                self.fail(format!("command 0x{o:04X} answered with status 0x{s:02X}"))
            }
            Is::Ignored => Vec::new(),
            Is::Other => self.other_event(event),
        }
    }

    fn on_timeout(&mut self) -> Vec<Action> {
        if self.done {
            return Vec::new();
        }
        match self.wait {
            Wait::Pause => self.succeed(None),
            Wait::Answer { .. } => self.fail("no answer".into()),
            Wait::Event { .. } => self.fail("the event did not come".into()),
            Wait::Upload { next, .. } => self.fail(format!("no acknowledgment of command {next}")),
            Wait::None => Vec::new(),
        }
    }

    fn on_control(&mut self, result: Result<Vec<u8>, String>) -> Vec<Action> {
        if self.done {
            return Vec::new();
        }
        match result {
            Ok(data) => self.succeed(Some(data)),
            Err(e) => self.fail(e),
        }
    }

    fn take_report(&mut self) -> Option<ProbeReport> {
        Some(std::mem::take(&mut self.report))
    }
}

/// The report's lines as text, for the console: "line 1: hci 1001  3 ms  0e0c..."
pub fn report_text(report: &ProbeReport) -> Vec<String> {
    report
        .steps
        .iter()
        .map(|s| {
            let outcome = match (&s.answer, &s.error) {
                (_, Some(e)) => format!("failed: {e}"),
                (Some(a), None) => hex_text(a),
                (None, None) => "ok".into(),
            };
            format!("{}  {} ms  {outcome}", s.name, s.millis)
        })
        .collect()
}
