//! CCP: the call model of the GTBS server on the PC (v1 le_audio::ccp in
//! le_audio_ccp.{h,cpp}).
//!
//! A2DPWB has no telephony: Windows has no generic call-state API and the LE
//! Audio path has no microphone. The GTBS server (bt-host's CCP module) is
//! spec-correct around this in-memory model, which two sides drive:
//! - the headset, through the Call Control Point (Accept, Terminate, Local
//!   Hold / Retrieve, Originate): [`CallModel::control_point`] /
//!   [`CallModel::accept`] ...;
//! - the telephony side through the events ([`CallModel::incoming`] ...):
//!   today only the simulation (`--ccp-sim`, `--ccp-console`,
//!   [`CallModel::simulate`]); a real integration would implement
//!   [`Backend`].
//!
//! Simulation commands: `incoming <uri>[|<friendly name>[|<target uri>]]`,
//! `alerting [<index>]`, `answered [<index>]`, `remote-hold [<index>]`,
//! `remote-retrieve [<index>]`, `hangup [<index> [<reason>]]` (default
//! reason 0x02 remote party ended; 3 = the server ended it), `status`.
//! `<index>` defaults to the most recent call.
//!
//! The model logs what v1's module logged about the calls ("LE Audio: CCP:
//! incoming call 1 from ...", "... call 1 ended (reason 0x02)", "... calls
//! after ...: ...", "... simulate \"...\""); the GATT side (handles,
//! control point lines with the connection, long reads) is the module's.
//!
//! Notifications: after a change the module sends what
//! [`CallModel::take_changes`] says, in the order of Zephyr's tbs.c: Bearer
//! List Current Calls and Call State, the Termination Reason(s), then (an
//! incoming call) Incoming Call Target Bearer URI if the call has one, Call
//! Friendly Name if it has one, and Incoming Call; after a Call Control
//! Point write the control point result to the writer last.

/// TBS 1.0 Call State values
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CallState {
    #[default]
    Incoming,
    /// Outgoing, not yet ringing at the far end
    Dialing,
    /// Outgoing, ringing at the far end
    Alerting,
    Active,
    LocallyHeld,
    RemotelyHeld,
    LocallyAndRemotelyHeld,
}

impl CallState {
    /// The TBS value (0x00 Incoming ... 0x06)
    pub fn value(self) -> u8 {
        match self {
            CallState::Incoming => 0x00,
            CallState::Dialing => 0x01,
            CallState::Alerting => 0x02,
            CallState::Active => 0x03,
            CallState::LocallyHeld => 0x04,
            CallState::RemotelyHeld => 0x05,
            CallState::LocallyAndRemotelyHeld => 0x06,
        }
    }

    /// Name for the log
    pub fn name(self) -> &'static str {
        match self {
            CallState::Incoming => "Incoming",
            CallState::Dialing => "Dialing",
            CallState::Alerting => "Alerting",
            CallState::Active => "Active",
            CallState::LocallyHeld => "Locally Held",
            CallState::RemotelyHeld => "Remotely Held",
            CallState::LocallyAndRemotelyHeld => "Locally and Remotely Held",
        }
    }
}

// Call Flags
pub const CALL_FLAG_OUTGOING: u8 = 0x01;
pub const CALL_FLAG_WITHHELD_BY_SERVER: u8 = 0x02;
pub const CALL_FLAG_WITHHELD_BY_NETWORK: u8 = 0x04;

// Call Control Point opcodes
pub const OP_ACCEPT: u8 = 0x00;
pub const OP_TERMINATE: u8 = 0x01;
pub const OP_LOCAL_HOLD: u8 = 0x02;
pub const OP_LOCAL_RETRIEVE: u8 = 0x03;
pub const OP_ORIGINATE: u8 = 0x04;
pub const OP_JOIN: u8 = 0x05;

// Call Control Point notification result codes
pub const RESULT_SUCCESS: u8 = 0x00;
pub const RESULT_OPCODE_NOT_SUPPORTED: u8 = 0x01;
pub const RESULT_OPERATION_NOT_POSSIBLE: u8 = 0x02;
pub const RESULT_INVALID_CALL_INDEX: u8 = 0x03;
pub const RESULT_STATE_MISMATCH: u8 = 0x04;
pub const RESULT_LACK_OF_RESOURCES: u8 = 0x05;
pub const RESULT_INVALID_OUTGOING_URI: u8 = 0x06;

// Termination Reason codes
pub const REASON_IMPROPER_URI: u8 = 0x00;
pub const REASON_CALL_FAILED: u8 = 0x01;
pub const REASON_REMOTE_ENDED: u8 = 0x02;
pub const REASON_SERVER_ENDED: u8 = 0x03;
pub const REASON_LINE_BUSY: u8 = 0x04;
pub const REASON_NETWORK_CONGESTION: u8 = 0x05;
pub const REASON_CLIENT_TERMINATED: u8 = 0x06;
pub const REASON_NO_SERVICE: u8 = 0x07;
pub const REASON_NO_ANSWER: u8 = 0x08;
pub const REASON_UNSPECIFIED: u8 = 0x09;

/// ATT application error of TBS: the value changed between the parts of a
/// long read
pub const ATT_ERROR_VALUE_CHANGED_DURING_READ_LONG: u8 = 0x80;

/// ATT error of a Call Control Point write of the wrong length
pub const ATT_ERROR_INVALID_ATTRIBUTE_VALUE_LENGTH: u8 = 0x0D;

/// Calls the server keeps at most (Originate / incoming beyond: lack of
/// resources)
pub const MAX_CALLS: usize = 4;

/// An item of Bearer List Current Calls has a 1-octet length: 3 + URI
pub const MAX_URI: usize = 252;

