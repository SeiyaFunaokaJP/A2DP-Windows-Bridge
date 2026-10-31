//! The Bluetooth host: BTstack on its own thread, operated through
//! [`BtHost`].
//!
//! The run loop thread owns all BTstack state (`app::HostApp`). The calls on
//! [`BtHost`] send it jobs and wait for their outcome with timeouts, and can
//! be cancelled from another thread ([`Canceller`]) — the synchronous model
//! of v1's BtStackTransport, which core's state machine builds on. Media
//! packets take a lock-free queue ([`MediaSink`]).

#![forbid(unsafe_code)]

pub mod afh;
mod app;
pub mod capture;
pub mod chips;
pub mod events;
mod le;
pub mod link_keys;
pub mod media;
pub mod rfcomm;
mod shared;
pub mod stats;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use btstack_sys::{HciBackend, Remote, Stack};
use codec::caps::{Configuration, RemoteCaps};
use hci_transport::{TcpH4, UsbSelector, UsbTransport};

use app::{HostApp, Job, Setup};
pub use btstack_sys::expect_transport_loss;
pub use capture::HciCapture;
pub use events::{addr_str, parse_addr};
pub use le::{
    LeAddr, LeConfig, LeDeviceId, LeDeviceInfo, LeRequest, LeScanResult, LeSduSink, LeStreamRequest, LeStreamStarted,
    LeStreamStats, LeStreamStatus,
};
pub use media::{MediaPacket, MediaSink};
pub use rfcomm::{
    MAX_SEND_BYTES as RFCOMM_MAX_SEND_BYTES, OPEN_DEADLINE as RFCOMM_OPEN_DEADLINE, RfcommCloseReason, RfcommError,
    RfcommEvent, RfcommOpened, RfcommReply, RfcommSink, parse_uuid,
};
use shared::{Shared, Wait};
pub use stats::{LinkRadio, LinkStats, LiveCodec, MaxGauge, StreamTiming};

const INIT_TIMEOUT: Duration = Duration::from_secs(30);
/// No HCI event at all this long after power on: the controller does not
/// answer HCI Reset (BTstack resends it every 200 ms)
const INIT_NO_ANSWER: Duration = Duration::from_secs(5);
/// Must exceed the HCI page timeout (~20 s)
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const STREAM_TIMEOUT: Duration = Duration::from_secs(10);
const DISCONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// A job the run loop should take at once
const JOB_TIMEOUT: Duration = Duration::from_secs(5);
/// An RFCOMM channel open: the BTstack thread answers by its deadline
/// (rfcomm::OPEN_DEADLINE); this only guards against a run loop that hangs
const RFCOMM_OPEN_TIMEOUT: Duration = Duration::from_secs(22);
/// One inquiry of a scan: 8 x 1.28 s = 10.24 s, BlueZ's and Android's
const INQUIRY_UNITS: u8 = 8;
/// One remote name request, then it is cancelled: a device in range answers
/// within a second or two, one that does not would hold the controller for
/// the page timeout (~20 s, gap_setup)
const NAME_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
/// A cancelled name request completes at once
const NAME_CANCEL_TIMEOUT: Duration = Duration::from_secs(2);
/// The name requests after an inquiry, all together: at most this long, at
/// least the scan's time left or else [`NAME_REQUESTS_AFTER`]
const NAME_REQUESTS_BUDGET: Duration = Duration::from_secs(12);
/// The name requests after the scan's last inquiry
const NAME_REQUESTS_AFTER: Duration = Duration::from_secs(5);
/// Wait for the BTstack thread to end at shutdown: 10 s while the PC is awake
const THREAD_END_STEP: Duration = Duration::from_millis(20);
const THREAD_END_STEPS: u32 = 500;
/// connect_retrying: attempts in all
pub const CONNECT_ATTEMPTS: u32 = 3;

/// How to reach the controller
#[derive(Debug, Clone)]
pub enum TransportConfig {
    Usb(UsbSelector),
    /// H4 over TCP, "host:port" (tools/emu)
    Tcp(String),
}

/// Fills the AFH host channel classification from the PC's Wi-Fi (core
/// implements it with win-sys); called on the BTstack thread every 30 s or
/// when the policy changes
pub trait AfhSource: Send {
    /// None: nothing to avoid (policy "off", no Wi-Fi)
    fn classify(&mut self, policy: &str) -> Option<AfhClassification>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AfhClassification {
    /// HCI Set AFH Host Channel Classification map (79 bits, 1 = unknown)
    pub map: [u8; 10],
    pub usable: u32,
    /// Wi-Fi channels avoided, "6, 11"
    pub avoided: String,
}

pub struct HostConfig {
    pub transport: TransportConfig,
    /// Folder of link_keys.txt (None: in memory only)
    pub link_key_dir: Option<PathBuf>,
    /// Folder of the firmware files (None: the executable's folder)
    pub firmware_dir: Option<PathBuf>,
    /// Realtek USB product ID of the chip for the firmware download (0: not
    /// Realtek)
    pub realtek_pid: u16,
    /// Realtek firmware / config file stems (no extension), e.g.
    /// ("rtl8761bu_fw", "rtl8761bu_config")
    pub realtek_files: Option<(String, String)>,
    /// Run the Intel bootloader firmware download first (experimental)
    pub intel_loader: bool,
    /// VID:PID of present Broadcom adapters (PatchRAM file names)
    pub broadcom_ids: Vec<(u16, u16)>,
    /// Upper limit of the media payload (clamped)
    pub media_payload_limit: u16,
    pub capture: HciCapture,
    pub afh: Option<Box<dyn AfhSource>>,
    /// LE Audio (experimental); None: LE stays off and hidden
    pub le_audio: Option<LeConfig>,
    /// Headphone battery level: an HFP audio gateway that only makes the
    /// service level connection, where the headphones report it (and the LE
    /// Battery Service). Off: no HFP at all.
    pub battery: bool,
}

impl HostConfig {
    pub fn new(transport: TransportConfig) -> HostConfig {
        HostConfig {
            transport,
            link_key_dir: None,
            firmware_dir: None,
            realtek_pid: 0,
            realtek_files: None,
            intel_loader: false,
            broadcom_ids: Vec::new(),
            media_payload_limit: media::MEDIA_PAYLOAD_LIMIT_DEFAULT,
            capture: HciCapture::new(),
            afh: None,
            le_audio: None,
            battery: false,
        }
    }
}

/// How starting the stack failed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitFailure {
    /// The controller sent no HCI event at all after power on (e.g. it hangs
    /// on HCI Reset)
    NoAnswer,
    /// The transport could not be opened, or a firmware loader gave up
    Failed,
    /// The controller answered but did not finish init in time
    Timeout,
    /// Another BtHost runs in this process
    Busy,
}

/// How a connection attempt failed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectFailure {
    /// Page timeout: the device did not answer
    NoAnswer,
    /// Link up, then no answer on it (L2CAP RTX / connection timeout)
    LinkLost,
    /// Link up, then ended by the device itself (turning off, not accepting)
    Refused,
    /// Authentication / security refused, e.g. a stale link key
    AuthFailed,
    /// The adapter or BTstack stopped working: only a new stack helps
    Stalled,
    /// Cancelled by the user
    Cancelled,
    /// Anything else, e.g. no A2DP service
    Other,
}

/// A device found by [`BtHost::scan`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredDevice {
    pub addr: [u8; 6],
    pub name: String,
    pub class_of_device: u32,
    pub rssi: i8,
}

