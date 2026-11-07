//! Qualcomm (Atheros) USB rampatch and NVM download (experimental)
//!
//! A Qualcomm USB Bluetooth chip (QCA61x4 / QCA9377 "ROME", WCN6855
//! "FastConnect 6900") runs its ROM after power-up. Before it is used, the
//! host sends it a patch for that ROM (the rampatch) and its board settings
//! (the NVM). All of it goes over USB, not HCI:
//!
//! - vendor control requests on the default pipe: 0x05 reads a status byte
//!   (bit 7: the rampatch is in, bit 6: the NVM is in), 0x09 reads the
//!   20-byte version: ROM version, patch version, RAM (SoC) version (u32
//!   each), chip ID, platform ID, a flag word (high byte 0x80: the chip and
//!   platform IDs name its board), 4 reserved bytes (the second 0x23 on a
//!   "TF" ROME, which has files of its own)
//! - a download is one file: its first bytes (the header: 28 on ROME, 40 on
//!   the WCN6855, 4 for an NVM) in vendor request 0x01 (OUT), the rest on
//!   the bulk OUT pipe (the ACL data pipe): three transfers of 320 bytes,
//!   10 ms apart, then the rest. The status byte then tells whether the chip
//!   took it. The bulk parts go out with [`Action::BulkCommand`], which
//!   sends its bytes as they are (the HCI capture lists them as commands);
//!   its writes are not waited for, so the status is read until it shows
//!   the file.
//!
//! The files are TLV: a type byte (1 rampatch, 2 NVM) and a 24-bit length.
//! A rampatch tells its ROM at 0x10 (ROME: the low half at 0x12 only) and
//! its patch version at 0x14; it is sent only to that ROM, and only when it
//! is newer than the patch the chip reports.
//!
//! What runs: both status bits set means the firmware is in already (the
//! chip keeps it until it loses power), and nothing is sent.
//!
//! Read from Qualcomm's Windows driver packages (BtFilter.sys and the files
//! they ship: 10.0.0.1272 for ROME, 2.0.0.1400 and 2.0.0.1292 for the
//! WCN6855); written without vendor or GPL source. The driver sends its
//! vendor requests to the endpoint recipient; Linux's btusb, which loads
//! the same files, to the device. The chip ignores it: the loader uses the
//! device recipient, which WinUSB passes as it is.
//!
//! Firmware: the firmware folder first (also its `qca\` subfolder), by the
//! Windows driver's names (`AthrBT_0x00000302.dfu`,
//! `ramps_0x00000302_48.dfu`) or linux-firmware's (`rampatch_usb_00000302.bin`,
//! `nvm_usb_00000302.bin`), then the files Qualcomm's Windows driver
//! installed (System32\drivers or the driver store, see
//! [`from_windows_driver`]): a PC with such an adapter has them even after
//! it moved to WinUSB.

use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::bringup::{
    Action, ChipLoader, ControlData, ControlKind, ControlRecipient, ControlRequest, FileReader, LoadError, Outcome,
};

/// Vendor requests: a download's header (OUT), the status byte, the
/// version (IN)
pub const REQUEST_DOWNLOAD: u8 = 0x01;
pub const REQUEST_STATUS: u8 = 0x05;
pub const REQUEST_VERSION: u8 = 0x09;
pub const VERSION_LEN: u16 = 20;

/// Status bits: the rampatch, the NVM is in
pub const STATUS_PATCHED: u8 = 0x80;
pub const STATUS_NVM: u8 = 0x40;
/// Status values (low 6 bits) in which Qualcomm's driver does not load
/// the chip
const STATUS_REFUSED: [u8; 2] = [0x0B, 0x0C];

/// File types (the TLV's first byte)
const TYPE_RAMPATCH: u8 = 1;
const TYPE_NVM: u8 = 2;
const NVM_HEADER: usize = 4;

/// The first bulk transfers of a download are this short, 10 ms apart, as
/// Qualcomm's driver sends them; then the rest at once
pub const FIRST_CHUNK: usize = 320;
const FIRST_CHUNKS: usize = 3;
const REST_CHUNK: usize = 0x7FFFC;
const CHUNK_GAP_MS: u32 = 10;
/// Before the version read (after the status read), and between the end of
/// a download and each status read that checks it
const VERSION_WAIT_MS: u32 = 10;
pub const SETTLE_MS: u32 = 100;
/// Status reads before a download counts as not taken. The bulk writes
/// are not waited for, so this also covers the transfer (a 180 KB
/// rampatch at full speed)
const MAX_POLLS: u32 = 30;

/// A ROM the loader knows: its rampatch header, and whether it is a ROME
/// (the file names its ROM by the low half)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rom {
    version: u32,
    rampatch_header: usize,
    rome: bool,
}

/// The ROMs whose files Qualcomm's Windows drivers ship: ROME 2.0, 3.0,
/// 3.2 (QCA61x4, QCA9377) and the WCN6855 2.0 and 2.1. ROME 1.x (another
/// layout) and the WCN785x (no Windows files seen) are refused.
const ROMS: &[Rom] = &[
    Rom { version: 0x0000_0200, rampatch_header: 28, rome: true },
    Rom { version: 0x0000_0300, rampatch_header: 28, rome: true },
    Rom { version: 0x0000_0302, rampatch_header: 28, rome: true },
    Rom { version: 0x0013_0200, rampatch_header: 40, rome: false },
    Rom { version: 0x0013_0201, rampatch_header: 40, rome: false },
];

/// The version the chip reports (request 0x09)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Version {
    pub rom: u32,
    pub patch: u32,
    pub ram: u32,
    pub chip_id: u8,
    pub platform_id: u8,
    pub flag: u16,
    /// A "TF" ROME (reserved byte 1 is 0x23)
    pub tf: bool,
}

