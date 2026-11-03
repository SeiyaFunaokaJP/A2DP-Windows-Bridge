//! Tests of the Intel firmware loader (src/intel.rs): file parsing, and the
//! loader against a simulated controller

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use bt_host::intel::*;

// ---- packets ----

fn le16(v: u16) -> [u8; 2] {
    v.to_le_bytes()
}

fn command_complete(opcode: u16, ret: &[u8]) -> Vec<u8> {
    let [lo, hi] = le16(opcode);
    let mut e = vec![0x0E, (3 + ret.len()) as u8, 1, lo, hi];
    e.extend_from_slice(ret);
    e
}

fn vendor(code: u8, data: &[u8]) -> Vec<u8> {
    let mut e = vec![0xFF, (1 + data.len()) as u8, code];
    e.extend_from_slice(data);
    e
}

fn opcode(command: &[u8]) -> u16 {
    u16::from_le_bytes([command[0], command[1]])
}

fn tlv(t: u8, v: &[u8]) -> Vec<u8> {
    let mut out = vec![t, v.len() as u8];
    out.extend_from_slice(v);
    out
}

/// CNVi / CNVr top register for a file name part: "0040" is type 0x400,
/// step 0
fn top(kind: u32, step: u32) -> u32 {
    (step << 24) | kind
}

/// cnvi_bt of an Intel controller of `variant`
fn cnvi_bt(variant: u32) -> u32 {
    (variant << 16) | (0x37 << 8)
}

/// Read Version return parameters in TLV form
fn tlv_version(variant: u32, img_type: u8, sbe_type: u8, fw_id: Option<&str>) -> Vec<u8> {
    let mut ret = vec![0x00];
    ret.extend(tlv(0x10, &top(0x400, 0).to_le_bytes()));
    ret.extend(tlv(0x11, &top(0x410, 0).to_le_bytes()));
    ret.extend(tlv(0x12, &cnvi_bt(variant).to_le_bytes()));
    ret.extend(tlv(0x1C, &[img_type]));
    ret.extend(tlv(0x2D, &[0x0A, 0x21, 0x15]));
    ret.extend(tlv(0x2F, &[sbe_type]));
    // A field the loader does not know
    ret.extend(tlv(0x24, &[0x02, 0x00]));
    if let Some(id) = fw_id {
        ret.extend(tlv(0x50, id.as_bytes()));
    }
    ret
}

/// Legacy Read Version return parameters
fn legacy_version(hw_variant: u8, hw_revision: u8, fw_variant: u8, fw_revision: u8) -> Vec<u8> {
    vec![0x00, 0x37, hw_variant, hw_revision, fw_variant, fw_revision, 0x0A, 0x21, 0x15, 0x00]
}

/// Read Secure Boot Params return parameters
fn boot_params(dev_revid: u16, limited_cce: u8) -> Vec<u8> {
    let mut ret = vec![0u8; 23];
    ret[4..6].copy_from_slice(&le16(dev_revid));
    ret[6] = 1;
    ret[21] = limited_cce;
    ret
}

// ---- firmware files ----

const BOOT_ADDRESS: u32 = 0x0004_0800;

/// Write Boot Params, an Intel NOP and a 256-byte command: 16 + 256 bytes
fn command_buffer() -> Vec<u8> {
    let mut b = vec![0x0E, 0xFC, 7];
    b.extend_from_slice(&BOOT_ADDRESS.to_le_bytes());
    b.extend_from_slice(&[0x0A, 0x22, 0x15]);
    b.extend_from_slice(&[0x02, 0xFC, 3, 0, 0, 0]);
    b.extend_from_slice(&[0x01, 0xFC, 253]);
    b.extend((0..253).map(|i| i as u8));
    b
}

