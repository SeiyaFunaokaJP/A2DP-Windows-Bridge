//! The resident core's state as the UI sees it (an ipc::Snapshot with a
//! revision), the events that change it, and the requests that act on it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use a2dpwb_core::diag::AudioRates;
use a2dpwb_core::paths;
use a2dpwb_core::profiles::{self, Profile as CoreProfile};
use a2dpwb_core::receiver::{self, ReceiverClient};
use a2dpwb_core::settings::Settings as CoreSettings;
use a2dpwb_core::{
    LeStreamDetails, ScannedDevice, Service, ServiceEvent, SourceRequest, StreamRequest,
    StreamState as CoreStreamState, VolumeSync, devices,
};
use bt_host::LinkStats;
use bt_host::chips::{self, ChipVendor};
use ipc::{
    AdapterInfo, AdapterState, AdapterStatus, AudioApp, AudioDevice, CaptureState, DevLog, DevOverrides, Device,
    DiagInfo, DriverKind, ErrorInfo, Event, FirmwareFile, LeStreamInfo, LinkTotals, Method, Msg, Profile,
    ReceiverConnection, ReceiverState, Settings, SettingsApplied, Snapshot, SourceInfo, StreamState, StreamStatus,
    UpdateState,
};
use serde_json::Value;
use tokio::sync::broadcast;

/// Work of a request that blocks (scan, driver change), run off the runtime
type BlockingJob = Box<dyn FnOnce(&Arc<CoreState>) -> Result<Value, ErrorInfo> + Send>;

/// A tray balloon to show (on the tray's thread)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BalloonRequest {
    pub title: String,
    pub text: String,
    /// Opened by a click on it
    pub url: Option<String>,
}

/// settings.json key (kept among the unknown ones): the "still running in
/// the tray" hint was shown once
const TRAY_HINT_SHOWN: &str = "tray_hint_shown";

/// The low-battery balloon is shown again only after the level was back
/// this much above the setting's level (charged)
const BATTERY_REARM_MARGIN: u8 = 5;

/// How long a restart for a save waits for a newer request to take over
/// ([`CoreState::restart_profile`])
const RESTART_COALESCE: std::time::Duration = std::time::Duration::from_millis(250);

/// How long the "connected" balloon waits for the battery: HFP is opened once
/// the stream is established, so the level often comes just after it
const BATTERY_WAIT: std::time::Duration = std::time::Duration::from_secs(4);

/// Whether a battery report shows the low-battery balloon: once when the
/// level is below `low` (the setting; also the first report of a
/// connection), again only after it was back [`BATTERY_REARM_MARGIN`]
/// above. Never when `low` is 0. `warned` is cleared for each connection.
fn battery_low_alert(warned: &mut bool, level: u8, low: u8) -> bool {
    if level >= low.saturating_add(BATTERY_REARM_MARGIN) {
        *warned = false;
        return false;
    }
    if level < low && !*warned {
        *warned = true;
        return true;
    }
    false
}

/// The streamed profile's device: its name, else its address
fn streamed_device(s: &Snapshot) -> Option<&str> {
    s.stream.profile.as_ref().and_then(|n| s.profiles.iter().find(|p| &p.name == n)).map(|p| device_name(&s.devices, p))
}

/// What users call a profile (issue #86: never its own name): its saved
/// device's name (renamed after the profile was made, too), else the name
/// the profile kept, else the address
fn device_name<'a>(devices: &'a [Device], p: &'a Profile) -> &'a str {
    devices
        .iter()
        .find(|d| d.saved && d.le_audio == p.le_audio && d.address.eq_ignore_ascii_case(&p.device_address))
        .map(|d| d.name.as_str())
        .filter(|n| !n.is_empty())
        .unwrap_or(if p.device_name.is_empty() { p.device_address.as_str() } else { p.device_name.as_str() })
}

/// What the tray shows
#[derive(Debug, Clone)]
pub struct TrayView {
    pub streaming: bool,
    pub tooltip: String,
    pub profiles: Vec<String>,
    /// The menu's item for each of `profiles`: its device, as the flyout
    /// lists it
    pub labels: Vec<String>,
    pub active_profile: Option<String>,
    pub language: String,
}

pub struct CoreState {
    snapshot: Mutex<(u64, Snapshot)>,
    events: broadcast::Sender<(u64, Arc<Event>)>,
    pub service: Arc<Service>,
    /// Clients subscribed to link samples
    pub link_subscribers: AtomicUsize,
    /// Clients subscribed to the receiver test's state
    pub receiver_subscribers: AtomicUsize,
    /// The peer receiver test's connection: kept while no window is open
    pub receiver: ReceiverClient,
    pub started: Instant,
    /// What the stream moved over the last second (the sampler), for the
    /// flow's "Audio sending" step
    rates: Mutex<Option<AudioRates>>,
    /// The profile the last stream.start asked for: the session stopped for
    /// it reports Idle after it, which clears the snapshot's profile
    starting_profile: Mutex<Option<String>>,
    /// The low-battery balloon was shown for this connection
    battery_warned: AtomicBool,
    /// A new connection started: its balloon is shown once it streams with
    /// the codec settled (not after a reconnect of the same connection)
    connect_announce: AtomicBool,
    /// Until when the balloon of this connection waits for the battery
    /// ([`BATTERY_WAIT`])
    battery_deadline: Mutex<Option<Instant>>,
    /// The session starting is the profile in use started again with what
    /// was saved ([`CoreState::restart_profile`]): no balloon for it
    quiet_connect: AtomicBool,
    /// Set by [`CoreState::restart_profile`] for the [`CoreState::begin_stream`]
    /// it calls
    restarting: AtomicBool,
    tray: Box<dyn Fn(TrayView) + Send + Sync>,
    quit: Box<dyn Fn() + Send + Sync>,
    balloon: Box<dyn Fn(BalloonRequest) + Send + Sync>,
}

pub fn msg(m: &a2dpwb_core::Msg) -> Msg {
    Msg { key: m.key.to_string(), args: m.args.clone() }
}

pub(crate) fn err(code: &str, key: &str) -> ErrorInfo {
    ErrorInfo { code: code.into(), message: Msg::new(key) }
}

/// A request whose parameters do not make sense (ipc.error.bad_request)
pub(crate) fn bad_request(code: &str, method: &str, why: String) -> ErrorInfo {
    ErrorInfo {
        code: code.into(),
        message: Msg { key: "ipc.error.bad_request".into(), args: vec![method.into(), why] },
    }
}

/// A result as JSON (plain data: it cannot fail)
fn json<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}

fn to_ipc_profile(p: &CoreProfile, legacy: u16) -> Profile {
    Profile {
        name: p.name.clone(),
        device_address: p.device_address.clone(),
        device_name: p.device_name.clone(),
        codec: p.codec.clone(),
        quality: p.quality.clone(),
        abr: p.abr,
        codec_fallback: p.codec_fallback,
        sample_rate: p.sample_rate,
        bit_depth: p.bit_depth,
        capture_mode: p.capture_mode.clone(),
        audio_device_id: p.audio_device_id.clone(),
        audio_device_name: p.audio_device_name.clone(),
        auto_switch_device: p.auto_switch_device,
        app_exe: p.app_exe.clone(),
        app_name: p.app_name.clone(),
        volume_sync: p.volume_sync.clone(),
        volume_device_id: p.volume_device_id.clone(),
        volume_device_name: p.volume_device_name.clone(),
        max_media_payload: p.max_media_payload.unwrap_or(legacy),
        le_audio: p.le_audio,
        le_low_latency: p.le_low_latency,
        sbc_bitpool: p.sbc_bitpool,
        sbcxq_bitpool: p.sbcxq_bitpool,
        sbc_min_bitpool: p.sbc_min_bitpool,
        sbcxq_min_bitpool: p.sbcxq_min_bitpool,
        sbc_blocks: p.sbc_blocks,
        sbc_subbands: p.sbc_subbands,
        sbc_allocation: p.sbc_allocation.clone(),
        sbc_channel_mode: p.sbc_channel_mode.clone(),
        codec_settings: p.codec_settings.iter().map(|(c, v)| (c.clone(), to_ipc_codec_values(v))).collect(),
        codec_order: p.codec_order.clone(),
    }
}

fn to_ipc_codec_values(v: &profiles::CodecValues) -> ipc::CodecValues {
    ipc::CodecValues {
        quality: v.quality.clone(),
        sample_rate: v.sample_rate,
        bit_depth: v.bit_depth,
        abr: v.abr,
        sbc_bitpool: v.sbc_bitpool,
        sbcxq_bitpool: v.sbcxq_bitpool,
        sbc_min_bitpool: v.sbc_min_bitpool,
        sbcxq_min_bitpool: v.sbcxq_min_bitpool,
        sbc_blocks: v.sbc_blocks,
        sbc_subbands: v.sbc_subbands,
        sbc_allocation: v.sbc_allocation.clone(),
        sbc_channel_mode: v.sbc_channel_mode.clone(),
    }
}

fn from_ipc_codec_values(v: &ipc::CodecValues) -> profiles::CodecValues {
    profiles::CodecValues {
        quality: v.quality.clone(),
        sample_rate: v.sample_rate,
        bit_depth: v.bit_depth,
        abr: v.abr,
        sbc_bitpool: v.sbc_bitpool,
        sbcxq_bitpool: v.sbcxq_bitpool,
        sbc_min_bitpool: v.sbc_min_bitpool,
        sbcxq_min_bitpool: v.sbcxq_min_bitpool,
        sbc_blocks: v.sbc_blocks,
        sbc_subbands: v.sbc_subbands,
        sbc_allocation: v.sbc_allocation.clone(),
        sbc_channel_mode: v.sbc_channel_mode.clone(),
    }
}

/// Applies the UI's profile onto the stored one (unknown keys kept)
fn merge_profile(base: Option<&CoreProfile>, p: &Profile) -> CoreProfile {
    let mut c = base.cloned().unwrap_or_default();
    c.name = p.name.clone();
    c.device_address = p.device_address.to_uppercase();
    c.device_name = p.device_name.clone();
    c.codec = p.codec.clone();
    c.quality = p.quality.clone();
    c.abr = p.abr;
    c.codec_fallback = p.codec_fallback;
    c.sample_rate = p.sample_rate;
    c.bit_depth = p.bit_depth;
    c.capture_mode = p.capture_mode.clone();
    c.audio_device_id = p.audio_device_id.clone();
    c.audio_device_name = p.audio_device_name.clone();
    c.auto_switch_device = p.auto_switch_device;
    c.app_exe = p.app_exe.clone();
    c.app_name = p.app_name.clone();
    c.volume_sync = p.volume_sync.clone();
    c.volume_device_id = p.volume_device_id.clone();
    c.volume_device_name = p.volume_device_name.clone();
    c.max_media_payload = Some(bt_host::media::clamp_media_payload_limit(p.max_media_payload as u32));
    c.le_audio = p.le_audio;
    c.le_low_latency = p.le_low_latency;
    c.sbc_bitpool = p.sbc_bitpool;
    c.sbcxq_bitpool = p.sbcxq_bitpool;
    c.sbc_min_bitpool = p.sbc_min_bitpool;
    c.sbcxq_min_bitpool = p.sbcxq_min_bitpool;
    c.sbc_blocks = p.sbc_blocks;
    c.sbc_subbands = p.sbc_subbands;
    c.sbc_allocation = p.sbc_allocation.clone();
    c.sbc_channel_mode = p.sbc_channel_mode.clone();
    // The codec in use has its settings in the fields above
    c.codec_settings = p
        .codec_settings
        .iter()
        .filter(|(k, _)| **k != p.codec)
        .map(|(k, v)| (k.clone(), from_ipc_codec_values(v)))
        .collect();
    // The codecs' keys, each once (the cards' order: FastStream and Opus 05
    // too, which Auto and the fallback never take); the UI sends none for
    // the default order
    c.codec_order = Vec::new();
    for k in p.codec_order.iter().filter_map(|k| codec::CodecKind::from_key(k)) {
        if !c.codec_order.iter().any(|x| x == k.key()) {
            c.codec_order.push(k.key().to_string());
        }
    }
    c
}

fn to_ipc_settings(s: &CoreSettings) -> Settings {
    Settings {
        language: s.language.clone(),
        theme: s.theme.clone(),
        start_with_windows: s.start_with_windows,
        start_minimized: s.start_minimized,
        minimize_to_tray: s.minimize_to_tray,
        tray_click_opens_window: s.tray_click_opens_window,
        notify_connected: s.notify_connected,
        battery_low_percent: s.battery_low_percent,
        check_updates_on_startup: s.check_updates_on_startup,
        include_prereleases: s.include_prereleases,
        media_keys: s.media_keys,
        remember_volume: s.remember_volume,
        auto_reconnect: s.auto_reconnect,
        volume_sync: s.volume_sync,
        developer_mode: s.debug_mode,
        dev_overrides: DevOverrides {
            rtp_marker: s.dev_overrides.rtp_marker,
            packet_dump: s.dev_overrides.packet_dump,
            aac_config_bitrate0: s.dev_overrides.aac_config_bitrate0,
            aac_bitrate_kbps: s.dev_overrides.aac_bitrate_kbps,
            aac_no_peak: s.dev_overrides.aac_no_peak,
            aac_no_afterburner: s.dev_overrides.aac_no_afterburner,
            aac_latm_v0: s.dev_overrides.aac_latm_v0,
        },
        dev_log: DevLog {
            keep_days: s.dev_log.keep_days,
            day_cap_mb: s.dev_log.day_cap_mb,
            always_verbose: match s.dev_log.always_verbose.as_str() {
                "debug" => Some(ipc::LogLevel::Debug),
                "trace" => Some(ipc::LogLevel::Trace),
                _ => None,
            },
            health_every_interval: s.dev_log.health_every_interval,
        },
        bt_chip_pid: s.bt_chip_pid,
        bt_chip_fw_stem: s.bt_chip_fw_stem.clone(),
        afh: s.afh.clone(),
        remote_sink: s.remote_sink.clone(),
        le_audio_experimental: s.le_audio_experimental,
        le_audio_low_latency: s.le_audio_low_latency,
        experimental_codecs: s.experimental_codecs,
        auto_codec: s.auto_codec,
    }
}

fn merge_settings(base: &CoreSettings, s: &Settings) -> CoreSettings {
    let mut c = base.clone();
    c.language = s.language.clone();
    c.theme = s.theme.clone();
    c.start_with_windows = s.start_with_windows;
    c.start_minimized = s.start_minimized;
    c.minimize_to_tray = s.minimize_to_tray;
    c.tray_click_opens_window = s.tray_click_opens_window;
    c.notify_connected = s.notify_connected;
    c.battery_low_percent = s.battery_low_percent;
    c.check_updates_on_startup = s.check_updates_on_startup;
    c.include_prereleases = s.include_prereleases;
    c.media_keys = s.media_keys;
    c.remember_volume = s.remember_volume;
    c.auto_reconnect = s.auto_reconnect;
    c.volume_sync = s.volume_sync;
    c.debug_mode = s.developer_mode;
    c.dev_overrides = codec::dev::DevOverrides {
        rtp_marker: s.dev_overrides.rtp_marker,
        packet_dump: s.dev_overrides.packet_dump,
        aac_config_bitrate0: s.dev_overrides.aac_config_bitrate0,
        // A bit rate AAC can use at all: 0 (off) or 32..=512 kbit/s
        aac_bitrate_kbps: match s.dev_overrides.aac_bitrate_kbps {
            0 => 0,
            k => k.clamp(32, 512),
        },
        aac_no_peak: s.dev_overrides.aac_no_peak,
        aac_no_afterburner: s.dev_overrides.aac_no_afterburner,
        aac_latm_v0: s.dev_overrides.aac_latm_v0,
    };
    c.dev_log = a2dpwb_core::settings::DevLog {
        keep_days: s.dev_log.keep_days,
        day_cap_mb: s.dev_log.day_cap_mb,
        always_verbose: match s.dev_log.always_verbose {
            Some(ipc::LogLevel::Debug) => "debug".into(),
            Some(ipc::LogLevel::Trace) => "trace".into(),
            _ => String::new(),
        },
        health_every_interval: s.dev_log.health_every_interval,
    }
    .clamped();
    c.bt_chip_pid = s.bt_chip_pid;
    c.bt_chip_fw_stem = s.bt_chip_fw_stem.clone();
    c.afh = s.afh.clone();
    c.remote_sink = s.remote_sink.clone();
    c.le_audio_experimental = s.le_audio_experimental;
    c.le_audio_low_latency = s.le_audio_low_latency;
    // experimental_codecs: always on (issue #86), as loaded
    c.auto_codec = s.auto_codec;
    c
}

