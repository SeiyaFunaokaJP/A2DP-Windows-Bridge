//! The service: owns the Bluetooth stack (started on demand, restarted when
//! it stops working or after sleep), runs streaming sessions on their own
//! thread and reports what happens as [`ServiceEvent`]s.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use bt_host::{
    BtHost, DiscoveredDevice, HciCapture, HostConfig, HostEvent, InitFailure, LeConfig, LeScanResult, LinkStats,
    TransportConfig,
};
use hci_transport::UsbSelector;

use crate::devices::SavedDevice;
use crate::messages::Msg;
use crate::settings::Settings;
use crate::stream::{self, SourceRequest, StreamInfo, StreamRequest, StreamState};

/// Wait for the controller to answer when checking a stack before use
const CONTROLLER_PROBE: Duration = Duration::from_secs(2);
/// Time for a USB adapter to come back after resume before it is opened
const RESUME_SETTLE: Duration = Duration::from_secs(3);
/// Waiting for the resume after the adapter went away for sleep: ticks of
/// [`RESUME_TICK`], so about a minute while the PC stays awake (a wait across
/// sleep ends once on wake, whatever the clock says)
const RESUME_TICK: Duration = Duration::from_millis(200);
const RESUME_WAIT_TICKS: u32 = 300;
/// LE Audio (experimental): the LE scan after the classic one (v1)
const LE_SCAN: Duration = Duration::from_secs(5);
/// A link younger than this keeps the stack ([`Service::host_for_stream`])
const FRESH_LINK: Duration = Duration::from_secs(60);
/// A connection started during a scan waits this long for the scan to end
/// (a name request being cancelled takes up to ~2 s)
const SCAN_END_WAIT: Duration = Duration::from_secs(5);
/// ... once the LE core is ready (HCI works, so at once or a moment later)
const LE_SCAN_READY_WAIT: Duration = Duration::from_secs(1);

/// MMCSS "Pro Audio" for the calling thread (the priority hooks). Said in
/// the log either way: a thread that runs without it is one place a
/// stutter can come from (issue #84).
fn pro_audio(thread: &str) -> Box<dyn std::any::Any> {
    let guard = match win_sys::mmcss::MmcssGuard::register(win_sys::mmcss::MmcssTask::ProAudio) {
        Ok(g) => {
            tracing::info!("{thread}: MMCSS Pro Audio (task {})", g.task_index());
            Some(g)
        }
        Err(e) => {
            tracing::warn!("{thread}: MMCSS registration failed, normal priority: {e}");
            None
        }
    };
    Box::new(guard)
}

/// LE Audio (experimental) from the command line (`--le-audio` and the
/// test options): LE is on whatever the setting says
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LeCliOptions {
    /// `--mcp-dry-run` (implied by `--hci-tcp`): MCP answers from a
    /// simulated player and only logs the commands
    pub mcp_dry_run: bool,
    /// `--ccp-sim "<t>:<cmd>;..."`
    pub ccp_sim: Option<String>,
    /// `--ccp-console`
    pub ccp_console: bool,
}

/// A device of a scan ([`Service::scan_devices`]): the classic ones, then
/// the LE Audio ones (experimental). A dual-mode device has two entries: its
/// classic entry is never an LE Audio one.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ScannedDevice {
    /// XX:XX:XX:XX:XX:XX
    pub address: String,
    pub name: String,
    pub rssi: i8,
    /// Classic: the class of device; 0 for LE Audio entries
    pub class_of_device: u32,
    /// Classic: Audio/Video class; LE Audio entries are audio devices
    pub audio: bool,
    /// An LE Audio entry
    pub le_audio: bool,
    /// LE Audio: member of a coordinated set (advertises a CSIP RSI);
    /// connecting it streams to the whole set
    pub le_audio_set: bool,
}

impl ScannedDevice {
    fn classic(d: &DiscoveredDevice) -> ScannedDevice {
        ScannedDevice {
            address: bt_host::addr_str(&d.addr),
            name: d.name.clone(),
            rssi: d.rssi,
            class_of_device: d.class_of_device,
            audio: d.is_audio(),
            le_audio: false,
            le_audio_set: false,
        }
    }
}

/// The LE Audio entries of an LE scan (v1 A2dpService scan thread): LE
/// Audio advertisers only; a saved LE Audio device is listed (with its new
/// name); of a coordinated set nobody saved, one member (connecting it
/// finds the others). Classic entries of the same address are another
/// list. Returns the entries and the number of LE Audio advertisers seen.
fn le_scan_entries(results: &[LeScanResult], saved_le: &[SavedDevice]) -> (Vec<ScannedDevice>, usize) {
    let mut out: Vec<ScannedDevice> = Vec::new();
    let mut sets_listed: Vec<u32> = Vec::new();
    let mut found = 0;
    for r in results.iter().filter(|r| r.le_audio) {
        found += 1;
        let address = r.addr.to_string();
        let set_member = r.rsi.is_some();
        if let Some(e) = out.iter_mut().find(|e| e.address == address) {
            // Seen twice (advertising and scan response)
            if !r.name.is_empty() {
                e.name = r.name.clone();
            }
            e.le_audio_set |= set_member;
            continue;
        }
        let saved = saved_le.iter().any(|d| d.address.eq_ignore_ascii_case(&address));
        if !saved && let Some(set) = r.set_id {
            if sets_listed.contains(&set) {
                continue;
            }
            sets_listed.push(set);
        }
        out.push(ScannedDevice {
            address,
            name: r.name.clone(),
            rssi: r.rssi,
            class_of_device: 0,
            audio: true,
            le_audio: true,
            le_audio_set: set_member,
        });
    }
    (out, found)
}

/// Sleep and resume as the service sees them (set by the power
/// notifications, read by the session)
#[derive(Debug, Default)]
pub(crate) struct PowerState {
    /// Between the going-to-sleep notification and the resume: the adapter
    /// goes away, so transport failures and a dropped link are expected and
    /// nothing is restarted until the PC is back
    suspending: AtomicBool,
    /// The PC resumed from sleep since the stack started: the adapter lost
    /// power or was re-enumerated, so the stack is restarted before the
    /// next use (v1 needed an app restart)
    stale: AtomicBool,
    resumed_at: Mutex<Option<Instant>>,
}

impl PowerState {
    /// The going-to-sleep notification; false when it was seen already
    pub fn suspend(&self) -> bool {
        !self.suspending.swap(true, Ordering::AcqRel)
    }

    /// A resume notification; true for the first of one resume (Windows
    /// sends two), which marks the stack stale
    pub fn resume(&self) -> bool {
        let was_suspending = self.suspending.swap(false, Ordering::AcqRel);
        let was_stale = self.stale.swap(true, Ordering::AcqRel);
        let first = was_suspending || !was_stale;
        if first {
            *self.resumed_at.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
        }
        first
    }

    /// Going to sleep: the adapter is going away
    pub fn suspending(&self) -> bool {
        self.suspending.load(Ordering::Acquire)
    }

    /// The running stack is left from before sleep
    pub fn stale(&self) -> bool {
        self.stale.load(Ordering::Acquire)
    }

    /// A new stack starts
    fn clear_stale(&self) {
        self.stale.store(false, Ordering::Release);
    }

    /// No resume came after all: losses count as failures again
    fn end_suspend(&self) {
        self.suspending.store(false, Ordering::Release);
    }

