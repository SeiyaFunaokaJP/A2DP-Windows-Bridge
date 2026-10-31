//! Diagnostics: the connection flow (11 steps recognised from log lines) and
//! the "copy diagnostic info" text — v1's debug_log_model.cpp and the copies
//! of wx_debug_console_dialog.cpp.
//!
//! v1 tailed debug.log and recognised its milestones by wording. v2 gets the
//! log as tracing events: [`ConnectionFlow::feed`] takes each line (target,
//! level, message) and matches the messages bt-host, audio-capture and core
//! print today, by target and message prefix. Keep the rules in sync with
//! those messages (bt-host/src/lib.rs and app.rs, core/src/stream.rs and
//! service.rs, audio-capture/src/loopback.rs).
//!
//! Copied text is English (bug reports) and can mask Bluetooth addresses as
//! v1 did: "AA:BB:CC:xx:xx:xx" keeps the vendor part (OUI).

use std::fmt::Write as _;
use std::path::Path;

use bt_host::LinkStats;
use serde::Serialize;
use tracing::Level;

use crate::messages::Msg;
use crate::profiles::Profile;
use crate::settings::Settings;
use crate::stream::{StreamInfo, StreamState};

/// Warnings and errors of the current attempt in the copied diagnostics
const DIAG_ISSUES: usize = 40;
/// Log lines of the current attempt in the copied diagnostics
const DIAG_LINES: usize = 200;
/// Adapter / controller lines kept for the diagnostics
const MAX_ADAPTER_LINES: usize = 16;

/// A connection attempt's milestones, in order
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    /// The session started (profile / request)
    Start,
    /// Adapter, HCI power on
    Btstack,
    /// ACL + AVDTP signaling
    Connect,
    /// Capability discovery
    Caps,
    /// Codec selection
    Codec,
    /// WASAPI capture opened
    Capture,
    /// SET_CONFIGURATION / OPEN
    Config,
    /// Encoder created
    Encoder,
    /// AVDTP START
    Stream,
    /// Capture running, packets flowing
    Audio,
    /// Disconnects / reconnects while streaming
    Link,
}

pub const STEPS: [Step; 11] = [
    Step::Start,
    Step::Btstack,
    Step::Connect,
    Step::Caps,
    Step::Codec,
    Step::Capture,
    Step::Config,
    Step::Encoder,
    Step::Stream,
    Step::Audio,
    Step::Link,
];

impl Step {
    fn index(self) -> usize {
        self as usize
    }

    /// English name (copied text)
    pub fn name(self) -> &'static str {
        match self {
            Step::Start => "Start (profile)",
            Step::Btstack => "Bluetooth adapter (BTstack)",
            Step::Connect => "Connect (ACL / AVDTP signaling)",
            Step::Caps => "Capability discovery",
            Step::Codec => "Codec selection",
            Step::Capture => "Audio capture (WASAPI)",
            Step::Config => "Stream configuration",
            Step::Encoder => "Encoder",
            Step::Stream => "Stream start (AVDTP START)",
            Step::Audio => "Audio sending",
            Step::Link => "Connection upkeep",
        }
    }

    /// Translation key (app/lang, shared with v1)
    pub fn key(self) -> &'static str {
        match self {
            Step::Start => "dbgc.step.start",
            Step::Btstack => "dbgc.step.btstack",
            Step::Connect => "dbgc.step.connect",
            Step::Caps => "dbgc.step.caps",
            Step::Codec => "dbgc.step.codec",
            Step::Capture => "dbgc.step.capture",
            Step::Config => "dbgc.step.config",
            Step::Encoder => "dbgc.step.encoder",
            Step::Stream => "dbgc.step.stream",
            Step::Audio => "dbgc.step.audio",
            Step::Link => "dbgc.step.link",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    #[default]
    Pending,
    Running,
    Done,
    Warning,
    Failed,
}

impl StepStatus {
    /// Done or done with a warning
    pub fn finished(self) -> bool {
        matches!(self, StepStatus::Done | StepStatus::Warning)
    }

    /// Fixed-width marker of the copied flow
    pub fn tag(self) -> &'static str {
        match self {
            StepStatus::Pending => "[ -- ]",
            StepStatus::Running => "[ .. ]",
            StepStatus::Done => "[ OK ]",
            StepStatus::Warning => "[WARN]",
            StepStatus::Failed => "[FAIL]",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            StepStatus::Pending => "dbgc.status.pending",
            StepStatus::Running => "dbgc.status.running",
            StepStatus::Done => "dbgc.status.done",
            StepStatus::Warning => "dbgc.status.warning",
            StepStatus::Failed => "dbgc.status.failed",
        }
    }
}

/// One step of the current attempt, as the log set it
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowStep {
    pub status: StepStatus,
    /// When the status last changed (time_ms of the line)
    pub time_ms: Option<u64>,
    pub detail: String,
    /// The line that set the status ([`ConnectionFlow::lines`] count), 0 =
    /// none
    pub seq: u64,
}

/// A connection attempt (one streaming session)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Session {
    /// 1, 2, ...; 0 = no attempt yet
    pub id: u32,
    pub start_ms: u64,
    /// When the session ended (stopped, or failed for good)
    pub stop_ms: Option<u64>,
    /// First line of the attempt
    pub first_seq: u64,
    pub reconnects: u32,
    /// Connection attempts that failed before one succeeded
    pub connect_retries: u32,
    /// Warning and error lines since the start
    pub issues: u32,
    pub steps: [FlowStep; 11],
}

impl Session {
    pub fn step(&self, s: Step) -> &FlowStep {
        &self.steps[s.index()]
    }

    pub fn active(&self) -> bool {
        self.id != 0 && self.stop_ms.is_none()
    }
}

/// What the streaming loop moves right now (packet and capture rates over
/// the last second), for the "Audio sending" step
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct AudioRates {
    pub packets_per_second: f64,
    pub kbps: f64,
    pub capture_frames_per_second: f64,
}

/// A step as shown: the log's status plus what the live state adds
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FlowRow {
    pub step: Step,
    pub status: StepStatus,
    pub time_ms: Option<u64>,
    /// Since the attempt started
    pub elapsed_ms: Option<u64>,
    /// English
    pub detail: String,
    /// The detail as a translatable message, where it is computed rather
    /// than taken from the log
    pub detail_msg: Option<Msg>,
    pub seq: u64,
}

#[derive(Clone, Copy)]
enum Action {
    Done,
    Running,
    Warning,
    Failed,
    Detail,
    /// A failed connection attempt that is retried
    Retry,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Match {
    Prefix,
    Contains,
}

#[derive(Clone, Copy)]
enum DetailFrom {
    None,
    After,
    Whole,
}

struct Rule {
    /// Target prefix ("bt_host" covers bt_host::app)
    target: &'static str,
    how: Match,
    needle: &'static str,
    step: Step,
    action: Action,
    detail: DetailFrom,
}

const fn rule(
    target: &'static str,
    how: Match,
    needle: &'static str,
    step: Step,
    action: Action,
    detail: DetailFrom,
) -> Rule {
    Rule { target, how, needle, step, action, detail }
}

const BT: &str = "bt_host";
const STREAM: &str = "a2dpwb_core::stream";
const CAPTURE: &str = "audio_capture";

use Action as A;
use DetailFrom as D;
use Match::{Contains, Prefix};

/// Milestones. Failures that only show up as an error state are caught by
/// the "state Error" handling in feed(). First match wins.
const RULES: &[Rule] = &[
    rule(BT, Prefix, "hci_init done, powering on", Step::Btstack, A::Running, D::None),
    rule(BT, Prefix, "Intel adapter: checking bootloader firmware", Step::Btstack, A::Running, D::Whole),
    rule(BT, Prefix, "controller: ", Step::Btstack, A::Detail, D::After),
    rule(BT, Prefix, "HCI init took < 500 ms", Step::Btstack, A::Warning, D::Whole),
    rule(BT, Prefix, "Realtek firmware NOT loaded", Step::Btstack, A::Warning, D::Whole),
    rule(BT, Prefix, "firmware file missing: ", Step::Btstack, A::Warning, D::Whole),
    rule(BT, Contains, "needs a firmware download that is not implemented", Step::Btstack, A::Warning, D::Whole),
    rule(BT, Prefix, "could not open the Bluetooth adapter", Step::Btstack, A::Failed, D::Whole),
    rule(BT, Prefix, "HCI initialization failed", Step::Btstack, A::Failed, D::Whole),
    rule(BT, Prefix, "HCI initialization timed out", Step::Btstack, A::Failed, D::Whole),
    rule(BT, Prefix, "HCI ready", Step::Btstack, A::Done, D::None),
    rule(BT, Prefix, "connecting to ", Step::Connect, A::Running, D::After),
    rule(BT, Prefix, "HCI ACL connection established to ", Step::Connect, A::Detail, D::Whole),
    rule(BT, Prefix, "signaling connection established", Step::Connect, A::Done, D::Whole),
    rule(BT, Prefix, "connection attempt ", Step::Connect, A::Retry, D::Whole),
    rule(BT, Prefix, "authentication failed, dropping the stored link key", Step::Connect, A::Retry, D::Whole),
    rule(BT, Prefix, "authentication collided with the device's own", Step::Connect, A::Retry, D::Whole),
    rule(BT, Prefix, "pairing again failed: the dropped link key", Step::Connect, A::Detail, D::Whole),
    rule(BT, Prefix, "signaling connection failed", Step::Connect, A::Detail, D::Whole),
    rule(BT, Prefix, "connection timed out", Step::Connect, A::Detail, D::Whole),
    rule(BT, Prefix, "a2dp_source_establish_stream failed", Step::Connect, A::Detail, D::Whole),
    rule(BT, Prefix, "remote SEPs in use by another connection", Step::Caps, A::Warning, D::Whole),
    rule(BT, Prefix, "capability discovery complete", Step::Caps, A::Done, D::After),
    rule(STREAM, Prefix, "codec fallback: ", Step::Codec, A::Warning, D::After),
    rule(STREAM, Prefix, "codec selected: ", Step::Codec, A::Done, D::After),
    rule(CAPTURE, Prefix, "audio capture: loopback of ", Step::Capture, A::Detail, D::After),
    rule(CAPTURE, Prefix, "audio capture: process ", Step::Capture, A::Detail, D::After),
    rule(STREAM, Prefix, "failed to make \u{201C}", Step::Capture, A::Warning, D::Whole),
    rule(STREAM, Prefix, "capture: ", Step::Capture, A::Done, D::After),
    rule(STREAM, Prefix, "audio capture: ", Step::Capture, A::Failed, D::Whole),
    rule(BT, Prefix, "codec configured", Step::Config, A::Running, D::After),
    rule(BT, Prefix, "stream established", Step::Config, A::Done, D::After),
    rule(BT, Prefix, "stream configuration timed out", Step::Config, A::Failed, D::Whole),
    rule(BT, Prefix, "set configuration failed", Step::Config, A::Failed, D::Whole),
    rule(BT, Prefix, "stream establishment failed", Step::Config, A::Failed, D::Whole),
    rule(STREAM, Prefix, "encoder ready: ", Step::Encoder, A::Done, D::After),
    rule(STREAM, Prefix, "encoder: ", Step::Encoder, A::Failed, D::Whole),
    rule(BT, Prefix, "streaming started", Step::Stream, A::Done, D::None),
    rule(BT, Prefix, "start stream failed", Step::Stream, A::Failed, D::Whole),
    rule(BT, Prefix, "stream start timed out", Step::Stream, A::Failed, D::Whole),
    rule(CAPTURE, Prefix, "audio capture started: ", Step::Audio, A::Done, D::After),
    rule(CAPTURE, Prefix, "audio source: test tone at ", Step::Audio, A::Done, D::Whole),
    rule(STREAM, Prefix, "capturing app ", Step::Audio, A::Done, D::After),
    rule(STREAM, Contains, "is not running yet, waiting for it", Step::Audio, A::Running, D::Whole),
    rule(STREAM, Prefix, "audio capture start: ", Step::Audio, A::Failed, D::Whole),
    rule(STREAM, Contains, ", reconnecting", Step::Link, A::Running, D::Whole),
    rule(BT, Prefix, "reconnecting to ", Step::Link, A::Detail, D::Whole),
    rule(STREAM, Prefix, "reconnect attempt ", Step::Link, A::Detail, D::Whole),
    rule(STREAM, Prefix, "reconnected on attempt ", Step::Link, A::Warning, D::Whole),
];

/// Adapter and controller lines kept for the diagnostics (target, message
/// prefix: a newer line with the same prefix replaces the older one)
const ADAPTER_LINES: &[(&str, &str)] = &[
    ("a2dpwb_core::adapter", "chip: "),
    ("a2dpwb_core::adapter", "no Realtek chip "),
    (BT, "controller: "),
    (BT, "Realtek adapter PID "),
    (BT, "no Realtek PID configured"),
    (BT, "H4 over TCP"),
    (BT, "Realtek firmware "),
    (BT, "HCI init took < 500 ms"),
    (BT, "Intel "),
    (BT, "Broadcom "),
    (BT, "this vendor needs a firmware download"),
];

fn target_matches(target: &str, prefix: &str) -> bool {
    target.strip_prefix(prefix).is_some_and(|rest| rest.is_empty() || rest.starts_with("::"))
}

/// The text after `needle`, without a trailing "..." or surrounding
/// parentheses
fn after_needle(text: &str, needle: &str) -> String {
    let s = text.find(needle).map(|p| &text[p + needle.len()..]).unwrap_or("").trim();
    let s = s.strip_suffix("...").unwrap_or(s);
    let s = s.strip_prefix('(').and_then(|x| x.strip_suffix(')')).unwrap_or(s);
    s.to_string()
}

fn state_from_name(name: &str) -> Option<StreamState> {
    Some(match name {
        "Idle" => StreamState::Idle,
        "Connecting" => StreamState::Connecting,
        "Streaming" => StreamState::Streaming,
        "Reconnecting" => StreamState::Reconnecting,
        "Error" => StreamState::Error,
        _ => return None,
    })
}

fn state_name(s: StreamState) -> &'static str {
    match s {
        StreamState::Idle => "Idle",
        StreamState::Connecting => "Connecting",
        StreamState::Streaming => "Streaming",
        StreamState::Reconnecting => "Reconnecting",
        StreamState::Error => "Error",
    }
}