impl DiscoveredDevice {
    /// Major device class "Audio/Video"
    pub fn is_audio(&self) -> bool {
        (self.class_of_device >> 8) & 0x1F == 0x04
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKey {
    PlayPause,
    Stop,
    Next,
    Previous,
}

/// Things that happen without a call
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostEvent {
    /// The A2DP connection went down (also see [`BtHost::check_disconnected`])
    ConnectionLost,
    /// The headphones reported their volume (0..127)
    Volume(u8),
    /// The headphones reported their battery level (percent)
    Battery(u8),
    /// The headphones refused the HFP connection that reports the battery
    /// level, typically because another device (a phone) holds their HFP
    BatteryRefused,
    /// A headphone button to pass to Windows as a media key
    MediaKey(MediaKey),
    /// The HCI transport stopped working for good
    TransportFailed,
}

/// Settings and readings shared with the BTstack thread without locks
pub(crate) struct Controls {
    /// Last volume the headphones reported (0..127), -1 unknown
    pub volume: AtomicI32,
    /// Queued volume request, -1 none
    pub volume_request: AtomicI32,
    /// Volume to set when AVRCP connects, -1 keep the headphones' own
    pub restore_volume: AtomicI32,
    pub media_keys: AtomicBool,
    afh_policy: Mutex<String>,
    pub afh_version: AtomicU32,
    /// Upper limit of the media payload, for streams established afterwards
    pub media_payload_limit: AtomicU32,
}

impl Controls {
    pub fn afh_policy(&self) -> String {
        self.afh_policy.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

/// Ends the blocking waits of a [`BtHost`] from another thread (a user stop)
#[derive(Clone)]
pub struct Canceller(Arc<Shared>);

impl Canceller {
    pub fn cancel(&self) {
        self.0.update(|s| s.cancel = true);
    }
}

/// Why [`BtHost::configure`] failed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigureError {
    NotConnected,
    /// BTstack or the device refused the configuration
    Refused,
    /// No answer in time, or cancelled
    NoAnswer,
}

/// A stream set up again by [`BtHost::reconnect`]
#[derive(Debug, Clone)]
pub struct Reconnected {
    /// The device's capabilities as it lists them now
    pub remote: RemoteCaps,
    /// The configuration built from them
    pub config: Configuration,
    /// Media payload size of the new stream (RTP header excluded)
    pub mtu: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconnectError {
    /// Not connected, configured or started (worth another attempt)
    Failed,
    /// The device no longer offers a stream like the one before
    CodecGone,
}

/// The stream of the last successful configuration, for [`BtHost::reconnect`]
struct LastStream {
    addr: [u8; 6],
    config: Configuration,
}

pub struct BtHost {
    shared: Arc<Shared>,
    controls: Arc<Controls>,
    stats: Arc<LinkStats>,
    jobs: mpsc::Sender<Job>,
    remote: Remote,
    thread: Mutex<Option<JoinHandle<()>>>,
    media: Mutex<Option<MediaSink>>,
    events: Mutex<Option<mpsc::Receiver<HostEvent>>>,
    tcp: bool,
    remote_addr: Mutex<Option<[u8; 6]>>,
    last: Mutex<Option<LastStream>>,
    capture: HciCapture,
    /// LE Audio is configured (HostConfig::le_audio was Some)
    le_on: bool,
    /// What the LE worker API reads without a round trip
    le: Arc<le::core::LeShared>,
}

fn backend(transport: &TransportConfig) -> Box<dyn HciBackend> {
    match transport {
        TransportConfig::Tcp(addr) => Box::new(TcpH4::new(addr.clone())),
        TransportConfig::Usb(sel) => {
            let mut sel = sel.clone();
            // Realtek controllers BTstack's driver knows, and a few others
            // that do not identify as Bluetooth class
            sel.known.extend(btstack_sys::realtek_usb_ids());
            sel.known.extend([(0x0A12, 0x0001), (0x8087, 0x0029), (0x8087, 0x0032)]);
            Box::new(UsbTransport::new(sel))
        }
    }
}

/// BTstack's Realtek firmware table: VID, PID, firmware name ("rtl8852bu_fw")
pub fn realtek_usb_firmware() -> &'static [(u16, u16, &'static str)] {
    btstack_sys::realtek_usb_firmware()
}

/// The BTstack thread raises its priority (MMCSS "Pro Audio" on Windows, as
/// the encode thread and the USB ACL writer: every media packet passes
/// through it) through a hook core installs, so this crate stays free of
/// Win32. What the hook returns is dropped when the thread ends.
pub type ThreadPriorityHook = fn() -> Box<dyn std::any::Any>;

static RUNLOOP_PRIORITY_HOOK: std::sync::OnceLock<ThreadPriorityHook> = std::sync::OnceLock::new();

pub fn set_runloop_priority_hook(hook: ThreadPriorityHook) {
    let _ = RUNLOOP_PRIORITY_HOOK.set(hook);
}

fn exe_dir() -> PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())).unwrap_or_else(|| ".".into())
}

impl BtHost {
    /// Starts BTstack and waits until HCI works
    pub fn start(config: HostConfig, stats: Arc<LinkStats>) -> Result<BtHost, InitFailure> {
        let shared = Arc::new(Shared::default());
        let controls = Arc::new(Controls {
            volume: AtomicI32::new(-1),
            volume_request: AtomicI32::new(-1),
            restore_volume: AtomicI32::new(-1),
            media_keys: AtomicBool::new(true),
            afh_policy: Mutex::new("auto".into()),
            afh_version: AtomicU32::new(0),
            media_payload_limit: AtomicU32::new(
                media::clamp_media_payload_limit(config.media_payload_limit as u32) as u32
            ),
        });
        let (jobs_tx, jobs_rx) = mpsc::channel::<Job>();
        let (events_tx, events_rx) = mpsc::channel::<HostEvent>();
        let (remote_tx, remote_rx) = mpsc::channel::<(Remote, MediaSink)>();
        let tcp = matches!(config.transport, TransportConfig::Tcp(_));
        let capture = config.capture.clone();
        let le_on = config.le_audio.is_some();
        let le_shared = Arc::new(le::core::LeShared::default());
        tracing::debug!(
            "host start: transport {:?}, realtek pid 0x{:04X} files {:?}, intel loader {}, broadcom {:04X?}, payload limit {}, le audio {}, battery {}",
            config.transport,
            config.realtek_pid,
            config.realtek_files,
            config.intel_loader,
            config.broadcom_ids,
            config.media_payload_limit,
            le_on,
            config.battery
        );
        let thread = {
            let (shared, controls, stats) = (shared.clone(), controls.clone(), stats.clone());
            let (le_shared, le_jobs) = (le_shared.clone(), jobs_tx.clone());
            let le_events = events_tx.clone();
            std::thread::Builder::new()
                .name("bt-runloop".into())
                .spawn(move || {
                    let _priority = RUNLOOP_PRIORITY_HOOK.get().map(|h| h());
                    let HostConfig {
                        transport,
                        link_key_dir,
                        firmware_dir,
                        realtek_pid,
                        realtek_files,
                        intel_loader,
                        broadcom_ids,
                        media_payload_limit: _,
                        capture,
                        afh,
                        le_audio,
                        battery,
                    } = config;
                    let failed_shared = shared.clone();
                    let result = btstack_sys::run(move |stack, remote| {
                        // LE Audio: the controller's LE support visible to
                        // BTstack (before hci_init / the transport)
                        if le_audio.is_some() {
                            stack.set_le_hidden(false);
                        }
                        let le = le_audio.map(|cfg| {
                            let poster = le::core::LePoster::new(le_jobs, remote);
                            le::core::LeCore::new(cfg, le_shared, controls.clone(), poster, le_events, battery)
                        });
                        capture.reset_tracking();
                        let logger = capture.clone();
                        stack
                            .set_packet_logger(Some(Box::new(move |t, incoming, p| logger.log_packet(t, incoming, p))));
                        let (sink, consumer, media_streaming) = media::queue(stats.clone(), remote);
                        let _ = remote_tx.send((remote, sink));
                        let setup = Setup {
                            tcp,
                            link_key_dir,
                            firmware_dir: firmware_dir.unwrap_or_else(exe_dir),
                            realtek_pid: if tcp { 0 } else { realtek_pid },
                            realtek_files,
                            broadcom_ids,
                            battery,
                        };
                        let mut app = HostApp::new(
                            shared,
                            controls,
                            stats,
                            jobs_rx,
                            events_tx,
                            consumer,
                            media_streaming,
                            setup,
                            afh,
                            le,
                        );
                        let backend = backend(&transport);
                        if intel_loader && !tcp {
                            // Intel controllers boot into a bootloader and
                            // need ibt-*.sfi/.ddc before regular HCI init;
                            // the loader finishes at once if the controller
                            // is operational already
                            stack.install_transport(backend);
                            tracing::info!(
                                "Intel adapter: checking bootloader firmware (experimental), folder {}",
                                app.setup.firmware_dir.display()
                            );
                            stack.intel_download_firmware(&app.setup.firmware_dir.to_string_lossy());
                        } else {
                            app.setup_hci_and_power_on(stack, Some(backend));
                        }
                        Box::new(app)
                    });
                    if result.is_err() {
                        failed_shared.update(|s| s.init_done = Some(Err(InitFailure::Busy)));
                    }
                    tracing::info!("BTstack run loop exited");
                })
                .map_err(|_| InitFailure::Failed)?
        };
        let (remote, sink) = match remote_rx.recv_timeout(JOB_TIMEOUT) {
            Ok(r) => r,
            Err(_) => {
                let _ = thread.join();
                return Err(shared.lock().init_done.and_then(|r| r.err()).unwrap_or(InitFailure::Failed));
            }
        };
        let host = BtHost {
            shared,
            controls,
            stats,
            jobs: jobs_tx,
            remote,
            thread: Mutex::new(Some(thread)),
            media: Mutex::new(Some(sink)),
            events: Mutex::new(Some(events_rx)),
            tcp,
            remote_addr: Mutex::new(None),
            last: Mutex::new(None),
            capture,
            le_on,
            le: le_shared,
        };
        match host.wait_init() {
            Ok(()) => Ok(host),
            Err(f) => {
                drop(host); // shuts the stack down, releasing the adapter
                Err(f)
            }
        }
    }

