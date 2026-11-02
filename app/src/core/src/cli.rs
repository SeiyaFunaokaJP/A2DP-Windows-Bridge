//! The command line: `a2dpwb-core --cli -d XX:XX:XX:XX:XX:XX [options]`.
//! The options are v1's; streaming goes through the same core service as the
//! resident process.
//!
//! Without -d (and without a receiver giving its address), the first
//! Windows-paired device whose class of device is Audio/Video is used, as v1
//! did; -l lists the devices Windows has paired. Both read Windows' device
//! list only: the adapter A2DPWB uses is not touched.
//!
//! `--le-audio` (implied by `--le-device` and `--le-scan`) streams LC3 over
//! LE Audio (experimental) instead: v1's le_audio_cli.cpp flow and printed
//! lines (the emulator tests read them), on the same core service with LE
//! Audio forced on.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use a2dpwb_core::{Service, ServiceEvent, ServiceOptions, SourceRequest, StreamRequest, StreamState, VolumeSync};
use bt_host::HciCapture;
use bt_host::media::clamp_media_payload_limit;
use codec::{CodecKind, Quality};
use hci_transport::UsbSelector;
use win_sys::bt_paired::{self, PairedDevice};

const USAGE: &str = "\
Usage: a2dpwb-core --cli [-d XX:XX:XX:XX:XX:XX] [options]

  -d <address>          Bluetooth address of the headphones / receiver
                        (default: the first audio device paired with Windows)
  --device-name <name>  its name, with -d (as a saved device's: Huawei earbuds
                        are told by it)
  -l                  list the Bluetooth devices paired with Windows and exit
  -u <adapter>          USB Bluetooth adapter to use when several have WinUSB:
                        instance ID (USB\\VID_0BDA&PID_8771\\...), device path
                        (\\\\?\\usb#vid_0bda&pid_8771#...) or VID:PID (0bda:8771)
  -c <codec>           ldac, aptxhd, aptxll, aptx, aptxad, sbcxq, sbc, aac, lhdcv5, opus, opus05, faststream, auto (default)
  -q <quality>          hq (default); sq, mq for LDAC and LHDC V5; mhq for LHDC V5; ll (low latency)
                        for aptX Adaptive (other codecs: hq, their highest bit rate)
  -a                    adaptive bit rate (LDAC, LHDC V5, aptX Adaptive; SBC / SBC XQ: bitpool)
  --fallback            a codec -c the device does not offer falls back to the
                        best one it offers (as the app does), instead of an error
  --no-experimental     never use the experimental codecs (aptX HD, aptX LL, Opus,
                        Opus 05, FastStream: experimental in the README)
  -m <mode>             capture: loopback (default), virtual, app
  --audio-device <id>   render device for -m virtual
  --app <exe>           executable for -m app
  --rate <hz>           capture sample rate (default: 48 kHz, or the best the device lists)
  --bits <16|24>        bit depth (default: 24 where the codec carries it)
  --max-packet <bytes>  max media packet size (679-1679, default 1023)
  --sbc-blocks <n>      SBC / SBC XQ block length: 4, 8, 12, 16 (default)
  --sbc-subbands <n>    SBC / SBC XQ subbands: 4, 8 (default)
  --sbc-alloc <method>  SBC / SBC XQ allocation: loudness (default), snr
  --sbc-mode <mode>     SBC channel mode: joint_stereo (default), stereo,
                        mono (dual channel: -c sbcxq)
  --sbc-bitpool <n>     SBC / SBC XQ bitpool (default: by quality)
  --afh <policy>        auto, off or Wi-Fi channels to avoid such as 6,11
  --remote-sink <host[:port]|auto>
                        statistics of a measuring receiver (tools/linux_sink);
                        without -d, stream to that receiver
  --scan [seconds]      list nearby devices and exit

LE Audio (experimental, LC3 over LE isochronous channels):
  --le-audio            stream to the LE Audio device -d (BAP unicast, LC3)
  --le-device <address> another device of the group (e.g. the second earbud)
  --le-preset <name>    use only this BAP configuration (e.g. 48_2_2, 16_2_1)
  --le-low-latency      GMAP low latency (7.5 / 10 ms frames) for a gaming headset
  --le-scan [seconds]   list the LE Audio devices nearby and exit

Development / testing:
  --hci-tcp <host:port> H4 over TCP to a virtual controller (tools/emu)
  --hci-capture <path>  HCI capture (.pklg) of the whole run
  --test-tone           1 kHz left / 1.5 kHz right at 48 kHz instead of capture
  --duration <seconds>  stop after this time (default: Ctrl+C)
  --sessions <n>        stream n times in a row with --duration each, as
                        Disconnect / Connect in the app (A2DP)
  --restart             with --sessions: between sessions restart Bluetooth
                        as the Adapters page does, which connects again
  --le-sim-volume <level>[,<s>:<level>...]
                        LE Audio: VolumeSync with a simulated Windows volume
                        0-127 (changed at <s> seconds); the real volume is not
                        touched
  --mcp-dry-run         LE Audio: media control answered by a simulated player,
                        commands only logged (implied by --hci-tcp)
  --ccp-sim \"<s>:<command>;...\"
                        LE Audio: simulated calls (incoming, alerting,
                        answered, hold, retrieve, hangup)
  --ccp-console         LE Audio: call simulation commands from the console
";

struct Args {
    device: Option<String>,
    device_name: String,
    /// -l: list the Windows-paired devices and exit
    list: bool,
    /// -u: the adapter, as given and as parsed
    usb: Option<(String, UsbSelector)>,
    codec: Option<CodecKind>,
    quality: Quality,
    abr: bool,
    /// --fallback: a codec the device does not offer falls back, as a profile's
    fallback: bool,
    /// --no-experimental: settings experimental_codecs off
    no_experimental: bool,
    mode: String,
    audio_device: String,
    app: String,
    rate: Option<u32>,
    bits: Option<u8>,
    max_packet: u16,
    /// --sbc-*: the SBC setup and bitpool (defaults: 0 / None)
    sbc: codec::sbc::SbcParams,
    sbc_bitpool: u8,
    afh: Option<String>,
    remote_sink: Option<String>,
    scan: Option<u32>,
    hci_tcp: Option<String>,
    hci_capture: Option<PathBuf>,
    test_tone: bool,
    duration: Option<u64>,
    /// --sessions: streams one after the other in this process (Connect,
    /// --duration, Disconnect), as the UI's Disconnect / Connect (issue #84)
    sessions: u32,
    /// --restart: between sessions Bluetooth is restarted instead, as on
    /// the Adapters page, and the session connects again (issue #86)
    restart: bool,
    le: LeArgs,
}

/// LE Audio (experimental) options (v1 LeAudioCliOptions)
#[derive(Debug, Default, PartialEq, Eq)]
struct LeArgs {
    /// --le-audio, or implied by --le-device / --le-scan
    on: bool,
    /// --le-device: more devices of the group (after -d)
    devices: Vec<String>,
    preset: Option<String>,
    sim_volume: Option<String>,
    low_latency: bool,
    /// --le-scan [seconds]
    scan: Option<u32>,
    mcp_dry_run: bool,
    ccp_sim: Option<String>,
    ccp_console: bool,
}

/// --le-scan without a time (v1 scan_seconds)
const LE_SCAN_SECONDS: u32 = 8;

impl LeArgs {
    /// The stream request: --le-preset tries only that preset. GMAP low
    /// latency is the flag only (not the GUI setting), and an explicit
    /// preset wins: the message to print then.
    fn request(&self) -> (bt_host::LeStreamRequest, bool, Option<&'static str>) {
        let mut request = bt_host::LeStreamRequest::default();
        if let Some(p) = &self.preset {
            request.preset_order = vec![p.clone()];
        }
        let low_latency = self.low_latency && self.preset.is_none();
        let note =
            (self.low_latency && self.preset.is_some()).then_some("--le-preset given: --le-low-latency is not used");
        (request, low_latency, note)
    }
}

fn parse(args: &[String]) -> Result<Args, String> {
    let mut a = Args {
        device: None,
        device_name: String::new(),
        list: false,
        usb: None,
        codec: None,
        quality: Quality::High,
        abr: false,
        fallback: false,
        no_experimental: false,
        mode: "loopback".into(),
        audio_device: String::new(),
        app: String::new(),
        rate: None,
        bits: None,
        max_packet: bt_host::media::MEDIA_PAYLOAD_LIMIT_DEFAULT,
        sbc: codec::sbc::SbcParams::default(),
        sbc_bitpool: 0,
        afh: None,
        remote_sink: None,
        scan: None,
        hci_tcp: None,
        hci_capture: None,
        test_tone: false,
        duration: None,
        sessions: 1,
        restart: false,
        le: LeArgs::default(),
    };
    let mut it = args.iter().peekable();
    let value = |it: &mut std::iter::Peekable<std::slice::Iter<String>>, name: &str| {
        it.next().cloned().ok_or_else(|| format!("{name} needs a value"))
    };
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--cli" => {}
            "-d" => a.device = Some(value(&mut it, "-d")?),
            "--device-name" => a.device_name = value(&mut it, "--device-name")?,
            "-l" => a.list = true,
            "-u" => {
                let u = value(&mut it, "-u")?;
                let selector = UsbSelector::from_arg(&u)?;
                a.usb = Some((u.trim().to_string(), selector));
            }
            "-c" => {
                let c = value(&mut it, "-c")?;
                a.codec = if c.eq_ignore_ascii_case("auto") {
                    None
                } else {
                    Some(CodecKind::from_key(&c).ok_or_else(|| {
                        format!("Unknown codec '{c}'. Use: ldac, aptxhd, aptxll, aptx, aptxad, sbcxq, sbc, aac, lhdcv5, opus, opus05, faststream, auto")
                    })?)
                };
            }
            "-q" => a.quality = Quality::from_key(&value(&mut it, "-q")?.to_lowercase()),
            "-a" => a.abr = true,
            "--fallback" => a.fallback = true,
            "--no-experimental" => a.no_experimental = true,
            "-m" => a.mode = value(&mut it, "-m")?.to_lowercase(),
            "--audio-device" => a.audio_device = value(&mut it, "--audio-device")?,
            "--app" => a.app = value(&mut it, "--app")?,
            "--rate" => a.rate = Some(value(&mut it, "--rate")?.parse().map_err(|_| "bad --rate")?),
            "--bits" => a.bits = Some(value(&mut it, "--bits")?.parse().map_err(|_| "bad --bits")?),
            "--max-packet" => {
                let v: u32 = value(&mut it, "--max-packet")?.parse().map_err(|_| "bad --max-packet")?;
                a.max_packet = clamp_media_payload_limit(v);
                if a.max_packet as u32 != v {
                    println!("--max-packet {v} is outside 679-1679, using {}", a.max_packet);
                }
            }
            "--sbc-blocks" => {
                a.sbc.blocks = value(&mut it, "--sbc-blocks")?.parse().map_err(|_| "bad --sbc-blocks")?;
                if !matches!(a.sbc.blocks, 4 | 8 | 12 | 16) {
                    return Err("--sbc-blocks takes 4, 8, 12 or 16".into());
                }
            }
            "--sbc-subbands" => {
                a.sbc.subbands = value(&mut it, "--sbc-subbands")?.parse().map_err(|_| "bad --sbc-subbands")?;
                if !matches!(a.sbc.subbands, 4 | 8) {
                    return Err("--sbc-subbands takes 4 or 8".into());
                }
            }
            "--sbc-alloc" => {
                let v = value(&mut it, "--sbc-alloc")?.to_lowercase();
                a.sbc.allocation =
                    Some(codec::sbc::Allocation::from_key(&v).ok_or("--sbc-alloc takes loudness or snr")?);
            }
            "--sbc-mode" => {
                let v = value(&mut it, "--sbc-mode")?.to_lowercase();
                // Dual channel is SBC XQ's (issue #86)
                a.sbc.channel_mode = Some(
                    codec::sbc::ChannelMode::from_key(&v)
                        .filter(|m| *m != codec::sbc::ChannelMode::DualChannel)
                        .ok_or("--sbc-mode takes joint_stereo, stereo or mono (dual channel: -c sbcxq)")?,
                );
            }
            "--sbc-bitpool" => {
                a.sbc_bitpool = value(&mut it, "--sbc-bitpool")?.parse().map_err(|_| "bad --sbc-bitpool")?;
                if !(2..=53).contains(&a.sbc_bitpool) {
                    return Err("--sbc-bitpool takes 2 to 53".into());
                }
            }
            "--afh" => a.afh = Some(value(&mut it, "--afh")?),
            "--remote-sink" => a.remote_sink = Some(value(&mut it, "--remote-sink")?),
            "--scan" => {
                let secs = match it.peek() {
                    Some(s) if !s.starts_with('-') => it.next().and_then(|s| s.parse().ok()).unwrap_or(10),
                    _ => 10,
                };
                a.scan = Some(secs);
            }
            "--hci-tcp" => a.hci_tcp = Some(value(&mut it, "--hci-tcp")?),
            "--hci-capture" => a.hci_capture = Some(value(&mut it, "--hci-capture")?.into()),
            "--test-tone" => a.test_tone = true,
            "--duration" => a.duration = Some(value(&mut it, "--duration")?.parse().map_err(|_| "bad --duration")?),
            "--sessions" => {
                a.sessions = value(&mut it, "--sessions")?.parse().map_err(|_| "bad --sessions")?;
                if a.sessions == 0 {
                    return Err("--sessions needs at least 1".into());
                }
            }
            "--restart" => a.restart = true,
            "--le-audio" => a.le.on = true,
            "--le-device" => {
                a.le.on = true;
                a.le.devices.push(value(&mut it, "--le-device")?);
            }
            "--le-preset" => a.le.preset = Some(value(&mut it, "--le-preset")?),
            "--le-sim-volume" => a.le.sim_volume = Some(value(&mut it, "--le-sim-volume")?),
            "--le-low-latency" => a.le.low_latency = true,
            "--le-scan" => {
                a.le.on = true;
                let secs = match it.peek() {
                    Some(s) if !s.starts_with('-') => it.next().and_then(|s| s.parse().ok()).unwrap_or(LE_SCAN_SECONDS),
                    _ => LE_SCAN_SECONDS,
                };
                a.le.scan = Some(secs);
            }
            "--mcp-dry-run" => a.le.mcp_dry_run = true,
            "--ccp-sim" => a.le.ccp_sim = Some(value(&mut it, "--ccp-sim")?),
            "--ccp-console" => a.le.ccp_console = true,
            "-h" | "--help" => return Err(String::new()),
            other => return Err(format!("unknown option {other}")),
        }
    }
    Ok(a)
}