/// Follows the log and derives the steps of the current connection attempt.
/// Cheap per line (prefix checks; allocates only when a rule matches), so it
/// can be fed every event.
#[derive(Debug, Clone, Default)]
pub struct ConnectionFlow {
    session: Session,
    lines: u64,
    last_state: Option<(StreamState, String)>,
    adapter_lines: Vec<String>,
}

impl ConnectionFlow {
    pub fn new() -> ConnectionFlow {
        ConnectionFlow::default()
    }

    /// Builds a flow from lines kept elsewhere (e.g. the log ring buffer)
    pub fn from_lines<'a>(lines: impl IntoIterator<Item = LogEntry<'a>>) -> ConnectionFlow {
        let mut f = ConnectionFlow::new();
        for l in lines {
            f.feed(l.time_ms, l.target, l.level, l.message);
        }
        f
    }

    /// The current (or last) attempt
    pub fn session(&self) -> &Session {
        &self.session
    }

    /// Lines fed so far
    pub fn lines(&self) -> u64 {
        self.lines
    }

    /// The last service state the log reported, and its text
    pub fn last_state(&self) -> Option<(StreamState, &str)> {
        self.last_state.as_ref().map(|(s, t)| (*s, t.as_str()))
    }

    /// Controller, chip and firmware lines (newest of each kind)
    pub fn adapter_lines(&self) -> &[String] {
        &self.adapter_lines
    }

    /// One log line. `time_ms` is the line's time (ms since the process
    /// started, as ipc::LogLine).
    pub fn feed(&mut self, time_ms: u64, target: &str, level: Level, message: &str) {
        self.lines += 1;
        let seq = self.lines;
        let message = message.trim_end();
        self.track_adapter(target, message);

        // Service state: "state Connecting: Connecting to the device..."
        let mut state = None;
        if target_matches(target, "a2dpwb_core::service")
            && let Some(rest) = message.strip_prefix("state ")
            && let Some((name, text)) = rest.split_once(": ")
            && let Some(s) = state_from_name(name)
        {
            self.last_state = Some((s, text.to_string()));
            state = Some(s);
        }

        // A new attempt
        if target_matches(target, STREAM) && message.starts_with("stream: device ") {
            let id = self.session.id + 1;
            self.session = Session { id, start_ms: time_ms, first_seq: seq, ..Session::default() };
            let detail = after_needle(message, "stream: device ");
            self.set_step(Step::Start, StepStatus::Done, time_ms, seq, detail);
            return;
        }
        if !self.session.active() {
            return;
        }
        if matches!(level, Level::ERROR | Level::WARN) {
            self.session.issues += 1;
        }

        match state {
            Some(StreamState::Error) => {
                // Blame the first step that did not complete
                let text = self.last_state.as_ref().map(|(_, t)| t.clone()).unwrap_or_default();
                if let Some(&s) = STEPS.iter().find(|&&s| !self.session.step(s).status.finished() || s == Step::Link) {
                    self.set_step(s, StepStatus::Failed, time_ms, seq, text);
                }
                // The session thread ends after an error
                self.session.stop_ms = Some(time_ms);
                return;
            }
            Some(StreamState::Idle) => {
                self.session.stop_ms = Some(time_ms);
                return;
            }
            _ => {}
        }

        let Some(r) = RULES.iter().find(|r| {
            target_matches(target, r.target)
                && match r.how {
                    Prefix => message.starts_with(r.needle),
                    Contains => message.contains(r.needle),
                }
        }) else {
            return;
        };
        let detail = match r.detail {
            DetailFrom::None => String::new(),
            DetailFrom::After => after_needle(message, r.needle),
            DetailFrom::Whole => message.to_string(),
        };

        // While reconnecting, the steps keep the first connection's results:
        // what goes wrong in an attempt belongs to the upkeep step
        if r.step != Step::Link && self.session.step(Step::Link).status == StepStatus::Running {
            if matches!(r.action, Action::Failed | Action::Warning) {
                self.set_detail(Step::Link, message.to_string());
            }
            return;
        }

        // A connect that fails also fails the stream BTstack was asked to
        // establish ("stream establishment failed (0x66)"): until signaling
        // is up, that belongs to the connect step, which may still retry
        if r.step.index() > Step::Connect.index()
            && matches!(r.action, Action::Failed | Action::Warning)
            && self.session.step(Step::Connect).status == StepStatus::Running
        {
            self.set_detail(Step::Connect, detail);
            return;
        }

        // A stack that was already up from an earlier attempt logs no init:
        // connecting means it is there
        if r.step == Step::Connect && self.session.step(Step::Btstack).status == StepStatus::Pending {
            self.set_step(Step::Btstack, StepStatus::Done, time_ms, seq, String::new());
            self.fill_controller_detail();
        }

        let link_was_running = self.session.step(Step::Link).status == StepStatus::Running;
        match r.action {
            Action::Done => {
                if r.step == Step::Connect && self.session.connect_retries > 0 {
                    let n = self.session.connect_retries;
                    let detail = format!("{detail} (after {n} failed attempt(s))");
                    self.set_step(r.step, StepStatus::Warning, time_ms, seq, detail);
                } else {
                    self.set_step(r.step, StepStatus::Done, time_ms, seq, detail);
                }
            }
            Action::Running => self.set_step(r.step, StepStatus::Running, time_ms, seq, detail),
            Action::Warning => self.set_step(r.step, StepStatus::Warning, time_ms, seq, detail),
            Action::Failed => self.set_step(r.step, StepStatus::Failed, time_ms, seq, detail),
            Action::Detail => self.set_detail(r.step, detail),
            Action::Retry => {
                self.session.connect_retries += 1;
                self.set_detail(r.step, detail);
            }
        }

        // Follow-ups
        if r.step == Step::Btstack && matches!(r.action, Action::Done) {
            self.fill_controller_detail();
        }
        if r.step == Step::Audio
            && matches!(r.action, Action::Done)
            && self.session.step(Step::Link).status == StepStatus::Pending
        {
            self.set_step(Step::Link, StepStatus::Done, time_ms, seq, String::new());
        }
        // One per interruption: around sleep, both "going to sleep" and
        // "resumed" say reconnecting
        if r.step == Step::Link && matches!(r.action, Action::Running) && !link_was_running {
            self.session.reconnects += 1;
        }
    }

    fn fill_controller_detail(&mut self) {
        if !self.session.step(Step::Btstack).detail.is_empty() {
            return;
        }
        if let Some(l) = self.adapter_lines.iter().find(|l| l.starts_with("controller: ")) {
            let d = after_needle(l, "controller: ");
            self.set_detail(Step::Btstack, d);
        }
    }

    fn track_adapter(&mut self, target: &str, message: &str) {
        let Some(&(_, prefix)) =
            ADAPTER_LINES.iter().find(|(t, p)| target_matches(target, t) && message.starts_with(p))
        else {
            return;
        };
        self.adapter_lines.retain(|l| !l.starts_with(prefix));
        self.adapter_lines.push(message.to_string());
        if self.adapter_lines.len() > MAX_ADAPTER_LINES {
            self.adapter_lines.remove(0);
        }
    }

    fn set_step(&mut self, s: Step, status: StepStatus, time_ms: u64, seq: u64, detail: String) {
        let st = &mut self.session.steps[s.index()];
        match status {
            // Keep the first cause
            StepStatus::Failed if st.status == StepStatus::Failed => return,
            StepStatus::Warning if st.status == StepStatus::Failed => return,
            StepStatus::Done if st.status == StepStatus::Failed => return,
            StepStatus::Done if st.status == StepStatus::Warning => {
                // Completed, but keep the warning
                if !detail.is_empty() {
                    st.detail = detail;
                }
                return;
            }
            // Only the connection upkeep goes back from finished to running
            StepStatus::Running if st.status == StepStatus::Failed => return,
            StepStatus::Running if st.status.finished() && s != Step::Link => return,
            _ => {}
        }
        st.status = status;
        st.time_ms = Some(time_ms);
        st.seq = seq;
        if !detail.is_empty() {
            st.detail = detail;
        }
    }

    fn set_detail(&mut self, s: Step, detail: String) {
        if !detail.is_empty() {
            self.session.steps[s.index()].detail = detail;
        }
    }

    /// The steps as shown: `state` is the service's state now (None: the
    /// last one the log reported), `rates` what the stream moves now
    pub fn rows(&self, state: Option<StreamState>, rates: Option<AudioRates>) -> Vec<FlowRow> {
        let sess = &self.session;
        let state = state.or(self.last_state.as_ref().map(|(s, _)| *s));
        let active = sess.active();
        STEPS
            .iter()
            .map(|&step| {
                let st = sess.step(step);
                let mut r = FlowRow {
                    step,
                    status: st.status,
                    time_ms: st.time_ms,
                    elapsed_ms: st.time_ms.map(|t| t.saturating_sub(sess.start_ms)),
                    detail: st.detail.clone(),
                    detail_msg: None,
                    seq: st.seq,
                };
                // The step being worked on: the first unfinished one while
                // connecting
                if active
                    && r.status == StepStatus::Pending
                    && state == Some(StreamState::Connecting)
                    && STEPS[..step.index()].iter().all(|&p| sess.step(p).status.finished())
                {
                    r.status = StepStatus::Running;
                }
                if step == Step::Audio
                    && active
                    && r.status.finished()
                    && state == Some(StreamState::Streaming)
                    && let Some(rates) = rates
                {
                    let msg = if rates.packets_per_second < 1.0 && rates.capture_frames_per_second < 1.0 {
                        // Loopback capture gets no data while nothing plays,
                        // so nothing is sent either: normal while paused
                        r.status = StepStatus::Done;
                        Msg::new("dbgc.audio_idle")
                    } else if rates.packets_per_second < 1.0 {
                        r.status = StepStatus::Warning;
                        Msg::new("dbgc.audio_stalled")
                    } else {
                        Msg::with(
                            "dbgc.audio_live",
                            &[&format!("{:.0}", rates.packets_per_second), &format!("{:.0}", rates.kbps)],
                        )
                    };
                    r.detail = msg.english();
                    r.detail_msg = Some(msg);
                }
                if step == Step::Link && sess.id != 0 {
                    if sess.reconnects > 0 {
                        let msg = Msg::with("dbgc.link_reconnects", &[&sess.reconnects]);
                        let count = msg.english();
                        r.detail = if r.detail.is_empty() { count } else { format!("{count} - {}", r.detail) };
                        r.detail_msg = Some(msg);
                    } else if r.status == StepStatus::Done {
                        let msg = Msg::new("dbgc.link_stable");
                        r.detail = msg.english();
                        r.detail_msg = Some(msg);
                    }
                }
                r
            })
            .collect()
    }

    /// The copied flow ("Copy Flow"); `clock` as [`format_time`]
    pub fn text(&self, state: Option<StreamState>, rates: Option<AudioRates>, clock: Option<u64>) -> String {
        let sess = &self.session;
        if sess.id == 0 {
            return "Connection flow: no connection attempt yet\n".into();
        }
        let mut out = format!("Connection flow: attempt #{}, started {}", sess.id, format_time(sess.start_ms, clock));
        if let Some(stop) = sess.stop_ms {
            let _ = write!(out, ", stopped {}", format_time(stop, clock));
        }
        if sess.reconnects > 0 {
            let _ = write!(out, ", {} reconnect(s)", sess.reconnects);
        }
        out.push('\n');
        for r in self.rows(state, rates) {
            let time = r.time_ms.map(|t| format_time(t, clock)).unwrap_or_default();
            let elapsed = r.elapsed_ms.map(format_elapsed).unwrap_or_default();
            let line = format!("  {} {:<32} {:<12} {:<10} {}", r.status.tag(), r.step.name(), time, elapsed, r.detail);
            out.push_str(line.trim_end());
            out.push('\n');
        }
        out
    }
}

