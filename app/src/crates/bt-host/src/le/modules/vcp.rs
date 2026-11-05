//! VCP Volume Controller (v1 le_audio_vcp.cpp).
//!
//! The headset volume follows the Windows volume and the other way round,
//! as AVRCP absolute volume does for A2DP: core's LE session runs
//! VolumeSync against `BtHost::le_remote_volume` / `le_request_volume`,
//! which this module answers ([`VcpHooks`]).
//!
//! Per device with a Volume Control Service (VCS, 0x1844), `on_gatt_ready`:
//! notifications of Volume State (0x2B7D: Volume_Setting 0-255, Mute,
//! Change Counter) on, Volume State and Volume Flags (0x2B7F) read; the
//! Volume Control Point (0x2B7E) is written with the last known Change
//! Counter. An "Invalid Change Counter" (ATT 0x80) answer re-reads Volume
//! State and sends the operation once more. VOCS / AICS (included services)
//! are only logged.
//!
//! The group: every connected device with VCS (A2DPWB connects only the
//! devices it streams to, e.g. both earbuds of a set) shares one volume,
//! the "group volume" (Volume_Setting 0-255):
//! - It starts as the volume of the first device whose Volume State is read
//!   (the `-d` device). Devices read later (the other earbud) are set to
//!   it. VolumeSync then makes Windows take this volume over, as it does
//!   with the AVRCP volume on an A2DP connect: the headset's volume is kept.
//! - `request_volume` (VolumeSync: the Windows volume changed) sets it and
//!   writes Set Absolute Volume to every device; a muted device is unmuted
//!   too when the level is above 0 (the user turned the volume up).
//! - A device that reports another volume by notification while A2DPWB has
//!   no write of its own pending (the earbud's buttons) sets it: the other
//!   members get it written, and `remote_volume` reports it so VolumeSync
//!   moves the Windows volume.
//! - `remote_volume` is the group volume mapped to 0-127, None while no VCS
//!   device is known. Mute is not mapped (VolumeSync syncs the level only).
//!
//! Requests are coalesced per device: one Control Point write at a time,
//! followed by a read of Volume State; a newer level waits for it.
//!
//! The volume logic is [`Group`] (no BTstack: it returns the GATT
//! operations to run, unit-tested below); [`VcpModule`] runs them with the
//! GATT client queue. The scale and values are `le_audio::vcp_scale`.
//!
//! Bumble side: tools/emu/le_audio/vcp.py (bumble.profiles.vcs on the sink).

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, AtomicUsize, Ordering};

use le_audio::defs::{UUID_SERVICE_AICS, UUID_SERVICE_VCS, UUID_SERVICE_VOCS};
use le_audio::vcp_scale::{
    ATT_ERROR_INVALID_CHANGE_COUNTER, ATT_ERROR_OPCODE_NOT_SUPPORTED, OP_UNMUTE, UUID_VOLUME_CONTROL_POINT,
    UUID_VOLUME_FLAGS, UUID_VOLUME_STATE, cp_set_absolute_volume, cp_simple, to_level, to_setting,
};
use tracing::{info, warn};

use super::super::LeDeviceId;
use super::super::core::{LeCtx, LePoster};
use super::super::gatt_client::{ATT_ERROR_HCI_DISCONNECT_RECEIVED, PROPERTY_NOTIFY};
use super::super::module::{LeGroupHooks, LeModule, LeModuleEntry, LeModuleSetup};

fn att_error_text(status: u8) -> &'static str {
    match status {
        ATT_ERROR_INVALID_CHANGE_COUNTER => "Invalid Change Counter",
        ATT_ERROR_OPCODE_NOT_SUPPORTED => "Opcode Not Supported",
        ATT_ERROR_HCI_DISCONNECT_RECEIVED => "disconnected",
        _ => "error",
    }
}

/// What the module shares with its any-thread hooks
struct Shared {
    /// The group volume as 0-127, -1 unknown
    level: AtomicI32,
    /// Devices with VCS
    count: AtomicUsize,
}

impl Shared {
    fn new() -> Shared {
        Shared { level: AtomicI32::new(-1), count: AtomicUsize::new(0) }
    }
}

/// The Control Point operation of a device that runs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sent {
    SetAbsoluteVolume(u8),
    Unmute,
}

impl Sent {
    fn name(self) -> String {
        match self {
            Sent::SetAbsoluteVolume(setting) => format!("Set Absolute Volume {setting}"),
            Sent::Unmute => "Unmute".to_string(),
        }
    }
}

/// One device with VCS (BTstack thread)
#[derive(Debug, Clone)]
struct VcsDevice {
    con: u16,
    addr: String,
    state_handle: u16,
    cp_handle: u16,