/// An .sfi: RSA header (and ECDSA header after it), then the command buffer
fn sfi(with_ecdsa: bool) -> Vec<u8> {
    let header = if with_ecdsa { 964 } else { 644 };
    let mut d: Vec<u8> = (0..header).map(|i| (i % 251) as u8).collect();
    d[8..12].copy_from_slice(&0x0001_0000u32.to_le_bytes());
    if with_ecdsa {
        d[644] = 0x06;
        d[652..656].copy_from_slice(&0x0002_0000u32.to_le_bytes());
    }
    d.extend(command_buffer());
    d
}

fn hybrid_sfi() -> Vec<u8> {
    let mut d: Vec<u8> = (0..2116).map(|i| (i % 249) as u8).collect();
    d[8..12].copy_from_slice(&0x0006_9700u32.to_le_bytes());
    d.extend(command_buffer());
    d
}

fn ddc() -> Vec<u8> {
    // Two records: length, ID (2 bytes), value
    vec![0x03, 0x01, 0x00, 0xAA, 0x04, 0x02, 0x00, 0xBB, 0xCC]
}

// ---- parsing ----

#[test]
fn tlv_version_and_file_name() {
    let Ok(Version::Tlv(v)) = parse_version(&tlv_version(0x18, IMG_BOOTLOADER, 1, None)) else { panic!() };
    assert_eq!(v.hw_variant(), 0x18);
    assert_eq!(v.hw_platform(), 0x37);
    assert_eq!(v.img_type, IMG_BOOTLOADER);
    assert_eq!(v.sbe_type, 1);
    assert_eq!(v.min_fw, (0x0A, 0x21, 0x15));
    assert_eq!(v.firmware_stem(), "ibt-0040-0041");
    // The step goes into the low nibble before the byte swap
    let v = TlvVersion { cnvi_top: top(0x400, 0), cnvr_top: top(0x410, 2), ..v };
    assert_eq!(v.firmware_stem(), "ibt-0040-0241");
}

#[test]
fn blazar_file_names() {
    let Ok(Version::Tlv(v)) = parse_version(&tlv_version(0x1E, IMG_BOOTLOADER, 1, Some("0190"))) else { panic!() };
    assert_eq!(v.firmware_stem(), "ibt-0040-0041-iml");
    let v = TlvVersion { img_type: IMG_IML, ..v };
    assert_eq!(v.firmware_stem(), "ibt-0040-0041-0190");
    let v = TlvVersion { fw_id: String::new(), ..v };
    assert_eq!(v.firmware_stem(), "ibt-0040-0041");
}

#[test]
fn version_answers() {
    let Ok(Version::Legacy(v)) = parse_version(&legacy_version(0x12, 0x10, 0x06, 0x01)) else { panic!() };
    assert_eq!((v.hw_variant, v.fw_variant), (0x12, 0x06));
    assert_eq!(legacy_firmware_stem(&v, 0), Some("ibt-18-16-1".into()));
    let sfp = LegacyVersion { hw_variant: 0x0B, ..v };
    assert_eq!(legacy_firmware_stem(&sfp, 5), Some("ibt-11-5".into()));
    assert_eq!(legacy_firmware_stem(&LegacyVersion { hw_variant: 0x08, ..v }, 0), None);
    assert_eq!(parse_version(&[0x01]), Err(VersionError::Status(0x01)));
    assert_eq!(parse_version(&[]), Err(VersionError::Malformed));
    // A TLV that runs past the end, one too short for its type
    assert_eq!(parse_version(&[0x00, 0x12, 0x04, 0x00]), Err(VersionError::Malformed));
    assert_eq!(parse_version(&[0x00, 0x12, 0x02, 0x00, 0x18]), Err(VersionError::Malformed));
    // A later answer keeps what it lacks
    let Ok(Version::Tlv(mut v)) = parse_version(&tlv_version(0x18, IMG_BOOTLOADER, 1, None)) else { panic!() };
    v.update(&tlv(0x1C, &[IMG_OPERATIONAL])).unwrap();
    assert_eq!((v.img_type, v.sbe_type, v.hw_variant()), (IMG_OPERATIONAL, 1, 0x18));
}

