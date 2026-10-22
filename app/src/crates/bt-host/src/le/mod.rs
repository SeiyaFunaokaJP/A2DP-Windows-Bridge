//! LE Audio (experimental): BAP unicast client (audio source to sink ASEs)
//! on the same BTstack session as A2DP, ported from v1's app/src/le_audio
//! (branch feat/le-audio, commit b819893; docs/le-audio-dev.md).
//!
//! Off unless [`crate::HostConfig::le_audio`] is Some: then none of this
//! runs and HCI init is that of a build without LE.
//!
//! Threads (v1's one rule): everything that touches BTstack, an
//! [`core::LeDevice`], the GATT client queue or the GATT server runs on the
//! BTstack thread, inside the LE core ([`core::LeCtx`]). The worker API is
//! the second `impl BtHost` in [`api`] (blocking calls with timeouts, for
//! core's LE session and the CLI). Module hooks run on the BTstack thread
//! ([`module::LeModule`]) except the worker / any-thread ones of
//! [`module::LeGroupHooks`].
//!
//! Files:
//! - [`api`]: the worker API on [`crate::BtHost`]
//! - [`module`]: the profile module traits
//! - [`core`]: the LE core on the BTstack thread: LE scan, connect, SM,
//!   RPA / identity merge, GATT cache, PACS / ASCS, stream start / stop,
//!   module dispatch (v1 le_audio_core.cpp)
//! - [`gatt_client`]: the per-connection GATT client queue
//! - [`gatt_server`]: the PC's GATT server (GAP, GATT with Service Changed
//!   and Database Hash, the modules' services)
//! - [`bap`]: PACS / ASCS client and the BAP unicast state machine
//! - [`iso_send`]: ISO SDU sending on HCI_EVENT_CIS_CAN_SEND_NOW
//! - [`modules`]: CSIP, VCP, MCP, CCP, TMAP / GMAP (registered in that
//!   order)

pub mod api;
pub mod bap;
pub mod core;
pub mod gatt_client;
pub mod gatt_server;
pub mod iso_send;
pub mod module;
pub mod modules;

use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use le_audio::mcp_model::MediaBackend;
use le_audio::plan::StreamPlan;
use le_audio::sdu_queue::{SduQueue, SduSet};

/// An LE address in BTstack order (as printed, most significant octet
/// first, like [`crate::parse_addr`]) plus its BTstack address type
/// (BD_ADDR_TYPE_LE_PUBLIC 0, BD_ADDR_TYPE_LE_RANDOM 1)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct LeAddr {
    pub addr: [u8; 6],
    pub addr_type: u8,
}

impl LeAddr {
    pub const PUBLIC: u8 = 0;
    pub const RANDOM: u8 = 1;

    /// A resolvable private address (random, top bits 0b01): it changes
    /// every few minutes; the identity behind it is found with the IRK
    pub fn is_rpa(&self) -> bool {
        self.addr_type == LeAddr::RANDOM && self.addr[0] & 0xC0 == 0x40
    }
}

impl fmt::Display for LeAddr {
    /// "XX:XX:XX:XX:XX:XX" (the type is not shown, as in v1's log lines)
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&crate::addr_str(&self.addr))
    }
}

/// A device of the LE core: stable while the BtHost runs (from 0 at each
/// start, v1's `int` id). A device using resolvable private addresses keeps
/// its id across RPAs once its identity is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LeDeviceId(pub u32);