    fn resumed_at(&self) -> Option<Instant> {
        *self.resumed_at.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ServiceEvent {
    State(StreamState, Msg),
    StreamInfo(StreamInfo),
    /// The headphones reported their volume (0..127)
    Volume(u8),
    /// The headphones reported their battery level (percent)
    Battery(u8),
    /// The headphones refused to report their battery level (HFP refused,
    /// another device may hold it)
    BatteryRefused,
    /// While the session waited for its headphones that went off, another
    /// saved device connected to the PC (its address): its session may
    /// start instead
    OtherDeviceBack([u8; 6]),
}

pub struct ServiceOptions {
    pub config_dir: PathBuf,
    /// H4 over TCP to a virtual controller instead of USB (tools/emu)
    pub hci_tcp: Option<String>,
    pub capture: HciCapture,
    /// AFH policy instead of the settings' (command line)
    pub afh: Option<String>,
    /// The USB adapter to open (the command line's -u); default: the
    /// first one that works. Not used with hci_tcp.
    pub usb: UsbSelector,
}

/// The LE Audio configuration of the next stack, None: LE off
fn le_config(le_on: bool, cli: Option<&LeCliOptions>, opts: &ServiceOptions) -> Option<LeConfig> {
    if !le_on {
        return None;
    }
    // --mcp-dry-run, implied by --hci-tcp (the emulator): the simulated
    // player answers, nothing in Windows is read or controlled
    let dry_run = cli.is_some_and(|c| c.mcp_dry_run) || opts.hci_tcp.is_some();
    Some(LeConfig {
        bonds_file: Some(crate::paths::le_bonds_file(&opts.config_dir)),
        bond_dir: Some(crate::paths::pairing_dir(&opts.config_dir)),
        media: if dry_run { None } else { crate::le_media::windows_backend() },
        ccp_sim: cli.and_then(|c| c.ccp_sim.clone()),
        ccp_console: cli.is_some_and(|c| c.ccp_console),
    })
}

pub(crate) struct Inner {
    opts: ServiceOptions,
    pub settings: Mutex<Settings>,
    events: Mutex<Sender<ServiceEvent>>,
    host: Mutex<Option<Arc<BtHost>>>,
    stats: Arc<LinkStats>,
    stop: AtomicBool,
    running: AtomicBool,
    pub power: PowerState,
    tick: (Mutex<()>, Condvar),
    state: Mutex<StreamState>,
    /// What each device (address) offered at its last connection, also
    /// in device_codecs.json
    codecs: Mutex<BTreeMap<String, stream::KnownCodecs>>,
    /// The chip or firmware setting changed while the stack was in use:
    /// the next use starts a new stack (which loads the firmware again)
    reload: AtomicBool,
    /// LE Audio forced on by the command line
    le_cli: Mutex<Option<LeCliOptions>>,
    /// The running stack was started with LE Audio
    le_active: AtomicBool,
    /// A device scan runs
    scanning: AtomicBool,
    /// The LE Audio stream being sent (le_stream)
    le_stream: Mutex<Option<crate::le_stream::LeStreamDetails>>,
    /// An A2DP stream ran on the running stack (issue #84)
    streamed: AtomicBool,
    /// The running adapter has LE Audio's isochronous channels: known once
    /// a stack started on it (issue #86)
    le_support: Mutex<Option<bool>>,
    /// The session waits for headphones that were turned off: a scan may
    /// end it
    waiting: AtomicBool,
    /// The headphones of the last session were turned off and did not come
    /// back while it ran: a Bluetooth restart does not reconnect them
    /// (issues #86, #117)
    turned_off: AtomicBool,
    /// The source the running A2DP session changes to at its next tick
    source_change: Mutex<Option<SourceRequest>>,
}

impl Inner {
    pub fn stop_requested(&self) -> bool {
        self.stop.load(Ordering::Acquire)
    }

    pub fn take_source_change(&self) -> Option<SourceRequest> {
        self.source_change.lock().unwrap_or_else(|e| e.into_inner()).take()
    }

    /// The session waits for headphones that were turned off
    pub fn waiting_for_device(&self) {
        self.turned_off.store(true, Ordering::Release);
        self.waiting.store(true, Ordering::Release);
    }

    /// The wait ended: `back`, the headphones connected again
    pub fn done_waiting_for_device(&self, back: bool) {
        if back {
            self.turned_off.store(false, Ordering::Release);
        }
        self.waiting.store(false, Ordering::Release);
    }

    pub fn notify(&self, state: StreamState, msg: Msg) {
        tracing::info!("state {state:?}: {}", msg.english());
        *self.state.lock().unwrap_or_else(|e| e.into_inner()) = state;
        // What went over the air before a failure or a lost connection
        if matches!(state, StreamState::Error | StreamState::Reconnecting) {
            let dir = crate::paths::recorder_dir(&self.opts.config_dir);
            let what = format!("{state:?}-{}", msg.key.trim_start_matches("error."));
            self.opts.capture.save_recent(&dir, &crate::paths::local_stamp(), &what, false);
        }
        let _ = self.events.lock().unwrap_or_else(|e| e.into_inner()).send(ServiceEvent::State(state, msg));
    }

    /// The settings folder (devices, profiles, pairings)
    pub fn config_dir(&self) -> &std::path::Path {
        &self.opts.config_dir
    }

    pub fn other_device_back(&self, addr: [u8; 6]) {
        let _ = self.events.lock().unwrap_or_else(|e| e.into_inner()).send(ServiceEvent::OtherDeviceBack(addr));
    }

    pub fn stream_info(&self, info: StreamInfo) {
        let _ = self.events.lock().unwrap_or_else(|e| e.into_inner()).send(ServiceEvent::StreamInfo(info));
    }

    /// Sleeps up to `d`, returning early on a stop
    pub fn wait_tick(&self, d: Duration) {
        let guard = self.tick.0.lock().unwrap_or_else(|e| e.into_inner());
        if !self.stop_requested() {
            let _ = self.tick.1.wait_timeout(guard, d);
        }
    }

    fn wait_after_resume(&self) {
        let Some(at) = self.power.resumed_at() else { return };
        let since = at.elapsed();
        if since >= RESUME_SETTLE {
            return;
        }
        tracing::info!("waiting {} ms for the adapter after resume", (RESUME_SETTLE - since).as_millis());
        while at.elapsed() < RESUME_SETTLE && !self.stop_requested() {
            std::thread::sleep(Duration::from_millis(100));
        }
        tracing::debug!(
            "resume settle: done {} ms after the resume (stop requested {})",
            at.elapsed().as_millis(),
            self.stop_requested()
        );
    }

    /// LE Audio (experimental) for the next stack: the setting, or the
    /// command line's --le-audio
    pub fn le_wanted(&self) -> bool {
        self.le_cli.lock().unwrap_or_else(|e| e.into_inner()).is_some()
            || self.settings.lock().unwrap_or_else(|e| e.into_inner()).le_audio_experimental
    }

    /// The running stack has LE Audio
    pub fn le_active(&self) -> bool {
        self.le_active.load(Ordering::Acquire)
    }

    fn host_config(&self, le_on: bool) -> HostConfig {
        let s = self.settings.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let transport = match &self.opts.hci_tcp {
            Some(addr) => TransportConfig::Tcp(addr.clone()),
            // bt-host adds the adapters it knows to `known`
            None => TransportConfig::Usb(self.opts.usb.clone()),
        };
        let mut c = HostConfig::new(transport);
        c.link_key_file = Some(crate::paths::link_keys_file(&self.opts.config_dir));
        c.firmware_dir = Some(crate::paths::firmware_dir(&self.opts.config_dir));
        c.capture = self.opts.capture.clone();
        crate::adapter::configure(&mut c, &s, &self.opts.config_dir);
        let cli = self.le_cli.lock().unwrap_or_else(|e| e.into_inner()).clone();
        c.le_audio = le_config(le_on, cli.as_ref(), &self.opts);
        c.battery = true;
        tracing::debug!(
            "host config: transport {:?}, firmware dir {:?}, realtek pid {:04x} files {:?}, intel loader {}, broadcom ids {}, LE {} (cli {:?})",
            c.transport,
            c.firmware_dir,
            c.realtek_pid,
            c.realtek_files,
            c.intel_loader,
            c.broadcom_ids.len(),
            c.le_audio.is_some(),
            cli
        );
        c
    }

    fn start_host(self: &Arc<Self>) -> Result<Arc<BtHost>, Msg> {
        // A controller that did not answer at all is opened once more right
        // away: opening it resets its USB pipes, which can be enough
        let attempts = 2;
        let mut last = InitFailure::Failed;
        let le_on = self.le_wanted();
        if le_on {
            tracing::info!("LE Audio (experimental) is on: BTstack starts with LE");
        }
        for attempt in 1..=attempts {
            if attempt > 1 {
                tracing::warn!("the Bluetooth adapter did not answer, opening it again ({attempt}/{attempts})");
            }
            let t = Instant::now();
            let started = BtHost::start(self.host_config(le_on), self.stats.clone());
            tracing::debug!(
                "host start: attempt {attempt}/{attempts} {} after {} ms",
                if started.is_ok() { "ok" } else { "failed" },
                t.elapsed().as_millis()
            );
            match started {
                Ok(h) => {
                    let h = Arc::new(h);
                    let s = self.settings.lock().unwrap_or_else(|e| e.into_inner()).clone();
                    h.set_media_keys(s.media_keys);
                    h.set_afh_policy(self.opts.afh.as_deref().unwrap_or(&s.afh));
                    self.le_active.store(le_on, Ordering::Release);
                    self.streamed.store(false, Ordering::Release);
                    self.note_le_capable(&h);
                    self.pump_events(&h);
                    return Ok(h);
                }
                Err(f) => {
                    tracing::debug!("host start: {f:?}");
                    last = f;
                    if f != InitFailure::NoAnswer {
                        break;
                    }
                }
            }
        }
        Err(Msg::new(if last == InitFailure::NoAnswer { "error.btstack_no_answer" } else { "error.btstack_init" }))
    }

    /// Remembers whether the adapter the stack opened has LE Audio's
    /// isochronous channels, also for the next start (adapter_caps.json)
    fn note_le_capable(&self, host: &Arc<BtHost>) {
        let capable = host.le_audio_capable();
        tracing::debug!("adapter: LE Audio capable {capable}, opened {:?}", hci_transport::usb::last_opened());
        *self.le_support.lock().unwrap_or_else(|e| e.into_inner()) = Some(capable);
        if self.opts.hci_tcp.is_none()
            && let Some((vid, pid)) = hci_transport::usb::last_opened()
        {
            crate::adapter::save_le_audio(&self.opts.config_dir, vid, pid, capable);
            // The address comes with the controller's answer, a moment after init
            let dir = self.opts.config_dir.clone();
            let host = host.clone();
            let _ = std::thread::Builder::new().name("adapter-address".into()).spawn(move || {
                for _ in 0..20 {
                    if let Some(a) = host.local_address() {
                        crate::adapter::save_address(&dir, vid, pid, &a);
                        // Read Local Version is answered before Read BD_ADDR
                        match host.hci_version() {
                            Some(v) => crate::adapter::save_hci_version(&dir, vid, pid, v),
                            None => tracing::debug!("adapter: no HCI version, not saved"),
                        }
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(250));
                }
                tracing::debug!("adapter: no local address after 5 s, not saved");
            });
        }
    }

    /// Headphone buttons to Windows media keys, volume reports to the
    /// service's events
    fn pump_events(self: &Arc<Self>, host: &Arc<BtHost>) {
        let Some(rx) = host.take_events() else { return };
        let weak: Weak<Inner> = Arc::downgrade(self);
        let _ = std::thread::Builder::new().name("host-events".into()).spawn(move || {
            while let Ok(e) = rx.recv() {
                match e {
                    HostEvent::MediaKey(k) => {
                        tracing::debug!("host event: media key {k:?}");
                        let key = match k {
                            bt_host::MediaKey::PlayPause => win_sys::media_keys::MediaKey::PlayPause,
                            bt_host::MediaKey::Stop => win_sys::media_keys::MediaKey::Stop,
                            bt_host::MediaKey::Next => win_sys::media_keys::MediaKey::Next,
                            bt_host::MediaKey::Previous => win_sys::media_keys::MediaKey::Previous,
                        };
                        if let Err(e) = win_sys::media_keys::send(key) {
                            tracing::warn!("media key {k:?}: {e}");
                        }
                    }
                    HostEvent::Volume(v) => {
                        tracing::debug!("host event: device volume {v}");
                        if let Some(inner) = weak.upgrade() {
                            let _ =
                                inner.events.lock().unwrap_or_else(|e| e.into_inner()).send(ServiceEvent::Volume(v));
                        }
                    }
                    HostEvent::Battery(v) => {
                        tracing::debug!("host event: battery {v}%");
                        if let Some(inner) = weak.upgrade() {
                            let _ =
                                inner.events.lock().unwrap_or_else(|e| e.into_inner()).send(ServiceEvent::Battery(v));
                        }
                    }
                    HostEvent::BatteryRefused => {
                        tracing::debug!("host event: battery refused");
                        if let Some(inner) = weak.upgrade() {
                            let _ = inner
                                .events
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .send(ServiceEvent::BatteryRefused);
                        }
                    }
                    HostEvent::ConnectionLost | HostEvent::TransportFailed => {
                        tracing::debug!("host event: {e:?}");
                    }
                }
            }
            tracing::debug!("host events: channel closed");
        });
    }

    /// The running stack, started (or restarted) as needed
    pub fn ensure_host(self: &Arc<Self>) -> Result<Arc<BtHost>, Msg> {
        let mut slot = self.host.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(h) = slot.as_ref() {
            // A stack that stopped working stays so until it is started anew
            // (v1 issues #42, #43: only an app restart helped)
            let le_wanted = self.le_wanted();
            let why = if self.power.stale() {
                Some(stream::RESUMED)
            } else if self.reload.load(Ordering::Acquire) {
                Some("the adapter settings changed")
            } else if self.le_active() != le_wanted {
                // Turned on or off while streaming or scanning: applied now,
                // before the next scan or connection (v1)
                Some(if le_wanted {
                    "LE Audio (experimental) was turned on"
                } else {
                    "LE Audio (experimental) was turned off"
                })
            } else if !h.controller_responds(CONTROLLER_PROBE) {
                Some("the Bluetooth adapter stopped answering")
            } else {
                None
            };
            match why {
                None => {
                    tracing::trace!("ensure host: the running stack is fine");
                    return Ok(h.clone());
                }
                Some(why) => {
                    tracing::warn!("restarting BTstack: {why}");
                    h.shutdown();
                    *slot = None;
                }
            }
        }
        tracing::debug!("ensure host: starting a new stack");
        self.power.clear_stale();
        self.reload.store(false, Ordering::Release);
        self.wait_after_resume();
        let t = Instant::now();
        let h = self.start_host()?;
        tracing::debug!("ensure host: stack up in {} ms", t.elapsed().as_millis());
        *slot = Some(h.clone());
        Ok(h)
    }

    /// The stack for a new A2DP stream. One a stream already ran on is
    /// replaced: streams after a Disconnect / Connect on the same stack
    /// stuttered until the app was restarted (issue #84), and a new stack
    /// (HCI Reset; a running Realtek firmware is kept) is what a restart
    /// gave. While idle the old stack stays up, for headphones that connect
    /// by themselves. Kept when the device to stream connected to the PC by
    /// itself on it: the HCI Reset would drop that link, and headphones that
    /// just connected then refused the PC's page for 30 s to 2 min (issue
    /// #86). So is a link that came up moments ago ([`FRESH_LINK`]): a codec
    /// chosen right after connecting. An older link left from the stream
    /// that just ended does not count.
    pub fn host_for_stream(self: &Arc<Self>, device: [u8; 6]) -> Result<Arc<BtHost>, Msg> {
        let old = self.host.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let streamed = self.streamed.swap(false, Ordering::AcqRel);
        if let Some(h) = old {
            // Claimed by this stream either way: the next one replaces the stack
            let own_link = h.take_link_from_device(device);
            if streamed && own_link {
                tracing::info!("keeping BTstack: {} connected to the PC by itself on it", bt_host::addr_str(&device));
            } else if streamed && link_is_fresh(h.link_age(device)) {
                tracing::info!("keeping BTstack: the link to {} just came up", bt_host::addr_str(&device));
            } else if streamed {
                tracing::info!("restarting BTstack: a stream ran on it before");
                self.shutdown_host();
            }
        }
        self.ensure_host()
    }

    /// The running stack carried an A2DP stream to `device`: the next one
    /// starts on a new stack, also when the device opened its link again
    /// during the stream (turned off and on)
    pub fn mark_streamed(&self, device: [u8; 6]) {
        if let Some(h) = self.host.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            h.take_link_from_device(device);
        }
        self.streamed.store(true, Ordering::Release);
    }

    /// Shuts the stack down and starts a new one (the session thread, for a
    /// stack that stopped working or is left from before sleep)
    pub fn restart_host(self: &Arc<Self>, why: &str, max_media_payload: u16) -> Result<Arc<BtHost>, Msg> {
        if why == stream::RESUMED {
            tracing::info!("restarting BTstack: {why}");
        } else {
            tracing::warn!("restarting BTstack: {why}");
        }
        self.shutdown_host();
        let h = self.ensure_host()?;
        tracing::debug!("restart host: media payload limit {max_media_payload}");
        h.set_media_payload_limit(max_media_payload);
        Ok(h)
    }

    /// The PC is going to sleep and the stack lost (or is losing) its
    /// adapter: shut down without the noise of a failure; after the resume
    /// a new one starts
    pub fn shutdown_host_for_sleep(&self) {
        let old = self.host.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(h) = old {
            tracing::info!("stopping BTstack until the PC resumes");
            h.shutdown_lost();
        }
    }

    /// Waits for the resume notification while the PC goes to sleep. Gives
    /// up after about a minute awake without one (the stack is then
    /// restarted as after a failure). False when the session was stopped.
    pub fn wait_for_resume(&self) -> bool {
        tracing::debug!("waiting for the resume (up to {RESUME_WAIT_TICKS} ticks awake)");
        let mut ticks = 0;
        while self.power.suspending() && !self.stop_requested() {
            if ticks == RESUME_WAIT_TICKS {
                tracing::warn!("no resume notification after going to sleep");
                self.power.end_suspend();
                bt_host::expect_transport_loss(false);
                break;
            }
            self.wait_tick(RESUME_TICK);
            ticks += 1;
        }
        tracing::debug!("resume wait over after {ticks} tick(s), stop requested {}", self.stop_requested());
        !self.stop_requested()
    }

    fn shutdown_host(&self) {
        let old = self.host.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(h) = old {
            let t = Instant::now();
            // Also when the session still holds the Arc: the adapter must be
            // free for the next stack
            h.shutdown();
            tracing::debug!("host shutdown: took {} ms", t.elapsed().as_millis());
        }
    }

    /// Whether the headphones' buttons act as media keys (one setting for
    /// every device, issue #86)
    pub fn media_keys(&self) -> bool {
        self.settings.lock().unwrap_or_else(|e| e.into_inner()).media_keys
    }

    pub fn saved_volume(&self, addr: &str) -> Option<u8> {
        let s = self.settings.lock().unwrap_or_else(|e| e.into_inner());
        if !s.remember_volume {
            return None;
        }
        s.saved_volumes.get(&addr.to_uppercase()).and_then(|&v| u8::try_from(v).ok())
    }

    /// Keeps the headphones' volume for the next connection (saved with the
    /// settings when it changes)
    pub fn remember_volume(&self, addr: &str, volume: Option<u8>) {
        let Some(v) = volume else { return };
        let mut s = self.settings.lock().unwrap_or_else(|e| e.into_inner());
        if !s.remember_volume {
            return;
        }
        let key = addr.to_uppercase();
        if s.saved_volumes.get(&key) == Some(&(v as i32)) {
            return;
        }
        tracing::debug!("volume: remembering {v} for {key} (was {:?})", s.saved_volumes.get(&key));
        s.saved_volumes.insert(key, v as i32);
        if let Err(e) = s.save(&self.opts.config_dir) {
            tracing::warn!("cannot save the settings: {e}");
        }
    }

    /// What a device offered (after capability discovery)
    pub fn record_codecs(&self, addr: &str, codecs: stream::KnownCodecs) {
        tracing::debug!("codecs of {addr}: {:?}, sbc {:?}", codecs.keys, codecs.sbc);
        let mut known = self.codecs.lock().unwrap_or_else(|e| e.into_inner());
        if known.get(&addr.to_uppercase()) == Some(&codecs) {
            return;
        }
        known.insert(addr.to_uppercase(), codecs);
        if let Err(e) = crate::devices::save_codecs(&self.opts.config_dir, &known) {
            tracing::warn!("cannot save the devices' codecs: {e}");
        }
    }

    pub fn save_device(&self, addr: &str, name: &str) {
        if let Err(e) = crate::devices::save(&self.opts.config_dir, addr, name) {
            tracing::warn!("cannot save the device: {e}");
        }
    }

    /// The LE Audio stream now (set before its StreamInfo is sent)
    pub fn set_le_stream(&self, details: Option<crate::le_stream::LeStreamDetails>) {
        *self.le_stream.lock().unwrap_or_else(|e| e.into_inner()) = details;
    }

    /// An LE Audio device that streamed (le_audio_devices.txt)
    pub fn save_le_device(&self, addr: &str, name: &str) {
        if let Err(e) = crate::devices::save_le(&self.opts.config_dir, addr, name) {
            tracing::warn!("cannot save the LE Audio device: {e}");
        }
    }
}

pub struct Service {
    inner: Arc<Inner>,
    worker: Mutex<Option<JoinHandle<()>>>,
    /// Held while a session is stopped and the next one started: one start
    /// or stop at a time (issue #86: two codec switches in quick succession
    /// ran two sessions on one stack, no sound or crackling)
    control: Mutex<()>,
    /// The latest start or stop asked for ([`Service::ticket`]); an older
    /// one still waiting for `control` is dropped
    latest: AtomicU64,
    /// What the last session was started with: a Bluetooth restart
    /// connects it again (issue #86)
    last_request: Mutex<Option<StreamRequest>>,
    _power: Option<win_sys::power::PowerNotifier>,
}

/// The developer overrides (and the health log's interval) count only while
/// developer mode is on
fn apply_dev_overrides(settings: &Settings) {
    codec::dev::set_overrides(if settings.debug_mode { settings.dev_overrides } else { Default::default() });
    crate::health::set_every_interval(settings.log().health_every_interval);
}

impl Service {
    pub fn new(opts: ServiceOptions, events: Sender<ServiceEvent>) -> Service {
        // A folder v1 used is backed up and brought to v2's schema first
        tracing::debug!(
            "service: config dir {}, hci tcp {:?}, usb {:?}, afh override {:?}",
            opts.config_dir.display(),
            opts.hci_tcp,
            opts.usb,
            opts.afh
        );
        crate::migrate::prepare(&opts.config_dir);
        let settings = Settings::load(&opts.config_dir);
        if settings.debug_mode {
            tracing::debug!("service: developer overrides {:?}", settings.dev_overrides);
        }
        apply_dev_overrides(&settings);
        let codecs = crate::devices::load_codecs(&opts.config_dir);
        let inner = Arc::new(Inner {
            opts,
            settings: Mutex::new(settings),
            events: Mutex::new(events),
            host: Mutex::new(None),
            stats: Arc::new(LinkStats::default()),
            stop: AtomicBool::new(false),
            running: AtomicBool::new(false),
            power: PowerState::default(),
            tick: (Mutex::new(()), Condvar::new()),
            state: Mutex::new(StreamState::Idle),
            codecs: Mutex::new(codecs),
            reload: AtomicBool::new(false),
            le_cli: Mutex::new(None),
            le_active: AtomicBool::new(false),
            scanning: AtomicBool::new(false),
            le_stream: Mutex::new(None),
            le_support: Mutex::new(None),
            streamed: AtomicBool::new(false),
            waiting: AtomicBool::new(false),
            turned_off: AtomicBool::new(false),
            source_change: Mutex::new(None),
        });
        // Without a window, also while only the tray runs
        let weak = Arc::downgrade(&inner);
        let power = win_sys::power::PowerNotifier::new(move |e| {
            let Some(inner) = weak.upgrade() else { return };
            match e {
                win_sys::power::PowerEvent::Suspend => {
                    if inner.power.suspend() {
                        tracing::info!("the PC is going to sleep");
                        // The USB transfers get cancelled now: not an error
                        bt_host::expect_transport_loss(true);
                    } else {
                        tracing::debug!("power: repeated suspend notification ignored");
                    }
                }
                win_sys::power::PowerEvent::Resume => {
                    // Both resume kinds may come for one resume
                    if inner.power.resume() {
                        tracing::info!("the PC resumed from sleep, Bluetooth will be restarted");
                    } else {
                        tracing::debug!("power: second resume notification ignored");
                    }
                    bt_host::expect_transport_loss(false);
                }
            }
        });
        let power = match power {
            Ok(p) => Some(p),
            Err(e) => {
                tracing::warn!("no sleep / resume notifications: {e}");
                None
            }
        };
        pipeline::set_thread_priority_hook(|| pro_audio("encode thread"));
        hci_transport::set_acl_out_priority_hook(|| pro_audio("USB ACL writer"));
        bt_host::set_runloop_priority_hook(|| pro_audio("BTstack thread"));
        Service {
            inner,
            worker: Mutex::new(None),
            control: Mutex::new(()),
            latest: AtomicU64::new(0),
            last_request: Mutex::new(None),
            _power: power,
        }
    }