/// Name of a Call Control Point opcode for the log
pub fn op_name(op: u8) -> &'static str {
    match op {
        OP_ACCEPT => "Accept",
        OP_TERMINATE => "Terminate",
        OP_LOCAL_HOLD => "Local Hold",
        OP_LOCAL_RETRIEVE => "Local Retrieve",
        OP_ORIGINATE => "Originate",
        OP_JOIN => "Join",
        _ => "unknown opcode",
    }
}

/// Name of a Call Control Point result code for the log
pub fn result_name(result: u8) -> &'static str {
    match result {
        RESULT_SUCCESS => "Success",
        RESULT_OPCODE_NOT_SUPPORTED => "Opcode Not Supported",
        RESULT_OPERATION_NOT_POSSIBLE => "Operation Not Possible",
        RESULT_INVALID_CALL_INDEX => "Invalid Call Index",
        RESULT_STATE_MISMATCH => "State Mismatch",
        RESULT_LACK_OF_RESOURCES => "Lack Of Resources",
        RESULT_INVALID_OUTGOING_URI => "Invalid Outgoing URI",
        _ => "?",
    }
}

/// One call
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Call {
    /// 1-255
    pub index: u8,
    pub state: CallState,
    /// CALL_FLAG_*
    pub flags: u8,
    /// Remote party, e.g. "tel:+15551234567"
    pub uri: String,
    /// May be empty
    pub friendly_name: String,
    /// Incoming: our URI the call came in on (may be empty)
    pub target_uri: String,
}

/// The telephony side, asked before the model applies a request of the
/// headset (v1 ccp::Backend). The defaults are the simulation: everything
/// is allowed.
pub trait Backend {
    fn provider_name(&self) -> String {
        "A2DPWB".into()
    }
    /// Uniform Caller Identifier (Assigned Numbers); "un000" = unknown
    fn uci(&self) -> String {
        "un000".into()
    }
    /// Bearer Technology: 0x04 Wi-Fi (TBS has no "wired IP" value)
    fn technology(&self) -> u8 {
        0x04
    }
    /// Comma-separated URI schemes (Originate checks them)
    fn uri_schemes(&self) -> String {
        "tel,sip".into()
    }
    /// Local Hold / Local Retrieve available (Call Control Point Optional
    /// Opcodes)
    fn supports_local_hold(&self) -> bool {
        true
    }
    // Requests of the headset; return a RESULT_* code
    fn accept(&mut self, _call: &Call) -> u8 {
        RESULT_SUCCESS
    }
    fn terminate(&mut self, _call: &Call) -> u8 {
        RESULT_SUCCESS
    }
    fn hold(&mut self, _call: &Call) -> u8 {
        RESULT_SUCCESS
    }
    fn retrieve(&mut self, _call: &Call) -> u8 {
        RESULT_SUCCESS
    }
    fn originate(&mut self, _uri: &str) -> u8 {
        RESULT_SUCCESS
    }
}

/// The simulation backend (allows everything)
#[derive(Debug, Clone, Copy, Default)]
pub struct SimBackend;

impl Backend for SimBackend {}

/// What changed since [`CallModel::take_changes`] was last called: the
/// server notifies Call State and Bearer List Current Calls when `calls`,
/// one Termination Reason per entry of `terminations`, and for `incoming`
/// Incoming Call Target Bearer URI (if the call has one), Call Friendly
/// Name (if it has one) and Incoming Call
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Changes {
    pub calls: bool,
    /// (call index, REASON_*)
    pub terminations: Vec<(u8, u8)>,
    /// A call came in (its index); added to the Step 0 contract
    pub incoming: Option<u8>,
}

/// A Call Control Point write that the model handled (the module notifies
/// `[opcode, call_index, result]` to the writer after the changes)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpOutcome {
    pub opcode: u8,
    /// The call index of the request, or of the new call (Originate)
    pub call_index: u8,
    /// RESULT_*
    pub result: u8,
}

/// The calls of the bearer
#[derive(Debug, Clone)]
pub struct CallModel {
    calls: Vec<Call>,
    next_index: u8,
    changes: Changes,
    /// The call Call Friendly Name shows (0 none)
    friendly_index: u8,
}

impl Default for CallModel {
    fn default() -> Self {
        CallModel::new()
    }
}

impl CallModel {
    pub fn new() -> CallModel {
        CallModel { calls: Vec::new(), next_index: 1, changes: Changes::default(), friendly_index: 0 }
    }

    /// The current calls, oldest first
    pub fn calls(&self) -> &[Call] {
        &self.calls
    }

    pub fn find(&self, index: u8) -> Option<&Call> {
        if index == 0 {
            return None;
        }
        self.calls.iter().find(|c| c.index == index)
    }

    fn find_mut(&mut self, index: u8) -> Option<&mut Call> {
        if index == 0 {
            return None;
        }
        self.calls.iter_mut().find(|c| c.index == index)
    }

    /// The most recent call
    pub fn latest(&self) -> Option<&Call> {
        self.calls.last()
    }

    /// Call indices count up 1..255 and wrap, skipping those in use; 0 if
    /// all are used
    fn allocate_index(&mut self) -> u8 {
        for _ in 0..255 {
            let idx = self.next_index;
            self.next_index = if self.next_index == 255 { 1 } else { self.next_index + 1 };
            if self.find(idx).is_none() {
                return idx;
            }
        }
        0
    }

    /// A call becomes active: the other active calls go on hold
    fn hold_others(&mut self, except: u8) {
        for c in self.calls.iter_mut().filter(|c| c.index != except) {
            c.state = match c.state {
                CallState::Active => CallState::LocallyHeld,
                CallState::RemotelyHeld => CallState::LocallyAndRemotelyHeld,
                s => s,
            };
        }
    }