impl fmt::Display for LeDeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Turns LE Audio on for a BtHost ([`crate::HostConfig::le_audio`]): SM (LE
/// Secure Connections, Just Works, bonding), the GATT client, the PC's GATT
/// server and the profile modules are set up with the stack. Nothing is
/// advertised.
#[derive(Default)]
pub struct LeConfig {
    /// Folder of le_audio_bonds.tlv (LE bonds, GATT server CCC values) and
    /// le_audio_csip.txt (CSIP sets); None: in memory only
    pub bond_dir: Option<PathBuf>,
    /// MCP: the Windows side of the media control server (core, win-sys).
    /// None: dry run (`--mcp-dry-run`, implied by `--hci-tcp`): a simulated
    /// player, commands only logged
    pub media: Option<Box<dyn MediaBackend>>,
    /// CCP: `--ccp-sim "<t>:<cmd>;..."` script
    pub ccp_sim: Option<String>,
    /// CCP: `--ccp-console`, simulation commands from stdin (CLI)
    pub ccp_console: bool,
}

/// What core's session wants of an LE Audio stream (core's
/// `StreamRequest::le`; `StreamRequest::device` is the first device)
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LeRequest {
    /// More devices of the group (`--le-device`, repeatable), addresses as
    /// [`crate::parse_addr`] returns them
    pub extra_devices: Vec<[u8; 6]>,
    /// Try only this preset (`--le-preset`, e.g. "48_2_2"); None: the
    /// default order
    pub preset: Option<String>,
    /// GMAP low-latency presets for a gaming headset (setting
    /// le_audio_low_latency, `--le-low-latency`)
    pub low_latency: bool,
}

/// One advertiser seen by [`crate::BtHost::le_scan`] /
/// [`crate::BtHost::le_find`] (v1 ScanResult)
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LeScanResult {
    /// As advertised (connect with this)
    pub addr: LeAddr,
    /// `addr` is an RPA that SM resolved with the IRK of a bond: the
    /// bonded device's identity address
    pub identity: Option<LeAddr>,
    pub name: String,
    pub rssi: i8,
    pub appearance: u16,
    /// ASCS / PACS / CAS advertised (or a module said so)
    pub le_audio: bool,
    pub connectable: bool,
    /// Service UUIDs and service data UUIDs
    pub uuids16: Vec<u16>,
    /// ASCS service data: available sink contexts
    pub ascs_sink_contexts: u16,
    /// ASCS / CAS targeted announcement
    pub targeted: bool,
    /// CSIP Resolvable Set Identifier
    pub rsi: Option<[u8; 6]>,
    /// CSIP module: the coordinated set the advertiser belongs to
    pub set_id: Option<u32>,
    /// Module annotation for the UI / log
    pub note: String,
    /// Last raw advertising data
    pub adv: Vec<u8>,
    /// Last raw scan response data
    pub scan_rsp: Vec<u8>,
}

/// What a stream tries (v1 le_audio::StreamRequest); modules may change it
/// in [`module::LeGroupHooks::adjust_stream_request`]
pub type LeStreamRequest = le_audio::plan::StreamRequest;

/// The stream for the UI / log (v1 StreamStatus)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeStreamStatus {
    pub active: bool,
    pub plan: StreamPlan,
    pub devices: Vec<LeDeviceId>,
    /// `plan.describe()`
    pub summary: String,
}

/// Counters of the running stream (v1 Stream::Stats)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LeStreamStats {
    /// SDU sets from captured audio
    pub sets_encoded: u64,
    /// SDU sets sent (audio + silence)
    pub sets_sent: u64,
    pub sdus_sent: u64,
    /// Sets sent while the buffer was empty
    pub silence_sets: u64,
    /// Times the buffer ran empty
    pub underruns: u64,
    /// Sets dropped, buffer full
    pub overflows: u64,
    pub send_errors: u64,
    /// Sets the buffer fills before sending audio (queue + 3, growing to
    /// queue + 7 after underruns)
    pub prefill: u32,
}

/// A device as the worker sees it (a copy; v1 DeviceInfo)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeDeviceInfo {
    pub id: LeDeviceId,
    /// The identity once known, else the address it was connected with
    pub addr: LeAddr,
    pub name: String,
    pub connected: bool,
    pub ready: bool,
    /// HCI reason the link last went down (0 since a connect)
    pub last_error: u8,
    pub sink_locations: u32,
    pub sink_ases: usize,
    /// Short summary of the sink PAC records
    pub pacs: String,
    /// TMAP / GMAP roles as read ("TMAP: UMR, GMAP: UGT (...)"); empty
    /// while unknown or without TMAS / GMAS
    pub roles: String,
}

/// The encode thread's end of the stream's SDU jitter buffer
/// ([`le_audio::sdu_queue`]); the BTstack thread pops from the same queue
#[derive(Clone)]
pub struct LeSduSink {
    queue: Arc<Mutex<SduQueue>>,
}

impl LeSduSink {
    pub(crate) fn new(queue: Arc<Mutex<SduQueue>>) -> LeSduSink {
        LeSduSink { queue }
    }

    /// One SDU per CIS for the next frame interval, in plan order
    pub fn push(&self, set: SduSet) {
        let mut q = self.queue.lock().unwrap_or_else(|e| e.into_inner());
        q.push(set);
        tracing::trace!("LE Audio: SDU set queued, {} buffered", q.len());
    }

    /// A set to encode the next frame into (one the sender has sent and
    /// given back), so that no round allocates; None: allocate one
    pub fn spare(&self) -> Option<SduSet> {
        self.queue.lock().unwrap_or_else(|e| e.into_inner()).spare()
    }
}

/// What [`crate::BtHost::le_start_stream`] returns once audio may flow
pub struct LeStreamStarted {
    pub status: LeStreamStatus,
    /// Push the encoded SDU sets here (pipeline's LE encode thread)
    pub sink: LeSduSink,
}
