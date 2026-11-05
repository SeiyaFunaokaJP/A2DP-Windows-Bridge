//! The profile module hook interface (v1 le_audio_module.h).
//!
//! Each LE Audio profile beyond BAP lives in its own module file under
//! `le/modules/` and is registered in `modules::create_all` in v1's order:
//! CSIP, VCP, MCP, CCP, TMAP / GMAP. A module has
//! - a BTstack-thread part, [`LeModule`]: owned by the LE core, called with
//!   an [`LeCtx`] (devices, GATT client queue, GATT server, presets,
//!   timers, other modules); it must not block;
//! - optionally a shared part, [`LeGroupHooks`] (`Send + Sync`), for the
//!   worker hooks (may block, with timeouts; reach the BTstack thread with
//!   `BtHost::le_run` / `le_call`, or post with the module's [`LePoster`])
//!   and the any-thread volume hooks. The two parts share state through an
//!   `Arc` the module creates.
//!
//! All hooks have empty defaults; override only what the profile needs.
//! Modules live as long as their BtHost; `on_stack_init` /
//! `on_stack_deinit` mark the BTstack session. Keep per-device state keyed
//! by [`LeDeviceId`] and drop it in `on_disconnected` (device ids start
//! from 0 again with the next BtHost).
//!
//! Callbacks of GATT client operations and timers are closures that get
//! the [`LeCtx`]; they reach their module with [`LeCtx::module_mut`].

use std::any::Any;
use std::path::PathBuf;
use std::sync::Arc;

use le_audio::mcp_model::MediaBackend;

use super::core::{LeCtx, LePoster};
use super::gatt_server::{AttRead, AttWrite, GattServerBuilder};
use super::{LeDeviceId, LeScanResult, LeStreamRequest, LeStreamStatus};
use crate::BtHost;

/// ATT error "Read Not Permitted": the default answer of
/// [`LeModule::on_att_read`]
pub const ATT_ERROR_READ_NOT_PERMITTED: u8 = 0x02;
/// ATT error "Write Not Permitted": the default answer of
/// [`LeModule::on_att_write`]
pub const ATT_ERROR_WRITE_NOT_PERMITTED: u8 = 0x03;

/// The BTstack-thread part of a profile module (v1 le_audio::Module, all
/// hooks but the worker / any-thread ones)
pub trait LeModule: Any {
    /// Log name ("CSIP", "VCP", ...)
    fn name(&self) -> &'static str;

