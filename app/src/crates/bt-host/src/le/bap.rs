//! BAP unicast client (v1 le_audio_bap.{h,cpp}).
//!
//! [`read_device`]: PACS client (Sink PAC records, Sink Audio Locations,
//! Available / Supported Audio Contexts) and ASCS client (Sink / Source
//! ASEs, ASE Control Point), notifications enabled, once per connection.
//!
//! [`Unicast`]: the BAP unicast procedure for a StreamPlan over one or more
//! devices, a state machine on the BTstack thread:
//!
//! ```text
//! Config Codec (all planned ASEs)   -> Codec Configured
//! LE Set CIG Parameters (one CIG for the group; framed when an ASE's
//!   Preferred_Framing says unframed is not supported)
//! Config QoS                        -> QoS Configured
//! Enable (Streaming Audio Contexts + CCID List from the modules; waits
//!   for an Available Audio Contexts notification when none is available:
//!   "no audio context available for Enable")
//! LE Create CIS (all at once) + ISO data paths
//! Sink ASEs: the server's Receiver Start Ready -> Streaming
//! ISO SDUs (iso_send)
//! ```
//!
//! and back: Disable -> QoS Configured, HCI Disconnect of the CIS
//! ("disconnecting CIS"; servers may keep an ASE in Releasing until the CIS
//! is gone), Release -> Idle, LE Remove CIG. Each step has a timeout; a
//! failing step tears down what was set up and reports why.
//!
//! The state machine reaches BTstack, the devices and the core through
//! [`UnicastEnv`] (the LE core; a fake in the tests).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use btstack_sys::consts::{ERROR_CODE_ACL_CONNECTION_ALREADY_EXISTS, ERROR_CODE_MEMORY_CAPACITY_EXCEEDED};
use btstack_sys::{CigParams, CisParams, events};
use le_audio::defs::*;
use le_audio::pacs_ascs::{
    Ase, ConfigCodec, ConfigQos, cp_ase_ids, cp_config_codec, cp_config_qos, cp_with_metadata, enable_metadata,
    parse_ase_value, parse_pac_records,
};
use le_audio::plan::{CisPlan, StreamPlan, choose_streaming_contexts, codec_config_ltvs};

use super::LeDeviceId;
use super::core::{CON_HANDLE_INVALID, LeCtx, LeDevice};
use super::gatt_client::{GattChar, PROPERTY_INDICATE, PROPERTY_NOTIFY};
use super::iso_send::queue_depth;

/// Completion of [`read_device`]: true when done (also without PACS /
/// ASCS), false when the link went down or a read failed
pub type ReadDeviceDone = Box<dyn FnOnce(&mut LeCtx<'_>, bool)>;

/// What read_device does with a value (read or notified)
type OnValue = Rc<dyn Fn(&mut LeCtx<'_>, &[u8])>;

/// Read PACS and ASCS of a device and subscribe to their notifications.
/// `done(true)` also when the device has neither (then has_pacs / has_ascs
/// stay false).
pub fn read_device(ctx: &mut LeCtx<'_>, dev: LeDeviceId, done: ReadDeviceDone) {
    let Some(d) = ctx.device_mut(dev) else { return };
    let con = d.acl;
    d.sink_pacs.clear();
    d.ases.clear();
    d.ase_cp_handle = 0;
    d.has_pacs = d.find_service(UUID_SERVICE_PACS).is_some();
    d.has_ascs = d.find_service(UUID_SERVICE_ASCS).is_some();
    let sink_pacs: Vec<GattChar> = d.find_chars(UUID_SERVICE_PACS, UUID_CHR_SINK_PAC).into_iter().cloned().collect();
    let locations = d.find_char(UUID_SERVICE_PACS, UUID_CHR_SINK_AUDIO_LOCATIONS).cloned();
    let available = d.find_char(UUID_SERVICE_PACS, UUID_CHR_AVAILABLE_AUDIO_CONTEXTS).cloned();
    let supported = d.find_char(UUID_SERVICE_PACS, UUID_CHR_SUPPORTED_AUDIO_CONTEXTS).cloned();
    let sink_ases: Vec<GattChar> = d.find_chars(UUID_SERVICE_ASCS, UUID_CHR_SINK_ASE).into_iter().cloned().collect();
    let source_ases: Vec<GattChar> =
        d.find_chars(UUID_SERVICE_ASCS, UUID_CHR_SOURCE_ASE).into_iter().cloned().collect();
    let cp = d.find_char(UUID_SERVICE_ASCS, UUID_CHR_ASE_CONTROL_POINT).cloned();
    tracing::debug!(
        "LE Audio: {}: reading PACS / ASCS: sink PAC {:04X?}, locations {:04X?}, available {:04X?}, supported {:04X?}, \
         sink ASE {:04X?}, source ASE {:04X?}, control point {:04X?}",
        d.addr,
        sink_pacs.iter().map(|c| c.value_handle).collect::<Vec<_>>(),
        locations.as_ref().map(|c| c.value_handle),
        available.as_ref().map(|c| c.value_handle),
        supported.as_ref().map(|c| c.value_handle),
        sink_ases.iter().map(|c| c.value_handle).collect::<Vec<_>>(),
        source_ases.iter().map(|c| c.value_handle).collect::<Vec<_>>(),
        cp.as_ref().map(|c| c.value_handle)
    );
    ctx.core.sink_pacs_by_char.retain(|(c, _), _| *c != con);

    // Everything queued below counts; done() once all completed
    let counter = Rc::new(RefCell::new(Counter { pending: 0, done: Some(done) }));
    let begin = |counter: &Rc<RefCell<Counter>>| counter.borrow_mut().pending += 1;
    let subscribe = |ctx: &mut LeCtx<'_>, chr: &GattChar| {
        if chr.properties & (PROPERTY_NOTIFY | PROPERTY_INDICATE) == 0 {
            return;
        }
        begin(&counter);
        let ind = chr.properties & PROPERTY_NOTIFY == 0;
        let h = chr.value_handle;
        let c = counter.clone();
        ctx.gatt().subscribe(
            con,
            chr,
            ind,
            Some(Box::new(move |ctx, status| {
                if status != 0 {
                    tracing::warn!("LE Audio: enabling notifications of 0x{h:04x} failed (0x{status:02x})");
                } else {
                    tracing::debug!(
                        "LE Audio: {} enabled on 0x{h:04X}",
                        if ind { "indications" } else { "notifications" }
                    );
                }
                Counter::end(&c, ctx);
            })),
        );
    };
    // A read of `h` into on_value, and its notifications
    let read_and_listen = |ctx: &mut LeCtx<'_>, h: u16, on_value: OnValue, listen: bool| {
        begin(&counter);
        let c = counter.clone();
        let f = on_value.clone();
        ctx.gatt().read(
            con,
            h,
            Box::new(move |ctx, status, v| {
                if status == 0 {
                    f(ctx, v);
                } else {
                    tracing::debug!("LE Audio: read of 0x{h:04X} failed (ATT 0x{status:02X})");
                }
                Counter::end(&c, ctx);
            }),
        );
        if listen {
            ctx.gatt().listen(con, h, Box::new(move |ctx, v| on_value(ctx, v)));
        }
    };

    // ---- PACS ----
    for chr in &sink_pacs {
        let h = chr.value_handle;
        let on_value = Rc::new(move |ctx: &mut LeCtx<'_>, v: &[u8]| {
            let Some(d) = ctx.device_on(dev, con) else { return };
            tracing::debug!("LE Audio: {}: Sink PAC 0x{h:04X}: {v:02X?}", d.addr);
            let Some(recs) = parse_pac_records(v) else {
                tracing::warn!("LE Audio: {}: malformed Sink PAC ({} bytes)", d.addr, v.len());
                return;
            };
            ctx.core.sink_pacs_by_char.insert((con, h), recs);
            let all: Vec<_> = ctx
                .core
                .sink_pacs_by_char
                .iter()
                .filter(|((c, _), _)| *c == con)
                .flat_map(|(_, r)| r.iter().cloned())
                .collect();
            if let Some(d) = ctx.device_mut(dev) {
                d.sink_pacs = all;
            }
        });
        read_and_listen(ctx, h, on_value, true);
        subscribe(ctx, chr);
    }
    if let Some(chr) = &locations {
        let on_value = Rc::new(move |ctx: &mut LeCtx<'_>, v: &[u8]| {
            if let Some(d) = ctx.device_on_mut(dev, con)
                && v.len() >= 4
            {
                d.sink_locations = u32::from_le_bytes([v[0], v[1], v[2], v[3]]);
                tracing::debug!("LE Audio: {}: sink audio locations 0x{:08X}", d.addr, d.sink_locations);
            }
        });
        read_and_listen(ctx, chr.value_handle, on_value, true);
        subscribe(ctx, chr);
    }
    if let Some(chr) = &available {
        let on_value = Rc::new(move |ctx: &mut LeCtx<'_>, v: &[u8]| {
            if let Some(d) = ctx.device_on_mut(dev, con)
                && v.len() >= 4
            {
                d.available_sink_contexts = u16::from_le_bytes([v[0], v[1]]);
                d.available_source_contexts = u16::from_le_bytes([v[2], v[3]]);
                tracing::debug!(
                    "LE Audio: {}: available contexts sink 0x{:04X} source 0x{:04X}",
                    d.addr,
                    d.available_sink_contexts,
                    d.available_source_contexts
                );
                ctx.with_unicast(|u, env| u.on_available_contexts(env, dev));
            }
        });
        read_and_listen(ctx, chr.value_handle, on_value, true);
        subscribe(ctx, chr);
    }
    if let Some(chr) = &supported {
        let on_value = Rc::new(move |ctx: &mut LeCtx<'_>, v: &[u8]| {
            if let Some(d) = ctx.device_on_mut(dev, con)
                && v.len() >= 4
            {
                d.supported_sink_contexts = u16::from_le_bytes([v[0], v[1]]);
                d.supported_source_contexts = u16::from_le_bytes([v[2], v[3]]);
                tracing::debug!(
                    "LE Audio: {}: supported contexts sink 0x{:04X} source 0x{:04X}",
                    d.addr,
                    d.supported_sink_contexts,
                    d.supported_source_contexts
                );
            }
        });
        read_and_listen(ctx, chr.value_handle, on_value, false);
    }

    // ---- ASCS ----
    for (chars, sink) in [(&sink_ases, true), (&source_ases, false)] {
        for chr in chars {
            let h = chr.value_handle;
            // The ASE ID is only known from the value: keep the entry by
            // handle until the read fills it in
            if let Some(d) = ctx.device_mut(dev) {
                d.ases.push(Ase { sink, value_handle: h, ..Ase::default() });
            }
            let on_value = Rc::new(move |ctx: &mut LeCtx<'_>, v: &[u8]| on_ase_value(ctx, dev, con, h, v));
            read_and_listen(ctx, h, on_value, true);
            if sink {
                subscribe(ctx, chr);
            }
        }
    }
    if let Some(chr) = &cp {
        if let Some(d) = ctx.device_mut(dev) {
            d.ase_cp_handle = chr.value_handle;
        }
        ctx.gatt().listen(
            con,
            chr.value_handle,
            Box::new(move |ctx, v| {
                if ctx.device_on(dev, con).is_some() {
                    ctx.with_unicast(|u, env| u.on_cp_response(env, dev, v));
                }
            }),
        );
        subscribe(ctx, chr);
    }

    // Summary once everything queued above completed
    begin(&counter);
    let c = counter.clone();
    ctx.core.gatt.barrier(
        con,
        Box::new(move |ctx, _| {
            if let Some(d) = ctx.device_on(dev, con) {
                tracing::info!(
                    "LE Audio: {}: PACS {}, sink locations 0x{:08x}, contexts available 0x{:04x} supported 0x{:04x}; \
                     sink PAC: {}",
                    d.addr,
                    if d.has_pacs { "yes" } else { "no" },
                    d.sink_locations,
                    d.available_sink_contexts,
                    d.supported_sink_contexts,
                    le_audio::pacs_ascs::describe_pac_records(&d.sink_pacs)
                );
                tracing::info!(
                    "LE Audio: {}: ASCS {}, {} sink ASE(s), {} ASE(s) in all, control point 0x{:04x}",
                    d.addr,
                    if d.has_ascs { "yes" } else { "no" },
                    d.sink_ase_count(),
                    d.ases.len(),
                    d.ase_cp_handle
                );
            }
            Counter::end(&c, ctx);
        }),
    );
}

