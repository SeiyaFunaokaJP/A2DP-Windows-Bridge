//! bluecaster-driver.exe: puts a USB Bluetooth adapter on Windows' WinUSB driver
//! and back (win-sys::driver), for the core service and the UI.
//!
//! ```text
//! bluecaster-driver list [--all] [--out <file>]
//! bluecaster-driver state <instance-id> [--out <file>]
//! bluecaster-driver assign <instance-id> [--out <file>]
//! bluecaster-driver restore <instance-id> [--previous <json> | --previous @<file>] [--out <file>]
//! bluecaster-driver restore --records <dir> [--out <file>]
//! ```
//!
//! The result is one JSON object on stdout (and in `--out`), logs go to
//! stderr (and to the file of `--log <file>`, at debug level with
//! `--verbose`: every command takes both), nothing is asked interactively. `assign` and `restore` need
//! administrator rights; `list` and `state` do not.
//!
//! Elevation: the exe has no manifest asking for administrator rights, so
//! `list` and `state` run without a UAC prompt. The caller starts `assign`
//! and `restore` with win-sys::elevation::run_elevated (ShellExecuteEx
//! "runas"). An elevated process started that way has no stdout the caller
//! can read, hence `--out <file>`: the caller names a file and reads the
//! JSON from it after the exit code. Run unelevated, `assign` and `restore`
//! stop at once with exit code 3.
//!
//! `assign` prints the driver the adapter had before as `previous`; the
//! caller keeps it and passes it back to `restore --previous` (the whole
//! `assign` result is accepted too), which then reports whether the same
//! driver came back.
//!
//! `restore --records <dir>` restores every adapter that has an assignment
//! record in `<dir>` (the core keeps them in `<settings>\driver\`, one
//! `assign` result per adapter) and deletes the records of the adapters
//! given back. The uninstaller uses it: one UAC prompt for all adapters.

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::ExitCode;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use win_sys::driver::{self, DriverError, DriverInfo, DriverRecord, DriverState};
use win_sys::usb_adapters::{self, UsbDevice};

/// Exit codes, also in the JSON as "exit_code"
mod exit {
    pub const OK: u8 = 0;
    /// The operation failed (details in "error")
    pub const FAILED: u8 = 1;
    /// Bad command line
    pub const USAGE: u8 = 2;
    /// assign / restore without administrator rights
    pub const NOT_ELEVATED: u8 = 3;
    /// No such device, or not a USB device
    pub const NO_DEVICE: u8 = 4;
    /// Done, but the result is not what was asked for: after assign the
    /// adapter is not on WinUSB, after restore it did not come back or has
    /// another driver than the recorded one
    pub const UNVERIFIED: u8 = 5;
}

const USAGE: &str = "usage:
  bluecaster-driver list [--all] [--out <file>]
  bluecaster-driver state <instance-id> [--out <file>]
  bluecaster-driver assign <instance-id> [--out <file>]
  bluecaster-driver restore <instance-id> [--previous <json> | --previous @<file>] [--out <file>]
  bluecaster-driver restore --records <dir> [--out <file>]
every command: [--log <file>] [--verbose]";

#[derive(Debug, PartialEq)]
enum Command {
    List { all: bool },
    State { instance_id: String },
    Assign { instance_id: String },
    Restore { instance_id: String, previous: Option<String> },
    RestoreRecords { dir: PathBuf },
}

#[derive(Debug, PartialEq)]
struct Args {
    command: Command,
    out: Option<PathBuf>,
}

/// A log file this big is moved to `<name>.old` before the next run writes
/// to it
const LOG_ROTATE_BYTES: u64 = 1024 * 1024;

/// `--log <file>` (append the log to it as well) and `--verbose` (debug
/// level): the core passes them so that the elevated run's log ends up in
/// its logs folder. They are taken out before the command is parsed.
fn log_args(argv: Vec<String>) -> (Option<PathBuf>, bool, Vec<String>) {
    let (mut log, mut verbose, mut rest) = (None, false, Vec::new());
    let mut it = argv.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--log" => log = it.next().map(PathBuf::from),
            "--verbose" => verbose = true,
            _ => rest.push(a),
        }
    }
    (log, verbose, rest)
}

