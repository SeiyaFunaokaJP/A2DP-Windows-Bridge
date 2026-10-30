//! CCP: the Call Control Server (GTBS) on the PC (v1 le_audio_ccp.cpp).
//!
//! GTBS (TBS 1.0, 0x184C) with all mandatory characteristics plus the
//! optional Bearer Signal Strength (+ Reporting Interval), Incoming Call
//! Target Bearer URI and Call Friendly Name, all on encrypted links only:
//!
//! ```text
//! 0x2BB3 Bearer Provider Name            Read, Notify
//! 0x2BB4 Bearer UCI                      Read
//! 0x2BB5 Bearer Technology               Read, Notify
//! 0x2BB6 Bearer URI Schemes Supp. List   Read, Notify
//! 0x2BB7 Bearer Signal Strength          Read, Notify   (255: unavailable)
//! 0x2BB8 ... Reporting Interval          Read, Write, Write w/o Response
//! 0x2BB9 Bearer List Current Calls       Read, Notify
//! 0x2BBA Content Control ID              Read           (CCID_GTBS)
//! 0x2BBB Status Flags                    Read, Notify   (0: no in-band ringtone, not silent)
//! 0x2BBC Incoming Call Target Bearer URI Read, Notify
//! 0x2BBD Call State                      Read, Notify
//! 0x2BBE Call Control Point              Write, Write w/o Response, Notify
//! 0x2BBF Call Control Point Opt. Opcodes Read           (bit 0: Local Hold / Retrieve)
//! 0x2BC0 Termination Reason              Notify
//! 0x2BC1 Incoming Call                   Read, Notify
//! 0x2BC2 Call Friendly Name              Read, Notify
//! ```
//!
//! The call model (`le_audio::ccp_model::CallModel`, BTstack thread) is
//! changed by the Call Control Point (the headset) and by the telephony side:
//! today only the simulation of `LeModuleSetup::ccp_sim` (`--ccp-sim`, the
//! script starts when the first client enables Call State notifications)
//! and `ccp_console` (`--ccp-console`, commands from stdin on a thread,
//! posted to the BTstack thread). After every change the module notifies
//! what `CallModel::take_changes` lists, in the order of Zephyr's tbs.c:
//! Bearer List Current Calls and Call State, the Termination Reason(s), then
//! an incoming call's Target Bearer URI, Friendly Name and Incoming Call;
//! after a Call Control Point write the result to the writer last.
//!
//! Long values: notifications are cut to ATT_MTU - 3 (TBS: the client reads
//! the rest); a long read whose value changes between its parts gets the
//! TBS error "Value Changed During Read Long" (0x80).
//!
//! `add_ccids`: `CCID_GTBS` for streams with the Conversational context.

use std::collections::HashMap;
use std::io::BufRead;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use btstack_sys::consts::GATT_CLIENT_CHARACTERISTICS_CONFIGURATION_NOTIFICATION;
use le_audio::ccp_model::{
    ATT_ERROR_INVALID_ATTRIBUTE_VALUE_LENGTH, ATT_ERROR_VALUE_CHANGED_DURING_READ_LONG, Backend, CallModel,
    RESULT_SUCCESS, SimBackend, SimStep, op_name, parse_sim_script, result_name,
};
use le_audio::defs::{CCID_GTBS, CONTEXT_CONVERSATIONAL, UUID_SERVICE_GTBS};

use super::super::LeDeviceId;
use super::super::core::{LeCtx, LePoster};
use super::super::gatt_server::{
    ATT_PROPERTY_NOTIFY, ATT_PROPERTY_READ, ATT_PROPERTY_WRITE, ATT_PROPERTY_WRITE_WITHOUT_RESPONSE,
    ATT_SECURITY_ENCRYPTED, AttRead, AttUuid, AttWrite, GattServerBuilder,
};
use super::super::module::{
    ATT_ERROR_READ_NOT_PERMITTED, ATT_ERROR_WRITE_NOT_PERMITTED, LeModule, LeModuleEntry, LeModuleSetup,
};