    pub fn stats(&self) -> Arc<LinkStats> {
        self.inner.stats.clone()
    }

    pub fn config_dir(&self) -> &std::path::Path {
        &self.inner.opts.config_dir
    }

    pub fn capture(&self) -> &HciCapture {
        &self.inner.opts.capture
    }

    /// Replaces the settings (saved by the caller) and applies what affects
    /// a running stack.
    ///
    /// LE Audio (experimental) on / off (v1 OnToggleLeAudio): LE (SM, GATT,
    /// ISO) is set up when BTstack starts, so an idle stack is stopped and
    /// the next scan or connection starts one with the new setting; while
    /// streaming or scanning the stack keeps running and is restarted
    /// before the next scan or connection ([`Service::le_audio_deferred`]
    /// says so). The low-latency setting applies to the next LE stream.
    pub fn set_settings(&self, settings: Settings) {
        tracing::debug!(
            "settings applied: media keys {}, volume sync {}, afh {:?}, LE {}, debug mode {}",
            settings.media_keys,
            settings.volume_sync,
            settings.afh,
            settings.le_audio_experimental,
            settings.debug_mode
        );
        apply_dev_overrides(&settings);
        let host = self.host();
        if let Some(h) = &host {
            h.set_media_keys(settings.media_keys);
            h.set_afh_policy(self.inner.opts.afh.as_deref().unwrap_or(&settings.afh));
        }
        let old = std::mem::replace(&mut *self.inner.settings.lock().unwrap_or_else(|e| e.into_inner()), settings);
        let new_le = self.inner.settings.lock().unwrap_or_else(|e| e.into_inner()).le_audio_experimental;
        if old.le_audio_experimental != new_le && host.is_some() && self.inner.le_active() != self.inner.le_wanted() {
            if self.is_running() || self.is_scanning() {
                tracing::info!(
                    "LE Audio (experimental) turned {}: BTstack restarts before the next scan or connection",
                    if new_le { "on" } else { "off" }
                );
            } else {
                tracing::info!(
                    "LE Audio (experimental) turned {}: stopping BTstack, the next scan or connection starts it anew",
                    if new_le { "on" } else { "off" }
                );
                self.inner.shutdown_host();
            }
        }
    }