/// An ASE value (read or notified): the device's ASE, then the state
/// machine
pub(crate) fn on_ase_value(ctx: &mut LeCtx<'_>, dev: LeDeviceId, con: u16, h: u16, v: &[u8]) {
    let Some(d) = ctx.device_on_mut(dev, con) else { return };
    let addr = d.addr;
    let Some(ase) = d.ases.iter_mut().find(|a| a.value_handle == h) else { return };
    let mut parsed = ase.clone();
    if !parse_ase_value(v, &mut parsed) {
        tracing::warn!("LE Audio: {addr}: malformed ASE value ({} bytes)", v.len());
        return;
    }
    let old = ase.state;
    tracing::debug!("LE Audio: {addr}: ASE value 0x{h:04X} {v:02X?} -> {parsed:?}");
    *ase = parsed;
    let (id, state, sink) = (ase.id, ase.state, ase.sink);
    if old != state {
        tracing::info!("LE Audio: {addr} ASE {id} ({}): {}", if sink { "sink" } else { "source" }, state.name());
    }
    ctx.with_unicast(|u, env| u.on_ase_update(env, dev, id));
}

/// Operations of read_device still to complete
struct Counter {
    pending: usize,
    done: Option<ReadDeviceDone>,
}

impl Counter {
    fn end(c: &Rc<RefCell<Counter>>, ctx: &mut LeCtx<'_>) {
        let done = {
            let mut c = c.borrow_mut();
            c.pending = c.pending.saturating_sub(1);
            if c.pending == 0 { c.done.take() } else { None }
        };
        if let Some(d) = done {
            d(ctx, true);
        }
    }
}

// ============================================================================
// Unicast stream
// ============================================================================

const START_STEP_TIMEOUT_MS: u32 = 8000;
const STOP_STEP_TIMEOUT_MS: u32 = 3000;
/// While stopping, the ASEs that are not there yet are read again this
/// often: a server's notifications may arrive out of order (the emulator's
/// Bumble sent Idle before Releasing), leaving a stale state that only the
/// step timeout would end
const REFRESH_MS: u32 = 500;

/// Step of the unicast procedure
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Idle,
    ConfigCodec,
    CreateCig,
    ConfigQos,
    Enable,
    CreateCis,
    WaitStreaming,
    Streaming,
    Disable,
    Release,
    RemoveCig,
}

impl Phase {
    pub fn name(self) -> &'static str {
        match self {
            Phase::Idle => "idle",
            Phase::ConfigCodec => "Config Codec",
            Phase::CreateCig => "Set CIG Parameters",
            Phase::ConfigQos => "Config QoS",
            Phase::Enable => "Enable",
            Phase::CreateCis => "Create CIS",
            Phase::WaitStreaming => "Receiver Start Ready",
            Phase::Streaming => "streaming",
            Phase::Disable => "Disable",
            Phase::Release => "Release",
            Phase::RemoveCig => "Remove CIG",
        }
    }

    fn stopping(self) -> bool {
        matches!(self, Phase::Disable | Phase::Release | Phase::RemoveCig)
    }
}

/// Completion of [`Unicast::start`]: Ok, or why it failed
pub type UnicastDone = Box<dyn FnOnce(Result<(), String>)>;

/// Completion of [`Unicast::stop`]
pub type UnicastStopped = Box<dyn FnOnce()>;

/// The timers of the state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnicastTimer {
    /// The step timeout ([`Unicast::on_timeout`])
    Step,
    /// Retry of the CIS disconnections ([`Unicast::drop_cis`])
    Drop,
    /// Reading the ASEs again while stopping ([`Unicast::on_refresh`])
    Refresh,
}

impl UnicastTimer {
    pub(crate) const ALL: usize = 3;
}

/// What the state machine needs of the LE core
pub trait UnicastEnv {
    /// The device with this id (`CisPlan::device_id`)
    fn device(&self, id: u32) -> Option<&LeDevice>;
    /// A write to the device's ASE Control Point (`what` for the log)
    fn cp_write(&mut self, device: u32, value: Vec<u8>, what: &'static str);
    /// The CCIDs of the modules for Enable with these contexts
    fn ccids(&mut self, contexts: u16) -> Vec<u8>;
    fn cig_create(&mut self, params: &CigParams) -> u8;
    fn cig_remove(&mut self, cig_id: u8) -> u8;
    fn cis_create(&mut self, cig_id: u8, cis_handles: &[u16], acl_handles: &[u16]) -> u8;
    fn can_send_command(&self) -> bool;
    /// HCI Disconnect of a CIS
    fn disconnect_cis(&mut self, handle: u16) -> u8;
    /// Total_Num_ISO_Data_Packets of the controller
    fn iso_packet_count(&self) -> u8;
    fn start_sending(&mut self, cis_handles: &[u16], queue_depth: u8, frame_us: u32);
    fn stop_sending(&mut self);
    /// A CIS of the stream went down (the sender stops waiting for it)
    fn cis_down(&mut self, handle: u16);
    /// The running stream broke (BtHost::le_stream_lost)
    fn stream_lost(&mut self, why: &str);
    /// Audio flows (true) or stopped (false): the modules' on_stream_state
    fn stream_state(&mut self, plan: &StreamPlan, streaming: bool);
    fn arm(&mut self, timer: UnicastTimer, ms: u32);
    fn disarm(&mut self, timer: UnicastTimer);
    /// Read an ASE's value again (it comes to on_ase_update)
    fn read_ase(&mut self, device: u32, ase_id: u8);
}

/// The BAP unicast state machine (one stream at a time)
#[derive(Default)]
pub struct Unicast {
    phase: Phase,
    plan: Option<StreamPlan>,
    start_done: Option<UnicastDone>,
    stop_done: Vec<UnicastStopped>,
    /// Created in BTstack, not removed yet
    cig_exists: bool,
    /// cig_create waiting for GAP_SUBEVENT_CIG_CREATED
    cig_pending: bool,
    cig_removing: bool,
    /// GAP_SUBEVENT_CIS_CREATED still to come
    cis_pending: usize,
    cis_up: usize,
    /// start() not reported yet
    starting: bool,
    fail_reason: String,
    latency_ms: u16,
    phy: u8,
    /// CIG / Config QoS: 0 unframed, 1 framed
    framing: u8,
    /// Enable waits for an available context
    waiting_contexts: bool,
    /// CIS handles established, not down yet
    cis_connected: Vec<u16>,
    /// HCI Disconnect still to send
    cis_to_drop: Vec<u16>,
    /// HCI Disconnect sent, waiting for the event
    cis_dropping: Vec<u16>,
}

/// The CIS of each device in the plan, by device id (one CP write per
/// device)
fn by_device(plan: &StreamPlan) -> BTreeMap<u32, Vec<CisPlan>> {
    let mut m: BTreeMap<u32, Vec<CisPlan>> = BTreeMap::new();
    for c in &plan.cis {
        m.entry(c.device_id).or_default().push(c.clone());
    }
    m
}

fn connected(env: &dyn UnicastEnv, id: u32) -> bool {
    env.device(id).is_some_and(|d| d.connected())
}

fn ase_of<'e>(env: &'e dyn UnicastEnv, c: &CisPlan) -> Option<&'e Ase> {
    env.device(c.device_id)?.ase(c.ase_id)
}