/// What the UI tells about an adapter's firmware besides Realtek's files
fn vendor_note(vendor: ChipVendor) -> Option<Msg> {
    let key = match vendor {
        ChipVendor::Intel => "firmware.vendor_intel",
        ChipVendor::Broadcom => "firmware.vendor_broadcom",
        ChipVendor::Csr => "firmware.vendor_csr",
        ChipVendor::MediaTek | ChipVendor::Qualcomm => "firmware.vendor_unsupported",
        ChipVendor::Realtek | ChipVendor::Unknown => return None,
    };
    Some(Msg::new(key))
}

/// Device Manager's "Windows cannot start this device until it restarts"
const CM_PROB_NEED_RESTART: u32 = 14;

/// LE Audio of the adapter A2DPWB uses (AdapterState::le_audio): the
/// running stack's, else what a stack found out on the first WinUSB adapter
fn le_audio_now(running: Option<bool>, adapters: &[AdapterInfo]) -> Option<bool> {
    running.or_else(|| adapters.iter().find(|a| a.driver == DriverKind::Winusb).and_then(|a| a.le_audio))
}

/// The Realtek chip (PID, firmware stem) whose files an adapter's card
/// reports on. The chip setting is one for all adapters, so an adapter of
/// another known vendor (MediaTek, Intel, ...) gets none, else it showed the
/// Realtek adapter's "Firmware OK" (issue #86)
fn adapter_chip(vendor: ChipVendor, realtek_pid: Option<u16>, settings: &CoreSettings) -> (u16, &str) {
    if vendor_note(vendor).is_some() {
        return (0, "");
    }
    (chips::resolve_chip_pid(settings.bt_chip_pid, &settings.bt_chip_fw_stem, [realtek_pid]), &settings.bt_chip_fw_stem)
}

fn adapter_infos(config_dir: &Path, settings: &CoreSettings) -> Vec<AdapterInfo> {
    let radios = win_sys::bt_radios::radios();
    a2dpwb_core::adapter::adapters(false)
        .into_iter()
        .map(|a| {
            // Windows' Bluetooth runs it: what Device Manager shows, kept
            // for when it is on WinUSB (issue #86). A radio it cannot be asked
            // on or that is not running (a vendor's driver, a MediaTek
            // composite device) may still have what Windows' stack stored on
            // its device node: also for an adapter A2DPWB does not support
            let radio = radios
                .iter()
                .find(|r| r.on_device(&a.device.instance_id))
                .cloned()
                .map(|mut r| {
                    if r.address.is_none()
                        && let Some(p) = win_sys::bt_radios::device_radio(&a.device.instance_id)
                    {
                        r.address = p.address;
                        r.hci_version = r.hci_version.or(p.hci_version);
                        r.lmp_version = r.lmp_version.or(p.lmp_version);
                    }
                    r
                })
                .or_else(|| win_sys::bt_radios::device_radio(&a.device.instance_id));
            if let Some(r) = &radio {
                a2dpwb_core::adapter::save_windows_radio(
                    config_dir,
                    a.device.vid,
                    a.device.pid,
                    &r.name,
                    r.address.as_deref(),
                    r.hci_version,
                );
            }
            let driver = match a.device.driver_kind {
                win_sys::driver::DriverKind::WinUsb => DriverKind::Winusb,
                win_sys::driver::DriverKind::Bluetooth => DriverKind::Bluetooth,
                win_sys::driver::DriverKind::Other => DriverKind::Other,
                win_sys::driver::DriverKind::None => DriverKind::None,
            };
            let (chip_pid, fw_stem) = adapter_chip(a.vendor, a.realtek_pid, settings);
            let files = chips::firmware_files(chip_pid, fw_stem);
            let check = chips::firmware_check(&paths::firmware_dir(config_dir), chip_pid, fw_stem);
            AdapterInfo {
                instance_id: a.device.instance_id.clone(),
                vid: a.device.vid,
                pid: a.device.pid,
                display_name: a.display_name.clone(),
                device_name: a.device.display_name().to_owned(),
                vendor: a.vendor.name().to_string(),
                driver,
                driver_service: a.device.driver.service.clone(),
                realtek_chip: a.realtek_pid.and_then(chips::realtek_chip_name).map(str::to_owned),
                firmware_present: check.as_ref().map(|c| c.present),
                firmware_download: files.as_ref().is_some_and(|(f, _)| a2dpwb_core::firmware::available(f)),
                firmware_files: files.map(|(f, c)| vec![f, c]).unwrap_or_default(),
                firmware_invalid: check.and_then(|c| c.invalid_file),
                vendor_note: vendor_note(a.vendor),
                restart_needed: a.device.problem_code == Some(CM_PROB_NEED_RESTART),
                le_audio: a2dpwb_core::adapter::le_audio(config_dir, a.device.vid, a.device.pid),
                // What Windows' radio of this very device reads now, else
                // what it or a stack of A2DPWB read when one ran it (kept per
                // VID:PID), else a Realtek adapter's USB serial: an adapter
                // neither ran yet (Windows uses one radio only, the second
                // one tells nothing) still shows it (issue #86)
                address: radio
                    .as_ref()
                    .and_then(|r| r.address.clone())
                    .or_else(|| a2dpwb_core::adapter::address(config_dir, a.device.vid, a.device.pid))
                    .or_else(|| a2dpwb_core::adapter::serial_address(a.vendor, &a.device.instance_id)),
                bt_version: radio
                    .as_ref()
                    .and_then(|r| r.hci_version)
                    .or_else(|| a2dpwb_core::adapter::hci_version(config_dir, a.device.vid, a.device.pid))
                    .and_then(a2dpwb_core::adapter::bt_core_version)
                    .map(str::to_owned),
                // An adapter already on WinUSB when A2DPWB came to know
                // radios: the device's own product string, as Device
                // Manager shows it (the WinUSB INF only names it "WinUSB
                // Device", in Windows' language)
                windows_name: a2dpwb_core::adapter::windows_name(config_dir, a.device.vid, a.device.pid).or_else(
                    || {
                        a.device
                            .bus_reported_name
                            .as_deref()
                            .map(str::trim)
                            .filter(|n| !n.is_empty())
                            .map(str::to_owned)
                    },
                ),
            }
        })
        .collect()
}

/// What a new adapter list tells about the firmware against the last one
#[derive(Debug, Default, PartialEq, Eq)]
struct FirmwareChange {
    /// An adapter's firmware was missing or invalid and is there now
    fixed: bool,
    /// Adapters (display name) with a file newly found invalid
    invalid: Vec<(String, String)>,
}

fn firmware_change(old: &[AdapterInfo], new: &[AdapterInfo]) -> FirmwareChange {
    let mut change = FirmwareChange::default();
    for a in new {
        let before = old.iter().find(|o| o.instance_id == a.instance_id);
        if a.firmware_present == Some(true) && before.is_some_and(|o| o.firmware_present == Some(false)) {
            change.fixed = true;
        }
        if let Some(file) = &a.firmware_invalid
            && before.is_none_or(|o| o.firmware_invalid.as_ref() != Some(file))
        {
            change.invalid.push((a.display_name.clone(), file.clone()));
        }
    }
    change
}

/// The log line of v1 check_firmware_present for each file found invalid
fn warn_invalid_firmware(config_dir: &Path, invalid: &[(String, String)]) {
    for (adapter, file) in invalid {
        tracing::warn!(
            "{adapter}: {file} in {} is not a Realtek firmware file (a saved web page?)",
            config_dir.display()
        );
    }
}

/// The saved devices (classic, then LE Audio), then the scanned ones; with
/// the codecs each classic device offered. An entry is its address AND its
/// kind: a dual-mode device can be listed twice, classic and LE Audio.
fn saved_devices(
    config_dir: &Path,
    scanned: &[Device],
    codecs: &BTreeMap<String, a2dpwb_core::KnownCodecs>,
) -> Vec<Device> {
    let saved = devices::load(config_dir).into_iter().chain(devices::load_le(config_dir));
    let mut list: Vec<Device> = saved
        .map(|d| Device {
            address: d.address,
            name: d.name,
            saved: true,
            audio: true,
            rssi: None,
            codecs: Vec::new(),
            sbc: None,
            codec_caps: Vec::new(),
            le_audio: d.le_audio,
            le_audio_set: false,
        })
        .collect();
    for s in scanned {
        match list.iter_mut().find(|d| d.le_audio == s.le_audio && d.address.eq_ignore_ascii_case(&s.address)) {
            Some(d) => {
                d.rssi = s.rssi;
                d.le_audio_set |= s.le_audio_set;
                if d.name.is_empty() {
                    d.name = s.name.clone();
                }
            }
            None => list.push(s.clone()),
        }
    }
    for d in list.iter_mut().filter(|d| !d.le_audio) {
        if let Some(c) = codecs.get(&d.address.to_uppercase()) {
            d.codecs = c.keys.clone();
            d.sbc = c.sbc.clone().map(to_ipc_sbc_caps);
            d.codec_caps = c
                .caps
                .iter()
                .map(|k| ipc::CodecDeviceCaps {
                    codec: k.codec.clone(),
                    sample_rates: k.sample_rates.clone(),
                    bit_depths: k.bit_depths.clone(),
                    max_bitrate: k.max_bitrate,
                })
                .collect();
        }
    }
    list
}

/// A scan's entry as the UI lists it (not saved yet)
fn scanned_device(d: &ScannedDevice) -> Device {
    Device {
        address: d.address.clone(),
        name: d.name.clone(),
        saved: false,
        audio: d.audio,
        rssi: Some(d.rssi),
        codecs: Vec::new(),
        sbc: None,
        codec_caps: Vec::new(),
        le_audio: d.le_audio,
        le_audio_set: d.le_audio_set,
    }
}

/// The codec the UI shows for an LE Audio (experimental) stream
const LE_CODEC: &str = "LC3";

/// The service's state message while a scan's LE Audio part runs
const LE_SCANNING_KEY: &str = "status.le_scanning";

/// StreamInfo.le of an LE Audio (experimental) stream
fn to_ipc_sbc_caps(c: a2dpwb_core::SbcDeviceCaps) -> ipc::SbcDeviceCaps {
    ipc::SbcDeviceCaps {
        sample_rates: c.sample_rates,
        channel_modes: c.channel_modes,
        blocks: c.blocks,
        subbands: c.subbands,
        allocations: c.allocations,
        min_bitpool: c.min_bitpool,
        max_bitpool: c.max_bitpool,
    }
}

/// The codec details as the IPC sends them (the same fields)
fn to_ipc_codec_details(d: a2dpwb_core::CodecDetails) -> ipc::CodecDetails {
    ipc::CodecDetails {
        mtu: d.mtu,
        rtp: d.rtp,
        media_header: d.media_header,
        frame_samples: d.frame_samples,
        frame_bytes: d.frame_bytes,
        frames_per_packet: d.frames_per_packet,
        sbc: d.sbc.map(|s| ipc::SbcDetails {
            channel_mode: s.channel_mode,
            blocks: s.blocks,
            subbands: s.subbands,
            allocation: s.allocation,
            bitpool: s.bitpool,
            config_bitpool: s.config_bitpool,
            device: s.device.map(to_ipc_sbc_caps),
            abr_bitpool: s.abr_bitpool,
        }),
        aac: d.aac.map(|a| ipc::AacDetails {
            object_type: a.object_type,
            bit_rate: a.bit_rate,
            device: a.device.map(|c| ipc::AacDeviceCaps {
                object_types: c.object_types,
                sample_rates: c.sample_rates,
                channels: c.channels,
                max_bitrate: c.max_bitrate,
                vbr: c.vbr,
            }),
        }),
        device: d.device.map(|c| ipc::CodecDeviceCaps {
            codec: c.codec,
            sample_rates: c.sample_rates,
            bit_depths: c.bit_depths,
            max_bitrate: c.max_bitrate,
        }),
    }
}

fn to_ipc_le_stream(d: &LeStreamDetails) -> LeStreamInfo {
    LeStreamInfo {
        preset: d.preset.clone(),
        frame_us: d.frame_us,
        octets: d.octets,
        cis: d.cis,
        devices: d.devices,
        low_latency: d.low_latency,
        summary: d.summary.clone(),
    }
}

/// The stream info with what an ABR encoder runs at now (the encode
/// thread's [`LinkStats::live_codec`]): the bit rate, and SBC's bitpool,
/// frame length and frames per packet. None when nothing changed, or the
/// stream is not an A2DP one under ABR (the values may be a stream's that
/// ended).
pub fn live_info(info: &ipc::StreamInfo, live: bt_host::LiveCodec) -> Option<ipc::StreamInfo> {
    if !info.abr || info.le.is_some() {
        return None;
    }
    let mut next = info.clone();
    next.bitrate_kbps = live.bitrate_kbps;
    if let Some(d) = next.details.as_deref_mut() {
        if let Some(s) = d.sbc.as_mut().filter(|s| s.abr_bitpool.is_some() && live.bitpool > 0) {
            s.bitpool = live.bitpool.into();
        }
        if live.frame_bytes > 0 && d.frame_bytes.is_some() {
            let bytes = u32::from(live.frame_bytes);
            d.frame_bytes = Some(bytes);
            // As stream::codec_details: whole frames in the payload, SBC
            // counts them in 4 bits
            let n = d.mtu.saturating_sub(d.media_header) / bytes;
            d.frames_per_packet = Some(if d.media_header == 1 { n.min(15) } else { n });
        }
    }
    (next != *info).then_some(next)
}

/// The cumulative media counters, those when the current stream started,
/// and the number of streams started
pub fn link_totals(s: &LinkStats) -> (LinkTotals, LinkTotals, u32) {
    let r = Ordering::Relaxed;
    // stream_starts is written after the start_* values
    let starts = s.stream_starts.load(Ordering::Acquire);
    let start = LinkTotals {
        packets: s.start_packets.load(r),
        bytes: s.start_bytes.load(r),
        dropped: s.start_dropped.load(r),
        capture_frames: s.start_capture_frames.load(r),
        capture_dropped: s.start_capture_dropped.load(r),
    };
    let total = LinkTotals {
        packets: s.packets_sent.load(r),
        bytes: s.bytes_sent.load(r),
        dropped: s.queue_drops.load(r) + s.send_errors.load(r),
        capture_frames: s.capture_frames.load(r),
        capture_dropped: s.capture_dropped_frames.load(r),
    };
    (total, start, starts)
}

/// The core's receiver types to the protocol's (the same fields)
fn convert<T: serde::Serialize, U: serde::de::DeserializeOwned>(v: &T) -> Option<U> {
    serde_json::to_value(v).ok().and_then(|j| serde_json::from_value(j).ok())
}

fn to_ipc_source(s: &a2dpwb_core::SourceInfo) -> SourceInfo {
    SourceInfo { kind: s.kind.to_string(), name: s.name.clone(), waiting: s.waiting }
}

/// The parameters of stream.start_custom
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CustomStream {
    pub device: String,
    pub device_name: Option<String>,
    pub codec: String,
    pub quality: String,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u32>,
    pub source: String,
    pub source_id: Option<String>,
    pub source_name: Option<String>,
    pub max_media_payload: Option<u16>,
    pub abr: Option<bool>,
    pub codec_fallback: Option<bool>,
}

/// What to stream for stream.start_custom: built as a profile is (same
/// checks and defaults), with the test tone as a source of its own
pub fn custom_request(c: &CustomStream, legacy_max_media_payload: u16) -> Result<StreamRequest, ErrorInfo> {
    const METHOD: &str = "stream.start_custom";
    let codec = c.codec.trim().to_ascii_lowercase();
    let codec = if codec.is_empty() { "auto".to_string() } else { codec };
    if codec != "auto" && codec::CodecKind::from_key(&codec).is_none() {
        return Err(bad_request("stream.bad_codec", METHOD, format!("unknown codec \"{codec}\"")));
    }
    let source = c.source.trim().to_ascii_lowercase();
    let (mode, tone) = match source.as_str() {
        "tone" => ("loopback", true),
        "" | "loopback" => ("loopback", false),
        "virtual" => ("virtual", false),
        "app" => ("app", false),
        other => return Err(bad_request("stream.bad_source", METHOD, format!("unknown source \"{other}\""))),
    };
    let id = c.source_id.clone().unwrap_or_default();
    let name = c.source_name.clone().unwrap_or_default();
    let p = CoreProfile {
        name: String::new(),
        device_address: c.device.trim().to_uppercase(),
        device_name: c.device_name.clone().unwrap_or_default(),
        codec,
        quality: if c.quality.trim().is_empty() { "hq".into() } else { c.quality.trim().to_ascii_lowercase() },
        abr: c.abr.unwrap_or(false),
        codec_fallback: c.codec_fallback.unwrap_or(false),
        sample_rate: c.sample_rate.unwrap_or(0),
        bit_depth: c.bit_depth.unwrap_or(0),
        capture_mode: mode.into(),
        audio_device_id: if mode == "virtual" { id.clone() } else { String::new() },
        audio_device_name: if mode == "virtual" { name.clone() } else { String::new() },
        // A test leaves Windows' default device alone
        auto_switch_device: false,
        app_exe: if mode == "app" { id } else { String::new() },
        app_name: if mode == "app" { name } else { String::new() },
        volume_sync: "off".into(),
        max_media_payload: c.max_media_payload.map(|v| bt_host::media::clamp_media_payload_limit(v as u32)),
        ..CoreProfile::default()
    };
    let mut req = p
        .to_request(legacy_max_media_payload)
        .map_err(|_| err("profile.invalid_address", "error.invalid_address_format"))?;
    // A profile always falls back; a custom stream only when asked
    req.codec_fallback = c.codec_fallback.unwrap_or(false);
    if tone {
        req.source = SourceRequest::TestTone;
    }
    Ok(req)
}