const UUID_CHR_BEARER_PROVIDER_NAME: u16 = 0x2BB3;
const UUID_CHR_BEARER_UCI: u16 = 0x2BB4;
const UUID_CHR_BEARER_TECHNOLOGY: u16 = 0x2BB5;
const UUID_CHR_BEARER_URI_SCHEMES: u16 = 0x2BB6;
const UUID_CHR_BEARER_SIGNAL_STRENGTH: u16 = 0x2BB7;
const UUID_CHR_BEARER_SIGNAL_INTERVAL: u16 = 0x2BB8;
const UUID_CHR_BEARER_LIST_CURRENT_CALLS: u16 = 0x2BB9;
const UUID_CHR_CONTENT_CONTROL_ID: u16 = 0x2BBA;
const UUID_CHR_STATUS_FLAGS: u16 = 0x2BBB;
const UUID_CHR_INCOMING_TARGET_URI: u16 = 0x2BBC;
const UUID_CHR_CALL_STATE: u16 = 0x2BBD;
const UUID_CHR_CALL_CONTROL_POINT: u16 = 0x2BBE;
const UUID_CHR_CALL_CP_OPTIONAL_OPCODES: u16 = 0x2BBF;
const UUID_CHR_TERMINATION_REASON: u16 = 0x2BC0;
const UUID_CHR_INCOMING_CALL: u16 = 0x2BC1;
const UUID_CHR_CALL_FRIENDLY_NAME: u16 = 0x2BC2;

const SIGNAL_STRENGTH_UNAVAILABLE: u8 = 255;

/// How often the simulation script checks its steps (v1: 100 ms)
const SIM_POLL_MS: u32 = 100;

/// `--ccp-console`: the thread reading stdin runs once per process (as in
/// v1); its lines go to the BTstack thread of the latest BtHost created
/// with the option
static CONSOLE_STARTED: AtomicBool = AtomicBool::new(false);
static CONSOLE_POSTER: Mutex<Option<LePoster>> = Mutex::new(None);

/// The value handles of GTBS (0: not built)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Handles {
    provider: u16,
    uci: u16,
    technology: u16,
    schemes: u16,
    signal: u16,
    interval: u16,
    list: u16,
    ccid: u16,
    status: u16,
    target: u16,
    state: u16,
    cp: u16,
    optional: u16,
    reason: u16,
    incoming: u16,
    friendly: u16,
}

/// What a write of the Call Control Point asks the module to send
#[derive(Debug, PartialEq, Eq)]
struct CpReply {
    /// To every subscribed client first: (value handle, value)
    notifications: Vec<(u16, Vec<u8>)>,
    /// To the writer last: [opcode, call index, result]
    result: [u8; 3],
}

/// BTstack-thread part (no worker hooks)
pub struct CcpModule {
    /// The telephony side; the simulation allows everything
    backend: Box<dyn Backend>,
    model: CallModel,
    h: Handles,
    built: bool,
    signal_interval: u8,
    /// Long reads: the value given at offset 0, per (con, value handle)
    long_reads: HashMap<(u16, u16), Vec<u8>>,
    /// Simulation script (--ccp-sim): runs once per BtHost
    script: Vec<SimStep>,
    script_pos: usize,
    script_armed: bool,
    armed_at_ms: u32,
    timer_active: bool,
    /// --ccp-console
    console: bool,
}

impl Default for CcpModule {
    fn default() -> Self {
        CcpModule {
            backend: Box::new(SimBackend),
            model: CallModel::new(),
            h: Handles::default(),
            built: false,
            signal_interval: 0,
            long_reads: HashMap::new(),
            script: Vec::new(),
            script_pos: 0,
            script_armed: false,
            armed_at_ms: 0,
            timer_active: false,
            console: false,
        }
    }
}