#[test]
fn sfi_headers() {
    // RSA engine: the RSA header, payload behind the ECDSA header
    let image = parse_sfi(&sfi(true), 0x18, 0).unwrap();
    assert_eq!(image.header, Header::Rsa);
    assert_eq!(image.commands[0][3], 0x00);
    assert_eq!(&image.commands[0][4..8], &[0, 1, 2, 3]);
    // ECDSA engine: the second header
    let data = sfi(true);
    let image = parse_sfi(&data, 0x18, 1).unwrap();
    assert_eq!(image.header, Header::Ecdsa);
    assert_eq!(&image.commands[0][4..], &data[644..772]);
    assert_eq!((image.commands[1][3], image.commands[1].len()), (0x03, 4 + 96));
    assert_eq!((image.commands[2][3], &image.commands[2][4..]), (0x02, &data[868..964]));
    assert_eq!(image.boot_address, BOOT_ADDRESS);
    assert_eq!(image.build, Some((0x0A, 0x22, 0x15)));
    // Data: the 16 aligned bytes, then 256 bytes in 252 + 4
    let data_commands: Vec<&Vec<u8>> = image.commands.iter().filter(|c| c[3] == 0x01).collect();
    let sizes: Vec<usize> = data_commands.iter().map(|c| c.len() - 4).collect();
    assert_eq!(sizes, [16, 252, 4]);
    assert!(image.commands.iter().all(|c| c[..2] == [0x09, 0xFC] && c[2] as usize == c.len() - 3));
    // Legacy controllers: RSA only, payload at 644
    let image = parse_sfi(&sfi(false), 0x12, 0).unwrap();
    assert_eq!(image.commands.iter().map(|c| c[3]).take(5).collect::<Vec<_>>(), [0x00, 0x03, 0x03, 0x02, 0x02]);
    // Hybrid (Scorpius Peak 2): CSS, ECDSA key / signature, LMS key / signature
    let image = parse_sfi(&hybrid_sfi(), 0x20, 0).unwrap();
    assert_eq!(image.header, Header::Hybrid);
    let types: Vec<u8> = image.commands.iter().map(|c| c[3]).take(5).collect();
    assert_eq!(types, [0x00, 0x03, 0x02, 0x05, 0x04]);
    let lms: usize = image.commands.iter().filter(|c| c[3] == 0x04).map(|c| c.len() - 4).sum();
    assert_eq!(lms, 1744);
}

#[test]
fn sfi_errors() {
    assert!(parse_sfi(&[0u8; 8], 0x18, 0).is_err());
    // A legacy file for a TLV controller, the other way round
    assert!(parse_sfi(&sfi(false), 0x18, 0).is_err());
    assert!(parse_sfi(&sfi(true), 0x12, 1).is_err());
    assert!(parse_sfi(&sfi(true), 0x18, 2).is_err());
    assert!(parse_sfi(&hybrid_sfi(), 0x18, 0).is_err());
    // A command buffer cut short, one not 4-byte aligned
    let mut cut = sfi(true);
    cut.truncate(cut.len() - 1);
    assert!(parse_sfi(&cut, 0x18, 0).is_err());
    let mut unaligned = sfi(true);
    unaligned.extend_from_slice(&[0x02, 0xFC, 0]);
    assert!(parse_sfi(&unaligned, 0x18, 0).is_err());
    // No command buffer
    assert!(parse_sfi(&sfi(true)[..964], 0x18, 0).is_err());
}

#[test]
fn ddc_records() {
    let commands = parse_ddc(&ddc()).unwrap();
    assert_eq!(
        commands,
        [vec![0x8B, 0xFC, 4, 0x03, 0x01, 0x00, 0xAA], vec![0x8B, 0xFC, 5, 0x04, 0x02, 0x00, 0xBB, 0xCC]]
    );
    assert!(parse_ddc(&[0x05, 0x01, 0x00]).is_err());
    assert!(parse_ddc(&[0x01, 0x01]).is_err());
    assert_eq!(parse_ddc(&[]).unwrap(), Vec::<Vec<u8>>::new());
}

// ---- the loader against a simulated controller ----