    fn wait_init(&self) -> Result<(), InitFailure> {
        let started = Instant::now();
        let r = self.shared.wait(INIT_TIMEOUT, false, |s| {
            if s.init_done.is_some() {
                return true;
            }
            // A controller that answers nothing will not answer later either
            s.power_on_at.is_some_and(|t| t.elapsed() >= INIT_NO_ANSWER) && s.controller_events == 0
        });
        let s = self.shared.lock();
        tracing::debug!(
            "init wait: {r:?} after {} ms, init_done {:?}, controller events {}, realtek rom {}",
            started.elapsed().as_millis(),
            s.init_done,
            s.controller_events,
            s.realtek_rom
        );
        match (r, s.init_done) {
            (_, Some(Ok(()))) => {
                tracing::info!("HCI ready after {} ms", started.elapsed().as_millis());
                Ok(())
            }
            (_, Some(Err(f))) => {
                tracing::error!("HCI initialization failed: {f:?}");
                Err(f)
            }
            (Wait::Done, None) => {
                tracing::error!(
                    "HCI initialization failed: the controller did not answer HCI Reset within {} ms",
                    INIT_NO_ANSWER.as_millis()
                );
                Err(InitFailure::NoAnswer)
            }
            _ => {
                tracing::error!("HCI initialization timed out");
                Err(InitFailure::Timeout)
            }
        }
    }

