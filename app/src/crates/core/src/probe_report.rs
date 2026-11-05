//! Chip probe reports (bt_host::probe) as JSON files in `logs\probe\`, for
//! whoever adds a loader for the chip (docs/chip-support.md). They hold the
//! adapter's address and name, so the diagnostic bundle takes them only
//! with the HCI captures.

use std::path::{Path, PathBuf};

use bt_host::probe::{ProbeReport, ProbeSummary, StepKind};
use serde_json::{Value, json};

use crate::adapter::bt_core_version;

/// Reports kept in their folder (newest)
const KEEP: usize = 10;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// "Intel, USB 8087:0033, Bluetooth 5.3 (HCI 12), LMP subversion 0x3102,
/// 10 of 10 requests answered"
pub fn summary_line(report: &ProbeReport) -> String {
    let s = report.summary();
    let mut parts = Vec::new();
    match (s.company_name, s.company) {
        (Some(name), _) => parts.push(name.to_owned()),
        (None, Some(id)) => parts.push(format!("company 0x{id:04X}")),
        (None, None) => {}
    }
    if let Some((vid, pid)) = s.usb_ids {
        parts.push(format!("USB {vid:04X}:{pid:04X}"));
    }
    if let Some(v) = s.hci_version {
        parts.push(match bt_core_version(v) {
            Some(core) => format!("Bluetooth {core} (HCI {v})"),
            None => format!("HCI {v}"),
        });
    }
    if let Some(sub) = s.lmp_subversion {
        parts.push(format!("LMP subversion 0x{sub:04X}"));
    }
    parts.push(format!("{} of {} requests answered", s.answered, s.steps));
    if let Some(e) = &report.error {
        parts.push(e.clone());
    }
    parts.join(", ")
}

fn summary_json(s: &ProbeSummary) -> Value {
    json!({
        "hci_version": s.hci_version,
        "bluetooth": s.hci_version.and_then(bt_core_version),
        "hci_revision": s.hci_revision.map(|v| format!("0x{v:04X}")),
        "lmp_version": s.lmp_version,
        "lmp_subversion": s.lmp_subversion.map(|v| format!("0x{v:04X}")),
        "company": s.company.map(|v| format!("0x{v:04X}")),
        "company_name": s.company_name,
        "address": s.address,
        "name": s.name,
        "acl_buffers": s.acl_buffers.map(|(len, n)| json!({ "length": len, "count": n })),
        "le_acl_buffers": s.le_acl_buffers.map(|(len, n)| json!({ "length": len, "count": n })),
        "usb_ids": s.usb_ids.map(|(vid, pid)| format!("{vid:04X}:{pid:04X}")),
        "usb_manufacturer": s.usb_manufacturer,
        "usb_product": s.usb_product,
        "answered": s.answered,
        "steps": s.steps,
    })
}

/// The report with its decoded summary; packets in hex as sent / received
/// (HCI command: opcode, length, parameters; event: code, length,
/// parameters; USB: the SETUP packet and the bytes read)
pub fn to_json(report: &ProbeReport, adapter: Option<&str>, version: &str) -> Value {
    json!({
        "bluecaster": version,
        "adapter": adapter,
        "summary": summary_json(&report.summary()),
        "error": report.error,
        "millis": report.millis,
        "steps": report.steps.iter().map(|s| json!({
            "name": s.name,
            "kind": match s.kind { StepKind::Hci => "hci", StepKind::Usb => "usb", StepKind::Other => "other" },
            "command": hex(&s.command),
            "answer": s.answer.as_deref().map(hex),
            "error": s.error,
            "millis": s.millis,
        })).collect::<Vec<_>>(),
        "other_events": report.other_events.iter().map(|e| hex(e)).collect::<Vec<_>>(),
        "usb": report.usb.as_ref().map(|u| json!({
            "interfaces": u.summary,
            "device_descriptor": hex(&u.device),
            "configuration_descriptors": u.configurations.iter().map(|c| hex(c)).collect::<Vec<_>>(),
        })),
    })
}

/// Writes `probe-<time>.json` to `logs\probe\` and keeps the newest
/// [`KEEP`]
pub fn save(config: &Path, report: &ProbeReport, adapter: Option<&str>, version: &str) -> std::io::Result<PathBuf> {
    write(config, "probe", to_json(report, adapter, version))
}

/// Writes a chip script's report, with the script, as `script-<time>.json`
/// beside the probe reports (the newest [`KEEP`] of them)
pub fn save_script(
    config: &Path,
    report: &ProbeReport,
    script: &str,
    adapter: Option<&str>,
    version: &str,
) -> std::io::Result<PathBuf> {
    let mut v = to_json(report, adapter, version);
    v["script"] = Value::String(script.to_owned());
    write(config, "script", v)
}

fn write(config: &Path, prefix: &str, v: Value) -> std::io::Result<PathBuf> {
    let dir = crate::paths::probe_dir(config);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{prefix}-{}.json", crate::paths::local_stamp()));
    std::fs::write(&path, serde_json::to_string_pretty(&v).unwrap_or_default())?;
    prune(&dir, prefix);
    Ok(path)
}

