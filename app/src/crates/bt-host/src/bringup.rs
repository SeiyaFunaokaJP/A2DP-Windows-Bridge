//! Controller bring-up before HCI init: what every chip loader shares
//!
//! Some controllers need the host to do something before HCI init can run:
//! download firmware to a bootloader (Intel), or later read what an
//! unsupported chip is (the probe, docs/chip-bringup-dev.md). A loader is a
//! state machine fed with the controller's HCI events and its timer; it
//! answers with [`Action`]s that the app carries out on the raw transport
//! (`Stack::raw_begin`), so its packets are in the HCI capture too.
//!
//! Plain data and std only: a loader can be tested against a simulated
//! controller (tests/intel.rs). [`sample`] is a complete loader with its
//! tests to start a new one from (docs/chip-bringup-dev.md, "Adding a
//! vendor").

use std::fmt;

pub use btstack_sys::{ControlData, ControlKind, ControlRecipient, ControlRequest, UsbDescriptors};

#[cfg(test)]
mod sample;

/// Reads a file of the firmware folder by name ("ibt-0040-0041.sfi")
pub type FileReader = Box<dyn FnMut(&str) -> Option<Vec<u8>>>;

/// What the caller does for a [`ChipLoader`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Send an HCI command (opcode, length, parameters) on the control pipe
    Command(Vec<u8>),
    /// Send a command on the bulk OUT pipe (the ACL slot), as Intel's
    /// bootloader takes Secure Send
    BulkCommand(Vec<u8>),
    /// The controller runs its bootloader (events also on the bulk IN pipe,
    /// no HCI Reset), or no longer
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
/// [`on_timeout`](Self::on_timeout), until [`Action::Done`]
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
}