/// What the simulated controller does
#[derive(Clone, Copy, PartialEq, Eq)]
enum Fault {
    None,
    /// No answer to anything
    Silent,
    /// Vendor event 0x06 with this result
    Reject(u8),
    /// Vendor event 0x06 before the last fragment's Command Complete
    EarlyResult,
}

struct Controller {
    /// Read Version answers, one per call
    versions: VecDeque<Vec<u8>>,
    boot_params: Vec<u8>,
    fault: Fault,
    /// Secure Send commands of the image in progress
    expected: usize,
    received: usize,
    bulk_events: bool,
    commands: Vec<Vec<u8>>,
    bulk: Vec<Vec<u8>>,
    resets: Vec<Vec<u8>>,
    bulk_toggles: Vec<bool>,
}

impl Controller {
    fn new(versions: Vec<Vec<u8>>) -> Controller {
        Controller {
            versions: versions.into(),
            boot_params: boot_params(0, 0),
            fault: Fault::None,
            expected: 0,
            received: 0,
            bulk_events: false,
            commands: Vec::new(),
            bulk: Vec::new(),
            resets: Vec::new(),
            bulk_toggles: Vec::new(),
        }
    }

    /// Events the controller sends for a command
    fn answer(&mut self, command: &[u8]) -> Vec<Vec<u8>> {
        if self.fault == Fault::Silent {
            return Vec::new();
        }
        let op = opcode(command);
        match op {
            0xFC05 => vec![command_complete(op, &self.versions.pop_front().expect("unexpected Read Version"))],
            0xFC0D => vec![command_complete(op, &self.boot_params)],
            0xFC01 => {
                self.resets.push(command.to_vec());
                vec![command_complete(op, &[0x00]), vendor(0x02, &[0, 1, 0, 0, 0, 0])]
            }
            _ => vec![command_complete(op, &[0x00])],
        }
    }

    fn answer_bulk(&mut self, command: &[u8]) -> Vec<Vec<u8>> {
        assert!(self.bulk_events, "Secure Send without bootloader events on");
        assert_eq!(opcode(command), 0xFC09);
        self.received += 1;
        let complete = command_complete(0xFC09, &[0x00]);
        if self.received < self.expected {
            return vec![complete];
        }
        let result = vendor(0x06, &[if let Fault::Reject(r) = self.fault { r } else { 0 }, 0x09, 0xFC, 0x00]);
        if self.fault == Fault::EarlyResult { vec![result, complete] } else { vec![complete, result] }
    }
}

struct Run {
    result: Option<Result<Outcome, LoadError>>,
    files_read: Vec<String>,
}

/// Runs the loader to its end; `images`: the Secure Send command count of
/// each image in download order
fn run(ctl: &mut Controller, files: HashMap<String, Vec<u8>>, images: &[(u8, u8, &str)]) -> Run {
    let read: Rc<RefCell<Vec<String>>> = Rc::default();
    let log = read.clone();
    let files_for_reader = files.clone();
    let mut loader = Loader::new(Box::new(move |name| {
        log.borrow_mut().push(name.to_owned());
        files_for_reader.get(name).cloned()
    }));
    let mut images: VecDeque<usize> = images
        .iter()
        .map(|&(variant, sbe, name)| parse_sfi(&files[name], variant, sbe).unwrap().commands.len())
        .collect();
    let mut actions: VecDeque<Action> = loader.start().into();
    let mut timer = false;
    let mut result = None;
    while result.is_none() {
        let Some(action) = actions.pop_front() else {
            // Nothing more to do: the timer fires
            assert!(timer, "loader stuck without a timer");
            actions.extend(loader.on_timeout());
            continue;
        };
        let events = match action {
            Action::Command(c) => {
                ctl.commands.push(c.clone());
                ctl.answer(&c)
            }
            Action::BulkCommand(c) => {
                if ctl.received == ctl.expected {
                    ctl.expected = images.pop_front().expect("more images than expected");
                    ctl.received = 0;
                }
                ctl.bulk.push(c.clone());
                ctl.answer_bulk(&c)
            }
            Action::Bootloader(on) => {
                ctl.bulk_events = on;
                ctl.bulk_toggles.push(on);
                Vec::new()
            }
            Action::Timer(_) => {
                timer = true;
                Vec::new()
            }
            Action::Done(r) => {
                result = Some(r);
                Vec::new()
            }
        };
        for e in events {
            actions.extend(loader.on_event(&e));
        }
    }
    assert!(loader.done());
    assert!(loader.on_event(&command_complete(0xFC05, &[0])).is_empty(), "events after the end");
    let files_read = read.borrow().clone();
    Run { result, files_read }
}

