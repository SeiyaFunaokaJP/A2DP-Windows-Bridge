//! Controller bring-up before HCI init: what every chip loader shares
//!
//! Some controllers need the host to do something before HCI init can run:
//! download firmware to a bootloader (Intel), or read what an unsupported
//! chip is (crate::probe). A loader is a state machine fed with the
//! controller's HCI events, the results of its USB requests and its timer;
//! it answers with [`Action`]s that the app carries out on the raw transport
//! (`Stack::raw_begin`), so its packets are in the HCI capture too.
//!
//! Plain data and std only: a loader is tested against a simulated
//! controller ([`sim`]). `bringup/sample.rs` is a complete loader with its
//! tests to start a new one from (docs/chip-support.md).

use std::fmt;

pub use btstack_sys::{ControlData, ControlKind, ControlRecipient, ControlRequest, UsbDescriptors};

#[cfg(test)]
mod sample;
pub mod sim;

/// Reads a file of the firmware folder by name, without a folder
/// ("ibt-0040-0041.sfi"): the app looks in the firmware folder and in the
/// vendor's subfolder there ("intel\"), as linux-firmware has them
pub type FileReader = Box<dyn FnMut(&str) -> Option<Vec<u8>>>;

/// An HCI command as sent: opcode, length, parameters
pub fn hci_command(opcode: u16, params: &[u8]) -> Vec<u8> {
    let mut c = opcode.to_le_bytes().to_vec();
    c.push(params.len() as u8);
    c.extend_from_slice(params);
    c
}

