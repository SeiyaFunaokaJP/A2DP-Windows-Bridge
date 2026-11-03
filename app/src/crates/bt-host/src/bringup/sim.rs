//! Running a [`ChipLoader`] against a simulated controller, for its tests
//!
//! [`run`] carries out the loader's actions as the app's runner does
//! (`HostApp::run_loader` in app.rs): commands go to the [`SimController`],
//! whose events go back to the loader; a USB control transfer's result goes
//! to `on_control` before the next action; when nothing is left to do and
//! the loader's timer is running, it fires. A test then checks the result
//! and what the controller received.
//!
//! The controller answers the way the real one did in a capture: copy its
//! bytes. See `sample.rs` for a complete example.

use std::collections::VecDeque;

use super::{Action, ChipLoader, ControlRequest, LoadError, Outcome};

/// A simulated controller: what it answers to each request
pub trait SimController {
    /// The events that answer an HCI command on the control pipe, in order;
    /// none: it stays silent (the loader's timer fires)
    fn command(&mut self, command: &[u8]) -> Vec<Vec<u8>>;
    /// The same for a command on the bulk OUT pipe ([`Action::BulkCommand`])
    fn bulk_command(&mut self, command: &[u8]) -> Vec<Vec<u8>> {
        self.command(command)
    }
    /// A USB control transfer; by default the way H4 (the emulator) refuses
    /// one
    fn control(&mut self, _request: &ControlRequest) -> Result<Vec<u8>, String> {
        Err("not a USB transport".into())
    }
}

/// What a [`run`] did
#[derive(Debug)]
pub struct SimRun {
    pub result: Result<Outcome, LoadError>,
    /// Every command sent, both pipes, in order
    pub commands: Vec<Vec<u8>>,
    /// Every control transfer, in order
    pub controls: Vec<ControlRequest>,
    /// The last [`Action::Bootloader`]
    pub bootloader: bool,
    /// How often the timer fired
    pub timeouts: usize,
}

impl SimRun {
    /// The opcodes of the commands sent
    pub fn opcodes(&self) -> Vec<u16> {
        self.commands.iter().map(|c| u16::from_le_bytes([c[0], c[1]])).collect()
    }
}

/// Upper bound on actions in one run: a loader that never ends fails the
/// test instead of hanging it
const MAX_ACTIONS: usize = 1_000_000;

/// Runs `loader` from [`ChipLoader::start`] to its [`Action::Done`]
pub fn run(loader: &mut dyn ChipLoader, chip: &mut dyn SimController) -> SimRun {
    let mut actions: VecDeque<Action> = loader.start().into();
    let (mut commands, mut controls) = (Vec::new(), Vec::new());
    let (mut bootloader, mut timer, mut timeouts) = (false, false, 0);
    for _ in 0..MAX_ACTIONS {
        let Some(action) = actions.pop_front() else {
            assert!(timer, "the loader waits with no timer running: it would hang");
            timer = false;
            timeouts += 1;
            actions.extend(loader.on_timeout());
            continue;
        };
        match action {
            Action::Command(c) => {
                let events = chip.command(&c);
                commands.push(c);
                for e in events {
                    actions.extend(loader.on_event(&e));
                }
            }
            Action::BulkCommand(c) => {
                let events = chip.bulk_command(&c);
                commands.push(c);
                for e in events {
                    actions.extend(loader.on_event(&e));
                }
            }
            Action::UsbControl(r) => {
                let result = chip.control(&r);
                controls.push(r);
                for a in loader.on_control(result).into_iter().rev() {
                    actions.push_front(a);
                }
            }
            Action::Bootloader(on) => bootloader = on,
            Action::Timer(_) => timer = true,
            Action::Done(result) => return SimRun { result, commands, controls, bootloader, timeouts },
        }
    }
    panic!("the loader did not finish within {MAX_ACTIONS} actions");
}

/// A Command Complete event: opcode, status, return parameters after it
pub fn command_complete(opcode: u16, status: u8, ret: &[u8]) -> Vec<u8> {
    let [lo, hi] = opcode.to_le_bytes();
    let mut e = vec![0x0E, (4 + ret.len()) as u8, 1, lo, hi, status];
    e.extend_from_slice(ret);
    e
}

/// A Command Status event
pub fn command_status(opcode: u16, status: u8) -> Vec<u8> {
    let [lo, hi] = opcode.to_le_bytes();
    vec![0x0F, 4, status, 1, lo, hi]
}

/// A vendor event (code 0xFF) with these parameters
pub fn vendor_event(params: &[u8]) -> Vec<u8> {
    let mut e = vec![0xFF, params.len() as u8];
    e.extend_from_slice(params);
    e
}

/// The opcode of a command as sent (for matching in a [`SimController`])
pub fn opcode(command: &[u8]) -> u16 {
    u16::from_le_bytes([command[0], command[1]])
}

/// The parameters of a command as sent
pub fn params(command: &[u8]) -> &[u8] {
    command.get(3..).unwrap_or(&[])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bringup::hci_command;

    #[test]
    fn helpers_build_what_hci_sends() {
        assert_eq!(hci_command(0xFC20, &[1, 2]), [0x20, 0xFC, 2, 1, 2]);
        assert_eq!(command_complete(0x1001, 0, &[12]), [0x0E, 5, 1, 0x01, 0x10, 0, 12]);
        assert_eq!(command_status(0xFC20, 0x01), [0x0F, 4, 0x01, 1, 0x20, 0xFC]);
        assert_eq!(vendor_event(&[0x07]), [0xFF, 1, 0x07]);
        let c = hci_command(0xFC20, &[9]);
        assert_eq!((opcode(&c), params(&c)), (0xFC20, &[9u8][..]));
    }
}
