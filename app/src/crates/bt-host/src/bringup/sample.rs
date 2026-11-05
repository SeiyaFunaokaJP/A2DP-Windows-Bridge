//! A sample chip loader with its tests: the starting point for a new
//! vendor (docs/chip-support.md, "Step 4: Write the loader")
//!
//! It shows every piece a loader uses, for a made-up chip whose ROM is a
//! bootloader:
//! 1. bootloader mode on before the first command ([`Action::Bootloader`])
//! 2. ask the controller what runs (standard Read Local Version); done at
//!    once when the operational firmware runs already
//! 3. read a chip register over USB ([`Action::UsbControl`]) to pick the
//!    firmware file
//! 4. read the file ([`FileReader`]), check it, and send it in vendor
//!    commands on the bulk pipe ([`Action::BulkCommand`]), each acknowledged
//!    with a Command Complete
//! 5. start the firmware: a Command Status, then a vendor event with the
//!    result (two waits for one command; a bad result is
//!    [`LoadError::Rejected`])
//! 6. finish ([`Action::Done`]): bootloader mode off when the operational
//!    firmware runs, left on after a failure (the chip is still in its
//!    bootloader)
//!
//! Every opcode, request, register, event and file name marked "made up"
//! is invented: no chip takes them. Compiled for the tests only, it never
//! reaches a controller. Copy the file to `bt-host/src/<vendor>.rs`, set
//! `vendor()`, replace what is made up with what the chip's probe report
//! and captures show (and drop what the chip does not need, such as
//! bootloader mode or the bulk pipe), and keep the tests, with a simulated
//! chip that answers what the real one did.

use std::time::Instant;

use crate::bringup::{
    Action, ChipLoader, ControlData, ControlKind, ControlRecipient, ControlRequest, FileReader, HciEvent, LoadError,
    Outcome, hci_command, parse_event,
};

const OP_READ_LOCAL_VERSION: u16 = 0x1001;
/// Made up: "download a firmware chunk" (vendor command, OGF 0x3F), taken
/// on the bulk OUT pipe
const OP_DOWNLOAD: u16 = 0xFC20;
/// Made up: "start the downloaded firmware": Command Status, then
/// [`EVENT_STARTED`]
const OP_LAUNCH: u16 = 0xFC21;
/// Made up: the vendor event's first parameter when the firmware started;
/// the second is the result (0: running, else an error code)
const EVENT_STARTED: u8 = 0x07;

/// Made up: the LMP subversion the chip reports while it runs its ROM code
const ROM_SUBVERSION: u16 = 0x1234;
/// Made up: the vendor request that reads a chip register, and the register
const REQUEST_READ_REGISTER: u8 = 0x01;
const REGISTER_CHIP_ID: u16 = 0x0010;
/// Made up: every firmware file starts with this
const FILE_MAGIC: &[u8; 4] = b"SMPL";

/// Data per download command. Made up: its parameters are the chunk index
/// and the data (real chips often take a byte offset instead); HCI
/// parameters are at most 255 bytes
const CHUNK: usize = 252;

pub const COMMAND_TIMEOUT_MS: u32 = 2000;
/// Made up: how long the firmware takes to report that it started
pub const START_TIMEOUT_MS: u32 = 3000;

/// Where the loader is: each waiting state names what it waits for
enum State {
    Idle,
    /// Read Local Version sent
    Version,
    /// The chip ID transfer asked for
    ChipId,
    /// Download commands, one after the other's Command Complete
    Downloading {
        file: String,
        chunks: Vec<Vec<u8>>,
        next: usize,
    },
    /// The launch command sent: its Command Status next
    Launching {
        file: String,
    },
    /// Launch accepted: the "started" vendor event next
    Starting {
        file: String,
    },
    Done,
}

pub struct Loader {
    read_file: FileReader,
    state: State,
    started: Instant,
}

impl Loader {
    pub fn new(read_file: FileReader) -> Loader {
        Loader { read_file, state: State::Idle, started: Instant::now() }
    }

    /// The end. Success: the operational firmware runs, so bootloader mode
    /// goes off (HCI init may send HCI Reset again). A failure leaves it
    /// on: the chip is still in its bootloader.
    fn finish(&mut self, result: Result<Outcome, LoadError>) -> Vec<Action> {
        tracing::debug!("sample loader finished after {} ms: {result:?}", self.started.elapsed().as_millis());
        self.state = State::Done;
        let mut actions = Vec::new();
        if result.is_ok() {
            actions.push(Action::Bootloader(false));
        }
        actions.push(Action::Done(result));
        actions
    }