/// All planned ASEs satisfy pred (devices that are gone count as done while
/// stopping)
fn all_ases(env: &dyn UnicastEnv, plan: &StreamPlan, stopping: bool, pred: impl Fn(AseState) -> bool) -> bool {
    plan.cis.iter().all(|c| {
        if !connected(env, c.device_id) {
            return stopping;
        }
        ase_of(env, c).is_some_and(|a| pred(a.state))
    })
}

impl Unicast {
    #[cfg(test)]
    pub fn phase(&self) -> Phase {
        self.phase
    }

    #[cfg(test)]
    pub fn plan(&self) -> Option<&StreamPlan> {
        self.plan.as_ref()
    }

    #[cfg(test)]
    pub fn streaming(&self) -> bool {
        self.phase == Phase::Streaming
    }

    pub fn busy(&self) -> bool {
        self.phase != Phase::Idle
    }

    /// Does the stream use this ACL or CIS handle?
    pub fn uses_handle(&self, handle: u16) -> bool {
        self.phase != Phase::Idle
            && self.plan.as_ref().is_some_and(|p| p.cis.iter().any(|c| c.acl == handle || c.cis_handle == handle))
    }

    pub fn start(&mut self, env: &mut dyn UnicastEnv, plan: StreamPlan, done: UnicastDone) {
        tracing::info!("LE Audio: starting unicast stream: {}", plan.describe());
        for c in &plan.cis {
            let locs: Vec<String> = c.channels.iter().map(|ch| format!("0x{:x}", ch.allocation)).collect();
            tracing::info!(
                "LE Audio:   CIS {}: device {} ASE {}, locations {}, SDU {} bytes",
                c.cis_id,
                c.device_id,
                c.ase_id,
                locs.join(","),
                c.sdu_len(plan.preset.octets)
            );
        }
        self.plan = Some(plan);
        self.start_done = Some(done);
        self.starting = true;
        self.fail_reason.clear();
        self.cis_up = 0;
        self.cis_pending = 0;
        self.waiting_contexts = false;
        self.cis_connected.clear();
        self.cis_to_drop.clear();
        self.cis_dropping.clear();
        env.disarm(UnicastTimer::Drop);
        self.enter(env, Phase::ConfigCodec);
    }

    pub fn stop(&mut self, env: &mut dyn UnicastEnv, done: UnicastStopped) {
        tracing::debug!(
            "LE Audio: unicast stop requested in {} (starting {}, {} stop(s) waiting)",
            self.phase.name(),
            self.starting,
            self.stop_done.len()
        );
        if self.phase == Phase::Idle {
            done();
            return;
        }
        // Already stopping: chained
        self.stop_done.push(done);
        if self.phase.stopping() {
            return;
        }
        if self.starting && self.fail_reason.is_empty() {
            self.fail_reason = "stopped".into();
        }
        self.enter(env, Phase::Disable);
    }

    fn enter(&mut self, env: &mut dyn UnicastEnv, p: Phase) {
        self.phase = p;
        if p == Phase::Streaming || p == Phase::Idle {
            env.disarm(UnicastTimer::Step);
        } else {
            env.arm(UnicastTimer::Step, if p.stopping() { STOP_STEP_TIMEOUT_MS } else { START_STEP_TIMEOUT_MS });
        }
        if matches!(p, Phase::Disable | Phase::Release) {
            env.arm(UnicastTimer::Refresh, REFRESH_MS);
        } else {
            env.disarm(UnicastTimer::Refresh);
        }
        tracing::info!("LE Audio: unicast: {}", p.name());
        let Some(plan) = self.plan.clone() else { return };

        match p {
            Phase::ConfigCodec => {
                for (dev, cis) in by_device(&plan) {
                    if !connected(env, dev) {
                        self.fail(env, "a device is not connected");
                        return;
                    }
                    let ases: Vec<ConfigCodec> = cis
                        .iter()
                        .map(|c| ConfigCodec {
                            ase_id: c.ase_id,
                            target_latency: plan.preset.target_latency,
                            target_phy: plan.phy,
                            codec_id: [CODING_FORMAT_LC3, 0, 0, 0, 0],
                            config: codec_config_ltvs(&plan, c),
                        })
                        .collect();
                    for a in &ases {
                        tracing::debug!(
                            "LE Audio: device {dev} Config Codec ASE {}: latency {}, PHY 0x{:02X}, codec {:02X?}, config {:02X?}",
                            a.ase_id,
                            a.target_latency,
                            a.target_phy,
                            a.codec_id,
                            a.config
                        );
                    }
                    env.cp_write(dev, cp_config_codec(&ases), "Config Codec");
                }
            }

            Phase::CreateCig => {
                if !self.create_cig(env, &plan) {
                    return;
                }
            }

            Phase::ConfigQos => {
                for (dev, cis) in by_device(&plan) {
                    if !connected(env, dev) {
                        self.fail(env, "a device is not connected");
                        return;
                    }
                    let ases: Vec<ConfigQos> = cis
                        .iter()
                        .map(|c| ConfigQos {
                            ase_id: c.ase_id,
                            cig_id: plan.cig_id,
                            cis_id: c.cis_id,
                            sdu_interval_us: plan.preset.sdu_interval_us(),
                            framing: self.framing,
                            phy: self.phy,
                            max_sdu: c.sdu_len(plan.preset.octets),
                            rtn: plan.preset.rtn,
                            max_transport_latency_ms: self.latency_ms,
                            presentation_delay_us: plan.presentation_delay_us,
                        })
                        .collect();
                    for a in &ases {
                        tracing::debug!("LE Audio: device {dev} Config QoS {a:?}");
                    }
                    env.cp_write(dev, cp_config_qos(&ases), "Config QoS");
                }
            }

            Phase::Enable => {
                self.waiting_contexts = false;
                if !self.send_enable(env) {
                    return;
                }
            }

            Phase::CreateCis => {
                let mut cis = Vec::new();
                let mut acl = Vec::new();
                if let Some(pl) = self.plan.as_mut() {
                    for c in pl.cis.iter_mut() {
                        c.acl = env.device(c.device_id).map_or(CON_HANDLE_INVALID, |d| d.acl);
                        cis.push(c.cis_handle);
                        acl.push(c.acl);
                    }
                }
                self.cis_up = 0;
                let status = env.cis_create(plan.cig_id, &cis, &acl);
                if status != 0 {
                    self.fail(env, &format!("LE Create CIS could not start (0x{status:02x})"));
                    return;
                }
                self.cis_pending = cis.len();
            }

            // Sink ASEs: the server (the audio receiver) sends Receiver
            // Start Ready itself; the client waits for Streaming
            Phase::WaitStreaming => {}

            Phase::Streaming => {
                let handles: Vec<u16> = plan.cis.iter().map(|c| c.cis_handle).collect();
                // BTstack queues up to `queue` SDUs per CIS and sends each as
                // one HCI ISO data packet (plan_stream made sure none needs
                // fragmenting): queue x CIS must stay within the
                // controller's Total_Num_ISO_Data_Packets
                let depth = queue_depth(env.iso_packet_count(), handles.len());
                tracing::debug!(
                    "LE Audio: streaming on CIS {handles:04X?}: {} ISO buffers -> queue depth {depth} per CIS",
                    env.iso_packet_count()
                );
                if depth == 0 {
                    self.fail(env, "the adapter has fewer ISO data buffers than the stream has CIS");
                    return;
                }
                env.start_sending(&handles, depth, u32::from(plan.preset.frame_us));
                self.starting = false;
                env.stream_state(&plan, true);
                if let Some(d) = self.start_done.take() {
                    d(Ok(()));
                }
            }

            Phase::Disable => {
                env.stop_sending();
                let mut any = false;
                for (dev, cis) in by_device(&plan) {
                    if !connected(env, dev) {
                        continue;
                    }
                    let ids: Vec<u8> = cis
                        .iter()
                        .filter(|c| {
                            ase_of(env, c).is_some_and(|a| matches!(a.state, AseState::Enabling | AseState::Streaming))
                        })
                        .map(|c| c.ase_id)
                        .collect();
                    if ids.is_empty() {
                        continue;
                    }
                    tracing::debug!("LE Audio: device {dev} Disable ASE {ids:?}");
                    env.cp_write(dev, cp_ase_ids(ASE_OP_DISABLE, &ids), "Disable");
                    any = true;
                }
                if !any {
                    self.enter(env, Phase::Release);
                    return;
                }
            }

            Phase::Release => {
                // The ASEs left Streaming (Disable): end their CIS now.
                // Servers may keep an ASE in Releasing (or not leave QoS
                // Configured) until the client drops the CIS (BAP 1.0.1 5.6.5
                // / 5.6.6); BTstack would do it only when the CIG is removed,
                // after Release.
                for h in self.cis_connected.clone() {
                    if !self.cis_to_drop.contains(&h) && !self.cis_dropping.contains(&h) {
                        self.cis_to_drop.push(h);
                    }
                }
                self.drop_cis(env);
                let mut any = false;
                for (dev, cis) in by_device(&plan) {
                    if !connected(env, dev) {
                        continue;
                    }
                    let ids: Vec<u8> = cis
                        .iter()
                        .filter(|c| {
                            ase_of(env, c).is_some_and(|a| {
                                !matches!(a.state, AseState::Idle | AseState::Releasing | AseState::Unknown)
                            })
                        })
                        .map(|c| c.ase_id)
                        .collect();
                    if ids.is_empty() {
                        continue;
                    }
                    tracing::debug!("LE Audio: device {dev} Release ASE {ids:?}");
                    env.cp_write(dev, cp_ase_ids(ASE_OP_RELEASE, &ids), "Release");
                    any = true;
                }
                if !any {
                    self.enter(env, Phase::RemoveCig);
                    return;
                }
            }

            Phase::RemoveCig => {
                self.try_remove_cig(env);
                return;
            }

            Phase::Idle => {}
        }
        self.advance(env);
    }