fn init_logging(log: Option<&std::path::Path>, verbose: bool) {
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    let level = if verbose { "debug" } else { "info" };
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(level));
    let file = log.and_then(|path| {
        if std::fs::metadata(path).is_ok_and(|m| m.len() > LOG_ROTATE_BYTES) {
            let _ = std::fs::rename(path, path.with_extension("log.old"));
        }
        std::fs::OpenOptions::new().create(true).append(true).open(path).ok()
    });
    let file_layer =
        file.map(|f| tracing_subscriber::fmt::layer().with_writer(std::sync::Mutex::new(f)).with_ansi(false));
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr).with_ansi(false))
        .with(file_layer)
        .try_init();
}

fn main() -> ExitCode {
    let (log, verbose, argv) = log_args(std::env::args().skip(1).collect());
    init_logging(log.as_deref(), verbose);
    tracing::info!(
        "bluecaster-driver {} ({}), elevated {}: {}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::ARCH,
        win_sys::elevation::is_elevated(),
        argv.join(" ")
    );

    let (code, result, out) = match parse_args(&argv) {
        Ok(args) => {
            let (code, result) = run(&args.command);
            (code, result, args.out)
        }
        Err(message) => {
            eprintln!("{message}\n{USAGE}");
            (exit::USAGE, error_json("usage", &message), None)
        }
    };
    let mut result = result;
    result["exit_code"] = json!(code);
    let text = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".into());
    tracing::info!("exit {code}: {result}");
    println!("{text}");
    if let Some(path) = out
        && let Err(e) = std::fs::write(&path, &text)
    {
        eprintln!("could not write {}: {e}", path.display());
        return ExitCode::from(exit::FAILED);
    }
    ExitCode::from(code)
}

fn parse_args(argv: &[String]) -> Result<Args, String> {
    let mut positional = Vec::new();
    let mut out = None;
    let mut previous = None;
    let mut records = None;
    let mut all = false;
    let mut it = argv.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--out" => out = Some(PathBuf::from(it.next().ok_or("--out needs a file")?)),
            "--previous" => previous = Some(it.next().ok_or("--previous needs a value")?.clone()),
            "--records" => records = Some(PathBuf::from(it.next().ok_or("--records needs a folder")?)),
            "--all" => all = true,
            s if s.starts_with("--") => return Err(format!("unknown option {s}")),
            s => positional.push(s.to_string()),
        }
    }
    let mut positional = positional.into_iter();
    let command = positional.next().ok_or("no command")?;
    let mut instance_id = || positional.next().ok_or(format!("{command} needs an instance ID"));
    let command = match command.as_str() {
        "list" => Command::List { all },
        "state" => Command::State { instance_id: instance_id()? },
        "assign" => Command::Assign { instance_id: instance_id()? },
        "restore" if previous.is_none() && records.is_some() => {
            Command::RestoreRecords { dir: records.take().unwrap_or_default() }
        }
        "restore" => Command::Restore { instance_id: instance_id()?, previous: previous.take() },
        other => return Err(format!("unknown command {other}")),
    };
    if let Some(extra) = positional.next() {
        return Err(format!("unexpected argument {extra}"));
    }
    if previous.is_some() {
        return Err("--previous is for restore only".into());
    }
    if records.is_some() {
        return Err("--records is for restore without an instance ID or --previous only".into());
    }
    Ok(Args { command, out })
}

fn run(command: &Command) -> (u8, Value) {
    match command {
        Command::List { all } => list(*all),
        Command::State { instance_id } => match driver::driver_state(instance_id) {
            Ok(state) => (exit::OK, json!({ "ok": true, "command": "state", "state": StateJson::from(&state) })),
            Err(e) => driver_error("state", &e),
        },
        Command::Assign { instance_id } => assign(instance_id),
        Command::Restore { instance_id, previous } => restore(instance_id, previous.as_deref()),
        Command::RestoreRecords { dir } => restore_records(dir),
    }
}