    fn on_version(&mut self, ret: &[u8]) -> Vec<Action> {
        // HCI version, revision (2), LMP version, company (2), LMP
        // subversion (2)
        let Some(subversion) = ret.get(6..8).map(|v| u16::from_le_bytes([v[0], v[1]])) else {
            return self.finish(Err(LoadError::BadVersion));
        };
        tracing::info!("sample chip: LMP subversion 0x{subversion:04X}");
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
        tracing::info!("sample chip: chip ID 0x{id:08X}");
        // The name only: the app also looks in the vendor's subfolder
        let file = format!("sample_{id:08x}.bin");
        let Some(data) = (self.read_file)(&file) else { return self.finish(Err(LoadError::MissingFile(file))) };
        // Check the whole file before sending any of it
        let Some(body) = data.strip_prefix(FILE_MAGIC).filter(|b| !b.is_empty()) else {
            return self.finish(Err(LoadError::BadFile { file, why: "not a sample firmware file" }));
        };
        // A bulk command has the same bytes as one on the control pipe
        let chunks: Vec<Vec<u8>> = body
            .chunks(CHUNK)
            .enumerate()
            .map(|(i, part)| {
                let mut params = vec![i as u8];
                params.extend_from_slice(part);
                hci_command(OP_DOWNLOAD, &params)
            })
            .collect();
        tracing::info!("sample loader: {file}, {} bytes in {} commands", body.len(), chunks.len());
        let first = chunks[0].clone();
        self.state = State::Downloading { file, chunks, next: 1 };
        vec![Action::BulkCommand(first), Action::Timer(COMMAND_TIMEOUT_MS)]
    }

    /// A successful Command Complete or Command Status (the acknowledgment
    /// of a command whose result comes later)
    fn on_acknowledged(&mut self, opcode: u16, ret: &[u8]) -> Vec<Action> {
        match (std::mem::replace(&mut self.state, State::Idle), opcode) {
            (State::Version, OP_READ_LOCAL_VERSION) => self.on_version(ret),
            (State::Downloading { file, chunks, next }, OP_DOWNLOAD) => match chunks.get(next) {
                Some(c) => {
                    let c = c.clone();
                    self.state = State::Downloading { file, chunks, next: next + 1 };
                    vec![Action::BulkCommand(c), Action::Timer(COMMAND_TIMEOUT_MS)]
                }
                None => {
                    self.state = State::Launching { file };
                    vec![Action::Command(hci_command(OP_LAUNCH, &[])), Action::Timer(COMMAND_TIMEOUT_MS)]
                }
            },
            // Accepted; the firmware says itself when it runs
            (State::Launching { file }, OP_LAUNCH) => {
                self.state = State::Starting { file };
                vec![Action::Timer(START_TIMEOUT_MS)]
            }
            // Not the answer the state waits for (a late one): ignored
            (state, _) => {
                self.state = state;
                Vec::new()
            }
        }
    }

    fn on_started(&mut self, result: u8) -> Vec<Action> {
        match std::mem::replace(&mut self.state, State::Idle) {
            State::Starting { file } if result == 0 => {
                self.finish(Ok(Outcome::Loaded { firmware: file, detail: None }))
            }
            State::Starting { .. } => self.finish(Err(LoadError::Rejected(result))),
            // Not waiting for it (the chip restarted on its own)
            state => {
                self.state = state;
                Vec::new()
            }
        }
    }
}

