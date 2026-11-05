//! TMAP and GMAP (v1 le_audio_tmap_gmap.cpp; WP5).
//!
//! TMAP (Telephony and Media Audio Profile 1.0) and GMAP (Gaming Audio
//! Profile 1.0) are role profiles on top of BAP / CAP.
//!
//! - TMAS on the PC (0x1855): TMAP Role = Unicast Media Sender only. A2DPWB
//!   sends media to unicast sink ASEs, which is what UMS is. Not claimed:
//!   Call Gateway (needs a call control server and conversational,
//!   bidirectional audio: source ASEs, the headset's microphone), Call
//!   Terminal / Unicast Media Receiver / Broadcast Media Receiver (the PC
//!   never renders LE Audio), Broadcast Media Sender (no BIS).
//! - GMAS on the PC (0x1858): GMAP Role = Unicast Game Gateway, UGG
//!   Features = Multiplex (both channels of a stereo headset as two LC3
//!   frames in one SDU when the headset allows it) | 96 kbps Source
//!   (48_3_gr / 48_4_gr). Not Multisink: that is about receiving audio from
//!   the headsets (`le_audio::defs::PC_*`).
//! - The headset's TMAP Role, GMAP Role and UGT Features are read in
//!   `on_gatt_ready` and logged ("TMAP: UMR, GMAP: UGT (UGT features 0x3C:
//!   sink, ...)"). TMAP does not change the stream: the media presets
//!   already are what UMS / UMR need.
//! - `adjust_stream_request` (worker): with low latency on
//!   (`host.le_low_latency()`) and every device of the group a UGT with the
//!   Sink feature, the "_gr" presets go first with the Game context, or
//!   Media when a device has Media but not Game available ("GMAP: streaming
//!   48_3_gr, contexts 0x0008", "has no Game context available (0x0006):
//!   Media context instead"); plan_stream still takes only what the sink
//!   PAC records allow and falls back to the media presets behind them.
//!   Otherwise the request stays as it is (media presets, Media context).
//!
//! The state per device id is shared by the BTstack-thread module and the
//! worker hooks under a mutex.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use le_audio::defs::{
    CONTEXT_GAME, CONTEXT_MEDIA, GMAP_ROLE_UGT, PC_GMAP_ROLE, PC_TMAP_ROLE, PC_UGG_FEATURES, UGT_FEATURE_SINK,
    UUID_CHR_GMAP_ROLE, UUID_CHR_TMAP_ROLE, UUID_CHR_UGG_FEATURES, UUID_CHR_UGT_FEATURES, UUID_SERVICE_GMAS,
    UUID_SERVICE_TMAS,
};

use super::super::core::{LeCtx, LeDone};
use super::super::gatt_server::{ATT_PROPERTY_READ, ATT_SECURITY_NONE, AttRead, AttUuid, GattServerBuilder};
use super::super::module::{ATT_ERROR_READ_NOT_PERMITTED, LeGroupHooks, LeModule, LeModuleEntry, LeModuleSetup};
use super::super::{LeDeviceId, LeStreamRequest, LeStreamStatus};
use crate::BtHost;

/// GMAP presets for a UGT, best first: 48 kHz before 32 kHz, the 96 kbps
/// settings (UGG 96 kbps Source) before 80 kbps, 7.5 ms before 10 ms frames
/// at the same rate. plan_stream skips what the PAC records do not allow.
pub const GMAP_PRESET_ORDER: [&str; 6] = ["48_3_gr", "48_4_gr", "48_1_gr", "48_2_gr", "32_1_gr", "32_2_gr"];

/// How long adjust_stream_request waits for role reads still queued
const ROLE_READ_WAIT: Duration = Duration::from_millis(5000);
/// v1 Core::run's default
const RUN_TIMEOUT: Duration = Duration::from_secs(3);

/// The roles a connected device has (v1 TmapGmapInfo)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TmapGmapInfo {
    /// All reads finished
    pub done: bool,
    pub has_tmas: bool,
    /// Valid when has_tmas
    pub tmap_role: u16,
    pub has_gmas: bool,
    /// Valid when has_gmas
    pub gmap_role: u8,
    /// Valid with GMAP_ROLE_UGT
    pub ugt_features: u8,
}

