//! Tests of the chip probe (src/probe.rs) against a simulated controller

use bt_host::bringup::{Action, ChipLoader, ControlData, ControlRequest, Outcome};
use bt_host::probe::{Probe, ProbeReport, StepKind};

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
/// (none: it stays silent and the step times out), `usb` the result of a
/// control transfer
fn run_with(
    answer: impl Fn(u16) -> Vec<Vec<u8>>,
    usb: impl Fn(&ControlRequest) -> Result<Vec<u8>, String>,
) -> (ProbeReport, Vec<u16>, Vec<ControlRequest>) {
    let mut probe = Probe::new();
    let mut actions: std::collections::VecDeque<Action> = probe.start().into();
    let (mut sent, mut controls) = (Vec::new(), Vec::new());
    while let Some(action) = actions.pop_front() {
        match action {
            Action::Command(c) => {
                let opcode = u16::from_le_bytes([c[0], c[1]]);
                sent.push(opcode);
                let events = answer(opcode);
                if events.is_empty() {
                    actions.extend(probe.on_timeout());
                }
                for e in events {
                    actions.extend(probe.on_event(&e));
                }
            }
            Action::UsbControl(r) => {
                let result = usb(&r);
                controls.push(r);
                for a in probe.on_control(result).into_iter().rev() {
                    actions.push_front(a);
                }
            }
            Action::Timer(_) => {}
            // No HCI Reset when the adapter is closed afterwards
            Action::Bootloader(on) => assert!(on),
            Action::Done(r) => {
                assert_eq!(r, Ok(Outcome::Probed));
                return (probe.take_report().unwrap(), sent, controls);
            }
            other => panic!("the probe asked for {other:?}"),
        }
    }
    panic!("the probe stopped without Done");
}

fn run(answer: impl Fn(u16) -> Vec<Vec<u8>>) -> (ProbeReport, Vec<u16>) {
    let (report, sent, _) = run_with(answer, intel_usb);
    (report, sent)
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

fn string_descriptor(s: &str) -> Vec<u8> {
    let mut d = vec![0, 0x03];
    for u in s.encode_utf16() {
        d.extend_from_slice(&u.to_le_bytes());
    }
    d[0] = d.len() as u8;
    d
}

/// A USB device 8087:0033 whose product string is index 2 and that has no
/// manufacturer string
fn intel_usb(r: &ControlRequest) -> Result<Vec<u8>, String> {
    assert_eq!(r.request, 0x06, "GET_DESCRIPTOR only");
    assert!(matches!(r.data, ControlData::In(_)), "reads only");
    match r.value {
        0x0100 => Ok(vec![18, 0x01, 0x00, 0x02, 0xE0, 0x01, 0x01, 64, 0x87, 0x80, 0x33, 0x00, 0x00, 0x00, 0, 2, 0, 1]),
        0x0302 => Ok(string_descriptor("Wireless_Device")),
        v => Err(format!("unexpected descriptor 0x{v:04X}")),
    }
}

#[test]
fn reads_every_step_and_sends_no_reset() {
    let (report, sent, controls) = run_with(intel_like, intel_usb);
    assert_eq!(sent, [0x1001, 0x1009, 0x1002, 0x1003, 0x1005, 0x0C14, 0x2003, 0x2002]);
    assert!(!sent.contains(&0x0C03), "no HCI Reset");
    // The device descriptor, then the product string it names (no
    // manufacturer string: index 0)
    assert_eq!(controls.len(), 2);
    assert_eq!(controls[0].setup_packet(), [0x80, 0x06, 0x00, 0x01, 0x00, 0x00, 18, 0]);
    assert_eq!(controls[1].setup_packet(), [0x80, 0x06, 0x02, 0x03, 0x09, 0x04, 255, 0]);
    assert!(report.steps.iter().all(|s| s.answer.is_some()));
    assert_eq!(report.steps.iter().filter(|s| s.kind == StepKind::Usb).count(), 2);
    assert_eq!(report.error, None);
    let s = report.summary();
    assert_eq!((s.answered, s.steps), (10, 10));
    assert_eq!(s.hci_version, Some(12));
    assert_eq!(s.company, Some(0x0002));
    assert_eq!(s.company_name, Some("Intel"));
    assert_eq!(s.lmp_subversion, Some(0x3102));
    assert_eq!(s.address.as_deref(), Some("00:1A:7D:DA:71:13"));
    assert_eq!(s.name.as_deref(), Some("AX211"));
    assert_eq!(s.acl_buffers, Some((1021, 4)));
    assert_eq!(s.le_acl_buffers, Some((251, 3)));
    assert_eq!(s.usb_ids, Some((0x8087, 0x0033)));
    assert_eq!(s.usb_manufacturer, None);
    assert_eq!(s.usb_product.as_deref(), Some("Wireless_Device"));
}

#[test]
fn over_h4_the_usb_step_fails_and_is_kept() {
    let (report, _, controls) = run_with(intel_like, |_| Err("not a USB transport".into()));
    assert_eq!(controls.len(), 1, "no string requests without a device descriptor");
    let usb = report.steps.last().unwrap();
    assert_eq!((usb.kind, usb.answer.as_deref()), (StepKind::Usb, None));
    assert_eq!(usb.error.as_deref(), Some("not a USB transport"));
    assert_eq!(report.summary().usb_ids, None);
}

#[test]
fn a_silent_step_times_out_and_the_probe_goes_on() {
    let (report, sent) = run(|op| if op == 0x1002 { Vec::new() } else { intel_like(op) });
    assert_eq!(sent.len(), 8);
    let silent = report.steps.iter().find(|s| s.name == "Read Local Supported Commands").unwrap();
    assert_eq!(silent.answer, None);
    assert_eq!(report.summary().answered, 9);
}

#[test]
fn a_bootloader_that_answers_nothing_gives_a_report_of_timeouts() {
    let (report, sent) = run(|_| Vec::new());
    assert_eq!(sent.len(), 8);
    let s = report.summary();
    // Only USB answers
    assert_eq!((s.answered, s.hci_version, s.address), (2, None, None));
    assert_eq!(s.usb_ids, Some((0x8087, 0x0033)));
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