pub fn create(setup: &mut LeModuleSetup) -> LeModuleEntry {
    let mut m = CcpModule { console: setup.ccp_console, ..CcpModule::default() };
    if let Some(spec) = setup.ccp_sim.as_deref() {
        m.script = parse_script(spec);
        if !m.script.is_empty() {
            tracing::info!(
                "LE Audio: CCP: simulation script of {} step(s), starts when a client subscribes to Call State",
                m.script.len()
            );
        }
    }
    if m.console {
        *CONSOLE_POSTER.lock().unwrap_or_else(|e| e.into_inner()) = Some(setup.poster.clone());
    }
    LeModuleEntry { module: Box::new(m), hooks: None }
}

/// `--ccp-sim` as v1 read it: [`parse_sim_script`]; a script it refuses
/// gets v1's leniency (a step without a time is skipped with v1's warning,
/// a time that is no number counts as its numeric prefix, as atof did)
fn parse_script(spec: &str) -> Vec<SimStep> {
    if let Some(steps) = parse_sim_script(spec) {
        return steps;
    }
    let mut steps = Vec::new();
    for step in spec.split(';').map(str::trim).filter(|s| !s.is_empty()) {
        match step.split_once(':') {
            None => tracing::warn!("LE Audio: CCP: --ccp-sim step \"{step}\" has no time (<seconds>:<command>)"),
            Some((t, command)) => {
                let t = atof(t).max(0.0);
                steps.push(SimStep { at_ms: (t * 1000.0) as u32, command: command.trim().into() });
            }
        }
    }
    steps.sort_by_key(|s| s.at_ms);
    steps
}

/// C's atof: the longest numeric prefix, 0 if none
fn atof(s: &str) -> f64 {
    let s = s.trim_start();
    (1..=s.len())
        .rev()
        .filter(|&n| s.is_char_boundary(n))
        .find_map(|n| s[..n].parse::<f64>().ok().filter(|t| t.is_finite()))
        .unwrap_or(0.0)
}

/// Runs `f` with the CCP module from a timer / posted callback (the module
/// is moved out of the registry meanwhile, so `f` also gets the LeCtx)
fn with_module(ctx: &mut LeCtx<'_>, f: impl FnOnce(&mut CcpModule, &mut LeCtx<'_>)) {
    let Some(m) = ctx.module_mut::<CcpModule>() else { return };
    let mut taken = std::mem::take(m);
    f(&mut taken, ctx);
    if let Some(m) = ctx.module_mut::<CcpModule>() {
        *m = taken;
    }
}

impl CcpModule {
    /// A BTstack session starts or ends: no calls, nothing built
    fn reset_session(&mut self) {
        self.built = false;
        self.model = CallModel::new();
        self.long_reads.clear();
        self.timer_active = false;
        self.h = Handles::default();
    }

    // ---- Characteristic values (no BTstack) ----

    /// The whole value of a readable characteristic, and whether it may be
    /// longer than one PDU (its long reads are checked for changes)
    fn value(&self, handle: u16) -> Option<(Vec<u8>, bool)> {
        let h = &self.h;
        if handle == 0 {
            return None;
        }
        let v = if handle == h.provider {
            (self.backend.provider_name().into_bytes(), true)
        } else if handle == h.uci {
            (self.backend.uci().into_bytes(), true)
        } else if handle == h.technology {
            (vec![self.backend.technology()], false)
        } else if handle == h.schemes {
            (self.backend.uri_schemes().into_bytes(), true)
        } else if handle == h.signal {
            (vec![SIGNAL_STRENGTH_UNAVAILABLE], false)
        } else if handle == h.interval {
            (vec![self.signal_interval], false)
        } else if handle == h.list {
            (self.model.list_value(), true)
        } else if handle == h.ccid {
            (vec![CCID_GTBS], false)
        } else if handle == h.status {
            (vec![0x00, 0x00], false)
        } else if handle == h.target {
            (self.model.target_value(), true)
        } else if handle == h.state {
            (self.model.state_value(), true)
        } else if handle == h.optional {
            // bit 0: Local Hold / Retrieve; bit 1 (Join): no
            (u16::from(self.backend.supports_local_hold()).to_le_bytes().to_vec(), false)
        } else if handle == h.incoming {
            (self.model.incoming_value(), true)
        } else if handle == h.friendly {
            (self.model.friendly_value(), true)
        } else {
            return None;
        };
        Some(v)
    }