/// A profile's Windows Volume Sync: the setting for every device alone
/// (issue #86), on the captured device (Auto). A profile's own "off" from
/// before that setting, or a device it followed (once set in developer
/// mode), stays in the file but no longer counts: no screen shows or
/// resets it.
fn effective_volume_sync(global: bool, _profile: VolumeSync) -> VolumeSync {
    if global { VolumeSync::Auto } else { VolumeSync::Off }
}

/// HKCU Run value of v2: the resident core, which opens the UI unless
/// started with --background (start_minimized: only the tray at logon)
fn apply_autostart(enabled: bool, minimized: bool) -> Result<(), String> {
    use win_sys::autostart;
    if enabled {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        autostart::set(autostart::VALUE_NAME, &autostart::command_line(&exe, autostart_args(minimized)))
            .map_err(|e| e.to_string())
    } else {
        autostart::remove(autostart::VALUE_NAME).map(|_| ()).map_err(|e| e.to_string())
    }
}

/// What the start does with the Run value
#[derive(Debug, PartialEq, Eq)]
enum AutostartFix {
    Keep,
    /// Points it at this core (start_with_windows on)
    Write,
    /// Removes it (start_with_windows off)
    Remove,
}

/// The Run value at start, from `current` and start_with_windows
/// (`enabled`). v1's (its A2DPWB.exe; v2 registers only the core) is taken
/// over as by an update when `replaces_v1`: pointed at v2, or removed when
/// v2's setting is off, so that v1 does not start at logon. Otherwise it is
/// written only when on and missing, empty, or naming an a2dpwb-core.exe
/// that does not `exist` any more. Any other program is not ours to replace.
fn autostart_fix(
    current: Option<&str>,
    enabled: bool,
    replaces_v1: bool,
    exists: impl Fn(&std::path::Path) -> bool,
) -> AutostartFix {
    let program = current.and_then(win_sys::autostart::program_of);
    let named = |name: &str| program.as_ref().and_then(|p| p.file_name()).is_some_and(|n| n.eq_ignore_ascii_case(name));
    if replaces_v1 && named("A2DPWB.exe") {
        return if enabled { AutostartFix::Write } else { AutostartFix::Remove };
    }
    if !enabled {
        return AutostartFix::Keep;
    }
    match &program {
        None => AutostartFix::Write,
        Some(p) if named("a2dpwb-core.exe") && !exists(p) => AutostartFix::Write,
        Some(_) => AutostartFix::Keep,
    }
}

fn autostart_args(minimized: bool) -> &'static str {
    if minimized { "--background" } else { "" }
}

/// The Run value needs writing for this settings change
fn autostart_changed(before: &CoreSettings, after: &CoreSettings) -> bool {
    before.start_with_windows != after.start_with_windows
        || (after.start_with_windows && before.start_minimized != after.start_minimized)
}

/// The chip or firmware setting changed: the stack must load firmware anew
fn chip_changed(before: &CoreSettings, after: &CoreSettings) -> bool {
    before.bt_chip_pid != after.bt_chip_pid || before.bt_chip_fw_stem != after.bt_chip_fw_stem
}

/// The codec and what it runs with: "LDAC 990 kbps | 96 kHz | 24-bit | ABR"
/// (AAC's variable bit rate is VBR, as the UI calls it)
fn tray_codec_line(codec: &str, kbps: u32, sample_rate: u32, bits: u32, abr: bool) -> String {
    format!("{codec} {}", codec_params(codec, kbps, sample_rate, bits, abr))
}

/// The codec's values without its name: "990 kbps | 96 kHz | 24-bit | ABR"
fn codec_params(codec: &str, kbps: u32, sample_rate: u32, bits: u32, abr: bool) -> String {
    let mut line = format!("{kbps} kbps | {} kHz", sample_rate as f64 / 1000.0);
    if bits > 0 {
        line.push_str(&format!(" | {bits}-bit"));
    }
    if abr {
        line.push_str(if codec == "AAC" { " | VBR" } else { " | ABR" });
    }
    line
}

/// The balloon of a new connection (issue #86): "WH-1000XM4 connected",
/// then, as the connection info card shows them, the battery when it is
/// known already, the codec, its values on a line of their own
/// ([`codec_params`]), and the codec asked for when another one is used
/// (the UI's note beside the codec). None without the stream info: the
/// codec's values are the negotiated ones, known once it is set up.
fn connected_balloon(lang: &str, device: Option<&str>, stream: &StreamStatus) -> Option<BalloonRequest> {
    let i = stream.info.as_ref()?;
    let tr = |key: &str, default: &str| a2dpwb_core::messages::translate(lang, key, default);
    let printf = a2dpwb_core::messages::format_printf;
    let title = match device.filter(|d| !d.is_empty()) {
        Some(d) => printf(&tr("notify.connected", "%s connected"), &[d.to_string()]),
        None => tr("status.connected", "Connected"),
    };
    let mut lines = Vec::new();
    if let Some(b) = stream.battery {
        lines.push(printf(&tr("ui.home.battery", "Battery %d%%"), &[b.to_string()]));
    }
    lines.push(i.codec.clone());
    lines.push(codec_params(&i.codec, i.bitrate_kbps, i.sample_rate, i.bits, i.abr));
    if let Some(req) = &i.requested_codec {
        let (key, default) = if stream.message.key == "status.connected_fallback_experimental" {
            ("ui.codec.fallback_experimental", "(%s is experimental)")
        } else if i.requested_in_use {
            ("ui.codec.fallback_in_use", "(%s is in use)")
        } else {
            ("ui.codec.fallback_unsupported", "(%s is unsupported)")
        };
        lines.push(printf(&tr(key, default), std::slice::from_ref(req)));
    }
    Some(BalloonRequest { title, text: lines.join("\n"), url: None })
}

/// The tray icon's tooltip while streaming: "A2DPWB - Streaming (WH-1000XM4)"
/// and one line each for the codec and the battery
fn tray_tooltip(base: &str, device: Option<&str>, lines: &[String]) -> String {
    let mut t = match device.filter(|d| !d.is_empty()) {
        Some(d) => format!("{base} ({d})"),
        None => base.to_string(),
    };
    for l in lines {
        t.push('\n');
        t.push_str(l);
    }
    t
}

/// A tray menu item: the device, its LE Audio entry tagged as in the
/// flyout ("WF-1000XM5 (LE Audio)")
fn tray_label(device: &str, le_audio: bool, le_tag: &str) -> String {
    if le_audio { format!("{device} ({le_tag})") } else { device.to_string() }
}

/// A saved profile is the one streaming or connecting (under its name before
/// the save): it is started again with the new settings
fn restart_after_save(stream: &StreamStatus, saved: &str) -> bool {
    matches!(stream.state, StreamState::Connecting | StreamState::Streaming | StreamState::Reconnecting)
        && stream.profile.as_deref() == Some(saved)
}

/// The save changed only the source of an A2DP profile streaming: the stream
/// takes it in place ([`Service::change_source`]). What is still connecting
/// starts again, as a restart already on its way shows as connecting too.
fn source_changes_in_place(stream: &StreamStatus, before: Option<&CoreProfile>, saved: &CoreProfile) -> bool {
    stream.state == StreamState::Streaming && !saved.le_audio && before.is_some_and(|b| b.only_source_differs(saved))
}

/// Whether turning Auto-select (for every device) on or off may change the
/// codec of the profile in use: it chose one, or its own codec order (a
/// profile left on "auto" in the default order streams as Auto either
/// way). Not LE Audio (LC3).
fn restart_for_auto(stream: &StreamStatus, p: &CoreProfile) -> bool {
    restart_after_save(stream, &p.name) && !p.le_audio && (p.codec != "auto" || !p.order().is_default())
}

/// Whether a new codec order of the device (its cards dragged, issue #86
/// round 8) may change the codec of the profile in use (`p` as it streams):
/// it is Auto or fell back (the fallback takes the first codec in the
/// order). Not LE Audio.
fn restart_for_order(stream: &StreamStatus, p: &CoreProfile) -> bool {
    let fell_back = stream.info.as_ref().is_some_and(|i| i.requested_codec.is_some());
    restart_after_save(stream, &p.name) && !p.le_audio && (p.codec == "auto" || fell_back)
}

fn unix_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

impl CoreState {
    pub fn new(
        service: Arc<Service>,
        tray: Box<dyn Fn(TrayView) + Send + Sync>,
        quit: Box<dyn Fn() + Send + Sync>,
        balloon: Box<dyn Fn(BalloonRequest) + Send + Sync>,
    ) -> Arc<CoreState> {
        let dir = service.config_dir().to_path_buf();
        let settings = service.settings();
        let legacy = settings.legacy_max_media_payload();
        let profiles: Vec<Profile> = profiles::load(&dir).iter().map(|p| to_ipc_profile(p, legacy)).collect();
        let active = Some(settings.last_profile.clone()).filter(|n| profiles.iter().any(|p| &p.name == n));
        let adapters = adapter_infos(&dir, &settings);
        warn_invalid_firmware(&dir, &firmware_change(&[], &adapters).invalid);
        let snapshot = Snapshot {
            adapter: AdapterState {
                status: AdapterStatus::Stopped,
                le_audio: le_audio_now(service.le_audio_supported(), &adapters),
                adapters,
                error: None,
                rom_firmware: false,
            },
            devices: saved_devices(&dir, &[], &service.device_codecs()),
            scanning: false,
            le_scanning: false,
            stream: StreamStatus { message: Msg::new("status.ready"), ..Default::default() },
            profiles,
            active_profile: active,
            settings: to_ipc_settings(&settings),
            update: UpdateState { current_version: env!("CARGO_PKG_VERSION").into(), ..Default::default() },
            config_dir: dir.display().to_string(),
            // Where main.rs has the log files written
            log_dir: dir.join("logs").display().to_string(),
            capture: None,
            // Set by logging::init, first thing in main
            clock_ms: crate::logging::hub().clock(),
            // Set by the resident core's watch (resident.rs)
            v1_running: false,
            verbose: crate::logging::verbose(),
            crashes: crate::crashes::unseen(&dir, &settings),
        };
        let (events, _) = broadcast::channel(256);
        let state = Arc::new(CoreState {
            snapshot: Mutex::new((0, snapshot)),
            events,
            service,
            link_subscribers: AtomicUsize::new(0),
            receiver_subscribers: AtomicUsize::new(0),
            receiver: ReceiverClient::new(),
            started: Instant::now(),
            rates: Mutex::new(None),
            starting_profile: Mutex::new(None),
            battery_warned: AtomicBool::new(false),
            connect_announce: AtomicBool::new(false),
            battery_deadline: Mutex::new(None),
            quiet_connect: AtomicBool::new(false),
            restarting: AtomicBool::new(false),
            tray,
            quit,
            balloon,
        });
        state.update_tray();
        state
    }

    pub fn config_dir(&self) -> PathBuf {
        self.service.config_dir().to_path_buf()
    }