impl TmapGmapInfo {
    /// A Unicast Game Terminal that renders audio (GMAP low latency
    /// applies)
    pub fn ugt_sink(&self) -> bool {
        self.has_gmas && self.gmap_role & GMAP_ROLE_UGT != 0 && self.ugt_features & UGT_FEATURE_SINK != 0
    }

    /// "TMAP: UMR, GMAP: UGT (UGT features 0x3C: sink, ...)" for the UI /
    /// log; "" while not done or without TMAS / GMAS (v1
    /// tmap_gmap_summary)
    pub fn summary(&self) -> String {
        if !self.done {
            return String::new();
        }
        let mut s = String::new();
        if self.has_tmas {
            s = format!("TMAP: {}", describe_tmap_role(self.tmap_role));
        }
        if self.has_gmas {
            if !s.is_empty() {
                s += ", ";
            }
            s += &format!("GMAP: {}", describe_gmap_role(self.gmap_role));
            if self.gmap_role & GMAP_ROLE_UGT != 0 {
                s += &format!(
                    " (UGT features 0x{:02X}: {})",
                    self.ugt_features,
                    describe_ugt_features(self.ugt_features)
                );
            }
        }
        s
    }
}

/// The set bits by name, joined with `sep`; unnamed bits as hex; "none"
fn bit_names(value: u32, names: &[&str], sep: &str) -> String {
    let mut parts: Vec<String> =
        names.iter().enumerate().filter(|(i, _)| value & (1 << i) != 0).map(|(_, n)| n.to_string()).collect();
    let rest = value & !((1u32 << names.len()) - 1);
    if rest != 0 {
        parts.push(format!("0x{rest:X}"));
    }
    if parts.is_empty() { "none".to_string() } else { parts.join(sep) }
}

/// "UMS", "CT+UMR", ...
pub fn describe_tmap_role(role: u16) -> String {
    bit_names(u32::from(role), &["CG", "CT", "UMS", "UMR", "BMS", "BMR"], "+")
}

/// "UGG", "UGT", ...
pub fn describe_gmap_role(role: u8) -> String {
    bit_names(u32::from(role), &["UGG", "UGT", "BGS", "BGR"], "+")
}

/// "sink, 64 kbps sink, multiplex, ..."
pub fn describe_ugt_features(features: u8) -> String {
    bit_names(
        u32::from(features),
        &["source", "80 kbps source", "sink", "64 kbps sink", "multiplex", "multisink", "multisource"],
        ", ",
    )
}

/// Why GMAP low latency cannot be used for this group (the first device
/// that is not a UGT with the Sink feature), None if it can. A `None` info:
/// the module does not know the device.
fn low_latency_refusal(devices: &[(LeDeviceId, Option<TmapGmapInfo>)]) -> Option<String> {
    devices.iter().find_map(|(id, info)| match info {
        Some(i) if i.ugt_sink() => None,
        Some(i) if i.has_gmas && i.gmap_role & GMAP_ROLE_UGT == 0 => {
            Some(format!("device {id} is not a Unicast Game Terminal (GMAP {})", describe_gmap_role(i.gmap_role)))
        }
        Some(i) if i.has_gmas => Some(format!("device {id} is a UGT without the Sink feature")),
        _ => Some(format!("device {id} has no GMAP role")),
    })
}

/// The GMAP presets first, then the request's own ones not among them
fn gmap_preset_order(current: &[String]) -> Vec<String> {
    let mut order: Vec<String> = GMAP_PRESET_ORDER.iter().map(|s| s.to_string()).collect();
    for name in current {
        if !order.contains(name) {
            order.push(name.clone());
        }
    }
    order
}

/// The Streaming Audio Context of a GMAP stream from each device's
/// (address, Available Sink Contexts): Game if every device has it
/// available (or reports nothing); a device that has Media available but
/// not Game gets Media, as close as it allows (plan_stream would otherwise
/// fall back to Unspecified for the whole group). Also the note about the
/// first device without Game.
fn game_context(devices: &[(String, u16)]) -> (u16, Option<String>) {
    let (mut game, mut media) = (true, true);
    let mut note = None;
    for (addr, available) in devices {
        if *available == 0 {
            continue;
        }
        if available & CONTEXT_GAME == 0 {
            game = false;
            note.get_or_insert_with(|| format!("{addr} has no Game context available (0x{available:04x})"));
        }
        if available & CONTEXT_MEDIA == 0 {
            media = false;
        }
    }
    let contexts = if !game && media { CONTEXT_MEDIA } else { CONTEXT_GAME };
    (contexts, note)
}