    fn remove(&mut self, index: u8, reason: u8) {
        self.calls.retain(|c| c.index != index);
        if self.friendly_index == index {
            self.friendly_index = 0;
        }
        self.changes.terminations.push((index, reason));
        self.changes.calls = true;
        tracing::info!("LE Audio: CCP: call {index} ended (reason 0x{reason:02x})");
    }

    /// The calls for the log, e.g. "1 Active, 2 Dialing (outgoing)", or
    /// "none"
    pub fn describe_calls(&self) -> String {
        if self.calls.is_empty() {
            return "none".into();
        }
        let items: Vec<String> = self
            .calls
            .iter()
            .map(|c| {
                format!(
                    "{} {}{}",
                    c.index,
                    c.state.name(),
                    if c.flags & CALL_FLAG_OUTGOING != 0 { " (outgoing)" } else { "" }
                )
            })
            .collect();
        items.join(", ")
    }

    /// Logs `LE Audio: CCP: calls after <why>: <calls>` (v1 log_calls; the
    /// module logs it after a successful control point operation, with
    /// [`op_name`])
    pub fn log_calls(&self, why: &str) {
        tracing::info!("LE Audio: CCP: calls after {why}: {}", self.describe_calls());
    }

    // ---- Events of the telephony side ----

    /// A call comes in: its call index, None if the list is full
    pub fn incoming(&mut self, uri: &str, friendly_name: &str, target_uri: &str) -> Option<u8> {
        if self.calls.len() >= MAX_CALLS {
            tracing::warn!("LE Audio: CCP: incoming call from {uri} refused, {MAX_CALLS} calls already");
            return None;
        }
        let index = self.allocate_index();
        if index == 0 {
            return None;
        }
        let call = Call {
            index,
            state: CallState::Incoming,
            flags: 0,
            uri: truncate(uri, MAX_URI).into(),
            friendly_name: truncate(friendly_name, 255).into(),
            target_uri: truncate(target_uri, 254).into(),
        };
        if !call.friendly_name.is_empty() {
            self.friendly_index = index;
        }
        tracing::info!(
            "LE Audio: CCP: incoming call {index} from {}{}{}",
            call.uri,
            if call.friendly_name.is_empty() { "" } else { " " },
            call.friendly_name
        );
        self.calls.push(call);
        self.changes.calls = true;
        self.changes.incoming = Some(index);
        self.log_calls("incoming call");
        Some(index)
    }

    /// An outgoing call rings at the far end (Dialing -> Alerting)
    pub fn remote_alerting(&mut self, index: u8) -> bool {
        let Some(c) = self.find_mut(index) else { return false };
        if c.state != CallState::Dialing {
            tracing::debug!("LE Audio: CCP: remote alerting for call {index} ignored (state {:?})", c.state);
            return false;
        }
        c.state = CallState::Alerting;
        self.changes.calls = true;
        self.log_calls("remote alerting");
        true
    }

    /// The far end answered an outgoing call (Dialing / Alerting -> Active)
    pub fn remote_answered(&mut self, index: u8) -> bool {
        let Some(c) = self.find_mut(index) else { return false };
        if c.state != CallState::Dialing && c.state != CallState::Alerting {
            tracing::debug!("LE Audio: CCP: remote answer for call {index} ignored (state {:?})", c.state);
            return false;
        }
        c.state = CallState::Active;
        self.hold_others(index);
        self.changes.calls = true;
        self.log_calls("remote answer");
        true
    }

    /// The far end put the call on hold / took it back
    pub fn remote_hold(&mut self, index: u8, held: bool) -> bool {
        let Some(c) = self.find_mut(index) else { return false };
        let next = match (held, c.state) {
            (true, CallState::Active) => CallState::RemotelyHeld,
            (true, CallState::LocallyHeld) => CallState::LocallyAndRemotelyHeld,
            (false, CallState::RemotelyHeld) => CallState::Active,
            (false, CallState::LocallyAndRemotelyHeld) => CallState::LocallyHeld,
            (_, state) => {
                tracing::debug!("LE Audio: CCP: remote hold {held} for call {index} ignored (state {state:?})");
                return false;
            }
        };
        c.state = next;
        if next == CallState::Active {
            self.hold_others(index);
        }
        self.changes.calls = true;
        self.log_calls(if held { "remote hold" } else { "remote retrieve" });
        true
    }

    /// The call ended (REASON_*)
    pub fn ended(&mut self, index: u8, reason: u8) -> bool {
        if self.find(index).is_none() {
            return false;
        }
        self.remove(index, reason);
        self.log_calls("call end");
        true
    }

    // ---- Requests of the headset (Call Control Point): RESULT_* ----

    /// Accept an incoming call (other active calls go on local hold)
    pub fn accept(&mut self, backend: &mut dyn Backend, index: u8) -> u8 {
        let Some(c) = self.find(index) else { return RESULT_INVALID_CALL_INDEX };
        if c.state != CallState::Incoming {
            return RESULT_STATE_MISMATCH;
        }
        let others_active = self.calls.iter().any(|o| o.index != index && o.state == CallState::Active);
        if others_active && !backend.supports_local_hold() {
            return RESULT_OPERATION_NOT_POSSIBLE;
        }
        let r = backend.accept(c);
        if r != RESULT_SUCCESS {
            tracing::debug!("LE Audio: CCP: backend refused to accept call {index} ({})", result_name(r));
            return r;
        }
        if let Some(c) = self.find_mut(index) {
            c.state = CallState::Active;
        }
        self.hold_others(index);
        self.changes.calls = true;
        RESULT_SUCCESS
    }