    pub fn snapshot(&self) -> (u64, Snapshot) {
        self.snapshot.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// The revision of the state now
    pub fn rev(&self) -> u64 {
        self.snapshot.lock().unwrap_or_else(|e| e.into_inner()).0
    }

    /// Reads part of the snapshot without copying all of it
    pub fn with_snapshot<R>(&self, f: impl FnOnce(&Snapshot) -> R) -> R {
        f(&self.snapshot.lock().unwrap_or_else(|e| e.into_inner()).1)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<(u64, Arc<Event>)> {
        self.events.subscribe()
    }

    /// Applies an event to the snapshot and sends it to the clients
    pub fn publish(&self, event: Event) {
        let rev = {
            let mut g = self.snapshot.lock().unwrap_or_else(|e| e.into_inner());
            let s = &mut g.1;
            match &event {
                Event::Adapter(a) => s.adapter = a.clone(),
                Event::Devices(d) => s.devices = d.clone(),
                Event::Scanning(b) => s.scanning = *b,
                Event::LeScanning(b) => s.le_scanning = *b,
                Event::Stream(st) => s.stream = st.clone(),
                Event::Profiles { profiles, active_profile } => {
                    s.profiles = profiles.clone();
                    s.active_profile = active_profile.clone();
                }
                Event::Settings(x) => s.settings = x.clone(),
                Event::Update(u) => s.update = u.clone(),
                Event::Capture(c) => s.capture = c.clone(),
                Event::V1Running(b) => s.v1_running = *b,
                Event::Verbose(v) => s.verbose = *v,
                Event::Crashes(c) => s.crashes = c.clone(),
                // Not state: no revision
                Event::Link(_)
                | Event::Log(_)
                | Event::Receiver(_)
                | Event::RfcommData(_)
                | Event::RfcommClosed(_)
                | Event::Bye => {
                    let _ = self.events.send((g.0, Arc::new(event)));
                    return;
                }
            }
            g.0 += 1;
            g.0
        };
        let _ = self.events.send((rev, Arc::new(event)));
        self.update_tray();
    }

    /// What an ABR encoder runs at now into the stream info (the sampler, a
    /// few times a second): published when it changed. Checked and applied
    /// under one lock, so that a state change meanwhile is never undone.
    pub fn apply_live_codec(&self, live: Option<bt_host::LiveCodec>) {
        let Some(live) = live else { return };
        {
            let mut g = self.snapshot.lock().unwrap_or_else(|e| e.into_inner());
            let stream = &g.1.stream;
            let next = match (&stream.state, &stream.info) {
                (StreamState::Streaming, Some(i)) => live_info(i, live),
                _ => None,
            };
            let Some(next) = next else { return };
            tracing::trace!(
                "stream info: {} kbps, bitpool {:?}",
                next.bitrate_kbps,
                next.details.as_ref().and_then(|d| d.sbc.as_ref()).map(|s| s.bitpool)
            );
            g.1.stream.info = Some(next);
            g.0 += 1;
            let event = Event::Stream(g.1.stream.clone());
            let _ = self.events.send((g.0, Arc::new(event)));
        }
        self.update_tray();
    }

    /// A2DPWB v1 started or exited: published when it changed
    pub fn set_v1_running(&self, on: bool) {
        if self.with_snapshot(|s| s.v1_running) == on {
            return;
        }
        if on {
            tracing::warn!("A2DPWB v1 is running: it may hold the Bluetooth adapter");
        } else {
            tracing::info!("A2DPWB v1 exited");
        }
        self.publish(Event::V1Running(on));
    }

    fn update_tray(&self) {
        let (_, s) = self.snapshot();
        let lang = s.settings.language.clone();
        let streaming = s.stream.state == StreamState::Streaming;
        let base = if streaming {
            a2dpwb_core::messages::translate(&lang, "tray.tooltip_streaming", "A2DP Windows Bridge - Connected")
        } else {
            a2dpwb_core::messages::translate(&lang, "tray.tooltip_idle", "A2DP Windows Bridge - Idle")
        };
        // The streamed profile's device (v1)
        let device = streamed_device(&s);
        let tooltip = if streaming {
            let mut lines = Vec::new();
            if let Some(i) = &s.stream.info {
                lines.push(tray_codec_line(&i.codec, i.bitrate_kbps, i.sample_rate, i.bits, i.abr));
            }
            if let Some(b) = s.stream.battery {
                let template = a2dpwb_core::messages::translate(&lang, "ui.home.battery", "Battery %d%%");
                lines.push(a2dpwb_core::messages::format_printf(&template, &[b.to_string()]));
            }
            tray_tooltip(&base, device, &lines)
        } else {
            base
        };
        let le = a2dpwb_core::messages::translate(&lang, "device.tag_le_audio", "LE Audio");
        let labels = s.profiles.iter().map(|p| tray_label(device_name(&s.devices, p), p.le_audio, &le)).collect();
        (self.tray)(TrayView {
            streaming,
            tooltip,
            profiles: s.profiles.iter().map(|p| p.name.clone()).collect(),
            labels,
            active_profile: s.active_profile.clone(),
            language: lang,
        });
    }

    /// A service event (from its thread)
    pub fn on_service_event(self: &Arc<Self>, e: ServiceEvent) {
        let (_, s) = self.snapshot();
        let mut stream = s.stream.clone();
        let mut devices_changed = false;
        match e {
            ServiceEvent::State(state, m) => {
                // The scan's LE Audio part runs while the state says so
                // (Service::scan_devices; the next message ends it)
                let le_scanning = m.key == LE_SCANNING_KEY;
                if le_scanning != s.le_scanning {
                    self.publish(Event::LeScanning(le_scanning));
                }
                stream.state = match state {
                    CoreStreamState::Idle => StreamState::Idle,
                    CoreStreamState::Connecting => StreamState::Connecting,
                    CoreStreamState::Streaming => StreamState::Streaming,
                    CoreStreamState::Reconnecting => StreamState::Reconnecting,
                    CoreStreamState::Error => StreamState::Error,
                };
                stream.message = msg(&m);
                // A new connection may warn about its battery again
                if matches!(state, CoreStreamState::Idle | CoreStreamState::Error | CoreStreamState::Connecting) {
                    self.battery_warned.store(false, Ordering::Release);
                }
                // A connection (also the headphones back on after they went
                // off) is announced; a lost link reconnecting is not, its
                // dropouts would repeat the same balloon; nor is the same
                // device started again with a new codec (issue #86)
                match state {
                    CoreStreamState::Connecting => {
                        if !self.quiet_connect.load(Ordering::Acquire) {
                            self.connect_announce.store(true, Ordering::Release);
                            *self.battery_deadline.lock().unwrap_or_else(|e| e.into_inner()) = None;
                        }
                    }
                    CoreStreamState::Streaming => self.quiet_connect.store(false, Ordering::Release),
                    CoreStreamState::Idle | CoreStreamState::Error => {
                        self.connect_announce.store(false, Ordering::Release)
                    }
                    _ => {}
                }
                // A lost link's battery is stale until the device reports
                // again (issue #86: a battery shown while disconnected)
                if state == CoreStreamState::Reconnecting {
                    stream.battery = None;
                    stream.battery_refused = false;
                }
                if matches!(state, CoreStreamState::Idle | CoreStreamState::Error) {
                    stream.info = None;
                    stream.volume = None;
                    stream.battery = None;
                    stream.battery_refused = false;
                    if state == CoreStreamState::Idle {
                        stream.profile = None;
                    }
                }
                // The new session starts (after the old one's Idle when
                // switching profiles): its profile is the one asked for
                if state == CoreStreamState::Connecting {
                    stream.profile = self.starting_profile.lock().unwrap_or_else(|e| e.into_inner()).clone();
                }
                // The stack runs once anything connected
                let adapter_status = match state {
                    CoreStreamState::Error if m.key.starts_with("error.btstack") => Some(AdapterStatus::Error),
                    CoreStreamState::Streaming => Some(AdapterStatus::Ready),
                    _ => None,
                };
                if let Some(st) = adapter_status {
                    let mut a = s.adapter.clone();
                    if a.status != st {
                        a.status = st;
                        a.error = (st == AdapterStatus::Error).then(|| msg(&m));
                        self.publish(Event::Adapter(a));
                    }
                }
                // A connection learns the device's codecs, also one that
                // fails afterwards
                devices_changed = state == CoreStreamState::Error;
            }
            ServiceEvent::StreamInfo(i) => {
                // LE Audio (experimental): what the stream was set up with
                let le = self.service.le_stream().map(|d| to_ipc_le_stream(&d));
                stream.info = Some(ipc::StreamInfo {
                    codec: if le.is_some() { LE_CODEC.into() } else { i.codec },
                    bitrate_kbps: i.bitrate_kbps,
                    sample_rate: i.sample_rate,
                    channels: i.channels,
                    bits: i.bits,
                    source_sample_rate: i.source_sample_rate,
                    source_channels: i.source_channels,
                    source_bits: i.source_bits,
                    requested_codec: i.requested_codec,
                    requested_in_use: i.requested_in_use,
                    abr: i.abr,
                    bitrate_capped: i.bitrate_capped,
                    source: Some(to_ipc_source(&i.source)),
                    le,
                    details: i.details.map(|d| Box::new(to_ipc_codec_details(*d))),
                });
                // The device was saved when it connected
                devices_changed = true;
            }
            ServiceEvent::Volume(v) => stream.volume = Some(v),
            // Another saved device connected while the one streamed was off
            // (issue #86): its own profile starts, as a connect on Home
            ServiceEvent::OtherDeviceBack(addr) => {
                let address = bt_host::addr_str(&addr);
                let profile = profiles::load(&self.config_dir())
                    .into_iter()
                    .find(|p| !p.le_audio && p.device_address.eq_ignore_ascii_case(&address));
                match profile {
                    Some(p) => {
                        tracing::info!("{address} connected while the device streamed was off: starting {}", p.name);
                        if let Err(e) = self.start_profile(&p.name) {
                            tracing::warn!("{address}: {}", e.message.key);
                        }
                    }
                    None => tracing::debug!("{address} connected, but has no profile"),
                }
                return;
            }
            // A report that arrives after the device went off (HFP still
            // closing) is not shown on a device no longer connected
            ServiceEvent::Battery(_) | ServiceEvent::BatteryRefused
                if matches!(stream.state, StreamState::Idle | StreamState::Error) =>
            {
                tracing::debug!("battery report while not connected: ignored");
            }
            ServiceEvent::BatteryRefused => {
                stream.battery = None;
                stream.battery_refused = true;
            }
            ServiceEvent::Battery(v) => {
                stream.battery = Some(v);
                stream.battery_refused = false;
                let mut warned = self.battery_warned.load(Ordering::Acquire);
                let low = self.service.settings().battery_low_percent;
                let alert = battery_low_alert(&mut warned, v, low);
                self.battery_warned.store(warned, Ordering::Release);
                if alert {
                    self.battery_low(&s, v, low);
                }
            }
        }
        self.publish(Event::Stream(stream));
        self.announce_connected();
        if devices_changed {
            self.refresh_devices();
        }
        // A stack started and found out the adapter's LE Audio (remembered
        // in adapter_caps.json): the adapters anew
        if le_audio_now(self.service.le_audio_supported(), &s.adapter.adapters) != s.adapter.le_audio {
            self.list_adapters();
        }
        // The stack found its Realtek adapter on the ROM code, or a new
        // stack loaded the firmware
        let rom = self.service.host().is_some_and(|h| h.realtek_rom_firmware());
        let (_, s) = self.snapshot();
        if rom != s.adapter.rom_firmware {
            self.publish(Event::Adapter(AdapterState { rom_firmware: rom, ..s.adapter }));
        }
    }

    /// The device list again (saved file, last scan, codecs); published if
    /// it changed
    fn refresh_devices(&self) {
        let (_, s) = self.snapshot();
        let scanned: Vec<Device> = s.devices.iter().filter(|d| !d.saved).cloned().collect();
        let list = saved_devices(&self.config_dir(), &scanned, &self.service.device_codecs());
        if list != s.devices {
            self.publish(Event::Devices(list));
        }
    }

    fn save_profiles(&self, list: &[CoreProfile], active: Option<String>) -> Result<(), ErrorInfo> {
        let dir = self.config_dir();
        profiles::save(&dir, list).map_err(|_| err("profile.save_failed", "error.profile_save"))?;
        let legacy = self.service.settings().legacy_max_media_payload();
        self.publish(Event::Profiles {
            profiles: list.iter().map(|p| to_ipc_profile(p, legacy)).collect(),
            active_profile: active,
        });
        Ok(())
    }

    fn save_settings(&self, s: CoreSettings) -> Result<(), ErrorInfo> {
        s.save(&self.config_dir()).map_err(|_| err("settings.save_failed", "error.settings_save"))?;
        crate::logging::set_dev(&s.log());
        let ui = to_ipc_settings(&s);
        self.service.set_settings(s);
        self.publish(Event::Settings(ui));
        Ok(())
    }

    fn set_active_profile(&self, name: Option<String>) {
        let mut s = self.service.settings();
        if let Some(n) = &name
            && s.last_profile != *n
        {
            s.last_profile = n.clone();
            let _ = s.save(&self.config_dir());
            self.service.set_settings(s);
        }
        let (_, snap) = self.snapshot();
        self.publish(Event::Profiles { profiles: snap.profiles, active_profile: name });
    }

    pub fn refresh_adapters(&self) -> Vec<AdapterInfo> {
        let (list, fixed) = self.list_adapters();
        if fixed {
            // Files placed or replaced meanwhile: a stack started without
            // them keeps running the ROM code (v1 OnOpenFirmware)
            tracing::info!("firmware files are there now");
            self.restart_for_firmware();
        }
        list
    }

    /// The adapters anew, published; true if an adapter's firmware is there
    /// now that was missing or invalid
    fn list_adapters(&self) -> (Vec<AdapterInfo>, bool) {
        let (_, s) = self.snapshot();
        let list = adapter_infos(&self.config_dir(), &self.service.settings());
        let change = firmware_change(&s.adapter.adapters, &list);
        warn_invalid_firmware(&paths::firmware_dir(&self.config_dir()), &change.invalid);
        let le_audio = le_audio_now(self.service.le_audio_supported(), &list);
        self.publish(Event::Adapter(AdapterState { adapters: list.clone(), le_audio, ..s.adapter }));
        (list, change.fixed)
    }

    /// The chip or firmware setting changed: the firmware state anew, and a
    /// stack that loads the firmware again (v1 reset_btstack)
    fn reload_adapter(&self) {
        self.list_adapters();
        self.restart_for_firmware();
    }

    /// A stack that loads the firmware again: stopped now when idle, else
    /// at its next use
    fn restart_for_firmware(&self) {
        let (_, s) = self.snapshot();
        let idle = !s.scanning && matches!(s.stream.state, StreamState::Idle | StreamState::Error);
        if self.service.reload_adapter(idle) {
            tracing::info!("adapter settings changed: Bluetooth stopped, it starts again at the next use");
            self.publish(Event::Adapter(AdapterState {
                status: AdapterStatus::Stopped,
                error: None,
                le_audio: le_audio_now(self.service.le_audio_supported(), &s.adapter.adapters),
                rom_firmware: false,
                ..s.adapter
            }));
        } else {
            tracing::info!("adapter settings changed: Bluetooth restarts at its next use");
        }
    }

    /// Starts streaming a profile (tray and UI)
    pub fn start_profile(&self, name: &str) -> Result<(), ErrorInfo> {
        let dir = self.config_dir();
        let settings = self.service.settings();
        let list = profiles::load(&dir);
        let p =
            list.iter().find(|p| p.name == name).ok_or_else(|| err("profile.not_found", "error.profile_not_found"))?;
        // Auto-select for every device: Auto in the default order, whatever
        // codec and order the profile chose
        let auto = (settings.auto_codec && !p.le_audio).then(|| p.as_auto());
        if auto.is_some() && p.codec != "auto" {
            tracing::debug!(
                "profile {}: codec {} chosen, Auto-select streams Auto",
                a2dpwb_core::diag::quoted(&p.name),
                p.codec
            );
        }
        let mut req = auto
            .as_ref()
            .unwrap_or(p)
            .to_request(settings.legacy_max_media_payload())
            .map_err(|_| err("profile.invalid_address", "error.invalid_address"))?;
        req.experimental_codecs = settings.experimental_codecs;
        req.volume_sync = effective_volume_sync(settings.volume_sync, req.volume_sync);
        self.set_active_profile(Some(name.to_string()));
        self.begin_stream(req, Some(name.to_string()));
        Ok(())
    }

    /// Starts streaming without a profile (stream.start_custom)
    pub fn start_custom(&self, c: &CustomStream) -> Result<(), ErrorInfo> {
        let settings = self.service.settings();
        let mut req = custom_request(c, settings.legacy_max_media_payload())?;
        req.experimental_codecs = settings.experimental_codecs;
        tracing::info!("custom stream to {}: codec {}, source {}", c.device, c.codec, req.source);
        self.begin_stream(req, None);
        Ok(())
    }

    fn begin_stream(&self, req: StreamRequest, profile: Option<String>) {
        *self.starting_profile.lock().unwrap_or_else(|e| e.into_inner()) = profile.clone();
        self.publish(Event::Stream(StreamStatus {
            state: StreamState::Connecting,
            message: Msg::new("status.initializing"),
            profile,
            info: None,
            volume: None,
            battery: None,
            battery_refused: false,
        }));
        let service = self.service.clone();
        // A restart for a save waits a moment: a request right behind it
        // takes over before the first connection is under way (issue #86)
        let restart = self.restarting.swap(false, Ordering::AcqRel);
        self.quiet_connect.store(restart, Ordering::Release);
        // start_streaming joins a running session first: not on a runtime
        // thread. The ticket, taken now, keeps the order of quick switches:
        // only the last one asked for starts (issue #86)
        let ticket = service.ticket();
        std::thread::spawn(move || {
            if restart {
                std::thread::sleep(RESTART_COALESCE);
            }
            service.start_streaming_for(req, ticket)
        });
    }

    /// Starts the profile in use again with what was saved (a codec chosen,
    /// Auto-select turned off): the same device, so no "connected" balloon,
    /// and a second save right behind it (Home turns Auto-select off, then
    /// saves the codec) is one restart, not two (issue #86: a second restart
    /// broke off the first one's connection)
    fn restart_profile(&self, name: &str) -> Result<(), ErrorInfo> {
        self.restarting.store(true, Ordering::Release);
        let r = self.start_profile(name);
        self.restarting.store(false, Ordering::Release);
        r
    }

    pub fn stop(&self) {
        let service = self.service.clone();
        let ticket = service.ticket();
        std::thread::spawn(move || service.disconnect_for(ticket));
    }

    /// Settings > "Clicking the tray icon opens the main window"
    pub fn tray_click_opens_window(&self) -> bool {
        self.service.settings().tray_click_opens_window
    }

    pub fn quit(&self) {
        self.publish(Event::Bye);
        (self.quit)();
    }

    /// Start with Windows: at start v1's Run value is taken over (when
    /// `replaces_v1`), and v2's is written when it is missing or names a
    /// copy of the core that is gone (the program moved). One naming another
    /// copy of v2 that exists (the installed copy while a test build runs)
    /// is left alone; the settings toggle still writes it. See autostart_fix.
    pub fn refresh_autostart(&self, replaces_v1: bool) {
        let s = self.service.settings();
        let current = match win_sys::autostart::get(win_sys::autostart::VALUE_NAME) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("cannot read the start with Windows entry: {e}");
                return;
            }
        };
        let fix = autostart_fix(current.as_deref(), s.start_with_windows, replaces_v1, |p| p.exists());
        if fix == AutostartFix::Keep {
            tracing::debug!("start with Windows entry kept: {}", current.unwrap_or_default());
            return;
        }
        tracing::info!("start with Windows entry {fix:?}: was {}", current.unwrap_or_default());
        if let Err(e) = apply_autostart(fix == AutostartFix::Write, s.start_minimized) {
            tracing::warn!("cannot refresh the start with Windows entry: {e}");
        }
    }

    // ---- Tray balloons ------------------------------------------------------

    fn tr(&self, key: &str, default: &str) -> String {
        a2dpwb_core::messages::translate(&self.service.settings().language, key, default)
    }

    // ---- Crash reports -------------------------------------------------------

    /// Lists the crash reports not dismissed yet, if they changed
    fn refresh_crashes(&self) {
        let now = crate::crashes::unseen(&self.config_dir(), &self.service.settings());
        if self.with_snapshot(|s| s.crashes != now) {
            self.publish(Event::Crashes(now));
        }
    }

    /// The reports shown were seen: only newer ones are shown from now on
    fn crashes_seen(&self) {
        let Some(newest) = self.with_snapshot(|s| s.crashes.first().map(|c| c.name.clone())) else { return };
        let mut s = self.service.settings();
        crate::crashes::mark_seen(&mut s, &newest);
        if let Err(e) = s.save(&self.config_dir()) {
            tracing::warn!("cannot save the settings: {e}");
        }
        self.service.set_settings(s);
        self.publish(Event::Crashes(Vec::new()));
    }

    /// The UI hid its window to the tray (app.hidden_to_tray): v1's "still
    /// running in the tray" balloon, once
    pub fn hidden_to_tray(&self) {
        let mut s = self.service.settings();
        if s.other.get(TRAY_HINT_SHOWN).and_then(Value::as_bool) == Some(true) {
            return;
        }
        // No title: Windows heads the notification with the app's name
        // already (issue #86: the name three times)
        (self.balloon)(BalloonRequest {
            title: String::new(),
            text: self.tr("tray.minimized_hint", "A2DP Windows Bridge is still running in the system tray."),
            url: None,
        });
        s.other.insert(TRAY_HINT_SHOWN.into(), Value::Bool(true));
        if let Err(e) = s.save(&self.config_dir()) {
            tracing::warn!("cannot save the settings: {e}");
        }
        self.service.set_settings(s);
    }