pub fn run(args: &[String]) -> i32 {
    let a = match parse(args) {
        Ok(a) => a,
        Err(e) => {
            if !e.is_empty() {
                eprintln!("{e}\n");
            }
            print!("{USAGE}");
            return if e.is_empty() { 0 } else { 2 };
        }
    };
    if a.list {
        // Windows' list only: no service, no adapter
        return match bt_paired::paired_devices() {
            Ok(devices) => {
                print_paired(&devices);
                println!("\nDone.");
                0
            }
            Err(e) => {
                eprintln!("BluetoothFindFirstDevice failed: {e}");
                1
            }
        };
    }
    let running = Arc::new(AtomicBool::new(true));
    {
        let running = running.clone();
        let _ = ctrlc::set_handler(move || running.store(false, Ordering::Release));
    }

    match (&a.hci_tcp, &a.usb) {
        (Some(addr), usb) => {
            println!("Transport: H4 over TCP to {addr} (development / testing)");
            if let Some((u, _)) = usb {
                println!("-u {u} is not used with --hci-tcp");
            }
        }
        (None, Some((u, _))) => println!("Transport: USB (WinUSB), adapter {u}"),
        (None, None) => println!("Transport: USB (WinUSB)"),
    }
    let capture = HciCapture::new();
    if let Some(path) = &a.hci_capture
        && let Err(e) = capture.start(path)
    {
        eprintln!("Cannot create HCI capture file {}: {e}", path.display());
        return 1;
    }
    let (tx, rx) = mpsc::channel();
    let service = Service::new(
        ServiceOptions {
            config_dir: a2dpwb_core::paths::config_dir(),
            hci_tcp: a.hci_tcp.clone(),
            capture: capture.clone(),
            afh: a.afh.clone(),
            usb: a.usb.as_ref().map(|(_, s)| s.clone()).unwrap_or_default(),
        },
        tx,
    );

    if a.le.on {
        // LE Audio (experimental): its own flow (v1 le_audio_cli.cpp)
        let code = le_run(&a, &service, &running);
        service.shutdown();
        capture.stop();
        return code;
    }

    if let Some(secs) = a.scan {
        let code = match service.scan(secs) {
            Ok(found) => {
                for d in found {
                    println!(
                        "{}  {:<32} CoD 0x{:06X}  RSSI {}{}",
                        bt_host::addr_str(&d.addr),
                        d.name,
                        d.class_of_device,
                        d.rssi,
                        if d.is_audio() { "  (audio)" } else { "" }
                    );
                }
                0
            }
            Err(m) => {
                eprintln!("{m}");
                1
            }
        };
        service.shutdown();
        capture.stop();
        return code;
    }

    // The measuring receiver: its IP for the statistics and, without -d, its
    // Bluetooth address as the device to stream to
    let receiver = a2dpwb_core::receiver::ReceiverClient::new();
    let mut device_arg = a.device.clone();
    if let Some(target) = &a.remote_sink {
        use a2dpwb_core::receiver;
        let Some((host, port)) = receiver::split_target(target) else {
            eprintln!("Bad --remote-sink {target}");
            return 2;
        };
        let auto = host.eq_ignore_ascii_case(receiver::AUTO);
        let found = receiver::discover((!auto).then_some(host.as_str()), port, receiver::DISCOVER_TIMEOUT);
        let mut stats_target = target.clone();
        match found.first() {
            None => println!("Receiver: none answered {}", if auto { "on the local network" } else { host.as_str() }),
            Some(r) => {
                println!("{}", r.describe(found.len() > 1));
                if device_arg.is_none() && !r.bt_address.is_empty() {
                    device_arg = Some(r.bt_address.clone());
                    println!("Streaming to the receiver (no -d given)");
                }
                if auto {
                    stats_target = format!("{}:{}", r.ip, r.port);
                }
            }
        }
        println!("Receiver statistics: {stats_target}");
        receiver.start(&stats_target);
    }

    let stop = |code: i32| {
        service.shutdown();
        capture.stop();
        if a.remote_sink.is_some() {
            receiver.stop();
        }
        code
    };
    let mut device_name = a.device_name.clone();
    let device = match device_arg.as_deref() {
        Some(d) => match parse_device_address(d) {
            Some(addr) => addr,
            None => {
                eprintln!("Invalid Bluetooth address: {d}");
                return stop(1);
            }
        },
        // v1: the first Windows-paired device that is an A2DP sink by its
        // class of device (works while the adapter has WinUSB: Windows
        // remembers its paired devices)
        None => {
            println!("Scanning for Bluetooth audio devices...");
            let devices = match bt_paired::paired_devices() {
                Ok(devices) => devices,
                Err(e) => {
                    eprintln!("BluetoothFindFirstDevice failed: {e}");
                    Vec::new()
                }
            };
            print_paired(&devices);
            match bt_paired::first_a2dp_sink(&devices) {
                Some(d) => {
                    println!("Auto-selected device: {} ({})", d.name, d.address_string());
                    device_name = d.name.clone();
                    d.address
                }
                None => {
                    eprintln!(
                        "No Bluetooth audio device found.\n\
                         Use -d XX:XX:XX:XX:XX:XX to specify the device address.\n\
                         (Find the address in Windows Settings > Bluetooth before switching to WinUSB,\n\
                         or list the devices nearby with --scan)"
                    );
                    return stop(1);
                }
            }
        }
    };
    let source = if a.test_tone {
        SourceRequest::TestTone
    } else {
        match a.mode.as_str() {
            "virtual" => SourceRequest::VirtualDevice {
                id: a.audio_device.clone(),
                name: a.audio_device.clone(),
                auto_switch: true,
            },
            "app" => SourceRequest::Application { exe: a.app.clone() },
            _ => SourceRequest::SystemLoopback,
        }
    };
    println!("Target device: {}", bt_host::addr_str(&device));
    let request = StreamRequest {
        device,
        device_name,
        codec: a.codec,
        codec_fallback: a.fallback,
        experimental_codecs: !a.no_experimental,
        auto_order: codec::AutoOrder::default(),
        quality: a.quality,
        abr: a.abr,
        sbc_bitpool: a.sbc_bitpool,
        sbcxq_bitpool: a.sbc_bitpool,
        sbc_min_bitpool: 0,
        sbcxq_min_bitpool: 0,
        sbc: a.sbc,
        sample_rate: a.rate,
        bit_depth: a.bits,
        source,
        volume_sync: VolumeSync::Off,
        mute_output: false,
        max_media_payload: a.max_packet,
        le: None,
    };

    let mut code = 0;
    let mut request = request;
    for session in 1..=a.sessions {
        if session > 1 && a.restart {
            // Restart Bluetooth, as the Adapters page does: the session
            // connects again on the new stack
            println!("\nRestarting Bluetooth (session {} of {})...", session - 1, a.sessions);
            let Some(again) = service.restart_adapter() else {
                println!("Bluetooth restarted, but nothing to connect again");
                code = 1;
                break;
            };
            request = again;
            std::thread::sleep(SESSION_PAUSE);
            if !running.load(Ordering::Acquire) {
                break;
            }
            println!("Connecting again after the restart (session {session} of {})", a.sessions);
        } else if session > 1 {
            // Disconnect, as the app's button does, and connect again
            println!("\nDisconnecting (session {} of {})...", session - 1, a.sessions);
            service.disconnect();
            while let Ok(e) = rx.try_recv() {
                if let ServiceEvent::State(state, msg) = e {
                    println!("[{state:?}] {msg}");
                }
            }
            std::thread::sleep(SESSION_PAUSE);
            if !running.load(Ordering::Acquire) {
                break;
            }
            println!("Connecting again (session {session} of {})", a.sessions);
        }
        service.start_streaming(request.clone());
        let duration_reached = stream_session(&a, &service, &rx, &running, &receiver, &mut code);
        if !duration_reached || code != 0 {
            break;
        }
    }
    service.shutdown();
    capture.stop();
    if a.remote_sink.is_some() {
        // Let the last packets arrive and the receiver report them
        std::thread::sleep(Duration::from_millis(1500));
    }
    let stats = service.stats();
    println!(
        "Sent {} media packets ({} bytes), {} dropped before the radio",
        stats.packets_sent.load(Ordering::Relaxed),
        stats.bytes_sent.load(Ordering::Relaxed),
        stats.queue_drops.load(Ordering::Relaxed) + stats.send_errors.load(Ordering::Relaxed)
    );
    if a.remote_sink.is_some() {
        println!("{}", receiver.summary());
        receiver.stop();
    }
    code
}

