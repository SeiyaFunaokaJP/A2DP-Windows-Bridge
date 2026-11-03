//! Tests of chip scripts (src/script.rs): parsing, what counts as a read,
//! and runs against a simulated controller

use std::collections::HashMap;

use bt_host::bringup::sim::{self, SimController, command_complete, command_status, event};
use bt_host::bringup::{ControlData, ControlKind, ControlRecipient, ControlRequest, Outcome};
use bt_host::probe::StepKind;
use bt_host::script::{Ack, ScriptLoader, Step, parse};

#[test]
fn parses_every_request() {
    let s = parse(
        "# a comment\n\
         hci 0x1001\n\
         hci fc05 ff   # Intel Read Version\n\
         bulk fc09 00 01\n\
         usb in vendor device 63 0 0 4\n\
         usb out vendor interface 64 0 1 01 02\n\
         wait 100\n\
         expect e4 02 01 timeout=5000\n\
         bootloader off\n\
         upload mediatek/patch.bin fc6f 01 01 chunk=240 ack=event:e4:02,01\n\
         upload-bulk p.bin fc09 01\n",
    )
    .unwrap();
    let steps: Vec<&Step> = s.lines.iter().map(|l| &l.step).collect();
    assert_eq!(s.lines[0].line, 2);
    assert_eq!(steps[0], &Step::Hci { opcode: 0x1001, params: vec![], bulk: false });
    assert_eq!(steps[1], &Step::Hci { opcode: 0xFC05, params: vec![0xFF], bulk: false });
    assert_eq!(s.lines[1].text, "hci fc05 ff");
    assert_eq!(steps[2], &Step::Hci { opcode: 0xFC09, params: vec![0, 1], bulk: true });
    assert_eq!(
        steps[3],
        &Step::Usb(ControlRequest {
            kind: ControlKind::Vendor,
            recipient: ControlRecipient::Device,
            request: 0x63,
            value: 0,
            index: 0,
            data: ControlData::In(4),
        })
    );
    assert!(matches!(steps[4], Step::Usb(ControlRequest { data: ControlData::Out(d), .. }) if d == &[1, 2]));
    assert_eq!(steps[5], &Step::Wait(100));
    assert_eq!(steps[6], &Step::Expect { code: 0xE4, prefix: vec![2, 1], timeout: 5000 });
    assert_eq!(steps[7], &Step::Bootloader(false));
    assert_eq!(
        steps[8],
        &Step::Upload {
            file: "mediatek/patch.bin".into(),
            opcode: 0xFC6F,
            header: vec![1, 1],
            chunk: 240,
            ack: Ack::Event { code: 0xE4, prefix: vec![2, 1] },
            bulk: false,
            timeout: 2000,
        }
    );
    assert!(matches!(steps[9], Step::Upload { bulk: true, opcode: 0xFC09, .. }));
}

#[test]
fn names_the_lines_it_cannot_read() {
    let errors = parse("hci 1001\nhci zz\nusb in vendor device 63 0 0\nfly away\nwait 999999\nupload ../x.bin fc00")
        .unwrap_err();
    assert_eq!(errors.len(), 5, "{errors:?}");
    assert!(errors[0].starts_with("line 2: 'zz'"));
    assert!(errors[1].starts_with("line 3: usb in"));
    assert!(errors[2].starts_with("line 4: 'fly' is not a request"));
    assert!(errors[3].starts_with("line 5:"));
    assert!(errors[4].starts_with("line 6: '../x.bin'"));
    assert!(parse("# only a comment\n").is_err());
}

#[test]
fn only_known_reads_need_no_permission() {
    let reads = parse(
        "hci 1001\nhci 1009\nhci 1405\nhci 0c14\nhci 2002\n\
         usb in standard device 06 0100 0 12\nwait 10\nexpect 0e\nbootloader on\n",
    )
    .unwrap();
    assert!(reads.writes().is_empty());
    let writes = parse(
        "hci 0c03\nhci fc05 ff\nhci 1402\nbulk 1001\nusb in vendor device 63 0 0 4\n\
         usb out standard device 09 1 0\nupload x.bin fc20\n",
    )
    .unwrap();
    assert_eq!(writes.writes().len(), 7);
    assert_eq!(writes.denied(&[0xFC20]).len(), 1);
}