    /// Volume State read
    known: bool,
    /// Volume_Setting 0-255
    setting: u8,
    mute: bool,
    /// Change_Counter
    counter: u8,

    /// A Control Point write (and the read after it) runs: this one
    sent: Option<Sent>,
    /// Volume_Setting still to write
    want: Option<u8>,
    want_unmute: bool,
    /// The current operation was sent again after 0x80
    retried: bool,
    /// While busy: the settings the operation may leave (before / after
    /// it). A Volume State seen meanwhile with any other setting was changed
    /// on the device (its buttons) and is applied once the operation is
    /// done.
    op_from: u8,
    op_to: u8,
    device_changed: bool,
}

impl VcsDevice {
    fn new(con: u16, addr: String, state_handle: u16, cp_handle: u16) -> VcsDevice {
        VcsDevice {
            con,
            addr,
            state_handle,
            cp_handle,
            known: false,
            setting: 0,
            mute: false,
            counter: 0,
            sent: None,
            want: None,
            want_unmute: false,
            retried: false,
            op_from: 0,
            op_to: 0,
            device_changed: false,
        }
    }

    fn busy(&self) -> bool {
        self.sent.is_some()
    }
}

/// A GATT operation [`Group`] wants run
#[derive(Debug, Clone, PartialEq, Eq)]
enum GattOp {
    /// Control Point write; its completion goes to [`Group::write_done`]
    Write { id: LeDeviceId, con: u16, handle: u16, value: Vec<u8> },
    /// Volume State read after a write (`write_status`); its completion
    /// goes to [`Group::read_done`]
    ReadAfterWrite { id: LeDeviceId, con: u16, handle: u16, write_status: u8 },
}

/// The group volume and the VCS devices (BTstack thread, no BTstack calls)
struct Group {
    /// By device id (v1: std::map, the order of writes to the members)
    devices: BTreeMap<LeDeviceId, VcsDevice>,
    /// Group Volume_Setting, None unknown
    target: Option<u8>,
    shared: Arc<Shared>,
}

impl Group {
    fn new(shared: Arc<Shared>) -> Group {
        Group { devices: BTreeMap::new(), target: None, shared }
    }

    fn publish(&self) {
        self.shared.level.store(self.target.map_or(-1, |t| to_level(t) as i32), Ordering::Release);
    }

    fn publish_count(&self) {
        self.shared.count.store(self.devices.len(), Ordering::Release);
    }

    fn addr(&self, id: LeDeviceId) -> Option<&str> {
        self.devices.get(&id).map(|v| v.addr.as_str())
    }

    /// The device, if it is still on the link `con`
    fn find(&mut self, id: LeDeviceId, con: u16) -> Option<&mut VcsDevice> {
        self.devices.get_mut(&id).filter(|v| v.con == con)
    }

    /// A device with VCS (again: its old state is dropped)
    fn add(&mut self, id: LeDeviceId, device: VcsDevice) {
        self.devices.insert(id, device);
        self.publish_count();
    }

    fn remove(&mut self, id: LeDeviceId) {
        if self.devices.remove(&id).is_none() {
            return;
        }
        self.publish_count();
        if self.devices.is_empty() {
            // Unknown until a device is read again: VolumeSync starts over
            self.target = None;
            self.publish();
        }
    }

    fn clear(&mut self) {
        self.devices.clear();
        self.publish_count();
        self.target = None;
        self.publish();
    }

    /// Volume State read (`from_notification` false) or notified
    fn on_state(&mut self, id: LeDeviceId, con: u16, value: &[u8], from_notification: bool, out: &mut Vec<GattOp>) {
        let Some(v) = self.devices.get_mut(&id).filter(|v| v.con == con) else { return };
        tracing::debug!(
            "VCP: {}: Volume State {value:02X?} ({}), busy {}, want {:?}",
            v.addr,
            if from_notification { "notified" } else { "read" },
            v.busy(),
            v.want
        );
        let [setting, mute, counter, ..] = *value else {
            warn!("VCP: {}: malformed Volume State ({} bytes)", v.addr, value.len());
            return;
        };
        let mute = mute != 0;
        let first = !v.known;
        let changed = first || setting != v.setting || mute != v.mute;
        let old_mute = v.mute;
        v.known = true;
        v.setting = setting;
        v.mute = mute;
        v.counter = counter;
        if changed {
            info!(
                "VCP: {}: Volume State {}/255 ({}/127){}, change counter {}{}",
                v.addr,
                v.setting,
                to_level(v.setting),
                if v.mute { ", muted" } else { "" },
                v.counter,
                if from_notification { " (notified)" } else { "" }
            );
        }

        if v.busy() && !first && v.setting != v.op_from && v.setting != v.op_to && !v.device_changed {
            // Not what A2DPWB's write leaves: the device changed meanwhile;
            // applied when the write and its read are done (finish)
            v.device_changed = true;
            info!("VCP: {}: volume changed on the device during a write of A2DPWB", v.addr);
        }

        if first {
            match self.target {
                None => {
                    // The first device decides: Windows takes its volume over
                    info!("VCP: group volume {}/255 ({}/127) from {}", setting, to_level(setting), v.addr);
                    self.target = Some(setting);
                    self.publish();
                }
                Some(target) if setting != target && v.want.is_none() => {
                    info!("VCP: {}: setting it to the group volume {}/255", v.addr, target);
                    v.want = Some(target);
                }
                Some(_) => {}
            }
            self.sync(id, out);
            return;
        }
        // A2DPWB's own write runs or waits: its notifications are not
        // followed (no echo); a change of the device meanwhile is flagged
        // above and applied by finish()
        if !from_notification || v.busy() || v.want.is_some() || v.want_unmute {
            return;
        }
        if v.mute != old_mute {
            info!("VCP: {} {} on the device", v.addr, if v.mute { "muted" } else { "unmuted" });
        }
        self.follow_device(id, out);
    }