    /// Runs `f` on the BTstack thread and returns its result; None if the run
    /// loop did not take it within `timeout` (it may still run later)
    fn call<R: Send + 'static>(
        &self,
        timeout: Duration,
        f: impl FnOnce(&mut HostApp, &Stack) -> R + Send + 'static,
    ) -> Option<R> {
        let (tx, rx) = mpsc::sync_channel(1);
        let job: Job = Box::new(move |app, stack| {
            let _ = tx.send(f(app, stack));
        });
        self.jobs.send(job).ok()?;
        self.remote.wake();
        let r = rx.recv_timeout(timeout).ok();
        if r.is_none() {
            tracing::warn!("the BTstack run loop did not take a request within {} ms", timeout.as_millis());
        }
        r
    }

    /// Runs `f` on the BTstack thread without waiting
    fn post(&self, f: impl FnOnce(&mut HostApp, &Stack) + Send + 'static) {
        if self.jobs.send(Box::new(f)).is_ok() {
            self.remote.wake();
        }
    }

    pub fn stats(&self) -> &Arc<LinkStats> {
        &self.stats
    }

    pub fn capture(&self) -> &HciCapture {
        &self.capture
    }

    pub fn canceller(&self) -> Canceller {
        Canceller(self.shared.clone())
    }

    /// Events that happen without a call (once; None afterwards)
    pub fn take_events(&self) -> Option<mpsc::Receiver<HostEvent>> {
        self.events.lock().unwrap_or_else(|e| e.into_inner()).take()
    }

    /// The media queue's producer end, for the encode thread; give it back
    /// with [`BtHost::return_media_sink`] when the stream ends
    pub fn take_media_sink(&self) -> Option<MediaSink> {
        self.media.lock().unwrap_or_else(|e| e.into_inner()).take()
    }

    pub fn return_media_sink(&self, sink: MediaSink) {
        *self.media.lock().unwrap_or_else(|e| e.into_inner()) = Some(sink);
    }

    pub fn hci_ready(&self) -> bool {
        self.shared.lock().hci_ready
    }

    /// The Realtek controller runs its ROM code: the firmware was not loaded
    pub fn realtek_rom_firmware(&self) -> bool {
        self.shared.lock().realtek_rom
    }

    /// The controller's own address, once it has answered
    /// The headphones' battery level in percent, once they reported it
    pub fn battery(&self) -> Option<u8> {
        self.shared.lock().battery
    }

    pub fn local_address(&self) -> Option<String> {
        self.shared.lock().local_address.clone()
    }

    /// The HCI version the controller reports (the Bluetooth core version:
    /// 12 = 5.3), once it has answered Read Local Version
    pub fn hci_version(&self) -> Option<u8> {
        self.shared.lock().hci_version
    }

    /// The controller has LE Audio's isochronous channels (its supported
    /// commands at HCI init; also known with LE Audio off). Streaming still
    /// needs ISO buffers ([`BtHost::le_iso_buffers`]).
    pub fn le_audio_capable(&self) -> bool {
        self.shared.lock().le_audio_capable
    }

    // ---- RFCOMM client channels ----

    /// Opens an RFCOMM channel to the service `uuid` of `addr`, the device
    /// being streamed to, over the link the stack has to it (it never
    /// connects for this: else [`RfcommError::NotConnected`]). Answered
    /// within [`RFCOMM_OPEN_DEADLINE`]. `sink` receives the channel's data
    /// and its close, on the BTstack thread.
    pub fn rfcomm_open(&self, addr: [u8; 6], uuid: [u8; 16], sink: RfcommSink) -> Result<RfcommOpened, RfcommError> {
        self.rfcomm_open_service(addr, rfcomm::Service::Uuid(uuid), sink)
    }

    /// [`BtHost::rfcomm_open`] of a server channel of `addr`, for a service
    /// its SDP does not list
    pub fn rfcomm_open_channel(
        &self,
        addr: [u8; 6],
        channel: u8,
        sink: RfcommSink,
    ) -> Result<RfcommOpened, RfcommError> {
        self.rfcomm_open_service(addr, rfcomm::Service::Channel(channel), sink)
    }

    fn rfcomm_open_service(
        &self,
        addr: [u8; 6],
        service: rfcomm::Service,
        sink: RfcommSink,
    ) -> Result<RfcommOpened, RfcommError> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.post(move |app, s| {
            // Only to the device being streamed to, once connected (not
            // while the connection is set up, which may be given up on):
            // its channels close with the A2DP connection (close_device)
            if app.a2dp_cid == 0 || app.remote_addr != Some(addr) || !app.shared.lock().connected {
                let _ = tx.send(Err(RfcommError::NotConnected));
                return;
            }
            app.rfcomm.open(s, addr, service, sink, tx)
        });
        match rx.recv_timeout(RFCOMM_OPEN_TIMEOUT) {
            Ok(r) => r,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // The run loop did not answer: the channel is closed if it
                // opens later
                tracing::warn!("RFCOMM: open to {} timed out", addr_str(&addr));
                Err(RfcommError::NoAnswer)
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(RfcommError::NotConnected),
        }
    }

    /// Queues `data` on channel `id`, without waiting: the order of the
    /// calls is the order on the air. The reply tells whether it was queued.
    pub fn rfcomm_send(&self, id: u32, data: Vec<u8>) -> RfcommReply {
        let (tx, rx) = mpsc::sync_channel(1);
        self.post(move |app, s| {
            let _ = tx.send(app.rfcomm.send(s, id, &data));
        });
        RfcommReply(rx)
    }

    /// Closes channel `id` after what was queued before, without waiting
    /// (no [`RfcommEvent::Closed`] follows)
    pub fn rfcomm_close(&self, id: u32) -> RfcommReply {
        let (tx, rx) = mpsc::sync_channel(1);
        self.post(move |app, s| {
            let _ = tx.send(app.rfcomm.close(s, id));
        });
        RfcommReply(rx)
    }

    // ---- scanning ----

    /// GAP inquiry for about `seconds` ([`BtHost::scan_with`])
    pub fn scan(&self, seconds: u32) -> Vec<DiscoveredDevice> {
        self.scan_with(seconds, &mut |_| {})
    }

    /// GAP inquiry for about `seconds` (3..60): inquiries of 10.24 s one
    /// after the other, as BlueZ and Android run them (a device one misses
    /// answers the next), with the name requests for devices that did not
    /// send their name in between. `on_found` gets the list each time a
    /// device answers or a name comes, so it can be shown at once;
    /// [`BtHost::stop_scan`] ends the scan early.
    pub fn scan_with(&self, seconds: u32, on_found: &mut dyn FnMut(&[DiscoveredDevice])) -> Vec<DiscoveredDevice> {
        if !self.hci_ready() {
            return Vec::new();
        }
        let total = Duration::from_secs(seconds.clamp(3, 600).into());
        tracing::info!("starting inquiry (~{} s)", total.as_secs());
        let deadline = Instant::now() + total;
        let mut shown = self.shared.update(|s| {
            s.cancel = false;
            s.scan_stop = false;
            s.discovered.clear();
            s.discovered_paging.clear();
            s.inquiry_done = true;
            s.found_rev
        });
        let mut asked = Vec::new();
        let mut first = true;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            // The last inquiry may run a little over rather than be too
            // short to find anything
            if !first && left < Duration::from_secs(2) {
                break;
            }
            first = false;
            let units = left.as_millis().div_ceil(1280).clamp(2, INQUIRY_UNITS.into()) as u8;
            if !self.inquiry(units, &mut shown, on_found) {
                break;
            }
            let budget = deadline.saturating_duration_since(Instant::now());
            let names_until = Instant::now() + budget.clamp(NAME_REQUESTS_AFTER, NAME_REQUESTS_BUDGET);
            self.request_names(names_until, &mut asked, &mut shown, on_found);
            if self.scan_stopped() {
                break;
            }
        }
        let found = self.shared.lock().discovered.clone();
        tracing::info!("inquiry complete, {} device(s) found", found.len());
        found
    }

    /// Ends a running scan early: it returns what it found so far
    pub fn stop_scan(&self) {
        // cancel ends the LE scan of a scan too
        self.shared.update(|s| {
            s.scan_stop = true;
            s.cancel = true;
        });
    }

    /// The running (or last) scan was stopped
    pub fn scan_stopped(&self) -> bool {
        let s = self.shared.lock();
        s.scan_stop || s.cancel
    }

    /// Hands the scan's list to `on_found` when it changed since `shown`
    fn report_found(&self, shown: &mut u64, on_found: &mut dyn FnMut(&[DiscoveredDevice])) {
        let list = {
            let s = self.shared.lock();
            if s.found_rev == *shown {
                return;
            }
            *shown = s.found_rev;
            s.discovered.clone()
        };
        tracing::debug!("scan: {} device(s) listed", list.len());
        on_found(&list);
    }

    /// One inquiry of `units` x 1.28 s; the devices are reported as they
    /// answer. False when the scan ends here (stopped, or no inquiry).
    fn inquiry(&self, units: u8, shown: &mut u64, on_found: &mut dyn FnMut(&[DiscoveredDevice])) -> bool {
        self.shared.update(|s| s.inquiry_done = false);
        match self.call(JOB_TIMEOUT, move |_, s| s.inquiry_start(units)) {
            Some(0) => {}
            r => {
                tracing::warn!("gap_inquiry_start failed ({r:?})");
                self.shared.update(|s| s.inquiry_done = true);
                return false;
            }
        }
        let until = Instant::now() + Duration::from_millis(units as u64 * 1280 + 5000);
        loop {
            let rev = *shown;
            let w = self.shared.wait(until.saturating_duration_since(Instant::now()), false, |s| {
                s.inquiry_done || s.scan_stop || s.cancel || s.found_rev != rev
            });
            self.report_found(shown, on_found);
            if self.shared.lock().inquiry_done {
                return !self.scan_stopped();
            }
            let stopped = self.scan_stopped();
            if stopped || w == Wait::TimedOut {
                if stopped {
                    tracing::debug!("scan stopped");
                } else {
                    tracing::warn!("inquiry timed out");
                }
                self.call(Duration::from_secs(2), |_, s| s.inquiry_stop());
                // Inquiry Cancel completes it (BTstack reports that as the
                // inquiry's end)
                let _ = self.shared.wait(Duration::from_secs(2), false, |s| s.inquiry_done);
                self.shared.update(|s| s.inquiry_done = true);
                return false;
            }
        }
    }

    /// Asks the devices found without a name (none in their extended
    /// inquiry response) for it, one at a time and each once a scan: audio
    /// devices first, until `until`. Between inquiries: a page during an
    /// inquiry stalls many controllers.
    fn request_names(
        &self,
        until: Instant,
        asked: &mut Vec<[u8; 6]>,
        shown: &mut u64,
        on_found: &mut dyn FnMut(&[DiscoveredDevice]),
    ) {
        let mut ask: Vec<(bool, [u8; 6], u8, u16)> = {
            let s = self.shared.lock();
            if s.scan_stop || s.cancel {
                return;
            }
            s.discovered
                .iter()
                .filter(|d| d.name.is_empty() && !asked.contains(&d.addr))
                .filter_map(|d| {
                    let &(_, psrm, clock) = s.discovered_paging.iter().find(|x| x.0 == d.addr)?;
                    Some((d.is_audio(), d.addr, psrm, clock))
                })
                .collect()
        };
        if ask.is_empty() {
            return;
        }
        ask.sort_by_key(|x| !x.0);
        tracing::debug!("name requests: {} device(s) without a name", ask.len());
        for (audio, addr, psrm, clock) in ask {
            tracing::debug!(
                "name request: {} (audio {audio}, psrm {psrm}, clock offset 0x{clock:04X})",
                addr_str(&addr)
            );
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                tracing::info!("no time left to ask the other devices for their names");
                break;
            }
            asked.push(addr);
            self.shared.update(|s| s.name_done = false);
            if self.call(JOB_TIMEOUT, move |_, s| s.remote_name_request(&addr, psrm, clock | 0x8000)) != Some(0) {
                tracing::warn!("remote name request to {} not sent", addr_str(&addr));
                continue;
            }
            let _ = self.shared.wait(left.min(NAME_REQUEST_TIMEOUT), false, |s| s.name_done || s.scan_stop || s.cancel);
            let stopped = self.scan_stopped();
            if !self.shared.lock().name_done {
                if !stopped {
                    tracing::warn!("remote name request to {} timed out", addr_str(&addr));
                }
                // A device that does not answer would hold the controller
                // for the page timeout; cancelled, the request completes at
                // once (BTstack runs one at a time)
                self.call(JOB_TIMEOUT, move |_, s| s.cmd_remote_name_request_cancel(&addr));
                if self.shared.wait(NAME_CANCEL_TIMEOUT, false, |s| s.name_done) != Wait::Done {
                    tracing::warn!("remote name request to {} did not end", addr_str(&addr));
                    break;
                }
            }
            self.report_found(shown, on_found);
            if stopped {
                tracing::debug!("name requests stopped");
                break;
            }
        }
    }

    // ---- connection ----

    /// Connects A2DP signaling to `addr` and discovers its codecs
    pub fn connect(&self, addr: [u8; 6]) -> Result<RemoteCaps, ConnectFailure> {
        if !self.hci_ready() {
            tracing::debug!("connect {}: HCI not ready", addr_str(&addr));
            return Err(ConnectFailure::Other);
        }
        if self.stalled() {
            tracing::debug!("connect {}: stack stalled", addr_str(&addr));
            self.set_failure(ConnectFailure::Stalled);
            return Err(ConnectFailure::Stalled);
        }
        *self.remote_addr.lock().unwrap_or_else(|e| e.into_inner()) = Some(addr);
        self.shared.update(|s| {
            s.remote_caps = Default::default();
            s.disconnect_occurred = false;
            s.last_connect_failure = None;
            // The link the device opened itself (turned on) is up already:
            // no Connection Complete names it
            s.target_acl_handle = s.acl_link_to(&addr);
            s.target_acl_down_reason = 0;
            s.target_auth_status = None;
            // A fresh user-initiated operation: clear a cancel left by an
            // earlier stop, or the wait below aborts at once
            s.cancel = false;
            s.connect_done = None;
            s.stream_done = None;
        });
        tracing::info!("connecting to {}", addr_str(&addr));
        self.release_leftover_avdtp(addr);
        self.establish_and_wait(addr)
    }

    fn set_failure(&self, f: ConnectFailure) {
        self.shared.update(|s| s.last_connect_failure = Some(f));
    }

    pub fn last_connect_failure(&self) -> Option<ConnectFailure> {
        self.shared.lock().last_connect_failure
    }

    fn establish_and_wait(&self, addr: [u8; 6]) -> Result<RemoteCaps, ConnectFailure> {
        let started = Instant::now();
        match self.call(JOB_TIMEOUT, move |app, s| app.establish(s, addr)) {
            Some(Ok(())) => {}
            r => {
                tracing::warn!("a2dp_source_establish_stream failed ({r:?})");
                self.set_failure(ConnectFailure::Other);
                return Err(ConnectFailure::Other);
            }
        }
        match self.shared.wait(CONNECT_TIMEOUT, true, |s| s.connect_done.is_some()) {
            Wait::Done => {}
            w => {
                let cancelled = w == Wait::Cancelled;
                tracing::warn!("connection {}", if cancelled { "cancelled" } else { "timed out" });
                let stuck = !cancelled && self.check_stalled();
                if !stuck && !cancelled {
                    // An answer that never came (e.g. a lost SDP response)
                    self.set_failure(ConnectFailure::LinkLost);
                    self.classify_link_lost();
                }
                let f = if stuck {
                    ConnectFailure::Stalled
                } else if cancelled {
                    ConnectFailure::Cancelled
                } else {
                    self.last_connect_failure().unwrap_or(ConnectFailure::LinkLost)
                };
                tracing::debug!(
                    "connect {}: wait ended {w:?} after {} ms, stuck {stuck}, failure {f:?}",
                    addr_str(&addr),
                    started.elapsed().as_millis()
                );
                self.abort_pending_connection();
                // Set after the abort, whose failed CONNECTION_ESTABLISHED is
                // classified as a lost link
                self.set_failure(f);
                return Err(f);
            }
        }
        if self.shared.lock().connect_done != Some(true) {
            self.classify_link_lost();
            let f = self.last_connect_failure().unwrap_or(ConnectFailure::Other);
            tracing::debug!("connect {}: failed after {} ms ({f:?})", addr_str(&addr), started.elapsed().as_millis());
            self.abort_pending_connection();
            self.set_failure(f);
            return Err(f);
        }
        let caps = self.shared.lock().remote_caps.clone();
        tracing::debug!(
            "connect {}: signaling up after {} ms, remote caps {caps:?}",
            addr_str(&addr),
            started.elapsed().as_millis()
        );
        Ok(caps)
    }

    /// LinkLost -> Refused when the device ended the link itself
    fn classify_link_lost(&self) {
        if self.last_connect_failure() != Some(ConnectFailure::LinkLost) {
            return;
        }
        // L2CAP reports a link that went down as "baseband disconnect" before
        // HCI Disconnection Complete reaches us: wait for it to learn why
        self.shared.wait(Duration::from_millis(1000), false, |s| {
            s.target_acl_down_reason != 0 || s.target_acl_handle.is_none()
        });
        let reason = self.shared.lock().target_acl_down_reason;
        tracing::debug!("link lost: ACL down reason 0x{reason:02X} ({})", events::hci_error_string(reason));
        use btstack_sys::consts::*;
        if matches!(
            reason,
            ERROR_CODE_REMOTE_USER_TERMINATED_CONNECTION
                | ERROR_CODE_REMOTE_DEVICE_TERMINATED_CONNECTION_DUE_TO_LOW_RESOURCES
                | ERROR_CODE_REMOTE_DEVICE_TERMINATED_CONNECTION_DUE_TO_POWER_OFF
        ) {
            tracing::info!("the device ended the link itself (0x{reason:02x})");
            self.set_failure(ConnectFailure::Refused);
        }
    }

    /// The stack cannot be used any more: the transport lost the adapter, or
    /// a connect wait timed out on a controller that stopped answering. Shut
    /// down and start a new BtHost.
    pub fn stalled(&self) -> bool {
        self.shared.lock().stalled
    }

    /// After a connect wait timed out: is the stack stuck rather than the
    /// remote silent? Sets stalled.
    fn check_stalled(&self) -> bool {
        if self.stalled() {
            return true;
        }
        let why = match self.call(Duration::from_secs(2), |app, s| app.still_paging(s)) {
            None => Some("the BTstack run loop does not respond"),
            // Still paging long after the page timeout
            Some(true) => Some("the controller never finished the connection"),
            Some(false) if !self.controller_responds(Duration::from_secs(2)) => {
                Some("the controller does not answer HCI commands")
            }
            _ => None,
        };
        let Some(why) = why else {
            tracing::debug!("stall check: the stack answers, the remote was silent");
            return false;
        };
        tracing::error!("Bluetooth stack stalled: {why}");
        self.shared.update(|s| s.stalled = true);
        true
    }

    /// Whether the controller still answers an HCI command (Read BD_ADDR)
    /// within `timeout`; false when the adapter is gone or BTstack can no
    /// longer send commands (e.g. after the PC slept with the adapter open)
    pub fn controller_responds(&self, timeout: Duration) -> bool {
        if !self.hci_ready() || self.stalled() {
            return false;
        }
        let deadline = Instant::now() + timeout;
        self.shared.update(|s| {
            s.probe_answered = false;
            s.probe_pending = true;
        });
        // A command still waiting for its answer holds the next one back
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.call(left.max(Duration::from_millis(1)), |app, s| app.probe(s)) {
                Some(true) => break,
                Some(false) if !left.is_zero() => std::thread::sleep(Duration::from_millis(50)),
                _ => {
                    tracing::warn!("no HCI command could be sent for {} ms", timeout.as_millis());
                    self.shared.update(|s| s.probe_pending = false);
                    return false;
                }
            }
        }
        let left = deadline.saturating_duration_since(Instant::now());
        let answered = self.shared.wait(left, false, |s| s.probe_answered) == Wait::Done;
        self.shared.update(|s| s.probe_pending = false);
        if !answered {
            tracing::warn!("the controller did not answer HCI Read BD_ADDR within {} ms", timeout.as_millis());
        } else {
            tracing::debug!(
                "controller probe: answered within {} ms",
                timeout.saturating_sub(deadline.saturating_duration_since(Instant::now())).as_millis()
            );
        }
        answered
    }

    /// [`BtHost::connect`], tried again after a transient failure (no answer,
    /// link lost) up to [`CONNECT_ATTEMPTS`] times: a link can fail right
    /// after it came up and work on the next attempt. After an
    /// authentication failure (stale link key: the device was reset or paired
    /// elsewhere) the key is dropped and pairing tried once more; when that
    /// pairing fails too (the device is not in pairing mode), the key is
    /// stored again. An authentication that collided with the device's own
    /// (Command Disallowed, LMP transaction collision: the key is fine) is
    /// tried once more with the key kept.
    pub fn connect_retrying(
        &self,
        addr: [u8; 6],
        cancelled: &dyn Fn() -> bool,
        on_retry: &mut dyn FnMut(u32, u32),
    ) -> Result<RemoteCaps, ConnectFailure> {
        let mut dropped = None;
        let result = self.connect_retrying_keys(addr, cancelled, on_retry, &mut dropped);
        if result.is_err()
            && let Some((key, key_type)) = dropped
        {
            let restored = self
                .call(Duration::from_secs(2), move |_, s| {
                    // A key from a pairing that did succeed stays
                    let absent = s.link_key(&addr).is_none();
                    if absent {
                        s.store_link_key(&addr, &key, key_type);
                    }
                    absent
                })
                .unwrap_or(false);
            if restored {
                tracing::info!("pairing again failed: the dropped link key of {} is stored again", addr_str(&addr));
            }
        }
        result
    }

    /// The HCI status of the failed authentication on the link just tried.
    /// BTstack reports the failed L2CAP channel before the Authentication
    /// Complete reaches our handler, so it may take a moment.
    fn target_auth_status(&self) -> Option<u8> {
        let _ = self.shared.wait(Duration::from_millis(300), false, |s| s.target_auth_status.is_some());
        self.shared.lock().target_auth_status
    }

    fn connect_retrying_keys(
        &self,
        addr: [u8; 6],
        cancelled: &dyn Fn() -> bool,
        on_retry: &mut dyn FnMut(u32, u32),
        dropped: &mut Option<([u8; 16], i32)>,
    ) -> Result<RemoteCaps, ConnectFailure> {
        use btstack_sys::consts::{
            ERROR_CODE_COMMAND_DISALLOWED, ERROR_CODE_DIFFERENT_TRANSACTION_COLLISION,
            ERROR_CODE_LMP_ERROR_TRANSACTION_COLLISION,
        };
        let mut repaired = false;
        let mut collided = false;
        let mut attempt = 1;
        loop {
            let err = match self.connect(addr) {
                Ok(caps) => return Ok(caps),
                Err(e) => e,
            };
            tracing::debug!(
                "connect attempt {attempt}/{CONNECT_ATTEMPTS} to {}: {err:?} (re-paired {repaired})",
                addr_str(&addr)
            );
            if cancelled() {
                return Err(ConnectFailure::Cancelled);
            }
            if err == ConnectFailure::AuthFailed && !repaired {
                let status = self.target_auth_status();
                let collision = matches!(
                    status,
                    Some(
                        ERROR_CODE_COMMAND_DISALLOWED
                            | ERROR_CODE_LMP_ERROR_TRANSACTION_COLLISION
                            | ERROR_CODE_DIFFERENT_TRANSACTION_COLLISION
                    )
                );
                if collision && !collided {
                    collided = true;
                    tracing::warn!(
                        "authentication collided with the device's own (0x{:02X}): trying again with the stored link key",
                        status.unwrap_or(0)
                    );
                    on_retry(attempt + 1, CONNECT_ATTEMPTS);
                    self.drop_acl_link(Duration::from_secs(5));
                    for _ in 0..10 {
                        if cancelled() {
                            return Err(ConnectFailure::Cancelled);
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    continue;
                }
                repaired = true;
                tracing::warn!("authentication failed, dropping the stored link key and pairing again");
                tracing::debug!("authentication status {status:02X?}");
                *dropped = self
                    .call(Duration::from_secs(2), move |_, s| {
                        let key = s.link_key(&addr);
                        s.drop_link_key(&addr);
                        key
                    })
                    .flatten();
                on_retry(attempt + 1, CONNECT_ATTEMPTS);
                self.drop_acl_link(Duration::from_secs(5));
                continue;
            }
            let transient = matches!(err, ConnectFailure::NoAnswer | ConnectFailure::LinkLost);
            if !transient || attempt >= CONNECT_ATTEMPTS {
                return Err(err);
            }
            tracing::warn!("connection attempt {attempt}/{CONNECT_ATTEMPTS} failed ({err:?}), trying again");
            attempt += 1;
            on_retry(attempt, CONNECT_ATTEMPTS);
            self.drop_acl_link(Duration::from_secs(5));
            // Give both controllers a moment before paging again
            for _ in 0..10 {
                if cancelled() {
                    return Err(ConnectFailure::Cancelled);
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }

    /// HCI Disconnect of the ACL link to the last remote, waiting for
    /// Disconnection Complete. A link that came up and then went silent
    /// otherwise stays until the supervision timeout, and the next attempt
    /// would reuse it.
    pub fn drop_acl_link(&self, timeout: Duration) {
        if !self.hci_ready() {
            return;
        }
        self.shared.update(|s| s.acl_down = false);
        if self.call(Duration::from_secs(2), |app, s| app.drop_acl(s)) != Some(true) {
            tracing::debug!("drop ACL: no link to drop");
            return;
        }
        tracing::info!("dropping the ACL link before trying again");
        if self.shared.wait(timeout, false, |s| s.acl_down) != Wait::Done {
            tracing::warn!("no Disconnection Complete within {} ms", timeout.as_millis());
        }
    }

    /// Closes an AVDTP connection BTstack still knows for `addr`
    fn release_leftover_avdtp(&self, addr: [u8; 6]) {
        self.shared.update(|s| s.released = false);
        let Some(wait) = self.call(Duration::from_secs(2), move |app, s| app.release_leftover(s, addr)).flatten()
        else {
            return;
        };
        tracing::debug!("leftover AVDTP connection to {} released (wait {wait})", addr_str(&addr));
        // An open signaling channel is released asynchronously; one that
        // never opened is freed at once
        if wait && self.shared.wait(DISCONNECT_TIMEOUT, false, |s| s.released) != Wait::Done {
            tracing::warn!("leftover AVDTP release timed out");
        }
        self.call(Duration::from_secs(2), |app, s| app.forget_connection(s));
        self.shared.update(|s| {
            s.connected = false;
            s.streaming = false;
            s.disconnect_occurred = false;
            s.connect_done = None;
            s.stream_done = None;
        });
    }

    /// After a failed or timed-out connect the AVDTP connection may still be
    /// alive inside BTstack, and the next establish would fail with
    /// COMMAND_DISALLOWED. Also the disconnect of a live one.
    fn abort_pending_connection(&self) {
        self.shared.update(|s| s.released = false);
        let wait = self.call(JOB_TIMEOUT, |app, s| app.abort(s)).unwrap_or(false);
        tracing::debug!("abort pending connection: release wait {wait}");
        // A plain wait, not a cancellable one: cleanup must finish even when
        // a user stop latched the cancel
        if wait && self.shared.wait(DISCONNECT_TIMEOUT, false, |s| s.released) != Wait::Done {
            tracing::warn!("AVDTP signaling release timed out");
        }
        // This was never a live stream: the release / HCI disconnect handlers
        // must not make the streaming loop think a connection was lost
        self.call(Duration::from_secs(2), |app, s| app.forget_connection(s));
        self.shared.update(|s| {
            s.connected = false;
            s.streaming = false;
            s.disconnect_occurred = false;
            s.connect_done = None;
            s.stream_done = None;
        });
    }

    /// SET_CONFIGURATION and wait for the stream. Returns the media payload
    /// size to encode for.
    pub fn configure(&self, config: &Configuration) -> Result<u16, ConfigureError> {
        if !self.shared.lock().connected {
            tracing::debug!("configure: not connected");
            return Err(ConfigureError::NotConnected);
        }
        tracing::debug!("configure: {config:?}");
        self.shared.update(|s| s.stream_done = None);
        let c = config.clone();
        match self.call(JOB_TIMEOUT, move |app, s| app.set_config(s, &c)) {
            Some(0) => {}
            r => {
                tracing::warn!("set configuration failed ({r:?})");
                return Err(ConfigureError::Refused);
            }
        }
        if self.shared.wait(STREAM_TIMEOUT, true, |s| s.stream_done.is_some()) != Wait::Done {
            tracing::warn!("stream configuration timed out or cancelled");
            return Err(ConfigureError::NoAnswer);
        }
        let s = self.shared.lock();
        if s.stream_done != Some(true) {
            tracing::debug!("configure: stream not established ({:?})", s.stream_done);
            return Err(ConfigureError::Refused);
        }
        let mtu = s.media_mtu;
        drop(s);
        tracing::debug!("configure: stream established, media payload {mtu}");
        if let Some(addr) = *self.remote_addr.lock().unwrap_or_else(|e| e.into_inner()) {
            *self.last.lock().unwrap_or_else(|e| e.into_inner()) = Some(LastStream { addr, config: config.clone() });
        }
        Ok(mtu)
    }

    /// AVDTP Start; waits until it is acknowledged
    pub fn start_stream(&self) -> bool {
        if !self.shared.lock().connected {
            tracing::debug!("start stream: not connected");
            return false;
        }
        self.shared.update(|s| s.start_done = None);
        match self.call(JOB_TIMEOUT, |app, s| app.start_stream(s)) {
            Some(0) => {}
            r => {
                tracing::warn!("start stream failed ({r:?})");
                return false;
            }
        }
        if self.shared.wait(STREAM_TIMEOUT, true, |s| s.start_done.is_some()) != Wait::Done {
            tracing::warn!("stream start timed out or cancelled");
            return false;
        }
        let done = self.shared.lock().start_done;
        tracing::debug!("start stream: {done:?}");
        done == Some(true)
    }

    /// AVDTP Suspend
    pub fn stop_stream(&self) {
        if !self.shared.lock().streaming {
            return;
        }
        tracing::debug!("stop stream: suspending");
        self.call(JOB_TIMEOUT, |app, s| app.pause_stream(s));
        self.shared.update(|s| s.streaming = false);
    }

    pub fn disconnect(&self) {
        self.stop_stream();
        tracing::info!("disconnecting A2DP");
        self.abort_pending_connection();
    }

    pub fn is_connected(&self) -> bool {
        self.shared.lock().connected
    }

    pub fn is_streaming(&self) -> bool {
        self.shared.lock().streaming
    }

    /// The headphones ended the last lost connection themselves: turned off
    /// (Remote Device Terminated due to Power Off) or switched away by their
    /// user (Remote User Terminated). Not a link that faded (out of range).
    pub fn lost_to_the_device(&self) -> bool {
        matches!(self.shared.lock().link_lost_reason, 0x13 | 0x15)
    }

    /// The device opened a link to the PC since the last call (resets), as
    /// headphones turned back on do
    pub fn device_came_back(&self, addr: [u8; 6]) -> bool {
        self.link_came_up() == Some(addr)
    }

    /// The device that last opened a link to the PC since the last call
    /// (resets), whichever it is
    pub fn link_came_up(&self) -> Option<[u8; 6]> {
        self.shared.update(|s| std::mem::take(&mut s.acl_up_from))
    }

    /// The device opened an ACL link to the PC itself (turned on, it
    /// connected to the PC), it is still up, and no stream ran on it yet.
    /// Not the link of a stream that just ended. Claims it: true once.
    pub fn take_link_from_device(&self, addr: [u8; 6]) -> bool {
        self.shared
            .update(|s| s.acl_links.iter_mut().find(|(_, a, own)| *a == addr && *own).map(|l| l.2 = false).is_some())
    }

    /// Once per lost connection (resets)
    pub fn check_disconnected(&self) -> bool {
        self.shared.update(|s| std::mem::take(&mut s.disconnect_occurred))
    }

    /// Ends blocking waits (a user stop). Stays in effect for reconnect
    /// attempts until the next user-initiated connect or scan.
    pub fn cancel_pending_waits(&self) {
        self.shared.update(|s| s.cancel = true);
    }

    pub fn media_mtu(&self) -> u16 {
        match self.shared.lock().media_mtu {
            0 => 679,
            m => m,
        }
    }

    pub fn remote_caps(&self) -> RemoteCaps {
        self.shared.lock().remote_caps.clone()
    }

    /// Connects again to the last device and starts the stream. The
    /// configuration is built anew from the capabilities the device lists
    /// now by `rebuild` (its SEIDs or codec settings may have changed, as v1
    /// read them afresh); None from it: the device no longer offers the
    /// stream.
    pub fn reconnect(
        &self,
        rebuild: &dyn Fn(&RemoteCaps) -> Option<Configuration>,
    ) -> Result<Reconnected, ReconnectError> {
        let Some((addr, config)) =
            self.last.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|l| (l.addr, l.config.clone()))
        else {
            tracing::warn!("cannot reconnect: no earlier stream");
            return Err(ReconnectError::Failed);
        };
        if !self.hci_ready() || self.stalled() {
            tracing::warn!("cannot reconnect: the Bluetooth adapter stopped working");
            return Err(ReconnectError::Failed);
        }
        // The cancel of a user stop stays latched across reconnect attempts:
        // a stop arriving between attempts must not be swallowed
        if self.shared.lock().cancel {
            tracing::info!("reconnect aborted (cancel requested)");
            return Err(ReconnectError::Failed);
        }
        self.shared.update(|s| {
            s.connected = false;
            s.streaming = false;
            s.remote_caps = Default::default();
            s.disconnect_occurred = false;
            s.connect_done = None;
            s.stream_done = None;
            s.start_done = None;
            s.last_connect_failure = None;
        });
        tracing::info!("reconnecting to {}", addr_str(&addr));
        tracing::debug!("reconnect: previous configuration {config:?}");
        self.release_leftover_avdtp(addr);
        let Ok(remote) = self.establish_and_wait(addr) else {
            tracing::warn!("reconnection failed");
            return Err(ReconnectError::Failed);
        };
        self.configure_and_start(remote, &config, rebuild)
    }

    /// The stream of a connection just (re)established, with a configuration
    /// built from the capabilities it reported: what [`BtHost::reconnect`]
    /// does after connecting, for a caller that connected with
    /// [`BtHost::connect`] (a new stack)
    pub fn configure_and_start(
        &self,
        remote: RemoteCaps,
        before: &Configuration,
        rebuild: &dyn Fn(&RemoteCaps) -> Option<Configuration>,
    ) -> Result<Reconnected, ReconnectError> {
        let Some(config) = rebuild(&remote) else {
            tracing::warn!("reconnect: the device no longer offers {}", before.kind.name());
            self.disconnect();
            return Err(ReconnectError::CodecGone);
        };
        if config != *before {
            tracing::info!("reconnect: the device's capabilities changed: {before:?} -> {config:?}");
        }
        let mtu = match self.configure(&config) {
            Ok(m) => m,
            Err(_) => {
                tracing::warn!("reconnect: codec configuration failed");
                self.disconnect();
                return Err(ReconnectError::Failed);
            }
        };
        if !self.start_stream() {
            tracing::warn!("reconnect: stream start failed");
            self.disconnect();
            return Err(ReconnectError::Failed);
        }
        tracing::info!("reconnected and streaming (mtu {mtu})");
        Ok(Reconnected { remote, config, mtu })
    }

    // ---- volume, buttons, AFH ----

    /// Headphone volume over AVRCP absolute volume (0..127): the last value
    /// they reported, None while unknown
    pub fn remote_volume(&self) -> Option<u8> {
        u8::try_from(self.controls.volume.load(Ordering::Acquire)).ok()
    }

    /// For the encode thread: true while the headphones' volume is 0 (sent
    /// or reported). Many headphones still play faintly at absolute volume
    /// 0, so the audio is silenced there too (issue #86).
    pub fn volume_is_zero(&self) -> impl Fn() -> bool + Send + 'static {
        let controls = self.controls.clone();
        move || controls.volume.load(Ordering::Acquire) == 0
    }

    /// Sets the headphones' volume; while one request is in flight, newer
    /// values replace the queued one
    pub fn request_volume(&self, volume: u8) {
        tracing::debug!("volume request: {volume}");
        self.controls.volume_request.store(volume.min(127) as i32, Ordering::Release);
        self.post(|app, s| app.send_pending_volume(s));
    }

    /// Volume to set as soon as AVRCP connects, before any audio (None: keep
    /// the headphones' own). Follows the headphones' volume when the link
    /// drops, so a reconnect restores the last one.
    pub fn set_restore_volume(&self, volume: Option<u8>) {
        tracing::debug!("restore volume: {volume:?}");
        self.controls.restore_volume.store(volume.map_or(-1, |v| v.min(127) as i32), Ordering::Release);
    }

    pub fn restore_volume(&self) -> Option<u8> {
        u8::try_from(self.controls.restore_volume.load(Ordering::Acquire)).ok()
    }

    /// Headphone buttons as media keys ([`HostEvent::MediaKey`]), default on
    pub fn set_media_keys(&self, enabled: bool) {
        tracing::debug!("media keys: {enabled}");
        self.controls.media_keys.store(enabled, Ordering::Release);
    }

    /// AFH policy: "auto", "off" or Wi-Fi channels such as "6,11"; takes
    /// effect within seconds, also on a running link
    pub fn set_afh_policy(&self, policy: &str) {
        let mut p = self.controls.afh_policy.lock().unwrap_or_else(|e| e.into_inner());
        if *p != policy {
            tracing::debug!("AFH policy: {:?} -> {policy:?}", *p);
            *p = policy.to_string();
            self.controls.afh_version.fetch_add(1, Ordering::AcqRel);
        }
    }

    /// Upper limit of the media payload (clamped to the valid range); applies
    /// to streams established afterwards
    pub fn set_media_payload_limit(&self, limit: u16) {
        tracing::debug!("media payload limit: {limit} (clamped {})", media::clamp_media_payload_limit(limit as u32));
        self.controls
            .media_payload_limit
            .store(media::clamp_media_payload_limit(limit as u32) as u32, Ordering::Release);
    }

    pub fn is_tcp(&self) -> bool {
        self.tcp
    }

    /// Powers HCI off (releasing the adapter, also while it is still
    /// initializing) and stops the BTstack thread. Afterwards every call
    /// fails; a new stack needs a new BtHost.
    pub fn shutdown(&self) {
        self.stop_thread(false);
    }

    /// Shutdown for an adapter that went away as expected (the PC is going
    /// to sleep): no HCI power off, which cannot reach it any more, and a
    /// thread still closing the device is left to end on its own (after
    /// resume) without an error
    pub fn shutdown_lost(&self) {
        self.stop_thread(true);
    }

    fn stop_thread(&self, lost: bool) {
        let Some(thread) = self.thread.lock().unwrap_or_else(|e| e.into_inner()).take() else { return };
        if lost {
            tracing::info!("the Bluetooth adapter went away, closing it without HCI power off");
        } else {
            self.power_off();
        }
        self.remote.request_exit();
        // The run loop deinitializes BTstack and closes the transport. The
        // wait counts steps, not the clock: Windows' clock goes on in sleep,
        // and the PC may sleep right in this wait
        let mut steps = 0;
        while !thread.is_finished() && steps < THREAD_END_STEPS {
            std::thread::sleep(THREAD_END_STEP);
            steps += 1;
        }
        if thread.is_finished() {
            let _ = thread.join();
        } else if lost {
            tracing::info!("the BTstack thread is still closing the Bluetooth adapter");
        } else {
            tracing::error!("the BTstack thread did not end");
        }
        self.shared.update(|s| {
            s.hci_ready = false;
            s.connected = false;
            s.streaming = false;
        });
        tracing::info!("BTstack shut down");
    }

    fn power_off(&self) {
        tracing::info!("powering off HCI");
        self.shared.update(|s| s.hci_off = false);
        let dispatched = self.call(Duration::from_secs(3), |_, s| {
            if s.hci_initialized() && s.hci_state() != btstack_sys::HciState::Off {
                s.power_off();
                true
            } else {
                false
            }
        });
        match dispatched {
            Some(true) => {
                if self.shared.wait(Duration::from_secs(5), false, |s| s.hci_off) == Wait::Done {
                    tracing::info!("HCI powered off");
                } else {
                    tracing::warn!("HCI power off timed out");
                }
            }
            Some(false) => tracing::debug!("power off: HCI not on, nothing to do"),
            None => tracing::warn!("the run loop did not take the power off request"),
        }
    }
}

impl Drop for BtHost {
    fn drop(&mut self) {
        self.stop_thread(false);
    }
}