    /// A BTstack session with LE Audio starts: SM, GATT client and the core
    /// are up, HCI is not powered on yet.
    fn on_stack_init(&mut self, _ctx: &mut LeCtx<'_>) {}

    /// Add the PC's GATT services of this profile; runs after
    /// `on_stack_init`, before att_server_init(). Services cannot be added
    /// later. Reads and writes of the characteristics added here come to
    /// [`LeModule::on_att_read`] / [`LeModule::on_att_write`] of this module.
    fn add_gatt_services(&mut self, _server: &mut GattServerBuilder<'_>) {}

    /// The session ends (the run loop has stopped: no BTstack calls). Drop
    /// all per-session state.
    fn on_stack_deinit(&mut self) {}

    /// Every advertising report during a scan / find, after the core merged
    /// it into `result` (`ad`: the raw AD of this report). May set
    /// `result.le_audio`, `result.set_id` and `result.note` (CSIP: RSI of a
    /// known set).
    fn on_scan_result(&mut self, _ctx: &mut LeCtx<'_>, _result: &mut LeScanResult, _ad: &[u8]) {}

    /// LE link up (not encrypted yet: no GATT before `on_encrypted`)
    fn on_connected(&mut self, _ctx: &mut LeCtx<'_>, _dev: LeDeviceId) {}

    /// Link encrypted (paired / bonded)
    fn on_encrypted(&mut self, _ctx: &mut LeCtx<'_>, _dev: LeDeviceId) {}

    /// GATT cache filled and PACS / ASCS read by the core: find this
    /// profile's service with `LeDevice::find_service` / `find_chars` and
    /// queue reads, writes and subscriptions with `ctx.gatt()`.
    /// `BtHost::le_connect` returns right after this hook; queued operations
    /// keep running in the background.
    fn on_gatt_ready(&mut self, _ctx: &mut LeCtx<'_>, _dev: LeDeviceId) {}

    /// Link down (`reason`: HCI error code). The device still has its last
    /// data.
    fn on_disconnected(&mut self, _ctx: &mut LeCtx<'_>, _dev: LeDeviceId, _reason: u8) {}

    /// Unicast stream state: `streaming` true once all ASEs stream (audio
    /// flows), false when it stopped or was lost
    fn on_stream_state(&mut self, _ctx: &mut LeCtx<'_>, _status: &LeStreamStatus, _streaming: bool) {}

    /// Enable of a stream with these Streaming Audio Contexts (CONTEXT_*
    /// bits): append the CCIDs of this profile's content control services
    /// that belong to such a stream (MCP: GMCS for media, CCP: GTBS for
    /// calls). The core sends them, without duplicates, as the CCID List
    /// metadata of every ASE next to the Streaming Audio Contexts.
    fn add_ccids(&self, _contexts: u16, _ccids: &mut Vec<u8>) {}

    /// A client reads a characteristic this module added: the whole value
    /// (the GATT server serves `read.offset` from it), or an ATT error.
    /// `read.offset > 0` is the rest of a long read (Read Blob): a profile
    /// whose value changed since offset 0 answers its "Value Changed During
    /// Read Long" error (0x80).
    fn on_att_read(&mut self, _ctx: &mut LeCtx<'_>, _read: &AttRead) -> Result<Vec<u8>, u8> {
        Err(ATT_ERROR_READ_NOT_PERMITTED)
    }

    /// A client writes the whole value of a characteristic this module added
    /// (prepared / long writes are assembled by the GATT server); Ok or an
    /// ATT error
    fn on_att_write(&mut self, _ctx: &mut LeCtx<'_>, _write: &AttWrite<'_>) -> Result<(), u8> {
        Err(ATT_ERROR_WRITE_NOT_PERMITTED)
    }
}

/// The worker / any-thread part of a profile module (v1 Module's
/// prepare_group, adjust_stream_request, remote_volume, request_volume)
pub trait LeGroupHooks: Send + Sync {
    /// Log name, as [`LeModule::name`]
    fn name(&self) -> &'static str;

    /// Worker: before a stream is set up to `devices` (connected and ready).
    /// CSIP: find and connect the other members of a coordinated set and
    /// append their ids. `cancelled()` turns true when the user stops.
    fn prepare_group(&self, _host: &BtHost, _devices: &mut Vec<LeDeviceId>, _cancelled: &dyn Fn() -> bool) {}

    /// Worker: before the configuration is chosen. GMAP: low-latency
    /// presets first and the Game context for a gaming headset
    /// (`request.preset_order`, `request.contexts`; `host.le_low_latency()`).
    fn adjust_stream_request(&self, _host: &BtHost, _devices: &[LeDeviceId], _request: &mut LeStreamRequest) {}

    /// Any thread: the headset volume on the AVRCP scale 0-127 (VolumeSync
    /// keeps the Windows volume equal to it), None while unknown. The first
    /// module answering Some is used. VCP: the group volume.
    fn remote_volume(&self) -> Option<u8> {
        None
    }

    /// Any thread: set the headset volume (0-127); true if this module
    /// handles it (post the GATT work with the module's [`LePoster`])
    fn request_volume(&self, _level: u8) -> bool {
        false
    }
}

/// What a module gets when it is created (from [`super::LeConfig`])
pub struct LeModuleSetup {
    /// Folder of le_audio_bonds.tlv and le_audio_csip.txt; None: in memory
    pub bond_dir: Option<PathBuf>,
    /// MCP takes it; None: the simulated player (dry run)
    pub media: Option<Box<dyn MediaBackend>>,
    /// CCP `--ccp-sim` script
    pub ccp_sim: Option<String>,
    /// CCP `--ccp-console`
    pub ccp_console: bool,
    /// Runs closures on the BTstack thread from any thread (v1
    /// Core::post), e.g. for request_volume or a backend thread
    pub poster: LePoster,
    /// The host's events (BAS reports the battery level)
    pub events: std::sync::mpsc::Sender<crate::HostEvent>,
    /// The battery level setting (HostConfig::battery)
    pub battery: bool,
}

/// One registered module
pub struct LeModuleEntry {
    pub module: Box<dyn LeModule>,
    pub hooks: Option<Arc<dyn LeGroupHooks>>,
}