fn files(entries: &[(&str, Vec<u8>)]) -> HashMap<String, Vec<u8>> {
    entries.iter().map(|(n, d)| (n.to_string(), d.clone())).collect()
}

#[test]
fn tlv_download_boot_and_ddc() {
    // AX211-like: bootloader, ECDSA engine; operational after the boot
    let mut ctl = Controller::new(vec![
        tlv_version(0x18, IMG_BOOTLOADER, 1, None),
        [vec![0x00], tlv(0x1C, &[IMG_OPERATIONAL])].concat(),
    ]);
    let f = files(&[("ibt-0040-0041.sfi", sfi(true)), ("ibt-0040-0041.ddc", ddc())]);
    let run = run(&mut ctl, f, &[(0x18, 1, "ibt-0040-0041.sfi")]);
    assert_eq!(
        run.result,
        Some(Ok(Outcome::Loaded { firmware: "ibt-0040-0041.sfi".into(), detail: Some("DDC applied") }))
    );
    assert_eq!(run.files_read, ["ibt-0040-0041.sfi", "ibt-0040-0041.ddc"]);
    // Read Version with 0xFF, first the ECDSA header
    assert_eq!(ctl.commands[0], [0x05, 0xFC, 1, 0xFF]);
    assert_eq!(&ctl.bulk[0][4..], &sfi(true)[644..772]);
    // Intel Reset: soft, patch enable, boot from the image's address
    let mut reset = vec![0x01, 0xFC, 8, 0x00, 0x01, 0x00, 0x01];
    reset.extend_from_slice(&BOOT_ADDRESS.to_le_bytes());
    assert_eq!(ctl.resets, [reset]);
    let ops: Vec<u16> = ctl.commands.iter().map(|c| opcode(c)).collect();
    assert_eq!(ops, [0xFC05, 0xFC01, 0xFC05, 0xFC8B, 0xFC8B, 0xFC52]);
    assert_eq!(ctl.bulk_toggles, [true, false]);
}

#[test]
fn result_before_the_last_command_complete() {
    let mut ctl = Controller::new(vec![
        tlv_version(0x17, IMG_BOOTLOADER, 0, None),
        [vec![0x00], tlv(0x1C, &[IMG_OPERATIONAL])].concat(),
    ]);
    ctl.fault = Fault::EarlyResult;
    let f = files(&[("ibt-0040-0041.sfi", sfi(true))]);
    let run = run(&mut ctl, f, &[(0x17, 0, "ibt-0040-0041.sfi")]);
    // No .ddc: loaded anyway
    assert_eq!(
        run.result,
        Some(Ok(Outcome::Loaded { firmware: "ibt-0040-0041.sfi".into(), detail: Some("DDC not applied") }))
    );
    assert_eq!(ctl.resets.len(), 1);
}

#[test]
fn running_firmware_is_left_alone() {
    let mut ctl = Controller::new(vec![tlv_version(0x18, IMG_OPERATIONAL, 1, None)]);
    let run = run(&mut ctl, HashMap::new(), &[]);
    assert_eq!(run.result, Some(Ok(Outcome::AlreadyRunning)));
    assert_eq!(ctl.commands.len(), 1);
    assert!(run.files_read.is_empty());
    // Bootloader mode from the first command on, off at the end
    assert_eq!(ctl.bulk_toggles, [true, false]);
}

