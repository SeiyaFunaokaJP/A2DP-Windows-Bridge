//! The application on the BTstack thread ([`btstack_sys::App`]): setup of
//! HCI and the profiles, event handling, the jobs the callers of
//! [`crate::BtHost`] send, and the media sender.
//!
//! Ported from v1's BtStackTransport (btstack_transport.cpp); the comments
//! keep v1's reasons.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use btstack_sys::consts::*;
use btstack_sys::{AclState, App, AvdtpState, HciBackend, HciState, Source, Stack};
use codec::CodecKind;
use codec::caps::{AacCapabilities, AacObjectType, ConfigRequest, Configuration, SbcCapabilities, VendorCapability};
use codec::sbc::{Allocation, ChannelMode};

use crate::events::{addr_at, addr_str, event_type, hci_error_string, le16, le24, subevent, u8_at};
use crate::le::core::LeCore;
use crate::le::iso_send::TIMER_ISO;
use crate::link_keys::FileLinkKeyStore;
use crate::media::{MediaPacket, RTP_HEADER_SIZE};
use crate::rfcomm::{Rfcomm, TIMER_RFCOMM};
use crate::shared::Shared;
use crate::stats::LinkStats;
use crate::{AfhClassification, ConnectFailure, Controls, DiscoveredDevice, HostEvent, InitFailure, MediaKey};

pub(crate) type Job = Box<dyn FnOnce(&mut HostApp, &Stack) + Send>;

pub(crate) const TIMER_AFH: u32 = 1;
pub(crate) const TIMER_PLAYBACK: u32 = 2;
/// Read the version again after a Realtek download (HCI was busy)
const TIMER_RTK_VERIFY: u32 = 5;
const RTK_VERIFY_RETRY_MS: u32 = 100;
const RTK_VERIFY_TRIES: u8 = 20;

/// One radio reading per tick (RSSI and the AFH map in turn)
const AFH_TICK_MS: u32 = 2500;
/// Re-survey the Wi-Fi every 30 s
const AFH_CLASSIFY_EVERY_TICKS: u32 = 12;
const PLAYBACK_TICK_MS: u32 = 1000;
/// No audio for ~2 s = paused
const PLAYBACK_IDLE_TICKS: u32 = 2;
/// Let the player react to a button before the capture check takes over
const PLAYBACK_HOLD: Duration = Duration::from_millis(3000);
/// One volume command at a time; a lost response must not block for good
const VOLUME_RESPONSE_TIMEOUT: Duration = Duration::from_millis(1000);

/// What the BTstack thread needs to set up HCI
pub(crate) struct Setup {
    pub tcp: bool,
    pub link_key_dir: Option<PathBuf>,
    pub firmware_dir: PathBuf,
    pub realtek_pid: u16,
    /// Realtek firmware / config file stems (no extension) of the chip
    pub realtek_files: Option<(String, String)>,
    pub broadcom_ids: Vec<(u16, u16)>,
    /// HFP audio gateway for the headphones' battery level
    pub battery: bool,
}

/// LMP subversions Realtek chips report while running their ROM code,
/// before a firmware download (BTstack's btstack_chipset_realtek.c)
/// AVDTP signal identifiers (AVDTP 8.5)
const AVDTP_SI_SET_CONFIGURATION: u8 = 0x03;
const AVDTP_SI_START: u8 = 0x07;

fn is_realtek_rom_lmp(lmp_subver: u16) -> bool {
    matches!(lmp_subver, 0x1200 | 0x8723 | 0x8821 | 0x8761 | 0x8822 | 0x8852 | 0x8851)
}

pub(crate) struct HostApp {
    pub shared: Arc<Shared>,
    pub controls: Arc<Controls>,
    pub stats: Arc<LinkStats>,
    pub jobs: mpsc::Receiver<Job>,
    pub events: mpsc::Sender<HostEvent>,
    pub media: rtrb::Consumer<MediaPacket>,
    /// The encode thread queues packets only while this is set
    pub media_streaming: crate::media::StreamingFlag,
    pub setup: Setup,
    pub afh: Option<Box<dyn crate::AfhSource>>,
    /// LE Audio (experimental): None while off
    pub le: Option<LeCore>,
    /// RFCOMM client channels of IPC clients
    pub rfcomm: Rfcomm,

    // ---- connection (BTstack thread only) ----
    pub a2dp_cid: u16,
    pub avrcp_cid: u16,
    pub local_seid: u8,
    pub remote_seid: u8,
    pub remote_addr: Option<[u8; 6]>,
    /// ACL handle of the HFP service level connection, once up
    pub hfp_acl: Option<u16>,
    pub streaming: bool,
    /// Codec of the configured stream (RTP or not)
    pub stream_codec: Option<CodecKind>,
    pub endpoints: HashMap<CodecKind, u8>,
    pub can_send_requested: bool,
    /// Stream timing (stats.timing): when the pending can-send request was
    /// made, the last media packet sent, the last Number Of Completed
    /// Packets event
    can_send_asked: Option<Instant>,
    last_media_sent: Option<Instant>,
    last_completed: Option<Instant>,
    /// SEP discovery in progress
    sep_sinks: u8,
    sep_sinks_in_use: u8,
    /// Capabilities of the discovery in progress
    caps: codec::caps::RemoteCaps,

    // ---- init / chipset ----
    init_started: Instant,
    /// Manufacturer the controller reported in this init (Read Local Version)
    controller_company: Option<u16>,
    rtk_version_seen: bool,
    rtk_lmp_first: u16,
    rtk_verify_pending: bool,
    /// Version reads left before the download stays "not verified"
    rtk_verify_tries: u8,
    bcm_checked: bool,

    // ---- AVRCP ----
    volume_in_flight: Option<Instant>,
    playback_status: u8,
    playback_prev_frames: u64,
    playback_idle_ticks: u32,
    playback_hold_until: Instant,

    // ---- AFH ----
    afh_sent_map: Option<[u8; 10]>,
    afh_policy_version_seen: Option<u32>,
    afh_ticks: u32,
    /// The next radio reading is RSSI (else the AFH map). Its own toggle: a
    /// classification sent on a tick must not always skip the same reading
    read_rssi_next: bool,
}

/// Why the signaling connection failed. Page timeout, or a device that never
/// took the connection: no answer. Connection / L2CAP RTX timeout, a
/// connection BTstack gave up on, or an SDP query cut off: the link came up
/// and then carried nothing.
fn signaling_failure(status: u8) -> ConnectFailure {
    match status {
        ERROR_CODE_PAGE_TIMEOUT | ERROR_CODE_CONNECTION_ACCEPT_TIMEOUT_EXCEEDED => ConnectFailure::NoAnswer,
        L2CAP_CONNECTION_RESPONSE_RESULT_REFUSED_SECURITY
        | L2CAP_CONNECTION_PIN_OR_LINK_KEY_MISSING
        | ERROR_CODE_AUTHENTICATION_FAILURE
        | ERROR_CODE_PIN_OR_KEY_MISSING => ConnectFailure::AuthFailed,
        ERROR_CODE_CONNECTION_TIMEOUT
        | L2CAP_CONNECTION_RESPONSE_RESULT_RTX_TIMEOUT
        | L2CAP_CONNECTION_BASEBAND_DISCONNECT
        | SDP_QUERY_INCOMPLETE
        | ERROR_CODE_UNSPECIFIED_ERROR => ConnectFailure::LinkLost,
        _ => ConnectFailure::Other,
    }
}