    /// LE Audio (experimental) for every stack this service starts, whatever
    /// the setting says (the command line's --le-audio)
    pub fn set_le_audio_cli(&self, options: LeCliOptions) {
        *self.inner.le_cli.lock().unwrap_or_else(|e| e.into_inner()) = Some(options);
    }

    /// The running stack has LE Audio (experimental)
    pub fn le_audio_active(&self) -> bool {
        self.host().is_some() && self.inner.le_active()
    }

    /// The LE Audio setting changed while the stack was in use: it is
    /// applied before the next scan or connection (settings.le_audio_deferred)
    pub fn le_audio_deferred(&self) -> bool {
        self.host().is_some() && self.inner.le_active() != self.inner.le_wanted()
    }

    /// What the LE Audio (experimental) stream was set up with, while one
    /// runs (with its [`ServiceEvent::StreamInfo`]; codec "LC3 (LE Audio)")
    pub fn le_stream(&self) -> Option<crate::le_stream::LeStreamDetails> {
        self.inner.le_stream.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// A device scan runs
    pub fn is_scanning(&self) -> bool {
        self.inner.scanning.load(Ordering::Acquire)
    }

    /// The running stack, started (or restarted, as for a connection) as
    /// needed: the command line's LE Audio flow
    pub fn ensure_host(&self) -> Result<Arc<BtHost>, Msg> {
        self.inner.ensure_host()
    }

    /// Stops the stack, releasing the adapter (e.g. before its driver is
    /// changed); the next use starts a new one
    pub fn release_adapter(&self) {
        self.forget_session();
        self.shutdown();
    }

    /// Disconnect (the window, the tray): the session stops, and a
    /// Bluetooth restart does not connect it again (issue #86)
    pub fn disconnect(&self) {
        let _control = self.control.lock().unwrap_or_else(|e| e.into_inner());
        self.forget_session();
        self.stop_session();
    }

    /// Changes the source of the running A2DP stream: the link stays up.
    /// False when none is streaming (it connects or reconnects, a start or
    /// stop is under way, LE Audio has its own capture path): the caller
    /// starts it again instead.
    pub fn change_source(&self, source: SourceRequest) -> bool {
        let Ok(_control) = self.control.try_lock() else { return false };
        if !self.is_running() || self.state() != StreamState::Streaming {
            return false;
        }
        match self.last_request.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            Some(req) if req.le.is_none() => req.source = source.clone(),
            _ => return false,
        }
        *self.inner.source_change.lock().unwrap_or_else(|e| e.into_inner()) = Some(source);
        self.inner.tick.1.notify_all();
        true
    }