#[test]
fn missing_firmware_file() {
    let mut ctl = Controller::new(vec![tlv_version(0x18, IMG_BOOTLOADER, 1, None)]);
    let run = run(&mut ctl, HashMap::new(), &[]);
    assert_eq!(run.result, Some(Err(LoadError::MissingFile("ibt-0040-0041.sfi".into()))));
    assert!(ctl.bulk.is_empty());
}

#[test]
fn wrong_file_for_the_controller() {
    let mut ctl = Controller::new(vec![tlv_version(0x18, IMG_BOOTLOADER, 1, None)]);
    let f = files(&[("ibt-0040-0041.sfi", sfi(false))]);
    let run = run(&mut ctl, f, &[]);
    assert!(matches!(run.result, Some(Err(LoadError::BadFile { .. }))));
}

#[test]
fn rejected_image() {
    let mut ctl = Controller::new(vec![tlv_version(0x18, IMG_BOOTLOADER, 1, None)]);
    ctl.fault = Fault::Reject(0x01);
    let f = files(&[("ibt-0040-0041.sfi", sfi(true))]);
    let run = run(&mut ctl, f, &[(0x18, 1, "ibt-0040-0041.sfi")]);
    assert_eq!(run.result, Some(Err(LoadError::Rejected(0x01))));
    assert!(ctl.resets.is_empty());
    // Still in its bootloader: no HCI Reset on close
    assert_eq!(ctl.bulk_toggles, [true]);
}

#[test]
fn no_answer() {
    let mut ctl = Controller::new(vec![]);
    ctl.fault = Fault::Silent;
    let run = run(&mut ctl, HashMap::new(), &[]);
    assert_eq!(run.result, Some(Err(LoadError::Timeout("Read Version"))));
}

#[test]
fn read_version_sent_again() {
    // The answer to the first Read Version is lost (a bootloader with a
    // stale event of an earlier session); the second one is answered
    let mut ctl = Controller::new(vec![tlv_version(0x18, IMG_OPERATIONAL, 1, None)]);
    let mut loader = Loader::new(Box::new(|_| None));
    let first = loader.start();
    assert_eq!(first[..2], [Action::Bootloader(true), Action::Command(vec![0x05, 0xFC, 1, 0xFF])]);
    let again = loader.on_timeout();
    assert_eq!(again[0], Action::Command(vec![0x05, 0xFC, 1, 0xFF]));
    // A stale answer to another command is ignored
    assert!(loader.on_event(&command_complete(0x0C03, &[0x01])).is_empty());
    let answer = ctl.answer(&[0x05, 0xFC, 1, 0xFF]);
    assert_eq!(loader.on_event(&answer[0]), [Action::Bootloader(false), Action::Done(Ok(Outcome::AlreadyRunning))]);
    // Three tries, then the timeout
    let mut loader = Loader::new(Box::new(|_| None));
    loader.start();
    assert!(matches!(loader.on_timeout()[0], Action::Command(_)));
    assert!(matches!(loader.on_timeout()[0], Action::Command(_)));
    // Still in its bootloader as far as anyone knows
    assert_eq!(loader.on_timeout(), [Action::Done(Err(LoadError::Timeout("Read Version")))]);
}

#[test]
fn unknown_variant() {
    let mut ctl = Controller::new(vec![tlv_version(0x2A, IMG_BOOTLOADER, 1, None)]);
    let run = run(&mut ctl, HashMap::new(), &[]);
    assert_eq!(run.result, Some(Err(LoadError::UnsupportedVariant(0x2A))));
}