/// A chip: Read Local Version answered, the vendor register read, an
/// upload acknowledged by event 0xE4 [02 01 seq]
struct Chip {
    received: Vec<Vec<u8>>,
    silent: bool,
}

impl SimController for Chip {
    fn command(&mut self, c: &[u8]) -> Vec<Vec<u8>> {
        self.received.push(c.to_vec());
        if self.silent {
            return Vec::new();
        }
        match sim::opcode(c) {
            0xFC6F => vec![command_status(0xFC6F, 0), event(0xE4, &[2, 1, self.received.len() as u8])],
            0xFC20 => vec![command_complete(0xFC20, 0x12, &[])],
            op => vec![command_complete(op, 0, &[0xAA])],
        }
    }

    fn control(&mut self, r: &ControlRequest) -> Result<Vec<u8>, String> {
        match r.data {
            ControlData::In(n) => Ok((0..n as u8).collect()),
            ControlData::Out(_) => Ok(Vec::new()),
        }
    }
}

fn run(text: &str, chip: &mut Chip, files: HashMap<&'static str, Vec<u8>>) -> bt_host::probe::ProbeReport {
    let script = parse(text).unwrap();
    let mut loader = ScriptLoader::new(script, Box::new(move |name| files.get(name).cloned()));
    let r = sim::run(&mut loader, chip);
    assert_eq!(r.result, Ok(Outcome::Probed));
    assert!(r.bootloader || text.contains("bootloader off"), "no HCI Reset on close");
    use bt_host::bringup::ChipLoader;
    loader.take_report().unwrap()
}

#[test]
fn runs_a_script_and_records_each_line() {
    let mut chip = Chip { received: Vec::new(), silent: false };
    let files = HashMap::from([("patch.bin", vec![7u8; 500])]);
    let report = run(
        "hci 1001\nusb in vendor device 63 0 0 4\nwait 50\n\
         upload patch.bin fc6f 01 01 chunk=240 ack=event:e4:02,01\nusb out vendor device 64 0 0 01\n",
        &mut chip,
        files,
    );
    assert_eq!(report.error, None);
    let names: Vec<&str> = report.steps.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names[0], "line 1: hci 1001");
    assert_eq!(names[3], "line 4: upload patch.bin fc6f 01 01 chunk=240 ack=event:e4:02,01 (3 commands)");
    assert_eq!(report.steps[1].kind, StepKind::Usb);
    assert_eq!(report.steps[1].answer.as_deref(), Some(&[0u8, 1, 2, 3][..]));
    assert_eq!(report.steps[2].kind, StepKind::Other);
    // 500 bytes: 240 + 240 + 20, each with the header
    let uploads: Vec<&Vec<u8>> = chip.received.iter().filter(|c| sim::opcode(c) == 0xFC6F).collect();
    assert_eq!(uploads.len(), 3);
    assert_eq!(&uploads[2][..5], &[0x6F, 0xFC, 22, 1, 1]);
    assert!(report.other_events.is_empty(), "the Command Status of an event-acknowledged upload is not stray");
}

#[test]
fn stops_at_the_first_failure() {
    let mut chip = Chip { received: Vec::new(), silent: false };
    let files = HashMap::from([("x.bin", vec![1u8; 300])]);
    let report = run("upload x.bin fc20 chunk=200\nhci 1001\n", &mut chip, files);
    assert_eq!(report.error.as_deref(), Some("line 1: command 0xFC20 answered with status 0x12"));
    assert_eq!(chip.received.len(), 1, "nothing after the failure");
    assert_eq!(report.steps.len(), 1);

    let mut chip = Chip { received: Vec::new(), silent: true };
    let report = run("hci 1001\nhci 1009\n", &mut chip, HashMap::new());
    assert_eq!(report.error.as_deref(), Some("line 1: no answer"));

    let mut chip = Chip { received: Vec::new(), silent: false };
    let report = run("upload missing.bin fc6f\n", &mut chip, HashMap::new());
    assert_eq!(report.error.as_deref(), Some("line 1: missing.bin is not in the firmware folder"));
    assert!(chip.received.is_empty());

    let mut chip = Chip { received: Vec::new(), silent: false };
    let report = run("expect ff 07 timeout=100\n", &mut chip, HashMap::new());
    assert_eq!(report.error.as_deref(), Some("line 1: the event did not come"));
}