    /// LE Set CIG Parameters with the QoS of the preset within what the
    /// servers asked for; false when it failed (fail() ran)
    fn create_cig(&mut self, env: &mut dyn UnicastEnv, plan: &StreamPlan) -> bool {
        let (mut lower, mut upper) = (0u32, 0xFF_FFFFu32);
        let mut pd = plan.preset.presentation_delay_us;
        let mut latency = plan.preset.max_transport_latency_ms;
        let mut phy = plan.phy;
        let mut framing = plan.preset.framing;
        for c in &plan.cis {
            let Some(a) = ase_of(env, c) else { continue };
            tracing::debug!(
                "LE Audio: ASE {} preferences: framing {}, PHY 0x{:02X}, RTN {}, max latency {} ms, presentation delay {}-{} us",
                c.ase_id,
                a.pref_framing,
                a.pref_phy,
                a.pref_rtn,
                a.pref_max_latency_ms,
                a.pd_min_us,
                a.pd_max_us
            );
            // Preferred_Framing 0x01: unframed ISO PDUs not supported
            if a.pref_framing == 0x01 {
                framing = 1;
            }
            if a.pd_min_us != 0 {
                lower = lower.max(a.pd_min_us);
            }
            if a.pd_max_us != 0 {
                upper = upper.min(a.pd_max_us);
            }
            if a.pref_max_latency_ms != 0 {
                latency = latency.min(a.pref_max_latency_ms);
            }
            if a.pref_phy != 0 && a.pref_phy & phy == 0 {
                phy = PHY_1M;
            }
        }
        pd = pd.min(upper).max(lower);
        latency = latency.max(5);
        self.latency_ms = latency;
        self.phy = phy;
        self.framing = framing;
        if let Some(p) = self.plan.as_mut() {
            p.presentation_delay_us = pd;
            p.phy = phy;
        }
        tracing::info!(
            "LE Audio: QoS: SDU interval {} us, {}, max latency {latency} ms, RTN {}, PHY 0x{phy:02x}, presentation \
             delay {pd} us",
            plan.preset.sdu_interval_us(),
            if framing != 0 { "framed" } else { "unframed" },
            plan.preset.rtn
        );
        let mut params = CigParams {
            cig_id: plan.cig_id,
            sdu_interval_c_to_p: plan.preset.sdu_interval_us(),
            sdu_interval_p_to_c: plan.preset.sdu_interval_us(),
            worst_case_sca: 0,
            packing: 0,
            framing,
            max_transport_latency_c_to_p: latency,
            max_transport_latency_p_to_c: latency,
            cis: plan
                .cis
                .iter()
                .map(|c| CisParams {
                    cis_id: c.cis_id,
                    max_sdu_c_to_p: c.sdu_len(plan.preset.octets),
                    max_sdu_p_to_c: 0,
                    phy_c_to_p: phy,
                    phy_p_to_c: phy,
                    rtn_c_to_p: plan.preset.rtn,
                    rtn_p_to_c: plan.preset.rtn,
                })
                .collect(),
        };
        let mut status = env.cig_create(&params);
        let mut tries = 0;
        while status == ERROR_CODE_ACL_CONNECTION_ALREADY_EXISTS && tries < 3 {
            // BTstack still holds a CIG with this id (its removal never
            // finished, e.g. a controller that did not report a CIS down):
            // use another id
            let next = params.cig_id % 4 + 1;
            tracing::warn!("LE Audio: CIG {} still exists in BTstack, using CIG {next}", params.cig_id);
            params.cig_id = next;
            if let Some(p) = self.plan.as_mut() {
                p.cig_id = next;
            }
            status = env.cig_create(&params);
            tries += 1;
        }
        if status != 0 {
            let mut why = format!("LE Set CIG Parameters could not start (0x{status:02x})");
            if status == ERROR_CODE_MEMORY_CAPACITY_EXCEEDED {
                // The shim's 4 CIG slots hold CIGs BTstack never removed
                // (their CIS were never established)
                why.push_str(": BTstack still holds 4 CIGs it could not remove; restart Bluetooth");
            }
            self.fail(env, &why);
            return false;
        }
        self.cig_pending = true;
        self.cig_exists = true;
        true
    }

    fn try_remove_cig(&mut self, env: &mut dyn UnicastEnv) {
        if !self.cig_exists {
            self.finish_stop(env);
            return;
        }
        // BTstack must not get gap_cig_remove() while it still waits for the
        // CIG or CIS of a running command: it would resume them afterwards
        if self.cig_pending || self.cis_pending > 0 {
            tracing::debug!(
                "LE Audio: CIG removal waits (CIG pending {}, {} CIS pending)",
                self.cig_pending,
                self.cis_pending
            );
            return;
        }
        // ... nor while our own CIS disconnections are under way
        if !self.cis_to_drop.is_empty() || !self.cis_dropping.is_empty() {
            tracing::debug!(
                "LE Audio: CIG removal waits for CIS disconnections ({:04X?} to send, {:04X?} under way)",
                self.cis_to_drop,
                self.cis_dropping
            );
            return;
        }
        if !self.cig_removing {
            let cig_id = self.plan.as_ref().map_or(1, |p| p.cig_id);
            let status = env.cig_remove(cig_id);
            if status != 0 {
                tracing::warn!("LE Audio: gap_cig_remove failed (0x{status:02x})");
                self.cig_exists = false;
                self.finish_stop(env);
                return;
            }
            self.cig_removing = true;
        }
    }

    /// Streaming Audio Contexts for Enable from what the devices have
    /// available now (Available Audio Contexts may have changed since
    /// plan_stream)
    fn current_contexts(&self, env: &dyn UnicastEnv) -> u16 {
        let Some(plan) = &self.plan else { return 0 };
        let mut seen = Vec::new();
        let mut available = Vec::new();
        for c in &plan.cis {
            if seen.contains(&c.device_id) {
                continue;
            }
            seen.push(c.device_id);
            if let Some(d) = env.device(c.device_id) {
                available.push(d.available_sink_contexts);
            }
        }
        choose_streaming_contexts(&available, plan.requested_contexts)
    }

    fn contexts_text(&self, env: &dyn UnicastEnv) -> String {
        let Some(plan) = &self.plan else { return String::new() };
        let mut s = format!("wanted 0x{:04x}", plan.requested_contexts);
        let mut seen = Vec::new();
        for c in &plan.cis {
            if seen.contains(&c.device_id) {
                continue;
            }
            seen.push(c.device_id);
            if let Some(d) = env.device(c.device_id) {
                s.push_str(&format!(", {} has 0x{:04x}", d.addr, d.available_sink_contexts));
            }
        }
        s
    }