impl Version {
    pub fn parse(d: &[u8]) -> Option<Version> {
        let d = d.get(..VERSION_LEN as usize)?;
        let word = |at: usize| u32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]]);
        Some(Version {
            rom: word(0),
            patch: word(4),
            ram: word(8),
            chip_id: d[12],
            platform_id: d[13],
            flag: u16::from_le_bytes([d[14], d[15]]),
            tf: d[17] == 0x23,
        })
    }

    /// The board, when the chip names it (flag 0x80xx): chip and platform
    /// ID in hex ("010a")
    fn board(&self) -> Option<String> {
        (self.flag >> 8 == 0x80 && (self.chip_id, self.platform_id) != (0, 0))
            .then(|| format!("{:02x}{:02x}", self.chip_id, self.platform_id))
    }

    /// The WCN6855's "gf" variant, by its SoC version
    fn gf(&self) -> bool {
        matches!((self.rom, self.ram), (0x0013_0200, 0x400C_1200) | (0x0013_0201, 0x400C_1211))
    }

    /// The crystal (MHz) a ROME NVM is named after (flag 0): the chip ID
    fn crystal(&self) -> &'static str {
        match self.chip_id {
            0 => "_26",
            1 => "_40",
            2 => "_19",
            3 => "_24",
            4 => "_16",
            5 => "_37",
            6 => "_38",
            7 => "_48",
            8 => "_52",
            _ => "_0",
        }
    }
}

/// Adapters whose NVM Qualcomm's driver picks by board (DetectBoardId in
/// its INF)
const BOARD_ID_ADAPTERS: &[(u16, u16)] = &[(0x0CF3, 0xE600), (0x2C7C, 0x0137), (0x28DE, 0x1401)];

/// Adapters with an NVM of their own in Qualcomm's driver (its suffix)
const ADAPTER_NVM: &[(u16, u16, &str)] = &[
    (0x0CF3, 0xE302, "_tx8"),
    (0x0CF3, 0xE303, "_pars"),
    (0x0489, 0xE0FC, "_0112"),
    (0x2C7C, 0x0132, "_0112"),
    (0x0489, 0xE11F, "_010e"),
    (0x2C7C, 0x0131, "_010e"),
];

/// The rampatch files of a chip, first choice first: the Windows driver's,
/// then linux-firmware's (not for a TF chip, which has its own)
pub fn rampatch_names(v: &Version) -> Vec<String> {
    let tf = if v.tf { "TF_" } else { "" };
    let mut names = vec![format!("AthrBT_{tf}0x{:08x}.dfu", v.rom)];
    if !v.tf {
        names.push(format!("rampatch_usb_{:08x}.bin", v.rom));
    }
    names
}

/// The NVM files of a chip, first choice first. Qualcomm's driver names it
/// `ramps_[TF_]0x<ROM>[_gf]`, then the crystal (when the flag is 0) or the
/// board (for the adapters it detects it on), then the adapter's own
/// suffix; without that file, the one with neither. Then linux-firmware's:
/// `nvm_usb_<ROM>[_gf][_<board>].bin`.
pub fn nvm_names(v: &Version, usb: Option<(u16, u16)>) -> Vec<String> {
    let tf = if v.tf { "TF_" } else { "" };
    let gf = if v.gf() { "_gf" } else { "" };
    let base = format!("ramps_{tf}0x{:08x}{gf}", v.rom);
    let mut full = base.clone();
    if v.flag >> 8 == 0 {
        full.push_str(v.crystal());
    }
    if let (Some(board), Some(id)) = (v.board(), usb)
        && BOARD_ID_ADAPTERS.contains(&id)
    {
        full = format!("{full}_{board}");
    }
    if let Some(&(_, _, suffix)) = usb.and_then(|(vid, pid)| ADAPTER_NVM.iter().find(|a| (a.0, a.1) == (vid, pid))) {
        full.push_str(suffix);
    }
    let mut names = vec![format!("{full}.dfu")];
    if full != base {
        names.push(format!("{base}.dfu"));
    }
    if !v.tf {
        if let Some(board) = v.board() {
            names.push(format!("nvm_usb_{:08x}{gf}_{board}.bin", v.rom));
        }
        names.push(format!("nvm_usb_{:08x}{gf}.bin", v.rom));
    }
    names
}

/// The ROM and patch version a rampatch is for
fn rampatch_version(file: &[u8], rom: &Rom) -> Option<(u32, u16)> {
    let half = |at: usize| file.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let for_rom = if rom.rome { half(0x12)? as u32 } else { ((half(0x10)? as u32) << 16) | half(0x12)? as u32 };
    Some((for_rom, half(0x14)?))
}

/// A file's TLV header: its type, and that it holds what it says. Err: why
/// it is not a file of this type
fn check_tlv(file: &[u8], kind: u8, header: usize) -> Result<(), &'static str> {
    let [t, a, b, c, ..] = *file else { return Err("too short for a Qualcomm firmware file") };
    if t != kind {
        return Err(if kind == TYPE_RAMPATCH { "not a rampatch" } else { "not an NVM file" });
    }
    let len = u32::from_le_bytes([a, b, c, 0]) as usize;
    if len + 4 > file.len() || file.len() <= header {
        return Err("cut short");
    }
    Ok(())
}

fn vendor_in(request: u8, length: u16) -> ControlRequest {
    ControlRequest {
        kind: ControlKind::Vendor,
        recipient: ControlRecipient::Device,
        request,
        value: 0,
        index: 0,
        data: ControlData::In(length),
    }
}

pub fn read_status() -> ControlRequest {
    vendor_in(REQUEST_STATUS, 1)
}

pub fn read_version() -> ControlRequest {
    vendor_in(REQUEST_VERSION, VERSION_LEN)
}

fn send_header(header: &[u8]) -> ControlRequest {
    ControlRequest {
        kind: ControlKind::Vendor,
        recipient: ControlRecipient::Device,
        request: REQUEST_DOWNLOAD,
        value: 0,
        index: 0,
        data: ControlData::Out(header.to_vec()),
    }
}

/// One file to send: its name, how much goes in the control request, the
/// status bit that shows the chip took it
#[derive(Debug, Clone)]
struct Download {
    file: String,
    data: Vec<u8>,
    header: usize,
    done_bit: u8,
    what: &'static str,
}