#[derive(Debug, Default)]
struct DeviceState {
    info: TmapGmapInfo,
    /// Reads queued
    pending: u32,
}

/// Shared by the module (BTstack thread) and the hooks (worker)
#[derive(Default)]
struct Shared {
    devices: HashMap<LeDeviceId, DeviceState>,
    /// adjust_stream_request waiting for the role reads of these devices
    /// (completed on the BTstack thread)
    waiters: Vec<(Vec<LeDeviceId>, LeDone)>,
}

impl Shared {
    /// No read of these devices is pending (a device the module does not
    /// know counts as finished)
    fn reads_done(&self, ids: &[LeDeviceId]) -> bool {
        ids.iter().all(|id| self.devices.get(id).is_none_or(|s| s.info.done))
    }
}

type SharedState = Arc<Mutex<Shared>>;

fn lock(shared: &SharedState) -> MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(|e| e.into_inner())
}

fn info_of(shared: &SharedState, id: LeDeviceId) -> Option<TmapGmapInfo> {
    lock(shared).devices.get(&id).map(|s| s.info)
}

/// BTstack thread: completes the waiters whose devices are all read
fn wake_waiters(shared: &SharedState) {
    let ready: Vec<LeDone> = {
        let mut s = lock(shared);
        let waiters = std::mem::take(&mut s.waiters);
        let (ready, waiting): (Vec<_>, Vec<_>) = waiters.into_iter().partition(|(ids, _)| s.reads_done(ids));
        s.waiters = waiting;
        ready.into_iter().map(|(_, done)| done).collect()
    };
    for done in ready {
        done(true);
    }
}

/// BTstack thread: done(true) once no read of these devices is pending
fn wait_reads(shared: &SharedState, ids: Vec<LeDeviceId>, done: LeDone) {
    {
        let mut s = lock(shared);
        if !s.reads_done(&ids) {
            s.waiters.push((ids, done));
            return;
        }
    }
    done(true);
}

/// BTstack thread: all reads of `id` finished -> log, wake the waiters
fn finish_if_done(ctx: &mut LeCtx<'_>, shared: &SharedState, id: LeDeviceId) {
    let summary = {
        let mut s = lock(shared);
        let Some(st) = s.devices.get_mut(&id) else { return };
        if st.pending > 0 || st.info.done {
            return;
        }
        st.info.done = true;
        st.info.summary()
    };
    if !summary.is_empty() {
        let addr = ctx.device(id).map_or_else(|| "?".to_string(), |d| d.addr.to_string());
        tracing::info!("LE Audio: {addr}: {summary}");
    }
    wake_waiters(shared);
}

/// A little-endian value of `size` bytes to read; `apply` stores it
struct RoleRead {
    handle: u16,
    size: usize,
    what: &'static str,
    apply: fn(&mut TmapGmapInfo, u32),
}

/// Queues the read
fn read_u(ctx: &mut LeCtx<'_>, shared: &SharedState, id: LeDeviceId, con: u16, r: RoleRead) {
    lock(shared).devices.entry(id).or_default().pending += 1;
    let shared = shared.clone();
    ctx.gatt().read(
        con,
        r.handle,
        Box::new(move |ctx, status, value| {
            // Link gone: on_disconnected dropped the state
            if ctx.device_on(id, con).is_none() {
                return;
            }
            {
                let mut s = lock(&shared);
                let Some(st) = s.devices.get_mut(&id) else { return };
                tracing::debug!(
                    "LE Audio: device {}: {} 0x{:04X} (0x{status:02X}): {value:02X?}",
                    id.0,
                    r.what,
                    r.handle
                );
                if status == 0 && value.len() >= r.size {
                    let x = value[..r.size].iter().rev().fold(0u32, |x, b| (x << 8) | u32::from(*b));
                    (r.apply)(&mut st.info, x);
                } else {
                    tracing::warn!("LE Audio: {} read failed (status 0x{status:02x}, {} bytes)", r.what, value.len());
                }
                st.pending = st.pending.saturating_sub(1);
            }
            finish_if_done(ctx, &shared, id);
        }),
    );
}