    /// A ticket for a start or stop run later on its own thread
    /// ([`Service::start_streaming_for`], [`Service::disconnect_for`]): the
    /// latest one taken is the one that runs
    pub fn ticket(&self) -> u64 {
        self.latest.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// Takes `control` for the request with this ticket; None when a newer
    /// one was asked for meanwhile (it runs instead)
    fn control_for(&self, ticket: u64, what: &str) -> Option<std::sync::MutexGuard<'_, ()>> {
        let control = self.control.lock().unwrap_or_else(|e| e.into_inner());
        if self.latest.load(Ordering::Acquire) != ticket {
            tracing::debug!("{what}: skipped, a newer start or stop was asked for");
            return None;
        }
        Some(control)
    }

    /// [`Service::start_streaming`] unless a newer start or stop was asked
    /// for since the ticket was taken
    pub fn start_streaming_for(&self, req: StreamRequest, ticket: u64) {
        if let Some(_control) = self.control_for(ticket, "start streaming") {
            self.start_session(req);
        }
    }

    /// [`Service::disconnect`] unless a newer start or stop was asked for
    /// since the ticket was taken
    pub fn disconnect_for(&self, ticket: u64) {
        if let Some(_control) = self.control_for(ticket, "disconnect") {
            self.forget_session();
            // A start skipped for this stop left "connecting" shown
            if !self.stop_session() {
                self.inner.notify(StreamState::Idle, Msg::new("status.ready"));
            }
        }
    }