#[test]
fn blazar_loads_the_intermediate_loader_first() {
    let mut ctl = Controller::new(vec![
        tlv_version(0x1E, IMG_BOOTLOADER, 1, None),
        [vec![0x00], tlv(0x1C, &[IMG_IML]), tlv(0x50, b"0190")].concat(),
        [vec![0x00], tlv(0x1C, &[IMG_OPERATIONAL])].concat(),
    ]);
    let f = files(&[
        ("ibt-0040-0041-iml.sfi", sfi(true)),
        ("ibt-0040-0041-0190.sfi", sfi(true)),
        ("ibt-0040-0041-0190.ddc", ddc()),
    ]);
    let run = run(&mut ctl, f, &[(0x1E, 1, "ibt-0040-0041-iml.sfi"), (0x1E, 1, "ibt-0040-0041-0190.sfi")]);
    assert_eq!(
        run.result,
        Some(Ok(Outcome::Loaded { firmware: "ibt-0040-0041-0190.sfi".into(), detail: Some("DDC applied") }))
    );
    assert_eq!(run.files_read, ["ibt-0040-0041-iml.sfi", "ibt-0040-0041-0190.sfi", "ibt-0040-0041-0190.ddc"]);
    assert_eq!(ctl.resets.len(), 2);
    // Bootloader mode through the intermediate loader, off once the
    // operational firmware runs
    assert_eq!(ctl.bulk_toggles, [true, false]);
}

#[test]
fn tlv_bootloaders_that_are_not_served() {
    // Not every fragment acknowledged, an unknown secure boot engine
    for version in [
        [tlv_version(0x18, IMG_BOOTLOADER, 1, None), tlv(0x2E, &[0x01])].concat(),
        tlv_version(0x18, IMG_BOOTLOADER, 4, None),
    ] {
        let mut ctl = Controller::new(vec![version]);
        let run = run(&mut ctl, HashMap::new(), &[]);
        assert!(matches!(run.result, Some(Err(LoadError::Unsupported(_)))), "{:?}", run.result);
        assert!(run.files_read.is_empty());
    }
}

#[test]
fn legacy_bootloader() {
    // 9260-like (ThP): legacy answer, Secure Boot Params, RSA image
    let mut ctl = Controller::new(vec![legacy_version(0x12, 0x10, 0x06, 0x01)]);
    let f = files(&[("ibt-18-16-1.sfi", sfi(false)), ("ibt-18-16-1.ddc", ddc())]);
    let run = run(&mut ctl, f, &[(0x12, 0, "ibt-18-16-1.sfi")]);
    assert_eq!(
        run.result,
        Some(Ok(Outcome::Loaded { firmware: "ibt-18-16-1.sfi".into(), detail: Some("DDC applied") }))
    );
    let ops: Vec<u16> = ctl.commands.iter().map(|c| opcode(c)).collect();
    // No Read Version after the boot: the DDC name is known
    assert_eq!(ops, [0xFC05, 0xFC0D, 0xFC01, 0xFC8B, 0xFC8B, 0xFC52]);
}

#[test]
fn legacy_cases() {
    // Operational firmware, a ROM controller, a bootloader without
    // acknowledged fragments
    let mut ctl = Controller::new(vec![legacy_version(0x12, 0x10, 0x23, 0x01)]);
    assert_eq!(run(&mut ctl, HashMap::new(), &[]).result, Some(Ok(Outcome::AlreadyRunning)));
    let mut ctl = Controller::new(vec![legacy_version(0x08, 0x00, 0x00, 0x00)]);
    assert_eq!(run(&mut ctl, HashMap::new(), &[]).result, Some(Ok(Outcome::Rom)));
    let mut ctl = Controller::new(vec![legacy_version(0x0B, 0x00, 0x06, 0x00)]);
    ctl.boot_params = boot_params(5, 1);
    assert!(matches!(run(&mut ctl, HashMap::new(), &[]).result, Some(Err(LoadError::Unsupported(_)))));
}

#[test]
fn legacy_generation_answering_in_tlv() {
    // AX200 running its firmware answers in TLV; the legacy answer decides
    let mut ctl =
        Controller::new(vec![tlv_version(0x14, IMG_OPERATIONAL, 0, None), legacy_version(0x14, 0x10, 0x23, 0x01)]);
    let run = run(&mut ctl, HashMap::new(), &[]);
    assert_eq!(run.result, Some(Ok(Outcome::AlreadyRunning)));
    assert_eq!(ctl.commands[1], [0x05, 0xFC, 0]);
}