    /// Changed on the device (buttons): the group and Windows follow
    fn follow_device(&mut self, id: LeDeviceId, out: &mut Vec<GattOp>) {
        let Some(v) = self.devices.get(&id) else { return };
        let setting = v.setting;
        if Some(setting) == self.target {
            return;
        }
        info!("VCP: {}: volume changed on the device to {}/255 ({}/127)", v.addr, setting, to_level(setting));
        self.target = Some(setting);
        self.publish();
        let others: Vec<LeDeviceId> = self.devices.keys().copied().filter(|&o| o != id).collect();
        for o in others {
            let Some(d) = self.devices.get_mut(&o) else { continue };
            if d.known && d.setting == setting && d.want.is_none() {
                continue;
            }
            d.want = Some(setting);
            self.sync(o, out);
        }
    }

    /// request_volume(): the Windows volume changed
    fn set_group_volume(&mut self, setting: u8, out: &mut Vec<GattOp>) {
        if self.target.is_some_and(|t| to_level(t) == to_level(setting)) {
            tracing::debug!("VCP: volume {}/127 is the group level already", to_level(setting));
            self.publish();
            return; // the same level: keep the devices' exact setting
        }
        self.target = Some(setting);
        self.publish();
        info!("VCP: setting volume {}/127 ({}/255) on {} device(s)", to_level(setting), setting, self.devices.len());
        let ids: Vec<LeDeviceId> = self.devices.keys().copied().collect();
        for id in ids {
            if let Some(d) = self.devices.get_mut(&id) {
                d.want = Some(setting);
                d.want_unmute = setting > 0;
            }
            self.sync(id, out);
        }
    }

    /// Write what is still wanted, one Control Point operation at a time
    fn sync(&mut self, id: LeDeviceId, out: &mut Vec<GattOp>) {
        let Some(v) = self.devices.get_mut(&id) else { return };
        if v.busy() || !v.known {
            tracing::debug!("VCP: {}: sync deferred (busy {}, state known {})", v.addr, v.busy(), v.known);
            return;
        }
        let sent = match v.want {
            Some(want) if want != v.setting => Some(Sent::SetAbsoluteVolume(want)),
            _ if v.want_unmute && v.mute => Some(Sent::Unmute),
            _ => None,
        };
        v.want = None;
        let Some(sent) = sent else {
            v.want_unmute = false;
            v.retried = false;
            return;
        };
        let value = match sent {
            Sent::SetAbsoluteVolume(setting) => cp_set_absolute_volume(v.counter, setting).to_vec(),
            Sent::Unmute => {
                v.want_unmute = false;
                cp_simple(OP_UNMUTE, v.counter).to_vec()
            }
        };
        v.sent = Some(sent);
        v.op_from = v.setting;
        v.op_to = match sent {
            Sent::SetAbsoluteVolume(setting) => setting,
            Sent::Unmute => v.setting,
        };
        v.device_changed = false;
        tracing::debug!("VCP: {}: {} -> control point 0x{:04X} {value:02X?}", v.addr, sent.name(), v.cp_handle);
        out.push(GattOp::Write { id, con: v.con, handle: v.cp_handle, value });
    }