impl Download {
    /// The bulk transfers after the header
    fn chunks(&self) -> Vec<Vec<u8>> {
        let body = &self.data[self.header..];
        let first = body.len().min(FIRST_CHUNK * FIRST_CHUNKS);
        let mut chunks: Vec<Vec<u8>> = body[..first].chunks(FIRST_CHUNK).map(<[u8]>::to_vec).collect();
        chunks.extend(body[first..].chunks(REST_CHUNK).map(<[u8]>::to_vec));
        chunks
    }
}

enum State {
    Idle,
    /// The status byte asked for
    Status,
    /// Waiting before the version read, then reading it
    VersionWait,
    Version,
    /// The device descriptor (the adapter's USB IDs) asked for
    Descriptor,
    /// A download's header sent in the control request
    Header,
    /// Its bulk transfers: the next one when the timer fires
    Body {
        next: usize,
    },
    /// Its end: a status read when the timer fires, `polls` of them so far
    Check {
        polls: u32,
    },
    Done,
}

pub struct Loader {
    read_file: FileReader,
    state: State,
    started: Instant,
    status: u8,
    version: Version,
    downloads: Vec<Download>,
    /// The download running (index in `downloads`), its bulk transfers
    current: usize,
    chunks: Vec<Vec<u8>>,
    /// Bytes sent of all downloads
    sent: usize,
    total: usize,
    /// The rampatch was in the chip already
    patched_before: bool,
}

impl Loader {
    pub fn new(read_file: FileReader) -> Loader {
        Loader {
            read_file,
            state: State::Idle,
            started: Instant::now(),
            status: 0,
            version: Version::default(),
            downloads: Vec::new(),
            current: 0,
            chunks: Vec::new(),
            sent: 0,
            total: 0,
            patched_before: false,
        }
    }

    fn finish(&mut self, result: Result<Outcome, LoadError>) -> Vec<Action> {
        tracing::debug!("qualcomm loader finished after {} ms: {result:?}", self.started.elapsed().as_millis());
        self.state = State::Done;
        vec![Action::Done(result)]
    }

    /// The first of `names` the reader has
    fn find(&mut self, names: &[String]) -> Option<(String, Vec<u8>)> {
        names.iter().find_map(|n| (self.read_file)(n).map(|d| (n.clone(), d)))
    }

    /// The rampatch to send: None when the chip's own is as new
    fn rampatch(&mut self, rom: &Rom) -> Result<Option<Download>, LoadError> {
        let names = rampatch_names(&self.version);
        let Some((file, data)) = self.find(&names) else { return Err(LoadError::MissingFile(names[0].clone())) };
        check_tlv(&data, TYPE_RAMPATCH, rom.rampatch_header)
            .map_err(|why| LoadError::BadFile { file: file.clone(), why })?;
        let (for_rom, patch) =
            rampatch_version(&data, rom).ok_or(LoadError::BadFile { file: file.clone(), why: "cut short" })?;
        tracing::info!("qualcomm: {file}: {} bytes, for ROM 0x{for_rom:08X}, patch version 0x{patch:04X}", data.len());
        if for_rom != rom.version {
            return Err(LoadError::BadFile { file, why: "made for another ROM version" });
        }
        if u32::from(patch) <= self.version.patch {
            tracing::info!("qualcomm: the chip's patch 0x{:04X} is as new as the file's: not sent", self.version.patch);
            return Ok(None);
        }
        Ok(Some(Download { file, data, header: rom.rampatch_header, done_bit: STATUS_PATCHED, what: "rampatch" }))
    }

    fn nvm(&mut self, usb: Option<(u16, u16)>) -> Result<Download, LoadError> {
        let names = nvm_names(&self.version, usb);
        let Some((file, data)) = self.find(&names) else { return Err(LoadError::MissingFile(names[0].clone())) };
        if file != names[0] {
            tracing::info!("qualcomm: {} not found, using {file}", names[0]);
        }
        check_tlv(&data, TYPE_NVM, NVM_HEADER).map_err(|why| LoadError::BadFile { file: file.clone(), why })?;
        tracing::info!("qualcomm: {file}: {} bytes", data.len());
        Ok(Download { file, data, header: NVM_HEADER, done_bit: STATUS_NVM, what: "NVM" })
    }

    /// Status, version and USB IDs known: picks and checks every file
    /// before sending any
    fn plan(&mut self, usb: Option<(u16, u16)>) -> Vec<Action> {
        let v = self.version;
        let (patched, nvm) = (self.status & STATUS_PATCHED != 0, self.status & STATUS_NVM != 0);
        let Some(rom) = ROMS.iter().find(|r| r.version == v.rom).copied() else {
            if patched && nvm {
                // Nothing would be sent anyway
                return self.finish(Ok(Outcome::AlreadyRunning));
            }
            tracing::warn!("qualcomm: ROM version 0x{:08X} is not one this loader knows", v.rom);
            return self.finish(Err(LoadError::Unsupported("a Qualcomm chip this loader does not know")));
        };
        if patched && nvm {
            return self.finish(Ok(Outcome::AlreadyRunning));
        }
        self.patched_before = patched;
        let mut downloads = Vec::new();
        if !patched {
            match self.rampatch(&rom) {
                Ok(Some(d)) => downloads.push(d),
                Ok(None) => self.patched_before = true,
                Err(e) => return self.finish(Err(e)),
            }
        }
        if !nvm {
            match self.nvm(usb) {
                Ok(d) => downloads.push(d),
                Err(e) => return self.finish(Err(e)),
            }
        }
        if downloads.is_empty() {
            return self.finish(Ok(Outcome::AlreadyRunning));
        }
        self.total = downloads.iter().map(|d| d.data.len()).sum();
        self.downloads = downloads;
        self.start_download(0)
    }

    fn start_download(&mut self, index: usize) -> Vec<Action> {
        self.current = index;
        let d = &self.downloads[index];
        tracing::info!("qualcomm: sending the {} {}, {} bytes", d.what, d.file, d.data.len());
        self.chunks = d.chunks();
        self.state = State::Header;
        vec![Action::UsbControl(send_header(&d.data[..d.header]))]
    }