/// --sessions: the pause between Disconnect and the next Connect
const SESSION_PAUSE: Duration = Duration::from_secs(3);

/// One stream until --duration, Ctrl+C, an error or the end of the
/// session; true when --duration ended it
fn stream_session(
    a: &Args,
    service: &Service,
    rx: &mpsc::Receiver<ServiceEvent>,
    running: &AtomicBool,
    receiver: &a2dpwb_core::receiver::ReceiverClient,
    code: &mut i32,
) -> bool {
    let mut streaming_since: Option<Instant> = None;
    let mut last_summary = Instant::now();
    loop {
        if a.remote_sink.is_some() && streaming_since.is_some() && last_summary.elapsed() >= Duration::from_secs(5) {
            last_summary = Instant::now();
            println!("{}", receiver.summary());
        }
        if !running.load(Ordering::Acquire) {
            println!("\nStopping...");
            return false;
        }
        if let (Some(since), Some(d)) = (streaming_since, a.duration)
            && since.elapsed() >= Duration::from_secs(d)
        {
            println!("\n--duration {d} s reached");
            return true;
        }
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(ServiceEvent::State(state, msg)) => {
                println!("[{state:?}] {msg}");
                match state {
                    StreamState::Streaming if streaming_since.is_none() => streaming_since = Some(Instant::now()),
                    StreamState::Error => {
                        *code = 1;
                        return false;
                    }
                    StreamState::Idle if streaming_since.is_some() || !service.is_running() => return false,
                    _ => {}
                }
            }
            Ok(ServiceEvent::StreamInfo(i)) => println!(
                "Codec: {} | Bitrate: {} kbps | Sample rate: {} Hz | Channels: {}",
                i.codec, i.bitrate_kbps, i.sample_rate, i.channels
            ),
            Ok(ServiceEvent::Volume(v)) => println!("Headphone volume: {v} ({}%)", v as u32 * 100 / 127),
            Ok(ServiceEvent::Battery(v)) => println!("Headphone battery: {v}%"),
            Ok(ServiceEvent::BatteryRefused) => println!("Headphone battery: unknown (HFP refused)"),
            Ok(ServiceEvent::OtherDeviceBack(a)) => println!("Another device connected: {}", bt_host::addr_str(&a)),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return false,
        }
    }
}