/// "hh:mm:ss.mmm" with `clock` = the local time of day (ms) at time_ms 0;
/// without it, seconds since the process started ("T+12.345")
pub fn format_time(time_ms: u64, clock: Option<u64>) -> String {
    match clock {
        Some(base) => {
            let t = (base + time_ms) % 86_400_000;
            format!("{:02}:{:02}:{:02}.{:03}", t / 3_600_000, t / 60_000 % 60, t / 1000 % 60, t % 1000)
        }
        None => format!("T+{}.{:03}", time_ms / 1000, time_ms % 1000),
    }
}

/// "+1.234 s"
pub fn format_elapsed(ms: u64) -> String {
    format!("+{}.{:03} s", ms / 1000, ms % 1000)
}

/// Masks what identifies the user, the PC and their devices (the UI's
/// maskIdentifiers does the same), see [`Masker`]
pub fn mask_identifiers(text: &str) -> String {
    Masker::new().mask(text)
}

/// The quotes device names are logged in (`“Taro's AirPods”`): what the
/// mask takes for a name, see [`Masker`]
pub fn quoted(name: &str) -> String {
    format!("\u{201C}{name}\u{201D}")
}

/// Masks what identifies the user, the PC and their devices:
/// - Device names: the text between typographic quotes `“...”` (what
///   [`quoted`] makes: the app logs device names that way) and the names
///   it was told ([`Masker::know`]: the profile's, the saved devices')
///   wherever they appear -> `<device-1>`, `<device-2>`..., the same name
///   the same number for the masker's lifetime, so a bundle's files agree.
/// - Paths: everything above the app's own folder (`A2DPWB`, the settings
///   folder; `A2DP Windows Bridge`, the installed one) -> `<path>`, so
///   `C:\Users\taro\AppData\Roaming\A2DPWB\logs\x.log` is
///   `<path>\A2DPWB\logs\x.log`. A path without the app's folder keeps its
///   last part only: `<path>\a2dpwb-core.exe`, `<path>\foobar2000.exe`.
///   Drive paths ('/' too, JSON's doubled '\' too, `\\?\` before); the
///   drive, the user's folders and whatever else the PC's layout tells are
///   gone.
/// - The Windows user name in paths without a drive: `\Users\<name>\...`
///   -> as many 'x' as characters; `Public`, `Default` and `All Users`
///   are nobody's.
/// - Windows account SIDs (`S-1-5-21-<machine>-<user>`, the IPC pipe's
///   name is one) -> `<sid>`: the machine part is unique to the PC. The
///   well-known short ones (`S-1-5-18`, SYSTEM) say nothing and stay.
/// - Bluetooth addresses, "AA:BB:CC:DD:EE:FF" or with '-', and the same
///   without separators (a word of 12 hex digits: Windows' BTHENUM / BTHLE
///   IDs, USB serial numbers that are the adapter's address) ->
///   `<addr-1>`, `<addr-2>`..., the same address the same number whatever
///   its form; the adapters' own addresses ([`Masker::know_adapter`]) ->
///   `<adapter>`. Nothing of the address stays: its first three octets
///   name the maker (the OUI), and so the model. Not when part of a longer
///   run (an IPv6 address); a letter before is fine (Windows'
///   "BluetoothLE#BluetoothLE<adapter>-<device>" IDs); not after '-' (a
///   GUID's last part).
/// - Other USB serial numbers, in instance IDs and device paths
///   (`USB\VID_0BDA&PID_8771\<serial>`) and driver record names
///   (`USB_VID_0BDA_PID_8771_<serial>`) -> as many 'x'. IDs Windows
///   generates (`5&2C7B4E3&0&2`) say nothing about the device and stay.
#[derive(Debug, Default)]
pub struct Masker {
    /// Names seen or told, numbered from 1 in this order
    names: Vec<String>,
    /// Addresses seen (12 upper-case hex digits), numbered from 1
    addrs: Vec<String>,
    /// The adapters' own addresses (12 upper-case hex digits)
    adapters: Vec<String>,
}

/// The 12 hex digits of an address in any form, upper case
fn address_key(addr: &str) -> Option<String> {
    let key: String = addr.chars().filter(|c| !matches!(c, ':' | '-')).map(|c| c.to_ascii_uppercase()).collect();
    (key.len() == 12 && key.bytes().all(|c| c.is_ascii_hexdigit())).then_some(key)
}

impl Masker {
    pub fn new() -> Masker {
        Masker::default()
    }

    /// A name to mask wherever it appears, quoted or not (nothing for an
    /// empty or very short one: a word that short would hit other text)
    pub fn know(&mut self, name: &str) {
        let name = name.trim();
        if name.chars().count() >= 3 && !self.names.iter().any(|n| n == name) {
            self.names.push(name.to_owned());
        }
    }

    /// An adapter's own address, shown as `<adapter>` (any form)
    pub fn know_adapter(&mut self, addr: &str) {
        if let Some(key) = address_key(addr)
            && !self.adapters.contains(&key)
        {
            self.adapters.push(key);
        }
    }

    /// The pseudonym of a name
    fn pseudonym(&mut self, name: &str) -> String {
        let n = match self.names.iter().position(|n| n == name) {
            Some(i) => i + 1,
            None => {
                self.names.push(name.to_owned());
                self.names.len()
            }
        };
        format!("<device-{n}>")
    }

    /// The pseudonym of an address (any form)
    fn address_pseudonym(&mut self, addr: &str) -> String {
        let key = address_key(addr).unwrap_or_default();
        if self.adapters.contains(&key) {
            return "<adapter>".into();
        }
        let n = match self.addrs.iter().position(|a| *a == key) {
            Some(i) => i + 1,
            None => {
                self.addrs.push(key);
                self.addrs.len()
            }
        };
        format!("<addr-{n}>")
    }

    pub fn mask(&mut self, text: &str) -> String {
        let text = self.mask_quoted_names(text);
        let text = self.mask_known_names(&text);
        // The user name first: a path that ends in the user's folder keeps
        // that part
        let text = mask_user_paths(&text);
        let text = mask_path_heads(&text);
        let text = mask_sids(&text);
        let mut b = text.into_bytes();
        // Before the addresses: a serial that is an address is left to them
        mask_usb_serials(&mut b);
        let text = String::from_utf8(b).expect("only ASCII bytes were replaced");
        self.mask_addresses(&text)
    }

    /// "AA:BB:CC:DD:EE:FF", "AA-BB-CC-DD-EE-FF" and "AABBCCDDEEFF" ->
    /// `<addr-N>` / `<adapter>`
    fn mask_addresses(&mut self, text: &str) -> String {
        let b = text.as_bytes();
        let mut out = String::with_capacity(text.len());
        let mut i = 0;
        while i < b.len() {
            if let Some(len) = separated_address_at(b, i) {
                out.push_str(&self.address_pseudonym(&text[i..i + len]));
                i += len;
                continue;
            }
            if b[i].is_ascii_alphanumeric() && (i == 0 || !b[i - 1].is_ascii_alphanumeric()) {
                // A word of 12 hex digits, not after '-' (a GUID's last
                // part); any other word goes on by the character, as a
                // separated address can start inside it ("BluetoothLE00:1a:...")
                let mut j = i;
                while j < b.len() && b[j].is_ascii_alphanumeric() {
                    j += 1;
                }
                if j - i == 12 && b[i..j].iter().all(u8::is_ascii_hexdigit) && (i == 0 || b[i - 1] != b'-') {
                    out.push_str(&self.address_pseudonym(&text[i..j]));
                    i = j;
                    continue;
                }
            }
            let c = text[i..].chars().next().expect("at a char boundary");
            out.push(c);
            i += c.len_utf8();
        }
        out
    }

    /// `“name”` -> `<device-N>`, on one line, up to 256 characters
    fn mask_quoted_names(&mut self, text: &str) -> String {
        const OPEN: char = '\u{201C}';
        const CLOSE: char = '\u{201D}';
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(start) = rest.find(OPEN) {
            let after = &rest[start + OPEN.len_utf8()..];
            let end = after.find([CLOSE, OPEN, '\n']).filter(|&e| after[e..].starts_with(CLOSE));
            match end {
                Some(e) if e > 0 && after[..e].chars().count() <= 256 => {
                    out.push_str(&rest[..start]);
                    out.push_str(&self.pseudonym(&after[..e]));
                    rest = &after[e + CLOSE.len_utf8()..];
                }
                _ => {
                    out.push_str(&rest[..start + OPEN.len_utf8()]);
                    rest = after;
                }
            }
        }
        out.push_str(rest);
        out
    }

    /// The names told, longest first (a name that contains another)
    fn mask_known_names(&mut self, text: &str) -> String {
        let mut order: Vec<usize> = (0..self.names.len()).collect();
        order.sort_by_key(|&i| std::cmp::Reverse(self.names[i].len()));
        let mut out = text.to_owned();
        for i in order {
            if out.contains(&self.names[i]) {
                out = out.replace(&self.names[i], &format!("<device-{}>", i + 1));
            }
        }
        out
    }
}

/// The app's own folders: a path is kept from one of these on
const APP_FOLDERS: &[&str] = &["A2DPWB", "A2DP Windows Bridge"];

/// `X:\` (or '/') at `i`, not inside a word
fn path_start_at(b: &[u8], i: usize) -> bool {
    b.get(i).is_some_and(u8::is_ascii_alphabetic)
        && b.get(i + 1) == Some(&b':')
        && b.get(i + 2).is_some_and(|&c| c == b'\\' || c == b'/')
        && (i == 0 || !b[i - 1].is_ascii_alphanumeric())
}

/// A drive path -> `<path>` and the part from the app's own folder on
/// ([`APP_FOLDERS`]), or its last part. A part ends at what a file name
/// cannot contain, or at a space that starts another path.
fn mask_path_heads(text: &str) -> String {
    let b = text.as_bytes();
    let is_sep = |c: u8| c == b'\\' || c == b'/';
    let ends = |c: u8| c < 0x20 || b"\\/:*?\"<>|".contains(&c);
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < b.len() {
        if path_start_at(b, i) {
            // (the separators before it, the part): the part may be empty
            // after a trailing separator
            let mut parts: Vec<(usize, usize, usize)> = Vec::new();
            let mut j = i + 2;
            loop {
                let seps = j;
                while j < b.len() && is_sep(b[j]) {
                    j += 1;
                }
                let start = j;
                while j < b.len() && !ends(b[j]) && !(b[j].is_ascii_whitespace() && path_start_at(b, j + 1)) {
                    j += 1;
                }
                parts.push((seps, start, j));
                if !(j < b.len() && is_sep(b[j])) {
                    break;
                }
            }
            let keep = parts
                .iter()
                .position(|&(_, s, e)| APP_FOLDERS.iter().any(|f| text[s..e].eq_ignore_ascii_case(f)))
                .or_else(|| parts.iter().rposition(|&(_, s, e)| e > s))
                .unwrap_or(parts.len() - 1);
            out.push_str("<path>");
            out.push_str(&text[parts[keep].0..j]);
            i = j;
            continue;
        }
        let c = text[i..].chars().next().expect("at a char boundary");
        out.push(c);
        i += c.len_utf8();
    }
    out
}

/// A SID with a machine or domain part, `S-1-5-21-a-b-c-rid` (`S-1-`, then
/// five or more numbers) -> `<sid>`; not inside a longer word (a '-' on
/// either side is fine: the pipe's `a2dpwb-<sid>-<instance>`)
fn mask_sids(text: &str) -> String {
    let b = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < b.len() {
        if b[i..].starts_with(b"S-1-") && (i == 0 || !b[i - 1].is_ascii_alphanumeric()) {
            // The numbers after "S-1-", each after a '-'
            let mut j = i + 1;
            let mut groups = 0;
            while j < b.len() && b[j] == b'-' && b.get(j + 1).is_some_and(u8::is_ascii_digit) {
                j += 1;
                while j < b.len() && b[j].is_ascii_digit() {
                    j += 1;
                }
                groups += 1;
            }
            if groups >= 5 && !b.get(j).is_some_and(u8::is_ascii_alphanumeric) {
                out.push_str("<sid>");
                i = j;
                continue;
            }
        }
        let c = text[i..].chars().next().expect("at a char boundary");
        out.push(c);
        i += c.len_utf8();
    }
    out
}

