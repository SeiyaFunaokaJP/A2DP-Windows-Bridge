//! bluecaster-core.exe: the resident BlueCaster process (core service, tray, IPC
//! for the UI). `--cli` runs the command line instead (same core, no tray
//! and no IPC). A GUI-subsystem program: the resident process opens no
//! console window; `--cli` attaches to the console it was started from.
//! `--quit` asks a running core to exit (the installer uses it).

#![forbid(unsafe_code)]
#![windows_subsystem = "windows"]

mod bundle;
mod cli;
mod crashes;
mod driver;
mod logging;
mod quit;
mod resident;
mod rfcomm;
mod server;
mod state;
mod zip;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--quit") {
        // Neither logs nor the settings folder: only the running core's
        win_sys::console::attach_parent();
        std::process::exit(quit::run());
    }
    let cli_mode = args.iter().any(|a| a == "--cli");
    if cli_mode {
        win_sys::console::attach_parent();
    }
    // The command line logs to the console only; the resident process to
    // daily files in the settings folder
    let config = bluecaster_core::paths::config_dir();
    // Developer mode's logging from the first line (the settings are read
    // again, with their log lines, when the service starts)
    let dev = if cli_mode { Default::default() } else { bluecaster_core::settings::Settings::load(&config).log() };
    let log = logging::init(&logging::LogOptions {
        dir: (!cli_mode).then(|| bluecaster_core::paths::log_dir(&config)),
        console: cli_mode,
        crash_dir: Some(bluecaster_core::paths::crash_dir(&config)),
        dev,
    });
    let code = if cli_mode { cli::run(&args) } else { resident::run(&args) };
    // process::exit skips destructors: flush the log first
    drop(log);
    std::process::exit(code);
}
fn core_state(s: ipc::StreamState) -> bluecaster_core::StreamState {
    use bluecaster_core::StreamState as S;
    match s {
        ipc::StreamState::Idle => S::Idle,
        ipc::StreamState::Connecting => S::Connecting,
        ipc::StreamState::Streaming => S::Streaming,
        ipc::StreamState::Reconnecting => S::Reconnecting,
        ipc::StreamState::Error => S::Error,
    }
}

/// The connection flow for the diagnostics view (diag.flow)
pub(crate) fn flow_view(state: &state::CoreState) -> ipc::FlowView {
    let flow = logging::hub().flow();
    let now = state.with_snapshot(|s| core_state(s.stream.state));
    flow_view_of(&flow, now, state.rates())
}

fn flow_view_of(
    flow: &bluecaster_core::diag::ConnectionFlow,
    now: bluecaster_core::StreamState,
    rates: Option<bluecaster_core::diag::AudioRates>,
) -> ipc::FlowView {
    let sess = flow.session();
    let steps = flow
        .rows(Some(now), rates)
        .into_iter()
        .map(|r| ipc::FlowStepView {
            step: r.step.key().trim_start_matches("dbgc.step.").into(),
            status: r.status.key().trim_start_matches("dbgc.status.").into(),
            elapsed_ms: r.elapsed_ms,
            detail: r.detail,
            detail_msg: r.detail_msg.map(|m| ipc::Msg { key: m.key.into(), args: m.args }),
            seq: (r.seq != 0).then_some(r.seq),
            time_ms: r.time_ms,
        })
        .collect();
    ipc::FlowView {
        session: sess.id,
        first_seq: (sess.id != 0).then_some(sess.first_seq),
        start_ms: sess.start_ms,
        stop_ms: sess.stop_ms,
        reconnects: sess.reconnects,
        steps,
    }
}