// ---- LE Audio (experimental): v1 le_audio_cli.cpp ----

/// Waits for the LE core after BTstack started (v1: 50 x 100 ms)
const LE_READY_WAIT: Duration = Duration::from_secs(5);
/// le_find before each connection (v1 6000 ms)
const LE_FIND_TIMEOUT: Duration = Duration::from_secs(6);
/// le_connect (v1 30000 ms)
const LE_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
/// Set-ups after a lost stream, in all (v1)
const LE_RESTORE_ATTEMPTS: u32 = 3;
const LE_STATS_EVERY: Duration = Duration::from_secs(5);

fn le_print_stats(host: &bt_host::BtHost) {
    println!("{}", a2dpwb_core::le_stats_line(&host.le_stream_stats()));
}

/// What the CLI needs of the LE core (v1 le_audio::Core): [`bt_host::BtHost`],
/// or a fake in the tests
trait LeHost {
    fn le_find(&self, addr: [u8; 6], timeout: Duration) -> Option<bt_host::LeScanResult>;
    fn le_connect(&self, addr: bt_host::LeAddr, name: &str, timeout: Duration) -> Result<bt_host::LeDeviceId, String>;
}

impl LeHost for bt_host::BtHost {
    fn le_find(&self, addr: [u8; 6], timeout: Duration) -> Option<bt_host::LeScanResult> {
        bt_host::BtHost::le_find(self, addr, timeout)
    }