    /// Enable with the Streaming Audio Contexts the devices have available.
    /// False (nothing sent) while none is: waits for an Available Audio
    /// Contexts notification (on_available_contexts) until the step times
    /// out.
    fn send_enable(&mut self, env: &mut dyn UnicastEnv) -> bool {
        let contexts = self.current_contexts(env);
        if contexts == 0 {
            if !self.waiting_contexts {
                tracing::warn!(
                    "LE Audio: no audio context available for Enable ({}): waiting",
                    self.contexts_text(env)
                );
            }
            self.waiting_contexts = true;
            return false;
        }
        self.waiting_contexts = false;
        let Some(plan) = self.plan.as_mut() else { return false };
        if contexts != plan.requested_contexts {
            tracing::info!(
                "LE Audio: streaming context 0x{contexts:04x} (0x{:04x} is not available)",
                plan.requested_contexts
            );
        }
        plan.contexts = contexts;
        let plan = plan.clone();
        // Metadata: Streaming Audio Contexts, plus the CCID List of the
        // content control services the modules tie to these contexts
        let mut ccids = env.ccids(contexts);
        ccids.sort_unstable();
        ccids.dedup();
        let meta = enable_metadata(contexts, &ccids);
        tracing::debug!("LE Audio: Enable metadata {meta:02X?}");
        if !ccids.is_empty() {
            let list: Vec<String> = ccids.iter().map(|c| c.to_string()).collect();
            tracing::info!("LE Audio: Enable metadata: contexts 0x{contexts:04x}, CCID list {}", list.join(","));
        }
        for (dev, cis) in by_device(&plan) {
            if !connected(env, dev) {
                self.fail(env, "a device is not connected");
                return false;
            }
            let ases: Vec<(u8, Vec<u8>)> = cis.iter().map(|c| (c.ase_id, meta.clone())).collect();
            env.cp_write(dev, cp_with_metadata(ASE_OP_ENABLE, &ases), "Enable");
        }
        true
    }

    /// A device's Available Audio Contexts changed (PACS notification)
    pub fn on_available_contexts(&mut self, env: &mut dyn UnicastEnv, dev: LeDeviceId) {
        if self.phase != Phase::Enable || !self.waiting_contexts {
            return;
        }
        let planned = self.plan.as_ref().is_some_and(|p| p.cis.iter().any(|c| c.device_id == dev.0));
        tracing::debug!("LE Audio: device {} available contexts changed while Enable waits (planned {planned})", dev.0);
        if planned && self.send_enable(env) {
            self.advance(env);
        }
    }

    /// HCI Disconnect for the CIS in cis_to_drop. BTstack has no call for a
    /// CIS of a CIG that stays (gap_disconnect only knows ACL / SCO
    /// connections); its Disconnection Complete handling sets the ISO stream
    /// idle, so the CIG removal later skips it.
    pub fn drop_cis(&mut self, env: &mut dyn UnicastEnv) {
        env.disarm(UnicastTimer::Drop);
        while let Some(&h) = self.cis_to_drop.first() {
            if !env.can_send_command() {
                tracing::trace!("LE Audio: HCI busy, CIS disconnect 0x{h:04X} retried");
                env.arm(UnicastTimer::Drop, 2);
                return;
            }
            self.cis_to_drop.remove(0);
            let status = env.disconnect_cis(h);
            if status == 0 {
                tracing::info!("LE Audio: disconnecting CIS 0x{h:04x}");
                self.cis_dropping.push(h);
            } else {
                tracing::warn!("LE Audio: disconnecting CIS 0x{h:04x} failed (0x{status:02x})");
            }
        }
    }

    /// LE Remove CIG completed
    pub fn on_cig_removed(&mut self, env: &mut dyn UnicastEnv) {
        if !self.cig_removing {
            tracing::debug!("LE Audio: CIG removal complete, none requested (ignored)");
            return;
        }
        self.cig_removing = false;
        self.cig_exists = false;
        tracing::info!("LE Audio: CIG {} removed", self.plan.as_ref().map_or(0, |p| p.cig_id));
        if self.phase == Phase::RemoveCig {
            self.finish_stop(env);
        }
    }

    fn finish_stop(&mut self, env: &mut dyn UnicastEnv) {
        let was_starting = self.starting;
        self.phase = Phase::Idle;
        env.disarm(UnicastTimer::Step);
        env.disarm(UnicastTimer::Drop);
        env.disarm(UnicastTimer::Refresh);
        self.waiting_contexts = false;
        self.cis_connected.clear();
        self.cis_to_drop.clear();
        self.cis_dropping.clear();
        self.starting = false;
        if let Some(plan) = self.plan.clone() {
            env.stream_state(&plan, false);
        }
        tracing::info!("LE Audio: unicast stream ended");
        if was_starting && let Some(d) = self.start_done.take() {
            d(Err(if self.fail_reason.is_empty() { "stopped".into() } else { self.fail_reason.clone() }));
        }
        for d in std::mem::take(&mut self.stop_done) {
            d();
        }
    }

    fn fail(&mut self, env: &mut dyn UnicastEnv, why: &str) {
        if !self.fail_reason.is_empty() {
            return;
        }
        self.fail_reason = why.to_string();
        tracing::error!("LE Audio: unicast stream failed in {}: {why}", self.phase.name());
        self.enter(env, Phase::Disable);
    }

    /// While stopping: read the ASEs again that did not reach the state the
    /// step waits for
    pub fn on_refresh(&mut self, env: &mut dyn UnicastEnv) {
        let Some(plan) = self.plan.clone() else { return };
        let done: fn(AseState) -> bool = match self.phase {
            Phase::Disable => |s| !matches!(s, AseState::Enabling | AseState::Streaming | AseState::Disabling),
            Phase::Release => |s| matches!(s, AseState::Idle | AseState::CodecConfigured | AseState::Unknown),
            _ => return,
        };
        for c in &plan.cis {
            if !connected(env, c.device_id) {
                continue;
            }
            if let Some(a) = ase_of(env, c).filter(|a| !done(a.state)) {
                tracing::info!(
                    "LE Audio: device {} ASE {} still {} in {}: reading it again",
                    c.device_id,
                    c.ase_id,
                    a.state.name(),
                    self.phase.name()
                );
                env.read_ase(c.device_id, c.ase_id);
            }
        }
        env.arm(UnicastTimer::Refresh, REFRESH_MS);
    }

    /// The step timer fired
    pub fn on_timeout(&mut self, env: &mut dyn UnicastEnv) {
        tracing::debug!(
            "LE Audio: step timeout in {} (CIG exists {}, pending {}, removing {}, {} CIS pending)",
            self.phase.name(),
            self.cig_exists,
            self.cig_pending,
            self.cig_removing,
            self.cis_pending
        );
        match self.phase {
            Phase::Disable => {
                tracing::warn!("LE Audio: Disable timed out");
                self.enter(env, Phase::Release);
            }
            Phase::Release => {
                tracing::warn!("LE Audio: Release timed out");
                self.enter(env, Phase::RemoveCig);
            }
            Phase::RemoveCig => {
                tracing::warn!("LE Audio: CIG removal timed out");
                if !self.cig_removing && self.cig_exists {
                    // Still waiting for a CIG / CIS event: remove it anyway
                    self.cig_pending = false;
                    self.cis_pending = 0;
                    env.disarm(UnicastTimer::Drop);
                    self.cis_to_drop.clear();
                    self.cis_dropping.clear();
                    self.try_remove_cig(env);
                    if self.phase == Phase::RemoveCig {
                        env.arm(UnicastTimer::Step, STOP_STEP_TIMEOUT_MS);
                    }
                } else {
                    // BTstack never finished removing it (the shim keeps its
                    // storage until BTstack lets it go)
                    self.cig_removing = false;
                    self.cig_exists = false;
                    self.finish_stop(env);
                }
            }
            Phase::Idle | Phase::Streaming => {}
            Phase::Enable if self.waiting_contexts => {
                let why = format!("no audio context available for the stream ({})", self.contexts_text(env));
                self.fail(env, &why);
            }
            _ => {
                let mut why = format!("no answer in {}", self.phase.name());
                // Which ASEs did not get there
                if let Some(plan) = &self.plan {
                    for c in &plan.cis {
                        let state = ase_of(env, c).map_or("gone", |a| a.state.name());
                        why.push_str(&format!("; ASE {} {state}", c.ase_id));
                    }
                }
                self.fail(env, &why);
            }
        }
    }

