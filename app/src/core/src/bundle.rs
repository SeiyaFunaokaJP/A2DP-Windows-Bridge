//! The diagnostic bundle (diag.bundle): one .zip with everything a report
//! about a specific PC and headphones needs, so that nobody has to collect
//! files by hand.
//!
//! - `README.txt`: what is in it
//! - `diagnostics.txt`: "copy diagnostic info"
//! - `system.txt`: Windows, every Bluetooth-looking USB device with its
//!   driver, Windows' paired devices, the audio endpoints, the firmware and
//!   settings folders' files
//! - `logs/`: the core's, the UI's and the driver helper's log files (the
//!   last [`MAX_LOG_BYTES`] of each)
//! - `config/`: settings, profiles, the known devices and driver records;
//!   never the link keys or LE bonds
//! - `crash/`: crash reports (win_sys::crash)
//! - `hci/`: the flight recorder now and its saved files (unless left out:
//!   they hold Bluetooth addresses; the recorder zeroes pairing keys).
//!   Captures started by hand keep the keys and stay out.
//!
//! With `mask`, Bluetooth addresses (`<addr-N>`, the adapters' own
//! `<adapter>`), USB serial numbers, device names (`<device-N>`) and the
//! Windows user name in the text files (and the driver records' names) are
//! masked as in the diagnostics view ([`Masker`]), one masker for the whole
//! bundle so that the numbers agree between its files; the device names of
//! the profiles and the saved devices are masked wherever they appear, in
//! the config files too. Bundles go to `logs\bundles\`, the newest
//! [`KEEP_BUNDLES`] kept.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use bluecaster_core::diag::{Masker, quoted};
use bluecaster_core::paths;
use ipc::DiagBundle;

use crate::state::CoreState;
use crate::zip::ZipWriter;

// Size: GitHub takes attachments up to 25 MB. Text compresses about 10:1,
// HCI captures and dumps much less, hence these limits (newest first).
/// The end of each log file that goes in (a verbose day can be large)...
const MAX_LOG_BYTES: usize = 16 * 1024 * 1024;
/// ...and of all log files together
const LOG_BUDGET: usize = 48 * 1024 * 1024;
/// HCI flight recordings and crash minidumps that go in
const MAX_RECORDINGS: usize = 5;
const MAX_DUMPS: usize = 2;
const KEEP_BUNDLES: usize = 3;
/// Settings folder files that go in (text, with addresses)
const CONFIG_FILES: &[&str] =
    &["settings.json", "profiles.json", "adapter_caps.json", "devices.txt", "le_audio_devices.txt"];