    /// The streamed headphones' battery went low: a balloon, once per
    /// connection ([`battery_low_alert`])
    fn battery_low(&self, s: &Snapshot, level: u8, low: u8) {
        let device = streamed_device(s).unwrap_or("A2DP Windows Bridge");
        tracing::info!("battery: {device} at {level}%, below {low}%: balloon");
        let template = self.tr("notify.battery_low", "%s: battery low (%d%%)");
        // No title: Windows heads the notification with the app's name
        // already (issue #86: the name three times)
        (self.balloon)(BalloonRequest {
            title: String::new(),
            text: a2dpwb_core::messages::format_printf(&template, &[device.to_string(), level.to_string()]),
            url: None,
        });
    }

    /// A new connection streams with its codec settled (the stream info
    /// comes after the Streaming state on a first connection, before it on a
    /// reconnect): the balloon with what it runs ([`connected_balloon`]),
    /// once, when the setting is on. A battery not reported yet is waited
    /// for a moment (issue #86: the balloon came before it on some devices)
    fn announce_connected(self: &Arc<Self>) {
        let (_, s) = self.snapshot();
        if s.stream.state != StreamState::Streaming || s.stream.info.is_none() {
            return;
        }
        if !self.service.settings().notify_connected {
            self.connect_announce.store(false, Ordering::Release);
            return;
        }
        if !self.connect_announce.load(Ordering::Acquire) {
            return;
        }
        if s.stream.battery.is_none() && !s.stream.battery_refused && self.wait_for_battery() {
            return;
        }
        if !self.connect_announce.swap(false, Ordering::AcqRel) {
            return;
        }
        let Some(b) = connected_balloon(&s.settings.language, streamed_device(&s), &s.stream) else { return };
        tracing::info!("connected balloon: {} / {}", b.title, b.text.replace('\n', " / "));
        (self.balloon)(b);
    }

    /// Whether the balloon still waits for the battery; the first call
    /// starts the wait, which ends here again without a report. A wait left
    /// by an earlier connection finds this one's later deadline.
    fn wait_for_battery(self: &Arc<Self>) -> bool {
        let now = Instant::now();
        let mut deadline = self.battery_deadline.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(d) = *deadline {
            return now < d;
        }
        *deadline = Some(now + BATTERY_WAIT);
        let me = self.clone();
        std::thread::spawn(move || {
            std::thread::sleep(BATTERY_WAIT);
            me.announce_connected();
        });
        true
    }

    /// The automatic check found a newer release: a balloon whose click
    /// opens its page (v1)
    fn announce_update(&self, u: &UpdateState) {
        let Some(a) = &u.available else { return };
        let text = self
            .tr("update.balloon", "A2DP Windows Bridge %s is available. Click here to open the download page.")
            .replacen("%s", &a.version, 1);
        (self.balloon)(BalloonRequest {
            title: self.tr("update.available_title", "Update Available"),
            text,
            url: Some(a.url.clone()),
        });
    }

    // ---- Audio rates (flow) ------------------------------------------------

    pub fn set_rates(&self, r: Option<AudioRates>) {
        *self.rates.lock().unwrap_or_else(|e| e.into_inner()) = r;
    }

    pub fn rates(&self) -> Option<AudioRates> {
        *self.rates.lock().unwrap_or_else(|e| e.into_inner())
    }

    // ---- HCI capture -------------------------------------------------------

    fn start_capture(&self) -> Result<String, ErrorInfo> {
        let capture = self.service.capture();
        if let Some(p) = capture.path() {
            return Ok(p.display().to_string());
        }
        let path = self.config_dir().join(format!("hci_{}.pklg", local_stamp()));
        capture.start(&path).map_err(|_| err("diag.capture_failed", "error.capture_start"))?;
        let path = path.display().to_string();
        self.publish(Event::Capture(Some(CaptureState {
            path: path.clone(),
            bytes: capture.bytes_written(),
            started_ms: unix_ms(),
        })));
        Ok(path)
    }

    /// Stops the capture: its file and final size (None: none ran)
    fn stop_capture(&self) -> Option<CaptureState> {
        let capture = self.service.capture();
        let known = self.with_snapshot(|s| s.capture.clone());
        let path = capture.path();
        capture.stop();
        let done = match path {
            Some(p) => Some(CaptureState {
                path: p.display().to_string(),
                bytes: capture.bytes_written(),
                started_ms: known.as_ref().map_or(0, |k| k.started_ms),
            }),
            None => known.clone(),
        };
        if known.is_some() {
            self.publish(Event::Capture(None));
        }
        done
    }

    /// Verbose logging ran out (the sampler)
    pub fn expire_verbose(&self) {
        if crate::logging::expire_verbose() {
            self.publish(Event::Verbose(None));
        }
    }

    /// The running capture's size (the sampler, every few seconds)
    pub fn refresh_capture(&self) {
        let Some(known) = self.with_snapshot(|s| s.capture.clone()) else { return };
        let capture = self.service.capture();
        match capture.path() {
            None => self.publish(Event::Capture(None)),
            Some(_) => {
                let bytes = capture.bytes_written();
                if bytes != known.bytes {
                    self.publish(Event::Capture(Some(CaptureState { bytes, ..known })));
                }
            }
        }
    }

    // ---- Receiver test -----------------------------------------------------

    pub fn receiver_subscribed(&self) -> bool {
        self.receiver_subscribers.load(Ordering::Acquire) > 0
    }

    pub fn receiver_state(&self) -> ReceiverState {
        let (status, detail) = self.receiver.status();
        let connection = match status {
            receiver::ReceiverStatus::Off => ReceiverConnection::Off,
            receiver::ReceiverStatus::Connecting => ReceiverConnection::Connecting,
            receiver::ReceiverStatus::Connected => ReceiverConnection::Connected,
            receiver::ReceiverStatus::Failed => ReceiverConnection::Failed,
        };
        let stats = self.receiver.latest();
        let (total, start, starts) = link_totals(&self.service.stats());
        ReceiverState {
            connection,
            detail,
            target: if connection == ReceiverConnection::Off { String::new() } else { self.receiver.target() },
            receiver: self.receiver.receiver().as_ref().and_then(convert),
            stats_age_ms: stats.as_ref().and_then(|s| s.received).map(|t| t.elapsed().as_millis() as u64),
            stats: stats.as_ref().and_then(convert),
            sent: (starts > 0).then(|| total.since(&start)),
            stream_starts: starts,
        }
    }

    /// Sends the receiver state to its subscribers
    pub fn publish_receiver(&self) {
        if self.receiver_subscribed() {
            self.publish(Event::Receiver(Box::new(self.receiver_state())));
        }
    }

    // ---- Requests ----------------------------------------------------------

    /// One request; blocking work runs off the runtime's threads
    pub async fn handle(self: &Arc<Self>, method: Method) -> Result<Value, ErrorInfo> {
        let me = self.clone();
        let blocking = |f: BlockingJob| async move {
            tokio::task::spawn_blocking(move || f(&me))
                .await
                .unwrap_or_else(|_| Err(err("internal", "error.btstack_crash_stream")))
        };
        match method {
            Method::StateSnapshot => {
                // A window that opens after a crash (of itself, or of the
                // core while it was closed) finds the report
                self.refresh_crashes();
                Ok(json(&self.snapshot().1))
            }
            Method::StreamStart { profile } => self.start_profile(&profile).map(|()| Value::Null),
            Method::StreamStartCustom {
                device,
                device_name,
                codec,
                quality,
                sample_rate,
                bit_depth,
                source,
                source_id,
                source_name,
                max_media_payload,
                abr,
                codec_fallback,
            } => {
                let c = CustomStream {
                    device,
                    device_name,
                    codec,
                    quality,
                    sample_rate,
                    bit_depth,
                    source,
                    source_id,
                    source_name,
                    max_media_payload,
                    abr,
                    codec_fallback,
                };
                self.start_custom(&c).map(|()| Value::Null)
            }
            Method::StreamStop => {
                self.stop();
                Ok(Value::Null)
            }
            Method::StreamSetVolume { volume } => {
                if let Some(h) = self.service.host() {
                    h.request_volume(volume.min(127));
                }
                Ok(Value::Null)
            }
            Method::DeviceScan { seconds } => {
                blocking(Box::new(move |me| {
                    me.publish(Event::Scanning(true));
                    // The list shows the devices as they answer: first the
                    // saved ones alone (the last scan's are gone), then each
                    // device found. A scan refused (streaming) keeps the list.
                    let dir = me.config_dir();
                    let codecs = me.service.device_codecs();
                    if !me.service.is_running() {
                        me.publish(Event::Devices(saved_devices(&dir, &[], &codecs)));
                    }
                    let mut on_found = |found: &[ScannedDevice]| {
                        let scanned: Vec<Device> = found.iter().map(scanned_device).collect();
                        me.publish(Event::Devices(saved_devices(&dir, &scanned, &codecs)));
                    };
                    // Classic, then (LE Audio on) the LE Audio devices
                    let r = me.service.scan_devices(seconds.clamp(3, ipc::MAX_SCAN_SECONDS), &mut on_found);
                    if me.with_snapshot(|s| s.le_scanning) {
                        me.publish(Event::LeScanning(false));
                    }
                    me.publish(Event::Scanning(false));
                    let found = r.map_err(|m| ErrorInfo { code: "adapter.init_failed".into(), message: msg(&m) })?;
                    let scanned: Vec<Device> = found.iter().map(scanned_device).collect();
                    let list = saved_devices(&me.config_dir(), &scanned, &me.service.device_codecs());
                    me.publish(Event::Devices(list.clone()));
                    Ok(json(&list))
                }))
                .await
            }
            Method::DeviceScanStop => {
                self.service.stop_scan();
                Ok(Value::Null)
            }
            Method::DeviceForget { address, le_audio } => {
                let dir = self.config_dir();
                let r = if le_audio == Some(true) {
                    devices::remove_le(&dir, &address)
                } else {
                    devices::remove(&dir, &address)
                };
                r.map_err(|_| err("device.forget_failed", "error.device_forget"))?;
                self.refresh_devices();
                Ok(Value::Null)
            }
            Method::DeviceRename { address, name, le_audio } => {
                let name = name.trim();
                let Some(addr) = bt_host::parse_addr(&address) else {
                    return Err(err("device.invalid_address", "error.invalid_address_format"));
                };
                if name.is_empty() || name.contains(['\n', '\r', '|']) {
                    return Err(bad_request("device.invalid_name", "device.rename", format!("name \"{name}\"")));
                }
                let (dir, address) = (self.config_dir(), bt_host::addr_str(&addr));
                let r = if le_audio == Some(true) {
                    devices::save_le(&dir, &address, name)
                } else {
                    devices::save(&dir, &address, name)
                };
                r.map_err(|_| err("device.rename_failed", "error.device_rename"))?;
                self.refresh_devices();
                Ok(Value::Null)
            }
            Method::ProfileSave { profile, original_name } => {
                if profile.name.trim().is_empty() || bt_host::parse_addr(&profile.device_address).is_none() {
                    return Err(err("profile.invalid", "error.invalid_address_format"));
                }
                let dir = self.config_dir();
                let mut list = profiles::load(&dir);
                let key = original_name.as_deref().unwrap_or(&profile.name).to_string();
                let pos = list.iter().position(|p| p.name == key);
                let merged = merge_profile(pos.map(|i| &list[i]), &profile);
                let before = pos.map(|i| list[i].clone());
                // Only the device's codec order changed (its cards dragged):
                // the stream goes on unless the order changes its codec
                let order_only = pos.is_some_and(|i| {
                    list[i].codec_order != merged.codec_order
                        && CoreProfile { codec_order: merged.codec_order.clone(), ..list[i].clone() } == merged
                });
                // Under Auto-select the order is not used
                let order_unused = self.service.settings().auto_codec || merged.le_audio;
                let in_order = merged.clone();
                // The profile's device joins the saved ones of its kind (as
                // a connection saves it)
                let address = merged.device_address.clone();
                let known = if merged.le_audio { devices::load_le(&dir) } else { devices::load(&dir) };
                if !known.iter().any(|d| d.address == address) {
                    let r = if merged.le_audio {
                        devices::save_le(&dir, &address, &merged.device_name)
                    } else {
                        devices::save(&dir, &address, &merged.device_name)
                    };
                    if let Err(e) = r {
                        tracing::warn!("cannot save the device {address}: {e}");
                    }
                }
                match pos {
                    Some(i) => list[i] = merged,
                    None => list.push(merged),
                }
                let (_, s) = self.snapshot();
                let renamed_active = s.active_profile.as_deref() == Some(key.as_str()) && key != profile.name;
                let active = match s.active_profile {
                    Some(a) if a == key => Some(profile.name.clone()),
                    other => other.or(Some(profile.name.clone())),
                };
                self.save_profiles(&list, active)?;
                // A renamed active profile (its name follows the codec chosen
                // on the Codecs screen) stays the one remembered
                if renamed_active {
                    self.set_active_profile(Some(profile.name.clone()));
                }
                self.refresh_devices();
                // The profile in use starts again with what was saved (v1),
                // but for another source, which needs nothing of the link
                let order_keeps_codec = order_only && (order_unused || !restart_for_order(&s.stream, &in_order));
                if restart_after_save(&s.stream, &key) && !order_keeps_codec {
                    let in_place = source_changes_in_place(&s.stream, before.as_ref(), &in_order)
                        && self.service.change_source(in_order.source());
                    if in_place {
                        tracing::info!(
                            "profile {} saved while in use: its source changes in place",
                            a2dpwb_core::diag::quoted(&key)
                        );
                    } else {
                        tracing::info!(
                            "profile {} saved while in use: it starts again as {}",
                            a2dpwb_core::diag::quoted(&key),
                            a2dpwb_core::diag::quoted(&profile.name)
                        );
                        self.restart_profile(&profile.name)?;
                    }
                }
                Ok(Value::Null)
            }
            Method::ProfileDelete { name } => {
                let mut list = profiles::load(&self.config_dir());
                list.retain(|p| p.name != name);
                let (_, s) = self.snapshot();
                let active = s.active_profile.filter(|a| *a != name);
                self.save_profiles(&list, active)?;
                Ok(Value::Null)
            }
            Method::ProfileSetActive { name } => {
                self.set_active_profile(Some(name));
                Ok(Value::Null)
            }
            Method::SettingsSet { settings } => {
                blocking(Box::new(move |me| {
                    let base = me.service.settings();
                    let merged = merge_settings(&base, &settings);
                    if autostart_changed(&base, &merged) {
                        apply_autostart(merged.start_with_windows, merged.start_minimized).map_err(|e| {
                            tracing::warn!("start with Windows: {e}");
                            err("settings.autostart_failed", "error.autostart")
                        })?;
                    }
                    let chip = chip_changed(&base, &merged);
                    let le_toggled = base.le_audio_experimental != merged.le_audio_experimental;
                    let auto_toggled = base.auto_codec != merged.auto_codec;
                    let sync_toggled = base.volume_sync != merged.volume_sync;
                    me.save_settings(merged)?;
                    if chip {
                        me.reload_adapter();
                    }
                    // Auto-select on / off: a profile in use whose codec it
                    // changes starts again (as a profile save does); Windows
                    // Volume Sync on / off: the profile in use starts again
                    // (as when it was the profile's own setting)
                    if auto_toggled || sync_toggled {
                        let (_, s) = me.snapshot();
                        let list = profiles::load(&me.config_dir());
                        let in_use = s.stream.profile.as_deref().and_then(|n| list.iter().find(|p| p.name == n));
                        let restart = |p: &&CoreProfile| {
                            (auto_toggled && restart_for_auto(&s.stream, p))
                                || (sync_toggled && restart_after_save(&s.stream, &p.name))
                        };
                        if let Some(p) = in_use.filter(restart) {
                            tracing::info!(
                                "Auto-select {}, volume sync {}: profile {} starts again",
                                if settings.auto_codec { "on" } else { "off" },
                                if settings.volume_sync { "on" } else { "off" },
                                a2dpwb_core::diag::quoted(&p.name)
                            );
                            me.restart_profile(&p.name.clone())?;
                        }
                    }
                    // LE Audio on / off waits for the next scan or connection
                    // while the stack is in use (v1's deferred text)
                    let le_audio_deferred = le_toggled && me.service.le_audio_deferred();
                    Ok(json(&SettingsApplied { le_audio_deferred }))
                }))
                .await
            }
            Method::AdapterList => blocking(Box::new(|me| Ok(json(&me.refresh_adapters())))).await,
            Method::AdapterAssignWinusb { instance_id } => {
                blocking(Box::new(move |me| crate::driver::assign(me, &instance_id))).await
            }
            Method::AdapterRestoreDriver { instance_id } => {
                blocking(Box::new(move |me| crate::driver::restore(me, &instance_id))).await
            }
            Method::AdapterRestart => {
                blocking(Box::new(|me| {
                    let again = me.service.restart_adapter();
                    let (_, s) = me.snapshot();
                    me.publish(Event::Adapter(AdapterState {
                        status: AdapterStatus::Stopped,
                        error: None,
                        rom_firmware: false,
                        ..s.adapter
                    }));
                    // The device streamed before connects again on the new
                    // stack, with the same profile (issue #86)
                    if let Some(mut req) = again {
                        req.experimental_codecs = me.service.settings().experimental_codecs;
                        let profile = me.starting_profile.lock().unwrap_or_else(|e| e.into_inner()).clone();
                        me.begin_stream(req, profile);
                    }
                    Ok(Value::Null)
                }))
                .await
            }
            Method::AdapterFirmwareFiles => {
                blocking(Box::new(|me| {
                    let list: Vec<FirmwareFile> = chips::scan_firmware_files(&paths::firmware_dir(&me.config_dir()))
                        .into_iter()
                        .map(|f| FirmwareFile {
                            stem: f.stem,
                            display_name: f.display_name,
                            pid: f.pid,
                            has_config: f.has_config,
                        })
                        .collect();
                    Ok(json(&list))
                }))
                .await
            }
            Method::AdapterDownloadFirmware { file } => {
                blocking(Box::new(move |me| {
                    if !a2dpwb_core::firmware::available(&file) {
                        return Err(bad_request(
                            "firmware.no_download",
                            "adapter.download_firmware",
                            format!("no download for {file}"),
                        ));
                    }
                    if let Err(e) = a2dpwb_core::firmware::download(&paths::firmware_dir(&me.config_dir()), &file) {
                        tracing::warn!("firmware download failed: {e}");
                        return Err(err("firmware.download_failed", "firmware.download_failed"));
                    }
                    // The files are there now: a stack started without them
                    // loads them anew
                    Ok(json(&me.refresh_adapters()))
                }))
                .await
            }
            Method::AudioDevices => {
                blocking(Box::new(|_| {
                    let list: Vec<AudioDevice> = audio_capture::render_devices()
                        .unwrap_or_default()
                        .into_iter()
                        .map(|d| {
                            let mix = audio_capture::mix_format(Some(&d.id));
                            AudioDevice {
                                mix_rate: mix.map(|m| m.rate),
                                mix_channels: mix.map(|m| m.channels),
                                mix_bits: mix.map(|m| m.bits),
                                id: d.id,
                                name: d.name,
                                is_default: d.is_default,
                                is_virtual: d.is_virtual,
                            }
                        })
                        .collect();
                    Ok(json(&list))
                }))
                .await
            }
            Method::AudioApps => {
                blocking(Box::new(|_| {
                    let list: Vec<AudioApp> = audio_capture::audio_apps()
                        .into_iter()
                        .map(|a| AudioApp { exe: a.exe, name: a.name })
                        .collect();
                    Ok(json(&list))
                }))
                .await
            }
            // Handled per client by the server
            Method::DiagSubscribe { .. } => Ok(Value::Null),
            Method::RfcommOpen { .. } | Method::RfcommSend { .. } | Method::RfcommClose { .. } => {
                Err(err("rfcomm.no_channel", "ipc.error.rfcomm_no_channel"))
            }
            Method::DiagCaptureStart => self.start_capture().map(Value::String),
            Method::DiagCaptureStop => Ok(json(&self.stop_capture())),
            Method::DiagInfo(params) => {
                let mask = params.and_then(|p| p.mask).unwrap_or(true);
                // Lists the USB adapters: off the runtime
                blocking(Box::new(move |me| Ok(json(&DiagInfo { text: crate::diag_text(me, mask) })))).await
            }
            Method::DiagFlow => Ok(json(&crate::flow_view(self))),
            Method::DiagRecentLogs { level } => {
                let mut lines = crate::logging::hub().recent();
                lines.retain(|l| l.level <= level);
                Ok(json(&lines))
            }
            Method::DiagCrashesSeen => {
                self.crashes_seen();
                Ok(Value::Null)
            }
            Method::DiagMark { note } => Ok(json(&crate::logging::mark(&note))),
            Method::DiagSetVerbose { level } => {
                let v = crate::logging::set_verbose(level);
                self.publish(Event::Verbose(v));
                Ok(json(&v))
            }
            Method::DiagBundle(params) => {
                let p = params.unwrap_or_default();
                let (mask, hci) = (p.mask.unwrap_or(true), p.hci.unwrap_or(true));
                blocking(Box::new(move |me| {
                    crate::bundle::create(me, mask, hci).map(|b| json(&b)).map_err(|e| {
                        tracing::error!("diagnostic bundle: {e}");
                        ErrorInfo {
                            code: "diag.bundle_failed".into(),
                            message: Msg { key: "error.bundle_failed".into(), args: vec![e] },
                        }
                    })
                }))
                .await
            }
            Method::ReceiverDiscover => {
                blocking(Box::new(|_| {
                    let found: Vec<ipc::ReceiverInfo> =
                        receiver::discover(None, receiver::DEFAULT_PORT, receiver::DISCOVER_TIMEOUT)
                            .iter()
                            .filter_map(convert)
                            .collect();
                    Ok(json(&found))
                }))
                .await
            }
            Method::ReceiverConnect { target } => {
                let target = if target.trim().is_empty() { receiver::AUTO.to_string() } else { target.trim().into() };
                if receiver::split_target(&target).is_none() {
                    return Err(bad_request("receiver.invalid_target", "receiver.connect", format!("\"{target}\"")));
                }
                blocking(Box::new(move |me| {
                    // Ends a previous connection first (bounded by its
                    // connect and discovery timeouts)
                    me.receiver.start(&target);
                    let mut s = me.service.settings();
                    if s.remote_sink != target {
                        s.remote_sink = target;
                        me.save_settings(s)?;
                    }
                    me.publish_receiver();
                    Ok(json(&me.receiver_state()))
                }))
                .await
            }
            Method::ReceiverDisconnect => {
                blocking(Box::new(|me| {
                    me.receiver.stop();
                    me.publish_receiver();
                    Ok(Value::Null)
                }))
                .await
            }
            Method::ReceiverConfigure { mtu, buffer_ms } => {
                let (mtu, buffer_ms) = (mtu.unwrap_or(0), buffer_ms.unwrap_or(0));
                if mtu > u16::MAX as u32 || (mtu != 0 && mtu < 48) || buffer_ms > 10_000 {
                    return Err(bad_request(
                        "receiver.bad_settings",
                        "receiver.configure",
                        format!("mtu {mtu}, buffer {buffer_ms} ms"),
                    ));
                }
                if !self.receiver.configure(mtu, buffer_ms) {
                    return Err(err("receiver.not_connected", "receiver.off"));
                }
                Ok(Value::Null)
            }
            Method::ReceiverStatus => Ok(json(&self.receiver_state())),
            Method::AppCheckUpdate => blocking(Box::new(|me| me.check_update().map(|u| json(&u)))).await,
            Method::AppHiddenToTray => {
                self.hidden_to_tray();
                Ok(Value::Null)
            }
            Method::AppQuit => {
                self.quit();
                Ok(Value::Null)
            }
            Method::AppQuitForUpdate => match self.service.state() {
                CoreStreamState::Connecting | CoreStreamState::Streaming | CoreStreamState::Reconnecting => {
                    Err(err("app.streaming", "ui.update.streaming"))
                }
                CoreStreamState::Idle | CoreStreamState::Error => {
                    tracing::info!("exiting for an update");
                    self.quit();
                    Ok(Value::Null)
                }
            },
        }
    }