    /// A read: remember what offset 0 returned; a later part (Read Blob) of
    /// a changed value fails with Value Changed During Read Long
    fn read(&mut self, con: u16, handle: u16, offset: u16) -> Result<Vec<u8>, u8> {
        let Some((value, long)) = self.value(handle) else { return Err(ATT_ERROR_READ_NOT_PERMITTED) };
        if !long {
            return Ok(value);
        }
        if offset == 0 {
            self.long_reads.insert((con, handle), value.clone());
        } else if self.long_reads.get(&(con, handle)).is_some_and(|v| *v != value) {
            tracing::info!("LE Audio: CCP: 0x{handle:04x} changed during a long read by 0x{con:04x}");
            return Err(ATT_ERROR_VALUE_CHANGED_DURING_READ_LONG);
        }
        Ok(value)
    }

    /// What the last change left to notify, in the order of Zephyr's tbs.c:
    /// Bearer List Current Calls and Call State, the Termination Reason(s),
    /// then an incoming call's Target Bearer URI (if it has one), Friendly
    /// Name (if it has one) and Incoming Call
    fn take_notifications(&mut self) -> Vec<(u16, Vec<u8>)> {
        let changes = self.model.take_changes();
        let mut out = Vec::new();
        if changes.calls {
            out.push((self.h.list, self.model.list_value()));
            out.push((self.h.state, self.model.state_value()));
        }
        for (index, reason) in changes.terminations {
            out.push((self.h.reason, vec![index, reason]));
        }
        if let Some(index) = changes.incoming {
            if let Some(c) = self.model.find(index) {
                if !c.target_uri.is_empty() {
                    out.push((self.h.target, self.model.target_value()));
                }
                if !c.friendly_name.is_empty() {
                    out.push((self.h.friendly, self.model.friendly_value()));
                }
            }
            out.push((self.h.incoming, self.model.incoming_value()));
        }
        out
    }

    /// A Call Control Point write from `con`: Err with an ATT error (wrong
    /// length: nothing is notified), else what to send
    fn control_point(&mut self, con: u16, value: &[u8]) -> Result<CpReply, u8> {
        tracing::debug!("LE Audio: CCP: control point value {value:02X?} from 0x{con:04X}");
        let out = self.model.control_point(self.backend.as_mut(), value)?;
        tracing::info!(
            "LE Audio: CCP: control point {} (0x{:02x}) call {} from 0x{con:04x} -> {}",
            op_name(out.opcode),
            out.opcode,
            out.call_index,
            result_name(out.result)
        );
        Ok(CpReply { notifications: self.take_notifications(), result: [out.opcode, out.call_index, out.result] })
    }

    // ---- Sending (BTstack thread) ----

    /// Notifications carry at most ATT_MTU - 3 bytes (TBS: the client reads
    /// the whole value)
    fn notify(&self, ctx: &mut LeCtx<'_>, con: u16, handle: u16, value: &[u8]) {
        if !self.built || handle == 0 {
            return;
        }
        let mtu = usize::from(ctx.stack().att_server_mtu(con));
        let room = if mtu > 3 { mtu - 3 } else { 20 };
        ctx.gatt_server().notify(con, handle, &value[..value.len().min(room)]);
    }

    fn notify_all(&self, ctx: &mut LeCtx<'_>, handle: u16, value: &[u8]) {
        if !self.built {
            return;
        }
        for con in ctx.gatt_server().clients() {
            self.notify(ctx, con, handle, value);
        }
    }

    fn send_all(&self, ctx: &mut LeCtx<'_>, notifications: Vec<(u16, Vec<u8>)>) {
        if !notifications.is_empty() {
            tracing::debug!(
                "LE Audio: CCP: notifying {:04X?}",
                notifications.iter().map(|(h, _)| *h).collect::<Vec<_>>()
            );
        }
        for (handle, value) in notifications {
            self.notify_all(ctx, handle, &value);
        }
    }