/// Builds the bundle and writes it -> its path, size and file count
pub fn create(state: &CoreState, mask: bool, hci: bool) -> Result<DiagBundle, String> {
    let config = state.config_dir();
    let logs = paths::log_dir(&config);
    let stamp = paths::local_stamp();
    let t = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    let mut zip = ZipWriter::new((t.year() as u16, u8::from(t.month()), t.day(), t.hour(), t.minute(), t.second()));
    // One masker for all the files: a device name is the same <device-N>
    // in each of them
    let mut masker = Masker::new();
    for p in bluecaster_core::profiles::load(&config) {
        for n in [&p.device_name, &p.audio_device_name, &p.volume_device_name, &p.name] {
            masker.know(n);
        }
    }
    for d in bluecaster_core::devices::load(&config).into_iter().chain(bluecaster_core::devices::load_le(&config)) {
        masker.know(&d.name);
    }
    for a in crate::adapter_addresses(&state.snapshot().1.adapter.adapters) {
        masker.know_adapter(&a);
    }
    let mut text = |s: String| if mask { masker.mask(&s) } else { s };
    let mut skipped: Vec<String> = Vec::new();

    zip.add("diagnostics.txt", text(crate::diag_text(state, false)).as_bytes());
    zip.add("system.txt", text(system_text(&config)).as_bytes());

    // Logs: core, UI, driver helper; the newest first while the budget lasts
    let mut budget = LOG_BUDGET;
    for (name, path) in newest_first(files_in(&logs, |n| n.starts_with("bluecaster-") && n.contains(".log"))) {
        if budget == 0 {
            skipped.push(format!("{name}: over the size limit"));
            continue;
        }
        match std::fs::read(&path) {
            Ok(data) => {
                let keep = data.len().min(MAX_LOG_BYTES).min(budget);
                budget -= keep;
                if keep < data.len() {
                    skipped.push(format!("{name}: only its last {} MB", keep / (1024 * 1024)));
                }
                let tail = &data[data.len() - keep..];
                zip.add(&format!("logs/{name}"), text(String::from_utf8_lossy(tail).into_owned()).as_bytes());
            }
            Err(e) => skipped.push(format!("{name}: {e}")),
        }
    }
    for name in CONFIG_FILES {
        if let Ok(s) = std::fs::read_to_string(config.join(name)) {
            zip.add(&format!("config/{name}"), text(s).as_bytes());
        }
    }
    for (name, path) in files_in(&config.join("driver"), |n| n.ends_with(".json")) {
        if let Ok(s) = std::fs::read_to_string(path) {
            // Named after the adapter's instance ID
            zip.add(&text(format!("config/driver/{name}")), text(s).as_bytes());
        }
    }
    let mut dumps = 0;
    for (name, path) in newest_first(files_in(&paths::crash_dir(&config), |_| true)) {
        if name.ends_with(".dmp") {
            if dumps == MAX_DUMPS {
                skipped.push(format!("crash/{name}: older minidump"));
                continue;
            }
            dumps += 1;
        }
        if let Ok(data) = std::fs::read(path) {
            if name.ends_with(".txt") {
                // The report names its dump's path
                zip.add(&format!("crash/{name}"), text(String::from_utf8_lossy(&data).into_owned()).as_bytes());
            } else {
                zip.add(&format!("crash/{name}"), &data);
            }
        }
    }
    if hci {
        let capture = state.service.capture();
        let now = logs.join(format!("recorder-{stamp}.tmp"));
        if capture.write_recent(&now, "diagnostic bundle").is_ok()
            && let Ok(data) = std::fs::read(&now)
        {
            zip.add(&format!("hci/recorder-{stamp}.pklg"), &data);
        }
        let _ = std::fs::remove_file(&now);
        let recordings = newest_first(files_in(&paths::recorder_dir(&config), |n| n.ends_with(".pklg")));
        for (name, _) in recordings.iter().skip(MAX_RECORDINGS) {
            skipped.push(format!("hci/{name}: older recording"));
        }
        for (name, path) in recordings.into_iter().take(MAX_RECORDINGS) {
            if let Ok(data) = std::fs::read(path) {
                zip.add(&format!("hci/{name}"), &data);
            }
        }
    }
    zip.add("README.txt", readme(&stamp, mask, hci, &skipped).as_bytes());

    let files = zip.len() as u32;
    let data = zip.finish();
    let dir = logs.join("bundles");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let path = dir.join(format!("bluecaster-diag-{stamp}.zip"));
    std::fs::write(&path, &data).map_err(|e| format!("{}: {e}", path.display()))?;
    prune(&dir);
    tracing::info!("diagnostic bundle: {files} files, {} bytes -> {}", data.len(), path.display());
    Ok(DiagBundle { path: path.display().to_string(), bytes: data.len() as u64, files })
}

/// The files of `dir` whose names pass `keep`, by name
fn files_in(dir: &Path, keep: impl Fn(&str) -> bool) -> Vec<(String, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut v: Vec<(String, PathBuf)> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .filter_map(|e| e.file_name().to_str().map(|n| (n.to_owned(), e.path())))
        .filter(|(n, _)| keep(n))
        .collect();
    v.sort();
    v
}

/// By modification time, the newest first
fn newest_first(mut files: Vec<(String, PathBuf)>) -> Vec<(String, PathBuf)> {
    files.sort_by_key(|(_, p)| std::cmp::Reverse(std::fs::metadata(p).and_then(|m| m.modified()).ok()));
    files
}

fn prune(dir: &Path) {
    let all = files_in(dir, |n| n.starts_with("bluecaster-diag-") && n.ends_with(".zip"));
    for (_, p) in &all[..all.len().saturating_sub(KEEP_BUNDLES)] {
        let _ = std::fs::remove_file(p);
    }
}