    /// Terminate any call (Termination Reason: the client terminated it)
    pub fn terminate(&mut self, backend: &mut dyn Backend, index: u8) -> u8 {
        let Some(c) = self.find(index) else { return RESULT_INVALID_CALL_INDEX };
        let r = backend.terminate(c);
        if r != RESULT_SUCCESS {
            return r;
        }
        self.remove(index, REASON_CLIENT_TERMINATED);
        RESULT_SUCCESS
    }

    /// Incoming / Active -> Locally Held, Remotely Held -> Locally and
    /// Remotely Held; Opcode Not Supported if the backend has no local hold
    pub fn local_hold(&mut self, backend: &mut dyn Backend, index: u8) -> u8 {
        if !backend.supports_local_hold() {
            return RESULT_OPCODE_NOT_SUPPORTED;
        }
        let Some(c) = self.find(index) else { return RESULT_INVALID_CALL_INDEX };
        let next = match c.state {
            CallState::Incoming | CallState::Active => CallState::LocallyHeld,
            CallState::RemotelyHeld => CallState::LocallyAndRemotelyHeld,
            _ => return RESULT_STATE_MISMATCH,
        };
        let r = backend.hold(c);
        if r != RESULT_SUCCESS {
            return r;
        }
        if let Some(c) = self.find_mut(index) {
            c.state = next;
        }
        self.changes.calls = true;
        RESULT_SUCCESS
    }

    /// Locally Held -> Active (the others go on hold), Locally and Remotely
    /// Held -> Remotely Held; Opcode Not Supported if the backend has no
    /// local hold
    pub fn local_retrieve(&mut self, backend: &mut dyn Backend, index: u8) -> u8 {
        if !backend.supports_local_hold() {
            return RESULT_OPCODE_NOT_SUPPORTED;
        }
        let Some(c) = self.find(index) else { return RESULT_INVALID_CALL_INDEX };
        let next = match c.state {
            CallState::LocallyHeld => CallState::Active,
            CallState::LocallyAndRemotelyHeld => CallState::RemotelyHeld,
            _ => return RESULT_STATE_MISMATCH,
        };
        let r = backend.retrieve(c);
        if r != RESULT_SUCCESS {
            return r;
        }
        if let Some(c) = self.find_mut(index) {
            c.state = next;
        }
        if next == CallState::Active {
            self.hold_others(index);
        }
        self.changes.calls = true;
        RESULT_SUCCESS
    }

    /// Originate: (RESULT_*, the new call's index or 0). The URI must be
    /// `<scheme>:<something>` with a scheme of the backend's list, at most
    /// [`MAX_URI`] octets, without control characters.
    pub fn originate(&mut self, backend: &mut dyn Backend, uri: &str) -> (u8, u8) {
        let Some(colon) = uri.find(':') else { return (RESULT_INVALID_OUTGOING_URI, 0) };
        if colon == 0 || colon + 1 >= uri.len() || uri.len() > MAX_URI {
            return (RESULT_INVALID_OUTGOING_URI, 0);
        }
        let scheme = uri[..colon].to_ascii_lowercase();
        let known = backend.uri_schemes().split(',').any(|s| s.trim().to_ascii_lowercase() == scheme);
        if !known || uri.bytes().any(|b| b < 0x20) {
            tracing::debug!(
                "LE Audio: CCP: originate refused: scheme {scheme:?} known {known} (backend schemes {:?})",
                backend.uri_schemes()
            );
            return (RESULT_INVALID_OUTGOING_URI, 0);
        }
        if self.calls.len() >= MAX_CALLS {
            return (RESULT_LACK_OF_RESOURCES, 0);
        }
        let others_active = self.calls.iter().any(|o| o.state == CallState::Active);
        if others_active && !backend.supports_local_hold() {
            return (RESULT_OPERATION_NOT_POSSIBLE, 0);
        }
        let r = backend.originate(uri);
        if r != RESULT_SUCCESS {
            return (r, 0);
        }
        let index = self.allocate_index();
        if index == 0 {
            return (RESULT_LACK_OF_RESOURCES, 0);
        }
        self.hold_others(index);
        self.calls.push(Call {
            index,
            state: CallState::Dialing,
            flags: CALL_FLAG_OUTGOING,
            uri: uri.into(),
            friendly_name: String::new(),
            target_uri: String::new(),
        });
        self.changes.calls = true;
        (RESULT_SUCCESS, index)
    }

    /// A Call Control Point write (v1 on_cp without the GATT part): Err
    /// with an ATT error (wrong length: 0x0D, nothing is notified), else
    /// what to notify to the writer. Join and unknown opcodes: Opcode Not
    /// Supported. The module logs "LE Audio: CCP: control point <op_name>
    /// (0x..) call N from 0x.... -> <result_name>", sends the changes, the
    /// result, and on success calls [`CallModel::log_calls`] with the
    /// opcode's name.
    pub fn control_point(&mut self, backend: &mut dyn Backend, value: &[u8]) -> Result<CpOutcome, u8> {
        let Some(&opcode) = value.first() else { return Err(ATT_ERROR_INVALID_ATTRIBUTE_VALUE_LENGTH) };
        tracing::debug!("LE Audio: CCP: control point write {value:02X?}");
        let (call_index, result) = match opcode {
            OP_ACCEPT | OP_TERMINATE | OP_LOCAL_HOLD | OP_LOCAL_RETRIEVE => {
                if value.len() != 2 {
                    return Err(ATT_ERROR_INVALID_ATTRIBUTE_VALUE_LENGTH);
                }
                let index = value[1];
                let result = match opcode {
                    OP_ACCEPT => self.accept(backend, index),
                    OP_TERMINATE => self.terminate(backend, index),
                    OP_LOCAL_HOLD => self.local_hold(backend, index),
                    _ => self.local_retrieve(backend, index),
                };
                (index, result)
            }
            OP_ORIGINATE => {
                if value.len() < 2 {
                    return Err(ATT_ERROR_INVALID_ATTRIBUTE_VALUE_LENGTH);
                }
                let raw = &value[1..];
                if raw.len() > MAX_URI {
                    (0, RESULT_INVALID_OUTGOING_URI)
                } else {
                    let (result, index) = self.originate(backend, &String::from_utf8_lossy(raw));
                    (index, result)
                }
            }
            _ => (0, RESULT_OPCODE_NOT_SUPPORTED),
        };
        Ok(CpOutcome { opcode, call_index, result })
    }