    /// The next bulk transfer, or the check after the last
    fn body(&mut self, next: usize) -> Vec<Action> {
        match self.chunks.get(next).cloned() {
            Some(chunk) => {
                self.sent += chunk.len();
                self.state = State::Body { next: next + 1 };
                vec![Action::BulkCommand(chunk), Action::Timer(CHUNK_GAP_MS)]
            }
            None => {
                self.state = State::Check { polls: 0 };
                vec![Action::Timer(SETTLE_MS)]
            }
        }
    }

    fn on_status(&mut self, status: u8) -> Vec<Action> {
        match std::mem::replace(&mut self.state, State::Idle) {
            State::Status => {
                tracing::info!("qualcomm: status 0x{status:02X}");
                if STATUS_REFUSED.contains(&(status & 0x3F)) {
                    return self.finish(Err(LoadError::Unsupported(
                        "the chip reports a state Qualcomm's driver does not load it in",
                    )));
                }
                self.status = status;
                self.state = State::VersionWait;
                vec![Action::Timer(VERSION_WAIT_MS)]
            }
            State::Check { polls } => {
                let d = &self.downloads[self.current];
                if status & d.done_bit != 0 {
                    tracing::info!("qualcomm: the chip took the {} (status 0x{status:02X})", d.what);
                    let next = self.current + 1;
                    if next < self.downloads.len() {
                        return self.start_download(next);
                    }
                    let file = d.file.clone();
                    let detail = self.patched_before.then_some("its rampatch was in the chip already");
                    return self.finish(Ok(Outcome::Loaded { firmware: file, detail }));
                }
                if polls + 1 >= MAX_POLLS {
                    tracing::warn!("qualcomm: status 0x{status:02X} after the {}", d.what);
                    return self.finish(Err(LoadError::Timeout(d.what)));
                }
                self.state = State::Check { polls: polls + 1 };
                vec![Action::Timer(SETTLE_MS)]
            }
            state => {
                self.state = state;
                Vec::new()
            }
        }
    }
}

impl ChipLoader for Loader {
    fn vendor(&self) -> &'static str {
        "Qualcomm"
    }

    fn start(&mut self) -> Vec<Action> {
        self.started = Instant::now();
        self.state = State::Status;
        vec![Action::UsbControl(read_status())]
    }

    /// The chip runs its ROM, which answers HCI: nothing to wait for there
    fn on_event(&mut self, _event: &[u8]) -> Vec<Action> {
        Vec::new()
    }

    fn on_timeout(&mut self) -> Vec<Action> {
        match std::mem::replace(&mut self.state, State::Idle) {
            State::VersionWait => {
                self.state = State::Version;
                vec![Action::UsbControl(read_version())]
            }
            State::Body { next } => self.body(next),
            state @ State::Check { .. } => {
                self.state = state;
                vec![Action::UsbControl(read_status())]
            }
            state => {
                self.state = state;
                Vec::new()
            }
        }
    }

    fn on_control(&mut self, result: Result<Vec<u8>, String>) -> Vec<Action> {
        match (&self.state, result) {
            (State::Status | State::Check { .. }, Ok(d)) if d.len() == 1 => self.on_status(d[0]),
            (State::Version, Ok(d)) => match Version::parse(&d) {
                Some(v) => {
                    tracing::info!(
                        "qualcomm: ROM 0x{:08X}, patch 0x{:08X}, SoC 0x{:08X}, chip {:02X} platform {:02X}, flag 0x{:04X}{}",
                        v.rom,
                        v.patch,
                        v.ram,
                        v.chip_id,
                        v.platform_id,
                        v.flag,
                        if v.tf { ", TF" } else { "" }
                    );
                    self.version = v;
                    self.state = State::Descriptor;
                    vec![Action::UsbControl(ControlRequest::device_descriptor())]
                }
                None => self.finish(Err(LoadError::BadVersion)),
            },
            // The USB IDs only pick an adapter's own NVM: without them, the
            // chip's
            (State::Descriptor, result) => {
                let usb = match result {
                    Ok(d) if d.len() >= 12 => {
                        Some((u16::from_le_bytes([d[8], d[9]]), u16::from_le_bytes([d[10], d[11]])))
                    }
                    other => {
                        tracing::warn!("qualcomm: the device descriptor could not be read: {other:02X?}");
                        None
                    }
                };
                self.plan(usb)
            }
            (State::Header, Ok(_)) => {
                self.sent += self.downloads[self.current].header;
                self.body(0)
            }
            (State::Status | State::Check { .. }, Ok(d)) => {
                tracing::warn!("qualcomm: a status of {} bytes", d.len());
                self.finish(Err(LoadError::Unsupported("a status answer of another size")))
            }
            (State::Idle | State::Done | State::VersionWait | State::Body { .. }, _) => Vec::new(),
            (_, Err(e)) => {
                tracing::warn!("qualcomm: USB control transfer failed: {e}");
                self.finish(Err(LoadError::Host("a USB control transfer to the chip failed")))
            }
        }
    }

    fn missing_file_hint(&self, file: &str) -> String {
        format!(
            "{file}, from Qualcomm's Windows Bluetooth driver (read from it when installed) or linux-firmware (qca/)"
        )
    }

    fn step(&self) -> &'static str {
        match self.state {
            State::Status | State::VersionWait | State::Version | State::Descriptor => "version",
            State::Header | State::Body { .. } | State::Check { .. } => "download",
            State::Idle | State::Done => "",
        }
    }

    fn progress(&self) -> Option<(usize, usize)> {
        match self.state {
            State::Header | State::Body { .. } | State::Check { .. } => Some((self.sent, self.total)),
            _ => None,
        }
    }
}

// ---- Qualcomm's Windows driver ----