    fn advance(&mut self, env: &mut dyn UnicastEnv) {
        let Some(plan) = &self.plan else { return };
        let next = match self.phase {
            Phase::ConfigCodec if all_ases(env, plan, false, |s| s == AseState::CodecConfigured) => {
                Some(Phase::CreateCig)
            }
            Phase::ConfigQos if all_ases(env, plan, false, |s| s == AseState::QosConfigured) => Some(Phase::Enable),
            Phase::Enable if all_ases(env, plan, false, |s| matches!(s, AseState::Enabling | AseState::Streaming)) => {
                Some(Phase::CreateCis)
            }
            Phase::CreateCis if self.cis_up == plan.cis.len() => Some(Phase::WaitStreaming),
            Phase::WaitStreaming if all_ases(env, plan, false, |s| s == AseState::Streaming) => Some(Phase::Streaming),
            Phase::Disable
                if all_ases(env, plan, true, |s| {
                    !matches!(s, AseState::Enabling | AseState::Streaming | AseState::Disabling)
                }) =>
            {
                Some(Phase::Release)
            }
            Phase::Release
                if all_ases(env, plan, true, |s| {
                    matches!(s, AseState::Idle | AseState::CodecConfigured | AseState::Unknown)
                }) =>
            {
                Some(Phase::RemoveCig)
            }
            Phase::RemoveCig => {
                self.try_remove_cig(env);
                None
            }
            _ => None,
        };
        if let Some(p) = next {
            self.enter(env, p);
        }
    }

    // ---- events (from the core / the GATT listeners) ----

    /// An ASE value changed (read or notified)
    pub fn on_ase_update(&mut self, env: &mut dyn UnicastEnv, dev: LeDeviceId, ase_id: u8) {
        if self.phase == Phase::Idle {
            return;
        }
        let planned =
            self.plan.as_ref().is_some_and(|p| p.cis.iter().any(|c| c.device_id == dev.0 && c.ase_id == ase_id));
        if !planned {
            return;
        }
        if self.phase == Phase::Streaming {
            let Some(d) = env.device(dev.0) else { return };
            let state = d.ase(ase_id).map_or(AseState::Unknown, |a| a.state);
            if state != AseState::Streaming {
                // The server disabled / released it (e.g. the headset
                // switched to another source)
                tracing::warn!("LE Audio: {} ASE {ase_id} left Streaming ({})", d.addr, state.name());
                env.stream_lost("an ASE left the Streaming state");
                return;
            }
        }
        self.advance(env);
    }

    /// An ASE Control Point notification
    pub fn on_cp_response(&mut self, env: &mut dyn UnicastEnv, dev: LeDeviceId, v: &[u8]) {
        if v.len() < 2 {
            return;
        }
        let addr = env.device(dev.0).map(|d| d.addr.to_string()).unwrap_or_default();
        tracing::debug!("LE Audio: {addr}: ASE control point response {v:02X?} in {}", self.phase.name());
        let (op, count) = (v[0], v[1]);
        if count == 0xFF {
            if v.len() >= 5 && v[3] != 0 {
                tracing::warn!(
                    "LE Audio: {addr}: ASE control point op 0x{op:02x} rejected: response 0x{:02x} reason 0x{:02x}",
                    v[3],
                    v[4]
                );
                if self.phase != Phase::Idle && self.fail_reason.is_empty() && !self.phase.stopping() {
                    let why = format!("ASE control point op 0x{op:02x} rejected (0x{:02x} / 0x{:02x})", v[3], v[4]);
                    self.fail(env, &why);
                }
            }
            return;
        }
        for i in 0..count as usize {
            let Some(e) = v.get(2 + 3 * i..5 + 3 * i) else { break };
            let (ase_id, code, reason) = (e[0], e[1], e[2]);
            if code == 0 {
                continue;
            }
            tracing::warn!("LE Audio: {addr}: ASE {ase_id} op 0x{op:02x}: response 0x{code:02x} reason 0x{reason:02x}");
            let planned =
                self.plan.as_ref().is_some_and(|p| p.cis.iter().any(|c| c.device_id == dev.0 && c.ase_id == ase_id));
            let start_phase =
                matches!(self.phase, Phase::ConfigCodec | Phase::CreateCig | Phase::ConfigQos | Phase::Enable);
            if planned && start_phase {
                let why = format!("ASE {ase_id}: op 0x{op:02x} rejected (response 0x{code:02x} reason 0x{reason:02x})");
                self.fail(env, &why);
                return;
            }
        }
    }

    /// GAP_SUBEVENT_CIG_CREATED
    pub fn on_cig_created(&mut self, env: &mut dyn UnicastEnv, packet: &[u8]) {
        let Some(ev) = events::cig_created(packet) else { return };
        tracing::debug!("LE Audio: CIG created event {ev:?} in {}", self.phase.name());
        let Some(plan) = self.plan.as_mut() else { return };
        if !self.cig_pending || ev.cig_id != plan.cig_id {
            tracing::debug!("LE Audio: CIG event ignored (pending {}, planned CIG {})", self.cig_pending, plan.cig_id);
            return;
        }
        self.cig_pending = false;
        if ev.status != 0 {
            self.cig_exists = false; // BTstack dropped it
            self.fail(env, &format!("LE Set CIG Parameters failed (0x{:02x})", ev.status));
            return;
        }
        for (c, h) in plan.cis.iter_mut().zip(&ev.cis_handles) {
            c.cis_handle = *h;
        }
        tracing::info!("LE Audio: CIG {} created, {} CIS", plan.cig_id, ev.cis_handles.len());
        if self.phase == Phase::CreateCig {
            self.enter(env, Phase::ConfigQos);
        } else {
            // Stopping: may now remove it
            self.advance(env);
        }
    }

    /// GAP_SUBEVENT_CIS_CREATED
    pub fn on_cis_created(&mut self, env: &mut dyn UnicastEnv, packet: &[u8]) {
        let Some(ev) = events::cis_created(packet) else { return };
        tracing::debug!("LE Audio: CIS created event {ev:?} in {} ({} pending)", self.phase.name(), self.cis_pending);
        if self.plan.as_ref().is_none_or(|p| p.cig_id != ev.cig_id) || self.cis_pending == 0 {
            return;
        }
        self.cis_pending -= 1;
        if ev.status != 0 {
            let why = format!("CIS 0x{:04x} could not be established (0x{:02x})", ev.cis_handle, ev.status);
            if self.phase == Phase::CreateCis {
                self.fail(env, &why);
            } else {
                tracing::warn!("LE Audio: {why}");
            }
        } else {
            self.cis_up += 1;
            self.cis_connected.push(ev.cis_handle);
            tracing::info!(
                "LE Audio: CIS 0x{:04x} established (ISO interval {} x 1.25 ms)",
                ev.cis_handle,
                ev.iso_interval_1250us
            );
        }
        self.advance(env);
    }