    fn le_connect(&self, addr: bt_host::LeAddr, name: &str, timeout: Duration) -> Result<bt_host::LeDeviceId, String> {
        bt_host::BtHost::le_connect(self, addr, name, timeout)
    }
}

/// Connects every address of the group (v1 connect_group): the address
/// type and name come from the advertisement (an RPA of a bonded device
/// changes: le_find resolves it); the ids in order, None if one fails
fn le_connect_group(
    host: &dyn LeHost,
    addrs: &[String],
    out: &mut dyn std::io::Write,
) -> Option<Vec<bt_host::LeDeviceId>> {
    let mut ids = Vec::new();
    for a in addrs {
        let Some(parsed) = parse_device_address(a) else {
            eprintln!("Invalid LE address: {a}");
            return None;
        };
        let mut addr = bt_host::LeAddr { addr: parsed, addr_type: bt_host::LeAddr::PUBLIC };
        let mut name = String::new();
        match host.le_find(parsed, LE_FIND_TIMEOUT) {
            Some(found) => {
                addr = found.addr;
                name = found.name.clone();
                let _ = writeln!(
                    out,
                    "Found {a} \"{name}\" (address type {}){}",
                    if addr.addr_type != bt_host::LeAddr::PUBLIC { "random" } else { "public" },
                    if found.le_audio { ", LE Audio" } else { "" }
                );
                if let Some(identity) = found.identity
                    && found.addr.addr != parsed
                {
                    let _ = writeln!(out, "  advertising as {} now (identity {identity})", found.addr);
                }
            }
            None => {
                let _ = writeln!(out, "{a} not seen advertising, connecting with a public address");
            }
        }
        match host.le_connect(addr, &name, LE_CONNECT_TIMEOUT) {
            Ok(id) => ids.push(id),
            Err(e) => {
                eprintln!("LE Audio: cannot connect {a}: {e}");
                return None;
            }
        }
    }
    Some(ids)
}

/// v1's --le-scan listing line
fn le_scan_line(r: &bt_host::LeScanResult) -> String {
    format!(
        "  {}  {:<24}  RSSI {:4}  {}{}{}",
        r.addr,
        if r.name.is_empty() { "(no name)" } else { r.name.as_str() },
        r.rssi,
        if r.addr.addr_type != bt_host::LeAddr::PUBLIC { "random" } else { "public" },
        if r.rsi.is_some() { ", set member" } else { "" },
        if r.note.is_empty() { String::new() } else { format!(", {}", r.note) }
    )
}

/// Connects the group and starts the stream; Err: the error to print (empty
/// when it was printed already)
fn le_setup(
    host: &bt_host::BtHost,
    devices: &[String],
    request: &bt_host::LeStreamRequest,
) -> Result<bt_host::LeStreamStarted, String> {
    // (a failed connection printed why)
    let ids = le_connect_group(host, devices, &mut std::io::stdout()).ok_or_else(String::new)?;
    host.le_start_stream(ids, request.clone(), &a2dpwb_core::le_silence_set)
}