fn list(all: bool) -> (u8, Value) {
    match usb_adapters::enumerate(true) {
        Ok(devices) => {
            let adapters: Vec<AdapterJson> =
                devices.iter().filter(|d| all || d.looks_like_bluetooth()).map(AdapterJson::from).collect();
            (
                exit::OK,
                json!({
                    "ok": true,
                    "command": "list",
                    "elevated": win_sys::elevation::is_elevated(),
                    "adapters": adapters,
                }),
            )
        }
        Err(e) => (exit::FAILED, error_json("enumeration", &e.to_string())),
    }
}

fn assign(instance_id: &str) -> (u8, Value) {
    match driver::assign_winusb(instance_id) {
        Ok(outcome) => {
            let on_winusb = outcome.state.kind == driver::DriverKind::WinUsb;
            let code = if on_winusb { exit::OK } else { exit::UNVERIFIED };
            (
                code,
                json!({
                    "ok": on_winusb,
                    "command": "assign",
                    "instance_id": instance_id,
                    "previous": RecordJson::from(&outcome.previous),
                    "already_winusb": outcome.already_winusb,
                    "class_changed": outcome.class_changed,
                    "reboot_required": outcome.reboot_required,
                    "state": StateJson::from(&outcome.state),
                }),
            )
        }
        Err(e) => driver_error("assign", &e),
    }
}

fn restore(instance_id: &str, previous: Option<&str>) -> (u8, Value) {
    let previous = match previous.map(parse_previous).transpose() {
        Ok(p) => p,
        Err(message) => return (exit::USAGE, error_json("usage", &message)),
    };
    match driver::restore(instance_id, previous.as_ref()) {
        Ok(outcome) => {
            let verified = outcome.state.as_ref().is_some_and(|s| s.present && s.kind != driver::DriverKind::WinUsb)
                && outcome.matches_previous != Some(false);
            let code = if verified { exit::OK } else { exit::UNVERIFIED };
            (
                code,
                json!({
                    "ok": verified,
                    "command": "restore",
                    "instance_id": instance_id,
                    "already_restored": outcome.already_restored,
                    "reboot_required": outcome.reboot_required,
                    "matches_previous": outcome.matches_previous,
                    "state": outcome.state.as_ref().map(StateJson::from),
                }),
            )
        }
        Err(e) => driver_error("restore", &e),
    }
}

/// What to do with one file of `restore --records`
#[derive(Debug, PartialEq)]
enum RecordPlan {
    Restore {
        instance_id: String,
    },
    /// Nothing to restore; `remove`: the record is of no use any more
    Skip {
        reason: &'static str,
        remove: bool,
    },
}

fn plan_record(text: &str) -> RecordPlan {
    let Ok(value) = serde_json::from_str::<Value>(text.trim_start_matches('\u{FEFF}')) else {
        return RecordPlan::Skip { reason: "not_a_record", remove: false };
    };
    let Some(instance_id) = value.get("instance_id").and_then(Value::as_str).filter(|s| !s.is_empty()) else {
        return RecordPlan::Skip { reason: "not_a_record", remove: false };
    };
    // The adapter was on WinUSB (Zadig's, or another tool's) before BlueCaster
    // was asked to assign it: nothing of BlueCaster's to undo
    if value.get("already_winusb").and_then(Value::as_bool) == Some(true) {
        return RecordPlan::Skip { reason: "winusb_before", remove: true };
    }
    RecordPlan::Restore { instance_id: instance_id.to_string() }
}

/// The more telling exit code of two adapters
fn worse(a: u8, b: u8) -> u8 {
    let rank = |c: u8| match c {
        exit::OK => 0,
        exit::NO_DEVICE => 1,
        exit::UNVERIFIED => 2,
        exit::FAILED => 3,
        _ => 4,
    };
    if rank(b) > rank(a) { b } else { a }
}