    /// The Control Point write completed: read the state it left (also the
    /// Change Counter)
    fn write_done(&mut self, id: LeDeviceId, con: u16, status: u8, out: &mut Vec<GattOp>) {
        let Some(v) = self.find(id, con) else { return };
        tracing::debug!("VCP: {}: control point write done (ATT 0x{status:02X})", v.addr);
        if status == ATT_ERROR_HCI_DISCONNECT_RECEIVED {
            v.sent = None; // the link is going away; on_disconnected drops it
            return;
        }
        let Some(sent) = v.sent else { return };
        if status == ATT_ERROR_INVALID_CHANGE_COUNTER {
            warn!("VCP: {}: {}: Invalid Change Counter (0x80), reading Volume State again", v.addr, sent.name());
        }
        out.push(GattOp::ReadAfterWrite { id, con, handle: v.state_handle, write_status: status });
    }

    /// The read after a write completed (`read_status`, `value`)
    fn read_done(
        &mut self,
        id: LeDeviceId,
        con: u16,
        write_status: u8,
        read_status: u8,
        value: &[u8],
        out: &mut Vec<GattOp>,
    ) {
        if self.find(id, con).is_none() {
            return;
        }
        if read_status != 0 {
            tracing::debug!("VCP: device {}: Volume State read after the write failed (ATT 0x{read_status:02X})", id.0);
        }
        if read_status == 0 {
            self.on_state(id, con, value, false, out);
        }
        let Some(v) = self.find(id, con) else { return };
        let sent = v.sent.take();
        if read_status == ATT_ERROR_HCI_DISCONNECT_RECEIVED {
            return;
        }
        if let Some(sent) = sent {
            self.finish(id, write_status, sent, out);
        }
    }

    /// A Control Point operation and the read after it are done: retry it,
    /// or follow a change the device made meanwhile, then write what waits
    fn finish(&mut self, id: LeDeviceId, status: u8, sent: Sent, out: &mut Vec<GattOp>) {
        let target = self.target;
        let Some(v) = self.devices.get_mut(&id) else { return };
        let device = v.device_changed;
        v.device_changed = false;
        let what = sent.name();
        let mut retry = false;
        if status == 0 {
            info!("VCP: {}: {} done", v.addr, what);
            v.retried = false;
        } else if status == ATT_ERROR_INVALID_CHANGE_COUNTER && device {
            // The counter moved because the volume was changed on the
            // device: that change is newer than the request, do not
            // overwrite it
            info!("VCP: {}: {} not retried: the volume was changed on the device meanwhile", v.addr, what);
            v.retried = false;
        } else if status == ATT_ERROR_INVALID_CHANGE_COUNTER && !v.retried {
            info!("VCP: {}: {}: retrying with change counter {}", v.addr, what, v.counter);
            v.retried = true;
            retry = true;
        } else {
            warn!("VCP: {}: {} failed: {} (0x{:02x})", v.addr, what, att_error_text(status), status);
            v.retried = false;
        }
        if retry {
            // Send it again unless a newer request replaced it
            match sent {
                Sent::SetAbsoluteVolume(setting) => {
                    if v.want.is_none() {
                        v.want = Some(setting);
                    }
                }
                Sent::Unmute => v.want_unmute = true,
            }
        } else if device && v.want.is_none() && v.known {
            // No newer request from Windows: the device's change wins
            self.follow_device(id, out);
        } else if status != 0 && v.want.is_none() && v.known && Some(v.setting) != target {
            // Given up: report what the device really plays at
            self.target = Some(v.setting);
            self.publish();
        }
        self.sync(id, out);
    }
}

/// BTstack-thread part
pub struct VcpModule {
    group: Group,
}

impl VcpModule {
    /// Runs `f` on the module's group (if the device `check` is still on
    /// its link), then queues the GATT operations it asked for. For GATT,
    /// timer and poster callbacks (not the module's own hooks).
    fn with_group(ctx: &mut LeCtx<'_>, check: Option<(LeDeviceId, u16)>, f: impl FnOnce(&mut Group, &mut Vec<GattOp>)) {
        if let Some((id, con)) = check
            && ctx.device_on(id, con).is_none()
        {
            return;
        }
        let mut out = Vec::new();
        let Some(m) = ctx.module_mut::<VcpModule>() else { return };
        f(&mut m.group, &mut out);
        run(ctx, out);
    }
}

/// Queues the operations of [`Group`] with the GATT client
fn run(ctx: &mut LeCtx<'_>, ops: Vec<GattOp>) {
    for op in ops {
        match op {
            GattOp::Write { id, con, handle, value } => {
                let done = Box::new(move |ctx: &mut LeCtx<'_>, status: u8| {
                    VcpModule::with_group(ctx, Some((id, con)), |g, out| g.write_done(id, con, status, out));
                });
                ctx.gatt().write(con, handle, value, Some(done));
            }
            GattOp::ReadAfterWrite { id, con, handle, write_status } => {
                let done = Box::new(move |ctx: &mut LeCtx<'_>, status: u8, value: &[u8]| {
                    VcpModule::with_group(ctx, Some((id, con)), |g, out| {
                        g.read_done(id, con, write_status, status, value, out)
                    });
                });
                ctx.gatt().read(con, handle, done);
            }
        }
    }
}