/// The firmware line of the diagnostics: what each adapter needs and has
fn firmware_text(adapters: &[ipc::AdapterInfo]) -> Option<String> {
    let parts: Vec<String> = adapters
        .iter()
        .map(|a| {
            let files = a.firmware_files.join(", ");
            let status = match (a.firmware_present, &a.firmware_invalid) {
                (Some(true), _) => format!("present ({files})"),
                (Some(false), Some(bad)) => format!("INVALID: {bad} is not a valid file ({files})"),
                (Some(false), None) => format!("MISSING ({files})"),
                (None, _) => "none needed".to_string(),
            };
            format!("{}: {status}", a.display_name)
        })
        .collect();
    (!parts.is_empty()).then(|| parts.join("; "))
}

/// The codec details back in the core's types (the same fields)
fn core_codec_details(d: ipc::CodecDetails) -> bluecaster_core::CodecDetails {
    bluecaster_core::CodecDetails {
        mtu: d.mtu,
        rtp: d.rtp,
        media_header: d.media_header,
        frame_samples: d.frame_samples,
        frame_bytes: d.frame_bytes,
        frames_per_packet: d.frames_per_packet,
        sbc: d.sbc.map(|s| bluecaster_core::SbcDetails {
            channel_mode: s.channel_mode,
            blocks: s.blocks,
            subbands: s.subbands,
            allocation: s.allocation,
            bitpool: s.bitpool,
            config_bitpool: s.config_bitpool,
            abr_bitpool: s.abr_bitpool,
            device: s.device.map(|c| bluecaster_core::SbcDeviceCaps {
                sample_rates: c.sample_rates,
                channel_modes: c.channel_modes,
                blocks: c.blocks,
                subbands: c.subbands,
                allocations: c.allocations,
                min_bitpool: c.min_bitpool,
                max_bitpool: c.max_bitpool,
            }),
        }),
        aac: d.aac.map(|a| bluecaster_core::AacDetails {
            object_type: a.object_type,
            bit_rate: a.bit_rate,
            device: a.device.map(|c| bluecaster_core::AacDeviceCaps {
                object_types: c.object_types,
                sample_rates: c.sample_rates,
                channels: c.channels,
                max_bitrate: c.max_bitrate,
                vbr: c.vbr,
            }),
        }),
        device: d.device.map(|c| bluecaster_core::CodecDeviceCaps {
            codec: c.codec,
            sample_rates: c.sample_rates,
            bit_depths: c.bit_depths,
            max_bitrate: c.max_bitrate,
        }),
    }
}

/// The text of "copy diagnostic info"
pub(crate) fn diag_text(state: &state::CoreState, mask: bool) -> String {
    use bluecaster_core::diag;
    let (_, s) = state.snapshot();
    let hub = logging::hub();
    let flow = hub.flow();
    let lines = hub.recent();
    let log: Vec<_> = lines.iter().map(logging::entry).collect();
    let adapters: Vec<String> = bluecaster_core::adapter::adapters(false).iter().map(diag::adapter_line).collect();
    let settings = state.service.settings();
    let profile = s
        .active_profile
        .as_ref()
        .and_then(|n| bluecaster_core::profiles::load(&state.config_dir()).into_iter().find(|p| &p.name == n));
    let stream_state = core_state(s.stream.state);
    let info = s.stream.info.as_ref().map(|i| bluecaster_core::StreamInfo {
        codec: i.codec.clone(),
        bitrate_kbps: i.bitrate_kbps,
        sample_rate: i.sample_rate,
        channels: i.channels,
        bits: i.bits,
        source_sample_rate: i.source_sample_rate,
        source_channels: i.source_channels,
        source_bits: i.source_bits,
        requested_codec: i.requested_codec.clone(),
        requested_in_use: i.requested_in_use,
        abr: i.abr,
        bitrate_capped: i.bitrate_capped,
        source: core_source(i.source.as_ref()),
        details: i.details.clone().map(|d| Box::new(core_codec_details(*d))),
    });
    let firmware = firmware_text(&s.adapter.adapters);
    // LE Audio (experimental) of the running stack: stream, ISO counters,
    // devices with their TMAP / GMAP roles
    let le_audio = state.service.host().filter(|_| state.service.le_audio_active()).map(|h| {
        let stream = state.service.le_stream().map(|d| d.summary);
        diag::LeAudioDiag { stats: stream.is_some().then(|| h.le_stream_stats()), stream, devices: h.le_devices() }
    });
    let stats = state.service.stats();
    let capture = state.service.capture();
    let capture_path = capture.path();
    let adapter_addresses = adapter_addresses(&s.adapter.adapters);
    diag::diag_text(&diag::DiagInput {
        env: diag::Environment::current(),
        settings: &settings,
        state: stream_state,
        adapters: &adapters,
        firmware: firmware.as_deref(),
        profile: profile.as_ref(),
        stream: info.as_ref(),
        stats: Some(&stats),
        hci_capture: capture_path.as_deref().map(|p| (p, capture.bytes_written())),
        flow: &flow,
        rates: state.rates(),
        log: &log,
        clock: hub.clock(),
        mask,
        adapter_addresses: &adapter_addresses,
        le_audio: le_audio.as_ref(),
        logging: Some(&logging_line(&state.config_dir())),
    })
}