/// `restore --records <dir>`: every recorded adapter; a record is deleted
/// once its adapter is back on its own driver. An unplugged adapter keeps
/// its record (exit code 4 for the whole run).
fn restore_records(dir: &std::path::Path) -> (u8, Value) {
    let mut paths: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("json")))
            .collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return (exit::FAILED, error_json("records", &format!("cannot read {}: {e}", dir.display()))),
    };
    paths.sort();
    let plans: Vec<(PathBuf, RecordPlan)> = paths
        .into_iter()
        .map(|p| {
            let plan = match std::fs::read_to_string(&p) {
                Ok(text) => plan_record(&text),
                Err(_) => RecordPlan::Skip { reason: "unreadable", remove: false },
            };
            (p, plan)
        })
        .collect();
    if plans.iter().any(|(_, plan)| matches!(plan, RecordPlan::Restore { .. })) && !win_sys::elevation::is_elevated() {
        return driver_error("restore", &DriverError::NotElevated);
    }
    let mut code = exit::OK;
    let mut adapters = Vec::new();
    for (path, plan) in plans {
        let (one, mut result, remove) = match plan {
            RecordPlan::Restore { instance_id } => {
                let (one, result) = restore(&instance_id, Some(&format!("@{}", path.display())));
                (one, result, one == exit::OK)
            }
            RecordPlan::Skip { reason, remove } => (exit::OK, json!({ "ok": true, "skipped": reason }), remove),
        };
        if remove && let Err(e) = std::fs::remove_file(&path) {
            tracing::warn!("cannot delete the record {}: {e}", path.display());
        }
        result["record"] = json!(path.display().to_string());
        result["exit_code"] = json!(one);
        code = worse(code, one);
        adapters.push(result);
    }
    (
        code,
        json!({
            "ok": code == exit::OK,
            "command": "restore",
            "records": dir.display().to_string(),
            "adapters": adapters,
        }),
    )
}

/// The record from `--previous`: JSON text or @file, either the record
/// itself or a whole `assign` result holding it as "previous"
fn parse_previous(arg: &str) -> Result<DriverRecord, String> {
    let text = match arg.strip_prefix('@') {
        Some(path) => std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?,
        None => arg.to_string(),
    };
    let value: Value = serde_json::from_str(text.trim_start_matches('\u{FEFF}'))
        .map_err(|e| format!("--previous is not JSON: {e}"))?;
    let record = match value.get("previous") {
        Some(inner) if inner.is_object() => inner.clone(),
        _ => value,
    };
    let record: RecordJson =
        serde_json::from_value(record).map_err(|e| format!("--previous is not a driver record: {e}"))?;
    Ok(record.into())
}

fn driver_error(command: &str, e: &DriverError) -> (u8, Value) {
    let (code, kind) = match e {
        DriverError::NotElevated => (exit::NOT_ELEVATED, "not_elevated"),
        DriverError::DeviceNotFound(_) => (exit::NO_DEVICE, "device_not_found"),
        DriverError::NotUsb(_) => (exit::NO_DEVICE, "not_usb"),
        DriverError::WinUsbInfMissing(_) => (exit::FAILED, "winusb_inf_missing"),
        DriverError::WinUsbModelNotFound { .. } => (exit::FAILED, "winusb_model_not_found"),
        DriverError::Win32 { .. } => (exit::FAILED, "win32"),
    };
    let rollback = match e {
        DriverError::WinUsbModelNotFound { rollback, .. } | DriverError::Win32 { rollback, .. } => {
            Some(format!("{rollback:?}").to_ascii_lowercase())
        }
        _ => None,
    };
    tracing::error!("{command}: {e}");
    let mut v = error_json(kind, &e.to_string());
    v["command"] = json!(command);
    if let Some(r) = rollback {
        v["error"]["rollback"] = json!(r);
    }
    (code, v)
}

fn error_json(kind: &str, message: &str) -> Value {
    json!({ "ok": false, "error": { "kind": kind, "message": message } })
}

/// The driver record as JSON (passed back to `restore --previous`)
#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
struct RecordJson {
    inf_name: String,
    version: String,
    service: String,
    provider: String,
    description: String,
    class_guid: String,
}