impl ChipLoader for Loader {
    fn vendor(&self) -> &'static str {
        "Sample"
    }

    fn start(&mut self) -> Vec<Action> {
        self.started = Instant::now();
        self.state = State::Version;
        // Before the first command: the ROM may answer on the bulk pipe and
        // must not get an HCI Reset if the loader stops early
        vec![
            Action::Bootloader(true),
            Action::Command(hci_command(OP_READ_LOCAL_VERSION, &[])),
            Action::Timer(COMMAND_TIMEOUT_MS),
        ]
    }

    fn on_event(&mut self, event: &[u8]) -> Vec<Action> {
        if matches!(self.state, State::Idle | State::Done) {
            return Vec::new();
        }
        match parse_event(event) {
            HciEvent::Complete { opcode, status: 0, ret } => self.on_acknowledged(opcode, ret),
            HciEvent::Status { opcode, status: 0 } => self.on_acknowledged(opcode, &[]),
            HciEvent::Complete { opcode, status, .. } | HciEvent::Status { opcode, status } => {
                self.finish(Err(LoadError::Command { opcode, status }))
            }
            HciEvent::Vendor(&[EVENT_STARTED, result, ..]) => self.on_started(result),
            _ => Vec::new(),
        }
    }

    fn on_timeout(&mut self) -> Vec<Action> {
        let step = match self.state {
            State::Version => "Read Local Version",
            State::Downloading { .. } => "download",
            State::Launching { .. } => "launch",
            State::Starting { .. } => "firmware start",
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

    /// For the Adapters screen while the adapter starts
    fn step(&self) -> &'static str {
        match self.state {
            State::Version => "version",
            State::ChipId => "chip ID",
            State::Downloading { .. } => "download",
            State::Launching { .. } | State::Starting { .. } => "start",
            State::Idle | State::Done => "",
        }
    }

    /// The download: commands sent of all
    fn progress(&self) -> Option<(usize, usize)> {
        match &self.state {
            State::Downloading { chunks, next, .. } => Some((*next, chunks.len())),
            _ => None,
        }
    }
}