/// The address of a VCS device for a callback's log line ("?" if gone)
fn addr_of(ctx: &mut LeCtx<'_>, id: LeDeviceId) -> Option<String> {
    ctx.module_mut::<VcpModule>().and_then(|m| m.group.addr(id).map(str::to_string))
}

fn log_flags(ctx: &mut LeCtx<'_>, id: LeDeviceId, value: &[u8]) {
    let Some(&flags) = value.first() else { return };
    let Some(addr) = addr_of(ctx, id) else { return };
    info!(
        "VCP: {}: Volume Flags 0x{:02x} ({})",
        addr,
        flags,
        if flags & 0x01 != 0 { "volume set by the user" } else { "default volume" }
    );
}

impl LeModule for VcpModule {
    fn name(&self) -> &'static str {
        "VCP"
    }

    fn on_stack_deinit(&mut self) {
        self.group.clear();
    }

    fn on_gatt_ready(&mut self, ctx: &mut LeCtx<'_>, dev: LeDeviceId) {
        let Some(d) = ctx.device(dev) else { return };
        let Some(svc) = d.find_service(UUID_SERVICE_VCS) else { return };
        let addr = d.addr.to_string();
        for s in &d.services {
            if s.uuid16 != UUID_SERVICE_VOCS && s.uuid16 != UUID_SERVICE_AICS {
                continue;
            }
            info!(
                "VCP: {}: {} {}(not used)",
                addr,
                if s.uuid16 == UUID_SERVICE_VOCS { "Volume Offset Control" } else { "Audio Input Control" },
                if s.included_by == svc.start_handle { "included in VCS " } else { "" }
            );
        }
        let state = d.find_char(UUID_SERVICE_VCS, UUID_VOLUME_STATE).cloned();
        let cp = d.find_char(UUID_SERVICE_VCS, UUID_VOLUME_CONTROL_POINT).cloned();
        let flags = d.find_char(UUID_SERVICE_VCS, UUID_VOLUME_FLAGS).cloned();
        let con = d.acl;
        let (Some(state), Some(cp)) = (state, cp) else {
            warn!("VCP: {}: Volume Control Service without Volume State / Control Point", addr);
            return;
        };
        info!(
            "VCP: {}: Volume Control Service (Volume State 0x{:04x}, Control Point 0x{:04x})",
            addr, state.value_handle, cp.value_handle
        );
        self.group.add(dev, VcsDevice::new(con, addr, state.value_handle, cp.value_handle));

        let id = dev;
        let gatt = ctx.gatt();
        gatt.listen(
            con,
            state.value_handle,
            Box::new(move |ctx, value| {
                VcpModule::with_group(ctx, Some((id, con)), |g, out| g.on_state(id, con, value, true, out));
            }),
        );
        gatt.subscribe(
            con,
            &state,
            false,
            Some(Box::new(move |ctx, status| {
                if status != 0 && status != ATT_ERROR_HCI_DISCONNECT_RECEIVED {
                    let addr = addr_of(ctx, id).unwrap_or_else(|| "?".to_string());
                    warn!("VCP: {addr}: Volume State notifications not enabled (0x{status:02x})");
                }
            })),
        );
        gatt.read(
            con,
            state.value_handle,
            Box::new(move |ctx, status, value| {
                if status == 0 {
                    VcpModule::with_group(ctx, Some((id, con)), |g, out| g.on_state(id, con, value, false, out));
                } else if status != ATT_ERROR_HCI_DISCONNECT_RECEIVED {
                    let addr = addr_of(ctx, id).unwrap_or_else(|| "?".to_string());
                    warn!("VCP: {addr}: reading Volume State failed (0x{status:02x})");
                }
            }),
        );
        if let Some(flags) = flags {
            gatt.read(
                con,
                flags.value_handle,
                Box::new(move |ctx, status, value| {
                    if status == 0 {
                        log_flags(ctx, id, value);
                    }
                }),
            );
            if flags.properties & PROPERTY_NOTIFY != 0 {
                gatt.listen(con, flags.value_handle, Box::new(move |ctx, value| log_flags(ctx, id, value)));
                gatt.subscribe(con, &flags, false, None);
            }
        }
    }

    fn on_disconnected(&mut self, _ctx: &mut LeCtx<'_>, dev: LeDeviceId, _reason: u8) {
        tracing::debug!("VCP: device {} gone from the group", dev.0);
        self.group.remove(dev);
    }
}

/// Any-thread part (remote_volume / request_volume)
pub struct VcpHooks {
    shared: Arc<Shared>,
    poster: LePoster,
}