/// An HCI event as a loader reads it ([`parse_event`])
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HciEvent<'a> {
    /// Command Complete: the command, its status, the return parameters
    /// after the status
    Complete { opcode: u16, status: u8, ret: &'a [u8] },
    /// Command Status (a command whose result comes later, or a refused one)
    Status { opcode: u16, status: u8 },
    /// A vendor event (code 0xFF): its parameters
    Vendor(&'a [u8]),
    /// Any other event: code and parameters
    Other { code: u8, params: &'a [u8] },
}

/// Reads an event as received (code, length, parameters); a truncated one
/// is [`HciEvent::Other`]
pub fn parse_event(event: &[u8]) -> HciEvent<'_> {
    let code = event.first().copied().unwrap_or(0);
    let len = event.get(1).copied().unwrap_or(0) as usize;
    let params = event.get(2..2 + len).unwrap_or(&[]);
    match (code, params) {
        (0x0E, [_, lo, hi, status, ret @ ..]) => {
            HciEvent::Complete { opcode: u16::from_le_bytes([*lo, *hi]), status: *status, ret }
        }
        (0x0F, [status, _, lo, hi]) => HciEvent::Status { opcode: u16::from_le_bytes([*lo, *hi]), status: *status },
        (0xFF, params) => HciEvent::Vendor(params),
        (code, params) => HciEvent::Other { code, params },
    }
}

/// What the caller does for a [`ChipLoader`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Send an HCI command (opcode, length, parameters) on the control pipe
    Command(Vec<u8>),
    /// Send a command on the bulk OUT pipe (the ACL slot), as Intel's
    /// bootloader takes Secure Send
    BulkCommand(Vec<u8>),
    /// The controller runs its bootloader (events also on the bulk IN pipe,
    /// no HCI Reset when the adapter is closed), or no longer. Turn it on
    /// before the first command for a chip whose loader answers on the bulk
    /// pipe or does not take HCI Reset, and off once the operational
    /// firmware runs; a failed download leaves it on. ROM chips that answer
    /// HCI normally do not need it.
    Bootloader(bool),
    /// (Re)start the loader's timer: [`ChipLoader::on_timeout`] when it fires
    Timer(u32),
    /// A USB control transfer on the default pipe (vendor registers,
    /// descriptors). Done at once: [`ChipLoader::on_control`] gets its
    /// result before the actions after this one run. Over H4 (the
    /// emulator) it fails with "not a USB transport".
    UsbControl(ControlRequest),
    /// Finished. Ok: HCI init may start; Err: the controller is unusable
    Done(Result<Outcome, LoadError>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The operational firmware was running already
    AlreadyRunning,
    /// Firmware loaded and booted (`firmware`: the last file sent);
    /// `detail` for the log ("DDC applied")
    Loaded { firmware: String, detail: Option<&'static str> },
    /// A ROM controller: runs without a download
    Rom,
    /// The chip probe finished: no HCI init follows
    /// ([`ChipLoader::take_report`] has what it read)
    Probed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    /// The transport did not take a command, or no timer could be started
    Host(&'static str),
    /// A command was answered with this error status
    Command {
        opcode: u16,
        status: u8,
    },
    /// No answer in time
    Timeout(&'static str),
    /// The controller's version answer could not be read
    BadVersion,
    /// A controller generation the loader does not know
    UnsupportedVariant(u8),
    Unsupported(&'static str),
    /// This firmware file is not in the folder
    MissingFile(String),
    /// The firmware file is not what the controller takes
    BadFile {
        file: String,
        why: &'static str,
    },
    /// The controller rejected the image (its result code)
    Rejected(u8),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Host(what) => f.write_str(what),
            LoadError::Command { opcode, status } => {
                write!(f, "command 0x{opcode:04X} failed with status 0x{status:02X}")
            }
            LoadError::Timeout(step) => write!(f, "no answer from the controller ({step})"),
            LoadError::BadVersion => write!(f, "unreadable version information"),
            LoadError::UnsupportedVariant(v) => write!(f, "hardware variant 0x{v:02X} is not supported"),
            LoadError::Unsupported(why) => write!(f, "not supported: {why}"),
            LoadError::MissingFile(file) => write!(f, "firmware file {file} not found"),
            LoadError::BadFile { file, why } => write!(f, "firmware file {file}: {why}"),
            LoadError::Rejected(result) => {
                write!(
                    f,
                    "the controller rejected the firmware (result 0x{result:02X}); is the file for this controller?"
                )
            }
        }
    }
}

impl std::error::Error for LoadError {}

/// A controller bring-up: [`start`](Self::start), then every HCI event to
/// [`on_event`](Self::on_event) and the timer to
/// [`on_timeout`](Self::on_timeout), until [`Action::Done`]. The app drops
/// the loader at Done: a late answer never reaches it.
pub trait ChipLoader {
    /// The vendor, for log lines ("Intel")
    fn vendor(&self) -> &'static str;
    fn start(&mut self) -> Vec<Action>;
    /// An HCI event from the controller (either pipe)
    fn on_event(&mut self, event: &[u8]) -> Vec<Action>;
    fn on_timeout(&mut self) -> Vec<Action>;
    /// The result of an [`Action::UsbControl`]: the bytes read (IN), none
    /// (OUT), or why it failed. Only loaders that ask for one get it.
    fn on_control(&mut self, _result: Result<Vec<u8>, String>) -> Vec<Action> {
        Vec::new()
    }
    /// What to put in the firmware folder when `file` is missing
    /// ("x.sfi and x.ddc from linux-firmware (intel/)")
    fn missing_file_hint(&self, file: &str) -> String {
        file.to_owned()
    }
    /// The chip probe's record (crate::probe), also of a probe that stopped
    /// early; None for a loader
    fn take_report(&mut self) -> Option<crate::probe::ProbeReport> {
        None
    }
    /// The step running, for the Adapters screen while the adapter starts
    /// ("Secure Send"); empty: none to show
    fn step(&self) -> &'static str {
        ""
    }
    /// How far the running step is: commands acknowledged of all (a
    /// download)
    fn progress(&self) -> Option<(usize, usize)> {
        None
    }
}

/// Where a loader is while the adapter starts (HostConfig::on_loader)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoaderProgress {
    pub vendor: &'static str,
    /// [`ChipLoader::step`] while it runs; how it ended once done ("firmware
    /// running already", the error)
    pub step: String,
    /// Percent of the running step, when the loader tells
    pub percent: Option<u8>,
    /// Finished: true when the adapter can start
    pub done: Option<bool>,
}

/// Called on the BTstack thread when a loader's step, its percent or its
/// end changes
pub type LoaderHook = std::sync::Arc<dyn Fn(&LoaderProgress) + Send + Sync>;