/// The tests drive the loader with [`crate::bringup::sim::run`], which runs
/// it as the app does, against a simulated chip: copy them along with the
/// loader and replace the chip's answers with the real one's.
#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::bringup::sim::{self, SimController, command_complete, command_status, vendor_event};

    /// What the made-up chip does
    struct Chip {
        rom: bool,
        chip_id: Result<u32, String>,
        /// Answer chunk N with this status
        reject_chunk: Option<(u8, u8)>,
        /// Commands it does not answer
        silent: Option<u16>,
        /// The "started" event's result; None: no event
        start_result: Option<u8>,
    }

    impl Chip {
        fn new() -> Chip {
            Chip { rom: true, chip_id: Ok(0x0000_7961), reject_chunk: None, silent: None, start_result: Some(0) }
        }
    }

    impl SimController for Chip {
        fn command(&mut self, c: &[u8]) -> Vec<Vec<u8>> {
            let opcode = sim::opcode(c);
            if self.silent == Some(opcode) {
                return Vec::new();
            }
            match opcode {
                OP_READ_LOCAL_VERSION => {
                    let sub = if self.rom { ROM_SUBVERSION } else { 0x5678 }.to_le_bytes();
                    vec![command_complete(opcode, 0, &[12, 0, 0, 12, 0xFF, 0xFF, sub[0], sub[1]])]
                }
                OP_LAUNCH => {
                    let mut events = vec![command_status(opcode, 0)];
                    events.extend(self.start_result.map(|r| vendor_event(&[EVENT_STARTED, r])));
                    events
                }
                _ => vec![command_complete(opcode, 0, &[])],
            }
        }

        fn bulk_command(&mut self, c: &[u8]) -> Vec<Vec<u8>> {
            let opcode = sim::opcode(c);
            assert_eq!(opcode, OP_DOWNLOAD, "only the download goes on the bulk pipe");
            if self.silent == Some(opcode) {
                return Vec::new();
            }
            match self.reject_chunk {
                Some((index, status)) if sim::params(c)[0] == index => vec![command_complete(opcode, status, &[])],
                _ => vec![command_complete(opcode, 0, &[])],
            }
        }

        fn control(&mut self, r: &ControlRequest) -> Result<Vec<u8>, String> {
            assert_eq!((r.kind, r.request, r.index), (ControlKind::Vendor, REQUEST_READ_REGISTER, REGISTER_CHIP_ID));
            self.chip_id.clone().map(|id| id.to_le_bytes().to_vec())
        }
    }

    fn run(chip: &mut Chip, files: HashMap<&'static str, Vec<u8>>) -> sim::SimRun {
        let mut loader = Loader::new(Box::new(move |name| files.get(name).cloned()));
        sim::run(&mut loader, chip)
    }

    fn firmware(len: usize) -> HashMap<&'static str, Vec<u8>> {
        let mut data = FILE_MAGIC.to_vec();
        data.extend((0..len).map(|i| i as u8));
        HashMap::from([("sample_00007961.bin", data)])
    }

    #[test]
    fn loads_the_firmware_in_chunks_then_starts_it() {
        let r = run(&mut Chip::new(), firmware(600));
        assert_eq!(r.result, Ok(Outcome::Loaded { firmware: "sample_00007961.bin".into(), detail: None }));
        // 600 bytes: 252 + 252 + 96, on the bulk pipe
        assert_eq!(r.opcodes(), [OP_READ_LOCAL_VERSION, OP_DOWNLOAD, OP_DOWNLOAD, OP_DOWNLOAD, OP_LAUNCH]);
        let bulk: Vec<bool> = r.commands.iter().map(|c| c.bulk).collect();
        assert_eq!(bulk, [false, true, true, true, false]);
        let last: Vec<u8> = [&[2u8][..], &(504..600).map(|i| i as u8).collect::<Vec<_>>()].concat();
        assert_eq!(r.commands[3].params(), last);
        assert_eq!(r.controls.len(), 1);
        assert_eq!(r.timeouts, 0);
        assert!(!r.bootloader, "bootloader mode off: the firmware runs");
    }

    #[test]
    fn nothing_to_do_when_the_firmware_runs() {
        let r = run(&mut Chip { rom: false, ..Chip::new() }, HashMap::new());
        assert_eq!(r.result, Ok(Outcome::AlreadyRunning));
        assert_eq!(r.opcodes(), [OP_READ_LOCAL_VERSION]);
        assert!(!r.bootloader);
    }

    #[test]
    fn names_the_missing_file() {
        let r = run(&mut Chip::new(), HashMap::new());
        assert_eq!(r.result, Err(LoadError::MissingFile("sample_00007961.bin".into())));
        assert!(r.bootloader, "still in its bootloader");
    }

    #[test]
    fn checks_the_file_before_sending_any_of_it() {
        let r = run(&mut Chip::new(), HashMap::from([("sample_00007961.bin", b"MZ\x90\x00".to_vec())]));
        assert!(matches!(r.result, Err(LoadError::BadFile { .. })), "{:?}", r.result);
        assert_eq!(r.opcodes(), [OP_READ_LOCAL_VERSION], "nothing of it sent");
    }

    #[test]
    fn stops_at_a_rejected_chunk() {
        let r = run(&mut Chip { reject_chunk: Some((1, 0x12)), ..Chip::new() }, firmware(600));
        assert_eq!(r.result, Err(LoadError::Command { opcode: OP_DOWNLOAD, status: 0x12 }));
        assert_eq!(r.opcodes().len(), 3, "no chunk after the rejected one");
        assert!(r.bootloader);
    }

    #[test]
    fn a_firmware_that_fails_to_start_is_rejected() {
        let r = run(&mut Chip { start_result: Some(0x05), ..Chip::new() }, firmware(10));
        assert_eq!(r.result, Err(LoadError::Rejected(0x05)));
        assert!(r.bootloader);
    }

    #[test]
    fn a_silent_controller_times_out() {
        let r = run(&mut Chip { silent: Some(OP_DOWNLOAD), ..Chip::new() }, firmware(10));
        assert_eq!(r.result, Err(LoadError::Timeout("download")));
    }

    #[test]
    fn a_firmware_that_never_says_it_started_times_out() {
        let r = run(&mut Chip { start_result: None, ..Chip::new() }, firmware(10));
        assert_eq!(r.result, Err(LoadError::Timeout("firmware start")));
    }

    #[test]
    fn a_failed_usb_transfer_ends_it() {
        // Over H4 (the emulator) every control transfer fails like this
        let r = run(&mut Chip { chip_id: Err("not a USB transport".into()), ..Chip::new() }, firmware(10));
        assert!(matches!(r.result, Err(LoadError::Host(_))));
    }

    #[test]
    fn tells_its_step_and_progress_for_the_adapters_screen() {
        let files = firmware(600);
        let mut loader = Loader::new(Box::new(move |name| files.get(name).cloned()));
        loader.start();
        assert_eq!((loader.step(), loader.progress()), ("version", None));
        let sub = ROM_SUBVERSION.to_le_bytes();
        loader.on_event(&command_complete(OP_READ_LOCAL_VERSION, 0, &[12, 0, 0, 12, 0xFF, 0xFF, sub[0], sub[1]]));
        assert_eq!(loader.step(), "chip ID");
        loader.on_control(Ok(0x7961u32.to_le_bytes().to_vec()));
        assert_eq!((loader.step(), loader.progress()), ("download", Some((1, 3))));
        loader.on_event(&command_complete(OP_DOWNLOAD, 0, &[]));
        assert_eq!(loader.progress(), Some((2, 3)));
    }
}