/// The LE Audio flow (v1 le_audio_cli_run); the process exit code
fn le_run(a: &Args, service: &Service, running: &Arc<AtomicBool>) -> i32 {
    let le = &a.le;
    service.set_le_audio_cli(a2dpwb_core::LeCliOptions {
        mcp_dry_run: le.mcp_dry_run,
        ccp_sim: le.ccp_sim.clone(),
        ccp_console: le.ccp_console,
    });
    let is_running = || running.load(Ordering::Acquire);

    println!(
        "\n[LE Audio, experimental] Initializing BTstack ({} transport)...",
        if a.hci_tcp.is_none() { "WinUSB" } else { "H4 over TCP" }
    );
    let host = match service.ensure_host() {
        Ok(h) => h,
        Err(m) => {
            tracing::error!("{m}");
            eprintln!("Failed to initialize BTstack.");
            return 1;
        }
    };
    // Ctrl+C ends the blocking LE calls (find, connect, stream set-up)
    {
        let canceller = host.canceller();
        let running = running.clone();
        std::thread::Builder::new()
            .name("le-cancel".into())
            .spawn(move || {
                while running.load(Ordering::Acquire) {
                    std::thread::sleep(Duration::from_millis(100));
                }
                canceller.cancel();
            })
            .ok();
    }
    let until = Instant::now() + LE_READY_WAIT;
    while !host.le_ready() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(100));
    }
    if !host.le_ready() {
        eprintln!("LE Audio did not start (does the adapter support LE?)");
        return 1;
    }

    if let Some(secs) = le.scan {
        println!("Scanning for LE Audio devices ({secs} s)...");
        let results = host.le_scan(Duration::from_secs(secs.into()));
        let mut n = 0;
        for r in results.iter().filter(|r| r.le_audio) {
            n += 1;
            println!("{}", le_scan_line(r));
        }
        println!("{n} LE Audio device(s) found");
        return 0;
    }

    let devices: Vec<String> = a.device.iter().chain(le.devices.iter()).cloned().collect();
    if devices.is_empty() {
        eprintln!("No LE Audio device given (-d <address>; --le-scan lists them)");
        return 1;
    }

    let (request, low_latency, note) = le.request();
    host.le_set_low_latency(low_latency);
    if let Some(note) = note {
        println!("{note}");
    }

    println!("\nConnecting {} LE Audio device(s)...", devices.len());
    let started = match le_setup(&host, &devices, &request) {
        Ok(s) => s,
        Err(e) => {
            if !e.is_empty() {
                eprintln!("LE Audio: cannot stream: {e}");
            }
            host.le_stop_stream();
            host.le_disconnect_all();
            return 1;
        }
    };
    println!("\nStreaming LE Audio: {}", started.status.summary);

    // Audio at the LC3 rate (WASAPI converts to it)
    let source = if a.test_tone {
        SourceRequest::TestTone
    } else if a.mode == "virtual" && !a.audio_device.is_empty() {
        SourceRequest::VirtualDevice { id: a.audio_device.clone(), name: a.audio_device.clone(), auto_switch: false }
    } else {
        SourceRequest::SystemLoopback
    };
    let mut rc = 0;
    let start_audio = |started: &bt_host::LeStreamStarted| match a2dpwb_core::LeAudioPath::start(
        &source,
        started,
        host.stats().clone(),
    ) {
        Ok(path) => {
            let rate = path.rate();
            if a.test_tone {
                println!("Audio: test tone (1 kHz left / 1.5 kHz right) at {rate} Hz");
            } else {
                let f = path.format();
                println!("Audio: WASAPI capture {} Hz {} ch (LC3 at {rate} Hz)", f.sample_rate, f.channels);
            }
            Some(path)
        }
        Err(e) => {
            tracing::error!("LE Audio: audio: {e}");
            eprintln!("Failed to start audio capture");
            None
        }
    };
    let mut audio = start_audio(&started);
    if audio.is_none() {
        rc = 1;
    }

    // Test only: the headset volume synced with a simulated Windows volume
    let mut sim_volume = None;
    if let Some(spec) = &le.sim_volume {
        match a2dpwb_core::volume_sync::SimulatedWindowsVolume::parse(spec) {
            Some(v) => sim_volume = Some(v),
            None => {
                eprintln!("Invalid --le-sim-volume {spec}");
                rc = 1;
            }
        }
    }

    let start = Instant::now();
    let mut last_print = start;
    let mut restarts = 0;
    while rc == 0 && is_running() {
        std::thread::sleep(Duration::from_millis(100));
        if let Some(d) = a.duration
            && start.elapsed() >= Duration::from_secs(d)
        {
            println!("\n--duration {d} s reached");
            break;
        }
        if last_print.elapsed() >= LE_STATS_EVERY {
            last_print = Instant::now();
            le_print_stats(&host);
        }
        if let Some(v) = sim_volume.as_mut() {
            v.tick(&a2dpwb_core::volume_sync::LeHeadphones(&host));
        }
        if host.le_stream_lost() {
            // A device or CIS went away: tear down and set it up again
            println!("\n*** LE Audio stream lost - setting it up again ***");
            if let Some(p) = audio.take() {
                p.stop();
            }
            host.le_stop_stream();
            let mut again = None;
            while again.is_none() && restarts < LE_RESTORE_ATTEMPTS && is_running() {
                restarts += 1;
                std::thread::sleep(Duration::from_secs(2));
                match le_setup(&host, &devices, &request) {
                    Ok(s) => again = Some(s),
                    Err(e) => {
                        if !e.is_empty() {
                            tracing::warn!("LE Audio: set-up failed: {e}");
                        }
                        host.le_stop_stream();
                    }
                }
            }
            let Some(started) = again else {
                eprintln!("LE Audio: could not restore the stream");
                rc = 1;
                break;
            };
            println!("*** LE Audio stream restored: {} ***", started.status.summary);
            audio = start_audio(&started);
            if audio.is_none() {
                rc = 1;
            }
        }
    }

    println!("\nShutting down...");
    if let Some(p) = audio.take() {
        p.stop();
    }
    std::thread::sleep(Duration::from_millis(100));
    le_print_stats(&host);
    host.le_stop_stream();
    host.le_disconnect_all();
    service.shutdown();
    println!("Done.");
    rc
}

/// v1's listing of the Windows-paired devices (BtDeviceDiscovery::
/// scan_paired_devices)
fn print_paired(devices: &[PairedDevice]) {
    if devices.is_empty() {
        println!("No paired Bluetooth devices found");
        return;
    }
    for d in devices {
        println!("  [{}] {} ({})", if d.is_a2dp_sink() { "A2DP" } else { "----" }, d.name, d.address_string());
    }
    println!(
        "Found {} devices ({} with A2DP Sink)",
        devices.len(),
        devices.iter().filter(|d| d.is_a2dp_sink()).count()
    );
}