    /// One simulation command (v1 ccp::simulate), then what it changed is
    /// sent; Err: why it cannot be done
    fn simulate(&mut self, ctx: &mut LeCtx<'_>, command: &str) -> Result<(), String> {
        tracing::debug!("LE Audio: CCP: simulating \"{command}\"");
        let r = self.model.simulate(command);
        let n = self.take_notifications();
        self.send_all(ctx, n);
        r.map(|_| ())
    }

    // ---- Simulation script ----

    fn start_timer(&mut self, ctx: &mut LeCtx<'_>) {
        if self.timer_active {
            return;
        }
        self.timer_active = true;
        ctx.run_after(
            SIM_POLL_MS,
            Box::new(|ctx| {
                with_module(ctx, |m, ctx| {
                    m.timer_active = false;
                    m.on_timer(ctx);
                })
            }),
        );
    }

    fn on_timer(&mut self, ctx: &mut LeCtx<'_>) {
        if !self.built {
            // The GATT server is not up yet
            self.start_timer(ctx);
            return;
        }
        let now = ctx.stack().time_ms();
        if !self.script_armed {
            let state = self.h.state;
            let server = ctx.gatt_server();
            let subscribed = server.clients().into_iter().find(|&con| {
                server.client_config(con, state) & GATT_CLIENT_CHARACTERISTICS_CONFIGURATION_NOTIFICATION != 0
            });
            if let Some(con) = subscribed {
                self.script_armed = true;
                self.armed_at_ms = now;
                tracing::info!("LE Audio: CCP: client 0x{con:04x} subscribed to Call State, simulation starts");
            }
        }
        while self.script_armed
            && self.script_pos < self.script.len()
            && now.wrapping_sub(self.armed_at_ms) >= self.script[self.script_pos].at_ms
        {
            let command = self.script[self.script_pos].command.clone();
            if let Err(e) = self.simulate(ctx, &command) {
                tracing::warn!("LE Audio: CCP: simulation \"{command}\" failed: {e}");
            }
            self.script_pos += 1;
        }
        if self.script_pos < self.script.len() {
            self.start_timer(ctx);
        }
    }
}

/// `--ccp-console`: lines from stdin to the BTstack thread (Rust's exit does
/// not take the stdin lock, so a thread blocked here does not hold it up)
fn console_thread() {
    let stdin = std::io::stdin();
    let mut line = String::new();
    loop {
        line.clear();
        match stdin.lock().read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let command = line.trim().to_string();
        if command.is_empty() {
            continue;
        }
        let poster = CONSOLE_POSTER.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let Some(poster) = poster else { continue };
        poster.post(move |ctx| {
            with_module(ctx, |m, ctx| {
                if let Err(e) = m.simulate(ctx, &command) {
                    println!("CCP: {command}: {e}");
                }
            })
        });
    }
}