/// The adapters' own addresses, as remembered (`<adapter>` in masked text)
pub(crate) fn adapter_addresses(adapters: &[ipc::AdapterInfo]) -> Vec<String> {
    adapters.iter().filter_map(|a| a.address.clone()).collect()
}

/// "Logging" of the diagnostic info: verbose logging, and the crash
/// reports and flight recorder files a diagnostic bundle would include
fn logging_line(config: &std::path::Path) -> String {
    use bluecaster_core::paths;
    let count = |dir: std::path::PathBuf, ext: &str| {
        std::fs::read_dir(dir)
            .map_or(0, |d| d.flatten().filter(|e| e.path().extension().is_some_and(|x| x == ext)).count())
    };
    let verbose = match logging::verbose() {
        Some(v) => {
            format!("verbose ({:?}, {} min left)", v.level, v.until_ms.saturating_sub(logging::unix_ms()) / 60_000)
        }
        None => "normal".into(),
    };
    let always = match logging::always_verbose() {
        Some(l) => format!(", always {l:?} (developer mode)"),
        None => String::new(),
    };
    let capped = if logging::file_capped() { " (today's file is full: debug lines stopped)" } else { "" };
    format!(
        "{verbose}{always}{capped}; {} crash report(s); {} HCI flight recording(s)",
        count(paths::crash_dir(config), "txt"),
        count(paths::recorder_dir(config), "pklg")
    )
}