impl LeGroupHooks for VcpHooks {
    fn name(&self) -> &'static str {
        "VCP"
    }

    fn remote_volume(&self) -> Option<u8> {
        u8::try_from(self.shared.level.load(Ordering::Acquire)).ok()
    }

    fn request_volume(&self, level: u8) -> bool {
        if self.shared.count.load(Ordering::Acquire) == 0 {
            return false;
        }
        let level = level.min(127);
        self.shared.level.store(level as i32, Ordering::Release);
        self.poster.post(move |ctx| {
            VcpModule::with_group(ctx, None, |g, out| g.set_group_volume(to_setting(level), out));
        });
        true
    }
}

pub fn create(setup: &mut LeModuleSetup) -> LeModuleEntry {
    let shared = Arc::new(Shared::new());
    LeModuleEntry {
        module: Box::new(VcpModule { group: Group::new(shared.clone()) }),
        hooks: Some(Arc::new(VcpHooks { shared, poster: setup.poster.clone() })),
    }
}

#[cfg(test)]
mod tests {
    use le_audio::vcp_scale::OP_SET_ABSOLUTE_VOLUME;

    use super::*;

    const A: LeDeviceId = LeDeviceId(0);
    const B: LeDeviceId = LeDeviceId(1);
    const CON_A: u16 = 0x40;
    const CON_B: u16 = 0x41;
    const STATE: u16 = 0x20;
    const CP: u16 = 0x22;

    /// A VCS server as Bumble's: Volume State and Control Point
    #[derive(Clone, Copy)]
    struct Sink {
        setting: u8,
        mute: bool,
        counter: u8,
    }

    impl Sink {
        fn value(&self) -> [u8; 3] {
            [self.setting, self.mute as u8, self.counter]
        }

        /// Applies a Control Point write; the ATT status
        fn write(&mut self, value: &[u8]) -> u8 {
            if value[1] != self.counter {
                return ATT_ERROR_INVALID_CHANGE_COUNTER;
            }
            match value[0] {
                OP_SET_ABSOLUTE_VOLUME => {
                    if self.setting != value[2] {
                        self.setting = value[2];
                        self.counter = self.counter.wrapping_add(1);
                    }
                }
                OP_UNMUTE => {
                    if self.mute {
                        self.mute = false;
                        self.counter = self.counter.wrapping_add(1);
                    }
                }
                _ => return ATT_ERROR_OPCODE_NOT_SUPPORTED,
            }
            0
        }

        /// A button press (to be notified)
        fn press(&mut self, setting: u8) {
            self.setting = setting;
            self.counter = self.counter.wrapping_add(1);
        }
    }

    fn group() -> (Group, Arc<Shared>) {
        let shared = Arc::new(Shared::new());
        (Group::new(shared.clone()), shared)
    }

    fn add(g: &mut Group, id: LeDeviceId, con: u16, addr: &str) {
        g.add(id, VcsDevice::new(con, addr.to_string(), STATE, CP));
    }

    fn level(shared: &Shared) -> i32 {
        shared.level.load(Ordering::Acquire)
    }

    /// The single write of `ops` (asserting there is exactly one)
    fn write_of(ops: &[GattOp]) -> (LeDeviceId, Vec<u8>) {
        match ops {
            [GattOp::Write { id, value, handle: CP, .. }] => (*id, value.clone()),
            other => panic!("expected one Control Point write, got {other:?}"),
        }
    }

    /// Runs a write against `sink` up to the end of the read after it;
    /// returns what the group wants next
    fn complete(g: &mut Group, id: LeDeviceId, con: u16, sink: &mut Sink, value: &[u8]) -> Vec<GattOp> {
        let status = sink.write(value);
        let mut out = Vec::new();
        g.write_done(id, con, status, &mut out);
        assert_eq!(out, [GattOp::ReadAfterWrite { id, con, handle: STATE, write_status: status }]);
        out.clear();
        g.read_done(id, con, status, 0, &sink.value(), &mut out);
        out
    }

    /// One device read with `sink`'s state
    fn one(sink: &Sink) -> (Group, Arc<Shared>) {
        let (mut g, shared) = group();
        add(&mut g, A, CON_A, "a");
        let mut out = Vec::new();
        g.on_state(A, CON_A, &sink.value(), false, &mut out);
        assert!(out.is_empty(), "the headset volume is kept: {out:?}");
        (g, shared)
    }