/// `\Users\<name>\` (or '/'; JSON's `\\Users\\<name>\\` too) -> the name as
/// 'x's
fn mask_user_paths(text: &str) -> String {
    const NOBODY: &[&str] = &["Public", "Default", "Default User", "All Users"];
    let is_sep = |c: u8| c == b'\\' || c == b'/';
    // What a Windows user name cannot contain, and where a path stops
    let ends = |c: u8| c < 0x20 || b"\\/:*?\"<>|".contains(&c);
    let b = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < b.len() {
        if is_sep(b[i])
            && b.get(i + 1..i + 6).is_some_and(|w| w.eq_ignore_ascii_case(b"Users"))
            && b.get(i + 6).is_some_and(|&c| is_sep(c))
        {
            let mut j = i + 6;
            while j < b.len() && is_sep(b[j]) {
                j += 1;
            }
            let start = j;
            while j < b.len() && !ends(b[j]) {
                j += 1;
            }
            let name = &text[start..j];
            if !name.is_empty() && !NOBODY.iter().any(|n| n.eq_ignore_ascii_case(name)) {
                out.push_str(&text[i..start]);
                out.extend(std::iter::repeat_n('x', name.chars().count()));
                i = j;
                continue;
            }
        }
        let c = text[i..].chars().next().expect("at a char boundary");
        out.push(c);
        i += c.len_utf8();
    }
    out
}

/// The length of a separated address ("AA:BB:CC:DD:EE:FF" or with '-') at
/// `i`, if one is there
fn separated_address_at(b: &[u8], i: usize) -> Option<usize> {
    const LEN: usize = 17;
    if i + LEN > b.len() {
        return None;
    }
    let sep = b[i + 2];
    let is_addr = (sep == b':' || sep == b'-')
        && (0..LEN).all(|k| if k % 3 == 2 { b[i + k] == sep } else { b[i + k].is_ascii_hexdigit() })
        && (i == 0 || b[i - 1] != sep)
        // Not continued by hex, or by the separator and two more hex
        // digits ("00:11:22:33:44:55:66"); a colon that ends it is
        // punctuation ("LE Audio: 46:62:99:4A:5B:6C: pairing")
        && !b.get(i + LEN).is_some_and(u8::is_ascii_hexdigit)
        && !(b.get(i + LEN) == Some(&sep)
            && b.get(i + LEN + 1..i + LEN + 3).is_some_and(|h| h.iter().all(u8::is_ascii_hexdigit)));
    is_addr.then_some(LEN)
}

/// `VID_xxxx<joint>PID_xxxx` at `i` (any case) -> the joint
fn vid_pid_at(b: &[u8], i: usize) -> Option<u8> {
    let hex4 = |at: usize| b.get(at..at + 4).is_some_and(|h| h.iter().all(u8::is_ascii_hexdigit));
    let joint = *b.get(i + 8)?;
    (b.get(i..i + 4)?.eq_ignore_ascii_case(b"VID_")
        && hex4(i + 4)
        && (joint == b'&' || joint == b'_')
        && b.get(i + 9..i + 13)?.eq_ignore_ascii_case(b"PID_")
        && hex4(i + 13))
    .then_some(joint)
}

fn mask_usb_serials(b: &mut [u8]) {
    let ends = |c: u8| !c.is_ascii() || c.is_ascii_whitespace() || b"\\#\"',;:.()[]{}".contains(&c);
    let mut i = 0;
    while i < b.len() {
        let Some(joint) = vid_pid_at(b, i) else {
            i += 1;
            continue;
        };
        i += 17;
        if joint == b'&' {
            // An instance ID or device path: "&MI_00" and the like, then
            // '\' or '#' (doubled in JSON)
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'&') {
                i += 1;
            }
            let sep = i;
            while i < b.len() && (b[i] == b'\\' || b[i] == b'#') {
                i += 1;
            }
            if i == sep {
                continue;
            }
        } else if b.get(i) == Some(&b'_') {
            // A driver record name: the rest of it (where Windows' own '&' became '_')
            i += 1;
        } else {
            continue;
        }
        let start = i;
        while i < b.len() && !ends(b[i]) {
            i += 1;
        }
        let serial = &b[start..i];
        let generated = joint == b'&' && serial.contains(&b'&');
        let address = serial.len() == 12 && serial.iter().all(u8::is_ascii_hexdigit); // mask_bare_addresses
        if !generated && !address {
            b[start..i].fill(b'x');
        }
    }
}

/// A log line as the diagnostics read it (borrowed from the log buffer)
#[derive(Debug, Clone, Copy)]
pub struct LogEntry<'a> {
    pub time_ms: u64,
    pub level: Level,
    pub target: &'a str,
    pub message: &'a str,
}

/// "time LEVEL target: message" (copied log lines)
pub fn format_line(e: &LogEntry<'_>, clock: Option<u64>) -> String {
    format!("{:<12} {:<5} {}: {}", format_time(e.time_ms, clock), e.level.as_str(), e.target, e.message)
}

/// Where the diagnostics were made
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Environment {
    /// Local "YYYY-MM-DD hh:mm:ss"
    pub generated: String,
    pub app_version: String,
    pub windows_build: u32,
    pub arch: &'static str,
}

impl Environment {
    pub fn current() -> Environment {
        let now = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
        Environment {
            generated: format!(
                "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                now.year(),
                u8::from(now.month()),
                now.day(),
                now.hour(),
                now.minute(),
                now.second()
            ),
            app_version: env!("CARGO_PKG_VERSION").into(),
            windows_build: audio_capture::windows_build(),
            arch: std::env::consts::ARCH,
        }
    }
}

/// "a/b/c"
fn joined<T: std::fmt::Display>(v: &[T]) -> String {
    v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("/")
}

fn khz(rates: &[u32]) -> String {
    joined(&rates.iter().map(|&r| r as f64 / 1000.0).collect::<Vec<_>>()) + " kHz"
}

/// The codec's setup and what the device offered
fn codec_details(out: &mut String, c: &crate::CodecDetails) {
    let _ = write!(
        out,
        "  Packets: media payload up to {} bytes, {}, {}-byte payload header",
        c.mtu,
        if c.rtp { "RTP" } else { "no RTP" },
        c.media_header
    );
    match (c.frames_per_packet, c.frame_bytes) {
        (Some(n), Some(b)) => {
            let _ = write!(out, "; {n} frames of {b} bytes");
        }
        (None, Some(b)) => {
            let _ = write!(out, "; frames of {b} bytes");
        }
        _ => {}
    }
    let _ = writeln!(out, " ({} samples per frame)", c.frame_samples);
    if let Some(s) = &c.sbc {
        let _ = write!(
            out,
            "  SBC: {}, {} blocks, {} subbands, {}, bitpool {}",
            s.channel_mode, s.blocks, s.subbands, s.allocation, s.bitpool
        );
        if let Some((lo, hi)) = s.config_bitpool {
            let _ = write!(out, " (configured {lo}-{hi})");
        }
        if let Some((lo, hi)) = s.abr_bitpool {
            let _ = write!(out, " (ABR {lo}-{hi})");
        }
        out.push('\n');
        if let Some(d) = &s.device {
            let _ = writeln!(
                out,
                "  Device SBC: {}; {}; blocks {}; subbands {}; {}; bitpool {}-{}",
                khz(&d.sample_rates),
                d.channel_modes.join("/"),
                joined(&d.blocks),
                joined(&d.subbands),
                d.allocations.join("/"),
                d.min_bitpool,
                d.max_bitpool
            );
        }
    }
    if let Some(a) = &c.aac {
        let _ = writeln!(out, "  AAC: {}, CBR {} bps", a.object_type, a.bit_rate);
        if let Some(d) = &a.device {
            let _ = writeln!(
                out,
                "  Device AAC: {}; {}; {} ch; peak {}; VBR {}",
                d.object_types.join(", "),
                khz(&d.sample_rates),
                joined(&d.channels),
                if d.max_bitrate == 0 { "not given".to_string() } else { format!("{} bps", d.max_bitrate) },
                if d.vbr { "yes" } else { "no" }
            );
        }
    }
    if let Some(d) = &c.device {
        let _ = writeln!(
            out,
            "  Device {}: {}; {}; peak {}",
            d.codec,
            if d.sample_rates.is_empty() { "rates not given".to_string() } else { khz(&d.sample_rates) },
            if d.bit_depths.is_empty() {
                "bit depth not given".to_string()
            } else {
                format!("{}-bit", joined(&d.bit_depths))
            },
            if d.max_bitrate == 0 { "not given".to_string() } else { format!("{} bps", d.max_bitrate) }
        );
    }
}

/// Why the bit rate is below the profile's: ABR, or the device's maximum
fn bitrate_note(i: &StreamInfo) -> &'static str {
    if i.abr {
        " (ABR)"
    } else if i.bitrate_capped {
        " (limited by the device's maximum bit rate)"
    } else {
        ""
    }
}

/// The driver in the two names the Adapters screen uses (issue #86):
/// "WinUSB driver", or the "original driver" it replaces with its service
/// ("original driver (BTHUSB)")
pub fn driver_name(kind: win_sys::driver::DriverKind, service: &str) -> String {
    use win_sys::driver::DriverKind;
    match kind {
        DriverKind::WinUsb => "WinUSB driver".to_string(),
        DriverKind::Bluetooth | DriverKind::Other if service.is_empty() => "original driver".to_string(),
        DriverKind::Bluetooth | DriverKind::Other => format!("original driver ({service})"),
        DriverKind::None => "no driver".to_string(),
    }
}

/// One adapter for the diagnostics: name, VID:PID, driver
pub fn adapter_line(a: &crate::adapter::Adapter) -> String {
    let d = &a.device;
    let mut s =
        format!("{} [{:04X}:{:04X}], {}", a.display_name, d.vid, d.pid, driver_name(d.driver_kind, &d.driver.service));
    if let Some(code) = d.problem_code {
        let _ = write!(s, ", problem code {code}");
    }
    if let Some(Err(e)) = &d.probe {
        let _ = write!(s, ", {e}");
    }
    s
}

/// Everything the "copy diagnostic info" text is made of
pub struct DiagInput<'a> {
    pub env: Environment,
    pub settings: &'a Settings,
    pub state: StreamState,
    /// [`adapter_line`] of each present adapter
    pub adapters: &'a [String],
    /// Firmware files: e.g. "present (rtl8761bu_fw.bin)"; None = not known
    pub firmware: Option<&'a str>,
    /// The selected / active profile
    pub profile: Option<&'a Profile>,
    /// While streaming
    pub stream: Option<&'a StreamInfo>,
    pub stats: Option<&'a LinkStats>,
    /// A running HCI capture: its file and size
    pub hci_capture: Option<(&'a Path, u64)>,
    pub flow: &'a ConnectionFlow,
    pub rates: Option<AudioRates>,
    /// The kept log (oldest first)
    pub log: &'a [LogEntry<'a>],
    /// Local time of day (ms) at time_ms 0 ([`format_time`])
    pub clock: Option<u64>,
    /// Addresses, device names, USB serials and the user name masked
    /// ([`Masker`])
    pub mask: bool,
    /// The adapters' own addresses (`<adapter>` when masked)
    pub adapter_addresses: &'a [String],
    /// LE Audio (experimental) of the running stack
    pub le_audio: Option<&'a LeAudioDiag>,
    /// Verbose logging, crash reports, flight recorder files (one line)
    pub logging: Option<&'a str>,
}

/// What the diagnostics tell about LE Audio (experimental) of the running
/// stack
#[derive(Debug, Clone, Default)]
pub struct LeAudioDiag {
    /// The stream's summary (v1's StreamPlan::describe), while one runs
    pub stream: Option<String>,
    /// The ISO sender's counters of the running (or last) stream
    pub stats: Option<bt_host::LeStreamStats>,
    /// The devices the LE core knows
    pub devices: Vec<bt_host::LeDeviceInfo>,
}

/// The [LE Audio] section: only with the setting on or an LE stack running
fn le_audio_section(out: &mut String, s: &Settings, le: Option<&LeAudioDiag>) {
    if !s.le_audio_experimental && le.is_none() {
        return;
    }
    out.push_str("\n[LE Audio (experimental)]\n");
    let _ = writeln!(
        out,
        "  Setting: {}, low latency {}; BTstack LE {}",
        on_off(s.le_audio_experimental),
        on_off(s.le_audio_low_latency),
        if le.is_some() { "active" } else { "not running" }
    );
    let Some(le) = le else { return };
    if let Some(summary) = &le.stream {
        let _ = writeln!(out, "  Stream: {summary}");
    }
    if let Some(st) = &le.stats {
        let _ = writeln!(
            out,
            "  ISO: {} SDU sets sent ({} SDUs), {} silence, {} underruns, {} dropped, {} send errors, prefill {}",
            st.sets_sent, st.sdus_sent, st.silence_sets, st.underruns, st.overflows, st.send_errors, st.prefill
        );
    }
    for d in &le.devices {
        let state = match (d.connected, d.ready) {
            (true, true) => "ready",
            (true, false) => "connected",
            _ => "disconnected",
        };
        let _ = write!(out, "  Device {} {}: {state}, {} sink ASE(s)", d.addr, quoted(&d.name), d.sink_ases);
        if !d.roles.is_empty() {
            let _ = write!(out, ", {}", d.roles);
        }
        out.push('\n');
    }
}