fn prune(dir: &Path, prefix: &str) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(&format!("{prefix}-"))))
        .collect();
    // <prefix>-YYYYMMDD_HHMMSS: the name sorts by time
    files.sort();
    let excess = files.len().saturating_sub(KEEP);
    for old in &files[..excess] {
        let _ = std::fs::remove_file(old);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bt_host::bringup::UsbDescriptors;
    use bt_host::probe::ProbeStep;

    fn report() -> ProbeReport {
        ProbeReport {
            steps: vec![
                ProbeStep {
                    name: "Read Local Version Information".into(),
                    kind: StepKind::Hci,
                    command: vec![0x01, 0x10, 0x00],
                    answer: Some(vec![0x0E, 0x0C, 0x01, 0x01, 0x10, 0x00, 12, 0x02, 0x31, 12, 0x02, 0x00, 0x02, 0x31]),
                    error: None,
                    millis: 3,
                },
                ProbeStep {
                    name: "Read BD_ADDR".into(),
                    kind: StepKind::Hci,
                    command: vec![0x09, 0x10, 0x00],
                    answer: None,
                    error: None,
                    millis: 1000,
                },
                ProbeStep {
                    name: "USB device descriptor".into(),
                    kind: StepKind::Usb,
                    command: vec![0x80, 0x06, 0x00, 0x01, 0x00, 0x00, 18, 0],
                    answer: Some(vec![
                        18, 0x01, 0x00, 0x02, 0xE0, 0x01, 0x01, 64, 0x87, 0x80, 0x33, 0x00, 0x00, 0x00, 0, 0, 0, 1,
                    ]),
                    error: None,
                    millis: 1,
                },
            ],
            other_events: vec![vec![0xFF, 0x01, 0x06]],
            millis: 1004,
            error: None,
            usb: Some(UsbDescriptors {
                device: vec![18, 0x01],
                configurations: vec![vec![9, 0x02]],
                summary: vec!["interface 0 alt 0 class E0/01/01: 0x81 interrupt 64".into()],
            }),
        }
    }

    #[test]
    fn summary_names_the_chip_and_what_answered() {
        assert_eq!(
            summary_line(&report()),
            "Intel, USB 8087:0033, Bluetooth 5.3 (HCI 12), LMP subversion 0x3102, 2 of 3 requests answered"
        );
        let nothing = ProbeReport { error: Some("could not open the Bluetooth adapter".into()), ..report() };
        let nothing = ProbeReport { steps: Vec::new(), ..nothing };
        assert_eq!(summary_line(&nothing), "0 of 0 requests answered, could not open the Bluetooth adapter");
    }

    #[test]
    fn json_has_the_packets_in_hex() {
        let v = to_json(&report(), Some(r"USB\VID_8087&PID_0033\5&3B72E4CF&0&14"), "2.0.2");
        assert_eq!(v["summary"]["company_name"], "Intel");
        assert_eq!(v["summary"]["bluetooth"], "5.3");
        assert_eq!(v["steps"][0]["command"], "011000");
        assert_eq!(v["steps"][0]["answer"], "0e0c010110000c02310c02000231");
        assert_eq!(v["steps"][0]["kind"], "hci");
        assert_eq!(v["steps"][1]["answer"], Value::Null);
        assert_eq!(v["steps"][2]["kind"], "usb");
        assert_eq!(v["steps"][2]["command"], "8006000100001200");
        assert_eq!(v["summary"]["usb_ids"], "8087:0033");
        assert_eq!(v["other_events"][0], "ff0106");
        assert_eq!(v["usb"]["interfaces"][0], "interface 0 alt 0 class E0/01/01: 0x81 interrupt 64");
        assert_eq!(v["usb"]["configuration_descriptors"][0], "0902");
    }

    #[test]
    fn save_keeps_the_newest() {
        let dir = std::env::temp_dir().join(format!("bluecaster-probe-test-{}", std::process::id()));
        let probe = crate::paths::probe_dir(&dir);
        std::fs::create_dir_all(&probe).unwrap();
        for i in 0..KEEP + 2 {
            std::fs::write(probe.join(format!("probe-20260101_0000{i:02}.json")), "{}").unwrap();
        }
        let path = save(&dir, &report(), None, "2.0.2").unwrap();
        let left = std::fs::read_dir(&probe).unwrap().count();
        assert_eq!(left, KEEP);
        assert!(path.exists());
        assert!(!probe.join("probe-20260101_000000.json").exists());
        // Script reports are kept apart, with their script
        let script = save_script(&dir, &report(), "hci 1001\n", None, "2.0.2").unwrap();
        assert!(script.file_name().unwrap().to_string_lossy().starts_with("script-"));
        assert_eq!(std::fs::read_dir(&probe).unwrap().count(), KEEP + 1);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&script).unwrap()).unwrap();
        assert_eq!(v["script"], "hci 1001\n");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