impl HostApp {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        shared: Arc<Shared>,
        controls: Arc<Controls>,
        stats: Arc<LinkStats>,
        jobs: mpsc::Receiver<Job>,
        events: mpsc::Sender<HostEvent>,
        media: rtrb::Consumer<MediaPacket>,
        media_streaming: crate::media::StreamingFlag,
        setup: Setup,
        afh: Option<Box<dyn crate::AfhSource>>,
        le: Option<LeCore>,
    ) -> HostApp {
        HostApp {
            shared,
            controls,
            stats,
            jobs,
            events,
            media,
            media_streaming,
            setup,
            afh,
            le,
            rfcomm: Rfcomm::default(),
            a2dp_cid: 0,
            avrcp_cid: 0,
            local_seid: 0,
            remote_seid: 0,
            remote_addr: None,
            hfp_acl: None,
            streaming: false,
            stream_codec: None,
            endpoints: HashMap::new(),
            can_send_requested: false,
            can_send_asked: None,
            last_media_sent: None,
            last_completed: None,
            sep_sinks: 0,
            sep_sinks_in_use: 0,
            caps: Default::default(),
            init_started: Instant::now(),
            controller_company: None,
            rtk_version_seen: false,
            rtk_lmp_first: 0,
            rtk_verify_pending: false,
            rtk_verify_tries: 0,
            bcm_checked: false,
            volume_in_flight: None,
            playback_status: AVRCP_PLAYBACK_STATUS_STOPPED,
            playback_prev_frames: 0,
            playback_idle_ticks: 0,
            playback_hold_until: Instant::now(),
            afh_sent_map: None,
            afh_policy_version_seen: None,
            afh_ticks: 0,
            read_rssi_next: true,
        }
    }

    fn init_failed(&self, failure: InitFailure) {
        self.shared.update(|s| {
            if s.init_done.is_none() {
                s.init_done = Some(Err(failure));
            }
        });
    }

    // ======================================================================
    // Setup
    // ======================================================================

    /// hci_init() through hci_power_control(ON). Directly from the init
    /// closure, or once the Intel bootloader download has finished (then the
    /// transport is installed already: `backend` is None).
    pub fn setup_hci_and_power_on(&mut self, stack: &Stack, backend: Option<Box<dyn HciBackend>>) {
        match backend {
            Some(b) => stack.hci_init(b),
            None => stack.hci_init_installed(),
        }
        if self.setup.tcp && self.setup.realtek_pid != 0 {
            // The virtual controller is no Realtek chip, even when a Realtek
            // adapter is plugged in (it would never answer vendor commands)
            tracing::info!("H4 over TCP: ignoring Realtek PID 0x{:04X}", self.setup.realtek_pid);
            self.setup.realtek_pid = 0;
        }
        let pid = self.setup.realtek_pid;
        if pid != 0 {
            tracing::info!("Realtek adapter PID 0x{pid:04X}, firmware folder {}", self.setup.firmware_dir.display());
            // The driver opens the .bin files (csrc/realtek.c); the
            // extension-less copies earlier versions made are not needed
            if let Some((fw, cfg)) = self.setup.realtek_files.clone() {
                remove_alias(&self.setup.firmware_dir, &fw);
                remove_alias(&self.setup.firmware_dir, &cfg);
            }
            stack.realtek_setup(pid, &self.setup.firmware_dir.to_string_lossy());
            stack.set_chipset_realtek();
        } else {
            tracing::info!("no Realtek PID configured, skipping the Realtek chipset init");
        }

        tracing::debug!(
            "hci setup: link key folder {:?}, battery (HFP) {}, LE Audio {}, tcp {}",
            self.setup.link_key_dir,
            self.setup.battery,
            self.le.is_some(),
            self.setup.tcp
        );
        match &self.setup.link_key_dir {
            Some(dir) => stack.set_link_key_store(Some(Box::new(FileLinkKeyStore::open(dir)))),
            None => stack.set_link_key_store(None),
        }
        // Audio (major 0x04), Hi-Fi audio device (minor 0x28), A2DP source;
        // page timeout ~20 s (0x8000 * 0.625 ms)
        stack.gap_setup(0x200428, 0x8000);
        // L2CAP before any profile: it registers with HCI for Connection
        // Complete events
        stack.l2cap_init();
        stack.sdp_init();
        stack.add_hci_event_handler();
        stack.a2dp_source_init();
        stack.avrcp_init();
        // Controller category 2 (monitor / amplifier): our CT sends absolute
        // volume; target category 1 (player): the headphones' buttons
        stack.sdp_register_records(
            AVRCP_FEATURE_MASK_CATEGORY_MONITOR_OR_AMPLIFIER,
            AVRCP_FEATURE_MASK_CATEGORY_PLAYER_OR_RECORDER,
        );
        self.register_endpoints(stack);
        // Battery level (experimental, off by default): HFP's service level
        // connection only, no audio
        if self.setup.battery {
            stack.hfp_ag_init();
        }
        // LE Audio (experimental, off by default): SM, GATT client / server,
        // ISO. Nothing of it runs while the setting is off.
        if let Some(le) = self.le.as_mut() {
            le.stack_init(stack);
        }
        // The Realtek driver sends vendor commands before HCI Reset (reading
        // the LMP subversion); without custom pre-init that step runs in the
        // download's slot and the download never happens
        if pid != 0 {
            stack.enable_custom_pre_init();
        }
        self.controller_company = None;
        self.rtk_version_seen = false;
        self.rtk_lmp_first = 0;
        self.rtk_verify_pending = false;
        self.rtk_verify_tries = 0;
        self.init_started = Instant::now();
        self.shared.update(|s| {
            s.power_on_at = Some(Instant::now());
            s.realtek_rom = false;
        });
        tracing::info!("hci_init done, powering on");
        if stack.power_on().is_err() {
            tracing::error!("could not open the Bluetooth adapter");
            // Issue #81: an RTL8852BU whose firmware was missing failed here
            // with "failed to initialize WinUSB (error 50)", which reads as
            // a driver problem; the firmware was all it lacked
            let missing = self.missing_realtek_files(true);
            if !missing.is_empty() {
                tracing::warn!(
                    "the Realtek firmware is missing too ({} in {}): add it, then start again",
                    missing.join(", "),
                    self.setup.firmware_dir.display()
                );
            }
            self.init_failed(InitFailure::Failed);
        }
    }

    /// The chip's Realtek files not in the firmware folder ("rtl8852bu_fw.bin"),
    /// the config file too if `with_config`
    fn missing_realtek_files(&self, with_config: bool) -> Vec<String> {
        let Some((fw, cfg)) = self.setup.realtek_files.as_ref().filter(|_| self.setup.realtek_pid != 0) else {
            return Vec::new();
        };
        let dir = &self.setup.firmware_dir;
        [Some(fw), with_config.then_some(cfg)]
            .into_iter()
            .flatten()
            // The driver also takes the extension-less file alone
            .filter(|s| !dir.join(format!("{s}.bin")).is_file() && !dir.join(s.as_str()).is_file())
            .map(|s| format!("{s}.bin"))
            .collect()
    }

    /// Read the version after the Realtek download; the result decides
    /// between "loaded" and the ROM code (on_local_version)
    fn verify_realtek_download(&mut self, stack: &Stack) {
        if stack.cmd_read_local_version() == ERROR_CODE_SUCCESS {
            self.rtk_verify_pending = true;
        } else if self.rtk_verify_tries > 0 {
            // HCI is busy right after start-up: again a moment later
            tracing::debug!("realtek verify: HCI busy, retrying ({} tries left)", self.rtk_verify_tries);
            self.rtk_verify_tries -= 1;
            stack.timer_start(TIMER_RTK_VERIFY, RTK_VERIFY_RETRY_MS);
        } else {
            tracing::info!(
                "Realtek firmware download ran (ROM LMP subversion 0x{:04X}); not verified",
                self.rtk_lmp_first
            );
        }
    }

    fn register_endpoints(&mut self, stack: &Stack) {
        for (slot, ep) in codec::caps::local_endpoints().iter().enumerate() {
            match stack.create_stream_endpoint(slot, ep.codec_type, &ep.caps, &ep.config) {
                Some(seid) => {
                    tracing::debug!("registered the {} endpoint (SEID {seid})", ep.kind);
                    self.endpoints.insert(ep.kind, seid);
                }
                None => tracing::warn!("failed to register the {} endpoint", ep.kind),
            }
        }
    }

    // ======================================================================
    // Jobs (called through BtHost)
    // ======================================================================

    /// a2dp_source_establish_stream; resets the capability discovery
    pub fn establish(&mut self, stack: &Stack, addr: [u8; 6]) -> Result<(), u8> {
        self.remote_addr = Some(addr);
        self.caps = Default::default();
        self.sep_sinks = 0;
        self.sep_sinks_in_use = 0;
        self.stream_codec = None;
        // Packets of a previous stream must not go out on this one
        while self.media.pop().is_ok() {}
        self.can_send_requested = false;
        tracing::debug!("establish: {} (acl {:?})", addr_str(&addr), stack.acl_state(&addr));
        match stack.a2dp_establish_stream(&addr) {
            Ok(cid) => {
                tracing::debug!("establish: a2dp cid 0x{cid:04X}");
                self.a2dp_cid = cid;
                Ok(())
            }
            Err(status) => {
                tracing::debug!("establish: refused 0x{status:02X} ({})", hci_error_string(status));
                Err(status)
            }
        }
    }

    /// Closes an AVDTP connection to `addr` BTstack still knows (left over
    /// when a stream was given up on), which would make
    /// a2dp_source_establish_stream fail with COMMAND_DISALLOWED. Returns
    /// whether a release event follows.
    pub fn release_leftover(&mut self, stack: &Stack, addr: [u8; 6]) -> Option<bool> {
        let (state, cid) = stack.avdtp_state_for_addr(&addr);
        tracing::debug!("leftover AVDTP for {}: {state:?} (cid 0x{cid:04X})", addr_str(&addr));
        if state == AvdtpState::None {
            return None;
        }
        tracing::info!("closing a leftover AVDTP connection (cid 0x{cid:04x}) first");
        stack.a2dp_disconnect(cid);
        Some(state == AvdtpState::Open)
    }

    /// Tears down a half-open or open AVDTP connection after a failed or
    /// cancelled connect, or for a disconnect. Returns whether a release
    /// event follows.
    pub fn abort(&mut self, stack: &Stack) -> bool {
        if self.a2dp_cid == 0 {
            tracing::debug!("abort: no a2dp cid");
            return false;
        }
        tracing::debug!("abort: cid 0x{:04X} {:?}", self.a2dp_cid, stack.avdtp_state_for_cid(self.a2dp_cid));
        match stack.avdtp_state_for_cid(self.a2dp_cid) {
            // Already finalized by BTstack (e.g. signaling connect failed)
            AvdtpState::None => false,
            // Signaling channel up: normal close, RELEASED follows
            AvdtpState::Open => {
                stack.a2dp_disconnect(self.a2dp_cid);
                true
            }
            // Not open yet (SDP query / waiting for L2CAP): the disconnect
            // emits a failed CONNECTION_ESTABLISHED and frees the connection
            // at once, but a pending outgoing L2CAP channel would be
            // orphaned: drop the ACL too if it is up (never one still
            // paging: no valid handle yet)
            AvdtpState::Pending => {
                stack.a2dp_disconnect(self.a2dp_cid);
                if let Some(addr) = self.remote_addr
                    && let AclState::Open(handle) = stack.acl_state(&addr)
                {
                    tracing::debug!("abort: dropping ACL handle 0x{handle:04X} of the pending connection");
                    stack.disconnect(handle);
                }
                false
            }
        }
    }

    /// After an abort: this was never a live stream. RFCOMM channels to the
    /// device go too, in case no signaling release came to close them.
    pub fn forget_connection(&mut self, stack: &Stack) {
        tracing::debug!("forget connection (cid 0x{:04X}, streaming {})", self.a2dp_cid, self.streaming);
        if let Some(addr) = self.remote_addr {
            self.rfcomm.close_device(stack, addr);
        }
        self.a2dp_cid = 0;
        self.set_streaming(false);
        self.can_send_requested = false;
    }

    /// HCI Disconnect of the ACL link to the remote. True if one is going down.
    pub fn drop_acl(&mut self, stack: &Stack) -> bool {
        let Some(addr) = self.remote_addr else { return false };
        let state = stack.acl_state(&addr);
        tracing::debug!("drop ACL: {} {state:?}", addr_str(&addr));
        match state {
            AclState::Open(handle) => stack.disconnect(handle) == ERROR_CODE_SUCCESS,
            _ => false,
        }
    }

    pub fn set_config(&mut self, stack: &Stack, config: &Configuration) -> u8 {
        let Some(&local) = self.endpoints.get(&codec::caps::endpoint_kind(config.kind)) else {
            tracing::debug!("set configuration: no local endpoint for {}", config.kind);
            return ERROR_CODE_COMMAND_DISALLOWED;
        };
        self.local_seid = local;
        self.remote_seid = config.remote_seid;
        self.stream_codec = Some(config.kind);
        let cid = self.a2dp_cid;
        tracing::debug!(
            "set configuration: cid 0x{cid:04X}, local SEID {local}, remote SEID {}, {} {:?}",
            config.remote_seid,
            config.kind,
            config.request
        );
        let status = match &config.request {
            ConfigRequest::Sbc {
                sampling_frequency,
                channel_mode,
                blocks,
                subbands,
                allocation,
                min_bitpool,
                max_bitpool,
            } => stack.a2dp_set_config_sbc(
                cid,
                local,
                config.remote_seid,
                *sampling_frequency,
                match channel_mode {
                    ChannelMode::Mono => AVDTP_CHANNEL_MODE_MONO,
                    ChannelMode::DualChannel => AVDTP_CHANNEL_MODE_DUAL_CHANNEL,
                    ChannelMode::Stereo => AVDTP_CHANNEL_MODE_STEREO,
                    ChannelMode::JointStereo => AVDTP_CHANNEL_MODE_JOINT_STEREO,
                },
                match blocks {
                    4 => AVDTP_SBC_BLOCK_LENGTH_4,
                    8 => AVDTP_SBC_BLOCK_LENGTH_8,
                    12 => AVDTP_SBC_BLOCK_LENGTH_12,
                    _ => AVDTP_SBC_BLOCK_LENGTH_16,
                },
                if *subbands == 4 { AVDTP_SBC_SUBBANDS_4 } else { AVDTP_SBC_SUBBANDS_8 },
                match allocation {
                    Allocation::Loudness => AVDTP_SBC_ALLOCATION_METHOD_LOUDNESS,
                    Allocation::Snr => AVDTP_SBC_ALLOCATION_METHOD_SNR,
                },
                *min_bitpool,
                *max_bitpool,
            ),
            ConfigRequest::Aac { object_type, sampling_frequency, channels, bit_rate, vbr } => stack
                .a2dp_set_config_aac(
                    cid,
                    local,
                    config.remote_seid,
                    match object_type {
                        AacObjectType::Mpeg2Lc => AVDTP_AAC_MPEG2_LC,
                        AacObjectType::Mpeg4Lc => AVDTP_AAC_MPEG4_LC,
                    },
                    *sampling_frequency,
                    *channels,
                    *bit_rate,
                    *vbr,
                ),
            ConfigRequest::Vendor(info) => stack.a2dp_set_config_other(cid, local, config.remote_seid, info),
        };
        tracing::debug!("set configuration: request status 0x{status:02X}");
        status
    }

    pub fn start_stream(&mut self, stack: &Stack) -> u8 {
        if self.a2dp_cid == 0 || self.local_seid == 0 {
            tracing::debug!("start: no stream (cid 0x{:04X}, local SEID {})", self.a2dp_cid, self.local_seid);
            return ERROR_CODE_COMMAND_DISALLOWED;
        }
        let status = stack.a2dp_start_stream(self.a2dp_cid, self.local_seid);
        tracing::debug!(
            "start: AVDTP start (cid 0x{:04X}, local SEID {}) status 0x{status:02X}",
            self.a2dp_cid,
            self.local_seid
        );
        status
    }

    pub fn pause_stream(&mut self, stack: &Stack) {
        if self.a2dp_cid != 0 && self.local_seid != 0 {
            let status = stack.a2dp_pause_stream(self.a2dp_cid, self.local_seid);
            tracing::debug!(
                "suspend: AVDTP suspend (cid 0x{:04X}, local SEID {}) status 0x{status:02X}",
                self.a2dp_cid,
                self.local_seid
            );
        }
        self.set_streaming(false);
    }

    /// Paging long after the page timeout: the controller lost the Create
    /// Connection, or BTstack never got to send it
    pub fn still_paging(&self, stack: &Stack) -> bool {
        self.remote_addr.is_some_and(|a| stack.acl_state(&a) == AclState::Paging)
    }

    /// Sends HCI Read BD_ADDR for a liveness probe. False if HCI cannot send
    /// a command now (one still waits for its answer).
    pub fn probe(&mut self, stack: &Stack) -> bool {
        if !stack.can_send_command_now() {
            tracing::trace!("probe: HCI busy");
            return false;
        }
        let status = stack.cmd_read_bd_addr();
        tracing::debug!("probe: Read BD_ADDR sent (0x{status:02X})");
        status == ERROR_CODE_SUCCESS
    }

    /// Sends the queued volume request, if AVRCP is free
    pub fn send_pending_volume(&mut self, stack: &Stack) {
        if self.avrcp_cid == 0 {
            return;
        }
        if self.volume_in_flight.is_some_and(|t| t.elapsed() < VOLUME_RESPONSE_TIMEOUT) {
            tracing::debug!("volume: a request is in flight, queued");
            return;
        }
        if let Some(t) = self.volume_in_flight {
            tracing::debug!("volume: no response for {} ms, sending the next", t.elapsed().as_millis());
        }
        let v = self.controls.volume_request.swap(-1, Ordering::AcqRel);
        if v < 0 {
            return;
        }
        let status = stack.avrcp_set_absolute_volume(self.avrcp_cid, v as u8);
        if status != ERROR_CODE_SUCCESS {
            tracing::debug!("volume: set absolute volume {v} busy (0x{status:02X}), kept queued");
            // Busy with another AVRCP command: keep it unless a newer one came
            let _ = self.controls.volume_request.compare_exchange(-1, v, Ordering::AcqRel, Ordering::Acquire);
            return;
        }
        tracing::debug!("volume: set absolute volume {v} sent (avrcp cid 0x{:04X})", self.avrcp_cid);
        self.volume_in_flight = Some(Instant::now());
        self.controls.volume.store(v, Ordering::Release);
    }

    // ======================================================================
    // Media
    // ======================================================================

    /// The stream started or stopped. Packets left from before a suspend or
    /// a lost link are stale: dropped before the encode thread may queue
    /// again (v1 flushed after a reconnect).
    fn set_streaming(&mut self, on: bool) {
        if on && !self.streaming {
            let stale = self.media.slots();
            if stale > 0 {
                if let Ok(chunk) = self.media.read_chunk(stale) {
                    chunk.commit_all();
                }
                tracing::info!("dropped {stale} media packet(s) queued before the stream started");
            }
            self.stats.queue_depth.store(0, Ordering::Relaxed);
        }
        if on != self.streaming {
            tracing::debug!(
                "media: streaming {on} (codec {:?}, cid 0x{:04X}, local SEID {})",
                self.stream_codec,
                self.a2dp_cid,
                self.local_seid
            );
        }
        self.streaming = on;
        self.media_streaming.store(on, Ordering::Release);
        // Gaps are measured within one started stream
        self.can_send_asked = None;
        self.last_media_sent = None;
        self.last_completed = None;
    }

    fn kick_media(&mut self, stack: &Stack) {
        if self.streaming && !self.can_send_requested && self.a2dp_cid != 0 && !self.media.is_empty() {
            self.can_send_requested = true;
            self.can_send_asked = Some(Instant::now());
            tracing::trace!("media: request can send now (queued {})", self.media.slots());
            stack.a2dp_request_can_send_now(self.a2dp_cid, self.local_seid);
        }
    }

    fn can_send_now(&mut self, stack: &Stack) {
        self.can_send_requested = false;
        let now = Instant::now();
        if let Some(asked) = self.can_send_asked.take() {
            self.stats.timing.can_send_wait_us.record_duration(now - asked);
        }
        if let Ok(packet) = self.media.pop() {
            let depth = self.media.slots() as u32;
            self.stats.queue_depth.store(depth, Ordering::Relaxed);
            // Remote L2CAP MTU minus the RTP header; aptX / aptX LL go
            // without one
            let max = stack
                .a2dp_max_media_payload_size(self.a2dp_cid, self.local_seid)
                .map(|m| if packet.rtp { m as usize } else { m as usize + RTP_HEADER_SIZE });
            let payload = packet.payload();
            if codec::dev::overrides().packet_dump && packet.rtp {
                static DUMPED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
                if DUMPED.fetch_add(1, Ordering::Relaxed) < 6 {
                    let head: Vec<String> = payload.iter().take(24).map(|b| format!("{b:02X}")).collect();
                    tracing::info!(
                        "media packet: {} bytes, ts {}: {}",
                        payload.len(),
                        packet.timestamp,
                        head.join(" ")
                    );
                }
            }
            if max.is_some_and(|m| payload.len() <= m) {
                let status = if packet.rtp {
                    stack.a2dp_send_media_payload_rtp(
                        self.a2dp_cid,
                        self.local_seid,
                        u8::from(packet.marker),
                        packet.timestamp,
                        payload,
                    )
                } else {
                    stack.a2dp_send_media_packet(self.a2dp_cid, self.local_seid, payload)
                };
                if status == ERROR_CODE_SUCCESS {
                    if let Some(last) = self.last_media_sent.replace(now) {
                        self.stats.timing.send_gap_us.record_duration(now - last);
                    }
                    self.stats.packets_sent.fetch_add(1, Ordering::Relaxed);
                    let bytes = payload.len() + if packet.rtp { RTP_HEADER_SIZE } else { 0 };
                    self.stats.bytes_sent.fetch_add(bytes as u64, Ordering::Relaxed);
                    tracing::trace!(
                        "media: sent {} bytes, ts {}, marker {}, rtp {}, queued {depth}",
                        payload.len(),
                        packet.timestamp,
                        packet.marker,
                        packet.rtp
                    );
                } else {
                    let n = self.stats.send_errors.fetch_add(1, Ordering::Relaxed) + 1;
                    if n <= 10 || n.is_power_of_two() {
                        tracing::debug!(
                            "media: send failed 0x{status:02X} ({} bytes, ts {}), {n} send error(s) so far",
                            payload.len(),
                            packet.timestamp
                        );
                    }
                }
            } else {
                let n = self.stats.send_errors.fetch_add(1, Ordering::Relaxed) + 1;
                if n <= 10 || n.is_power_of_two() {
                    tracing::debug!(
                        "media: packet of {} bytes over the max payload {max:?}, dropped, {n} send error(s) so far",
                        payload.len()
                    );
                }
            }
        } else {
            tracing::trace!("media: can send now, queue empty");
        }
        // Chain the next one; when empty, the next queued packet wakes us
        self.kick_media(stack);
    }

    // ======================================================================
    // HCI events
    // ======================================================================

    fn handle_hci(&mut self, stack: &Stack, p: &[u8]) {
        let Some(ev) = event_type(p) else { return };
        // Events from the controller (not BTstack's own 0x60..0xFE)
        if ev < 0x60 || ev == HCI_EVENT_VENDOR_SPECIFIC {
            self.shared.update(|s| s.controller_events += 1);
        }
        if ev == HCI_EVENT_NUMBER_OF_COMPLETED_PACKETS && self.streaming {
            let now = Instant::now();
            if let Some(last) = self.last_completed.replace(now) {
                self.stats.timing.completed_gap_us.record_duration(now - last);
            }
            self.stats.timing.completed_events.fetch_add(1, Ordering::Relaxed);
        }
        if ev == HCI_EVENT_NUMBER_OF_COMPLETED_PACKETS {
            tracing::trace!("HCI completed packets: {:02X?}", p.get(2..));
        }
        match ev {
            BTSTACK_EVENT_STATE => match u8_at(p, 2) {
                Some(state @ (1 | 3..)) => tracing::debug!("HCI state {state} ({:?})", stack.hci_state()),
                Some(2) => self.on_working(stack),
                Some(0) => {
                    tracing::info!("HCI state OFF");
                    stack.timer_stop(TIMER_AFH);
                    stack.timer_stop(TIMER_PLAYBACK);
                    stack.timer_stop(TIMER_RTK_VERIFY);
                    self.stats.with_radio(|r| *r = Default::default());
                    self.shared.update(|s| {
                        s.hci_ready = false;
                        s.hci_off = true;
                    });
                }
                _ => {}
            },
            HCI_EVENT_COMMAND_STATUS => {
                if let (Some(status), Some(opcode)) = (u8_at(p, 2), le16(p, 4))
                    && status != ERROR_CODE_SUCCESS
                {
                    tracing::debug!("HCI command 0x{opcode:04X} status 0x{status:02X} ({})", hci_error_string(status));
                    // An inquiry the controller refused: BTstack goes idle
                    // without an Inquiry Complete, the scan would wait for it
                    if opcode == HCI_OPCODE_INQUIRY {
                        self.shared.update(|s| s.inquiry_done = true);
                    }
                }
            }
            HCI_EVENT_COMMAND_COMPLETE => {
                let opcode = le16(p, 3).unwrap_or(0);
                tracing::trace!("HCI command complete 0x{opcode:04X} status {:?}", u8_at(p, 5));
                if let Some(status) = u8_at(p, 5)
                    && status != ERROR_CODE_SUCCESS
                {
                    tracing::debug!(
                        "HCI command 0x{opcode:04X} complete with 0x{status:02X} ({})",
                        hci_error_string(status)
                    );
                }
                if opcode == HCI_OPCODE_READ_LOCAL_VERSION_INFORMATION && p.len() >= 14 {
                    self.on_local_version(stack, p);
                }
                if opcode == HCI_OPCODE_READ_BD_ADDR {
                    // Status, then the address little-endian
                    if p.len() >= 12 && p[5] == 0 {
                        let a: Vec<String> = p[6..12].iter().rev().map(|b| format!("{b:02X}")).collect();
                        let addr = a.join(":");
                        tracing::debug!("controller address {addr}");
                        self.shared.update(|s| s.local_address = Some(addr));
                    }
                    self.shared.update(|s| {
                        if s.probe_pending {
                            s.probe_pending = false;
                            s.probe_answered = true;
                        }
                    });
                }
                self.on_radio_command_complete(opcode, p);
            }
            HCI_EVENT_USER_CONFIRMATION_REQUEST => {
                // Just Works
                if let Some(addr) = addr_at(p, 2) {
                    tracing::info!("accepting pairing with {}", addr_str(&addr));
                    stack.ssp_confirmation_response(&addr);
                }
            }
            HCI_EVENT_PIN_CODE_REQUEST => {
                // Legacy pairing: the default PIN
                if let Some(addr) = addr_at(p, 2) {
                    tracing::info!("PIN code requested by {} (legacy pairing), answering 0000", addr_str(&addr));
                    stack.pin_code_response(&addr, "0000");
                }
            }
            // Security steps, logged so that an authentication failure
            // shows which side refused (BTstack answers these itself)
            HCI_EVENT_LINK_KEY_REQUEST => {
                if let Some(addr) = addr_at(p, 2) {
                    tracing::info!("link key requested for {}", addr_str(&addr));
                }
            }
            HCI_EVENT_LINK_KEY_NOTIFICATION => {
                // The key itself (bytes 8..24) is never logged
                if let (Some(addr), Some(kind)) = (addr_at(p, 2), u8_at(p, 24)) {
                    tracing::info!("new link key from pairing with {} (type {kind})", addr_str(&addr));
                }
            }
            HCI_EVENT_IO_CAPABILITY_RESPONSE => {
                if let (Some(addr), Some(io), Some(oob), Some(auth)) =
                    (addr_at(p, 2), u8_at(p, 8), u8_at(p, 9), u8_at(p, 10))
                {
                    tracing::info!(
                        "pairing: {} answers IO capability {io}, OOB {oob}, authentication requirements 0x{auth:02X}",
                        addr_str(&addr)
                    );
                }
            }
            HCI_EVENT_SIMPLE_PAIRING_COMPLETE => {
                if let (Some(status), Some(addr)) = (u8_at(p, 2), addr_at(p, 3)) {
                    if status == 0 {
                        tracing::info!("pairing with {} complete", addr_str(&addr));
                    } else {
                        tracing::warn!(
                            "pairing with {} failed: 0x{status:02X} ({})",
                            addr_str(&addr),
                            hci_error_string(status)
                        );
                    }
                }
            }
            HCI_EVENT_AUTHENTICATION_COMPLETE => {
                if let (Some(status), Some(handle)) = (u8_at(p, 2), le16(p, 3)) {
                    let handle = handle & 0x0FFF;
                    // What connect_retrying decides on: a key the device
                    // refused vs. a collision with its own authentication
                    self.shared.update(|s| {
                        if s.target_acl_handle == Some(handle) {
                            s.target_auth_status = Some(status);
                        }
                    });
                    if status == 0 {
                        tracing::info!("authentication complete (handle 0x{handle:04x})");
                    } else {
                        tracing::warn!(
                            "authentication failed (handle 0x{handle:04x}): 0x{status:02X} ({})",
                            hci_error_string(status)
                        );
                    }
                }
            }
            // LE links log theirs in le::core
            HCI_EVENT_ENCRYPTION_CHANGE => {
                if let (Some(status), Some(handle), Some(on)) = (u8_at(p, 2), le16(p, 3), u8_at(p, 5)) {
                    let handle = handle & 0x0FFF;
                    if self.shared.lock().target_acl_handle == Some(handle) {
                        if status == 0 {
                            tracing::info!("encryption {} (handle 0x{handle:04x})", if on != 0 { "on" } else { "off" });
                        } else {
                            tracing::warn!(
                                "encryption failed (handle 0x{handle:04x}): 0x{status:02X} ({})",
                                hci_error_string(status)
                            );
                        }
                    }
                }
            }
            HCI_EVENT_CONNECTION_REQUEST => {
                // BTstack accepts it; the link that comes up is the device's own
                let Some(addr) = addr_at(p, 2) else { return };
                tracing::debug!("HCI connection request from {}", addr_str(&addr));
                self.shared.update(|s| {
                    if !s.acl_requests.contains(&addr) {
                        s.acl_requests.push(addr);
                    }
                });
            }
            HCI_EVENT_CONNECTION_COMPLETE => {
                let (Some(status), Some(handle), Some(addr)) = (u8_at(p, 2), le16(p, 3), addr_at(p, 5)) else {
                    return;
                };
                tracing::debug!(
                    "HCI connection complete: {} status 0x{status:02X}, handle 0x{:04X}, link type {:?}, encryption {:?}, target {}",
                    addr_str(&addr),
                    handle & 0x0FFF,
                    u8_at(p, 11),
                    u8_at(p, 12),
                    self.remote_addr == Some(addr)
                );
                if status != 0 {
                    self.shared.update(|s| s.acl_requests.retain(|&a| a != addr));
                    tracing::warn!(
                        "HCI connection to {} failed: 0x{status:02X} ({})",
                        addr_str(&addr),
                        hci_error_string(status)
                    );
                } else {
                    tracing::info!("HCI ACL connection established to {}", addr_str(&addr));
                    let target = self.remote_addr == Some(addr);
                    self.shared.update(|s| {
                        s.acl_up_from = Some(addr);
                        let by_device = s.acl_requests.contains(&addr);
                        s.acl_requests.retain(|&a| a != addr);
                        s.acl_links.retain(|&(_, a, _)| a != addr);
                        s.acl_links.push((handle & 0x0FFF, addr, by_device));
                        if target {
                            s.target_acl_down_reason = 0;
                            s.target_acl_handle = Some(handle & 0x0FFF);
                        }
                    });
                }
            }
            GAP_EVENT_INQUIRY_RESULT => {
                let (Some(addr), Some(psrm), Some(cod), Some(clock), Some(rssi)) =
                    (addr_at(p, 2), u8_at(p, 8), le24(p, 9), le16(p, 12), u8_at(p, 15))
                else {
                    return;
                };
                let name = if u8_at(p, 25) == Some(1) {
                    let len = u8_at(p, 26).unwrap_or(0) as usize;
                    p.get(27..27 + len).map(|n| String::from_utf8_lossy(n).into_owned()).unwrap_or_default()
                } else {
                    String::new()
                };
                let line =
                    format!("inquiry result: {} CoD=0x{cod:06X} RSSI={} name=“{name}”", addr_str(&addr), rssi as i8);
                let d = DiscoveredDevice { addr, name, class_of_device: cod, rssi: rssi as i8 };
                // A device answers again and again (inquiry with RSSI / EIR):
                // the repeats at trace
                if self.shared.update(|s| !s.inquiry_done && s.add_found(d, psrm, clock)) {
                    tracing::info!("{line}");
                } else {
                    tracing::trace!("{line}");
                }
            }
            HCI_EVENT_REMOTE_NAME_REQUEST_COMPLETE => {
                let (Some(status), Some(addr)) = (u8_at(p, 2), addr_at(p, 3)) else { return };
                // Up to 248 bytes, NUL-terminated when shorter
                let raw = p.get(9..).unwrap_or_default();
                let raw = &raw[..raw.iter().position(|&b| b == 0).unwrap_or(raw.len())];
                let name = String::from_utf8_lossy(raw).trim().to_string();
                tracing::info!(
                    "remote name of {}: status 0x{status:02X} name={}",
                    addr_str(&addr),
                    format_args!("“{name}”")
                );
                self.shared.update(|s| {
                    if status == 0 && !name.is_empty() {
                        s.set_found_name(&addr, &name);
                    }
                    s.name_done = true;
                });
            }
            GAP_EVENT_INQUIRY_COMPLETE => {
                tracing::debug!("inquiry complete (status {:?})", u8_at(p, 2));
                self.shared.update(|s| s.inquiry_done = true);
            }
            HCI_EVENT_DISCONNECTION_COMPLETE => {
                let (Some(handle), Some(reason)) = (le16(p, 3), u8_at(p, 5)) else { return };
                let handle = handle & 0x0FFF;
                tracing::info!(
                    "HCI disconnection (handle 0x{handle:04x}, reason 0x{reason:02x}: {})",
                    hci_error_string(reason)
                );
                let lost = self.shared.update(|s| {
                    s.acl_down = true;
                    s.acl_links.retain(|&(h, _, _)| h != handle);
                    if s.target_acl_handle == Some(handle) {
                        s.target_acl_down_reason = reason;
                        s.target_acl_handle = None;
                    }
                    // Other links (e.g. a device used before that drops its
                    // idle link a moment later) must not end this stream
                    if let Some(a2dp) = s.a2dp_con_handle
                        && a2dp != handle
                    {
                        tracing::info!("not the A2DP link (handle 0x{a2dp:04x}), stream unaffected");
                        return false;
                    }
                    // Also when A2DP was released just before (headphones
                    // switched off release it first, then the link)
                    if s.a2dp_con_handle.take().is_some() || s.connected || s.streaming {
                        s.link_lost_reason = reason;
                    }
                    if s.connected || s.streaming {
                        s.connected = false;
                        s.streaming = false;
                        s.disconnect_occurred = true;
                        return true;
                    }
                    false
                });
                tracing::debug!(
                    "disconnection of handle 0x{handle:04X}: a2dp link lost {lost}, hfp acl {:?}, streaming {}",
                    self.hfp_acl,
                    self.streaming
                );
                if lost {
                    self.set_streaming(false);
                    self.can_send_requested = false;
                    self.stats.queue_depth.store(0, Ordering::Relaxed);
                    tracing::warn!("connection lost");
                    let _ = self.events.send(HostEvent::ConnectionLost);
                }
            }
            HCI_EVENT_A2DP_META => self.handle_a2dp(stack, p),
            HCI_EVENT_AVRCP_META => self.handle_avrcp(stack, p),
            _ => {}
        }
    }

    fn on_working(&mut self, stack: &Stack) {
        let elapsed = self.init_started.elapsed().as_millis();
        tracing::info!("HCI ready (took {elapsed} ms)");
        tracing::debug!(
            "realtek check: pid 0x{:04X}, version seen {}, first LMP subversion 0x{:04X} (ROM {})",
            self.setup.realtek_pid,
            self.rtk_version_seen,
            self.rtk_lmp_first,
            is_realtek_rom_lmp(self.rtk_lmp_first)
        );
        if self.setup.realtek_pid != 0 {
            if self.rtk_version_seen {
                // Judged by the LMP subversion, as BTstack's Realtek driver
                // does: firmware loaded in an earlier session survives until
                // the adapter loses power, and the download is then skipped
                if !is_realtek_rom_lmp(self.rtk_lmp_first) {
                    tracing::info!(
                        "Realtek firmware already running from an earlier session (LMP subversion 0x{:04X}), download skipped",
                        self.rtk_lmp_first
                    );
                } else if let [fw, ..] = self.missing_realtek_files(false).as_slice() {
                    // Issue #81: without the firmware file there was nothing
                    // to download, the controller stays on its ROM code
                    tracing::warn!(
                        "Realtek firmware NOT loaded (LMP subversion 0x{:04X} is the ROM code; {fw} missing in {})",
                        self.rtk_lmp_first,
                        self.setup.firmware_dir.display()
                    );
                    self.shared.update(|s| s.realtek_rom = true);
                } else {
                    // HCI init read the version only before the download
                    self.rtk_verify_tries = RTK_VERIFY_TRIES;
                    self.verify_realtek_download(stack);
                }
            } else if let Some(company) = self.controller_company.filter(|&c| c != COMPANY_ID_REALTEK) {
                // Another adapter on WinUSB opened instead of the Realtek
                // one: its quick init says nothing about Realtek firmware
                tracing::warn!(
                    "the opened controller is {} (company 0x{company:04X}), not the Realtek chip 0x{:04X} the setup is for: no Realtek firmware check",
                    crate::chips::company_name(company).unwrap_or("unknown"),
                    self.setup.realtek_pid
                );
            } else if elapsed < 500 {
                tracing::warn!("HCI init took < 500 ms: Realtek firmware likely NOT loaded");
                self.shared.update(|s| s.realtek_rom = true);
            }
        }
        let le_audio_capable = stack.controller_iso() == Some(true);
        tracing::info!(
            "controller: LE Audio isochronous channels {}",
            if le_audio_capable { "supported" } else { "not supported" }
        );
        self.shared.update(|s| {
            s.hci_ready = true;
            s.hci_off = false;
            s.le_audio_capable = le_audio_capable;
            s.init_done = Some(Ok(()));
        });
        self.afh_ticks = 0;
        self.afh_policy_version_seen = None;
        self.afh_sent_map = None;
        stack.timer_start(TIMER_AFH, 200);
    }

    fn on_local_version(&mut self, stack: &Stack, p: &[u8]) {
        // Command Complete: [5] status [6] HCI version [9] LMP version
        // [10..11] manufacturer [12..13] LMP subversion
        if u8_at(p, 5) != Some(0) {
            tracing::debug!("Read Local Version failed (status {:?})", u8_at(p, 5));
            return;
        }
        let hci_ver = u8_at(p, 6).unwrap_or(0);
        if let Some(v) = u8_at(p, 6) {
            self.shared.update(|s| s.hci_version = Some(v));
        }
        let manufacturer = le16(p, 10).unwrap_or(0);
        let lmp_subver = le16(p, 12).unwrap_or(0);
        tracing::debug!(
            "local version: HCI revision 0x{:04X}, LMP version {}, state {:?}, verify pending {}",
            le16(p, 7).unwrap_or(0),
            u8_at(p, 9).unwrap_or(0),
            stack.hci_state(),
            self.rtk_verify_pending
        );
        tracing::info!(
            "controller: company 0x{manufacturer:04X} ({}), HCI version {hci_ver}, LMP subversion 0x{lmp_subver:04X}",
            crate::chips::company_name(manufacturer).unwrap_or("unknown")
        );
        self.controller_company = Some(manufacturer);
        if manufacturer == COMPANY_ID_REALTEK && self.setup.realtek_pid != 0 {
            // The first read of an init comes before the firmware download
            if !self.rtk_version_seen {
                self.rtk_lmp_first = lmp_subver;
            }
            self.rtk_version_seen = true;
            if self.rtk_verify_pending {
                self.rtk_verify_pending = false;
                if is_realtek_rom_lmp(lmp_subver) {
                    tracing::warn!(
                        "Realtek firmware NOT loaded (LMP subversion 0x{lmp_subver:04X} is the ROM code); check the firmware files"
                    );
                    self.shared.update(|s| s.realtek_rom = true);
                } else {
                    tracing::info!(
                        "Realtek firmware loaded (LMP subversion 0x{:04X} -> 0x{lmp_subver:04X})",
                        self.rtk_lmp_first
                    );
                }
            }
        }
        if matches!(manufacturer, 0x0046 | 0x001D | 0x0045) {
            tracing::warn!(
                "this vendor needs a firmware download that is not implemented; the controller runs its ROM firmware only"
            );
        }
        // Broadcom (Cypress / Infineon map to it inside BTstack): the
        // PatchRAM goes in during custom init, which follows this command;
        // after the upload BTstack resets without re-reading the version
        if self.bcm_checked || self.setup.realtek_pid != 0 || self.setup.tcp {
            return;
        }
        if manufacturer != COMPANY_ID_BROADCOM || stack.hci_state() != HciState::Initializing {
            return;
        }
        self.bcm_checked = true;
        tracing::debug!(
            "broadcom: looking for PatchRAM in {} for adapters {:04X?}",
            self.setup.firmware_dir.display(),
            self.setup.broadcom_ids
        );
        match find_bcm_hcd(&self.setup.firmware_dir, &self.setup.broadcom_ids) {
            Some(path) => {
                tracing::info!("Broadcom PatchRAM {} (experimental)", path.display());
                stack.set_chipset_bcm(&path.to_string_lossy());
            }
            None => tracing::info!(
                "Broadcom controller: no usable .hcd in {}, running ROM firmware",
                self.setup.firmware_dir.display()
            ),
        }
    }

    // ======================================================================
    // A2DP / AVDTP
    // ======================================================================

    fn handle_sep(&mut self, p: &[u8]) {
        if event_type(p) != Some(HCI_EVENT_AVDTP_META) {
            return;
        }
        match subevent(p) {
            Some(AVDTP_SUBEVENT_SIGNALING_SEP_FOUND) => {
                let (seid, in_use, media, sep_type) = (
                    u8_at(p, 5).unwrap_or(0),
                    u8_at(p, 6).unwrap_or(0) != 0,
                    u8_at(p, 7).unwrap_or(0xFF),
                    u8_at(p, 8).unwrap_or(0xFF),
                );
                if sep_type == AVDTP_SINK && media == AVDTP_AUDIO {
                    self.sep_sinks += 1;
                    if in_use {
                        self.sep_sinks_in_use += 1;
                    }
                }
                tracing::info!(
                    "remote SEP {seid}: {} {}{}",
                    if sep_type == AVDTP_SINK { "sink" } else { "source" },
                    if media == AVDTP_AUDIO { "audio" } else { "other" },
                    if in_use { ", IN USE (codec not queried)" } else { "" }
                );
            }
            Some(AVDTP_SUBEVENT_SIGNALING_SEP_DICOVERY_DONE) => {
                // Counted per discovery: a later one replaces the result
                tracing::debug!(
                    "SEP discovery done: {} audio sink(s), {} in use",
                    self.sep_sinks,
                    self.sep_sinks_in_use
                );
                self.caps.audio_sinks = self.sep_sinks;
                self.caps.audio_sinks_in_use = self.sep_sinks_in_use;
                self.sep_sinks = 0;
                self.sep_sinks_in_use = 0;
                if self.caps.audio_sinks_in_use > 0 {
                    tracing::info!(
                        "remote SEPs in use by another connection: {} of {} audio sink SEP(s); their codecs stay unavailable \
                         while the device streams to it (e.g. a phone on a dual connection)",
                        self.caps.audio_sinks_in_use,
                        self.caps.audio_sinks
                    );
                }
            }
            _ => {}
        }
    }

    fn handle_a2dp(&mut self, stack: &Stack, p: &[u8]) {
        match subevent(p) {
            Some(A2DP_SUBEVENT_SIGNALING_CONNECTION_ESTABLISHED) => {
                let (Some(cid), Some(handle), Some(status)) = (le16(p, 3), le16(p, 11), u8_at(p, 13)) else { return };
                if self.a2dp_cid != 0 && cid != self.a2dp_cid {
                    // Late event of an old / aborted connection: must not
                    // clobber the one being established
                    tracing::info!(
                        "ignoring a signaling event of stale cid 0x{cid:04x} (current 0x{:04x})",
                        self.a2dp_cid
                    );
                    return;
                }
                self.a2dp_cid = cid;
                if status != ERROR_CODE_SUCCESS {
                    tracing::warn!("signaling connection failed (0x{status:02x})");
                    let failure = signaling_failure(status);
                    tracing::debug!(
                        "signaling connection failed: cid 0x{cid:04X}, status 0x{status:02X} ({}) -> {failure:?}",
                        hci_error_string(status)
                    );
                    self.shared.update(|s| {
                        s.last_connect_failure = Some(failure);
                        s.connect_done = Some(false);
                    });
                } else {
                    tracing::info!("signaling connection established (cid 0x{cid:04x}, handle 0x{handle:04x})");
                    // Not done yet: the capability discovery follows
                    self.shared.update(|s| s.a2dp_con_handle = Some(handle & 0x0FFF));
                }
            }
            Some(A2DP_SUBEVENT_SIGNALING_MEDIA_CODEC_OTHER_CAPABILITY) => {
                let seid = u8_at(p, 5).unwrap_or(0);
                let len = le16(p, 9).unwrap_or(0) as usize;
                // The length is the remote's; BTstack copies at most 30
                // bytes (AVDTP_MAX_MEDIA_CODEC_INFORMATION_LENGTH): aptX
                // Adaptive's 40 arrive cut to 30, which still hold all it uses
                let Some(info) = p.get(11..).map(|rest| &rest[..len.min(rest.len())]) else { return };
                tracing::debug!("SEID {seid} codec capability (header {:02X?}): {info:02X?}", p.get(3..11));
                match self.caps.add_vendor(seid, info) {
                    VendorCapability::Taken(kind) => {
                        tracing::info!("remote supports {kind} (SEID {seid}, {info:02X?})")
                    }
                    VendorCapability::Ignored(kind, why) => {
                        tracing::info!("remote supports {kind} (SEID {seid}) - {why}, ignored")
                    }
                    VendorCapability::AptxAdaptive => tracing::info!(
                        "remote supports aptX Adaptive (SEID {seid}) - not built in; classic aptX is used only \
                         if the remote lists it"
                    ),
                    VendorCapability::Unknown { vendor_id, codec_id } => tracing::info!(
                        "remote vendor codec 0x{vendor_id:08X}/0x{codec_id:04X} (SEID {seid}) - unsupported"
                    ),
                    VendorCapability::TooShort => {}
                }
            }
            Some(A2DP_SUBEVENT_SIGNALING_MEDIA_CODEC_SBC_CAPABILITY) => {
                let seid = u8_at(p, 5).unwrap_or(0);
                // 12111111111: subevent, cid, SEID, media type, then the
                // bitmaps and the bitpool range
                let at = |i: usize| u8_at(p, i).unwrap_or(0);
                tracing::debug!("SEID {seid} SBC capability event: {:02X?}", p.get(3..));
                let sbc = SbcCapabilities {
                    sampling_frequency: at(7),
                    channel_mode: at(8),
                    block_length: at(9),
                    subbands: at(10),
                    allocation_method: at(11),
                    min_bitpool: at(12),
                    max_bitpool: at(13),
                };
                self.caps.add_sbc_caps(seid, sbc);
                tracing::info!(
                    "remote supports SBC (SEID {seid}, frequencies 0x{:X}, channel modes 0x{:X}, blocks 0x{:X}, \
                     subbands 0x{:X}, allocation 0x{:X}, bitpool {}-{}){}",
                    sbc.sampling_frequency,
                    sbc.channel_mode,
                    sbc.block_length,
                    sbc.subbands,
                    sbc.allocation_method,
                    sbc.min_bitpool,
                    sbc.max_bitpool,
                    match sbc.xq_refusal() {
                        None => " - SBC XQ possible".to_string(),
                        Some(why) => format!(" - no SBC XQ: {why}"),
                    }
                );
            }
            Some(A2DP_SUBEVENT_SIGNALING_MEDIA_CODEC_MPEG_AAC_CAPABILITY) => {
                let seid = u8_at(p, 5).unwrap_or(0);
                // 1211121311: subevent, cid, SEID, media type, object types,
                // frequencies, channels, bit rate, VBR, DRC
                let at = |i: usize| u8_at(p, i).unwrap_or(0);
                tracing::debug!("SEID {seid} AAC capability event: {:02X?}", p.get(3..));
                let aac = AacCapabilities {
                    object_types: at(7),
                    sampling_frequency: le16(p, 8).unwrap_or(0),
                    channels: at(10),
                    bit_rate: le24(p, 11).unwrap_or(0),
                    vbr: at(14) != 0,
                };
                self.caps.add_aac_caps(seid, aac);
                tracing::info!(
                    "remote supports AAC (SEID {seid}, object types 0x{:X}, frequencies 0x{:X}, channels 0x{:X}, \
                     bit rate {}, VBR {}){}",
                    aac.object_types,
                    aac.sampling_frequency,
                    aac.channels,
                    aac.bit_rate,
                    aac.vbr,
                    if aac.usable() { "" } else { " - not usable" }
                );
            }
            Some(A2DP_SUBEVENT_SIGNALING_CAPABILITIES_COMPLETE) => {
                let c = &self.caps;
                tracing::info!(
                    "capability discovery complete (LDAC={} aptX HD={} aptX LL={} aptX={} aptX Adaptive={}{} \
                     SBC={} SBC XQ={} AAC={} LHDC V5={} Opus={} Opus 05={} FastStream={}{})",
                    c.ldac.is_some(),
                    c.aptx_hd.is_some(),
                    c.aptx_ll.is_some(),
                    c.aptx.is_some(),
                    c.aptx_adaptive,
                    if c.aptx_adaptive && c.aptxad.is_none() { " [not usable]" } else { "" },
                    c.sbc.is_some(),
                    c.has(CodecKind::SbcXq),
                    c.aac.is_some(),
                    c.lhdcv5.is_some(),
                    c.opus.is_some(),
                    c.opus05.is_some(),
                    c.faststream.is_some(),
                    if c.audio_sinks_in_use > 0 {
                        format!(", {} of {} audio sink SEP(s) in use", c.audio_sinks_in_use, c.audio_sinks)
                    } else {
                        String::new()
                    }
                );
                let caps = self.caps.clone();
                tracing::debug!("remote caps: {caps:?}");
                self.shared.update(|s| {
                    s.remote_caps = caps;
                    s.connected = true;
                    s.connect_done = Some(true);
                });
                // AVRCP for volume and buttons
                if let Some(addr) = self.remote_addr
                    && let Err(status) = stack.avrcp_connect(&addr)
                {
                    tracing::warn!("AVRCP connect request failed (0x{status:02x})");
                } else {
                    tracing::debug!("AVRCP connect requested");
                }
            }
            Some(A2DP_SUBEVENT_SIGNALING_CAPABILITIES_DONE) => {
                tracing::debug!("capabilities of SEID {:?} done", u8_at(p, 5));
            }
            Some(A2DP_SUBEVENT_COMMAND_ACCEPTED) => {
                tracing::debug!(
                    "A2DP command accepted (cid 0x{:04X}, local SEID {:?}, initiator {:?}, signal {:02X?})",
                    le16(p, 3).unwrap_or(0),
                    u8_at(p, 5),
                    u8_at(p, 6),
                    u8_at(p, 7)
                );
            }
            Some(A2DP_SUBEVENT_STREAM_STOPPED) => {
                tracing::debug!("stream stopped (local SEID {:?})", u8_at(p, 5));
            }
            Some(A2DP_SUBEVENT_SIGNALING_MEDIA_CODEC_SBC_CONFIGURATION)
            | Some(A2DP_SUBEVENT_SIGNALING_MEDIA_CODEC_MPEG_AAC_CONFIGURATION)
            | Some(A2DP_SUBEVENT_SIGNALING_MEDIA_CODEC_OTHER_CONFIGURATION) => {
                tracing::debug!("codec configuration event: {:02X?}", p.get(2..));
                tracing::info!(
                    "codec configured (local {}, remote {})",
                    u8_at(p, 5).unwrap_or(0),
                    u8_at(p, 6).unwrap_or(0)
                );
            }
            Some(A2DP_SUBEVENT_STREAM_ESTABLISHED) => {
                let status = u8_at(p, 13).unwrap_or(0xFF);
                tracing::debug!(
                    "stream established event: cid 0x{:04X}, local SEID {:?}, remote SEID {:?}, status 0x{status:02X}",
                    le16(p, 3).unwrap_or(0),
                    u8_at(p, 11),
                    u8_at(p, 12)
                );
                if status != ERROR_CODE_SUCCESS {
                    tracing::warn!("stream establishment failed (0x{status:02x})");
                    self.shared.update(|s| s.stream_done = Some(false));
                    return;
                }
                self.local_seid = u8_at(p, 11).unwrap_or(0);
                self.remote_seid = u8_at(p, 12).unwrap_or(0);
                let remote_mtu = stack.a2dp_max_media_payload_size(self.a2dp_cid, self.local_seid).unwrap_or(679);
                // Encoders size packets from this; the limit also keeps every
                // payload within a queue slot
                let limit = self.controls.media_payload_limit.load(Ordering::Acquire) as u16;
                let mtu = remote_mtu.min(limit);
                tracing::info!(
                    "stream established (local {}, remote {}, mtu {remote_mtu}, used {mtu}, limit {limit})",
                    self.local_seid,
                    self.remote_seid
                );
                self.shared.update(|s| {
                    s.media_mtu = mtu;
                    s.stream_done = Some(true);
                });
                self.connect_hfp(stack);
            }
            Some(A2DP_SUBEVENT_STREAM_STARTED) => {
                tracing::info!("streaming started");
                self.stats.mark_stream_start();
                self.set_streaming(true);
                self.can_send_requested = false;
                self.shared.update(|s| {
                    s.streaming = true;
                    s.start_done = Some(true);
                });
                self.kick_media(stack);
            }
            Some(A2DP_SUBEVENT_STREAMING_CAN_SEND_MEDIA_PACKET_NOW) => self.can_send_now(stack),
            Some(A2DP_SUBEVENT_STREAM_SUSPENDED) => {
                tracing::info!("stream suspended");
                self.set_streaming(false);
                self.shared.update(|s| s.streaming = false);
            }
            Some(A2DP_SUBEVENT_STREAM_RELEASED) => {
                tracing::info!("stream released");
                self.set_streaming(false);
                self.shared.update(|s| s.streaming = false);
            }
            Some(A2DP_SUBEVENT_SIGNALING_CONNECTION_RELEASED) => {
                let cid = le16(p, 3).unwrap_or(0);
                if self.a2dp_cid != 0 && cid != self.a2dp_cid {
                    // Late release of an old / aborted connection
                    tracing::info!(
                        "ignoring a signaling release of stale cid 0x{cid:04x} (current 0x{:04x})",
                        self.a2dp_cid
                    );
                    return;
                }
                if self.avrcp_cid != 0 {
                    stack.avrcp_disconnect(self.avrcp_cid);
                    self.avrcp_cid = 0;
                }
                if let Some(acl) = self.hfp_acl.take() {
                    stack.hfp_ag_release(acl);
                }
                if let Some(addr) = self.remote_addr {
                    self.rfcomm.close_device(stack, addr);
                }
                self.shared.update(|s| s.battery = None);
                if self.a2dp_cid == 0 {
                    // No current connection: wake a pending disconnect wait,
                    // but report no loss
                    tracing::info!("signaling connection released (cid 0x{cid:04x}, no current connection)");
                    self.shared.update(|s| s.released = true);
                    return;
                }
                tracing::info!("signaling connection released (cid 0x{cid:04x})");
                self.a2dp_cid = 0;
                self.set_streaming(false);
                self.can_send_requested = false;
                self.stats.queue_depth.store(0, Ordering::Relaxed);
                self.shared.update(|s| {
                    s.connected = false;
                    s.streaming = false;
                    s.disconnect_occurred = true;
                    s.link_lost_reason = 0;
                    s.released = true;
                });
                let _ = self.events.send(HostEvent::ConnectionLost);
            }
            Some(A2DP_SUBEVENT_COMMAND_REJECTED) => {
                // 12111: cid, local seid, is initiator, signal identifier.
                // A rejected SET_CONFIGURATION / START of ours ends its wait
                // now instead of at the timeout (BTstack reports neither as
                // a failed STREAM_ESTABLISHED / STREAM_STARTED).
                let initiator = u8_at(p, 6).unwrap_or(0) != 0;
                let signal = u8_at(p, 7).unwrap_or(0);
                tracing::debug!(
                    "A2DP command rejected: cid 0x{:04X}, local SEID {:?}, waits (stream, start): {:?}",
                    le16(p, 3).unwrap_or(0),
                    u8_at(p, 5),
                    {
                        let s = self.shared.lock();
                        (s.stream_done, s.start_done)
                    }
                );
                tracing::warn!(
                    "A2DP command rejected (signal 0x{signal:02x}{})",
                    if initiator { "" } else { ", remote's" }
                );
                if initiator {
                    match signal {
                        AVDTP_SI_SET_CONFIGURATION => self.shared.update(|s| {
                            s.stream_done.get_or_insert(false);
                        }),
                        AVDTP_SI_START => self.shared.update(|s| {
                            s.start_done.get_or_insert(false);
                        }),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    // ======================================================================
    // AVRCP
    // ======================================================================

    /// Opens the HFP service level connection to the headphones, where they
    /// report their battery (the headphones may also open it themselves)
    fn connect_hfp(&mut self, stack: &Stack) {
        if !self.setup.battery || self.hfp_acl.is_some() {
            tracing::debug!("HFP: not connecting (battery {}, acl {:?})", self.setup.battery, self.hfp_acl);
            return;
        }
        let Some(addr) = self.remote_addr else { return };
        let status = stack.hfp_ag_connect(&addr);
        tracing::info!("HFP: opening the service level connection to {} (0x{status:02x})", addr_str(&addr));
    }

    fn handle_hfp(&mut self, p: &[u8]) {
        match subevent(p) {
            Some(HFP_SUBEVENT_SERVICE_LEVEL_CONNECTION_ESTABLISHED) => {
                let status = u8_at(p, 5).unwrap_or(0xFF);
                if status == 0 {
                    self.hfp_acl = le16(p, 3);
                    tracing::debug!("HFP: service level connection on ACL {:04X?}", self.hfp_acl);
                    tracing::info!("HFP: service level connection established");
                } else {
                    tracing::info!("HFP: service level connection failed (0x{status:02x})");
                    // RFCOMM DM: the headphones have HFP but refuse it to us,
                    // seen while a phone held it (multipoint)
                    if status == ERROR_CODE_CONNECTION_REJECTED_DUE_TO_LIMITED_RESOURCES {
                        tracing::info!("HFP: refused by the headphones (another device may hold HFP): battery unknown");
                        self.shared.update(|s| s.battery = None);
                        let _ = self.events.send(HostEvent::BatteryRefused);
                    }
                }
            }
            Some(HFP_SUBEVENT_SERVICE_LEVEL_CONNECTION_RELEASED) => {
                tracing::info!("HFP: service level connection released");
                self.hfp_acl = None;
            }
            // AT+BIEV: HF indicator 2, 0..100 percent
            Some(HFP_SUBEVENT_HF_INDICATOR) => {
                tracing::debug!("HFP: HF indicator {:04X?} = {:?}", le16(p, 5), u8_at(p, 7));
                if le16(p, 5) == Some(HFP_HF_INDICATOR_UUID_BATTERY_LEVEL)
                    && let Some(v) = u8_at(p, 7)
                {
                    tracing::info!("HFP: battery level {v}% (HF indicator)");
                    self.report_battery(v.min(100));
                }
            }
            // AT+IPHONEACCEV (Apple accessories): 0..9
            Some(HFP_SUBEVENT_APPLE_BATTERY_LEVEL) => {
                if let Some(v) = u8_at(p, 3) {
                    // 0 is 0-10%, 9 is 90-100% (as Android shows it)
                    let pct = (v.min(9) + 1) * 10;
                    tracing::info!("HFP: battery level {v} of 0-9 = {pct}% (Apple accessory)");
                    self.report_battery(pct);
                }
            }
            Some(sub) => tracing::debug!("HFP: subevent 0x{sub:02X}"),
            None => {}
        }
    }

    fn report_battery(&mut self, percent: u8) {
        self.shared.update(|s| s.battery = Some(percent));
        let _ = self.events.send(HostEvent::Battery(percent));
    }

    fn handle_avrcp(&mut self, stack: &Stack, p: &[u8]) {
        match subevent(p) {
            Some(AVRCP_SUBEVENT_CONNECTION_ESTABLISHED) => {
                let (Some(status), Some(cid)) = (u8_at(p, 3), le16(p, 4)) else { return };
                tracing::debug!(
                    "AVRCP connection event: cid 0x{cid:04X}, status 0x{status:02X} ({}), a2dp cid 0x{:04X}",
                    hci_error_string(status),
                    self.a2dp_cid
                );
                if status != ERROR_CODE_SUCCESS {
                    tracing::warn!("AVRCP connection failed (0x{status:02x})");
                    return;
                }
                if self.a2dp_cid == 0 {
                    // Requested at capability discovery, but A2DP was torn
                    // down before it came up: an orphan AVRCP link keeps the
                    // ACL open and makes the next avrcp_connect fail
                    tracing::info!("AVRCP connected (cid 0x{cid:04x}) after A2DP closed, disconnecting");
                    stack.avrcp_disconnect(cid);
                    return;
                }
                self.avrcp_cid = cid;
                tracing::info!("AVRCP connected (cid 0x{cid:04x})");
                self.volume_in_flight = None;
                // Remembered volume first, while AVRCP is idle, so the audio
                // does not start at the headphones' own (maybe loud) level
                let restore = self.controls.restore_volume.load(Ordering::Acquire);
                if restore >= 0 {
                    tracing::info!("AVRCP: restoring volume {restore} ({}%)", restore * 100 / 127);
                    self.controls.volume_request.store(restore, Ordering::Release);
                    self.send_pending_volume(stack);
                }
                let notify = stack.avrcp_enable_notification(cid, AVRCP_NOTIFICATION_EVENT_VOLUME_CHANGED);
                // Target side: buttons arrive as pass-through commands; the
                // playback status lets the headphones pick play or pause
                let support = stack.avrcp_target_support_event(cid, AVRCP_NOTIFICATION_EVENT_PLAYBACK_STATUS_CHANGED);
                tracing::debug!(
                    "AVRCP: volume notification 0x{notify:02X}, playback status event 0x{support:02X}, restore {restore}"
                );
                self.playback_status = AVRCP_PLAYBACK_STATUS_STOPPED;
                self.playback_prev_frames = self.stats.capture_frames.load(Ordering::Relaxed);
                self.playback_idle_ticks = 0;
                self.playback_hold_until = Instant::now();
                stack.timer_start(TIMER_PLAYBACK, PLAYBACK_TICK_MS);
            }
            Some(AVRCP_SUBEVENT_CONNECTION_RELEASED) => {
                tracing::info!("AVRCP disconnected (cid 0x{:04x})", le16(p, 3).unwrap_or(0));
                stack.timer_stop(TIMER_PLAYBACK);
                self.avrcp_cid = 0;
                // A reconnect restores the volume the headphones had last
                let c = &self.controls;
                let v = c.volume.load(Ordering::Acquire);
                if c.restore_volume.load(Ordering::Acquire) >= 0 && v >= 0 {
                    tracing::debug!("AVRCP: restore volume follows the last one, {v}");
                    c.restore_volume.store(v, Ordering::Release);
                }
                c.volume.store(-1, Ordering::Release);
                c.volume_request.store(-1, Ordering::Release);
            }
            Some(AVRCP_SUBEVENT_NOTIFICATION_VOLUME_CHANGED) => {
                let vol = u8_at(p, 6).unwrap_or(0);
                tracing::info!("AVRCP volume changed to {vol} ({}%)", vol as u32 * 100 / 127);
                self.controls.volume.store(vol as i32, Ordering::Release);
                let _ = self.events.send(HostEvent::Volume(vol));
                // AVRCP needs a new registration after each notification
                stack.avrcp_enable_notification(self.avrcp_cid, AVRCP_NOTIFICATION_EVENT_VOLUME_CHANGED);
                self.send_pending_volume(stack);
            }
            Some(AVRCP_SUBEVENT_SET_ABSOLUTE_VOLUME_RESPONSE) => {
                let vol = u8_at(p, 6).unwrap_or(0);
                tracing::info!("AVRCP absolute volume confirmed: {vol} ({}%)", vol as u32 * 100 / 127);
                self.volume_in_flight = None;
                self.controls.volume.store(vol as i32, Ordering::Release);
                // The UI follows what the headphones took, not only their own changes
                let _ = self.events.send(HostEvent::Volume(vol));
                self.send_pending_volume(stack);
            }
            Some(AVRCP_SUBEVENT_OPERATION) => {
                if le16(p, 3) != Some(self.avrcp_cid) {
                    tracing::debug!(
                        "AVRCP operation on cid {:04X?} ignored (current 0x{:04X})",
                        le16(p, 3),
                        self.avrcp_cid
                    );
                    return;
                }
                let (Some(op), Some(pressed)) = (u8_at(p, 5), u8_at(p, 6)) else { return };
                tracing::debug!("AVRCP operation 0x{op:02X} {}", if pressed != 0 { "pressed" } else { "released" });
                self.on_button(stack, op, pressed != 0);
            }
            Some(AVRCP_SUBEVENT_PLAY_STATUS_QUERY) => {
                // GetPlayStatus: no track length or position to report
                if let Some(cid) = le16(p, 3) {
                    tracing::debug!("AVRCP play status query (cid 0x{cid:04X}), status {}", self.playback_status);
                    stack.avrcp_target_play_status(cid, 0xFFFF_FFFF, 0xFFFF_FFFF, self.playback_status);
                }
            }
            _ => {}
        }
    }

    fn on_button(&mut self, stack: &Stack, op: u8, pressed: bool) {
        // A button sends a press and then a release; act on the press
        if !pressed {
            return;
        }
        let key = match op {
            // Windows has one Play/Pause key. Headphones send PLAY or PAUSE
            // by what they believe the state is, which may be stale: a
            // toggle always does what the tap meant.
            AVRCP_OPERATION_ID_PLAY | AVRCP_OPERATION_ID_PAUSE => MediaKey::PlayPause,
            AVRCP_OPERATION_ID_STOP => MediaKey::Stop,
            AVRCP_OPERATION_ID_FORWARD => MediaKey::Next,
            AVRCP_OPERATION_ID_BACKWARD => MediaKey::Previous,
            _ => {
                // FAST_FORWARD / REWIND have no Windows media key; VOLUME_UP /
                // DOWN come from headphones without absolute volume, which
                // set their own level
                tracing::info!("AVRCP button 0x{op:02x} ignored");
                return;
            }
        };
        if !self.controls.media_keys.load(Ordering::Acquire) {
            tracing::info!("AVRCP button {key:?} ignored (media keys off)");
            return;
        }
        tracing::info!("AVRCP button {key:?} -> media key");
        let _ = self.events.send(HostEvent::MediaKey(key));
        // Tell the headphones the state the button asked for right away; the
        // capture check takes over once the player had time to react
        let status = match op {
            AVRCP_OPERATION_ID_PLAY => AVRCP_PLAYBACK_STATUS_PLAYING,
            AVRCP_OPERATION_ID_PAUSE => AVRCP_PLAYBACK_STATUS_PAUSED,
            AVRCP_OPERATION_ID_STOP => AVRCP_PLAYBACK_STATUS_STOPPED,
            _ => return,
        };
        self.set_playback_status(stack, status, "button");
        self.playback_hold_until = Instant::now() + PLAYBACK_HOLD;
        self.playback_idle_ticks = 0;
        self.playback_prev_frames = self.stats.capture_frames.load(Ordering::Relaxed);
    }

    fn set_playback_status(&mut self, stack: &Stack, status: u8, why: &str) {
        if self.avrcp_cid == 0 || status == self.playback_status {
            return;
        }
        let name = |s: u8| match s {
            AVRCP_PLAYBACK_STATUS_STOPPED => "stopped",
            AVRCP_PLAYBACK_STATUS_PLAYING => "playing",
            AVRCP_PLAYBACK_STATUS_PAUSED => "paused",
            _ => "other",
        };
        tracing::info!("AVRCP playback status {} -> {} ({why})", name(self.playback_status), name(status));
        self.playback_status = status;
        stack.avrcp_target_set_playback_status(self.avrcp_cid, status);
    }

    fn playback_tick(&mut self, stack: &Stack) {
        // Loopback capture gets no audio while nothing plays: audio arriving
        // means playing, none for a while means paused
        let frames = self.stats.capture_frames.load(Ordering::Relaxed);
        let audio = frames != self.playback_prev_frames;
        self.playback_prev_frames = frames;
        self.playback_idle_ticks = if audio { 0 } else { self.playback_idle_ticks + 1 };
        if Instant::now() < self.playback_hold_until {
            return;
        }
        if audio {
            self.set_playback_status(stack, AVRCP_PLAYBACK_STATUS_PLAYING, "audio");
        } else if self.playback_idle_ticks >= PLAYBACK_IDLE_TICKS
            && self.playback_status == AVRCP_PLAYBACK_STATUS_PLAYING
        {
            self.set_playback_status(stack, AVRCP_PLAYBACK_STATUS_PAUSED, "no audio");
        }
    }

    // ======================================================================
    // AFH classification and radio readings
    // ======================================================================

    fn afh_tick(&mut self, stack: &Stack) {
        if !self.shared.lock().hci_ready {
            return;
        }
        let version = self.controls.afh_version.load(Ordering::Acquire);
        let classify =
            self.afh_policy_version_seen != Some(version) || self.afh_ticks.is_multiple_of(AFH_CLASSIFY_EVERY_TICKS);
        self.afh_ticks += 1;

        // 1. The classification, when it changes (one command per tick)
        if classify {
            let policy = self.controls.afh_policy();
            let c: Option<AfhClassification> = self.afh.as_mut().and_then(|a| a.classify(&policy));
            self.stats.with_radio(|r| {
                r.afh_usable = c.as_ref().map(|c| c.usable);
                r.afh_avoided = c.as_ref().map(|c| c.avoided.clone()).unwrap_or_default();
            });
            // Nothing to pass and nothing passed before: leave the controller alone
            let changed = match (&c, &self.afh_sent_map) {
                (Some(c), Some(sent)) => &c.map != sent,
                (Some(_), None) => true,
                (None, sent) => sent.is_some(),
            };
            tracing::debug!(
                "AFH classify: policy {policy:?} (version {version}), result {:?}, changed {changed}",
                c.as_ref().map(|c| (&c.avoided, c.usable, c.map))
            );
            if changed && !stack.can_send_command_now() {
                tracing::debug!("AFH: HCI busy, classification sent on a later tick");
            }
            if changed && stack.can_send_command_now() {
                // All channels "unknown" clears the host classification
                let map =
                    c.as_ref().map(|c| c.map).unwrap_or([0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F]);
                stack.cmd_set_afh_classification(&map);
                tracing::info!(
                    "AFH host classification: {}",
                    match &c {
                        Some(c) => format!("avoid Wi-Fi {} ({} of 79 channels usable)", c.avoided, c.usable),
                        None => "cleared".into(),
                    }
                );
                self.afh_sent_map = c.map(|c| c.map);
                self.afh_policy_version_seen = Some(version);
                return;
            }
            if !changed {
                self.afh_policy_version_seen = Some(version);
            }
        }

        // 2. Readings of the current link: RSSI and AFH map in turn
        let handle = self.shared.lock().a2dp_con_handle;
        let Some(handle) = handle else {
            self.stats.with_radio(|r| {
                r.rssi = None;
                r.afh_in_use = None;
            });
            return;
        };
        if !stack.can_send_command_now() {
            return;
        }
        self.read_rssi_next = !self.read_rssi_next;
        if !self.read_rssi_next {
            stack.cmd_read_rssi(handle);
        } else {
            stack.cmd_read_afh_channel_map(handle);
        }
    }

    fn on_radio_command_complete(&mut self, opcode: u16, p: &[u8]) {
        let rp = p.get(5..).unwrap_or(&[]);
        match opcode {
            HCI_OPCODE_SET_AFH_HOST_CHANNEL_CLASSIFICATION => {
                if rp.first().is_some_and(|&s| s != ERROR_CODE_SUCCESS) {
                    tracing::warn!("AFH host classification refused (0x{:02x})", rp[0]);
                }
            }
            HCI_OPCODE_READ_RSSI if rp.len() >= 4 => {
                let ok = rp[0] == ERROR_CODE_SUCCESS;
                tracing::trace!("radio: RSSI {} (status 0x{:02X})", rp[3] as i8, rp[0]);
                self.stats.with_radio(|r| r.rssi = ok.then_some(rp[3] as i8));
            }
            HCI_OPCODE_READ_AFH_CHANNEL_MAP if rp.len() >= 14 => {
                // status, handle, mode, map[10]
                let in_use = if rp[0] != ERROR_CODE_SUCCESS {
                    None
                } else if rp[3] == 1 {
                    Some(rp[4..14].iter().map(|b| b.count_ones()).sum::<u32>().min(79))
                } else {
                    Some(79)
                };
                tracing::debug!(
                    "radio: AFH mode {}, {in_use:?} channels in use, map {:02X?} (status 0x{:02X})",
                    rp[3],
                    &rp[4..14],
                    rp[0]
                );
                self.stats.with_radio(|r| r.afh_in_use = in_use);
            }
            _ => {}
        }
    }
}

impl App for HostApp {
    fn on_packet(&mut self, stack: &Stack, source: Source, packet_type: u8, channel: u16, packet: &[u8]) {
        // RFCOMM's data packets as well as its events
        if source == Source::Rfcomm {
            self.rfcomm.on_packet(stack, packet_type, channel, packet);
            return;
        }
        if packet_type != HCI_EVENT_PACKET {
            return;
        }
        match source {
            Source::Hci => {
                self.handle_hci(stack, packet);
                if let Some(le) = self.le.as_mut() {
                    le.on_hci(stack, packet);
                }
            }
            Source::A2dp => self.handle_a2dp(stack, packet),
            Source::AvdtpSink => self.handle_sep(packet),
            Source::Hfp => {
                if event_type(packet) == Some(HCI_EVENT_HFP_META) {
                    self.handle_hfp(packet)
                }
            }
            Source::SdpClient => self.rfcomm.on_sdp(stack, packet),
            Source::Rfcomm => {}
            Source::Avrcp | Source::AvrcpController | Source::AvrcpTarget => {
                if event_type(packet) == Some(HCI_EVENT_AVRCP_META) {
                    self.handle_avrcp(stack, packet)
                }
            }
            // LE (only after Stack::le_init)
            Source::Sm => {
                if let Some(le) = self.le.as_mut() {
                    le.on_sm(stack, packet);
                }
            }
            Source::GattClient => {
                if let Some(le) = self.le.as_mut() {
                    le.on_gatt(stack, packet);
                }
            }
            Source::AttServer => {
                if let Some(le) = self.le.as_mut() {
                    le.on_att_event(packet);
                }
            }
            Source::Iso => {
                if let Some(le) = self.le.as_mut() {
                    le.on_iso(stack, packet);
                }
            }
        }
    }

    fn on_wake(&mut self, stack: &Stack) {
        while let Ok(job) = self.jobs.try_recv() {
            job(self, stack);
        }
        if stack.transport_failed() {
            let first = self.shared.update(|s| !std::mem::replace(&mut s.stalled, true));
            if first {
                tracing::debug!("transport failed: stack marked stalled (hci {:?})", stack.hci_state());
                let _ = self.events.send(HostEvent::TransportFailed);
            }
        }
        self.kick_media(stack);
    }

    fn on_timer(&mut self, stack: &Stack, id: u32) {
        match id {
            TIMER_AFH => {
                self.afh_tick(stack);
                stack.timer_start(TIMER_AFH, AFH_TICK_MS);
            }
            TIMER_PLAYBACK => {
                self.playback_tick(stack);
                stack.timer_start(TIMER_PLAYBACK, PLAYBACK_TICK_MS);
            }
            TIMER_RTK_VERIFY => self.verify_realtek_download(stack),
            TIMER_RFCOMM => self.rfcomm.on_timer(stack),
            TIMER_LE => {
                if let Some(le) = self.le.as_mut() {
                    le.on_timer(stack);
                }
            }
            TIMER_ISO => {
                if let Some(le) = self.le.as_mut() {
                    le.on_iso_timer(stack);
                }
            }
            _ => {}
        }
    }

    fn on_intel_firmware(&mut self, stack: &Stack, result: i32) {
        tracing::debug!("intel firmware loader finished: {result}");
        if result != 0 {
            tracing::error!(
                "Intel firmware download failed ({result}). Place the matching ibt-*.sfi and ibt-*.ddc \
                 (linux-firmware intel/) in {}. Newer TLV-mode controllers (AX210 and later) are not supported by BTstack yet.",
                self.setup.firmware_dir.display()
            );
            self.init_failed(InitFailure::Failed);
            return;
        }
        tracing::info!("Intel controller operational");
        self.setup_hci_and_power_on(stack, None);
    }

    fn on_exit(&mut self, stack: &Stack) {
        tracing::debug!("run loop exit (a2dp cid 0x{:04X}, avrcp cid 0x{:04X})", self.a2dp_cid, self.avrcp_cid);
        self.set_streaming(false);
        self.rfcomm.stop();
        if let Some(le) = self.le.as_mut() {
            le.stack_deinit(stack);
        }
    }

    fn on_att_read(&mut self, stack: &Stack, con: u16, handle: u16, offset: u16) -> Result<Vec<u8>, u8> {
        match self.le.as_mut() {
            Some(le) => le.att_read(stack, con, handle, offset),
            None => Err(ATT_ERROR_READ_NOT_PERMITTED),
        }
    }

    fn on_att_write(&mut self, stack: &Stack, write: &btstack_sys::AttWriteRequest) -> u8 {
        match self.le.as_mut() {
            Some(le) => le.att_write(stack, write),
            None => ATT_ERROR_WRITE_NOT_PERMITTED,
        }
    }
}

/// Earlier versions copied (hard linked) "${stem}.bin" to "${stem}" for
/// BTstack's Realtek driver, which now opens the .bin itself: delete such a
/// copy. One that differs from the .bin, or has no .bin beside it, is the
/// user's own and stays. The sizes go to the log, for telling why the
/// firmware was not loaded.
fn remove_alias(dir: &Path, stem: &str) {
    if stem.is_empty() {
        return;
    }
    let alias = dir.join(stem);
    let source = dir.join(format!("{stem}.bin"));
    let alias_data = std::fs::read(&alias).ok();
    let Ok(source_data) = std::fs::read(&source) else {
        match alias_data {
            Some(a) => tracing::info!("{stem}.bin missing, using {} ({} bytes)", alias.display(), a.len()),
            None => tracing::warn!("firmware file missing: {}", source.display()),
        }
        return;
    };
    tracing::info!("{stem}.bin: {} bytes", source_data.len());
    if alias_data.is_some_and(|a| a == source_data) {
        match std::fs::remove_file(&alias) {
            Ok(()) => tracing::info!("deleted {}, a copy of {stem}.bin", alias.display()),
            Err(e) => tracing::warn!("could not delete {}: {e}", alias.display()),
        }
    }
}

/// A Broadcom .hcd from the firmware folder: linux-firmware brcm/ names them
/// <chip>-<vid>-<pid>.hcd; prefer the one for a present adapter, take a lone
/// .hcd as meant for it
fn find_bcm_hcd(dir: &Path, adapters: &[(u16, u16)]) -> Option<PathBuf> {
    let files: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x.eq_ignore_ascii_case("hcd")))
        .collect();
    tracing::debug!("broadcom: .hcd files {files:?}");
    for (vid, pid) in adapters {
        let tag = format!("-{vid:04x}-{pid:04x}.");
        if let Some(f) =
            files.iter().find(|f| f.file_name().is_some_and(|n| n.to_string_lossy().to_lowercase().contains(&tag)))
        {
            return Some(f.clone());
        }
    }
    if files.len() == 1 {
        return Some(files[0].clone());
    }
    if !files.is_empty() {
        tracing::info!("{} .hcd files but none named for the adapter's VID:PID - skipping PatchRAM", files.len());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{ConnectFailure, remove_alias, signaling_failure};

    #[test]
    fn a_device_that_never_answered_is_not_told_to_pair() {
        // The ACL page ran out, and an SDP query cut off by a lost link
        assert_eq!(signaling_failure(0x04), ConnectFailure::NoAnswer);
        assert_eq!(signaling_failure(0x10), ConnectFailure::NoAnswer);
        assert_eq!(signaling_failure(0x81), ConnectFailure::LinkLost);
        assert_eq!(signaling_failure(0x05), ConnectFailure::AuthFailed);
        assert_eq!(signaling_failure(0x66), ConnectFailure::AuthFailed);
    }

    #[test]
    fn firmware_alias_is_removed() {
        let dir = std::env::temp_dir().join(format!("a2dpwb_alias_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (alias, bin) = (dir.join("rtl8761bu_fw"), dir.join("rtl8761bu_fw.bin"));

        remove_alias(&dir, "rtl8761bu_fw"); // neither there: nothing to do
        std::fs::write(&bin, b"firmware").unwrap();
        remove_alias(&dir, "rtl8761bu_fw");
        assert!(!alias.exists(), "no alias is made");

        // A copy the earlier versions made goes
        std::fs::hard_link(&bin, &alias).unwrap();
        remove_alias(&dir, "rtl8761bu_fw");
        assert!(!alias.exists());
        assert_eq!(std::fs::read(&bin).unwrap(), b"firmware");

        // A different file, or one without a .bin, stays
        std::fs::write(&alias, b"other").unwrap();
        remove_alias(&dir, "rtl8761bu_fw");
        assert!(alias.exists());
        std::fs::remove_file(&bin).unwrap();
        remove_alias(&dir, "rtl8761bu_fw");
        assert!(alias.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