/// BTstack-thread part
pub struct TmapGmapModule {
    shared: SharedState,
    tmap_role_handle: u16,
    gmap_role_handle: u16,
    ugg_features_handle: u16,
}

impl TmapGmapModule {
    /// What was read of a device; None while unknown (not connected / not
    /// read yet)
    pub fn info(&self, device: LeDeviceId) -> Option<TmapGmapInfo> {
        info_of(&self.shared, device)
    }
}

impl LeModule for TmapGmapModule {
    fn name(&self) -> &'static str {
        "TMAP/GMAP"
    }

    fn add_gatt_services(&mut self, server: &mut GattServerBuilder<'_>) {
        // TMAS: TMAP Role; GMAS: GMAP Role + UGG Features. Read only, no
        // security beyond the link's.
        let read_only = |server: &mut GattServerBuilder<'_>, uuid16| {
            server.add_characteristic(AttUuid::Uuid16(uuid16), ATT_PROPERTY_READ, ATT_SECURITY_NONE, ATT_SECURITY_NONE)
        };
        server.begin_service(AttUuid::Uuid16(UUID_SERVICE_TMAS), true);
        self.tmap_role_handle = read_only(server, UUID_CHR_TMAP_ROLE);
        server.end_service();
        server.begin_service(AttUuid::Uuid16(UUID_SERVICE_GMAS), true);
        self.gmap_role_handle = read_only(server, UUID_CHR_GMAP_ROLE);
        self.ugg_features_handle = read_only(server, UUID_CHR_UGG_FEATURES);
        server.end_service();
        tracing::info!(
            "LE Audio: TMAS: TMAP Role {}; GMAS: GMAP Role {}, UGG Features 0x{:02x}",
            describe_tmap_role(PC_TMAP_ROLE),
            describe_gmap_role(PC_GMAP_ROLE),
            PC_UGG_FEATURES
        );
    }

    fn on_stack_deinit(&mut self) {
        let mut s = lock(&self.shared);
        s.devices.clear();
        s.waiters.clear();
    }

    fn on_gatt_ready(&mut self, ctx: &mut LeCtx<'_>, id: LeDeviceId) {
        let Some(dev) = ctx.device(id) else { return };
        let (con, addr) = (dev.acl, dev.addr);
        let tmap = dev.find_char(UUID_SERVICE_TMAS, UUID_CHR_TMAP_ROLE).map(|c| c.value_handle);
        let gmap = dev.find_char(UUID_SERVICE_GMAS, UUID_CHR_GMAP_ROLE).map(|c| c.value_handle);
        let ugt = gmap.and(dev.find_char(UUID_SERVICE_GMAS, UUID_CHR_UGT_FEATURES).map(|c| c.value_handle));
        lock(&self.shared).devices.insert(id, DeviceState::default());
        tracing::debug!(
            "LE Audio: {addr}: TMAP Role {:04X?}, GMAP Role {:04X?}, UGT Features {:04X?}",
            tmap,
            gmap,
            ugt
        );
        if let Some(handle) = tmap {
            let apply = |i: &mut TmapGmapInfo, v: u32| {
                i.has_tmas = true;
                i.tmap_role = v as u16;
            };
            read_u(ctx, &self.shared, id, con, RoleRead { handle, size: 2, what: "TMAP Role", apply });
        }
        if let Some(handle) = gmap {
            let apply = |i: &mut TmapGmapInfo, v: u32| {
                i.has_gmas = true;
                i.gmap_role = v as u8;
            };
            read_u(ctx, &self.shared, id, con, RoleRead { handle, size: 1, what: "GMAP Role", apply });
        }
        if let Some(handle) = ugt {
            let apply = |i: &mut TmapGmapInfo, v: u32| i.ugt_features = v as u8;
            read_u(ctx, &self.shared, id, con, RoleRead { handle, size: 1, what: "UGT Features", apply });
        }
        if tmap.is_none() && gmap.is_none() {
            tracing::info!("LE Audio: {addr}: no TMAS / GMAS (neither TMAP nor GMAP)");
        }
        finish_if_done(ctx, &self.shared, id);
    }

    fn on_disconnected(&mut self, _ctx: &mut LeCtx<'_>, id: LeDeviceId, _reason: u8) {
        lock(&self.shared).devices.remove(&id);
        wake_waiters(&self.shared);
    }

    fn on_stream_state(&mut self, _ctx: &mut LeCtx<'_>, status: &LeStreamStatus, streaming: bool) {
        let plan = &status.plan;
        if streaming && plan.preset.name.contains("_gr") {
            tracing::info!(
                "LE Audio: GMAP: streaming {}, contexts 0x{:04x}, presentation delay {} us",
                plan.preset.name,
                plan.contexts,
                plan.presentation_delay_us
            );
        }
    }

    fn on_att_read(&mut self, _ctx: &mut LeCtx<'_>, read: &AttRead) -> Result<Vec<u8>, u8> {
        tracing::debug!("LE Audio: TMAS / GMAS read 0x{:04X} by 0x{:04X}", read.handle, read.con);
        match read.handle {
            h if h == self.tmap_role_handle => Ok(PC_TMAP_ROLE.to_le_bytes().to_vec()),
            h if h == self.gmap_role_handle => Ok(vec![PC_GMAP_ROLE]),
            h if h == self.ugg_features_handle => Ok(vec![PC_UGG_FEATURES]),
            _ => Err(ATT_ERROR_READ_NOT_PERMITTED),
        }
    }
}