    /// An ACL or CIS of the stream went down
    pub fn on_link_lost(&mut self, env: &mut dyn UnicastEnv, handle: u16) {
        if !self.uses_handle(handle) {
            return;
        }
        let is_cis = self.plan.as_ref().is_some_and(|p| p.cis.iter().any(|c| c.cis_handle == handle));
        tracing::debug!(
            "LE Audio: stream link 0x{handle:04X} ({}) lost in {}",
            if is_cis { "CIS" } else { "ACL" },
            self.phase.name()
        );
        if is_cis {
            if self.cis_dropping.contains(&handle) || self.cis_to_drop.contains(&handle) {
                tracing::info!("LE Audio: CIS 0x{handle:04x} disconnected");
            } else {
                tracing::warn!("LE Audio: CIS 0x{handle:04x} disconnected");
                if self.phase == Phase::CreateCis && self.cis_pending > 0 {
                    self.cis_pending -= 1;
                }
            }
            self.cis_connected.retain(|&h| h != handle);
            self.cis_to_drop.retain(|&h| h != handle);
            self.cis_dropping.retain(|&h| h != handle);
            env.cis_down(handle);
        }
        match self.phase {
            Phase::Streaming => env.stream_lost("a link of the stream went down"),
            Phase::Disable | Phase::Release | Phase::RemoveCig => self.advance(env),
            _ => self.fail(env, "a link went down"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use le_audio::plan::{ChannelPlan, ChannelSource};
    use le_audio::presets::PresetTable;

    use super::*;
    use crate::le::LeAddr;

    /// The core as the state machine sees it: devices, and a log of what it
    /// asked for
    struct Fake {
        devices: Vec<LeDevice>,
        log: Vec<String>,
        iso_count: u8,
        cig_status: Vec<u8>,
        can_send: bool,
        lost: Option<String>,
    }

    impl Fake {
        fn new(devices: Vec<LeDevice>) -> Fake {
            Fake { devices, log: Vec::new(), iso_count: 8, cig_status: Vec::new(), can_send: true, lost: None }
        }
        fn take(&mut self) -> Vec<String> {
            std::mem::take(&mut self.log)
        }
        fn set_state(&mut self, dev: u32, ase: u8, state: AseState) {
            self.devices[dev as usize].ase_mut(ase).unwrap().state = state;
        }
    }

    impl UnicastEnv for Fake {
        fn device(&self, id: u32) -> Option<&LeDevice> {
            self.devices.get(id as usize)
        }
        fn cp_write(&mut self, device: u32, value: Vec<u8>, what: &'static str) {
            self.log.push(format!("{what} dev {device} {:02x?}", &value[..2.min(value.len())]));
        }
        fn ccids(&mut self, _contexts: u16) -> Vec<u8> {
            vec![2, 1, 2]
        }
        fn cig_create(&mut self, params: &CigParams) -> u8 {
            self.log.push(format!("cig create {} ({} CIS)", params.cig_id, params.cis.len()));
            if self.cig_status.is_empty() { 0 } else { self.cig_status.remove(0) }
        }
        fn cig_remove(&mut self, cig_id: u8) -> u8 {
            self.log.push(format!("cig remove {cig_id}"));
            0
        }
        fn cis_create(&mut self, cig_id: u8, cis: &[u16], acl: &[u16]) -> u8 {
            self.log.push(format!("cis create {cig_id} {cis:x?} {acl:x?}"));
            0
        }
        fn can_send_command(&self) -> bool {
            self.can_send
        }
        fn disconnect_cis(&mut self, handle: u16) -> u8 {
            self.log.push(format!("disconnect {handle:x}"));
            0
        }
        fn iso_packet_count(&self) -> u8 {
            self.iso_count
        }
        fn start_sending(&mut self, cis: &[u16], depth: u8, frame_us: u32) {
            self.log.push(format!("send {cis:x?} queue {depth} {frame_us} us"));
        }
        fn stop_sending(&mut self) {
            self.log.push("stop sending".into());
        }
        fn cis_down(&mut self, handle: u16) {
            self.log.push(format!("cis down {handle:x}"));
        }
        fn stream_lost(&mut self, why: &str) {
            self.lost = Some(why.into());
        }
        fn stream_state(&mut self, _plan: &StreamPlan, streaming: bool) {
            self.log.push(format!("streaming {streaming}"));
        }
        fn arm(&mut self, timer: UnicastTimer, ms: u32) {
            if timer == UnicastTimer::Drop {
                self.log.push(format!("arm drop {ms}"));
            }
        }
        fn disarm(&mut self, _timer: UnicastTimer) {}
        fn read_ase(&mut self, device: u32, ase_id: u8) {
            self.log.push(format!("read ASE {ase_id} dev {device}"));
        }
    }

    fn device(id: u32, acl: u16, ases: &[u8], contexts: u16) -> LeDevice {
        let mut d =
            LeDevice::new(LeDeviceId(id), LeAddr { addr: [0, 0xA2, 0xD0, 0, 0, 0x12 + id as u8], addr_type: 0 });
        d.acl = acl;
        d.available_sink_contexts = contexts;
        d.ase_cp_handle = 0x30;
        for &a in ases {
            d.ases.push(Ase { id: a, state: AseState::Idle, value_handle: 0x20 + a as u16, ..Ase::default() });
        }
        d
    }

    /// Two earbuds, one ASE each (CIS 1 left on device 0, CIS 2 right on
    /// device 1)
    fn earbuds_plan() -> StreamPlan {
        let preset = PresetTable::new().find("48_6_2").unwrap().clone();
        let cis = |dev: u32, cis_id: u8, allocation: u32, source| CisPlan {
            device_id: dev,
            ase_id: 1,
            cis_id,
            acl: 0,
            cis_handle: 0xFFFF,
            channels: vec![ChannelPlan { allocation, source }],
        };
        StreamPlan {
            preset,
            cig_id: 1,
            phy: PHY_2M,
            requested_contexts: CONTEXT_MEDIA,
            contexts: CONTEXT_MEDIA,
            presentation_delay_us: 40_000,
            cis: vec![
                cis(0, 1, LOCATION_FRONT_LEFT, ChannelSource::Left),
                cis(1, 2, LOCATION_FRONT_RIGHT, ChannelSource::Right),
            ],
        }
    }

    fn cig_created(cig_id: u8, handles: &[u16]) -> Vec<u8> {
        let mut p = vec![0xE7, 0, 0x06, 0, cig_id, handles.len() as u8];
        for h in handles {
            p.extend_from_slice(&h.to_le_bytes());
        }
        p
    }

    fn cis_created(cig_id: u8, handle: u16, status: u8) -> Vec<u8> {
        let mut p = vec![0xE7, 0, 0x07, status, cig_id, 1];
        p.extend_from_slice(&handle.to_le_bytes());
        p.extend_from_slice(&[0x40, 0, 8, 0]);
        p
    }

    /// Both ASEs to `state`, the state machine told of each
    fn both(u: &mut Unicast, env: &mut Fake, state: AseState) {
        for dev in 0..2 {
            env.set_state(dev, 1, state);
            u.on_ase_update(env, LeDeviceId(dev), 1);
        }
    }

    type Outcome = Rc<RefCell<Option<Result<(), String>>>>;

    fn start(u: &mut Unicast, env: &mut Fake) -> Outcome {
        let outcome: Outcome = Rc::default();
        let o = outcome.clone();
        u.start(env, earbuds_plan(), Box::new(move |r| *o.borrow_mut() = Some(r)));
        outcome
    }

    /// Up to Streaming
    fn bring_up(u: &mut Unicast, env: &mut Fake) -> Outcome {
        let outcome = start(u, env);
        both(u, env, AseState::CodecConfigured);
        u.on_cig_created(env, &cig_created(1, &[0x60, 0x61]));
        both(u, env, AseState::QosConfigured);
        both(u, env, AseState::Enabling);
        u.on_cis_created(env, &cis_created(1, 0x60, 0));
        u.on_cis_created(env, &cis_created(1, 0x61, 0));
        both(u, env, AseState::Streaming);
        outcome
    }

    #[test]
    fn start_runs_the_bap_steps_in_order() {
        let mut env = Fake::new(vec![device(0, 0x40, &[1], CONTEXT_MEDIA), device(1, 0x41, &[1], CONTEXT_MEDIA)]);
        let mut u = Unicast::default();
        let outcome = start(&mut u, &mut env);
        assert_eq!(u.phase(), Phase::ConfigCodec);
        assert_eq!(env.take(), ["Config Codec dev 0 [01, 01]", "Config Codec dev 1 [01, 01]"]);
        // One device configured is not enough
        env.set_state(0, 1, AseState::CodecConfigured);
        u.on_ase_update(&mut env, LeDeviceId(0), 1);
        assert_eq!(u.phase(), Phase::ConfigCodec);
        env.set_state(1, 1, AseState::CodecConfigured);
        u.on_ase_update(&mut env, LeDeviceId(1), 1);
        assert_eq!(u.phase(), Phase::CreateCig);
        assert_eq!(env.take(), ["cig create 1 (2 CIS)"]);
        u.on_cig_created(&mut env, &cig_created(1, &[0x60, 0x61]));
        assert_eq!(u.phase(), Phase::ConfigQos);
        assert_eq!(env.take(), ["Config QoS dev 0 [02, 01]", "Config QoS dev 1 [02, 01]"]);
        both(&mut u, &mut env, AseState::QosConfigured);
        assert_eq!(u.phase(), Phase::Enable);
        assert_eq!(env.take(), ["Enable dev 0 [03, 01]", "Enable dev 1 [03, 01]"]);
        both(&mut u, &mut env, AseState::Enabling);
        assert_eq!(u.phase(), Phase::CreateCis);
        assert_eq!(env.take(), ["cis create 1 [60, 61] [40, 41]"]);
        u.on_cis_created(&mut env, &cis_created(1, 0x60, 0));
        assert_eq!(u.phase(), Phase::CreateCis);
        u.on_cis_created(&mut env, &cis_created(1, 0x61, 0));
        assert_eq!(u.phase(), Phase::WaitStreaming);
        assert!(outcome.borrow().is_none());
        both(&mut u, &mut env, AseState::Streaming);
        assert_eq!(u.phase(), Phase::Streaming);
        assert!(u.streaming());
        assert_eq!(env.take(), ["send [60, 61] queue 2 10000 us", "streaming true"]);
        assert_eq!(*outcome.borrow(), Some(Ok(())));
        assert!(u.uses_handle(0x41) && u.uses_handle(0x61) && !u.uses_handle(0x42));
        assert_eq!(u.plan().unwrap().contexts, CONTEXT_MEDIA);
    }

    #[test]
    fn stop_disables_drops_the_cis_releases_and_removes_the_cig() {
        let mut env = Fake::new(vec![device(0, 0x40, &[1], CONTEXT_MEDIA), device(1, 0x41, &[1], CONTEXT_MEDIA)]);
        let mut u = Unicast::default();
        bring_up(&mut u, &mut env);
        env.take();
        let stopped = Rc::new(RefCell::new(0));
        let s = stopped.clone();
        u.stop(&mut env, Box::new(move || *s.borrow_mut() += 1));
        assert_eq!(u.phase(), Phase::Disable);
        assert_eq!(env.take(), ["stop sending", "Disable dev 0 [05, 01]", "Disable dev 1 [05, 01]"]);
        // A second stop while stopping is chained
        let s = stopped.clone();
        u.stop(&mut env, Box::new(move || *s.borrow_mut() += 1));
        both(&mut u, &mut env, AseState::QosConfigured);
        assert_eq!(u.phase(), Phase::Release);
        // The CIS are dropped before the Release
        assert_eq!(env.take(), ["disconnect 60", "disconnect 61", "Release dev 0 [08, 01]", "Release dev 1 [08, 01]"]);
        // The server keeps the ASE Releasing until the CIS is gone
        both(&mut u, &mut env, AseState::Releasing);
        u.on_link_lost(&mut env, 0x60);
        u.on_link_lost(&mut env, 0x61);
        assert_eq!(env.take(), ["cis down 60", "cis down 61"]);
        assert_eq!(u.phase(), Phase::Release);
        both(&mut u, &mut env, AseState::Idle);
        assert_eq!(u.phase(), Phase::RemoveCig);
        assert_eq!(env.take(), ["cig remove 1"]);
        assert_eq!(*stopped.borrow(), 0);
        u.on_cig_removed(&mut env);
        assert_eq!(u.phase(), Phase::Idle);
        assert_eq!(env.take(), ["streaming false"]);
        assert_eq!(*stopped.borrow(), 2);
        assert!(env.lost.is_none());
        // Stopping an idle stream completes at once
        let s = stopped.clone();
        u.stop(&mut env, Box::new(move || *s.borrow_mut() += 1));
        assert_eq!(*stopped.borrow(), 3);
    }

    #[test]
    fn enable_waits_for_an_available_context() {
        let mut env = Fake::new(vec![device(0, 0x40, &[1], 0), device(1, 0x41, &[1], CONTEXT_MEDIA)]);
        let mut u = Unicast::default();
        start(&mut u, &mut env);
        both(&mut u, &mut env, AseState::CodecConfigured);
        u.on_cig_created(&mut env, &cig_created(1, &[0x60, 0x61]));
        env.take();
        both(&mut u, &mut env, AseState::QosConfigured);
        assert_eq!(u.phase(), Phase::Enable);
        assert!(env.take().is_empty(), "no Enable without a context");
        // Another device's contexts do not matter; this one's do
        u.on_available_contexts(&mut env, LeDeviceId(1));
        assert!(env.take().is_empty());
        env.devices[0].available_sink_contexts = CONTEXT_UNSPECIFIED | CONTEXT_MEDIA;
        u.on_available_contexts(&mut env, LeDeviceId(0));
        assert_eq!(env.take(), ["Enable dev 0 [03, 01]", "Enable dev 1 [03, 01]"]);
        assert_eq!(u.plan().unwrap().contexts, CONTEXT_MEDIA);
    }

    #[test]
    fn enable_timeout_without_context_fails_and_tears_down() {
        let mut env = Fake::new(vec![device(0, 0x40, &[1], 0), device(1, 0x41, &[1], 0)]);
        let mut u = Unicast::default();
        let outcome = start(&mut u, &mut env);
        both(&mut u, &mut env, AseState::CodecConfigured);
        u.on_cig_created(&mut env, &cig_created(1, &[0x60, 0x61]));
        both(&mut u, &mut env, AseState::QosConfigured);
        env.take();
        u.on_timeout(&mut env);
        // Nothing to disable or drop: Release, then the CIG
        assert_eq!(u.phase(), Phase::Release);
        assert_eq!(env.take(), ["stop sending", "Release dev 0 [08, 01]", "Release dev 1 [08, 01]"]);
        both(&mut u, &mut env, AseState::Idle);
        assert_eq!(env.take(), ["cig remove 1"]);
        u.on_cig_removed(&mut env);
        let r = outcome.borrow().clone().unwrap();
        assert!(r.unwrap_err().starts_with("no audio context available for the stream (wanted 0x0004"));
    }

    #[test]
    fn link_loss_while_streaming_is_reported() {
        let mut env = Fake::new(vec![device(0, 0x40, &[1], CONTEXT_MEDIA), device(1, 0x41, &[1], CONTEXT_MEDIA)]);
        let mut u = Unicast::default();
        bring_up(&mut u, &mut env);
        env.take();
        u.on_link_lost(&mut env, 0x42);
        assert!(env.lost.is_none(), "not a link of the stream");
        u.on_link_lost(&mut env, 0x61);
        assert_eq!(env.lost.as_deref(), Some("a link of the stream went down"));
        assert_eq!(env.take(), ["cis down 61"]);
        // An ASE leaving Streaming too
        env.lost = None;
        env.set_state(0, 1, AseState::Disabling);
        u.on_ase_update(&mut env, LeDeviceId(0), 1);
        assert_eq!(env.lost.as_deref(), Some("an ASE left the Streaming state"));
    }

    #[test]
    fn rejected_config_fails_the_start() {
        let mut env = Fake::new(vec![device(0, 0x40, &[1], CONTEXT_MEDIA), device(1, 0x41, &[1], CONTEXT_MEDIA)]);
        let mut u = Unicast::default();
        let outcome = start(&mut u, &mut env);
        env.take();
        // Config Codec of ASE 1 on device 1 rejected (response 0x09)
        u.on_cp_response(&mut env, LeDeviceId(1), &[ASE_OP_CONFIG_CODEC, 1, 1, 0x09, 0x02]);
        // Nothing to disable or release (the ASEs stayed Idle) and no CIG
        // was created: done at once, without LE Remove CIG
        assert_eq!(u.phase(), Phase::Idle);
        assert!(!env.take().iter().any(|l| l.starts_with("cig remove")));
        let r = outcome.borrow().clone().unwrap();
        assert_eq!(r.unwrap_err(), "ASE 1: op 0x01 rejected (response 0x09 reason 0x02)");
    }

    #[test]
    fn cig_id_in_use_takes_the_next_and_busy_hci_retries_cis_drop() {
        let mut env = Fake::new(vec![device(0, 0x40, &[1], CONTEXT_MEDIA), device(1, 0x41, &[1], CONTEXT_MEDIA)]);
        env.cig_status = vec![ERROR_CODE_ACL_CONNECTION_ALREADY_EXISTS];
        let mut u = Unicast::default();
        start(&mut u, &mut env);
        both(&mut u, &mut env, AseState::CodecConfigured);
        assert_eq!(&env.take()[2..], ["cig create 1 (2 CIS)", "cig create 2 (2 CIS)"]);
        assert_eq!(u.plan().unwrap().cig_id, 2);
        u.on_cig_created(&mut env, &cig_created(2, &[0x60, 0x61]));
        both(&mut u, &mut env, AseState::QosConfigured);
        both(&mut u, &mut env, AseState::Enabling);
        u.on_cis_created(&mut env, &cis_created(2, 0x60, 0));
        u.on_cis_created(&mut env, &cis_created(2, 0x61, 0));
        both(&mut u, &mut env, AseState::Streaming);
        env.take();
        u.stop(&mut env, Box::new(|| {}));
        both(&mut u, &mut env, AseState::QosConfigured);
        env.take();
        env.can_send = false;
        assert!(env.take().is_empty());
        env.can_send = true;
        u.drop_cis(&mut env);
        assert!(env.take().is_empty(), "already dropped");
        // HCI busy: the drop waits for the timer
        let mut env2 = Fake::new(vec![device(0, 0x40, &[1], CONTEXT_MEDIA), device(1, 0x41, &[1], CONTEXT_MEDIA)]);
        let mut u2 = Unicast::default();
        bring_up(&mut u2, &mut env2);
        u2.stop(&mut env2, Box::new(|| {}));
        env2.can_send = false;
        env2.take();
        both(&mut u2, &mut env2, AseState::QosConfigured);
        assert_eq!(env2.take(), ["arm drop 2", "Release dev 0 [08, 01]", "Release dev 1 [08, 01]"]);
        env2.can_send = true;
        u2.drop_cis(&mut env2);
        assert_eq!(env2.take(), ["disconnect 60", "disconnect 61"]);
    }

    #[test]
    fn a_stale_state_while_stopping_is_read_again() {
        let mut env = Fake::new(vec![device(0, 0x40, &[1], CONTEXT_MEDIA), device(1, 0x41, &[1], CONTEXT_MEDIA)]);
        let mut u = Unicast::default();
        bring_up(&mut u, &mut env);
        u.stop(&mut env, Box::new(|| {}));
        both(&mut u, &mut env, AseState::QosConfigured);
        u.on_link_lost(&mut env, 0x60);
        u.on_link_lost(&mut env, 0x61);
        // Device 1 notified Idle before Releasing
        env.set_state(0, 1, AseState::Idle);
        u.on_ase_update(&mut env, LeDeviceId(0), 1);
        env.set_state(1, 1, AseState::Releasing);
        u.on_ase_update(&mut env, LeDeviceId(1), 1);
        env.take();
        u.on_refresh(&mut env);
        assert_eq!(env.take(), ["read ASE 1 dev 1"]);
        env.set_state(1, 1, AseState::Idle);
        u.on_ase_update(&mut env, LeDeviceId(1), 1);
        assert_eq!(u.phase(), Phase::RemoveCig);
        // Not stopping in a step that waits for ASEs: nothing to read
        u.on_refresh(&mut env);
        assert!(!env.take().iter().any(|l| l.starts_with("read")));
    }
}