impl LeModule for CcpModule {
    fn name(&self) -> &'static str {
        "CCP"
    }

    fn on_stack_init(&mut self, ctx: &mut LeCtx<'_>) {
        self.reset_session();
        if self.console && !CONSOLE_STARTED.swap(true, Ordering::AcqRel) {
            if let Err(e) = std::thread::Builder::new().name("ccp-console".into()).spawn(console_thread) {
                tracing::warn!("LE Audio: CCP: console thread: {e}");
            }
            println!(
                "CCP console: incoming <uri>[|<name>[|<target uri>]], alerting, answered, remote-hold, \
                 remote-retrieve, hangup [<index> [<reason>]], status"
            );
        }
        if self.script_pos < self.script.len() {
            self.start_timer(ctx);
        }
    }

    fn add_gatt_services(&mut self, server: &mut GattServerBuilder<'_>) {
        const RN: u16 = ATT_PROPERTY_READ | ATT_PROPERTY_NOTIFY;
        // TBS: encrypted links only
        const ENC: u8 = ATT_SECURITY_ENCRYPTED;
        let start = server.begin_service(AttUuid::Uuid16(UUID_SERVICE_GTBS), true);
        let mut add =
            |uuid: u16, properties: u16| server.add_characteristic(AttUuid::Uuid16(uuid), properties, ENC, ENC);
        let h = Handles {
            provider: add(UUID_CHR_BEARER_PROVIDER_NAME, RN),
            uci: add(UUID_CHR_BEARER_UCI, ATT_PROPERTY_READ),
            technology: add(UUID_CHR_BEARER_TECHNOLOGY, RN),
            schemes: add(UUID_CHR_BEARER_URI_SCHEMES, RN),
            signal: add(UUID_CHR_BEARER_SIGNAL_STRENGTH, RN),
            interval: add(
                UUID_CHR_BEARER_SIGNAL_INTERVAL,
                ATT_PROPERTY_READ | ATT_PROPERTY_WRITE | ATT_PROPERTY_WRITE_WITHOUT_RESPONSE,
            ),
            list: add(UUID_CHR_BEARER_LIST_CURRENT_CALLS, RN),
            ccid: add(UUID_CHR_CONTENT_CONTROL_ID, ATT_PROPERTY_READ),
            status: add(UUID_CHR_STATUS_FLAGS, RN),
            target: add(UUID_CHR_INCOMING_TARGET_URI, RN),
            state: add(UUID_CHR_CALL_STATE, RN),
            cp: add(
                UUID_CHR_CALL_CONTROL_POINT,
                ATT_PROPERTY_WRITE | ATT_PROPERTY_WRITE_WITHOUT_RESPONSE | ATT_PROPERTY_NOTIFY,
            ),
            optional: add(UUID_CHR_CALL_CP_OPTIONAL_OPCODES, ATT_PROPERTY_READ),
            reason: add(UUID_CHR_TERMINATION_REASON, ATT_PROPERTY_NOTIFY),
            incoming: add(UUID_CHR_INCOMING_CALL, RN),
            friendly: add(UUID_CHR_CALL_FRIENDLY_NAME, RN),
        };
        let end = server.end_service();
        self.h = h;
        self.built = true;
        tracing::info!(
            "LE Audio: CCP: GTBS handles 0x{start:04x}-0x{end:04x}, CCID {CCID_GTBS}, call state 0x{:04x}, control \
             point 0x{:04x}",
            h.state,
            h.cp
        );
    }

    fn on_stack_deinit(&mut self) {
        // The run loop has stopped: drop the state, no BTstack calls
        self.reset_session();
    }

    fn on_disconnected(&mut self, ctx: &mut LeCtx<'_>, dev: LeDeviceId, _reason: u8) {
        // Keep the long reads of the links still up
        let up: Vec<u16> = ctx
            .device_ids()
            .into_iter()
            .filter(|&id| id != dev)
            .filter_map(|id| ctx.device(id))
            .filter(|d| d.connected())
            .map(|d| d.acl)
            .collect();
        self.long_reads.retain(|(con, _), _| up.contains(con));
    }

    fn add_ccids(&self, contexts: u16, ccids: &mut Vec<u8>) {
        // GTBS controls calls: only a Conversational stream belongs to it
        if contexts & CONTEXT_CONVERSATIONAL != 0 {
            ccids.push(CCID_GTBS);
        }
    }

    fn on_att_read(&mut self, _ctx: &mut LeCtx<'_>, read: &AttRead) -> Result<Vec<u8>, u8> {
        let r = self.read(read.con, read.handle, read.offset);
        tracing::debug!(
            "LE Audio: CCP: read 0x{:04X} offset {} by 0x{:04X}: {:02X?}",
            read.handle,
            read.offset,
            read.con,
            r.as_deref()
        );
        r
    }

    fn on_att_write(&mut self, ctx: &mut LeCtx<'_>, write: &AttWrite<'_>) -> Result<(), u8> {
        if write.handle == 0 {
            return Err(ATT_ERROR_WRITE_NOT_PERMITTED);
        }
        if write.handle == self.h.interval {
            let &[v] = write.value else { return Err(ATT_ERROR_INVALID_ATTRIBUTE_VALUE_LENGTH) };
            // The strength never changes: nothing to report
            tracing::debug!("LE Audio: CCP: signal strength reporting interval {v} s from 0x{:04X}", write.con);
            self.signal_interval = v;
            return Ok(());
        }
        if write.handle != self.h.cp {
            return Err(ATT_ERROR_WRITE_NOT_PERMITTED);
        }
        let reply = self.control_point(write.con, write.value)?;
        // What changed first (list, Call State, Termination Reason), the
        // result to the writer last, as Zephyr's tbs.c does
        self.send_all(ctx, reply.notifications);
        self.notify(ctx, write.con, self.h.cp, &reply.result);
        if reply.result[2] == RESULT_SUCCESS {
            self.model.log_calls(op_name(reply.result[0]));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use le_audio::ccp_model::{
        CallState, OP_ACCEPT, OP_JOIN, OP_TERMINATE, REASON_CLIENT_TERMINATED, RESULT_INVALID_CALL_INDEX,
        RESULT_OPCODE_NOT_SUPPORTED,
    };

    use super::super::super::gatt_server::GattServer;
    use super::*;

    const CON: u16 = 0x40;

    /// A module with GTBS built after GAP / GATT (as in a session)
    fn built() -> CcpModule {
        let mut s = GattServer::new();
        s.build_base();
        let mut m = CcpModule::default();
        m.add_gatt_services(&mut s.builder(3));
        m
    }

    #[test]
    fn gtbs_layout() {
        let m = built();
        let h = m.h;
        // GAP / GATT end at 11; service 12, then 16 characteristics, the 12
        // with NOTIFY with a CCC
        assert_eq!(h.provider, 14);
        assert_eq!(h.uci, 17);
        let all = [
            h.provider,
            h.uci,
            h.technology,
            h.schemes,
            h.signal,
            h.interval,
            h.list,
            h.ccid,
            h.status,
            h.target,
            h.state,
            h.cp,
            h.optional,
            h.reason,
            h.incoming,
            h.friendly,
        ];
        assert!(all.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(h.friendly, 12 + 2 * 16 + 11);
        assert!(m.built);
    }

    #[test]
    fn values_without_calls() {
        let mut m = built();
        let h = m.h;
        assert_eq!(m.read(CON, h.provider, 0), Ok(b"BlueCaster".to_vec()));
        assert_eq!(m.read(CON, h.uci, 0), Ok(b"un000".to_vec()));
        assert_eq!(m.read(CON, h.technology, 0), Ok(vec![0x04]));
        assert_eq!(m.read(CON, h.schemes, 0), Ok(b"tel,sip".to_vec()));
        assert_eq!(m.read(CON, h.signal, 0), Ok(vec![0xFF]));
        assert_eq!(m.read(CON, h.interval, 0), Ok(vec![0]));
        assert_eq!(m.read(CON, h.list, 0), Ok(vec![]));
        assert_eq!(m.read(CON, h.ccid, 0), Ok(vec![CCID_GTBS]));
        assert_eq!(m.read(CON, h.status, 0), Ok(vec![0, 0]));
        assert_eq!(m.read(CON, h.state, 0), Ok(vec![]));
        assert_eq!(m.read(CON, h.optional, 0), Ok(vec![0x01, 0x00]));
        assert_eq!(m.read(CON, h.incoming, 0), Ok(vec![]));
        assert_eq!(m.read(CON, h.cp, 0), Err(ATT_ERROR_READ_NOT_PERMITTED));
        assert_eq!(m.read(CON, h.reason, 0), Err(ATT_ERROR_READ_NOT_PERMITTED));
    }

    #[test]
    fn incoming_call_notifies_in_tbs_order() {
        let mut m = built();
        let h = m.h;
        m.model.incoming("tel:+1555", "Alice", "sip:pc@x").unwrap();
        let n: Vec<u16> = m.take_notifications().into_iter().map(|(h, _)| h).collect();
        assert_eq!(n, [h.list, h.state, h.target, h.friendly, h.incoming]);
        // Without target / name: only the list, Call State, Incoming Call
        m.model.incoming("tel:+1666", "", "").unwrap();
        let n = m.take_notifications();
        assert_eq!(n.iter().map(|(h, _)| *h).collect::<Vec<_>>(), [h.list, h.state, h.incoming]);
        assert_eq!(n[2].1, [&[2u8][..], b"tel:+1666"].concat());
        assert!(m.take_notifications().is_empty());
    }

    #[test]
    fn control_point_changes_then_result() {
        let mut m = built();
        let h = m.h;
        m.model.incoming("tel:+1555", "", "").unwrap();
        m.take_notifications();
        let r = m.control_point(CON, &[OP_ACCEPT, 1]).unwrap();
        assert_eq!(r.result, [OP_ACCEPT, 1, RESULT_SUCCESS]);
        assert_eq!(r.notifications.iter().map(|(h, _)| *h).collect::<Vec<_>>(), [h.list, h.state]);
        assert_eq!(r.notifications[1].1, [1, CallState::Active.value(), 0]);
        // Terminate: list, Call State, Termination Reason 0x06
        let r = m.control_point(CON, &[OP_TERMINATE, 1]).unwrap();
        assert_eq!(r.result, [OP_TERMINATE, 1, RESULT_SUCCESS]);
        assert_eq!(
            r.notifications,
            [(h.list, vec![]), (h.state, vec![]), (h.reason, vec![1, REASON_CLIENT_TERMINATED])]
        );
        // Errors: no notification but the result; a short write is an ATT error
        let r = m.control_point(CON, &[OP_ACCEPT, 9]).unwrap();
        assert_eq!((r.result, r.notifications.len()), ([OP_ACCEPT, 9, RESULT_INVALID_CALL_INDEX], 0));
        assert_eq!(m.control_point(CON, &[OP_JOIN, 1]).unwrap().result[2], RESULT_OPCODE_NOT_SUPPORTED);
        assert_eq!(m.control_point(CON, &[OP_ACCEPT]), Err(ATT_ERROR_INVALID_ATTRIBUTE_VALUE_LENGTH));
    }

    #[test]
    fn long_read_of_a_changed_value_fails() {
        let mut m = built();
        let h = m.h;
        m.model.incoming("tel:+1555", "", "").unwrap();
        let first = m.read(CON, h.list, 0).unwrap();
        // Unchanged: the rest is served
        assert_eq!(m.read(CON, h.list, 5), Ok(first));
        m.model.incoming("tel:+1666", "", "").unwrap();
        assert_eq!(m.read(CON, h.list, 5), Err(ATT_ERROR_VALUE_CHANGED_DURING_READ_LONG));
        // Another client's long read is its own
        assert!(m.read(0x41, h.list, 5).is_ok());
        // Short values are not tracked
        assert_eq!(m.read(CON, h.ccid, 1), Ok(vec![CCID_GTBS]));
    }

    #[test]
    fn script_as_v1_read_it() {
        let s = parse_script("2:hangup; 1:incoming tel:+1555|Alice");
        assert_eq!(
            s.iter().map(|s| (s.at_ms, s.command.as_str())).collect::<Vec<_>>(),
            [(1000, "incoming tel:+1555|Alice"), (2000, "hangup")]
        );
        // v1's leniency: a step without time is skipped, a bad time is atof's
        let s = parse_script("status; 1.5x:alerting;x:answered");
        assert_eq!(
            s.iter().map(|s| (s.at_ms, s.command.as_str())).collect::<Vec<_>>(),
            [(0, "answered"), (1500, "alerting")]
        );
        assert_eq!(atof(" 2.25s"), 2.25);
        assert_eq!(atof("abc"), 0.0);
    }

    #[test]
    fn ccid_only_for_conversational() {
        let m = CcpModule::default();
        let mut ccids = vec![1];
        m.add_ccids(0x0004, &mut ccids);
        assert_eq!(ccids, [1]);
        m.add_ccids(CONTEXT_CONVERSATIONAL | 0x0004, &mut ccids);
        assert_eq!(ccids, [1, CCID_GTBS]);
    }
}