    /// Asks GitHub for a newer release (blocking) and publishes the result;
    /// Err (update.failed) when it could not ask, and the state says so too
    pub fn check_update(&self) -> Result<UpdateState, ErrorInfo> {
        use a2dpwb_core::update;
        let (_, s) = self.snapshot();
        self.publish(Event::Update(UpdateState { checking: true, ..s.update.clone() }));
        let prereleases = self.service.settings().include_prereleases;
        let current_version = update::current_version().to_string();
        match update::check(update::current_version(), prereleases) {
            Ok(r) => {
                let available = r.map(|r| ipc::UpdateInfo {
                    manifest_url: update::manifest_url(&r.tag),
                    version: r.version,
                    url: r.url,
                    prerelease: r.prerelease,
                    notes: r.notes,
                });
                let u = UpdateState { checking: false, available, current_version, error: None };
                self.publish(Event::Update(u.clone()));
                Ok(u)
            }
            Err(e) => {
                tracing::warn!("update check failed: {e}");
                let error = Msg::new("update.error");
                self.publish(Event::Update(UpdateState {
                    checking: false,
                    available: s.update.available.clone(),
                    current_version,
                    error: Some(error.clone()),
                }));
                Err(ErrorInfo { code: "update.failed".into(), message: error })
            }
        }
    }

    /// The automatic check: at most once a day (recorded before asking, as v1)
    pub fn check_update_if_due(self: &Arc<Self>) {
        use a2dpwb_core::update;
        let mut s = self.service.settings();
        let today = update::today();
        if !update::automatic_check_due(&s, &today) {
            return;
        }
        s.last_update_check = today;
        let _ = s.save(&self.config_dir());
        self.service.set_settings(s);
        let me = self.clone();
        std::thread::spawn(move || {
            // A failure is in the update state
            if let Ok(u) = me.check_update() {
                me.announce_update(&u);
            }
        });
    }

    pub fn link_subscribed(&self) -> bool {
        self.link_subscribers.load(Ordering::Acquire) > 0
    }
}