/// The protocol's source back to the core's (for diag_text)
fn core_source(s: Option<&ipc::SourceInfo>) -> bluecaster_core::SourceInfo {
    let kind = match s.map(|s| s.kind.as_str()) {
        Some("virtual") => "virtual",
        Some("app") => "app",
        Some("tone") => "tone",
        _ => "loopback",
    };
    bluecaster_core::SourceInfo {
        kind,
        name: s.map(|s| s.name.clone()).unwrap_or_default(),
        waiting: s.is_some_and(|s| s.waiting),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bluecaster_core::diag::{AudioRates, ConnectionFlow};
    use tracing::Level;

    fn streaming_flow() -> ConnectionFlow {
        let mut f = ConnectionFlow::new();
        let lines = [
            (100, "bluecaster_core::stream", "stream: device AA:BB:CC:DD:EE:FF codec None"),
            (200, "bt_host", "connecting to AA:BB:CC:DD:EE:FF"),
            (300, "bt_host::app", "signaling connection established (cid 0x0041)"),
            (400, "bluecaster_core::service", "state Streaming: Streaming"),
        ];
        for (t, target, m) in lines {
            f.feed(t, target, Level::INFO, m);
        }
        f
    }

    #[test]
    fn flow_steps_name_their_lines() {
        let v = flow_view_of(&streaming_flow(), bluecaster_core::StreamState::Streaming, None);
        assert_eq!((v.session, v.first_seq), (1, Some(1)));
        let connect = v.steps.iter().find(|s| s.step == "connect").unwrap();
        assert_eq!((connect.seq, connect.time_ms, connect.elapsed_ms), (Some(3), Some(300), Some(200)));
        let pending = v.steps.iter().find(|s| s.step == "encoder").unwrap();
        assert_eq!((pending.seq, pending.time_ms), (None, None));
        let empty = flow_view_of(&ConnectionFlow::new(), bluecaster_core::StreamState::Idle, None);
        assert_eq!(empty.first_seq, None);
    }

    #[test]
    fn audio_rates_reach_the_audio_step() {
        // The Audio step only reports live rates once it finished
        let mut f = streaming_flow();
        f.feed(500, "bt_host::app", Level::INFO, "streaming started");
        f.feed(600, "audio_capture", Level::INFO, "audio capture started: 48000 Hz");
        let idle = AudioRates::default();
        let stalled = AudioRates { packets_per_second: 0.0, kbps: 0.0, capture_frames_per_second: 48000.0 };
        let live = AudioRates { packets_per_second: 120.0, kbps: 990.0, capture_frames_per_second: 48000.0 };
        for (rates, key) in [(idle, "dbgc.audio_idle"), (stalled, "dbgc.audio_stalled"), (live, "dbgc.audio_live")] {
            let v = flow_view_of(&f, bluecaster_core::StreamState::Streaming, Some(rates));
            let audio = v.steps.iter().find(|s| s.step == "audio").unwrap();
            assert_eq!(audio.detail_msg.as_ref().map(|m| m.key.as_str()), Some(key));
        }
        // Without rates the log's detail stays
        let v = flow_view_of(&f, bluecaster_core::StreamState::Streaming, None);
        let audio = v.steps.iter().find(|s| s.step == "audio").unwrap();
        assert!(audio.detail_msg.is_none());
    }

    #[test]
    fn firmware_line_per_adapter() {
        let a = ipc::AdapterInfo {
            instance_id: String::new(),
            vid: 0x2357,
            pid: 0x0604,
            display_name: "RTL8761BU".into(),
            device_name: String::new(),
            vendor: "Realtek".into(),
            driver: ipc::DriverKind::Winusb,
            driver_service: "WinUSB".into(),
            realtek_chip: Some("RTL8761BU".into()),
            firmware_present: Some(false),
            firmware_files: vec!["rtl8761bu_fw.bin".into(), "rtl8761bu_config.bin".into()],
            firmware_invalid: None,
            firmware_download: true,
            vendor_note: None,
            restart_needed: false,
            le_audio: None,
            address: None,
            bt_version: None,
            windows_name: None,
        };
        let csr = ipc::AdapterInfo { display_name: "CSR8510".into(), firmware_present: None, ..a.clone() };
        let page = ipc::AdapterInfo { firmware_invalid: Some("rtl8761bu_fw.bin".into()), ..a.clone() };
        assert_eq!(
            firmware_text(&[a, csr]).unwrap(),
            "RTL8761BU: MISSING (rtl8761bu_fw.bin, rtl8761bu_config.bin); CSR8510: none needed"
        );
        assert_eq!(
            firmware_text(&[page]).unwrap(),
            "RTL8761BU: INVALID: rtl8761bu_fw.bin is not a valid file (rtl8761bu_fw.bin, rtl8761bu_config.bin)"
        );
        assert_eq!(firmware_text(&[]), None);
    }

    #[test]
    fn sources_go_back_to_the_core() {
        let s = ipc::SourceInfo { kind: "app".into(), name: "x.exe".into(), waiting: true };
        let c = core_source(Some(&s));
        assert_eq!((c.kind, c.name.as_str(), c.waiting), ("app", "x.exe", true));
        assert_eq!(core_source(None).kind, "loopback");
    }
}