fn readme(stamp: &str, mask: bool, hci: bool, skipped: &[String]) -> String {
    let mut s = format!(
        "BlueCaster {} diagnostic bundle, {stamp}\n\n\
         diagnostics.txt  the diagnostic info (as copied from Diagnostics)\n\
         system.txt       Windows, USB Bluetooth adapters and drivers, paired devices, audio endpoints\n\
         logs/            log files: bluecaster-core (the service), bluecaster-ui (the window), bluecaster-driver (driver changes)\n\
         config/          settings, profiles, known devices (no link keys; addresses and names masked as everywhere)\n\
         crash/           crash reports: .txt, and .dmp minidumps for Visual Studio / WinDbg (raw memory: not masked)\n\
         hci/             HCI captures (.pklg) for Wireshark: recorder-* is the flight recorder at the time of\n\
                          this bundle, hci-* were saved when a stream failed. Media payloads are cut short\n\
                          and pairing keys zeroed (link keys, LTK, IRK, CSRK, DH key); captures started by\n\
                          hand are not included (they keep them)\n\n",
        env!("CARGO_PKG_VERSION")
    );
    let _ = writeln!(
        s,
        "Identifiers: {}",
        if mask {
            "masked in the text files: Bluetooth addresses (<addr-N>, the adapter's own <adapter>), USB serial \
             numbers, device names (<device-N>), paths above the app's own folder (<path>) and account SIDs \
             (<sid>); the same N for the same address or name in every file"
        } else {
            "not masked"
        }
    );
    if !hci {
        s.push_str("HCI captures: left out\n");
    } else if mask {
        s.push_str("HCI captures: they contain the full Bluetooth addresses and device names (not the pairing keys)\n");
    }
    for k in skipped {
        let _ = writeln!(s, "left out: {k}");
    }
    s
}

/// What the PC has: the part of a report the diagnostic info leaves out
fn system_text(config: &Path) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "BlueCaster {} ({}), Windows build {}, pid {}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::ARCH,
        audio_capture::windows_build(),
        std::process::id()
    );
    let _ = writeln!(s, "executable: {}", std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default());
    let _ = writeln!(s, "settings folder: {}", config.display());
    match crate::logging::verbose_now() {
        Some(l) => {
            let _ = writeln!(s, "verbose logging: {l:?}");
        }
        None => s.push_str("verbose logging: off\n"),
    }

    s.push_str("\n== USB devices that look like Bluetooth adapters ==\n");
    match win_sys::usb_adapters::enumerate(false) {
        Ok(devices) => {
            for d in devices.iter().filter(|d| d.looks_like_bluetooth()) {
                let _ = writeln!(
                    s,
                    "{} [{:04X}:{:04X}] {}\n  manufacturer {:?}, product string {:?}\n  driver {:?} {:?}, problem code {:?}\n  hardware IDs {:?}\n  compatible IDs {:?}",
                    d.display_name(),
                    d.vid,
                    d.pid,
                    d.instance_id,
                    d.manufacturer,
                    d.bus_reported_name,
                    d.driver_kind,
                    d.driver,
                    d.problem_code,
                    d.hardware_ids,
                    d.compatible_ids
                );
            }
        }
        Err(e) => {
            let _ = writeln!(s, "cannot list them: {e}");
        }
    }

    s.push_str("\n== Windows' paired Bluetooth devices ==\n");
    match win_sys::bt_paired::paired_devices() {
        Ok(devices) if devices.is_empty() => s.push_str("none (or Windows' Bluetooth is off)\n"),
        Ok(devices) => {
            for d in devices {
                let _ = writeln!(
                    s,
                    "{} {}: class 0x{:06X}, connected {}, remembered {}, authenticated {}",
                    d.address_string(),
                    quoted(&d.name),
                    d.class_of_device,
                    d.connected,
                    d.remembered,
                    d.authenticated
                );
            }
        }
        Err(e) => {
            let _ = writeln!(s, "cannot list them: {e}");
        }
    }

    s.push_str("\n== Audio output devices ==\n");
    match audio_capture::render_devices() {
        Ok(devices) => {
            for d in devices {
                let mix = audio_capture::mix_format(Some(&d.id));
                let _ = writeln!(
                    s,
                    "{}{}{}: {}",
                    quoted(&d.name),
                    if d.is_default { " (default)" } else { "" },
                    if d.is_virtual { " (virtual)" } else { "" },
                    mix.map_or("mix format unknown".into(), |m| format!(
                        "{} Hz, {} channel(s), {} bit",
                        m.rate, m.channels, m.bits
                    ))
                );
            }
        }
        Err(e) => {
            let _ = writeln!(s, "cannot list them: {e}");
        }
    }

    for (title, dir) in [("Firmware folder", paths::firmware_dir(config)), ("Settings folder", config.to_path_buf())] {
        let _ = writeln!(s, "\n== {title} ==");
        for (name, path) in files_in(&dir, |_| true) {
            let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let _ = writeln!(s, "{name}  {size} bytes");
        }
    }
    s
}
