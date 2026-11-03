//! A sample chip loader with its tests: the starting point for a new
//! vendor (docs/chip-support.md, "Write the loader")
//!
//! It shows every piece a loader uses, for a made-up chip:
//! 1. ask the controller what runs (standard Read Local Version)
//! 2. done at once when the operational firmware runs already
//! 3. read a chip register over USB ([`Action::UsbControl`]) to pick the
//!    firmware file
//! 4. read the file ([`FileReader`]) and send it in vendor commands, each
//!    acknowledged with a Command Complete
//! 5. start the firmware and finish ([`Action::Done`])
//!
//! Every opcode, request and file name below is invented for the example:
//! no chip takes them. Compiled for the tests only, it never reaches a
//! controller. Copy the file to `<vendor>.rs`, replace the made-up parts with
//! what the chip's probe reports and captures show, and keep the test style:
//! a simulated controller that answers what the real one did.

use std::time::Instant;

use crate::bringup::{
    Action, ChipLoader, ControlData, ControlKind, ControlRecipient, ControlRequest, FileReader, LoadError, Outcome,
};

const OP_READ_LOCAL_VERSION: u16 = 0x1001;
/// Made up: "download a firmware chunk" (vendor command, OGF 0x3F)
const OP_DOWNLOAD: u16 = 0xFC20;
/// Made up: "start the downloaded firmware"
const OP_LAUNCH: u16 = 0xFC21;

const EVENT_COMMAND_COMPLETE: u8 = 0x0E;
const EVENT_COMMAND_STATUS: u8 = 0x0F;

/// Made up: the LMP subversion the chip reports while it runs its ROM code
const ROM_SUBVERSION: u16 = 0x1234;
/// Made up: the vendor request that reads a chip register
const REQUEST_READ_REGISTER: u8 = 0x01;
const REGISTER_CHIP_ID: u16 = 0x0010;

/// Data per download command: its parameter is the chunk index and up to
/// this many bytes (HCI parameters are at most 255 bytes)
const CHUNK: usize = 252;

pub const COMMAND_TIMEOUT_MS: u32 = 2000;

/// Where the loader is
enum State {
    Idle,
    Version,
    ChipId,
    Downloading { file: String, chunks: Vec<Vec<u8>>, next: usize },
    Launching { file: String },
    Done,
}

pub struct SampleLoader {
    read_file: FileReader,
    state: State,
    started: Instant,
}

/// HCI command: opcode, length, parameters
fn command(opcode: u16, params: &[u8]) -> Vec<u8> {
    let mut c = opcode.to_le_bytes().to_vec();
    c.push(params.len() as u8);
    c.extend_from_slice(params);
    c
}

/// The opcode and return parameters of a Command Complete, or the opcode and
/// status of a failed Command Status
enum Answer<'a> {
    Complete(u16, &'a [u8]),
    Failed(u16, u8),
    Other,
}

fn answer(event: &[u8]) -> Answer<'_> {
    match event {
        [EVENT_COMMAND_COMPLETE, _, _, lo, hi, ret @ ..] => Answer::Complete(u16::from_le_bytes([*lo, *hi]), ret),
        [EVENT_COMMAND_STATUS, _, status, _, lo, hi, ..] if *status != 0 => {
            Answer::Failed(u16::from_le_bytes([*lo, *hi]), *status)
        }
        _ => Answer::Other,
    }
}

impl SampleLoader {
    pub fn new(read_file: FileReader) -> SampleLoader {
        SampleLoader { read_file, state: State::Idle, started: Instant::now() }
    }

    fn finish(&mut self, result: Result<Outcome, LoadError>) -> Vec<Action> {
        tracing::debug!("sample loader finished after {} ms: {result:?}", self.started.elapsed().as_millis());
        self.state = State::Done;
        vec![Action::Done(result)]
    }

    fn on_version(&mut self, ret: &[u8]) -> Vec<Action> {
        // Status, HCI version, revision (2), LMP version, company (2), LMP
        // subversion (2)
        let Some(subversion) = ret.get(7..9).map(|v| u16::from_le_bytes([v[0], v[1]])) else {
            return self.finish(Err(LoadError::BadVersion));
        };
        if subversion != ROM_SUBVERSION {
            return self.finish(Ok(Outcome::AlreadyRunning));
        }
        self.state = State::ChipId;
        vec![Action::UsbControl(ControlRequest {
            kind: ControlKind::Vendor,
            recipient: ControlRecipient::Device,
            request: REQUEST_READ_REGISTER,
            value: 0,
            index: REGISTER_CHIP_ID,
            data: ControlData::In(4),
        })]
    }