    #[test]
    fn first_device_sets_the_group_volume_and_windows_changes_it() {
        let mut sink = Sink { setting: 100, mute: true, counter: 3 };
        let (mut g, shared) = one(&sink);
        assert_eq!(shared.count.load(Ordering::Acquire), 1);
        assert_eq!((g.target, level(&shared)), (Some(100), 50));

        // The same level (VolumeSync taking it over): nothing written
        let mut out = Vec::new();
        g.set_group_volume(to_setting(50), &mut out);
        assert!(out.is_empty());
        assert_eq!(g.target, Some(100));

        // Windows -> 90: Set Absolute Volume 181, then Unmute
        g.set_group_volume(to_setting(90), &mut out);
        assert_eq!(write_of(&out), (A, vec![OP_SET_ABSOLUTE_VOLUME, 3, 181]));
        let out = complete(&mut g, A, CON_A, &mut sink, &write_of(&out).1);
        assert_eq!(write_of(&out), (A, vec![OP_UNMUTE, 4]));
        let out = complete(&mut g, A, CON_A, &mut sink, &write_of(&out).1);
        assert!(out.is_empty());
        assert_eq!((sink.setting, sink.mute), (181, false));
        assert_eq!(level(&shared), 90);
        assert!(!g.devices[&A].busy());
    }

    #[test]
    fn a_button_press_moves_the_group_without_a_write_back() {
        let mut sink = Sink { setting: 100, mute: false, counter: 0 };
        let (mut g, shared) = one(&sink);
        let mut out = Vec::new();
        sink.press(200);
        g.on_state(A, CON_A, &sink.value(), true, &mut out);
        assert!(out.is_empty());
        assert_eq!((g.target, level(&shared)), (Some(200), 100));
        // A notification of another link, a malformed one: ignored
        g.on_state(A, CON_B, &[10, 0, 9], true, &mut out);
        g.on_state(A, CON_A, &[10, 0], true, &mut out);
        assert_eq!(g.target, Some(200));
    }

    #[test]
    fn invalid_change_counter_is_retried_once() {
        let mut sink = Sink { setting: 100, mute: false, counter: 0 };
        let (mut g, shared) = one(&sink);
        // The counter moves without a notification
        sink.counter = 1;
        let mut out = Vec::new();
        g.set_group_volume(181, &mut out);
        assert_eq!(write_of(&out).1, [OP_SET_ABSOLUTE_VOLUME, 0, 181]);
        let out = complete(&mut g, A, CON_A, &mut sink, &write_of(&out).1);
        // Retried with the counter just read
        assert_eq!(write_of(&out).1, [OP_SET_ABSOLUTE_VOLUME, 1, 181]);
        assert!(g.devices[&A].retried);
        let out = complete(&mut g, A, CON_A, &mut sink, &write_of(&out).1);
        assert!(out.is_empty());
        assert_eq!(sink.setting, 181);
        assert!(!g.devices[&A].retried);
        assert_eq!(level(&shared), 90);

        // Twice 0x80: given up, the group takes the device's volume
        let mut out = Vec::new();
        g.set_group_volume(to_setting(40), &mut out);
        for _ in 0..2 {
            sink.counter = sink.counter.wrapping_add(1);
            out = complete(&mut g, A, CON_A, &mut sink, &write_of(&out).1);
        }
        assert!(out.is_empty());
        assert_eq!(sink.setting, 181);
        assert_eq!((g.target, level(&shared)), (Some(181), 90));
    }

    #[test]
    fn a_press_during_the_write_wins_over_it() {
        let mut sink = Sink { setting: 100, mute: false, counter: 0 };
        let (mut g, shared) = one(&sink);
        let mut out = Vec::new();
        g.set_group_volume(181, &mut out);
        let w = write_of(&out).1;
        // The write is applied, then the button sets 40 (notified)
        let status = sink.write(&w);
        assert_eq!(status, 0);
        sink.press(40);
        out.clear();
        g.on_state(A, CON_A, &sink.value(), true, &mut out);
        assert!(g.devices[&A].device_changed);
        g.write_done(A, CON_A, status, &mut out);
        assert!(matches!(out.as_slice(), [GattOp::ReadAfterWrite { .. }]), "{out:?}");
        out.clear();
        g.read_done(A, CON_A, status, 0, &sink.value(), &mut out);
        assert!(out.is_empty(), "181 must not be written again: {out:?}");
        assert_eq!((g.target, level(&shared)), (Some(40), 20));
    }

    #[test]
    fn a_press_before_the_write_is_not_overwritten() {
        let mut sink = Sink { setting: 100, mute: false, counter: 0 };
        let (mut g, shared) = one(&sink);
        let mut out = Vec::new();
        g.set_group_volume(181, &mut out);
        let w = write_of(&out).1;
        // The button sets 40 just before the write arrives: 0x80
        sink.press(40);
        let status = sink.write(&w);
        assert_eq!(status, ATT_ERROR_INVALID_CHANGE_COUNTER);
        out.clear();
        g.write_done(A, CON_A, status, &mut out);
        out.clear();
        // The read after the write sees the new volume first
        g.read_done(A, CON_A, status, 0, &sink.value(), &mut out);
        assert!(out.is_empty(), "not retried: {out:?}");
        assert_eq!((g.target, level(&shared)), (Some(40), 20));
        // Its notification afterwards changes nothing
        g.on_state(A, CON_A, &sink.value(), true, &mut out);
        assert!(out.is_empty());
        assert_eq!(g.target, Some(40));
    }