    /// Notifications due since the last call
    pub fn take_changes(&mut self) -> Changes {
        std::mem::take(&mut self.changes)
    }

    // ---- Characteristic values ----

    /// Bearer List Current Calls
    pub fn list_value(&self) -> Vec<u8> {
        let mut v = Vec::new();
        for c in &self.calls {
            v.extend_from_slice(&[(3 + c.uri.len()) as u8, c.index, c.state.value(), c.flags]);
            v.extend_from_slice(c.uri.as_bytes());
        }
        v
    }

    /// Call State
    pub fn state_value(&self) -> Vec<u8> {
        self.calls.iter().flat_map(|c| [c.index, c.state.value(), c.flags]).collect()
    }

    /// Incoming Call (the latest call still ringing in; empty if none)
    pub fn incoming_value(&self) -> Vec<u8> {
        self.calls
            .iter()
            .rev()
            .find(|c| c.state == CallState::Incoming)
            .map(|c| index_and(c.index, &c.uri))
            .unwrap_or_default()
    }

    /// Incoming Call Target Bearer URI (of the latest call ringing in that
    /// has one)
    pub fn target_value(&self) -> Vec<u8> {
        self.calls
            .iter()
            .rev()
            .find(|c| c.state == CallState::Incoming && !c.target_uri.is_empty())
            .map(|c| index_and(c.index, &c.target_uri))
            .unwrap_or_default()
    }

    /// Call Friendly Name (of the latest incoming call that had one, while
    /// it lasts)
    pub fn friendly_value(&self) -> Vec<u8> {
        self.calls
            .iter()
            .find(|c| c.index == self.friendly_index && !c.friendly_name.is_empty())
            .map(|c| index_and(c.index, &c.friendly_name))
            .unwrap_or_default()
    }

    // ---- Simulation ----

    /// One simulation command (see the module docs); Err with why it
    /// cannot be done (v1's texts, e.g. "no such call"). Ok with the calls
    /// afterwards ([`CallModel::describe_calls`], already logged).
    pub fn simulate(&mut self, command: &str) -> Result<String, String> {
        let line = command.trim();
        if line.is_empty() {
            return Err("empty command".into());
        }
        let (cmd, rest) = match line.find([' ', '\t']) {
            Some(sp) => (&line[..sp], line[sp + 1..].trim()),
            None => (line, ""),
        };
        let cmd = cmd.to_ascii_lowercase();
        tracing::info!("LE Audio: CCP: simulate \"{line}\"");

        if cmd == "incoming" {
            if rest.is_empty() {
                return Err("incoming needs a URI".into());
            }
            let mut parts = rest.split('|').map(str::trim);
            let uri = parts.next().unwrap_or("");
            let name = parts.next().unwrap_or("");
            let target = parts.next().unwrap_or("");
            return match self.incoming(uri, name, target) {
                Some(_) => Ok(self.describe_calls()),
                None => Err("no room for another call".into()),
            };
        }
        if cmd == "status" {
            self.log_calls("status");
            return Ok(self.describe_calls());
        }
        // [<index> [<reason>]]
        let mut args = rest.split_whitespace();
        let a = args.next();
        let b = args.next();
        let index = match a {
            Some(a) => c_strtoul(a) as u8,
            None => self.latest().map_or(0, |c| c.index),
        };
        if self.find(index).is_none() {
            return Err("no such call".into());
        }
        let ok = match cmd.as_str() {
            "alerting" => self.remote_alerting(index),
            "answered" => self.remote_answered(index),
            "remote-hold" => self.remote_hold(index, true),
            "remote-retrieve" => self.remote_hold(index, false),
            "hangup" => self.ended(index, b.map_or(REASON_REMOTE_ENDED, |b| c_strtoul(b) as u8)),
            _ => return Err("unknown command".into()),
        };
        if ok { Ok(self.describe_calls()) } else { Err("not possible in the call's state".into()) }
    }
}

/// One step of a `--ccp-sim` script
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimStep {
    /// Milliseconds after the first CCP client enabled Call State
    /// notifications
    pub at_ms: u32,
    pub command: String,
}

/// Parses `--ccp-sim "<t>:<cmd>;<t>:<cmd>;..."` (t in seconds, may have a
/// fraction), sorted by time (stable); empty steps are skipped. None if a
/// step has no time (v1 logged `--ccp-sim step "..." has no time
/// (<seconds>:<command>)` and skipped it, and took a time it could not
/// read as 0).
pub fn parse_sim_script(spec: &str) -> Option<Vec<SimStep>> {
    let mut steps = Vec::new();
    for step in spec.split(';').map(str::trim).filter(|s| !s.is_empty()) {
        let (t, command) = step.split_once(':')?;
        let t: f64 = t.trim().parse().ok().filter(|t: &f64| t.is_finite() && *t >= 0.0)?;
        steps.push(SimStep { at_ms: (t * 1000.0) as u32, command: command.trim().into() });
    }
    steps.sort_by_key(|s| s.at_ms);
    Some(steps)
}