fn on_off(b: bool) -> &'static str {
    if b { "on" } else { "off" }
}

/// The "copy diagnostic info" text (a fenced block for a bug report)
pub fn diag_text(d: &DiagInput<'_>) -> String {
    let mut out = String::with_capacity(16 * 1024);
    let s = d.settings;
    out.push_str("```text\n");
    let _ = writeln!(out, "A2DPWB diagnostics - {}", d.env.generated);
    let _ = writeln!(out, "Version      : {}", d.env.app_version);
    let _ = writeln!(out, "OS           : Windows build {} ({})", d.env.windows_build, d.env.arch);
    let _ = writeln!(out, "Language     : {}, theme {}", s.language, s.theme);
    let _ = writeln!(
        out,
        "Settings     : media keys {}, remember volume {}, reconnect {}, volume sync {}, mute output {}, auto-select {}, AFH {}, updates {}{}",
        on_off(s.media_keys),
        on_off(s.remember_volume),
        on_off(s.auto_reconnect),
        on_off(s.volume_sync),
        on_off(s.mute_output),
        on_off(s.auto_codec),
        s.afh,
        on_off(s.check_updates_on_startup),
        if s.include_prereleases { " (pre-releases too)" } else { "" }
    );
    if let Some(l) = d.logging {
        let _ = writeln!(out, "Logging      : {l}");
    }
    let _ = write!(out, "State        : {}", state_name(d.state));
    if let Some((_, text)) = d.flow.last_state()
        && !text.is_empty()
    {
        let _ = write!(out, " ({text})");
    }
    out.push('\n');

    out.push_str("\n[Adapter]\n");
    let chip_kind = if bt_host::chips::is_auto_chip(s.bt_chip_pid, &s.bt_chip_fw_stem) {
        " (auto)"
    } else if s.bt_chip_pid == bt_host::chips::NON_REALTEK_CHIP_PID {
        " (not Realtek)"
    } else if s.bt_chip_pid == bt_host::chips::CUSTOM_CHIP_PID {
        " (custom)"
    } else {
        ""
    };
    let _ = write!(out, "  Setting: chip PID 0x{:04X}{chip_kind}", s.bt_chip_pid);
    if !s.bt_chip_fw_stem.is_empty() {
        let _ = write!(out, ", firmware stem {}", s.bt_chip_fw_stem);
    }
    out.push('\n');
    if let Some(fw) = d.firmware {
        let _ = writeln!(out, "  Firmware files: {fw}");
    }
    if d.adapters.is_empty() {
        out.push_str("  No USB Bluetooth adapter found\n");
    }
    for a in d.adapters {
        let _ = writeln!(out, "  {a}");
    }
    for l in d.flow.adapter_lines() {
        let _ = writeln!(out, "  {l}");
    }

    out.push_str("\n[Audio device]\n");
    match d.profile {
        Some(p) => {
            let _ = writeln!(out, "  Name: {}", quoted(&p.name));
            let _ = writeln!(out, "  Device: {} [{}]", quoted(&p.device_name), p.device_address);
            let auto_or = |v: u32| if v == 0 { "auto".to_string() } else { v.to_string() };
            let order = p.order();
            let _ = writeln!(
                out,
                "  Codec: {}, quality {}, ABR {}, sample rate {}, bit depth {}{}",
                p.codec,
                p.quality,
                on_off(p.abr),
                auto_or(p.sample_rate),
                auto_or(p.bit_depth),
                if order.is_default() { String::new() } else { format!(", order {}", order.names()) }
            );
            let _ = write!(out, "  Capture: {}", p.capture_mode);
            if p.capture_mode == "app" {
                if !p.app_exe.is_empty() {
                    let _ = write!(out, " '{}'", if p.app_name.is_empty() { &p.app_exe } else { &p.app_name });
                }
            } else if !p.audio_device_name.is_empty() {
                let _ = write!(out, " {}", quoted(&p.audio_device_name));
            }
            let payload = p.max_media_payload.map(|v| v.to_string()).unwrap_or_else(|| "not set".into());
            let _ = writeln!(out, ", auto switch {}, max media packet {payload}", on_off(p.auto_switch_device));
            // As the stream has it: the setting for every device alone (the
            // profile's own is no longer used, issue #86)
            let _ = writeln!(out, "  Volume sync: {}", if s.volume_sync { "auto" } else { "off" });
        }
        None => out.push_str("  (none selected - receiver measurement or not connected)\n"),
    }

    out.push_str("\n[Stream]\n");
    match d.stream {
        Some(i) if d.state == StreamState::Streaming => {
            let _ = writeln!(
                out,
                "  {} {} Hz {} ch, {} kbps{}; source {} Hz {} ch {} bit",
                i.codec,
                i.sample_rate,
                i.channels,
                i.bitrate_kbps,
                bitrate_note(i),
                i.source_sample_rate,
                i.source_channels,
                i.source_bits
            );
            let src = &i.source;
            let _ = writeln!(
                out,
                "  Source: {}{}{}",
                src.kind,
                // An application's executable is no device name
                if src.name.is_empty() {
                    String::new()
                } else if src.kind == "app" {
                    format!(" '{}'", src.name)
                } else {
                    format!(" {}", quoted(&src.name))
                },
                if src.waiting { " (waiting for the app to start)" } else { "" }
            );
            if let Some(req) = &i.requested_codec {
                let _ = writeln!(
                    out,
                    "  Codec fallback: {req} requested{}",
                    if i.requested_in_use { " (in use by another connection)" } else { "" }
                );
            }
            if let Some(c) = &i.details {
                codec_details(&mut out, c);
            }
        }
        _ => out.push_str("  not streaming\n"),
    }
    if let Some(ls) = d.stats {
        use std::sync::atomic::Ordering::Relaxed;
        let _ = writeln!(
            out,
            "  Media packets sent {} ({} bytes), queue drops {}, send errors {}",
            ls.packets_sent.load(Relaxed),
            ls.bytes_sent.load(Relaxed),
            ls.queue_drops.load(Relaxed),
            ls.send_errors.load(Relaxed)
        );
        let _ = writeln!(
            out,
            "  Capture frames {}, dropped {}",
            ls.capture_frames.load(Relaxed),
            ls.capture_dropped_frames.load(Relaxed)
        );
        let radio = ls.radio();
        let mut parts = Vec::new();
        if let Some(rssi) = radio.rssi {
            parts.push(format!("RSSI {rssi:+} dB vs. golden range"));
        }
        if let Some(n) = radio.afh_in_use {
            parts.push(format!("AFH {n} channels in use"));
        }
        if let Some(n) = radio.afh_usable {
            let avoiding = if radio.afh_avoided.is_empty() {
                String::new()
            } else {
                format!(", avoiding Wi-Fi {}", radio.afh_avoided)
            };
            parts.push(format!("host classification {n} usable{avoiding}"));
        }
        if !parts.is_empty() {
            let _ = writeln!(out, "  Radio: {}", parts.join(", "));
        }
    }
    if let Some((path, bytes)) = d.hci_capture {
        let _ = writeln!(out, "  HCI capture running: {} ({:.1} MB)", path.display(), bytes as f64 / (1024.0 * 1024.0));
    }
    le_audio_section(&mut out, s, d.le_audio);

    out.push_str("\n[Flow]\n");
    out.push_str(&d.flow.text(Some(d.state), d.rates, d.clock));

    // Lines of the current attempt (everything if none yet)
    let sess = d.flow.session();
    let lines: Vec<&LogEntry<'_>> = d.log.iter().filter(|e| sess.id == 0 || e.time_ms >= sess.start_ms).collect();
    let issues: Vec<&&LogEntry<'_>> = lines.iter().filter(|e| matches!(e.level, Level::ERROR | Level::WARN)).collect();

    out.push_str("\n[Warnings and errors]\n");
    if issues.is_empty() {
        out.push_str("  none\n");
    }
    let skip = issues.len().saturating_sub(DIAG_ISSUES);
    if skip > 0 {
        let _ = writeln!(out, "  ({skip} earlier omitted)");
    }
    for e in &issues[skip..] {
        let _ = writeln!(out, "  {}", format_line(e, d.clock));
    }

    out.push_str("\n[Log]\n");
    let skip = lines.len().saturating_sub(DIAG_LINES);
    if skip > 0 {
        let _ = writeln!(out, "({skip} earlier lines omitted - see the log files)");
    }
    for e in &lines[skip..] {
        let _ = writeln!(out, "{}", format_line(e, d.clock));
    }
    out.push_str("```\n");
    if !d.mask {
        return out;
    }
    let mut m = Masker::new();
    if let Some(p) = d.profile {
        for n in [&p.device_name, &p.audio_device_name, &p.volume_device_name, &p.name] {
            m.know(n);
        }
    }
    for a in d.adapter_addresses {
        m.know_adapter(a);
    }
    m.mask(&out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SVC: &str = "a2dpwb_core::service";
    const APP: &str = "bt_host::app";

    /// A successful attempt as v2 logs it (abridged)
    fn good_session() -> Vec<(u64, &'static str, Level, &'static str)> {
        vec![
            (1000, SVC, Level::INFO, "state Connecting: Initializing..."),
            (
                1001,
                STREAM,
                Level::INFO,
                "stream: device 00:1A:7D:DA:71:13 codec Some(Ldac) quality High bit depth None rate None source SystemLoopback",
            ),
            (1002, SVC, Level::INFO, "state Connecting: Initializing Bluetooth..."),
            (1010, "a2dpwb_core::adapter", Level::INFO, "chip: RTL8761B (0x8771) (auto)"),
            (1020, APP, Level::INFO, "Realtek adapter PID 0x8771, firmware folder C:\\x"),
            (1030, APP, Level::INFO, "hci_init done, powering on"),
            (
                1500,
                APP,
                Level::INFO,
                "controller: company 0x005D (Realtek Semiconductor Corporation), HCI version 10, LMP subversion 0x8761",
            ),
            (1800, APP, Level::INFO, "Realtek firmware loaded (LMP subversion 0x8761 -> 0xDFC6)"),
            (1900, APP, Level::INFO, "HCI ready (took 870 ms)"),
            (1901, "bt_host", Level::INFO, "HCI ready after 880 ms"),
            (1950, "bt_host", Level::INFO, "connecting to 00:1A:7D:DA:71:13"),
            (2300, APP, Level::INFO, "HCI ACL connection established to 00:1A:7D:DA:71:13"),
            (2400, APP, Level::INFO, "signaling connection established (cid 0x0041, handle 0x000b)"),
            (2500, APP, Level::INFO, "remote supports LDAC (SEID 1, [..])"),
            (
                2600,
                APP,
                Level::INFO,
                "capability discovery complete (LDAC=true aptX HD=false aptX LL=false aptX=false aptX Adaptive=false \
                 [not encodable] SBC=true AAC=true LHDC V5=false Opus=false)",
            ),
            (2601, STREAM, Level::INFO, "device connected, choosing the codec"),
            (2602, STREAM, Level::INFO, "codec selected: LDAC"),
            (2603, SVC, Level::INFO, "state Connecting: Initializing audio..."),
            (
                2650,
                CAPTURE,
                Level::INFO,
                "audio capture: loopback of \"Speakers\": mix 48000 Hz 2 ch 32 bit, capturing 48000 Hz F32",
            ),
            (2700, STREAM, Level::INFO, "capture: 48000 Hz, 2 channel(s), F32 -> 2 channel(s)"),
            (2800, APP, Level::INFO, "codec configured (local 1, remote 1)"),
            (2900, APP, Level::INFO, "stream established (local 1, remote 1, mtu 1005, used 1005, limit 1023)"),
            (2950, STREAM, Level::INFO, "encoder ready: LDAC 990 kbps, mtu 1005, 24-bit, ABR false"),
            (3000, APP, Level::INFO, "streaming started"),
            (3001, SVC, Level::INFO, "state Streaming: Connected"),
            (3050, CAPTURE, Level::INFO, "audio capture started: 48000 Hz, 2 ch, F32, event mode, period 10 ms"),
        ]
    }

    fn feed(f: &mut ConnectionFlow, lines: &[(u64, &str, Level, &str)]) {
        for (t, target, level, msg) in lines {
            f.feed(*t, target, *level, msg);
        }
    }

    fn status(f: &ConnectionFlow, s: Step) -> StepStatus {
        f.session().step(s).status
    }

    #[test]
    fn driver_in_the_adapters_screens_two_names() {
        use win_sys::driver::DriverKind;
        assert_eq!(driver_name(DriverKind::WinUsb, "WINUSB"), "WinUSB driver");
        assert_eq!(driver_name(DriverKind::Bluetooth, "BTHUSB"), "original driver (BTHUSB)");
        // A vendor's own driver is the original one too
        assert_eq!(driver_name(DriverKind::Other, "mtkbtfilterx"), "original driver (mtkbtfilterx)");
        assert_eq!(driver_name(DriverKind::Other, ""), "original driver");
        assert_eq!(driver_name(DriverKind::None, ""), "no driver");
    }

    #[test]
    fn a_good_attempt_completes_every_step() {
        let mut f = ConnectionFlow::new();
        assert_eq!(f.text(None, None, None), "Connection flow: no connection attempt yet\n");
        feed(&mut f, &good_session());
        let sess = f.session();
        assert_eq!(sess.id, 1);
        assert!(sess.active());
        for s in STEPS {
            assert_eq!(status(&f, s), StepStatus::Done, "{s:?}");
        }
        assert!(sess.step(Step::Start).detail.starts_with("00:1A:7D:DA:71:13 codec Some(Ldac)"));
        assert!(sess.step(Step::Btstack).detail.starts_with("company 0x005D"));
        assert_eq!(sess.step(Step::Codec).detail, "LDAC");
        assert!(sess.step(Step::Caps).detail.starts_with("LDAC=true"));
        assert_eq!(sess.step(Step::Capture).detail, "48000 Hz, 2 channel(s), F32 -> 2 channel(s)");
        assert_eq!(sess.step(Step::Config).detail, "local 1, remote 1, mtu 1005, used 1005, limit 1023");
        assert_eq!(sess.step(Step::Stream).time_ms, Some(3000));
        assert_eq!(f.last_state(), Some((StreamState::Streaming, "Connected")));
        assert_eq!(f.adapter_lines().len(), 4, "{:?}", f.adapter_lines());

        let rows = f.rows(Some(StreamState::Streaming), None);
        assert_eq!(rows.len(), 11);
        assert_eq!(rows[Step::Link.index()].detail, "No disconnects");
        assert_eq!(rows[Step::Stream.index()].elapsed_ms, Some(1999));

        let rates = AudioRates { packets_per_second: 150.0, kbps: 990.4, capture_frames_per_second: 48000.0 };
        let rows = f.rows(Some(StreamState::Streaming), Some(rates));
        assert_eq!(rows[Step::Audio.index()].detail, "150 packets/s, 990 kbps");
        let idle = AudioRates::default();
        let rows = f.rows(Some(StreamState::Streaming), Some(idle));
        assert_eq!(rows[Step::Audio.index()].detail, "No audio playing - nothing to send");
        let stalled = AudioRates { capture_frames_per_second: 48000.0, ..idle };
        let rows = f.rows(Some(StreamState::Streaming), Some(stalled));
        assert_eq!(rows[Step::Audio.index()].status, StepStatus::Warning);
        assert_eq!(rows[Step::Audio.index()].detail_msg, Some(Msg::new("dbgc.audio_stalled")));

        let text = f.text(Some(StreamState::Streaming), None, Some(10 * 3_600_000));
        assert!(text.starts_with("Connection flow: attempt #1, started 10:00:01.001\n"), "{text}");
        assert!(text.contains("  [ OK ] Stream start (AVDTP START)       10:00:03.000 +1.999 s\n"), "{text}");
    }

    #[test]
    fn a_running_stack_counts_as_up() {
        let mut f = ConnectionFlow::new();
        feed(&mut f, &good_session());
        // Second attempt: no init lines, the stack is reused
        f.feed(9000, STREAM, Level::INFO, "stream: device 00:1A:7D:DA:71:13 codec None");
        assert_eq!(f.session().id, 2);
        assert_eq!(status(&f, Step::Btstack), StepStatus::Pending);
        let rows = f.rows(Some(StreamState::Connecting), None);
        assert_eq!(rows[Step::Btstack.index()].status, StepStatus::Running);
        assert_eq!(rows[Step::Connect.index()].status, StepStatus::Pending);
        f.feed(9010, "bt_host", Level::INFO, "connecting to 00:1A:7D:DA:71:13");
        assert_eq!(status(&f, Step::Btstack), StepStatus::Done);
        assert!(f.session().step(Step::Btstack).detail.starts_with("company 0x005D"));
        assert_eq!(status(&f, Step::Connect), StepStatus::Running);
        assert_eq!(f.session().step(Step::Connect).detail, "00:1A:7D:DA:71:13");
    }

    #[test]
    fn retried_connection_is_a_warning() {
        let mut f = ConnectionFlow::new();
        f.feed(0, STREAM, Level::INFO, "stream: device AA:BB:CC:DD:EE:FF codec None");
        f.feed(10, "bt_host", Level::INFO, "connecting to AA:BB:CC:DD:EE:FF");
        f.feed(20, APP, Level::WARN, "signaling connection failed (0x04)");
        f.feed(30, "bt_host", Level::WARN, "connection attempt 1/3 failed (NoAnswer), trying again");
        assert_eq!(status(&f, Step::Connect), StepStatus::Running);
        f.feed(40, "bt_host", Level::INFO, "connecting to AA:BB:CC:DD:EE:FF");
        f.feed(50, APP, Level::INFO, "signaling connection established (cid 0x0041, handle 0x000b)");
        assert_eq!(status(&f, Step::Connect), StepStatus::Warning);
        assert!(f.session().step(Step::Connect).detail.ends_with("(after 1 failed attempt(s))"));
        assert_eq!(f.session().issues, 2);
    }

    #[test]
    fn auth_collision_retry_does_not_fail_the_configuration() {
        let mut f = ConnectionFlow::new();
        f.feed(0, STREAM, Level::INFO, "stream: device AA:BB:CC:DD:EE:FF codec LDAC");
        f.feed(10, "bt_host", Level::INFO, "connecting to AA:BB:CC:DD:EE:FF");
        f.feed(20, APP, Level::WARN, "signaling connection failed (0x66)");
        f.feed(20, APP, Level::WARN, "stream establishment failed (0x66)");
        f.feed(20, APP, Level::WARN, "authentication failed (handle 0x0001): 0x0C (Command Disallowed)");
        f.feed(
            20,
            "bt_host",
            Level::WARN,
            "authentication collided with the device's own (0x0C): trying again with the stored link key",
        );
        assert_eq!(status(&f, Step::Config), StepStatus::Pending);
        assert_eq!(status(&f, Step::Connect), StepStatus::Running);
        f.feed(30, "bt_host", Level::INFO, "connecting to AA:BB:CC:DD:EE:FF");
        f.feed(40, APP, Level::INFO, "signaling connection established (cid 0x0002, handle 0x0002)");
        f.feed(50, APP, Level::INFO, "codec configured (local 1, remote 6)");
        f.feed(60, APP, Level::INFO, "stream established (local 1, remote 6, mtu 883, used 883, limit 1023)");
        assert_eq!(status(&f, Step::Connect), StepStatus::Warning);
        assert!(f.session().step(Step::Connect).detail.ends_with("(after 1 failed attempt(s))"));
        assert_eq!(status(&f, Step::Config), StepStatus::Done);
    }

    #[test]
    fn error_state_blames_the_first_open_step() {
        let mut f = ConnectionFlow::new();
        f.feed(0, STREAM, Level::INFO, "stream: device AA:BB:CC:DD:EE:FF codec None");
        f.feed(10, "bt_host", Level::INFO, "connecting to AA:BB:CC:DD:EE:FF");
        f.feed(20, "bt_host", Level::WARN, "connection timed out");
        f.feed(30, SVC, Level::INFO, "state Error: The device did not answer.");
        assert_eq!(status(&f, Step::Connect), StepStatus::Failed);
        assert_eq!(f.session().step(Step::Connect).detail, "The device did not answer.");
        assert_eq!(status(&f, Step::Caps), StepStatus::Pending);
        assert_eq!(f.session().stop_ms, Some(30));
        // Later lines belong to no attempt
        f.feed(40, APP, Level::INFO, "streaming started");
        assert_eq!(status(&f, Step::Stream), StepStatus::Pending);
        let text = f.text(None, None, None);
        assert!(text.contains("stopped T+0.030"), "{text}");
        assert!(text.contains("[FAIL] Connect (ACL / AVDTP signaling)"), "{text}");
    }

    #[test]
    fn a_failure_is_kept_and_warnings_stay() {
        let mut f = ConnectionFlow::new();
        f.feed(0, STREAM, Level::INFO, "stream: device AA:BB:CC:DD:EE:FF codec None");
        f.feed(5, APP, Level::INFO, "hci_init done, powering on");
        f.feed(6, APP, Level::WARN, "HCI init took < 500 ms: Realtek firmware likely NOT loaded");
        f.feed(7, "bt_host", Level::INFO, "HCI ready after 300 ms");
        assert_eq!(status(&f, Step::Btstack), StepStatus::Warning);
        f.feed(10, APP, Level::INFO, "remote SEPs in use by another connection: 1 of 2 audio sink SEP(s)");
        f.feed(11, APP, Level::INFO, "capability discovery complete (SBC=true)");
        assert_eq!(status(&f, Step::Caps), StepStatus::Warning);
        assert_eq!(f.session().step(Step::Caps).detail, "SBC=true");
        f.feed(20, "bt_host", Level::WARN, "set configuration failed (Err)");
        f.feed(21, "bt_host", Level::WARN, "stream configuration timed out or cancelled");
        assert_eq!(status(&f, Step::Config), StepStatus::Failed);
        assert_eq!(f.session().step(Step::Config).detail, "set configuration failed (Err)");
    }

    #[test]
    fn reconnects_are_counted() {
        let mut f = ConnectionFlow::new();
        feed(&mut f, &good_session());
        f.feed(10_000, STREAM, Level::WARN, "connection lost, reconnecting");
        f.feed(10_001, SVC, Level::INFO, "state Reconnecting: Connection lost");
        f.feed(12_999, APP, Level::WARN, "signaling connection failed (0x67)");
        f.feed(12_999, APP, Level::WARN, "stream establishment failed (0x67)");
        f.feed(13_000, STREAM, Level::WARN, "reconnect attempt 1 failed");
        f.feed(16_000, "bt_host", Level::INFO, "reconnecting to 00:1A:7D:DA:71:13");
        f.feed(16_500, STREAM, Level::INFO, "reconnected on attempt 2");
        // A failed attempt does not touch the first connection's steps
        assert_eq!(status(&f, Step::Connect), StepStatus::Done);
        assert_eq!(status(&f, Step::Config), StepStatus::Done);
        f.feed(20_000, STREAM, Level::WARN, "the PC resumed from sleep, reconnecting");
        let s = f.session();
        assert_eq!(s.reconnects, 2);
        assert_eq!(s.step(Step::Link).status, StepStatus::Running);
        let rows = f.rows(Some(StreamState::Reconnecting), None);
        assert_eq!(rows[Step::Link.index()].detail, "2 reconnect(s) - the PC resumed from sleep, reconnecting");
        // Stopping ends the attempt
        f.feed(21_000, STREAM, Level::INFO, "streaming stopped, cleaning up");
        f.feed(21_500, SVC, Level::INFO, "state Idle: Ready");
        assert_eq!(f.session().stop_ms, Some(21_500));
        assert!(!f.session().active());
    }

    #[test]
    fn sleep_is_one_quiet_reconnect() {
        let mut f = ConnectionFlow::new();
        feed(&mut f, &good_session());
        let issues = f.session().issues;
        // As the session logs a sleep that took the adapter away
        f.feed(10_000, SVC, Level::INFO, "the PC is going to sleep");
        f.feed(11_500, "hci", Level::INFO, "HCI transport stopped as expected: ACL read: transfer was cancelled");
        f.feed(11_600, STREAM, Level::INFO, "the PC is going to sleep, reconnecting after it resumes");
        f.feed(11_601, SVC, Level::INFO, "state Reconnecting: Connection lost, reconnecting...");
        f.feed(11_602, SVC, Level::INFO, "stopping BTstack until the PC resumes");
        f.feed(11_603, "bt_host", Level::INFO, "the Bluetooth adapter went away, closing it without HCI power off");
        assert_eq!(f.session().reconnects, 1);
        assert_eq!(status(&f, Step::Link), StepStatus::Running);
        f.feed(60_000, SVC, Level::INFO, "the PC resumed from sleep, Bluetooth will be restarted");
        f.feed(60_100, STREAM, Level::INFO, "the PC resumed from sleep, reconnecting");
        f.feed(63_100, SVC, Level::INFO, "state Reconnecting: Reconnect attempt 1/10...");
        f.feed(63_101, SVC, Level::INFO, "restarting BTstack: the PC resumed from sleep");
        f.feed(63_200, APP, Level::INFO, "hci_init done, powering on");
        f.feed(63_500, APP, Level::INFO, "HCI ready (took 300 ms)");
        f.feed(63_501, "bt_host", Level::INFO, "connecting to 00:1A:7D:DA:71:13");
        f.feed(65_000, STREAM, Level::INFO, "reconnected on attempt 1");
        let s = f.session();
        // Going to sleep and resuming are one interruption
        assert_eq!(s.reconnects, 1);
        assert_eq!(s.issues, issues);
        assert_eq!(s.step(Step::Link).status, StepStatus::Warning);
        // The new stack does not touch the first connection's steps
        assert_eq!(status(&f, Step::Btstack), StepStatus::Done);
        assert_eq!(status(&f, Step::Connect), StepStatus::Done);
    }

    #[test]
    fn app_capture_waits_for_the_app() {
        let mut f = ConnectionFlow::new();
        f.feed(0, STREAM, Level::INFO, "stream: device AA:BB:CC:DD:EE:FF codec None");
        f.feed(1, STREAM, Level::INFO, "spotify.exe is not running yet, waiting for it");
        assert_eq!(status(&f, Step::Audio), StepStatus::Running);
        f.feed(2, STREAM, Level::INFO, "capturing app spotify.exe (PID 1234)");
        assert_eq!(status(&f, Step::Audio), StepStatus::Done);
        assert_eq!(f.session().step(Step::Audio).detail, "spotify.exe (PID 1234)");
        // "audio capture: ..." of audio-capture is detail, of the session an error
        f.feed(3, CAPTURE, Level::INFO, "audio capture: process 1234 and its child processes, 48000 Hz");
        assert_eq!(status(&f, Step::Capture), StepStatus::Pending);
        f.feed(4, STREAM, Level::ERROR, "audio capture: device not found");
        assert_eq!(status(&f, Step::Capture), StepStatus::Failed);
    }

    #[test]
    fn targets_must_match() {
        let mut f = ConnectionFlow::new();
        f.feed(0, STREAM, Level::INFO, "stream: device AA:BB:CC:DD:EE:FF codec None");
        f.feed(1, "bt_hostile", Level::INFO, "streaming started");
        f.feed(2, "a2dpwb_core::streamer", Level::INFO, "codec selected: SBC");
        assert_eq!(status(&f, Step::Stream), StepStatus::Pending);
        assert_eq!(status(&f, Step::Codec), StepStatus::Pending);
    }

    #[test]
    fn masks_addresses() {
        // Every form of the same address -> the same <addr-N>; nothing of
        // it stays (the OUI names the maker)
        let mut m = Masker::new();
        assert_eq!(m.mask("to 00:1A:7D:DA:71:13."), "to <addr-1>.");
        assert_eq!(m.mask("aa:bb:cc:dd:ee:ff"), "<addr-2>");
        assert_eq!(m.mask("[00:11:22:33:44:55|66:77:88:99:AA:BB]"), "[<addr-3>|<addr-4>]");
        assert_eq!(
            m.mask("00-1A-7D-DA-71-13 and 001a7dda7113 and 00:1A:7D:DA:71:13"),
            "<addr-1> and <addr-1> and <addr-1>"
        );
        // A colon after it ends the address (the LE Audio pairing lines of #81)
        assert_eq!(
            m.mask("LE Audio: 46:62:99:4A:5B:6C: pairing (Just Works)"),
            "LE Audio: <addr-5>: pairing (Just Works)"
        );
        assert_eq!(m.mask("to 46:62:99:4A:5B:6C:"), "to <addr-5>:");
        assert_eq!(m.mask("46-62-99-4A-5B-6C-x"), "<addr-5>-x");
        // After a letter: Windows' LE device IDs
        assert_eq!(
            m.mask("BluetoothLE#BluetoothLE00:1a:7d:da:71:13-58:18:62:3a:1b:2c"),
            "BluetoothLE#BluetoothLE<addr-1>-<addr-6>"
        );
        // Part of a longer run (an IPv6 address): left alone
        assert_eq!(m.mask("fe80::00:11:22:33:44:55"), "fe80::00:11:22:33:44:55");
        assert_eq!(m.mask("00:11:22:33:44:55:66"), "00:11:22:33:44:55:66");
        assert_eq!(m.mask("0011:22:33:44:55"), "0011:22:33:44:55");
        assert_eq!(m.mask("00:11:22:33:44:556"), "00:11:22:33:44:556");
        // Not hex
        assert_eq!(m.mask("00:11:22:33:44:5G"), "00:11:22:33:44:5G");
        // Multi-byte text around it
        assert_eq!(m.mask("ヘッドホン 00:11:22:33:44:55 接続"), "ヘッドホン <addr-3> 接続");
        assert_eq!(m.mask("ヘッドホン5818623A1B2C接続"), "ヘッドホン<addr-6>接続");
        assert_eq!(m.mask(""), "");
        // The adapter's own address, in any form
        let mut m = Masker::new();
        m.know_adapter("00:e0:4c:00:00:01");
        m.know_adapter("not an address");
        assert_eq!(
            m.mask(r"local 00:E0:4C:00:00:01, USB\VID_0BDA&PID_4853\00E04C000001, to 00:1A:7D:DA:71:13"),
            r"local <adapter>, USB\VID_0BDA&PID_4853\<adapter>, to <addr-1>"
        );
        assert_eq!(mask_identifiers("00:E0:4C:00:00:01"), "<addr-1>");
    }

    #[test]
    fn masks_bare_addresses() {
        let mut m = Masker::new();
        assert_eq!(
            m.mask(r"BTHENUM\{0000110b-0000-1000-8000-00805f9b34fb}_LOCALMFG&005d\7&1a2b3c4d&0&5818623A1B2C_C00000000"),
            r"BTHENUM\{0000110b-0000-1000-8000-00805f9b34fb}_LOCALMFG&005d\7&1a2b3c4d&0&<addr-1>_C00000000"
        );
        assert_eq!(m.mask(r"BTHLE\DEV_5818623a1b2c\8&1&0"), r"BTHLE\DEV_<addr-1>\8&1&0");
        // A GUID's last part; longer hex, after 0x; not all hex or shorter
        assert_eq!(m.mask("{0000110b-0000-1000-8000-00805f9b34fb}"), "{0000110b-0000-1000-8000-00805f9b34fb}");
        assert_eq!(m.mask("5818623A1B2C5818623A1B2C, 0x5818623A1B2C"), "5818623A1B2C5818623A1B2C, 0x5818623A1B2C");
        assert_eq!(m.mask("5818623A1B2G 818623A1B2C"), "5818623A1B2G 818623A1B2C");
    }

    #[test]
    fn masks_path_heads() {
        let m = mask_identifiers;
        // From the app's own folder on, whatever is above it
        assert_eq!(
            m(r"settings folder: C:\Users\Taro Yamada\AppData\Roaming\A2DPWB"),
            r"settings folder: <path>\A2DPWB"
        );
        assert_eq!(
            m(r"dump: C:\Users\admin\AppData\Roaming\A2DPWB\logs\crash\a2dpwb-core-20261022-235220-831.dmp"),
            r"dump: <path>\A2DPWB\logs\crash\a2dpwb-core-20261022-235220-831.dmp"
        );
        assert_eq!(m(r"D:\Apps\a2dpwb\firmware\x.bin"), r"<path>\a2dpwb\firmware\x.bin");
        assert_eq!(
            m(r"C:\Program Files\A2DP Windows Bridge\a2dpwb-ui.exe"),
            r"<path>\A2DP Windows Bridge\a2dpwb-ui.exe"
        );
        // Without it: the last part only (the executable, an app captured)
        assert_eq!(
            m(r"starting: pid 1, Windows build 26200, I:\A2DP-Windows-Bridge\target\release\a2dpwb-core.exe"),
            r"starting: pid 1, Windows build 26200, <path>\a2dpwb-core.exe"
        );
        assert_eq!(m(r"C:\Users\山田太郎\Music\foobar2000\foobar2000.exe"), r"<path>\foobar2000.exe");
        // The last part takes the text after it (a path is not delimited),
        // but not another path; a trailing separator keeps the folder
        assert_eq!(
            m(r"HCI capture running: C:\x\A2DPWB\hci.pklg (3.0 MB)"),
            r"HCI capture running: <path>\A2DPWB\hci.pklg (3.0 MB)"
        );
        assert_eq!(m(r"moved C:\a\b\x.bin to C:\a\c\x.bin"), r"moved <path>\x.bin to <path>\x.bin");
        assert_eq!(m("folder C:\\a\\b\\\n"), "folder <path>\\b\\\n");
        assert_eq!(m(r"C:\ only"), r"<path>\ only");
        // JSON's doubled '\', '/', a device path, any case, a user's folder
        // at the end (its name as 'x's)
        assert_eq!(m(r#"{"path":"C:\\Users\\taro\\x.txt"}"#), r#"{"path":"<path>\\x.txt"}"#);
        assert_eq!(m("file:///C:/users/taro/x.txt"), "file:///<path>/x.txt");
        assert_eq!(m(r"\\?\C:\USERS\TARO\x"), r"\\?\<path>\x");
        assert_eq!(m("dump: C:\\Users\\taro\n"), "dump: <path>\\xxxx\n");
        assert_eq!(m(r#""C:\Users\taro""#), r#""<path>\xxxx""#);
        // Not paths
        assert_eq!(
            m("at 23:52:25 LE Audio: 46 kHz, x:y, C: drive, https://a.b/c"),
            "at 23:52:25 LE Audio: 46 kHz, x:y, C: drive, https://a.b/c"
        );
        assert_eq!(m(r"\\.\pipe\a2dpwb-x"), r"\\.\pipe\a2dpwb-x");
    }

    #[test]
    fn masks_user_paths() {
        // A user's folder in a path without a drive: the name as 'x's,
        // one per character
        let m = mask_identifiers;
        assert_eq!(m(r"\Users\Taro Yamada\AppData\Roaming\A2DPWB"), r"\Users\xxxxxxxxxxx\AppData\Roaming\A2DPWB");
        assert_eq!(m(r"\Users\山田太郎\AppData"), r"\Users\xxxx\AppData");
        assert_eq!(m(r"/users/taro/x.txt"), r"/users/xxxx/x.txt");
        // Nobody's folders, and "Users" elsewhere
        assert_eq!(m(r"\Users\Public\Desktop"), r"\Users\Public\Desktop");
        assert_eq!(m(r"\Users\Default\x and \Users\All Users\y"), r"\Users\Default\x and \Users\All Users\y");
        assert_eq!(m("3 Users, \\Users\\\n"), "3 Users, \\Users\\\n");
        assert_eq!(m("in Users\\taro"), "in Users\\taro");
        assert_eq!(m(r"\Users"), r"\Users");
        // The name stops at what a name cannot contain
        assert_eq!(m("dump: \\Users\\taro\n"), "dump: \\Users\\xxxx\n");
        assert_eq!(m(r#""\Users\taro""#), r#""\Users\xxxx""#);
    }

    #[test]
    fn masks_sids() {
        let m = mask_identifiers;
        assert_eq!(
            m(r"IPC: listening on \\.\pipe\a2dpwb-S-1-5-21-2123068572-4202720387-3275098450-1001"),
            r"IPC: listening on \\.\pipe\a2dpwb-<sid>"
        );
        assert_eq!(m("user S-1-5-21-1-2-3-500, then"), "user <sid>, then");
        // An instance suffix after it (ipc::instance_suffix)
        assert_eq!(m(r"S-1-5-21-1-2-3-1001\x and a2dpwb-S-1-5-21-1-2-3-1001-dev"), r"<sid>\x and a2dpwb-<sid>-dev");
        // Well-known SIDs, the start of one, a longer word
        assert_eq!(m("S-1-5-18 S-1-5-32-544 S-1-5-21- S-1-"), "S-1-5-18 S-1-5-32-544 S-1-5-21- S-1-");
        assert_eq!(m("xS-1-5-21-1-2-3-4 S-1-5-21-1-2-3-4x"), "xS-1-5-21-1-2-3-4 S-1-5-21-1-2-3-4x");
    }

    #[test]
    fn masks_device_names() {
        let mut m = Masker::new();
        // Quoted names, the same name the same number
        assert_eq!(
            m.mask("stream: device AA:BB:CC:DD:EE:FF source virtual device “Taro's AirPods”"),
            "stream: device <addr-1> source virtual device <device-1>"
        );
        assert_eq!(
            m.mask("inquiry result: 11:22:33:44:55:66 name=“Hanako's iPhone”"),
            "inquiry result: <addr-2> name=<device-2>"
        );
        assert_eq!(
            m.mask("remote name of AA:BB:CC:DD:EE:FF: name=“Taro's AirPods”"),
            "remote name of <addr-1>: name=<device-1>"
        );
        // Empty, unclosed, nested and multi-line quotes stay
        assert_eq!(m.mask("name=“” and “open"), "name=“” and “open");
        assert_eq!(m.mask("“a\nb”"), "“a\nb”");
        assert_eq!(m.mask("“x “y” z”"), "“x <device-3> z”");
        // Known names, quoted or not, inside other names; longest first
        let mut m = Masker::new();
        m.know("Taro's AirPods");
        m.know("Headphones (Taro's AirPods Stereo)");
        m.know("");
        m.know("LE");
        assert_eq!(
            m.mask(
                "Device: Taro's AirPods [AA:BB:CC:DD:EE:FF], loopback of “Headphones (Taro's AirPods Stereo)”, LE on"
            ),
            "Device: <device-1> [<addr-1>], loopback of <device-2>, LE on"
        );
        assert_eq!(m.mask(r#""device_name": "Taro's AirPods""#), r#""device_name": "<device-1>""#);
        assert_eq!(m.mask("AA:BB:CC:DD:EE:FF|Taro's AirPods"), "<addr-1>|<device-1>");
        // Known again: the same number
        m.know("Taro's AirPods");
        assert_eq!(m.mask("“Taro's AirPods”"), "<device-1>");
        // Names and addresses do not mask each other's text
        assert_eq!(mask_identifiers("“AA:BB:CC:DD:EE:FF”"), "<device-1>");
    }

    #[test]
    fn masks_usb_serials() {
        let m = mask_identifiers;
        // A serial that is an address is one (the adapter's, see masks_addresses)
        assert_eq!(
            m(r"USB: opened 0BDA:4853 USB\VID_0BDA&PID_4853\00E04C000001 (event 0x81)"),
            r"USB: opened 0BDA:4853 USB\VID_0BDA&PID_4853\<addr-1> (event 0x81)"
        );
        assert_eq!(m(r"USB\VID_18D1&PID_4EE7\HT7A1B234567"), r"USB\VID_18D1&PID_4EE7\xxxxxxxxxxxx");
        // A device path, any case; JSON's doubled '\'
        assert_eq!(
            m(r"\\?\usb#vid_0a12&pid_0001#serial42#{a5dcbf10-6530-11d2-901f-00c04fb951ed}"),
            r"\\?\usb#vid_0a12&pid_0001#xxxxxxxx#{a5dcbf10-6530-11d2-901f-00c04fb951ed}"
        );
        assert_eq!(
            m(r#"{"instance_id":"USB\\VID_2357&PID_0604\\SN123"}"#),
            r#"{"instance_id":"USB\\VID_2357&PID_0604\\xxxxx"}"#
        );
        // An interface of a composite device; IDs Windows generates stay
        assert_eq!(m(r"USB\VID_8087&PID_0026&MI_00\SN123"), r"USB\VID_8087&PID_0026&MI_00\xxxxx");
        assert_eq!(m(r"USB\VID_8087&PID_0026\5&2C7B4E3&0&14."), r"USB\VID_8087&PID_0026\5&2C7B4E3&0&14.");
        // A driver record's name
        assert_eq!(m("USB_VID_0BDA_PID_8771_SN123.json"), "USB_VID_0BDA_PID_8771_xxxxx.json");
        assert_eq!(m("USB_VID_0BDA_PID_8771_00E04C000001.json"), "USB_VID_0BDA_PID_8771_<addr-1>.json");
        // VID and PID alone
        assert_eq!(
            m("RTL8852BU (0BDA:4853 -> 0xB85B), VID_0BDA&PID_4853."),
            "RTL8852BU (0BDA:4853 -> 0xB85B), VID_0BDA&PID_4853."
        );
    }

    #[test]
    fn times() {
        assert_eq!(format_time(1_234, None), "T+1.234");
        assert_eq!(format_time(61_005, Some(23 * 3_600_000 + 59 * 60_000)), "00:00:01.005");
        assert_eq!(format_elapsed(1_999), "+1.999 s");
    }

    #[test]
    fn diagnostics_text() {
        let mut f = ConnectionFlow::new();
        let session = good_session();
        let mut log: Vec<LogEntry<'_>> = vec![LogEntry {
            time_ms: 10,
            level: Level::WARN,
            target: "a2dpwb_core::service",
            message: "before the attempt",
        }];
        for (t, target, level, msg) in &session {
            f.feed(*t, target, *level, msg);
            log.push(LogEntry { time_ms: *t, level: *level, target, message: msg });
        }
        log.push(LogEntry {
            time_ms: 4000,
            level: Level::WARN,
            target: APP,
            message: "AVRCP connect request failed (0x04) for 00:1A:7D:DA:71:13",
        });
        let settings = Settings::default();
        let profile = Profile {
            name: "WH".into(),
            device_address: "00:1A:7D:DA:71:13".into(),
            device_name: "WH-1000XM4".into(),
            codec: "ldac".into(),
            ..Profile::default()
        };
        let info = StreamInfo {
            codec: "LDAC".into(),
            bitrate_kbps: 990,
            sample_rate: 48000,
            channels: 2,
            bits: 24,
            source_sample_rate: 48000,
            source_channels: 2,
            source_bits: 32,
            requested_codec: None,
            requested_in_use: false,
            abr: false,
            bitrate_capped: false,
            source: crate::stream::SourceInfo { kind: "app", name: "Spotify.exe".into(), waiting: true },
            details: Some(Box::new(crate::CodecDetails {
                mtu: 679,
                rtp: true,
                media_header: 1,
                frame_samples: 128,
                frame_bytes: Some(119),
                frames_per_packet: Some(5),
                sbc: Some(crate::SbcDetails {
                    channel_mode: "joint_stereo".into(),
                    blocks: 16,
                    subbands: 8,
                    allocation: "loudness".into(),
                    bitpool: 53,
                    config_bitpool: Some((2, 53)),
                    abr_bitpool: None,
                    device: Some(crate::SbcDeviceCaps {
                        sample_rates: vec![44100, 48000],
                        channel_modes: vec!["mono".into(), "joint_stereo".into()],
                        blocks: vec![4, 8, 12, 16],
                        subbands: vec![4, 8],
                        allocations: vec!["snr".into(), "loudness".into()],
                        min_bitpool: 2,
                        max_bitpool: 53,
                    }),
                }),
                aac: None,
                device: None,
            })),
        };
        let stats = LinkStats::default();
        stats.packets_sent.store(1500, std::sync::atomic::Ordering::Relaxed);
        let adapters = vec!["Realtek RTL8761B [0BDA:8771], driver WinUsb".to_string()];
        let mut input = DiagInput {
            env: Environment {
                generated: "2026-10-17 10:00:00".into(),
                app_version: "2.0.0-alpha.1".into(),
                windows_build: 26200,
                arch: "x86_64",
            },
            settings: &settings,
            state: StreamState::Streaming,
            adapters: &adapters,
            firmware: Some("present"),
            profile: Some(&profile),
            stream: Some(&info),
            stats: Some(&stats),
            hci_capture: Some((Path::new("C:\\a2dpwb\\hci.pklg"), 3 * 1024 * 1024)),
            flow: &f,
            rates: None,
            log: &log,
            clock: None,
            mask: true,
            adapter_addresses: &[],
            le_audio: None,
            logging: None,
        };
        let text = diag_text(&input);
        assert!(text.starts_with("```text\nA2DPWB diagnostics - 2026-10-17 10:00:00\n"), "{text}");
        assert!(text.ends_with("```\n"));
        assert!(text.contains("OS           : Windows build 26200 (x86_64)\n"));
        assert!(text.contains("State        : Streaming (Connected)\n"));
        assert!(text.contains("  Setting: chip PID 0x0000 (auto)\n"), "{text}");
        assert!(text.contains("  Realtek RTL8761B [0BDA:8771], driver WinUsb\n"));
        assert!(text.contains("  chip: RTL8761B (0x8771) (auto)\n"));
        // The device's name goes with the addresses (the profile's names are known)
        assert!(text.contains("  Device: <device-1> [<addr-1>]\n"), "{text}");
        assert!(!text.contains("00:1A:7D"), "{text}");
        assert!(!text.contains("WH-1000XM4"), "{text}");
        assert!(text.contains("  Codec: ldac, quality hq, ABR off, sample rate auto, bit depth auto\n"));
        assert!(text.contains("  LDAC 48000 Hz 2 ch, 990 kbps; source 48000 Hz 2 ch 32 bit\n"));
        assert!(text.contains("  Source: app 'Spotify.exe' (waiting for the app to start)\n"));
        assert!(text.contains(
            "  Packets: media payload up to 679 bytes, RTP, 1-byte payload header; 5 frames of 119 bytes (128 samples per frame)\n"
        ));
        assert!(text.contains("  SBC: joint_stereo, 16 blocks, 8 subbands, loudness, bitpool 53 (configured 2-53)\n"));
        assert!(text.contains(
            "  Device SBC: 44.1/48 kHz; mono/joint_stereo; blocks 4/8/12/16; subbands 4/8; snr/loudness; bitpool 2-53\n"
        ));
        assert!(text.contains("  Media packets sent 1500 (0 bytes), queue drops 0, send errors 0\n"));
        // Masked: the path from the app's folder on
        assert!(text.contains("  HCI capture running: <path>\\a2dpwb\\hci.pklg (3.0 MB)\n"), "{text}");
        assert!(text.contains("[Flow]\nConnection flow: attempt #1, started T+1.001\n"));
        // Issues and lines of the current attempt only
        assert!(!text.contains("before the attempt"));
        assert!(text.contains(
            "[Warnings and errors]\n  T+4.000      WARN  bt_host::app: AVRCP connect request failed (0x04) for \
             <addr-1>\n"
        ));
        assert!(text.contains("T+3.000      INFO  bt_host::app: streaming started\n"));
        assert!(!text.contains("DA:71:13"));

        input.mask = false;
        input.profile = None;
        input.state = StreamState::Idle;
        let text = diag_text(&input);
        assert!(text.contains("DA:71:13"));
        assert!(text.contains("  (none selected - receiver measurement or not connected)\n"));
        assert!(text.contains("[Stream]\n  not streaming\n"));
        // LE Audio (experimental): off and no LE stack, no section
        let mut off_settings = settings.clone();
        off_settings.le_audio_experimental = false;
        input.settings = &off_settings;
        let text = diag_text(&input);
        assert!(!text.contains("[LE Audio"));

        let mut le_settings = settings.clone();
        le_settings.le_audio_experimental = true;
        input.settings = &le_settings;
        let text = diag_text(&input);
        assert!(text.contains("[LE Audio (experimental)]\n  Setting: on, low latency off; BTstack LE not running\n\n"));
        let le = LeAudioDiag {
            stream: Some("LC3 48 kHz 10 ms 100 octets (80 kbps/ch), 2 ch over 2 CIS, 48_4_2".into()),
            stats: Some(bt_host::LeStreamStats {
                sets_encoded: 990,
                sets_sent: 1000,
                sdus_sent: 2000,
                silence_sets: 12,
                underruns: 1,
                overflows: 3,
                send_errors: 0,
                prefill: 6,
            }),
            devices: vec![bt_host::LeDeviceInfo {
                id: bt_host::LeDeviceId(1),
                addr: bt_host::LeAddr { addr: [0xC0, 0x11, 0x22, 0x33, 0x44, 0x55], addr_type: 0 },
                name: "Earbuds".into(),
                connected: true,
                ready: true,
                last_error: 0,
                sink_locations: 3,
                sink_ases: 2,
                pacs: String::new(),
                roles: "TMAP: UMR, GMAP: UGT (UGT features 0x3C: sink)".into(),
            }],
        };
        input.le_audio = Some(&le);
        input.mask = true;
        let text = diag_text(&input);
        assert!(text.contains("  Setting: on, low latency off; BTstack LE active\n"), "{text}");
        assert!(text.contains("  Stream: LC3 48 kHz 10 ms 100 octets (80 kbps/ch), 2 ch over 2 CIS, 48_4_2\n"));
        assert!(text.contains(
            "  ISO: 1000 SDU sets sent (2000 SDUs), 12 silence, 1 underruns, 3 dropped, 0 send errors, prefill 6\n"
        ));
        // The device's name is masked with the rest
        assert!(
            text.contains(": ready, 2 sink ASE(s), TMAP: UMR, GMAP: UGT (UGT features 0x3C: sink)\n")
                && text.contains("  Device <addr-")
                && !text.contains("Earbuds"),
            "{text}"
        );
        input.mask = false;
        let text = diag_text(&input);
        assert!(text.contains("  Device C0:11:22:33:44:55 \u{201C}Earbuds\u{201D}: ready"), "{text}");
    }

    #[test]
    fn flow_from_kept_lines() {
        let lines: Vec<LogEntry<'_>> = good_session()
            .into_iter()
            .map(|(time_ms, target, level, message)| LogEntry { time_ms, level, target, message })
            .collect();
        let f = ConnectionFlow::from_lines(lines);
        assert_eq!(f.session().id, 1);
        assert_eq!(f.lines(), good_session().len() as u64);
        assert_eq!(status(&f, Step::Audio), StepStatus::Done);
    }
}