impl From<&DriverRecord> for RecordJson {
    fn from(r: &DriverRecord) -> Self {
        RecordJson {
            inf_name: r.inf_name.clone(),
            version: r.version.clone(),
            service: r.service.clone(),
            provider: r.provider.clone(),
            description: r.description.clone(),
            class_guid: r.class_guid.clone(),
        }
    }
}

impl From<RecordJson> for DriverRecord {
    fn from(r: RecordJson) -> Self {
        DriverRecord {
            inf_name: r.inf_name,
            version: r.version,
            service: r.service,
            provider: r.provider,
            description: r.description,
            class_guid: r.class_guid,
        }
    }
}

#[derive(Serialize)]
struct DriverJson {
    service: String,
    inf_name: String,
    inf_section: String,
    version: String,
    provider: String,
    description: String,
    class_name: String,
    class_guid: String,
}

impl From<&DriverInfo> for DriverJson {
    fn from(d: &DriverInfo) -> Self {
        DriverJson {
            service: d.service.clone(),
            inf_name: d.inf_name.clone(),
            inf_section: d.inf_section.clone(),
            version: d.version.clone(),
            provider: d.provider.clone(),
            description: d.description.clone(),
            class_name: d.class_name.clone(),
            class_guid: d.class_guid.clone(),
        }
    }
}

#[derive(Serialize)]
struct StateJson {
    instance_id: String,
    present: bool,
    kind: &'static str,
    driver: DriverJson,
    problem_code: Option<u32>,
    interface_guids: Vec<String>,
    bluecaster_interface: bool,
}

impl From<&DriverState> for StateJson {
    fn from(s: &DriverState) -> Self {
        StateJson {
            instance_id: s.instance_id.clone(),
            present: s.present,
            kind: s.kind.as_str(),
            driver: DriverJson::from(&s.driver),
            problem_code: s.problem_code,
            interface_guids: s.interface_guids.clone(),
            bluecaster_interface: s.has_bluecaster_interface(),
        }
    }
}

#[derive(Serialize)]
struct ProbeJson {
    ok: bool,
    /// Interface 0 class, "E0/01/01"
    class: Option<String>,
    hci_endpoints: Option<bool>,
    /// Why it could not be read ("open failed (error 5)": in use)
    error: Option<String>,
}

#[derive(Serialize)]
struct AdapterJson {
    instance_id: String,
    /// "0BDA:8771"
    id: String,
    vid: u16,
    pid: u16,
    name: String,
    bus_reported_name: Option<String>,
    manufacturer: String,
    /// From the compatible IDs, "E0/01/01"
    usb_class: Option<String>,
    kind: &'static str,
    driver: DriverJson,
    problem_code: Option<u32>,
    /// WinUSB devices: interface 0 read through WinUSB
    probe: Option<ProbeJson>,
    /// "standard" (E0/01/01) or "vendor" (FF/01/01 with HCI endpoints),
    /// from the WinUSB probe
    bluetooth_interface: Option<&'static str>,
    looks_like_bluetooth: bool,
}