    fn on_chip_id(&mut self, id: u32) -> Vec<Action> {
        let file = format!("sample_{id:08x}.bin");
        let Some(data) = (self.read_file)(&file) else { return self.finish(Err(LoadError::MissingFile(file))) };
        if data.is_empty() {
            return self.finish(Err(LoadError::BadFile { file, why: "empty" }));
        }
        let chunks: Vec<Vec<u8>> = data
            .chunks(CHUNK)
            .enumerate()
            .map(|(i, part)| {
                let mut params = vec![i as u8];
                params.extend_from_slice(part);
                command(OP_DOWNLOAD, &params)
            })
            .collect();
        tracing::info!("sample loader: {file}, {} bytes in {} commands", data.len(), chunks.len());
        let first = chunks[0].clone();
        self.state = State::Downloading { file, chunks, next: 1 };
        vec![Action::Command(first), Action::Timer(COMMAND_TIMEOUT_MS)]
    }
}

impl ChipLoader for SampleLoader {
    fn vendor(&self) -> &'static str {
        "Sample"
    }

    fn start(&mut self) -> Vec<Action> {
        self.started = Instant::now();
        self.state = State::Version;
        vec![Action::Command(command(OP_READ_LOCAL_VERSION, &[])), Action::Timer(COMMAND_TIMEOUT_MS)]
    }

    fn on_event(&mut self, event: &[u8]) -> Vec<Action> {
        let (opcode, ret) = match answer(event) {
            Answer::Complete(opcode, ret) => (opcode, ret),
            Answer::Failed(opcode, status) => return self.finish(Err(LoadError::Command { opcode, status })),
            Answer::Other => return Vec::new(),
        };
        let status = ret.first().copied().unwrap_or(0xFF);
        if status != 0 {
            return self.finish(Err(LoadError::Command { opcode, status }));
        }
        match (std::mem::replace(&mut self.state, State::Idle), opcode) {
            (State::Version, OP_READ_LOCAL_VERSION) => self.on_version(ret),
            (State::Downloading { file, chunks, next }, OP_DOWNLOAD) => match chunks.get(next) {
                Some(c) => {
                    let c = c.clone();
                    self.state = State::Downloading { file, chunks, next: next + 1 };
                    vec![Action::Command(c), Action::Timer(COMMAND_TIMEOUT_MS)]
                }
                None => {
                    self.state = State::Launching { file };
                    vec![Action::Command(command(OP_LAUNCH, &[])), Action::Timer(COMMAND_TIMEOUT_MS)]
                }
            },
            (State::Launching { file }, OP_LAUNCH) => self.finish(Ok(Outcome::Loaded { firmware: file, detail: None })),
            // Not the answer the state waits for (a late one): ignored
            (state, _) => {
                self.state = state;
                Vec::new()
            }
        }
    }

    fn on_timeout(&mut self) -> Vec<Action> {
        let step = match self.state {
            State::Version => "Read Local Version",
            State::Downloading { .. } => "download",
            State::Launching { .. } => "launch",
            State::Idle | State::ChipId | State::Done => return Vec::new(),
        };
        self.finish(Err(LoadError::Timeout(step)))
    }

    fn on_control(&mut self, result: Result<Vec<u8>, String>) -> Vec<Action> {
        if !matches!(self.state, State::ChipId) {
            return Vec::new();
        }
        match result {
            Ok(d) if d.len() == 4 => self.on_chip_id(u32::from_le_bytes([d[0], d[1], d[2], d[3]])),
            Ok(_) => self.finish(Err(LoadError::Unsupported("chip ID register of another size"))),
            Err(e) => {
                tracing::warn!("sample loader: reading the chip ID failed: {e}");
                self.finish(Err(LoadError::Host("the chip ID could not be read over USB")))
            }
        }
    }

    fn missing_file_hint(&self, file: &str) -> String {
        format!("{file} (the sample's made-up firmware)")
    }
}

/// The tests drive the loader as the app does (crate::app run_loader)
/// against a simulated controller: copy them along with the loader.
#[cfg(test)]
mod tests {
    use std::collections::{HashMap, VecDeque};

    use super::*;

    /// What the made-up chip does
    struct Chip {
        rom: bool,
        chip_id: Result<u32, String>,
        /// Answer chunk N with this status
        reject_chunk: Option<(u8, u8)>,
        /// Commands it does not answer
        silent: Option<u16>,
        received: Vec<Vec<u8>>,
    }

    impl Chip {
        fn new() -> Chip {
            Chip { rom: true, chip_id: Ok(0x0000_7961), reject_chunk: None, silent: None, received: Vec::new() }
        }

        fn complete(opcode: u16, ret: &[u8]) -> Vec<u8> {
            let [lo, hi] = opcode.to_le_bytes();
            let mut e = vec![EVENT_COMMAND_COMPLETE, (3 + ret.len()) as u8, 1, lo, hi];
            e.extend_from_slice(ret);
            e
        }

