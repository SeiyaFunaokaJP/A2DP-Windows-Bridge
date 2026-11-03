//! Tests of the chip probe (src/probe.rs) against a simulated controller

use bt_host::bringup::{Action, ChipLoader, Outcome};
use bt_host::probe::{Probe, ProbeReport};

fn command_complete(opcode: u16, ret: &[u8]) -> Vec<u8> {
    let [lo, hi] = opcode.to_le_bytes();
    let mut e = vec![0x0E, (3 + ret.len()) as u8, 1, lo, hi];
    e.extend_from_slice(ret);
    e
}

fn command_status(opcode: u16, status: u8) -> Vec<u8> {
    let [lo, hi] = opcode.to_le_bytes();
    vec![0x0F, 4, status, 1, lo, hi]
}

/// Runs the probe; `answer` gives the controller's events for a command
/// (none: it stays silent and the step times out)
fn run(answer: impl Fn(u16) -> Vec<Vec<u8>>) -> (ProbeReport, Vec<u16>) {
    let mut probe = Probe::new();
    let mut actions = probe.start();
    let mut sent = Vec::new();
    loop {
        let mut next = Vec::new();
        for action in actions {
            match action {
                Action::Command(c) => {
                    let opcode = u16::from_le_bytes([c[0], c[1]]);
                    sent.push(opcode);
                    let events = answer(opcode);
                    if events.is_empty() {
                        next.extend(probe.on_timeout());
                    }
                    for e in events {
                        next.extend(probe.on_event(&e));
                    }
                }
                Action::Timer(_) => {}
                Action::Done(r) => {
                    assert_eq!(r, Ok(Outcome::Probed));
                    return (probe.take_report().unwrap(), sent);
                }
                other => panic!("the probe asked for {other:?}"),
            }
        }
        actions = next;
    }
}

fn intel_like(opcode: u16) -> Vec<Vec<u8>> {
    match opcode {
        // HCI 5.3 (12), revision 0x3102, LMP 12, Intel (0x0002), subversion 0x3102
        0x1001 => vec![command_complete(opcode, &[0, 12, 0x02, 0x31, 12, 0x02, 0x00, 0x02, 0x31])],
        // 00:1A:7D:DA:71:13, little endian
        0x1009 => vec![command_complete(opcode, &[0, 0x13, 0x71, 0xDA, 0x7D, 0x1A, 0x00])],
        // ACL 1021 x 4, SCO 96 x 6
        0x1005 => vec![command_complete(opcode, &[0, 0xFD, 0x03, 96, 4, 0, 6, 0])],
        0x0C14 => {
            let mut ret = vec![0];
            ret.extend_from_slice(b"AX211");
            ret.resize(1 + 248, 0);
            vec![command_complete(opcode, &ret)]
        }
        0x2002 => vec![command_complete(opcode, &[0, 0xFB, 0x00, 3])],
        _ => vec![command_complete(opcode, &[0, 0xAA])],
    }
}

#[test]
fn reads_every_step_and_sends_no_reset() {
    let (report, sent) = run(intel_like);
    assert_eq!(sent, [0x1001, 0x1009, 0x1002, 0x1003, 0x1005, 0x0C14, 0x2003, 0x2002]);
    assert!(!sent.contains(&0x0C03), "no HCI Reset");
    assert!(report.steps.iter().all(|s| s.answer.is_some()));
    assert_eq!(report.error, None);
    let s = report.summary();
    assert_eq!((s.answered, s.steps), (8, 8));
    assert_eq!(s.hci_version, Some(12));
    assert_eq!(s.company, Some(0x0002));
    assert_eq!(s.company_name, Some("Intel"));
    assert_eq!(s.lmp_subversion, Some(0x3102));
    assert_eq!(s.address.as_deref(), Some("00:1A:7D:DA:71:13"));
    assert_eq!(s.name.as_deref(), Some("AX211"));
    assert_eq!(s.acl_buffers, Some((1021, 4)));
    assert_eq!(s.le_acl_buffers, Some((251, 3)));
}

#[test]
fn a_silent_step_times_out_and_the_probe_goes_on() {
    let (report, sent) = run(|op| if op == 0x1002 { Vec::new() } else { intel_like(op) });
    assert_eq!(sent.len(), 8);
    let silent = report.steps.iter().find(|s| s.name == "Read Local Supported Commands").unwrap();
    assert_eq!(silent.answer, None);
    assert_eq!(report.summary().answered, 7);
}

#[test]
fn a_bootloader_that_answers_nothing_gives_a_report_of_timeouts() {
    let (report, sent) = run(|_| Vec::new());
    assert_eq!(sent.len(), 8);
    let s = report.summary();
    assert_eq!((s.answered, s.hci_version, s.address), (0, None, None));
}

#[test]
fn errors_and_other_events_are_kept_as_received() {
    let vendor = vec![0xFF, 3, 0x06, 0x00, 0x01];
    let (report, _) = run(|op| match op {
        // Unknown HCI command
        0x2003 => vec![command_status(op, 0x01)],
        0x1001 => vec![vendor.clone(), intel_like(op).remove(0)],
        _ => intel_like(op),
    });
    let le = report.steps.iter().find(|s| s.name == "LE Read Local Supported Features").unwrap();
    assert_eq!(le.answer.as_deref(), Some(&command_status(0x2003, 0x01)[..]));
    assert_eq!(report.other_events, [vendor]);
    // An answer with an error status is not decoded
    let (report, _) = run(|op| if op == 0x1001 { vec![command_complete(op, &[0x01])] } else { intel_like(op) });
    assert_eq!(report.summary().hci_version, None);
}