/// `s` cut to at most `max` octets at a character boundary
fn truncate(s: &str, max: usize) -> &str {
    let mut n = s.len().min(max);
    while !s.is_char_boundary(n) {
        n -= 1;
    }
    &s[..n]
}

fn index_and(index: u8, text: &str) -> Vec<u8> {
    let mut v = vec![index];
    v.extend_from_slice(text.as_bytes());
    v
}

/// C's strtoul(s, nullptr, 0) as v1 used it: optional sign, "0x" hex, a
/// leading 0 octal, else decimal; digits up to the first other character,
/// 0 if none; saturates at u32::MAX (Windows' unsigned long)
fn c_strtoul(s: &str) -> u32 {
    let s = s.trim_start();
    let (neg, s) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let (radix, digits) =
        if (s.starts_with("0x") || s.starts_with("0X")) && s[2..].starts_with(|c: char| c.is_ascii_hexdigit()) {
            (16, &s[2..])
        } else if s.starts_with('0') {
            (8, s)
        } else {
            (10, s)
        };
    let mut v: u64 = 0;
    for c in digits.chars() {
        let Some(d) = c.to_digit(radix) else { break };
        v = (v * radix as u64 + d as u64).min(u64::from(u32::MAX) + 1);
    }
    let v = v.min(u64::from(u32::MAX)) as u32;
    if neg { v.wrapping_neg() } else { v }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A backend without local hold that refuses Originate
    struct Strict;

    impl Backend for Strict {
        fn supports_local_hold(&self) -> bool {
            false
        }
        fn originate(&mut self, _uri: &str) -> u8 {
            RESULT_OPERATION_NOT_POSSIBLE
        }
    }

    fn states(m: &CallModel) -> Vec<(u8, CallState)> {
        m.calls().iter().map(|c| (c.index, c.state)).collect()
    }

    use CallState::*;

    #[test]
    fn incoming_accept_hold_retrieve_hangup() {
        // ccp-incoming-accept: incoming, Accept, Local Hold, Local
        // Retrieve, the far end hangs up (reason 0x02)
        let mut m = CallModel::new();
        let mut sim = SimBackend;
        assert!(m.simulate("incoming tel:+15551234567|Caller Name|tel:+15550000000").is_ok());
        assert_eq!(states(&m), [(1, Incoming)]);
        assert_eq!(m.take_changes(), Changes { calls: true, terminations: vec![], incoming: Some(1) });
        assert_eq!(m.state_value(), [1, 0x00, 0]);
        assert_eq!(m.list_value(), [[19, 1, 0, 0].as_slice(), b"tel:+15551234567"].concat());
        assert_eq!(m.incoming_value(), [[1].as_slice(), b"tel:+15551234567"].concat());
        assert_eq!(m.target_value(), [[1].as_slice(), b"tel:+15550000000"].concat());
        assert_eq!(m.friendly_value(), [[1].as_slice(), b"Caller Name"].concat());

        assert_eq!(m.control_point(&mut sim, &[OP_ACCEPT, 1]), Ok(CpOutcome { opcode: 0, call_index: 1, result: 0 }));
        assert_eq!(states(&m), [(1, Active)]);
        assert_eq!(m.incoming_value(), []);
        assert_eq!(m.target_value(), []);
        assert_eq!(m.friendly_value(), [[1].as_slice(), b"Caller Name"].concat());
        assert_eq!(m.take_changes(), Changes { calls: true, ..Changes::default() });

        assert_eq!(m.local_hold(&mut sim, 1), RESULT_SUCCESS);
        assert_eq!(states(&m), [(1, LocallyHeld)]);
        assert_eq!(m.local_retrieve(&mut sim, 1), RESULT_SUCCESS);
        assert_eq!(states(&m), [(1, Active)]);
        m.take_changes();

        assert_eq!(m.simulate("hangup"), Ok("none".into()));
        assert_eq!(m.calls(), []);
        assert_eq!(
            m.take_changes(),
            Changes { calls: true, terminations: vec![(1, REASON_REMOTE_ENDED)], incoming: None }
        );
        assert_eq!(m.friendly_value(), []);
        assert_eq!(m.state_value(), []);
    }

    #[test]
    fn two_calls_and_terminate() {
        // ccp-terminate: two incoming calls, accept 1, terminate 2 then 1
        let mut m = CallModel::new();
        let mut sim = SimBackend;
        assert_eq!(m.incoming("tel:1", "", ""), Some(1));
        assert_eq!(m.accept(&mut sim, 1), RESULT_SUCCESS);
        assert_eq!(m.incoming("tel:2", "", ""), Some(2));
        assert_eq!(m.target_value(), []);
        assert_eq!(m.friendly_value(), []);
        assert_eq!(
            m.list_value(),
            [8, 1, 3, 0, b't', b'e', b'l', b':', b'1', 8, 2, 0, 0, b't', b'e', b'l', b':', b'2']
        );
        // Accepting 2 holds 1
        assert_eq!(m.accept(&mut sim, 2), RESULT_SUCCESS);
        assert_eq!(states(&m), [(1, LocallyHeld), (2, Active)]);
        m.take_changes();
        assert_eq!(m.terminate(&mut sim, 2), RESULT_SUCCESS);
        assert_eq!(m.terminate(&mut sim, 1), RESULT_SUCCESS);
        assert_eq!(
            m.take_changes(),
            Changes {
                calls: true,
                terminations: vec![(2, REASON_CLIENT_TERMINATED), (1, REASON_CLIENT_TERMINATED)],
                incoming: None
            }
        );
        // Indices go on counting
        assert_eq!(m.incoming("tel:3", "", ""), Some(3));
    }

    #[test]
    fn invalid_requests() {
        // ccp-invalid
        let mut m = CallModel::new();
        let mut sim = SimBackend;
        let cp = |m: &mut CallModel, sim: &mut SimBackend, v: &[u8]| m.control_point(sim, v);
        let outcome = |opcode, call_index, result| Ok(CpOutcome { opcode, call_index, result });
        assert_eq!(cp(&mut m, &mut sim, &[OP_ACCEPT, 9]), outcome(0, 9, RESULT_INVALID_CALL_INDEX));
        assert_eq!(cp(&mut m, &mut sim, &[OP_TERMINATE, 0]), outcome(1, 0, RESULT_INVALID_CALL_INDEX));
        assert_eq!(cp(&mut m, &mut sim, &[0x07]), outcome(7, 0, RESULT_OPCODE_NOT_SUPPORTED));
        assert_eq!(cp(&mut m, &mut sim, &[OP_JOIN, 1]), outcome(5, 0, RESULT_OPCODE_NOT_SUPPORTED));
        assert_eq!(cp(&mut m, &mut sim, &[OP_ACCEPT]), Err(0x0D));
        assert_eq!(cp(&mut m, &mut sim, &[OP_ACCEPT, 1, 2]), Err(0x0D));
        assert_eq!(cp(&mut m, &mut sim, &[OP_ORIGINATE]), Err(0x0D));
        assert_eq!(cp(&mut m, &mut sim, &[]), Err(0x0D));
        for bad in [&b"nocolon"[..], b"mailto:x", b":x", b"tel:", b"tel:1\n2", &[b't'; 300]] {
            let mut v = vec![OP_ORIGINATE];
            v.extend_from_slice(bad);
            assert_eq!(cp(&mut m, &mut sim, &v), outcome(4, 0, RESULT_INVALID_OUTGOING_URI), "{bad:?}");
        }
        let mut v = vec![OP_ORIGINATE];
        v.extend_from_slice(b"TEL:+15551234567");
        assert_eq!(cp(&mut m, &mut sim, &v), outcome(4, 1, RESULT_SUCCESS));
        assert_eq!(m.calls()[0].flags, CALL_FLAG_OUTGOING);
        assert_eq!(states(&m), [(1, Dialing)]);
        assert_eq!(cp(&mut m, &mut sim, &[OP_ACCEPT, 1]), outcome(0, 1, RESULT_STATE_MISMATCH));
        assert_eq!(cp(&mut m, &mut sim, &[OP_LOCAL_RETRIEVE, 1]), outcome(3, 1, RESULT_STATE_MISMATCH));
        assert_eq!(cp(&mut m, &mut sim, &[OP_LOCAL_HOLD, 1]), outcome(2, 1, RESULT_STATE_MISMATCH));
        assert_eq!(m.simulate("alerting"), Ok("1 Alerting (outgoing)".into()));
        assert_eq!(m.simulate("alerting"), Err("not possible in the call's state".into()));
        assert_eq!(m.simulate("answered"), Ok("1 Active (outgoing)".into()));
        assert_eq!(cp(&mut m, &mut sim, &[OP_LOCAL_HOLD, 1]), outcome(2, 1, RESULT_SUCCESS));
        assert_eq!(cp(&mut m, &mut sim, &[OP_LOCAL_RETRIEVE, 1]), outcome(3, 1, RESULT_SUCCESS));
        assert_eq!(cp(&mut m, &mut sim, &[OP_TERMINATE, 1]), outcome(1, 1, RESULT_SUCCESS));
        assert_eq!(m.take_changes().terminations, [(1, REASON_CLIENT_TERMINATED)]);
    }

    #[test]
    fn remote_hold_states() {
        let mut m = CallModel::new();
        let mut sim = SimBackend;
        m.incoming("tel:1", "", "");
        assert!(!m.remote_hold(1, true));
        m.accept(&mut sim, 1);
        assert!(m.remote_hold(1, true));
        assert_eq!(states(&m), [(1, RemotelyHeld)]);
        assert_eq!(m.local_hold(&mut sim, 1), RESULT_SUCCESS);
        assert_eq!(states(&m), [(1, LocallyAndRemotelyHeld)]);
        assert!(!m.remote_hold(1, true));
        assert!(m.remote_hold(1, false));
        assert_eq!(states(&m), [(1, LocallyHeld)]);
        assert!(m.remote_hold(1, true));
        assert_eq!(m.local_retrieve(&mut sim, 1), RESULT_SUCCESS);
        assert_eq!(states(&m), [(1, RemotelyHeld)]);
        // A second call accepted: the remotely held one is now held
        // locally too
        m.incoming("tel:2", "", "");
        assert_eq!(m.accept(&mut sim, 2), RESULT_SUCCESS);
        assert_eq!(states(&m), [(1, LocallyAndRemotelyHeld), (2, Active)]);
        // The far end takes it back: still held here
        assert!(m.remote_hold(1, false));
        assert_eq!(states(&m), [(1, LocallyHeld), (2, Active)]);
        // Retrieving it puts the other one on hold
        assert_eq!(m.local_retrieve(&mut sim, 1), RESULT_SUCCESS);
        assert_eq!(states(&m), [(1, Active), (2, LocallyHeld)]);
        // Local Hold of an incoming call
        m.incoming("tel:3", "", "");
        assert_eq!(m.local_hold(&mut sim, 3), RESULT_SUCCESS);
        assert_eq!(states(&m), [(1, Active), (2, LocallyHeld), (3, LocallyHeld)]);
        assert_eq!(m.local_hold(&mut sim, 9), RESULT_INVALID_CALL_INDEX);
        assert_eq!(m.local_retrieve(&mut sim, 1), RESULT_STATE_MISMATCH);
        assert_eq!(m.local_hold(&mut sim, 2), RESULT_STATE_MISMATCH);
    }

    #[test]
    fn backend_refusals_and_limits() {
        let mut m = CallModel::new();
        let mut strict = Strict;
        let mut sim = SimBackend;
        for i in 1..=4 {
            assert_eq!(m.incoming(&format!("tel:{i}"), "", ""), Some(i));
        }
        assert_eq!(m.incoming("tel:5", "", ""), None);
        assert_eq!(m.simulate("incoming tel:5"), Err("no room for another call".into()));
        assert_eq!(m.originate(&mut sim, "tel:9"), (RESULT_LACK_OF_RESOURCES, 0));
        assert_eq!(m.accept(&mut strict, 1), RESULT_SUCCESS);
        // Another call active and no local hold: not possible
        assert_eq!(m.accept(&mut strict, 2), RESULT_OPERATION_NOT_POSSIBLE);
        assert_eq!(m.local_hold(&mut strict, 1), RESULT_OPCODE_NOT_SUPPORTED);
        assert_eq!(m.control_point(&mut strict, &[OP_LOCAL_RETRIEVE, 1]).unwrap().result, RESULT_OPCODE_NOT_SUPPORTED);
        m.ended(2, REASON_SERVER_ENDED);
        m.ended(3, REASON_SERVER_ENDED);
        m.ended(4, REASON_SERVER_ENDED);
        assert_eq!(m.originate(&mut strict, "tel:9"), (RESULT_OPERATION_NOT_POSSIBLE, 0));
        m.ended(1, REASON_SERVER_ENDED);
        assert_eq!(m.originate(&mut strict, "tel:9"), (RESULT_OPERATION_NOT_POSSIBLE, 0));
        assert_eq!(m.originate(&mut strict, "sip:x"), (RESULT_OPERATION_NOT_POSSIBLE, 0));
        assert_eq!(m.calls(), []);
        assert!(!m.ended(1, 0));
    }

    #[test]
    fn index_allocation_wraps() {
        let mut m = CallModel::new();
        m.next_index = 254;
        assert_eq!(m.incoming("tel:a", "", ""), Some(254));
        assert_eq!(m.incoming("tel:b", "", ""), Some(255));
        assert_eq!(m.incoming("tel:c", "", ""), Some(1));
        m.next_index = 254;
        assert_eq!(m.incoming("tel:d", "", ""), Some(2));
    }

    #[test]
    fn simulation_commands() {
        let mut m = CallModel::new();
        assert_eq!(m.simulate("  "), Err("empty command".into()));
        assert_eq!(m.simulate("incoming"), Err("incoming needs a URI".into()));
        assert_eq!(m.simulate("alerting"), Err("no such call".into()));
        assert_eq!(m.simulate("status"), Ok("none".into()));
        assert_eq!(m.simulate("INCOMING  tel:1 | Name "), Ok("1 Incoming".into()));
        assert_eq!(m.calls()[0].friendly_name, "Name");
        assert_eq!(m.simulate("bogus"), Err("unknown command".into()));
        assert_eq!(m.simulate("hangup 7"), Err("no such call".into()));
        assert_eq!(m.simulate("hangup 0x01 3"), Ok("none".into()));
        assert_eq!(m.take_changes().terminations, [(1, REASON_SERVER_ENDED)]);
        // Long URIs and names are cut at a character boundary
        let long = format!("tel:{}", "é".repeat(200));
        m.simulate(&format!("incoming {long}|{}", "ü".repeat(200))).unwrap();
        let c = m.latest().unwrap();
        assert_eq!((c.uri.len(), c.friendly_name.len()), (252, 254));
    }

    #[test]
    fn strtoul_like_v1() {
        assert_eq!(c_strtoul("12"), 12);
        assert_eq!(c_strtoul("0x1f"), 31);
        assert_eq!(c_strtoul("010"), 8);
        assert_eq!(c_strtoul("3abc"), 3);
        assert_eq!(c_strtoul("abc"), 0);
        assert_eq!(c_strtoul("0x"), 0);
        assert_eq!(c_strtoul("99999999999") as u8, 255);
        assert_eq!(c_strtoul("-1") as u8, 255);
    }

    #[test]
    fn sim_scripts() {
        assert_eq!(
            parse_sim_script("4:hangup; 1:incoming tel:1|A|B ;;2.5 : answered"),
            Some(vec![
                SimStep { at_ms: 1000, command: "incoming tel:1|A|B".into() },
                SimStep { at_ms: 2500, command: "answered".into() },
                SimStep { at_ms: 4000, command: "hangup".into() },
            ])
        );
        assert_eq!(parse_sim_script(""), Some(vec![]));
        assert_eq!(parse_sim_script("1:a;1:b").unwrap()[1].command, "b");
        assert_eq!(parse_sim_script("incoming tel:1"), None);
        assert_eq!(parse_sim_script("x:status"), None);
        assert_eq!(parse_sim_script("-1:status"), None);
    }

    #[test]
    fn names_and_values() {
        assert_eq!(LocallyAndRemotelyHeld.value(), 6);
        assert_eq!(LocallyHeld.name(), "Locally Held");
        assert_eq!(op_name(OP_LOCAL_RETRIEVE), "Local Retrieve");
        assert_eq!(op_name(0x07), "unknown opcode");
        assert_eq!(result_name(RESULT_INVALID_OUTGOING_URI), "Invalid Outgoing URI");
        let b = SimBackend;
        assert_eq!(
            (b.provider_name(), b.uci(), b.technology(), b.uri_schemes()),
            ("A2DPWB".into(), "un000".into(), 4, "tel,sip".into())
        );
        assert!(b.supports_local_hold());
    }
}