    fn forget_session(&self) {
        *self.last_request.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    /// Restarts Bluetooth (the Adapters page): the stack is stopped, and
    /// the last session's request is returned to be started again on a new
    /// stack (the same device, profile and codec), as a session reconnects
    /// after the other stack restarts (sleep, a stopped adapter): while it
    /// connects or streams, and also after it ended on its own (an error, a
    /// lost link), which a restart is often meant to fix. Not after
    /// Disconnect, and not when the headphones were turned off: a session
    /// that waits for them just ends (issue #86).
    pub fn restart_adapter(&self) -> Option<StreamRequest> {
        let last = self.last_request.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let again = reconnect_after_restart(
            self.is_running() && self.inner.stop_requested(),
            self.inner.turned_off.load(Ordering::Acquire),
            last,
        );
        tracing::info!(
            "restarting Bluetooth (Adapters page): session running {}, {}",
            self.is_running(),
            match &again {
                Some(r) => format!("reconnecting {} afterwards", bt_host::addr_str(&r.device)),
                None => "nothing to reconnect".into(),
            }
        );
        self.shutdown();
        again
    }

    /// The chip or firmware setting changed (v1 reset_btstack): an idle
    /// stack is released now, one in use is replaced at its next use (the
    /// next connection or scan), so the firmware is loaded again. `idle`:
    /// nothing streams or scans. True when it was released now.
    pub fn reload_adapter(&self, idle: bool) -> bool {
        // Maybe another adapter: its LE Audio support is not known yet
        *self.inner.le_support.lock().unwrap_or_else(|e| e.into_inner()) = None;
        tracing::debug!(
            "reload adapter: idle {idle}, streaming {}, stack running {}",
            self.is_running(),
            self.host().is_some()
        );
        if idle && !self.is_running() {
            self.inner.reload.store(false, Ordering::Release);
            self.inner.shutdown_host();
            true
        } else {
            self.inner.reload.store(self.host().is_some(), Ordering::Release);
            false
        }
    }

    /// The running adapter has LE Audio's isochronous channels; None until a
    /// stack started on it since the adapter settings last changed
    pub fn le_audio_supported(&self) -> Option<bool> {
        *self.inner.le_support.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// What each device (upper-case address) offered when it last
    /// connected
    pub fn device_codecs(&self) -> BTreeMap<String, stream::KnownCodecs> {
        self.inner.codecs.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn settings(&self) -> Settings {
        self.inner.settings.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn state(&self) -> StreamState {
        *self.inner.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn is_running(&self) -> bool {
        self.inner.running.load(Ordering::Acquire)
    }

    /// The running stack, if any (for diagnostics)
    pub fn host(&self) -> Option<Arc<BtHost>> {
        self.inner.host.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Starts a session; a running one is stopped first
    pub fn start_streaming(&self, req: StreamRequest) {
        let _control = self.control.lock().unwrap_or_else(|e| e.into_inner());
        self.start_session(req);
    }

    fn start_session(&self, req: StreamRequest) {
        tracing::debug!(
            "start streaming: {} ({}), stopping any session first",
            bt_host::addr_str(&req.device),
            if req.le.is_some() { "LE Audio" } else { "A2DP" }
        );
        self.end_scan();
        // The running session leaves Windows' default output switched when
        // this one switches it to the same device (issue #203)
        stream::expect_default_switch(req.source.default_switch());
        self.stop_session();
        stream::expect_default_switch(None);
        // Saved in the profile, so in `req` too: left over from a session
        // that ended before it took it
        self.inner.take_source_change();
        *self.last_request.lock().unwrap_or_else(|e| e.into_inner()) = Some(req.clone());
        self.inner.turned_off.store(false, Ordering::Release);
        self.inner.stop.store(false, Ordering::Release);
        self.inner.running.store(true, Ordering::Release);
        self.inner.notify(StreamState::Connecting, Msg::new("status.initializing"));
        let inner = self.inner.clone();
        let t = std::thread::Builder::new()
            .name("session".into())
            .spawn(move || {
                // A panic in the session must not take the process down: the
                // state says what happened
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    // LE Audio (experimental) profiles have their own session
                    if req.le.is_some() { crate::le_stream::run(&inner, req) } else { stream::run(&inner, req) }
                }));
                if r.is_err() {
                    tracing::error!("the streaming session panicked");
                    inner.notify(StreamState::Error, Msg::new("error.btstack_crash_stream"));
                }
                stream::restore_unclaimed_default();
                inner.running.store(false, Ordering::Release);
            })
            .expect("spawning the session thread");
        *self.worker.lock().unwrap_or_else(|e| e.into_inner()) = Some(t);
    }

    /// Stops the session and waits for it (bounded by the stack's own
    /// timeouts: blocking waits are cancelled)
    pub fn stop_streaming(&self) {
        let _control = self.control.lock().unwrap_or_else(|e| e.into_inner());
        self.stop_session();
    }

    /// False when no session was there to stop
    fn stop_session(&self) -> bool {
        let Some(t) = self.worker.lock().unwrap_or_else(|e| e.into_inner()).take() else { return false };
        tracing::debug!("stop streaming: requested (state {:?})", self.state());
        let started = Instant::now();
        self.inner.stop.store(true, Ordering::Release);
        self.inner.tick.1.notify_all();
        if let Some(h) = self.host() {
            h.cancel_pending_waits();
        }
        let _ = t.join();
        tracing::debug!("stop streaming: session ended in {} ms", started.elapsed().as_millis());
        true
    }

    /// A session that waits for turned-off headphones holds nothing worth
    /// keeping: a scan ends it
    fn end_wait_for_device(&self) {
        if self.inner.waiting.load(Ordering::Acquire) {
            tracing::info!("scan: no longer waiting for the headphones");
            self.stop_streaming();
        }
    }

    /// Inquiry for nearby (classic) devices
    pub fn scan(&self, seconds: u32) -> Result<Vec<DiscoveredDevice>, Msg> {
        self.end_wait_for_device();
        if self.is_running() {
            return Err(Msg::new("status.streaming"));
        }
        let _scanning = ScanningGuard::new(&self.inner.scanning);
        self.inner.notify(StreamState::Idle, Msg::new("status.initializing_scan"));
        let host = self.inner.ensure_host()?;
        self.inner.notify(StreamState::Idle, Msg::new("status.scanning_devices"));
        let found = host.scan(seconds);
        for d in &found {
            tracing::debug!(
                "scan: {} \"{}\" rssi {} class {:06x}",
                bt_host::addr_str(&d.addr),
                d.name,
                d.rssi,
                d.class_of_device
            );
        }
        self.inner.notify(StreamState::Idle, Msg::with("status.scan_complete", &[&found.len()]));
        Ok(found)
    }

    /// Inquiry for nearby devices, then, when the stack has LE Audio
    /// (experimental), a 5 s LE scan for LE Audio devices (v1's scan): the
    /// classic devices, then the LE Audio ones ([`ScannedDevice`]).
    /// `on_found` gets the classic devices each time one answers (or its
    /// name comes), long before the scan ends.
    pub fn scan_devices(
        &self,
        seconds: u32,
        on_found: &mut dyn FnMut(&[ScannedDevice]),
    ) -> Result<Vec<ScannedDevice>, Msg> {
        self.end_wait_for_device();
        if self.is_running() {
            return Err(Msg::new("status.streaming"));
        }
        let _scanning = ScanningGuard::new(&self.inner.scanning);
        self.inner.notify(StreamState::Idle, Msg::new("status.initializing_scan"));
        let host = self.inner.ensure_host()?;
        self.inner.notify(StreamState::Idle, Msg::new("status.scanning_devices"));
        let classic = host.scan_with(seconds, &mut |found| {
            on_found(&found.iter().map(ScannedDevice::classic).collect::<Vec<_>>());
        });
        let mut list: Vec<ScannedDevice> = classic.iter().map(ScannedDevice::classic).collect();
        for d in &list {
            tracing::debug!(
                "scan: {} \"{}\" rssi {} class {:06x} audio {}",
                d.address,
                d.name,
                d.rssi,
                d.class_of_device,
                d.audio
            );
        }
        let mut le_found = 0;
        if self.inner.le_active() && !host.scan_stopped() {
            let until = Instant::now() + LE_SCAN_READY_WAIT;
            while !host.le_ready() && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(100));
            }
            if host.le_ready() {
                self.inner.notify(StreamState::Idle, Msg::new("status.le_scanning"));
                let results = host.le_scan(LE_SCAN);
                let saved = crate::devices::load_le(&self.inner.opts.config_dir);
                let (entries, found) = le_scan_entries(&results, &saved);
                tracing::debug!(
                    "le scan: {} result(s), {found} LE Audio advertiser(s), {} listed, {} saved",
                    results.len(),
                    entries.len(),
                    saved.len()
                );
                for e in &entries {
                    tracing::debug!(
                        "le scan: {} {} rssi {} set {}",
                        e.address,
                        crate::diag::quoted(&e.name),
                        e.rssi,
                        e.le_audio_set
                    );
                }
                le_found = found;
                list.extend(entries);
            } else {
                tracing::warn!("LE Audio not ready, LE scan skipped");
            }
        }
        self.inner.notify(StreamState::Idle, Msg::with("status.scan_complete", &[&(classic.len() + le_found)]));
        Ok(list)
    }

    /// Ends a running scan early ([`Service::scan_devices`] returns what it
    /// found so far)
    pub fn stop_scan(&self) {
        if self.is_scanning()
            && let Some(h) = self.host()
        {
            tracing::info!("scan: stopped");
            h.stop_scan();
        }
    }

    /// A connection while the scan inquires and pages on the same radio
    /// would be slow or fail: the scan ends first (a device it listed was
    /// picked)
    fn end_scan(&self) {
        if !self.is_scanning() {
            return;
        }
        self.stop_scan();
        let until = Instant::now() + SCAN_END_WAIT;
        while self.is_scanning() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(50));
        }
        tracing::debug!("start streaming: scan ended first (still running: {})", self.is_scanning());
    }