/// Worker part (adjust_stream_request)
pub struct TmapGmapHooks {
    shared: SharedState,
}

impl LeGroupHooks for TmapGmapHooks {
    fn name(&self) -> &'static str {
        "TMAP/GMAP"
    }

    fn adjust_stream_request(&self, host: &BtHost, devices: &[LeDeviceId], request: &mut LeStreamRequest) {
        if !host.le_low_latency() {
            tracing::info!("LE Audio: GMAP: low latency off, media configuration");
            return;
        }
        // The role reads were queued in on_gatt_ready and may still run:
        // wait for them (the BTstack thread re-checks when a read finishes)
        let (shared, ids) = (self.shared.clone(), devices.to_vec());
        if !host.le_call(ROLE_READ_WAIT, move |_ctx, done| wait_reads(&shared, ids, done)) {
            tracing::warn!("LE Audio: GMAP: role reads not finished, deciding with what is known");
        }

        let infos: Vec<_> = devices.iter().map(|&id| (id, info_of(&self.shared, id))).collect();
        tracing::debug!("LE Audio: GMAP: device roles {infos:?}");
        if let Some(why) = low_latency_refusal(&infos) {
            tracing::info!("LE Audio: GMAP: low latency not used: {why}; media configuration");
            return;
        }
        request.preset_order = gmap_preset_order(&request.preset_order);

        let ids = devices.to_vec();
        let available = host
            .le_run(RUN_TIMEOUT, move |ctx| {
                ids.iter()
                    .filter_map(|&id| ctx.device(id).map(|d| (d.addr.to_string(), d.available_sink_contexts)))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let (contexts, note) = game_context(&available);
        tracing::debug!("LE Audio: GMAP: available sink contexts {available:?} -> 0x{contexts:04X}");
        if let Some(note) = note {
            let what = if contexts == CONTEXT_MEDIA { "Media context instead" } else { "left to the stream planning" };
            tracing::info!("LE Audio: GMAP: {note}: {what}");
        }
        request.contexts = contexts;
        tracing::info!(
            "LE Audio: GMAP: low latency for {} Unicast Game Terminal(s): {} first, {} context",
            devices.len(),
            GMAP_PRESET_ORDER.join(", "),
            if contexts == CONTEXT_GAME { "Game" } else { "Media" }
        );
    }
}

pub fn create(_setup: &mut LeModuleSetup) -> LeModuleEntry {
    let shared = SharedState::default();
    LeModuleEntry {
        module: Box::new(TmapGmapModule {
            shared: shared.clone(),
            tmap_role_handle: 0,
            gmap_role_handle: 0,
            ugg_features_handle: 0,
        }),
        hooks: Some(Arc::new(TmapGmapHooks { shared })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use le_audio::defs::{
        CONTEXT_CONVERSATIONAL, GMAP_ROLE_BGR, GMAP_ROLE_UGG, TMAP_ROLE_CT, TMAP_ROLE_UMR, TMAP_ROLE_UMS,
        UGT_FEATURE_64KBPS_SINK, UGT_FEATURE_MULTIPLEX, UGT_FEATURE_MULTISINK,
    };
    use std::sync::atomic::{AtomicBool, Ordering};

    fn ugt(features: u8) -> TmapGmapInfo {
        TmapGmapInfo {
            done: true,
            has_tmas: true,
            tmap_role: TMAP_ROLE_UMR,
            has_gmas: true,
            gmap_role: GMAP_ROLE_UGT,
            ugt_features: features,
        }
    }

    #[test]
    fn roles_are_described_as_v1() {
        assert_eq!(describe_tmap_role(TMAP_ROLE_UMS), "UMS");
        assert_eq!(describe_tmap_role(TMAP_ROLE_CT | TMAP_ROLE_UMR), "CT+UMR");
        assert_eq!(describe_tmap_role(0), "none");
        assert_eq!(describe_tmap_role(0x0108), "UMR+0x100");
        assert_eq!(describe_gmap_role(GMAP_ROLE_UGG), "UGG");
        assert_eq!(describe_gmap_role(GMAP_ROLE_BGR), "BGR");
        assert_eq!(describe_gmap_role(0x30), "0x30");
        assert_eq!(describe_ugt_features(0x3C), "sink, 64 kbps sink, multiplex, multisink");
        assert_eq!(describe_ugt_features(0x81), "source, 0x80");
    }

    #[test]
    fn summary_as_v1() {
        let i = ugt(UGT_FEATURE_SINK | UGT_FEATURE_64KBPS_SINK | UGT_FEATURE_MULTIPLEX | UGT_FEATURE_MULTISINK);
        assert_eq!(i.summary(), "TMAP: UMR, GMAP: UGT (UGT features 0x3C: sink, 64 kbps sink, multiplex, multisink)");
        let bgr = TmapGmapInfo { gmap_role: GMAP_ROLE_BGR, ugt_features: 0, ..i };
        assert_eq!(bgr.summary(), "TMAP: UMR, GMAP: BGR");
        let tmap_only = TmapGmapInfo { has_gmas: false, ..i };
        assert_eq!(tmap_only.summary(), "TMAP: UMR");
        assert_eq!(TmapGmapInfo { done: true, ..TmapGmapInfo::default() }.summary(), "");
        assert_eq!(TmapGmapInfo { done: false, ..i }.summary(), "");
    }

    #[test]
    fn ugt_sink_needs_gmas_ugt_and_sink() {
        assert!(ugt(UGT_FEATURE_SINK).ugt_sink());
        assert!(!ugt(UGT_FEATURE_MULTIPLEX).ugt_sink());
        assert!(!TmapGmapInfo { gmap_role: GMAP_ROLE_BGR, ..ugt(UGT_FEATURE_SINK) }.ugt_sink());
        assert!(!TmapGmapInfo { has_gmas: false, ..ugt(UGT_FEATURE_SINK) }.ugt_sink());
    }

    #[test]
    fn low_latency_refusal_names_the_first_device() {
        let (a, b) = (LeDeviceId(0), LeDeviceId(1));
        assert_eq!(low_latency_refusal(&[(a, Some(ugt(0x3C))), (b, Some(ugt(0x3C)))]), None);
        assert_eq!(low_latency_refusal(&[]), None);
        assert_eq!(
            low_latency_refusal(&[(a, Some(ugt(0x3C))), (b, None)]).as_deref(),
            Some("device 1 has no GMAP role")
        );
        let no_gmas = TmapGmapInfo { has_gmas: false, ..ugt(0x3C) };
        assert_eq!(low_latency_refusal(&[(a, Some(no_gmas))]).as_deref(), Some("device 0 has no GMAP role"));
        let bgr = TmapGmapInfo { gmap_role: GMAP_ROLE_BGR, ..ugt(0) };
        assert_eq!(
            low_latency_refusal(&[(a, Some(bgr))]).as_deref(),
            Some("device 0 is not a Unicast Game Terminal (GMAP BGR)")
        );
        assert_eq!(
            low_latency_refusal(&[(b, Some(ugt(UGT_FEATURE_MULTIPLEX)))]).as_deref(),
            Some("device 1 is a UGT without the Sink feature")
        );
    }

    #[test]
    fn gmap_presets_go_first() {
        let current: Vec<String> = ["48_6_2", "48_4_gr", "16_2_2"].iter().map(|s| s.to_string()).collect();
        assert_eq!(
            gmap_preset_order(&current),
            ["48_3_gr", "48_4_gr", "48_1_gr", "48_2_gr", "32_1_gr", "32_2_gr", "48_6_2", "16_2_2"]
        );
        // Every GMAP preset exists in the built-in table
        let table = le_audio::presets::PresetTable::new();
        assert!(GMAP_PRESET_ORDER.iter().all(|n| table.find(n).is_some()));
    }

    #[test]
    fn game_context_falls_back_to_media() {
        let d = |a: &str, c: u16| (a.to_string(), c);
        assert_eq!(game_context(&[]), (CONTEXT_GAME, None));
        assert_eq!(game_context(&[d("A", CONTEXT_GAME | CONTEXT_MEDIA)]), (CONTEXT_GAME, None));
        // Reports nothing: Game
        assert_eq!(game_context(&[d("A", 0)]), (CONTEXT_GAME, None));
        assert_eq!(
            game_context(&[
                d("A", CONTEXT_GAME | CONTEXT_MEDIA),
                d("11:22:33:44:55:66", CONTEXT_MEDIA | CONTEXT_CONVERSATIONAL)
            ]),
            (CONTEXT_MEDIA, Some("11:22:33:44:55:66 has no Game context available (0x0006)".to_string()))
        );
        // A device with neither Game nor Media: left to plan_stream
        let (contexts, note) = game_context(&[d("A", CONTEXT_CONVERSATIONAL), d("B", CONTEXT_MEDIA)]);
        assert_eq!(contexts, CONTEXT_GAME);
        assert_eq!(note.as_deref(), Some("A has no Game context available (0x0002)"));
    }

    #[test]
    fn waiters_wake_when_reads_finish_or_devices_go() {
        let shared = SharedState::default();
        let (a, b) = (LeDeviceId(0), LeDeviceId(1));
        {
            let mut s = lock(&shared);
            s.devices.insert(a, DeviceState { pending: 1, ..DeviceState::default() });
            s.devices.insert(b, DeviceState { pending: 1, ..DeviceState::default() });
        }
        let woke = Arc::new(AtomicBool::new(false));
        let w = woke.clone();
        wait_reads(&shared, vec![a, b], Box::new(move |ok| w.store(ok, Ordering::SeqCst)));
        assert!(!woke.load(Ordering::SeqCst));
        lock(&shared).devices.get_mut(&a).unwrap().info.done = true;
        wake_waiters(&shared);
        assert!(!woke.load(Ordering::SeqCst));
        // b disconnected: an unknown device counts as finished
        lock(&shared).devices.remove(&b);
        wake_waiters(&shared);
        assert!(woke.load(Ordering::SeqCst));
        assert!(lock(&shared).waiters.is_empty());

        // Nothing pending: done at once
        let now = Arc::new(AtomicBool::new(false));
        let n = now.clone();
        wait_reads(&shared, vec![a, LeDeviceId(7)], Box::new(move |ok| n.store(ok, Ordering::SeqCst)));
        assert!(now.load(Ordering::SeqCst));
    }

    #[test]
    fn pc_server_values() {
        let shared = SharedState::default();
        let m = TmapGmapModule { shared, tmap_role_handle: 10, gmap_role_handle: 13, ugg_features_handle: 15 };
        // The values the sink checks: TMAP Role 0x0004 (UMS), GMAP Role 0x01
        // (UGG), UGG Features 0x03
        assert_eq!(PC_TMAP_ROLE.to_le_bytes(), [0x04, 0x00]);
        assert_eq!((PC_GMAP_ROLE, PC_UGG_FEATURES), (0x01, 0x03));
        assert_eq!(m.info(LeDeviceId(0)), None);
    }
}