impl From<&UsbDevice> for AdapterJson {
    fn from(d: &UsbDevice) -> Self {
        AdapterJson {
            instance_id: d.instance_id.clone(),
            id: format!("{:04X}:{:04X}", d.vid, d.pid),
            vid: d.vid,
            pid: d.pid,
            name: d.name.clone(),
            bus_reported_name: d.bus_reported_name.clone(),
            manufacturer: d.manufacturer.clone(),
            usb_class: d.usb_class.map(|c| c.to_string()),
            kind: d.driver_kind.as_str(),
            driver: DriverJson::from(&d.driver),
            problem_code: d.problem_code,
            probe: d.probe.as_ref().map(|p| match p {
                Ok(p) => ProbeJson {
                    ok: true,
                    class: Some(p.class.to_string()),
                    hci_endpoints: Some(p.hci_endpoints),
                    error: None,
                },
                Err(e) => ProbeJson { ok: false, class: None, hci_endpoints: None, error: Some(e.clone()) },
            }),
            bluetooth_interface: d.winusb_bluetooth().map(|vendor| if vendor { "vendor" } else { "standard" }),
            looks_like_bluetooth: d.looks_like_bluetooth(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &[&str]) -> Result<Args, String> {
        parse_args(&s.iter().map(|a| a.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn log_options_come_out_first() {
        let argv: Vec<String> = ["assign", "--log", r"C:\l\bluecaster-driver.log", "X", "--verbose", "--out", "o.json"]
            .map(String::from)
            .into();
        let (log, verbose, rest) = log_args(argv);
        assert_eq!(log, Some(PathBuf::from(r"C:\l\bluecaster-driver.log")));
        assert!(verbose);
        assert_eq!(rest, ["assign", "X", "--out", "o.json"]);
        let (log, verbose, _) = log_args(vec!["list".into()]);
        assert_eq!((log, verbose), (None, false));
    }

    #[test]
    fn command_line() {
        assert_eq!(args(&["list"]).unwrap(), Args { command: Command::List { all: false }, out: None });
        assert_eq!(
            args(&["list", "--all", "--out", r"C:\t\r.json"]).unwrap(),
            Args { command: Command::List { all: true }, out: Some(PathBuf::from(r"C:\t\r.json")) }
        );
        assert_eq!(
            args(&["assign", r"USB\VID_0BDA&PID_8771\1"]).unwrap().command,
            Command::Assign { instance_id: r"USB\VID_0BDA&PID_8771\1".into() }
        );
        assert_eq!(
            args(&["restore", "X", "--previous", "{}"]).unwrap().command,
            Command::Restore { instance_id: "X".into(), previous: Some("{}".into()) }
        );
        assert_eq!(
            args(&["restore", "X"]).unwrap().command,
            Command::Restore { instance_id: "X".into(), previous: None }
        );
        assert!(args(&[]).is_err());
        assert!(args(&["assign"]).is_err());
        assert!(args(&["assign", "X", "Y"]).is_err());
        assert!(args(&["assign", "X", "--previous", "{}"]).is_err());
        assert!(args(&["list", "--out"]).is_err());
        assert!(args(&["list", "--force"]).is_err());
        assert!(args(&["format"]).is_err());
        assert_eq!(
            args(&["restore", "--records", r"C:\u\BlueCaster\driver", "--out", "o.json"]).unwrap(),
            Args {
                command: Command::RestoreRecords { dir: PathBuf::from(r"C:\u\BlueCaster\driver") },
                out: Some(PathBuf::from("o.json"))
            }
        );
        assert!(args(&["restore", "--records"]).is_err());
        assert!(args(&["restore", "X", "--records", "D"]).is_err());
        assert!(args(&["restore", "--records", "D", "--previous", "{}"]).is_err());
        assert!(args(&["list", "--records", "D"]).is_err());
    }

    #[test]
    fn record_plans() {
        let assigned = json!({
            "ok": true, "command": "assign", "instance_id": r"USB\VID_2357&PID_0604\E848B8C82000",
            "already_winusb": false, "previous": { "inf_name": "oem91.inf", "service": "BTHUSB" },
        });
        assert_eq!(
            plan_record(&format!("\u{FEFF}{assigned}")),
            RecordPlan::Restore { instance_id: r"USB\VID_2357&PID_0604\E848B8C82000".into() }
        );
        let mut before = assigned.clone();
        before["already_winusb"] = json!(true);
        assert_eq!(plan_record(&before.to_string()), RecordPlan::Skip { reason: "winusb_before", remove: true });
        assert_eq!(plan_record("null"), RecordPlan::Skip { reason: "not_a_record", remove: false });
        assert_eq!(plan_record("{}"), RecordPlan::Skip { reason: "not_a_record", remove: false });
        assert_eq!(plan_record("garbage"), RecordPlan::Skip { reason: "not_a_record", remove: false });
    }

    #[test]
    fn worst_exit_code() {
        assert_eq!(worse(exit::OK, exit::OK), exit::OK);
        assert_eq!(worse(exit::OK, exit::NO_DEVICE), exit::NO_DEVICE);
        assert_eq!(worse(exit::UNVERIFIED, exit::NO_DEVICE), exit::UNVERIFIED);
        assert_eq!(worse(exit::UNVERIFIED, exit::FAILED), exit::FAILED);
        assert_eq!(worse(exit::NOT_ELEVATED, exit::FAILED), exit::NOT_ELEVATED);
    }

    #[test]
    fn records_folder_without_work() {
        // A missing folder, and records with nothing to restore: no elevation
        // needed, the useless record goes, the unreadable one stays
        let (code, v) = restore_records(std::path::Path::new(r"Z:\no\such\folder"));
        assert_eq!(code, exit::OK);
        assert_eq!(v["adapters"].as_array().unwrap().len(), 0);
        let dir = std::env::temp_dir().join(format!("bluecaster_driver_records_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.json"), r#"{"instance_id":"USB\\X","already_winusb":true}"#).unwrap();
        std::fs::write(dir.join("b.json"), "not json").unwrap();
        std::fs::write(dir.join("c.txt"), "ignored").unwrap();
        let (code, v) = restore_records(&dir);
        assert_eq!(code, exit::OK);
        assert_eq!(v["adapters"].as_array().unwrap().len(), 2);
        assert!(!dir.join("a.json").exists());
        assert!(dir.join("b.json").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn previous_record_roundtrip() {
        let record = DriverRecord {
            inf_name: "oem28.inf".into(),
            version: "24.40.11.1".into(),
            service: "BTHUSB".into(),
            provider: "Intel Corporation".into(),
            description: "インテル(R) ワイヤレス Bluetooth(R)".into(),
            class_guid: "{E0CBF06C-CD8B-4647-BB8A-263B43F0F974}".into(),
        };
        let text = serde_json::to_string(&RecordJson::from(&record)).unwrap();
        assert_eq!(parse_previous(&text).unwrap(), record);
        // The whole assign result is accepted too
        let assign_result = json!({ "ok": true, "command": "assign", "previous": RecordJson::from(&record) });
        assert_eq!(parse_previous(&assign_result.to_string()).unwrap(), record);
        // Missing fields are empty, unknown ones ignored
        let partial = parse_previous(r#"{"inf_name":"bth.inf","service":"BTHUSB","extra":1}"#).unwrap();
        assert_eq!(partial.inf_name, "bth.inf");
        assert_eq!(partial.version, "");
        assert!(parse_previous("not json").is_err());
        assert!(parse_previous(r#"{"inf_name": 5}"#).is_err());
    }

    #[test]
    fn previous_from_file() {
        let path = std::env::temp_dir().join(format!("bluecaster_driver_prev_{}.json", std::process::id()));
        // With a BOM, as PowerShell's Out-File writes it
        std::fs::write(&path, "\u{FEFF}{\"inf_name\":\"bth.inf\",\"version\":\"10.0.1\",\"service\":\"BTHUSB\"}")
            .unwrap();
        let record = parse_previous(&format!("@{}", path.display())).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(record.service, "BTHUSB");
        assert!(parse_previous("@Z:\\missing\\file.json").is_err());
    }

    #[test]
    fn errors_map_to_exit_codes() {
        assert_eq!(driver_error("assign", &DriverError::NotElevated).0, exit::NOT_ELEVATED);
        assert_eq!(driver_error("assign", &DriverError::DeviceNotFound("X".into())).0, exit::NO_DEVICE);
        let (code, v) = driver_error(
            "assign",
            &DriverError::WinUsbModelNotFound { class: "{X}".into(), rollback: driver::Rollback::Restored },
        );
        assert_eq!(code, exit::FAILED);
        assert_eq!(v["error"]["kind"], "winusb_model_not_found");
        assert_eq!(v["error"]["rollback"], "restored");
        assert_eq!(v["ok"], false);
    }
}