/// Where Qualcomm's Windows driver keeps its files on this PC, newest
/// first: System32\drivers (its INF copies them there), then its folders in
/// the driver store (atheros_bth.inf for ROME, btfilter.inf for the
/// WCN6855). Both stay after the adapter moved to WinUSB.
fn driver_dirs(windows: &Path) -> Vec<PathBuf> {
    let mut found = vec![windows.join("System32").join("drivers")];
    let store = windows.join("System32").join("DriverStore").join("FileRepository");
    let mut stored: Vec<PathBuf> = std::fs::read_dir(store)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| {
                    let n = e.file_name().to_string_lossy().to_ascii_lowercase();
                    n.starts_with("atheros_bth.inf") || n.starts_with("btfilter.inf")
                })
                .map(|e| e.path())
                .collect()
        })
        .unwrap_or_default();
    let modified = |p: &PathBuf| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    stored.sort_by_key(|p| std::cmp::Reverse(modified(p)));
    found.extend(stored);
    found
}

/// A firmware file from Qualcomm's Windows driver, if it is installed. Its
/// files are the Windows names (`.dfu`); linux-firmware's are not there.
pub fn from_windows_driver(name: &str) -> Option<Vec<u8>> {
    if !name.to_ascii_lowercase().ends_with(".dfu") || name.contains(['\\', '/']) {
        return None;
    }
    let windows = std::env::var_os("SystemRoot").map(PathBuf::from)?;
    driver_dirs(&windows).iter().find_map(|dir| {
        let path = dir.join(name);
        let data = std::fs::read(&path).ok()?;
        tracing::info!("qualcomm: {name} from Qualcomm's driver ({}), {} bytes", path.display(), data.len());
        Some(data)
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::bringup::sim::{self, SimController};

    /// A rampatch file for `rom` with this patch version and body size
    fn rampatch(rom: u32, patch: u16, body: usize) -> Vec<u8> {
        let r = ROMS.iter().find(|r| r.version == rom).unwrap();
        let mut f = vec![0u8; r.rampatch_header];
        f[0] = TYPE_RAMPATCH;
        let len = (r.rampatch_header - 4 + body) as u32;
        f[1..4].copy_from_slice(&len.to_le_bytes()[..3]);
        if r.rome {
            f[0x10..0x12].copy_from_slice(&8u16.to_le_bytes());
            f[0x12..0x14].copy_from_slice(&(rom as u16).to_le_bytes());
        } else {
            f[0x10..0x12].copy_from_slice(&((rom >> 16) as u16).to_le_bytes());
            f[0x12..0x14].copy_from_slice(&(rom as u16).to_le_bytes());
        }
        f[0x14..0x16].copy_from_slice(&patch.to_le_bytes());
        f.extend((0..body).map(|n| n as u8));
        f
    }

    fn nvm(body: usize) -> Vec<u8> {
        let mut f = vec![TYPE_NVM];
        f.extend_from_slice(&(body as u32).to_le_bytes()[..3]);
        f.extend((0..body).map(|n| (n * 7) as u8));
        f
    }

    fn version_bytes(v: &Version) -> Vec<u8> {
        let mut d = Vec::new();
        d.extend_from_slice(&v.rom.to_le_bytes());
        d.extend_from_slice(&v.patch.to_le_bytes());
        d.extend_from_slice(&v.ram.to_le_bytes());
        d.extend_from_slice(&[v.chip_id, v.platform_id]);
        d.extend_from_slice(&v.flag.to_le_bytes());
        d.extend_from_slice(&[0, if v.tf { 0x23 } else { 0 }, 0, 0]);
        d
    }

    /// The chip: its status and version; the files it took, as received
    struct Chip {
        status: u8,
        version: Version,
        usb: Result<(u16, u16), String>,
        /// The download in progress: header, then the bulk bytes
        receiving: Vec<u8>,
        received: Vec<Vec<u8>>,
        /// Takes the files but never reports them
        ignores: bool,
        fail_header: bool,
        /// Bulk transfer sizes, in order
        transfers: Vec<usize>,
        /// Status reads in a row that still miss the bit (the transfer is
        /// on its way)
        slow_reads: u32,
        slow_left: u32,
    }

    const ROME: Version = Version { rom: 0x302, patch: 0, ram: 0, chip_id: 7, platform_id: 0, flag: 0, tf: false };

    impl Chip {
        fn new(version: Version) -> Chip {
            Chip {
                status: 0,
                version,
                usb: Ok((0x0CF3, 0xE300)),
                receiving: Vec::new(),
                received: Vec::new(),
                ignores: false,
                fail_header: false,
                transfers: Vec::new(),
                slow_reads: 0,
                slow_left: 0,
            }
        }

        /// A file complete (its TLV length): taken
        fn take(&mut self) {
            let f = &self.receiving;
            if f.len() < 4 {
                return;
            }
            let len = u32::from_le_bytes([f[1], f[2], f[3], 0]) as usize + 4;
            if f.len() == len {
                let file = std::mem::take(&mut self.receiving);
                if !self.ignores {
                    self.status |= if file[0] == TYPE_RAMPATCH { STATUS_PATCHED } else { STATUS_NVM };
                }
                self.received.push(file);
                self.slow_left = self.slow_reads;
            }
        }
    }

    impl SimController for Chip {
        fn command(&mut self, c: &[u8]) -> Vec<Vec<u8>> {
            panic!("no HCI command before HCI init: {c:02X?}");
        }

        fn bulk_command(&mut self, data: &[u8]) -> Vec<Vec<u8>> {
            assert!(!self.receiving.is_empty(), "bulk data before a header");
            self.transfers.push(data.len());
            self.receiving.extend_from_slice(data);
            self.take();
            Vec::new()
        }

        fn control(&mut self, r: &ControlRequest) -> Result<Vec<u8>, String> {
            if r.kind == ControlKind::Standard {
                assert_eq!(*r, ControlRequest::device_descriptor());
                let (vid, pid) = self.usb.clone()?;
                let mut d = vec![18, 1, 0, 2, 0xE0, 1, 1, 64];
                d.extend_from_slice(&vid.to_le_bytes());
                d.extend_from_slice(&pid.to_le_bytes());
                d.extend_from_slice(&[1, 0, 1, 2, 3, 1]);
                return Ok(d);
            }
            assert_eq!((r.kind, r.recipient, r.value, r.index), (ControlKind::Vendor, ControlRecipient::Device, 0, 0));
            match (r.request, &r.data) {
                (REQUEST_STATUS, ControlData::In(1)) => {
                    if self.slow_left > 0 {
                        self.slow_left -= 1;
                        return Ok(vec![self.status & !(STATUS_PATCHED | STATUS_NVM)]);
                    }
                    Ok(vec![self.status])
                }
                (REQUEST_VERSION, ControlData::In(VERSION_LEN)) => Ok(version_bytes(&self.version)),
                (REQUEST_DOWNLOAD, ControlData::Out(header)) => {
                    if self.fail_header {
                        return Err("stall".into());
                    }
                    assert!(self.receiving.is_empty(), "a header while a file is still coming");
                    self.receiving = header.clone();
                    Ok(Vec::new())
                }
                other => panic!("unexpected control transfer {other:?}"),
            }
        }
    }

    fn run(chip: &mut Chip, files: HashMap<String, Vec<u8>>) -> sim::SimRun {
        let mut loader = Loader::new(Box::new(move |name| files.get(name).cloned()));
        sim::run(&mut loader, chip)
    }

    fn rome_files() -> HashMap<String, Vec<u8>> {
        HashMap::from([
            ("AthrBT_0x00000302.dfu".to_owned(), rampatch(0x302, 0x3E8, 2000)),
            ("ramps_0x00000302_48.dfu".to_owned(), nvm(600)),
        ])
    }

    fn headers(r: &sim::SimRun) -> Vec<usize> {
        r.controls
            .iter()
            .filter_map(|c| match (&c.request, &c.data) {
                (&REQUEST_DOWNLOAD, ControlData::Out(d)) => Some(d.len()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn loads_rampatch_then_nvm() {
        let mut chip = Chip::new(ROME);
        let r = run(&mut chip, rome_files());
        assert_eq!(r.result, Ok(Outcome::Loaded { firmware: "ramps_0x00000302_48.dfu".into(), detail: None }));
        let files = rome_files();
        assert_eq!(chip.received, [files["AthrBT_0x00000302.dfu"].clone(), files["ramps_0x00000302_48.dfu"].clone()]);
        assert_eq!(headers(&r), [28, 4]);
        // Three short transfers, then the rest at once, for each file
        assert_eq!(chip.transfers, [320, 320, 320, 2000 - 960, 320, 280]);
        assert!(r.commands.iter().all(|c| c.bulk), "the bodies go on the bulk pipe, no HCI command");
        assert_eq!(chip.status, STATUS_PATCHED | STATUS_NVM);
        assert!(!r.bootloader);
    }

    #[test]
    fn the_wcn6855_takes_a_40_byte_header() {
        let v = Version {
            rom: 0x0013_0201,
            patch: 0x10,
            ram: 0x400C_1211,
            chip_id: 1,
            platform_id: 0x0A,
            flag: 0x8000,
            tf: false,
        };
        let mut chip = Chip { usb: Ok((0x0489, 0xE0C7)), ..Chip::new(v) };
        let files = HashMap::from([
            ("AthrBT_0x00130201.dfu".to_owned(), rampatch(0x0013_0201, 0x3A98, 5000)),
            ("ramps_0x00130201_gf.dfu".to_owned(), nvm(300)),
        ]);
        let r = run(&mut chip, files);
        assert_eq!(r.result, Ok(Outcome::Loaded { firmware: "ramps_0x00130201_gf.dfu".into(), detail: None }));
        assert_eq!(headers(&r), [40, 4]);
    }

    #[test]
    fn names_the_nvm_as_qualcomms_driver() {
        let gf_board = Version {
            rom: 0x0013_0201,
            patch: 0,
            ram: 0x400C_1211,
            chip_id: 1,
            platform_id: 0x0A,
            flag: 0x8000,
            tf: false,
        };
        // The board only on the adapters the driver detects it on
        assert_eq!(
            nvm_names(&gf_board, Some((0x0CF3, 0xE600)))[..2],
            ["ramps_0x00130201_gf_010a.dfu", "ramps_0x00130201_gf.dfu"]
        );
        assert_eq!(nvm_names(&gf_board, Some((0x0489, 0xE0C7)))[0], "ramps_0x00130201_gf.dfu");
        assert_eq!(nvm_names(&gf_board, None)[0], "ramps_0x00130201_gf.dfu");
        // linux-firmware names it by the board always
        assert_eq!(nvm_names(&gf_board, None)[1..], ["nvm_usb_00130201_gf_010a.bin", "nvm_usb_00130201_gf.bin"]);
        let plain = Version { ram: 0x400C_0210, ..gf_board };
        assert_eq!(nvm_names(&plain, Some((0x0CF3, 0xE600)))[0], "ramps_0x00130201_010a.dfu");
        // ROME: the crystal, an adapter's own file
        assert_eq!(nvm_names(&ROME, None), ["ramps_0x00000302_48.dfu", "ramps_0x00000302.dfu", "nvm_usb_00000302.bin"]);
        assert_eq!(
            nvm_names(&ROME, Some((0x0CF3, 0xE302)))[..2],
            ["ramps_0x00000302_48_tx8.dfu", "ramps_0x00000302.dfu"]
        );
        assert_eq!(nvm_names(&Version { chip_id: 0, ..ROME }, None)[0], "ramps_0x00000302_26.dfu");
        let tf = Version { tf: true, ..ROME };
        assert_eq!(nvm_names(&tf, None), ["ramps_TF_0x00000302_48.dfu", "ramps_TF_0x00000302.dfu"]);
        assert_eq!(rampatch_names(&tf), ["AthrBT_TF_0x00000302.dfu"]);
        assert_eq!(rampatch_names(&ROME), ["AthrBT_0x00000302.dfu", "rampatch_usb_00000302.bin"]);
    }

    #[test]
    fn falls_back_to_the_chips_nvm_and_to_linux_firmware_names() {
        let files = HashMap::from([
            ("rampatch_usb_00000302.bin".to_owned(), rampatch(0x302, 0x3E8, 100)),
            ("nvm_usb_00000302.bin".to_owned(), nvm(50)),
        ]);
        let r = run(&mut Chip { usb: Ok((0x0CF3, 0xE302)), ..Chip::new(ROME) }, files);
        assert_eq!(r.result, Ok(Outcome::Loaded { firmware: "nvm_usb_00000302.bin".into(), detail: None }));
    }

    #[test]
    fn nothing_is_sent_when_the_firmware_is_in() {
        let mut chip = Chip { status: STATUS_PATCHED | STATUS_NVM, ..Chip::new(ROME) };
        let r = run(&mut chip, HashMap::new());
        assert_eq!(r.result, Ok(Outcome::AlreadyRunning));
        assert!(headers(&r).is_empty() && r.commands.is_empty());
    }

    #[test]
    fn only_the_nvm_when_the_rampatch_is_in() {
        let mut chip = Chip { status: STATUS_PATCHED, ..Chip::new(ROME) };
        let r = run(&mut chip, HashMap::from([("ramps_0x00000302_48.dfu".to_owned(), nvm(10))]));
        assert_eq!(
            r.result,
            Ok(Outcome::Loaded {
                firmware: "ramps_0x00000302_48.dfu".into(),
                detail: Some("its rampatch was in the chip already")
            })
        );
        assert_eq!(headers(&r), [4]);
    }

    #[test]
    fn a_rampatch_no_newer_than_the_chips_is_not_sent() {
        let mut chip = Chip::new(Version { patch: 0x3E8, ..ROME });
        let r = run(&mut chip, rome_files());
        assert!(r.result.is_ok(), "{:?}", r.result);
        assert_eq!(headers(&r), [4], "the NVM only");
    }

    #[test]
    fn an_unknown_rom_is_left_alone() {
        for rom in [0x0000_0101, 0x0019_0200, 0x3101_0000] {
            let r = run(&mut Chip::new(Version { rom, ..ROME }), rome_files());
            assert!(matches!(r.result, Err(LoadError::Unsupported(_))), "{rom:08X}: {:?}", r.result);
            assert!(headers(&r).is_empty() && r.commands.is_empty(), "nothing sent to it");
        }
        // Firmware in already: nothing to send either way
        let r = run(&mut Chip { status: 0xC0, ..Chip::new(Version { rom: 0x101, ..ROME }) }, HashMap::new());
        assert_eq!(r.result, Ok(Outcome::AlreadyRunning));
    }

    #[test]
    fn a_state_the_driver_refuses_is_refused() {
        let r = run(&mut Chip { status: 0x0B, ..Chip::new(ROME) }, rome_files());
        assert!(matches!(r.result, Err(LoadError::Unsupported(_))), "{:?}", r.result);
        assert_eq!(r.controls.len(), 1, "only the status read");
    }

    #[test]
    fn checks_every_file_before_sending_any() {
        // A missing NVM: not even the rampatch goes
        let mut files = rome_files();
        files.remove("ramps_0x00000302_48.dfu");
        let r = run(&mut Chip::new(ROME), files);
        assert_eq!(r.result, Err(LoadError::MissingFile("ramps_0x00000302_48.dfu".into())));
        assert!(headers(&r).is_empty());
        // A rampatch for another ROM, cut short, or no rampatch at all
        for bad in [rampatch(0x300, 0x3E8, 100), rampatch(0x302, 0x3E8, 100)[..60].to_vec(), nvm(100)] {
            let mut files = rome_files();
            files.insert("AthrBT_0x00000302.dfu".into(), bad);
            let r = run(&mut Chip::new(ROME), files);
            assert!(matches!(r.result, Err(LoadError::BadFile { .. })), "{:?}", r.result);
            assert!(headers(&r).is_empty());
        }
        let r = run(&mut Chip::new(ROME), HashMap::new());
        assert_eq!(r.result, Err(LoadError::MissingFile("AthrBT_0x00000302.dfu".into())));
    }

    #[test]
    fn waits_for_the_chip_to_report_the_file() {
        let mut chip = Chip { slow_reads: 4, ..Chip::new(ROME) };
        let r = run(&mut chip, rome_files());
        assert!(r.result.is_ok(), "{:?}", r.result);
        let reads = r.controls.iter().filter(|c| c.request == REQUEST_STATUS).count();
        assert_eq!(reads, 1 + 5 + 5);
    }

    #[test]
    fn a_file_the_chip_does_not_report_fails() {
        let r = run(&mut Chip { ignores: true, ..Chip::new(ROME) }, rome_files());
        assert_eq!(r.result, Err(LoadError::Timeout("rampatch")));
    }

    #[test]
    fn a_failed_transfer_ends_it() {
        let r = run(&mut Chip { fail_header: true, ..Chip::new(ROME) }, rome_files());
        assert!(matches!(r.result, Err(LoadError::Host(_))), "{:?}", r.result);
        // Over H4 (the emulator) the first request fails
        struct H4;
        impl SimController for H4 {
            fn command(&mut self, _: &[u8]) -> Vec<Vec<u8>> {
                Vec::new()
            }
        }
        let mut loader = Loader::new(Box::new(|_| None));
        let r = sim::run(&mut loader, &mut H4);
        assert!(matches!(r.result, Err(LoadError::Host(_))), "{:?}", r.result);
    }

    #[test]
    fn without_the_usb_ids_the_chips_own_nvm() {
        let mut files = rome_files();
        files.insert("ramps_0x00000302_48_tx8.dfu".into(), nvm(99));
        let r = run(&mut Chip { usb: Err("stall".into()), ..Chip::new(ROME) }, files.clone());
        assert_eq!(r.result, Ok(Outcome::Loaded { firmware: "ramps_0x00000302_48.dfu".into(), detail: None }));
        let r = run(&mut Chip { usb: Ok((0x0CF3, 0xE302)), ..Chip::new(ROME) }, files);
        assert_eq!(r.result, Ok(Outcome::Loaded { firmware: "ramps_0x00000302_48_tx8.dfu".into(), detail: None }));
    }

    #[test]
    fn reads_the_version() {
        let mut d = version_bytes(&ROME);
        d[17] = 0x23;
        let v = Version::parse(&d).unwrap();
        assert!(v.tf);
        assert_eq!((v.rom, v.chip_id), (0x302, 7));
        assert_eq!(Version::parse(&d[..19]), None);
    }

    #[test]
    fn tells_its_step_and_progress() {
        let files = rome_files();
        let mut loader = Loader::new(Box::new(move |name| files.get(name).cloned()));
        loader.start();
        assert_eq!(loader.step(), "version");
        loader.on_control(Ok(vec![0]));
        loader.on_timeout();
        loader.on_control(Ok(version_bytes(&ROME)));
        loader.on_control(Err("stall".into()));
        let total = rampatch(0x302, 0x3E8, 2000).len() + nvm(600).len();
        assert_eq!((loader.step(), loader.progress()), ("download", Some((0, total))));
        loader.on_control(Ok(Vec::new()));
        assert_eq!(loader.progress(), Some((28 + 320, total)));
    }

    #[test]
    fn every_listed_usb_id_is_taken_as_qualcomm() {
        use crate::chips::{ChipVendor, LOADER_CHIPS, vendor_for_usb};
        let ids: Vec<(u16, u16)> = LOADER_CHIPS
            .iter()
            .filter(|(v, _, _)| *v == ChipVendor::Qualcomm)
            .map(|&(_, vid, pid)| (vid, pid))
            .collect();
        for id in [(0x0CF3, 0xE300), (0x0CF3, 0xE360), (0x0CF3, 0xE600), (0x0489, 0xE0C7), (0x13D3, 0x3491)] {
            assert!(ids.contains(&id), "{id:04X?}");
        }
        for (vid, pid) in ids {
            assert_eq!(vendor_for_usb(vid, pid, None), ChipVendor::Qualcomm, "{vid:04X}:{pid:04X}");
        }
        // The adapters with their own NVM or a board ID are listed
        for &(vid, pid) in BOARD_ID_ADAPTERS.iter().chain(&[(0x0CF3, 0xE302), (0x0CF3, 0xE303)]) {
            assert!(LOADER_CHIPS.contains(&(ChipVendor::Qualcomm, vid, pid)), "{vid:04X}:{pid:04X}");
        }
    }

    #[test]
    fn reads_the_header_of_real_files() {
        // The first 0x16 bytes of AthrBT_0x00000302.dfu (10.0.0.1272) and
        // AthrBT_0x00130201.dfu (2.0.0.1400)
        let rome = [
            0x01, 0x60, 0x0E, 0x01, 0x60, 0x0D, 0x01, 0x00, 0x48, 0x0D, 0x01, 0x00, 0x01, 0x02, 0x00, 0x00, 0x08, 0x00,
            0x02, 0x03, 0xE8, 0x03,
        ];
        let hsp = [
            0x01, 0x08, 0xBB, 0x02, 0xE8, 0xBA, 0x02, 0x00, 0xC4, 0xBA, 0x02, 0x00, 0x01, 0x00, 0x03, 0x00, 0x13, 0x00,
            0x01, 0x02, 0x98, 0x3A,
        ];
        assert_eq!(rampatch_version(&rome, &ROMS[2]), Some((0x302, 0x3E8)));
        assert_eq!(rampatch_version(&hsp, &ROMS[4]), Some((0x0013_0201, 0x3A98)));
        let mut file = rome.to_vec();
        file.resize(0x10E60 + 4, 0);
        assert_eq!(check_tlv(&file, TYPE_RAMPATCH, 28), Ok(()));
        file.pop();
        assert!(check_tlv(&file, TYPE_RAMPATCH, 28).is_err());
    }

    /// Against Qualcomm's real driver files: `QCA_DRIVER_DIR=<folder with
    /// the .dfu files> cargo test -p bt-host --lib qualcomm::tests::real -- --ignored`
    #[test]
    #[ignore = "needs Qualcomm's driver files (QCA_DRIVER_DIR)"]
    fn real_driver_files_load() {
        let dir = PathBuf::from(std::env::var("QCA_DRIVER_DIR").expect("QCA_DRIVER_DIR"));
        let read = |name: &str| std::fs::read(dir.join(name)).ok();
        // ROM, SoC, chip ID, flag, TF of the chips the files are for
        let cases = [
            (0x0000_0200u32, 0u32, 7u8, 0u16, false),
            (0x0000_0300, 0, 7, 0, false),
            (0x0000_0302, 0, 7, 0, false),
            (0x0000_0302, 0, 7, 0, true),
            (0x0013_0200, 0, 1, 0x8000, false),
            (0x0013_0201, 0, 1, 0x8000, false),
            (0x0013_0201, 0x400C_1211, 1, 0x8000, false),
        ];
        let mut loaded = 0;
        for (rom, ram, chip_id, flag, tf) in cases {
            let v = Version { rom, patch: 0, ram, chip_id, platform_id: 0x0A, flag, tf };
            let (Some(patch_name), Some(nvm_name)) = (
                rampatch_names(&v).into_iter().find(|n| read(n).is_some()),
                nvm_names(&v, None).into_iter().find(|n| read(n).is_some()),
            ) else {
                continue;
            };
            let files: HashMap<String, Vec<u8>> =
                [&patch_name, &nvm_name].iter().map(|n| (n.to_string(), read(n).unwrap())).collect();
            let mut chip = Chip::new(v);
            let r = run(&mut chip, files.clone());
            assert_eq!(r.result, Ok(Outcome::Loaded { firmware: nvm_name.clone(), detail: None }), "{patch_name}");
            assert_eq!(chip.received, [files[&patch_name].clone(), files[&nvm_name].clone()]);
            eprintln!("ROM 0x{rom:08X}: {patch_name} and {nvm_name} loaded");
            loaded += 1;
        }
        assert!(loaded > 0, "no Qualcomm files in QCA_DRIVER_DIR");
    }
}