/// yyyyMMdd_HHmmss of the local time, for file names (as v1)
fn local_stamp() -> String {
    let t = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    format!("{:04}{:02}{:02}_{:02}{:02}{:02}", t.year(), u8::from(t.month()), t.day(), t.hour(), t.minute(), t.second())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn custom(source: &str) -> CustomStream {
        CustomStream {
            device: "aa:bb:cc:dd:ee:ff".into(),
            codec: "ldac".into(),
            quality: "sq".into(),
            source: source.into(),
            ..CustomStream::default()
        }
    }

    fn rtl(id: &str, present: Option<bool>, invalid: Option<&str>) -> AdapterInfo {
        AdapterInfo {
            instance_id: id.into(),
            vid: 0x0BDA,
            pid: 0x8771,
            display_name: "RTL8761BU".into(),
            device_name: String::new(),
            vendor: "Realtek".into(),
            driver: DriverKind::Winusb,
            driver_service: "WinUSB".into(),
            realtek_chip: Some("RTL8761BU".into()),
            firmware_present: present,
            firmware_files: vec!["rtl8761bu_fw.bin".into(), "rtl8761bu_config.bin".into()],
            firmware_invalid: invalid.map(str::to_owned),
            firmware_download: true,
            vendor_note: None,
            restart_needed: false,
            le_audio: None,
            address: None,
            bt_version: None,
            windows_name: None,
        }
    }

    #[test]
    fn a_chosen_realtek_chip_is_not_checked_on_other_vendors_adapters() {
        // RTL8761BU chosen by hand, a Realtek and a MediaTek adapter (issue #86)
        let s = CoreSettings { bt_chip_pid: 0x8771, ..CoreSettings::default() };
        assert_eq!(adapter_chip(ChipVendor::Realtek, Some(0x8771), &s), (0x8771, ""));
        assert_eq!(adapter_chip(ChipVendor::MediaTek, None, &s), (0, ""));
        assert_eq!(adapter_chip(ChipVendor::Intel, None, &s), (0, ""));
        // A chip known only by its files, on an adapter not recognised as another vendor's
        let stem = CoreSettings { bt_chip_fw_stem: "rtl8723bu".into(), ..s.clone() };
        assert_eq!(adapter_chip(ChipVendor::Unknown, None, &stem).1, "rtl8723bu");
        assert_eq!(adapter_chip(ChipVendor::Qualcomm, None, &stem), (0, ""));
    }

    /// This PC's adapters as the Adapters screen lists them, with nothing
    /// remembered yet (a fresh config folder; read-only otherwise): cargo
    /// test -p a2dpwb-core-exe list_adapter_infos -- --ignored --nocapture
    #[test]
    #[ignore = "reads this PC's USB devices and Bluetooth radios"]
    fn list_adapter_infos() {
        let dir = std::env::temp_dir().join(format!("a2dpwb-adapter-infos-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for a in adapter_infos(&dir, &CoreSettings::default()) {
            println!(
                "{} {:04X}:{:04X} {:?} ({}) address {:?} version {:?} name {:?}",
                a.instance_id, a.vid, a.pid, a.driver, a.driver_service, a.address, a.bt_version, a.windows_name
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn firmware_fixed_and_invalid_files_are_noticed() {
        let missing = rtl("a", Some(false), None);
        let page = rtl("a", Some(false), Some("rtl8761bu_fw.bin"));
        let ok = rtl("a", Some(true), None);
        // Placed, or a web page replaced by the file: the stack restarts
        assert!(firmware_change(std::slice::from_ref(&missing), std::slice::from_ref(&ok)).fixed);
        assert!(firmware_change(std::slice::from_ref(&page), std::slice::from_ref(&ok)).fixed);
        // Present all along, another adapter, or a newly plugged one
        assert!(!firmware_change(std::slice::from_ref(&ok), std::slice::from_ref(&ok)).fixed);
        assert!(!firmware_change(&[rtl("b", Some(false), None)], std::slice::from_ref(&ok)).fixed);
        assert!(!firmware_change(&[], std::slice::from_ref(&ok)).fixed);
        // An invalid file is told once, not at every refresh
        let c = firmware_change(std::slice::from_ref(&missing), std::slice::from_ref(&page));
        assert_eq!(c.invalid, [("RTL8761BU".to_string(), "rtl8761bu_fw.bin".to_string())]);
        assert!(!c.fixed);
        assert!(firmware_change(std::slice::from_ref(&page), std::slice::from_ref(&page)).invalid.is_empty());
        assert_eq!(firmware_change(&[], std::slice::from_ref(&page)).invalid.len(), 1);
    }

    #[test]
    fn the_tray_tooltip_names_the_device_and_the_codec() {
        let ldac = tray_codec_line("LDAC", 990, 96000, 24, true);
        assert_eq!(ldac, "LDAC 990 kbps | 96 kHz | 24-bit | ABR");
        assert_eq!(tray_codec_line("SBC", 328, 44100, 16, false), "SBC 328 kbps | 44.1 kHz | 16-bit");
        assert_eq!(tray_codec_line("LC3", 320, 48000, 0, false), "LC3 320 kbps | 48 kHz");
        assert_eq!(
            tray_tooltip("A2DP Windows Bridge - Connected", Some("WH-1000XM4"), &[ldac.clone(), "Battery 80%".into()]),
            "A2DP Windows Bridge - Connected (WH-1000XM4)\nLDAC 990 kbps | 96 kHz | 24-bit | ABR\nBattery 80%"
        );
        assert_eq!(
            tray_tooltip("A2DP Windows Bridge - Connected", None, &[ldac]),
            "A2DP Windows Bridge - Connected\nLDAC 990 kbps | 96 kHz | 24-bit | ABR"
        );
        assert_eq!(tray_tooltip("A2DP Windows Bridge - Connected", Some(""), &[]), "A2DP Windows Bridge - Connected");
        assert_eq!(
            tray_tooltip("A2DP Windows Bridge - Connected", Some("AA:BB:CC:DD:EE:FF"), &[]),
            "A2DP Windows Bridge - Connected (AA:BB:CC:DD:EE:FF)"
        );
        // AAC's variable bit rate, as the UI calls it
        assert_eq!(tray_codec_line("AAC", 320, 48000, 16, true), "AAC 320 kbps | 48 kHz | 16-bit | VBR");
    }

    /// A stream as the core reports it once the codec is set up
    fn streaming_with(codec: &str, kbps: u32, rate: u32, bits: u32, abr: bool, battery: Option<u8>) -> StreamStatus {
        StreamStatus {
            state: StreamState::Streaming,
            message: Msg::new("status.connected"),
            info: Some(ipc::StreamInfo {
                codec: codec.into(),
                bitrate_kbps: kbps,
                sample_rate: rate,
                channels: 2,
                bits,
                source_sample_rate: rate,
                source_channels: 2,
                source_bits: 32,
                requested_codec: None,
                requested_in_use: false,
                abr,
                bitrate_capped: false,
                source: None,
                le: None,
                details: None,
            }),
            battery,
            ..StreamStatus::default()
        }
    }

    #[test]
    fn the_connected_balloon_shows_the_device_codec_and_battery() {
        let ldac = streaming_with("LDAC", 990, 96000, 24, true, Some(80));
        let b = connected_balloon("en", Some("WH-1000XM4"), &ldac).unwrap();
        assert_eq!(b.title, "WH-1000XM4 connected");
        assert_eq!(b.text, "Battery 80%\nLDAC\n990 kbps | 96 kHz | 24-bit | ABR");
        assert_eq!(b.url, None);

        // Battery not known yet; an address when the device has no name
        let aptx = streaming_with("aptX Adaptive", 420, 48000, 24, true, None);
        let b = connected_balloon("en", Some("C0:11:22:33:44:55"), &aptx).unwrap();
        assert_eq!(b.title, "C0:11:22:33:44:55 connected");
        assert_eq!(b.text, "aptX Adaptive\n420 kbps | 48 kHz | 24-bit | ABR");

        // Another codec than the one asked for: the reason, as the UI's note
        let mut aac = streaming_with("AAC", 320, 44100, 16, true, Some(55));
        aac.info.as_mut().unwrap().requested_codec = Some("LDAC".into());
        let b = connected_balloon("en", None, &aac).unwrap();
        assert_eq!(b.title, "Connected");
        assert_eq!(b.text, "Battery 55%\nAAC\n320 kbps | 44.1 kHz | 16-bit | VBR\n(LDAC is unsupported)");
        aac.info.as_mut().unwrap().requested_in_use = true;
        assert!(connected_balloon("en", None, &aac).unwrap().text.contains("(LDAC is in use)"));

        // Translated; nothing before the codec is set up
        let b = connected_balloon("ja", Some("WH-1000XM4"), &ldac).unwrap();
        assert_eq!(b.text, "バッテリー 80%\nLDAC\n990 kbps | 96 kHz | 24-bit | ABR");
        assert!(connected_balloon("en", Some("WH"), &StreamStatus::default()).is_none());
    }

    #[test]
    fn a_connection_is_announced_once_with_its_negotiated_codec() {
        let dir = std::env::temp_dir().join(format!("a2dpwb-state-announce-{}", std::process::id()));
        let (tx, _rx) = std::sync::mpsc::channel();
        let options = a2dpwb_core::ServiceOptions {
            config_dir: dir.clone(),
            hci_tcp: None,
            capture: bt_host::HciCapture::new(),
            afh: None,
            usb: Default::default(),
        };
        let shown = Arc::new(Mutex::new(Vec::<BalloonRequest>::new()));
        let sink = shown.clone();
        let state = CoreState::new(
            Arc::new(Service::new(options, tx)),
            Box::new(|_| {}),
            Box::new(|| {}),
            Box::new(move |b| sink.lock().unwrap().push(b)),
        );
        let event = |st| state.on_service_event(ServiceEvent::State(st, a2dpwb_core::Msg::new("status.ready")));
        let info = |kbps| {
            state.on_service_event(ServiceEvent::StreamInfo(a2dpwb_core::StreamInfo {
                codec: "LDAC".into(),
                bitrate_kbps: kbps,
                sample_rate: 96000,
                channels: 2,
                bits: 24,
                source_sample_rate: 96000,
                source_channels: 2,
                source_bits: 32,
                requested_codec: None,
                requested_in_use: false,
                abr: false,
                bitrate_capped: false,
                source: a2dpwb_core::SourceInfo { kind: "tone", name: String::new(), waiting: false },
                details: None,
            }))
        };
        let texts = || shown.lock().unwrap().iter().map(|b| b.text.clone()).collect::<Vec<_>>();
        // A first connection: Streaming, the negotiated codec, then a moment
        // for the battery
        event(CoreStreamState::Connecting);
        event(CoreStreamState::Streaming);
        assert!(texts().is_empty());
        info(660);
        assert!(texts().is_empty());
        state.on_service_event(ServiceEvent::Battery(80));
        assert_eq!(texts().len(), 1);
        assert!(texts()[0].ends_with(" 80%\nLDAC\n660 kbps | 96 kHz | 24-bit"));
        // A lost link reconnecting (its info again, then Streaming): quiet
        event(CoreStreamState::Reconnecting);
        info(660);
        event(CoreStreamState::Streaming);
        assert_eq!(texts().len(), 1);
        // The headphones off and back on: a connection again
        event(CoreStreamState::Idle);
        event(CoreStreamState::Connecting);
        info(990);
        assert_eq!(texts().len(), 1);
        event(CoreStreamState::Streaming);
        assert_eq!(texts().len(), 1);
        // The battery refused: nothing to wait for
        state.on_service_event(ServiceEvent::BatteryRefused);
        assert_eq!(texts().len(), 2);
        assert_eq!(texts()[1], "LDAC\n990 kbps | 96 kHz | 24-bit");
        // Started again for a codec chosen (restart_profile): quiet, and
        // the next connection is announced again (issue #86)
        state.quiet_connect.store(true, Ordering::Release);
        event(CoreStreamState::Idle);
        event(CoreStreamState::Connecting);
        info(660);
        event(CoreStreamState::Streaming);
        assert_eq!(texts().len(), 2);
        event(CoreStreamState::Idle);
        event(CoreStreamState::Connecting);
        event(CoreStreamState::Streaming);
        info(660);
        assert_eq!(texts().len(), 2);
        // No report: without the battery once the wait is over
        *state.battery_deadline.lock().unwrap() = Some(Instant::now());
        state.announce_connected();
        assert_eq!(texts().len(), 3);
        assert_eq!(texts()[2], "LDAC\n660 kbps | 96 kHz | 24-bit");
        // The setting off: nothing
        let mut s = state.service.settings();
        s.notify_connected = false;
        state.service.set_settings(s);
        event(CoreStreamState::Idle);
        event(CoreStreamState::Connecting);
        event(CoreStreamState::Streaming);
        info(990);
        assert_eq!(texts().len(), 3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_tray_menu_names_devices_not_profiles() {
        let saved = |name: &str, le_audio| Device {
            address: "aa:bb:cc:dd:ee:ff".into(),
            name: name.into(),
            saved: true,
            audio: true,
            rssi: None,
            codecs: Vec::new(),
            sbc: None,
            codec_caps: Vec::new(),
            le_audio,
            le_audio_set: false,
        };
        let core = CoreProfile {
            name: "EarFun Air Pro 4+ | aptX Adaptive".into(),
            device_address: "AA:BB:CC:DD:EE:FF".into(),
            device_name: "EarFun".into(),
            ..CoreProfile::default()
        };
        let p = to_ipc_profile(&core, 679);
        // The saved device's name (renamed since), else the profile's, else the address
        assert_eq!(
            device_name(&[saved("Air Pro 4+", true), saved("Air Pro 4+ (renamed)", false)], &p),
            "Air Pro 4+ (renamed)"
        );
        assert_eq!(device_name(&[], &p), "EarFun");
        let unnamed = to_ipc_profile(&CoreProfile { device_name: String::new(), ..core }, 679);
        assert_eq!(device_name(&[], &unnamed), "AA:BB:CC:DD:EE:FF");
        assert_eq!(tray_label("Air Pro 4+", true, "LE Audio"), "Air Pro 4+ (LE Audio)");
        assert_eq!(tray_label("Air Pro 4+", false, "LE Audio"), "Air Pro 4+");
    }

    #[test]
    fn saving_the_profile_in_use_restarts_it() {
        let status = |state, profile: Option<&str>| StreamStatus {
            state,
            profile: profile.map(str::to_owned),
            ..StreamStatus::default()
        };
        for state in [StreamState::Connecting, StreamState::Streaming, StreamState::Reconnecting] {
            assert!(restart_after_save(&status(state, Some("WH LDAC")), "WH LDAC"));
            assert!(!restart_after_save(&status(state, Some("WH LDAC")), "Buds AAC"));
            // A custom stream (receiver test, test tone) has no profile
            assert!(!restart_after_save(&status(state, None), "WH LDAC"));
        }
        for state in [StreamState::Idle, StreamState::Error] {
            assert!(!restart_after_save(&status(state, Some("WH LDAC")), "WH LDAC"));
        }
    }

    #[test]
    fn another_source_changes_in_place_only_alone_and_while_streaming() {
        let before = CoreProfile { name: "WH".into(), ..CoreProfile::default() };
        let app = CoreProfile { capture_mode: "app".into(), app_exe: "Spotify.exe".into(), ..before.clone() };
        let stream = |state| StreamStatus { state, profile: Some("WH".into()), ..StreamStatus::default() };
        let streaming = stream(StreamState::Streaming);
        assert!(source_changes_in_place(&streaming, Some(&before), &app));
        // Saved with something else, or nothing that changes the source
        let codec = CoreProfile { codec: "aac".into(), ..app.clone() };
        assert!(!source_changes_in_place(&streaming, Some(&before), &codec));
        let names_only = CoreProfile { app_exe: "Spotify.exe".into(), ..before.clone() };
        assert!(!source_changes_in_place(&streaming, Some(&before), &names_only));
        // Still connecting (a restart may be on its way), LE Audio, a new profile
        for state in [StreamState::Connecting, StreamState::Reconnecting] {
            assert!(!source_changes_in_place(&stream(state), Some(&before), &app));
        }
        let le = CoreProfile { le_audio: true, ..before.clone() };
        assert!(!source_changes_in_place(&streaming, Some(&le), &CoreProfile { le_audio: true, ..app.clone() }));
        assert!(!source_changes_in_place(&streaming, None, &app));
    }

    #[test]
    fn a_new_codec_order_restarts_a_profile_on_auto_or_fallen_back() {
        let profile = |codec: &str| CoreProfile { name: "WH".into(), codec: codec.into(), ..CoreProfile::default() };
        let streaming = |requested: Option<&str>| StreamStatus {
            state: StreamState::Streaming,
            profile: Some("WH".into()),
            info: Some(ipc::StreamInfo {
                codec: "AAC".into(),
                bitrate_kbps: 0,
                sample_rate: 48000,
                channels: 2,
                bits: 16,
                source_sample_rate: 48000,
                source_channels: 2,
                source_bits: 32,
                requested_codec: requested.map(str::to_owned),
                requested_in_use: false,
                abr: false,
                bitrate_capped: false,
                source: None,
                le: None,
                details: None,
            }),
            ..StreamStatus::default()
        };
        assert!(restart_for_order(&streaming(None), &profile("auto")));
        assert!(restart_for_order(&streaming(Some("aptX HD")), &profile("aptxhd")));
        // Its own codec in use, LE Audio
        assert!(!restart_for_order(&streaming(None), &profile("aac")));
        assert!(!restart_for_order(&streaming(None), &CoreProfile { le_audio: true, ..profile("auto") }));
    }

    #[test]
    fn auto_select_on_or_off_restarts_a_profile_that_chose_its_codec() {
        let profile = |codec: &str| CoreProfile { name: "WH".into(), codec: codec.into(), ..CoreProfile::default() };
        let streaming =
            StreamStatus { state: StreamState::Streaming, profile: Some("WH".into()), ..StreamStatus::default() };
        assert!(restart_for_auto(&streaming, &profile("aac")));
        // Auto either way, another profile, nothing running, LE Audio
        assert!(!restart_for_auto(&streaming, &profile("auto")));
        // Auto in its own order: Auto-select takes the default order
        assert!(restart_for_auto(&streaming, &CoreProfile { codec_order: vec!["sbc".into()], ..profile("auto") }));
        assert!(!restart_for_auto(&streaming, &CoreProfile { name: "Buds".into(), ..profile("aac") }));
        assert!(!restart_for_auto(&StreamStatus::default(), &profile("aac")));
        assert!(!restart_for_auto(&streaming, &CoreProfile { le_audio: true, ..profile("aac") }));
    }

    #[test]
    fn switching_profiles_keeps_the_new_profile() {
        let dir = std::env::temp_dir().join(format!("a2dpwb-state-switch-{}", std::process::id()));
        let (tx, _rx) = std::sync::mpsc::channel();
        let options = a2dpwb_core::ServiceOptions {
            config_dir: dir.clone(),
            hci_tcp: None,
            capture: bt_host::HciCapture::new(),
            afh: None,
            usb: Default::default(),
        };
        let state =
            CoreState::new(Arc::new(Service::new(options, tx)), Box::new(|_| {}), Box::new(|| {}), Box::new(|_| {}));
        let event = |st| state.on_service_event(ServiceEvent::State(st, a2dpwb_core::Msg::new("status.ready")));
        // stream.start of B while A streams: the core says Connecting (B),
        // then A's session ends (Idle) and B's starts
        *state.starting_profile.lock().unwrap() = Some("B".into());
        event(CoreStreamState::Idle);
        assert_eq!(state.snapshot().1.stream.profile, None);
        event(CoreStreamState::Connecting);
        assert_eq!(state.snapshot().1.stream.profile.as_deref(), Some("B"));
        event(CoreStreamState::Streaming);
        assert_eq!(state.snapshot().1.stream.profile.as_deref(), Some("B"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SBC at 48 kHz under ABR (bitpool 19..53), 1011-byte payloads
    fn sbc_abr_info() -> ipc::StreamInfo {
        serde_json::from_value(serde_json::json!({
            "codec": "SBC", "bitrate_kbps": 357, "sample_rate": 48000, "channels": 2, "bits": 16,
            "source_sample_rate": 48000, "source_channels": 2, "source_bits": 32,
            "requested_codec": null, "requested_in_use": false, "abr": true,
            "details": {
                "mtu": 1011, "rtp": true, "media_header": 1, "frame_samples": 128,
                "frame_bytes": 119, "frames_per_packet": 8,
                "sbc": {
                    "channel_mode": "joint_stereo", "blocks": 16, "subbands": 8, "allocation": "loudness",
                    "bitpool": 53, "config_bitpool": [2, 53], "device": null, "abr_bitpool": [19, 53]
                },
                "aac": null
            }
        }))
        .unwrap()
    }

    #[test]
    fn live_codec_values_go_into_the_stream_info() {
        let info = sbc_abr_info();
        let live = bt_host::LiveCodec { bitrate_kbps: 249, bitpool: 35, frame_bytes: 83 };
        let next = live_info(&info, live).unwrap();
        assert_eq!(next.bitrate_kbps, 249);
        let d = next.details.as_deref().unwrap();
        assert_eq!((d.frame_bytes, d.frames_per_packet), (Some(83), Some(12)));
        let s = d.sbc.as_ref().unwrap();
        assert_eq!((s.bitpool, s.abr_bitpool, s.config_bitpool), (35, Some((19, 53)), Some((2, 53))));
        // Nothing changed: nothing to send
        assert_eq!(live_info(&next, live), None);
        // At most 15 frames (the 4-bit count)
        let small = bt_host::LiveCodec { bitrate_kbps: 100, bitpool: 12, frame_bytes: 40 };
        assert_eq!(live_info(&info, small).unwrap().details.unwrap().frames_per_packet, Some(15));
        // Not under ABR, or LE Audio: left alone
        let fixed = ipc::StreamInfo { abr: false, ..info.clone() };
        assert_eq!(live_info(&fixed, live), None);
        // LDAC under ABR: the bit rate only
        let ldac = ipc::StreamInfo { codec: "LDAC".into(), details: None, ..info };
        assert_eq!(live_info(&ldac, live).map(|i| (i.bitrate_kbps, i.details)), Some((249, None)));
    }

    #[test]
    fn live_codec_updates_only_a_streaming_snapshot() {
        let dir = std::env::temp_dir().join(format!("a2dpwb-state-live-{}", std::process::id()));
        let (tx, _rx) = std::sync::mpsc::channel();
        let options = a2dpwb_core::ServiceOptions {
            config_dir: dir.clone(),
            hci_tcp: None,
            capture: bt_host::HciCapture::new(),
            afh: None,
            usb: Default::default(),
        };
        let state =
            CoreState::new(Arc::new(Service::new(options, tx)), Box::new(|_| {}), Box::new(|| {}), Box::new(|_| {}));
        let live = bt_host::LiveCodec { bitrate_kbps: 249, bitpool: 35, frame_bytes: 83 };
        let mut events = state.subscribe();
        let set = |st: StreamState| {
            state.publish(Event::Stream(StreamStatus { state: st, info: Some(sbc_abr_info()), ..Default::default() }))
        };
        set(StreamState::Reconnecting);
        let _ = events.try_recv();
        state.apply_live_codec(Some(live));
        assert!(events.try_recv().is_err());
        set(StreamState::Streaming);
        let _ = events.try_recv();
        state.apply_live_codec(None);
        assert!(events.try_recv().is_err());
        let rev = state.snapshot().0;
        state.apply_live_codec(Some(live));
        let (r, e) = events.try_recv().unwrap();
        assert_eq!(r, rev + 1);
        let Event::Stream(st) = &*e else { panic!("{e:?}") };
        assert_eq!(st.info.as_ref().map(|i| i.bitrate_kbps), Some(249));
        assert_eq!(state.snapshot().1.stream.info.unwrap().details.unwrap().sbc.unwrap().bitpool, 35);
        // The same again: no event
        state.apply_live_codec(Some(live));
        assert!(events.try_recv().is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn custom_streams_are_built_as_profiles() {
        let r = custom_request(&custom("tone"), 679).unwrap();
        assert_eq!(r.device, [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
        assert_eq!(r.codec, Some(codec::CodecKind::Ldac));
        assert_eq!(r.quality, codec::Quality::Standard);
        assert_eq!(r.source, SourceRequest::TestTone);
        assert_eq!(r.volume_sync, a2dpwb_core::VolumeSync::Off);
        assert!(!r.codec_fallback);
        assert_eq!(r.max_media_payload, 679);
        assert_eq!((r.sample_rate, r.bit_depth), (None, None));

        let mut c = custom("app");
        c.source_id = Some("Spotify.exe".into());
        c.sample_rate = Some(96000);
        c.bit_depth = Some(24);
        c.max_media_payload = Some(1023);
        c.codec = "auto".into();
        let r = custom_request(&c, 679).unwrap();
        assert_eq!(r.source, SourceRequest::Application { exe: "Spotify.exe".into() });
        assert_eq!((r.sample_rate, r.bit_depth, r.codec, r.max_media_payload), (Some(96000), Some(24), None, 1023));

        let mut v = custom("virtual");
        v.source_id = Some("{dev}".into());
        v.source_name = Some("CABLE Input".into());
        let r = custom_request(&v, 679).unwrap();
        assert_eq!(
            r.source,
            SourceRequest::VirtualDevice { id: "{dev}".into(), name: "CABLE Input".into(), auto_switch: false }
        );
        assert_eq!(custom_request(&custom("loopback"), 679).unwrap().source, SourceRequest::SystemLoopback);
    }

    #[test]
    fn bad_custom_streams_are_refused() {
        assert_eq!(custom_request(&custom("radio"), 679).unwrap_err().code, "stream.bad_source");
        let mut c = custom("tone");
        c.codec = "mp3".into();
        assert_eq!(custom_request(&c, 679).unwrap_err().code, "stream.bad_codec");
        c.codec = "sbc".into();
        c.device = "not an address".into();
        assert_eq!(custom_request(&c, 679).unwrap_err().code, "profile.invalid_address");
    }

    #[test]
    fn receiver_types_convert_field_for_field() {
        let line = concat!(
            r#"{"type":"stats","version":1,"connected":true,"streaming":true,"device":"AA:BB:CC:DD:EE:FF","#,
            r#""buffer_target_ms":200,"mtu":1005,"codecs":["SBC","LDAC"],"rssi":-3,"tx_power":4,"rssi_note":"","#,
            r#""afh_channels":40,"stream":{"id":2,"codec":"LDAC","config":"LDAC 96 kHz","decoding":true,"#,
            r#""packets":100,"bytes":100000,"frames":400,"lost":1,"late":2,"ts_errors":3,"frame_errors":4,"#,
            r#""decode_errors":5,"underruns":6,"overflows":7,"pauses":8,"idle":false,"audio_ms":1000.5,"#,
            r#""underrun_ms":12.5,"overflow_ms":1.5,"max_gap_ms":30.0,"jitter_ms":2.5,"buffer_ms":180.0,"#,
            r#""interval":{"packets":120,"bytes":120000,"max_gap_ms":20.0}}}"#
        );
        let core = receiver::parse_stats(line).expect("a stats line");
        let ipc: ipc::ReceiverStats = convert(&core).expect("converts");
        // Every field arrives: both give the same JSON
        assert_eq!(json(&ipc), json(&core));
        assert_eq!(ipc.stream.as_ref().map(|s| (s.decode_errors, s.interval_bytes)), Some((5, 120000)));

        let info = receiver::ReceiverInfo {
            ip: "192.168.1.5".into(),
            port: 51201,
            host: "sink".into(),
            bt_address: "00:11:22:33:44:55".into(),
            bt_name: "hci0".into(),
            codecs: vec!["SBC".into()],
            mtu: 895,
            buffer_ms: 200,
        };
        let i: ipc::ReceiverInfo = convert(&info).expect("converts");
        assert_eq!(json(&i), json(&info));
    }

    #[test]
    fn link_totals_since_the_stream_started() {
        let s = LinkStats::default();
        let r = Ordering::Relaxed;
        s.packets_sent.store(10, r);
        s.queue_drops.store(1, r);
        s.send_errors.store(2, r);
        s.start_packets.store(4, r);
        s.start_dropped.store(1, r);
        s.stream_starts.store(1, r);
        let (total, start, starts) = link_totals(&s);
        assert_eq!(starts, 1);
        assert_eq!(total.dropped, 3);
        let since = total.since(&start);
        assert_eq!((since.packets, since.dropped), (6, 2));
    }

    #[test]
    fn autostart_follows_both_settings() {
        let off = CoreSettings::default();
        let on = CoreSettings { start_with_windows: true, ..CoreSettings::default() };
        let on_min = CoreSettings { start_minimized: true, ..on.clone() };
        assert!(autostart_changed(&off, &on));
        assert!(autostart_changed(&on, &on_min));
        assert!(!autostart_changed(&on, &on));
        // Minimized alone does not matter while autostart is off
        assert!(!autostart_changed(&off, &CoreSettings { start_minimized: true, ..off.clone() }));
        assert_eq!(autostart_args(true), "--background");
        assert_eq!(autostart_args(false), "");
    }

    #[test]
    fn autostart_fix_takes_over_v1_and_leaves_other_programs_alone() {
        use AutostartFix::{Keep, Remove, Write};
        let present = |p: &std::path::Path| {
            [r"C:\A2DP-Windows-Bridge\build\dist\A2DPWB.exe", r"C:\Program Files\A2DPWB\a2dpwb-core.exe"]
                .contains(&p.to_str().unwrap())
        };
        let fix = |current, enabled| autostart_fix(current, enabled, true, present);
        // Missing or empty: written when on
        assert_eq!(fix(None, true), Write);
        assert_eq!(fix(Some(""), true), Write);
        assert_eq!(fix(None, false), Keep);
        // A copy of the core that is gone (moved or uninstalled): written
        assert_eq!(fix(Some(r#""D:\old\a2dpwb-core.exe" --background"#), true), Write);
        assert_eq!(fix(Some(r#""D:\old\A2DPWB-Core.EXE""#), true), Write);
        // Another existing copy of v2: kept (a test build must not take it)
        assert_eq!(fix(Some(r#""C:\Program Files\A2DPWB\a2dpwb-core.exe""#), true), Keep);
        assert_eq!(fix(Some(r#""C:\Program Files\A2DPWB\a2dpwb-core.exe""#), false), Keep);
        // v1, existing or not: taken over as by an update
        let v1 = r#""C:\A2DP-Windows-Bridge\build\dist\A2DPWB.exe" --minimized"#;
        assert_eq!(fix(Some(v1), true), Write);
        assert_eq!(fix(Some(v1), false), Remove);
        assert_eq!(fix(Some(r#""E:\gone\A2DPWB.exe""#), true), Write);
        assert_eq!(fix(Some(r"E:\v1\a2dpwb.EXE --minimized"), false), Remove);
        // ... but not by a development instance
        assert_eq!(autostart_fix(Some(v1), true, false, present), Keep);
        assert_eq!(autostart_fix(Some(v1), false, false, present), Keep);
        // Any other program: kept
        assert_eq!(fix(Some(r"C:\Tools\other.exe /x"), true), Keep);
        assert_eq!(fix(Some(r"C:\Tools\other.exe /x"), false), Keep);
    }

    #[test]
    fn chip_changes_need_new_firmware() {
        let a = CoreSettings::default();
        assert!(!chip_changed(&a, &a.clone()));
        assert!(chip_changed(&a, &CoreSettings { bt_chip_pid: 0x8771, ..a.clone() }));
        assert!(chip_changed(&a, &CoreSettings { bt_chip_fw_stem: "rtl8761bu".into(), ..a.clone() }));
    }

    #[test]
    fn vendors_get_their_notes() {
        assert_eq!(vendor_note(ChipVendor::Intel).unwrap().key, "firmware.vendor_intel");
        assert_eq!(vendor_note(ChipVendor::Qualcomm).unwrap().key, "firmware.vendor_unsupported");
        assert!(vendor_note(ChipVendor::Realtek).is_none());
    }

    #[test]
    fn devices_carry_their_codecs() {
        let dir = std::env::temp_dir().join(format!("a2dpwb-state-dev-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        devices::save(&dir, "AA:BB:CC:DD:EE:FF", "Phones").unwrap();
        let scanned = vec![Device {
            address: "11:22:33:44:55:66".into(),
            name: "Speaker".into(),
            saved: false,
            audio: true,
            rssi: Some(-60),
            codecs: Vec::new(),
            sbc: None,
            codec_caps: Vec::new(),
            le_audio: false,
            le_audio_set: false,
        }];
        let mut codecs = BTreeMap::new();
        let sbc = a2dpwb_core::SbcDeviceCaps {
            sample_rates: vec![44100, 48000],
            channel_modes: vec!["joint_stereo".into()],
            blocks: vec![8, 16],
            subbands: vec![8],
            allocations: vec!["loudness".into()],
            min_bitpool: 2,
            max_bitpool: 53,
        };
        codecs.insert(
            "AA:BB:CC:DD:EE:FF".to_string(),
            a2dpwb_core::KnownCodecs {
                keys: vec!["sbc".to_string(), "ldac".to_string()],
                sbc: Some(sbc),
                caps: vec![a2dpwb_core::CodecDeviceCaps {
                    codec: "ldac".into(),
                    sample_rates: vec![44100, 48000],
                    ..Default::default()
                }],
            },
        );
        let list = saved_devices(&dir, &scanned, &codecs);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].codecs, ["sbc", "ldac"]);
        // What its SBC endpoint offered, for the profile editor
        assert_eq!(list[0].sbc.as_ref().map(|s| s.blocks.clone()), Some(vec![8, 16]));
        // And what each codec lists, for the codec settings (issue #86)
        assert_eq!(list[0].codec_caps.len(), 1);
        assert_eq!(
            (list[0].codec_caps[0].codec.as_str(), list[0].codec_caps[0].sample_rates.clone()),
            ("ldac", vec![44100, 48000])
        );
        assert!(list[1].sbc.is_none());
        assert!(list[0].saved && !list[1].saved && list[1].codecs.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn battery_low_balloon_once_per_connection() {
        let mut warned = false;
        let alerts: Vec<bool> = [50, 30, 20, 19, 15, 10, 22, 18, 25, 19, 5]
            .into_iter()
            .map(|v| battery_low_alert(&mut warned, v, 20))
            .collect();
        // Below 20 once; back at 22 is not enough, 25 rearms it
        assert_eq!(alerts, [false, false, false, true, false, false, false, false, false, true, false]);
        // A connection that starts low warns at its first report
        let mut warned = false;
        assert!(battery_low_alert(&mut warned, 12, 20));
        assert!(!battery_low_alert(&mut warned, 11, 20));
        // Below 5 %: 10 is not low; 0 never warns
        let mut warned = false;
        assert!(!battery_low_alert(&mut warned, 10, 5));
        assert!(battery_low_alert(&mut warned, 4, 5));
        let mut warned = false;
        assert!(!battery_low_alert(&mut warned, 0, 0));
    }

    #[test]
    fn codec_order_is_stored_with_known_keys_once() {
        let ui =
            to_ipc_profile(&CoreProfile { device_address: "C0:11:22:33:44:55".into(), ..CoreProfile::default() }, 679);
        assert!(ui.codec_order.is_empty());
        let order = ["SBCXQ", "nope", "faststream", "sbcxq", "aac"].map(String::from).to_vec();
        let merged = merge_profile(None, &Profile { codec_order: order, ..ui });
        assert_eq!(merged.codec_order, ["sbcxq", "faststream", "aac"]);
        assert_eq!(to_ipc_profile(&merged, 679).codec_order, merged.codec_order);
    }

    #[test]
    fn le_audio_profiles_and_settings_map_both_ways() {
        let core = CoreProfile {
            name: "Buds LE Audio".into(),
            device_address: "C0:11:22:33:44:55".into(),
            le_audio: true,
            ..CoreProfile::default()
        };
        let ui = to_ipc_profile(&core, 679);
        assert!(ui.le_audio);
        assert!(merge_profile(None, &ui).le_audio);
        // The UI turns it off: the stored profile follows
        let classic = Profile { le_audio: false, ..ui };
        assert!(!merge_profile(Some(&core), &classic).le_audio);

        let base = CoreSettings::default();
        let ui = to_ipc_settings(&base);
        // LE Audio on by default, GMAP low latency off
        assert!(ui.le_audio_experimental && !ui.le_audio_low_latency);
        let off = Settings { le_audio_experimental: false, le_audio_low_latency: true, ..ui };
        let merged = merge_settings(&base, &off);
        assert!(!merged.le_audio_experimental && merged.le_audio_low_latency);
        assert_eq!(to_ipc_settings(&merged), off);

        // Windows Volume Sync for every device: on by default
        assert!(to_ipc_settings(&base).volume_sync);
        let no_sync = Settings { volume_sync: false, ..off };
        let merged = merge_settings(&base, &no_sync);
        assert!(!merged.volume_sync);
        assert_eq!(to_ipc_settings(&merged), no_sync);
    }

    #[test]
    fn volume_sync_for_every_device_overrides_the_profiles() {
        let dev = || VolumeSync::Device("{0.0.0.00000000}.{x}".into());
        // Off for every device: no profile syncs
        for p in [VolumeSync::Auto, VolumeSync::Off, dev()] {
            assert_eq!(effective_volume_sync(false, p), VolumeSync::Off);
        }
        // On: neither a profile's old "off" nor a device it followed counts
        for p in [VolumeSync::Auto, VolumeSync::Off, dev()] {
            assert_eq!(effective_volume_sync(true, p), VolumeSync::Auto);
        }
    }

    #[test]
    fn le_audio_devices_are_their_own_entries() {
        let dir = std::env::temp_dir().join(format!("a2dpwb-state-le-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // A dual-mode headset saved as both kinds
        devices::save(&dir, "AA:BB:CC:DD:EE:FF", "Phones").unwrap();
        devices::save_le(&dir, "AA:BB:CC:DD:EE:FF", "Phones LE").unwrap();
        let scan = [
            ScannedDevice {
                address: "AA:BB:CC:DD:EE:FF".into(),
                name: "Phones".into(),
                rssi: -50,
                class_of_device: 0x240404,
                audio: true,
                le_audio: false,
                le_audio_set: false,
            },
            ScannedDevice {
                address: "AA:BB:CC:DD:EE:FF".into(),
                name: "Phones".into(),
                rssi: -55,
                class_of_device: 0,
                audio: true,
                le_audio: true,
                le_audio_set: false,
            },
            ScannedDevice {
                address: "C0:11:22:33:44:55".into(),
                name: "Earbuds".into(),
                rssi: -60,
                class_of_device: 0,
                audio: true,
                le_audio: true,
                le_audio_set: true,
            },
        ];
        let scanned: Vec<Device> = scan.iter().map(scanned_device).collect();
        assert!(scanned[2].le_audio && scanned[2].le_audio_set && !scanned[2].saved);
        assert_eq!(scanned[2].rssi, Some(-60));
        let mut codecs = BTreeMap::new();
        codecs.insert(
            "AA:BB:CC:DD:EE:FF".to_string(),
            a2dpwb_core::KnownCodecs { keys: vec!["sbc".to_string()], ..Default::default() },
        );
        let list = saved_devices(&dir, &scanned, &codecs);
        let kinds: Vec<(&str, bool, bool)> = list.iter().map(|d| (d.name.as_str(), d.le_audio, d.saved)).collect();
        // Merged by address AND kind: each saved entry got its own scan result
        assert_eq!(kinds, [("Phones", false, true), ("Phones LE", true, true), ("Earbuds", true, false)]);
        assert_eq!((list[0].rssi, list[1].rssi), (Some(-50), Some(-55)));
        assert!(list[2].le_audio_set);
        // Codecs are the classic entry's
        assert_eq!(list[0].codecs, ["sbc"]);
        assert!(list[1].codecs.is_empty());
        // Forgetting the LE Audio entry keeps the classic one
        devices::remove_le(&dir, "AA:BB:CC:DD:EE:FF").unwrap();
        let list = saved_devices(&dir, &[], &codecs);
        assert_eq!(list.len(), 1);
        assert!(!list[0].le_audio);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn le_stream_details_reach_the_ui() {
        let d = LeStreamDetails {
            preset: "48_3_gr".into(),
            frame_us: 7500,
            octets: 75,
            cis: 2,
            devices: 2,
            low_latency: true,
            summary: "LC3 48 kHz 7.5 ms 75 octets (80 kbps/ch), 2 ch over 2 CIS, 48_3_gr".into(),
        };
        let le = to_ipc_le_stream(&d);
        assert_eq!((le.preset.as_str(), le.frame_us, le.octets, le.cis, le.devices), ("48_3_gr", 7500, 75, 2, 2));
        assert!(le.low_latency);
        assert_eq!(le.summary, d.summary);
        assert_eq!(LE_CODEC, "LC3");
        assert_eq!(a2dpwb_core::Msg::new(LE_SCANNING_KEY).key, "status.le_scanning");
    }

    #[test]
    fn capture_names_use_the_local_time() {
        let s = local_stamp();
        assert_eq!(s.len(), 15);
        assert_eq!(&s[8..9], "_");
        assert!(s.bytes().enumerate().all(|(i, b)| i == 8 || b.is_ascii_digit()));
    }
}