/// A Bluetooth address as v1 took it (BtDeviceDiscovery::normalize_address):
/// 12 hex digits, optionally with ':', '-', '.' or ' ' between the pairs
fn parse_device_address(s: &str) -> Option<[u8; 6]> {
    let mut digits = Vec::with_capacity(12);
    let mut after_sep = false;
    for c in s.trim().chars() {
        if let Some(v) = c.to_digit(16) {
            if digits.len() == 12 {
                return None;
            }
            digits.push(v as u8);
            after_sep = false;
        } else if matches!(c, ':' | '-' | '.' | ' ') {
            // Only between pairs, and not two in a row
            if digits.is_empty() || digits.len() % 2 != 0 || after_sep {
                return None;
            }
            after_sep = true;
        } else {
            return None;
        }
    }
    if digits.len() != 12 || after_sep {
        return None;
    }
    let mut a = [0u8; 6];
    for (i, b) in a.iter_mut().enumerate() {
        *b = digits[2 * i] << 4 | digits[2 * i + 1];
    }
    Some(a)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_string).collect()
    }

    #[test]
    fn list_and_adapter_options() {
        let a = parse(&args("--cli -l")).unwrap();
        assert!(a.list && a.usb.is_none() && a.device.is_none());

        let a = parse(&args(r"--cli -u USB\VID_0BDA&PID_8771\00E04C000001 -d 00:11:22:33:44:55")).unwrap();
        let (given, sel) = a.usb.unwrap();
        assert_eq!(given, r"USB\VID_0BDA&PID_8771\00E04C000001");
        assert_eq!(sel.instance_id.as_deref(), Some(r"USB\VID_0BDA&PID_8771\00E04C000001"));

        let a = parse(&args(r"--cli -u \\?\usb#vid_0bda&pid_8771#00e04c000001#{a5dcbf10-6530-11d2-901f-00c04fb951ed}"))
            .unwrap();
        assert_eq!(a.usb.unwrap().1.instance_id.as_deref(), Some(r"usb\vid_0bda&pid_8771\00e04c000001"));

        let a = parse(&args("--cli -u 0bda:8771")).unwrap();
        assert_eq!(a.usb.unwrap().1.vid_pid, Some((0x0BDA, 0x8771)));

        assert!(parse(&args("--cli -u nonsense")).err().unwrap_or_default().contains("not a USB device"));
        assert_eq!(parse(&args("--cli -u")).err().as_deref(), Some("-u needs a value"));
    }

    #[test]
    fn sbc_options() {
        use codec::sbc::{Allocation, ChannelMode, SbcParams};
        let a = parse(&args("--cli -c sbc")).unwrap();
        assert_eq!((a.sbc, a.sbc_bitpool), (SbcParams::default(), 0));
        let a = parse(&args(
            "--cli -c sbc --sbc-blocks 8 --sbc-subbands 4 --sbc-alloc SNR --sbc-mode stereo --sbc-bitpool 40",
        ))
        .unwrap();
        assert_eq!(
            a.sbc,
            SbcParams {
                blocks: 8,
                subbands: 4,
                allocation: Some(Allocation::Snr),
                channel_mode: Some(ChannelMode::Stereo),
                min_bitpool: 0,
            }
        );
        assert_eq!(a.sbc_bitpool, 40);
        for bad in [
            "--sbc-blocks 6",
            "--sbc-subbands 2",
            "--sbc-alloc max",
            "--sbc-mode auto",
            "--sbc-mode dual_channel",
            "--sbc-bitpool 60",
        ] {
            assert!(parse(&args(&format!("--cli {bad}"))).is_err(), "{bad}");
        }
    }

    #[test]
    fn le_audio_options() {
        // As run_test.py --le-audio calls it
        let a = parse(&args(
            "--cli --hci-tcp 127.0.0.1:9610 --le-audio -d 00:A2:D0:00:00:12 --le-device 00:A2:D0:00:00:13 \
             --test-tone --duration 5 --hci-capture out\\a2dpwb.pklg --le-sim-volume 64,3:90",
        ))
        .unwrap();
        assert!(a.le.on && a.test_tone);
        assert_eq!(a.device.as_deref(), Some("00:A2:D0:00:00:12"));
        assert_eq!(a.le.devices, ["00:A2:D0:00:00:13"]);
        assert_eq!(a.le.sim_volume.as_deref(), Some("64,3:90"));
        assert_eq!(a.duration, Some(5));
        assert_eq!(a.hci_tcp.as_deref(), Some("127.0.0.1:9610"));

        // --le-device alone implies --le-audio, and repeats
        let a = parse(&args("--cli --le-device aa:bb:cc:dd:ee:01 --le-device aa:bb:cc:dd:ee:02")).unwrap();
        assert!(a.le.on);
        assert_eq!(a.le.devices.len(), 2);
        assert!(!parse(&args("--cli -d aa:bb:cc:dd:ee:01")).unwrap().le.on);

        let a = parse(&args("--cli --le-audio --le-preset 16_2_1 --mcp-dry-run --ccp-console")).unwrap();
        assert_eq!(a.le.preset.as_deref(), Some("16_2_1"));
        assert!(a.le.mcp_dry_run && a.le.ccp_console && !a.le.low_latency);
        let a = parse(&[
            "--cli".into(),
            "--le-audio".into(),
            "--ccp-sim".into(),
            "1:incoming tel:+1555|Alice|tel:+1666;4:hangup".into(),
        ])
        .unwrap();
        assert_eq!(a.le.ccp_sim.as_deref(), Some("1:incoming tel:+1555|Alice|tel:+1666;4:hangup"));

        assert_eq!(parse(&args("--cli --le-device")).err().as_deref(), Some("--le-device needs a value"));
        assert_eq!(parse(&args("--cli --le-preset")).err().as_deref(), Some("--le-preset needs a value"));
        assert_eq!(parse(&args("--cli --le-sim-volume")).err().as_deref(), Some("--le-sim-volume needs a value"));
        assert_eq!(parse(&args("--cli --ccp-sim")).err().as_deref(), Some("--ccp-sim needs a value"));
    }

    #[test]
    fn le_scan_takes_optional_seconds() {
        let a = parse(&args("--cli --le-scan")).unwrap();
        assert!(a.le.on);
        assert_eq!(a.le.scan, Some(LE_SCAN_SECONDS));
        assert_eq!(parse(&args("--cli --le-scan 3")).unwrap().le.scan, Some(3));
        let a = parse(&args("--cli --le-scan --hci-tcp 127.0.0.1:1")).unwrap();
        assert_eq!(a.le.scan, Some(LE_SCAN_SECONDS));
        assert_eq!(a.hci_tcp.as_deref(), Some("127.0.0.1:1"));
        // The classic --scan is another option
        let a = parse(&args("--cli --scan 4")).unwrap();
        assert_eq!((a.scan, a.le.on), (Some(4), false));
    }

    #[test]
    fn le_preset_overrides_low_latency() {
        let a = parse(&args("--cli --le-audio --le-low-latency")).unwrap();
        let (request, low_latency, note) = a.le.request();
        assert!(low_latency && note.is_none());
        assert_eq!(request, bt_host::LeStreamRequest::default());

        let a = parse(&args("--cli --le-audio --le-low-latency --le-preset 48_2_2")).unwrap();
        let (request, low_latency, note) = a.le.request();
        assert!(!low_latency);
        assert_eq!(note, Some("--le-preset given: --le-low-latency is not used"));
        assert_eq!(request.preset_order, ["48_2_2"]);

        let (_, low_latency, note) = parse(&args("--cli --le-audio")).unwrap().le.request();
        assert!(!low_latency && note.is_none());
    }

    #[test]
    fn every_le_case_of_run_test_parses() {
        // tools/emu/le_audio_tests: a2dpwb_args after the common ones
        for extra in [
            &["--afh", "6,11"][..],
            &["--ccp-sim", "2:alerting;3:answered"],
            &["--le-low-latency"],
            &["--le-preset", "16_2_1"],
            &["--le-sim-volume", "64"],
            &["--mcp-dry-run"],
        ] {
            let mut v = args("--cli --hci-tcp 127.0.0.1:9600 --le-audio -d 00:A2:D0:00:00:12 --test-tone --duration 8");
            v.extend(["--hci-capture".to_string(), r"C:\out dir\a2dpwb.pklg".to_string()]);
            v.extend(extra.iter().map(|s| s.to_string()));
            let a = parse(&v).unwrap_or_else(|e| panic!("{extra:?}: {e}"));
            assert!(a.le.on, "{extra:?}");
        }
    }

    /// The LE core of the tests: devices advertise as listed
    struct FakeLe {
        adverts: Vec<bt_host::LeScanResult>,
        refuse: Option<[u8; 6]>,
        calls: std::cell::RefCell<Vec<String>>,
    }

    impl LeHost for FakeLe {
        fn le_find(&self, addr: [u8; 6], _timeout: Duration) -> Option<bt_host::LeScanResult> {
            self.calls.borrow_mut().push(format!("find {}", bt_host::addr_str(&addr)));
            self.adverts.iter().find(|r| r.addr.addr == addr || r.identity.is_some_and(|i| i.addr == addr)).cloned()
        }
        fn le_connect(
            &self,
            addr: bt_host::LeAddr,
            name: &str,
            _timeout: Duration,
        ) -> Result<bt_host::LeDeviceId, String> {
            self.calls.borrow_mut().push(format!("connect {addr} type {} \"{name}\"", addr.addr_type));
            if self.refuse == Some(addr.addr) {
                return Err("no answer".into());
            }
            Ok(bt_host::LeDeviceId(self.calls.borrow().len() as u32))
        }
    }

    #[test]
    fn le_group_is_found_then_connected_in_order() {
        let rpa = bt_host::LeAddr { addr: [0x5A, 1, 2, 3, 4, 5], addr_type: bt_host::LeAddr::RANDOM };
        let identity = bt_host::LeAddr { addr: [0x00, 0xA2, 0xD0, 0, 0, 0x12], addr_type: bt_host::LeAddr::PUBLIC };
        let le = FakeLe {
            adverts: vec![bt_host::LeScanResult {
                addr: rpa,
                identity: Some(identity),
                name: "Buds L".into(),
                le_audio: true,
                ..Default::default()
            }],
            refuse: None,
            calls: Default::default(),
        };
        let mut out = Vec::new();
        let ids = le_connect_group(&le, &["00:a2:d0:00:00:12".into(), "00:A2:D0:00:00:13".into()], &mut out).unwrap();
        assert_eq!(ids.len(), 2);
        assert_eq!(
            *le.calls.borrow(),
            [
                "find 00:A2:D0:00:00:12",
                "connect 5A:01:02:03:04:05 type 1 \"Buds L\"",
                "find 00:A2:D0:00:00:13",
                "connect 00:A2:D0:00:00:13 type 0 \"\""
            ]
        );
        let text = String::from_utf8(out).unwrap();
        assert_eq!(
            text,
            "Found 00:a2:d0:00:00:12 \"Buds L\" (address type random), LE Audio\n  \
             advertising as 5A:01:02:03:04:05 now (identity 00:A2:D0:00:00:12)\n\
             00:A2:D0:00:00:13 not seen advertising, connecting with a public address\n"
        );

        // A failed connection ends the group; a bad address is refused first
        let le = FakeLe { adverts: Vec::new(), refuse: Some([0, 0xA2, 0xD0, 0, 0, 0x12]), calls: Default::default() };
        assert!(
            le_connect_group(&le, &["00:A2:D0:00:00:12".into(), "00:A2:D0:00:00:13".into()], &mut Vec::new()).is_none()
        );
        assert_eq!(le.calls.borrow().len(), 2);
        assert!(le_connect_group(&le, &["nonsense".into()], &mut Vec::new()).is_none());
    }

    #[test]
    fn le_scan_lines_as_v1() {
        let r = bt_host::LeScanResult {
            addr: bt_host::LeAddr { addr: [0x00, 0xA2, 0xD0, 0, 0, 0x12], addr_type: 0 },
            name: "Buds".into(),
            rssi: -40,
            rsi: Some([0; 6]),
            note: "CSIP set 0".into(),
            le_audio: true,
            ..Default::default()
        };
        assert_eq!(
            le_scan_line(&r),
            "  00:A2:D0:00:00:12  Buds                      RSSI  -40  public, set member, CSIP set 0"
        );
        let r = bt_host::LeScanResult { name: String::new(), rsi: None, note: String::new(), ..r };
        assert_eq!(le_scan_line(&r), "  00:A2:D0:00:00:12  (no name)                 RSSI  -40  public");
    }

    #[test]
    fn device_address_as_v1() {
        let want = Some([0x00, 0x1A, 0x7D, 0xDA, 0x71, 0x13]);
        for ok in ["00:1A:7D:DA:71:13", "00-1a-7d-da-71-13", "001A7DDA7113", "00.1A.7D.DA.71.13", " 00 1A 7D DA 71 13 "]
        {
            assert_eq!(parse_device_address(ok), want, "{ok}");
        }
        for bad in [
            "",
            "00:1A:7D:DA:71",
            "00:1A:7D:DA:71:13:00",
            "0:01A:7D:DA:71:13",
            "00::1A7DDA7113",
            "00:1A:7D:DA:71:13:",
            "00:1A:7D:DA:71:1G",
        ] {
            assert_eq!(parse_device_address(bad), None, "{bad}");
        }
    }
}