        /// The events for a command; empty: silent
        fn command(&mut self, c: &[u8]) -> Vec<Vec<u8>> {
            self.received.push(c.to_vec());
            let opcode = u16::from_le_bytes([c[0], c[1]]);
            if self.silent == Some(opcode) {
                return Vec::new();
            }
            let ret = match opcode {
                OP_READ_LOCAL_VERSION => {
                    let sub = if self.rom { ROM_SUBVERSION } else { 0x5678 }.to_le_bytes();
                    vec![0, 12, 0, 0, 12, 0xFF, 0xFF, sub[0], sub[1]]
                }
                OP_DOWNLOAD => match self.reject_chunk {
                    Some((index, status)) if c[3] == index => vec![status],
                    _ => vec![0],
                },
                _ => vec![0],
            };
            vec![Chip::complete(opcode, &ret)]
        }

        fn control(&mut self, r: &ControlRequest) -> Result<Vec<u8>, String> {
            assert_eq!((r.kind, r.request, r.index), (ControlKind::Vendor, REQUEST_READ_REGISTER, REGISTER_CHIP_ID));
            self.chip_id.clone().map(|id| id.to_le_bytes().to_vec())
        }
    }

    /// Runs the loader to its end -> its result
    fn run(chip: &mut Chip, files: HashMap<&'static str, Vec<u8>>) -> Result<Outcome, LoadError> {
        let mut loader = SampleLoader::new(Box::new(move |name| files.get(name).cloned()));
        let mut actions: VecDeque<Action> = loader.start().into();
        let mut timer = false;
        loop {
            let Some(action) = actions.pop_front() else {
                // Nothing to do but wait: the timer fires
                assert!(timer, "the loader waits without a timer");
                timer = false;
                actions.extend(loader.on_timeout());
                continue;
            };
            match action {
                Action::Command(c) => {
                    for e in chip.command(&c) {
                        actions.extend(loader.on_event(&e));
                    }
                }
                Action::UsbControl(r) => {
                    let result = chip.control(&r);
                    for a in loader.on_control(result).into_iter().rev() {
                        actions.push_front(a);
                    }
                }
                Action::Timer(_) => timer = true,
                Action::Done(result) => return result,
                other => panic!("unexpected {other:?}"),
            }
        }
    }

    fn firmware(len: usize) -> HashMap<&'static str, Vec<u8>> {
        HashMap::from([("sample_00007961.bin", (0..len).map(|i| i as u8).collect())])
    }

    #[test]
    fn loads_the_firmware_in_chunks_then_launches_it() {
        let mut chip = Chip::new();
        let r = run(&mut chip, firmware(600));
        assert_eq!(r, Ok(Outcome::Loaded { firmware: "sample_00007961.bin".into(), detail: None }));
        let ops: Vec<u16> = chip.received.iter().map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        // 600 bytes: 252 + 252 + 96
        assert_eq!(ops, [OP_READ_LOCAL_VERSION, OP_DOWNLOAD, OP_DOWNLOAD, OP_DOWNLOAD, OP_LAUNCH]);
        assert_eq!(chip.received[3][2] as usize, 1 + 96);
        assert_eq!(chip.received[3][3], 2, "chunk index");
    }

    #[test]
    fn nothing_to_do_when_the_firmware_runs() {
        let mut chip = Chip { rom: false, ..Chip::new() };
        assert_eq!(run(&mut chip, HashMap::new()), Ok(Outcome::AlreadyRunning));
        assert_eq!(chip.received.len(), 1);
    }

    #[test]
    fn names_the_missing_file() {
        let mut chip = Chip::new();
        assert_eq!(run(&mut chip, HashMap::new()), Err(LoadError::MissingFile("sample_00007961.bin".into())));
    }

    #[test]
    fn stops_at_a_rejected_chunk() {
        let mut chip = Chip { reject_chunk: Some((1, 0x12)), ..Chip::new() };
        assert_eq!(run(&mut chip, firmware(600)), Err(LoadError::Command { opcode: OP_DOWNLOAD, status: 0x12 }));
    }

    #[test]
    fn a_silent_controller_times_out() {
        let mut chip = Chip { silent: Some(OP_LAUNCH), ..Chip::new() };
        assert_eq!(run(&mut chip, firmware(10)), Err(LoadError::Timeout("launch")));
    }

    #[test]
    fn a_failed_usb_transfer_ends_it() {
        // Over H4 (the emulator) every control transfer fails like this
        let mut chip = Chip { chip_id: Err("not a USB transport".into()), ..Chip::new() };
        assert!(matches!(run(&mut chip, firmware(10)), Err(LoadError::Host(_))));
    }
}