    #[test]
    fn earbuds_share_the_group_volume() {
        let (mut g, shared) = group();
        add(&mut g, A, CON_A, "L");
        add(&mut g, B, CON_B, "R");
        let mut left = Sink { setting: 100, mute: false, counter: 0 };
        let mut right = Sink { setting: 140, mute: false, counter: 0 };
        let mut out = Vec::new();
        g.on_state(A, CON_A, &left.value(), false, &mut out);
        assert!(out.is_empty());
        // The right one is set to the left one's volume
        g.on_state(B, CON_B, &right.value(), false, &mut out);
        assert_eq!(write_of(&out), (B, vec![OP_SET_ABSOLUTE_VOLUME, 0, 100]));
        let out = complete(&mut g, B, CON_B, &mut right, &write_of(&out).1);
        assert!(out.is_empty());
        assert_eq!(level(&shared), 50);

        // Windows -> both, in id order
        let mut out = Vec::new();
        g.set_group_volume(181, &mut out);
        let [GattOp::Write { id: ia, value: wa, .. }, GattOp::Write { id: ib, value: wb, .. }] = out.as_slice() else {
            panic!("{out:?}")
        };
        assert_eq!((*ia, *ib), (A, B));
        let (wa, wb) = (wa.clone(), wb.clone());
        assert!(complete(&mut g, A, CON_A, &mut left, &wa).is_empty());
        assert!(complete(&mut g, B, CON_B, &mut right, &wb).is_empty());

        // A button on the right one: the left one and Windows follow
        right.press(60);
        let mut out = Vec::new();
        g.on_state(B, CON_B, &right.value(), true, &mut out);
        assert_eq!(write_of(&out), (A, vec![OP_SET_ABSOLUTE_VOLUME, left.counter, 60]));
        assert_eq!(level(&shared), 30);
        assert!(complete(&mut g, A, CON_A, &mut left, &write_of(&out).1).is_empty());
        assert_eq!((left.setting, right.setting), (60, 60));

        // Disconnects: unknown once the last one is gone
        g.remove(A);
        assert_eq!((level(&shared), shared.count.load(Ordering::Acquire)), (30, 1));
        g.remove(B);
        assert_eq!((level(&shared), shared.count.load(Ordering::Acquire)), (-1, 0));
        assert_eq!(g.target, None);
    }

    #[test]
    fn newer_levels_wait_for_the_running_write() {
        let mut sink = Sink { setting: 100, mute: false, counter: 0 };
        let (mut g, _) = one(&sink);
        let mut out = Vec::new();
        g.set_group_volume(181, &mut out);
        let w = write_of(&out).1;
        out.clear();
        g.set_group_volume(to_setting(20), &mut out);
        g.set_group_volume(to_setting(30), &mut out);
        assert!(out.is_empty(), "one write at a time: {out:?}");
        // The echo of A2DPWB's own write is not followed
        let status = sink.write(&w);
        g.on_state(A, CON_A, &sink.value(), true, &mut out);
        assert!(out.is_empty() && !g.devices[&A].device_changed);
        g.write_done(A, CON_A, status, &mut out);
        out.clear();
        g.read_done(A, CON_A, status, 0, &sink.value(), &mut out);
        assert_eq!(write_of(&out).1, [OP_SET_ABSOLUTE_VOLUME, 1, to_setting(30)]);

        // The link goes down during the write: no read, not busy
        out.clear();
        g.write_done(A, CON_A, ATT_ERROR_HCI_DISCONNECT_RECEIVED, &mut out);
        assert!(out.is_empty() && !g.devices[&A].busy());
    }

    #[test]
    fn hooks_answer_only_with_a_vcs_device() {
        let shared = Arc::new(Shared::new());
        let hooks = VcpHooks { shared: shared.clone(), poster: LePoster::detached() };
        assert_eq!(hooks.remote_volume(), None);
        assert!(!hooks.request_volume(64));
        let mut g = Group::new(shared.clone());
        add(&mut g, A, CON_A, "a");
        assert!(hooks.request_volume(200));
        assert_eq!(hooks.remote_volume(), Some(127));
        g.clear();
        assert_eq!((hooks.remote_volume(), shared.count.load(Ordering::Acquire)), (None, 0));
        assert!(!hooks.request_volume(64));
    }
}