    /// Stops streaming and the stack (the adapter is released)
    pub fn shutdown(&self) {
        self.stop_streaming();
        self.inner.shutdown_host();
    }

    pub fn saved_volumes(&self) -> BTreeMap<String, i32> {
        self.settings().saved_volumes
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// The request to start again after a Bluetooth restart: the last
/// session's (None after Disconnect), unless it is being stopped or its
/// headphones were turned off
fn reconnect_after_restart(stopping: bool, turned_off: bool, last: Option<StreamRequest>) -> Option<StreamRequest> {
    last.filter(|_| !stopping && !turned_off)
}

/// Marks a scan while it runs (also when it returns early)
struct ScanningGuard<'a>(&'a AtomicBool);

impl<'a> ScanningGuard<'a> {
    fn new(flag: &'a AtomicBool) -> ScanningGuard<'a> {
        flag.store(true, Ordering::Release);
        ScanningGuard(flag)
    }
}

impl Drop for ScanningGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// The link to the device came up moments ago: headphones that just
/// connected may refuse the PC's page for a while once it is dropped
fn link_is_fresh(age: Option<Duration>) -> bool {
    age.is_some_and(|a| a < FRESH_LINK)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_link_that_just_came_up_keeps_the_stack() {
        assert!(link_is_fresh(Some(Duration::from_secs(5))));
        assert!(!link_is_fresh(Some(FRESH_LINK)));
        assert!(!link_is_fresh(None));
    }

    #[test]
    fn a_bluetooth_restart_reconnects_the_running_session() {
        let req = crate::profiles::Profile {
            device_address: "00:11:22:33:44:55".into(),
            codec: "ldac".into(),
            ..Default::default()
        }
        .to_request(1000)
        .unwrap();
        let last = || Some(req.clone());
        // Streaming, connecting, or ended on its own: the same request again
        assert_eq!(reconnect_after_restart(false, false, last()), last());
        // Being stopped, the headphones are off, or nothing to reconnect
        // (none ran, or Disconnect forgot it)
        assert_eq!(reconnect_after_restart(true, false, last()), None);
        assert_eq!(reconnect_after_restart(false, true, last()), None);
        assert_eq!(reconnect_after_restart(false, false, None), None);
    }

    fn service() -> Service {
        let dir = std::env::temp_dir().join(format!("bluecaster-service-restart-{}", std::process::id()));
        let opts = ServiceOptions {
            config_dir: dir,
            hci_tcp: None,
            capture: bt_host::HciCapture::new(),
            afh: None,
            usb: Default::default(),
        };
        let (tx, _rx) = std::sync::mpsc::channel();
        Service::new(opts, tx)
    }

    fn request() -> StreamRequest {
        crate::profiles::Profile {
            device_address: "00:11:22:33:44:55".into(),
            codec: "sbc".into(),
            ..Default::default()
        }
        .to_request(1000)
        .unwrap()
    }

    #[test]
    fn a_source_changes_in_place_only_on_a_running_a2dp_stream() {
        let s = service();
        let tone = || SourceRequest::TestTone;
        // No session
        *s.last_request.lock().unwrap() = Some(request());
        assert!(!s.change_source(tone()));
        // A session that connects
        s.inner.running.store(true, Ordering::Release);
        s.inner.notify(StreamState::Connecting, Msg::new("status.initializing"));
        assert!(!s.change_source(tone()));
        // LE Audio
        s.inner.notify(StreamState::Streaming, Msg::new("status.connected"));
        let le = bt_host::LeRequest { extra_devices: Vec::new(), preset: None, low_latency: true };
        *s.last_request.lock().unwrap() = Some(StreamRequest { le: Some(le), ..request() });
        assert!(!s.change_source(tone()));
        assert_eq!(s.inner.take_source_change(), None);
        // A start or stop under way
        *s.last_request.lock().unwrap() = Some(request());
        let control = s.control.lock().unwrap();
        assert!(!s.change_source(tone()));
        drop(control);
        // Streaming: the session takes it, and a Bluetooth restart keeps it
        assert!(s.change_source(tone()));
        assert_eq!(s.inner.take_source_change(), Some(tone()));
        assert_eq!(s.last_request.lock().unwrap().as_ref().unwrap().source, tone());
    }

    #[test]
    fn a_bluetooth_restart_follows_disconnect_and_turned_off_headphones() {
        let s = service();
        // Nothing ran yet
        assert_eq!(s.restart_adapter(), None);
        // A session that ended on its own (an error): connected again, also
        // after a second restart that found nothing running
        *s.last_request.lock().unwrap() = Some(request());
        assert_eq!(s.restart_adapter(), Some(request()));
        assert_eq!(s.restart_adapter(), Some(request()));
        // Its headphones were turned off and did not come back
        s.inner.waiting_for_device();
        s.inner.done_waiting_for_device(false);
        assert_eq!(s.restart_adapter(), None);
        // ... or did
        s.inner.waiting_for_device();
        s.inner.done_waiting_for_device(true);
        assert_eq!(s.restart_adapter(), Some(request()));
        // Disconnect
        s.disconnect();
        assert_eq!(s.restart_adapter(), None);
    }

    #[test]
    fn only_the_latest_start_or_stop_asked_for_runs() {
        let s = service();
        // Two codec switches in quick succession, then a Disconnect: each
        // thread runs late, the first two find a newer one asked for
        let (first, second) = (s.ticket(), s.ticket());
        s.start_streaming_for(request(), first);
        assert!(s.worker.lock().unwrap().is_none());
        *s.last_request.lock().unwrap() = Some(request());
        let stop = s.ticket();
        s.start_streaming_for(request(), second);
        assert!(s.worker.lock().unwrap().is_none());
        // The Disconnect older than a later start is skipped too
        let start = s.ticket();
        s.disconnect_for(stop);
        assert_eq!(s.restart_adapter(), Some(request()));
        let _ = start;
        // The latest runs
        let stop = s.ticket();
        s.disconnect_for(stop);
        assert_eq!(s.restart_adapter(), None);
        assert_eq!(s.state(), StreamState::Idle);
    }

    #[test]
    fn sleep_marks_suspending_until_resume() {
        let p = PowerState::default();
        assert!(!p.suspending() && !p.stale());
        assert!(p.suspend());
        // A repeated notification
        assert!(!p.suspend());
        assert!(p.suspending());
        assert!(!p.stale());
        assert!(p.resumed_at().is_none());
        assert!(p.resume());
        assert!(!p.suspending());
        assert!(p.stale());
        assert!(p.resumed_at().is_some());
    }

    #[test]
    fn the_second_resume_notification_is_ignored() {
        let p = PowerState::default();
        p.suspend();
        assert!(p.resume());
        let at = p.resumed_at();
        assert!(!p.resume());
        assert_eq!(p.resumed_at(), at);
        assert!(p.stale() && !p.suspending());
    }

    #[test]
    fn a_resume_without_suspend_still_marks_the_stack_stale() {
        let p = PowerState::default();
        assert!(p.resume());
        assert!(p.stale() && !p.suspending());
    }

    #[test]
    fn a_new_sleep_counts_while_the_stack_is_still_stale() {
        // Slept and woke, nothing used the stack since, then slept again
        let p = PowerState::default();
        p.suspend();
        p.resume();
        let first = p.resumed_at().unwrap();
        std::thread::sleep(Duration::from_millis(5));
        assert!(p.suspend());
        assert!(p.suspending() && p.stale());
        assert!(p.resume());
        assert!(p.resumed_at().unwrap() > first);
    }

    fn le(addr: &str, name: &str, set: Option<u32>) -> LeScanResult {
        LeScanResult {
            addr: bt_host::LeAddr { addr: bt_host::parse_addr(addr).unwrap(), addr_type: 0 },
            name: name.into(),
            le_audio: true,
            rsi: set.map(|_| [1, 2, 3, 4, 5, 6]),
            set_id: set,
            ..LeScanResult::default()
        }
    }

    #[test]
    fn le_scan_lists_one_member_per_new_set() {
        let results = [
            le("00:A2:D0:00:00:12", "Buds L", Some(0)),
            le("00:A2:D0:00:00:13", "Buds R", Some(0)),
            le("00:A2:D0:00:00:20", "Speaker", None),
            LeScanResult { le_audio: false, ..le("00:A2:D0:00:00:30", "Mouse", None) },
            le("00:A2:D0:00:00:20", "Speaker 2", None),
            le("00:A2:D0:00:00:40", "Other L", Some(1)),
        ];
        let (list, found) = le_scan_entries(&results, &[]);
        let got: Vec<_> = list.iter().map(|d| (d.address.as_str(), d.name.as_str(), d.le_audio_set)).collect();
        assert_eq!(
            got,
            [
                ("00:A2:D0:00:00:12", "Buds L", true),
                ("00:A2:D0:00:00:20", "Speaker 2", false),
                ("00:A2:D0:00:00:40", "Other L", true)
            ]
        );
        assert_eq!(found, 5);
        assert!(list.iter().all(|d| d.le_audio && d.audio && d.class_of_device == 0));
    }

    #[test]
    fn le_scan_keeps_saved_members() {
        // The right earbud is saved: it is listed, and the left one still
        // stands for the set
        let saved = [SavedDevice { address: "00:A2:D0:00:00:13".into(), name: "R".into(), le_audio: true }];
        let results = [le("00:A2:D0:00:00:13", "Buds R", Some(0)), le("00:A2:D0:00:00:12", "Buds L", Some(0))];
        let (list, _) = le_scan_entries(&results, &saved);
        assert_eq!(list.len(), 2);
        // A third member is a duplicate of the listed set
        let results = [
            le("00:A2:D0:00:00:12", "Buds L", Some(0)),
            le("00:A2:D0:00:00:14", "Buds C", Some(0)),
            le("00:A2:D0:00:00:13", "Buds R", Some(0)),
        ];
        let (list, _) = le_scan_entries(&results, &saved);
        let got: Vec<_> = list.iter().map(|d| d.address.as_str()).collect();
        assert_eq!(got, ["00:A2:D0:00:00:12", "00:A2:D0:00:00:13"]);
    }

    #[test]
    fn a_dual_mode_device_keeps_its_classic_entry() {
        let classic = DiscoveredDevice {
            addr: bt_host::parse_addr("00:A2:D0:00:00:12").unwrap(),
            name: "Buds".into(),
            class_of_device: 0x240404,
            rssi: -40,
        };
        let c = ScannedDevice::classic(&classic);
        assert!(!c.le_audio && c.audio);
        let (le_list, _) = le_scan_entries(&[le("00:A2:D0:00:00:12", "Buds", None)], &[]);
        assert_eq!(le_list[0].address, c.address);
        assert!(le_list[0].le_audio);
    }

    #[test]
    fn le_config_from_the_setting_or_the_command_line() {
        let opts = ServiceOptions {
            config_dir: PathBuf::from("cfg"),
            hci_tcp: None,
            capture: HciCapture::new(),
            afh: None,
            usb: UsbSelector::default(),
        };
        assert!(le_config(false, None, &opts).is_none());
        let cli = LeCliOptions { mcp_dry_run: true, ccp_sim: Some("1:incoming x".into()), ccp_console: true };
        let c = le_config(true, Some(&cli), &opts).unwrap();
        assert_eq!(c.bond_dir.as_deref(), Some(std::path::Path::new(r"cfg\pairing")));
        assert_eq!(c.bonds_file.as_deref(), Some(std::path::Path::new(r"cfg\pairing\le_audio_bonds.tlv")));
        assert!(c.media.is_none());
        assert_eq!(c.ccp_sim.as_deref(), Some("1:incoming x"));
        assert!(c.ccp_console);
        // The emulator transport implies the MCP dry run
        let tcp = ServiceOptions { hci_tcp: Some("127.0.0.1:9510".into()), ..opts };
        let c = le_config(true, Some(&LeCliOptions::default()), &tcp).unwrap();
        assert!(c.media.is_none() && c.ccp_sim.is_none() && !c.ccp_console);
    }

    #[test]
    fn a_new_stack_clears_stale_but_not_suspending() {
        let p = PowerState::default();
        p.suspend();
        p.resume();
        p.clear_stale();
        assert!(!p.stale());
        p.suspend();
        p.clear_stale();
        assert!(p.suspending());
        p.end_suspend();
        assert!(!p.suspending() && !p.stale());
    }
}
