//! The connection to a2dpwb-core.exe: the user's Named Pipe, Hello, every
//! server message forwarded to the windows as the `ipc://message` event,
//! requests matched to their responses by id, and reconnecting when the pipe
//! breaks (the core restarted or crashed).
//!
//! The backend keeps no state of the core beyond what the transport needs:
//! the requests waiting for their answer and the diagnostics subscriptions
//! of each window (the core has one subscription per connection; both
//! windows share this connection). Snapshots, events and revisions are the
//! frontend's (src/ipc/store.ts).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use ipc::{ClientMessage, ErrorInfo, LogLevel, Method, Msg, PROTOCOL_VERSION};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter};
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient};
use tokio::sync::{mpsc, oneshot};

/// Event with every server message (JSON as the core sent it)
pub const MESSAGE_EVENT: &str = "ipc://message";
/// Event with the connection [`Status`]
pub const STATUS_EVENT: &str = "ipc://status";

/// All pipe instances busy: try again shortly
const ERROR_PIPE_BUSY: i32 = 231;
/// How long the core may take to create its pipe after we started it
const CORE_START_WAIT: Duration = Duration::from_secs(8);
/// Starting the core again after it did not come up
const CORE_RESPAWN_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Status {
    /// Looking for the core's pipe (also while it starts)
    Connecting,
    Connected {
        server: String,
    },
    /// a2dpwb-core.exe is not next to a2dpwb.exe, or did not start
    CoreMissing {
        detail: String,
    },
    /// The core speaks another protocol version
    Rejected {
        reason: String,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
struct Subscription {
    logs: Option<LogLevel>,
    link: bool,
    #[serde(default)]
    receiver: bool,
}

impl Subscription {
    fn method(self) -> Method {
        Method::DiagSubscribe { logs: self.logs, link: self.link, receiver: Some(self.receiver) }
    }
}

type Pending = oneshot::Sender<Result<Value, ErrorInfo>>;

pub struct Ipc {
    status: Mutex<Status>,
    out: Mutex<Option<mpsc::UnboundedSender<ClientMessage>>>,
    pending: Mutex<HashMap<u64, Pending>>,
    next_id: AtomicU64,
    /// diag.subscribe per window label; the core gets their union
    subscriptions: Mutex<HashMap<String, Subscription>>,
    /// An update stops the core (update.rs): its Bye does not end the UI,
    /// and it is not started again
    updating: AtomicBool,
}

/// A poisoned lock only means another thread panicked while holding it;
/// the maps stay usable.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn error(code: &str, key: &str, args: &[String]) -> ErrorInfo {
    ErrorInfo { code: code.into(), message: Msg { key: key.into(), args: args.to_vec() } }
}

/// How long to wait for the answer. Most calls are quick; scans, the
/// stack's start and driver changes take seconds, and a driver change waits
/// for the user to answer the UAC prompt.
fn timeout_for(method: &str, params: &Value) -> Duration {
    match method {
        // The inquiries, the name requests after the last one (~7 s), the
        // LE Audio scan (~6 s) and the stack's start
        "device.scan" => {
            let seconds = params.get("seconds").and_then(Value::as_u64).unwrap_or(10);
            Duration::from_secs(30.max(seconds + 25))
        }
        "adapter.assign_winusb" | "adapter.restore_driver" => Duration::from_secs(120),
        // Up to two files, each fetched with a 30 s limit, then the
        // adapters are listed again
        "adapter.download_firmware" => Duration::from_secs(90),
        "stream.start"
        | "stream.start_custom"
        | "adapter.restart"
        | "adapter.list"
        | "adapter.firmware_files"
        | "audio.devices"
        | "audio.apps"
        | "diag.info"
        | "diag.capture_start"
        | "diag.capture_stop"
        | "app.check_update"
        | "state.snapshot"
        | "settings.set"
        | "receiver.discover"
        | "receiver.connect"
        | "receiver.disconnect" => Duration::from_secs(30),
        _ => Duration::from_secs(5),
    }
}

impl Ipc {
    pub fn new() -> Arc<Ipc> {
        Arc::new(Ipc {
            status: Mutex::new(Status::Connecting),
            out: Mutex::new(None),
            pending: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            subscriptions: Mutex::new(HashMap::new()),
            updating: AtomicBool::new(false),
        })
    }

    pub fn set_updating(&self, on: bool) {
        self.updating.store(on, Ordering::Release);
    }

    fn updating(&self) -> bool {
        self.updating.load(Ordering::Acquire)
    }

    pub fn status(&self) -> Status {
        lock(&self.status).clone()
    }

    fn set_status(&self, app: &AppHandle, status: Status) {
        let mut s = lock(&self.status);
        if *s != status {
            match &status {
                Status::Connected { server } => tracing::info!("core connected ({server})"),
                Status::Connecting => tracing::info!("connecting to the core"),
                other => tracing::warn!("core: {other:?}"),
            }
            *s = status.clone();
            drop(s);
            let _ = app.emit(STATUS_EVENT, status);
        }
    }

    /// Sends a request and waits for its answer. `window`: the label of the
    /// window asking (for diag.subscribe).
    pub async fn request(&self, window: &str, method: &str, params: Option<Value>) -> Result<Value, ErrorInfo> {
        let params = params.filter(|p| !p.is_null());
        // The protocol's own types check the method and its parameters, so
        // a typo in the frontend is reported here and not by the core
        let mut wire = serde_json::json!({ "method": method });
        if let Some(p) = &params {
            wire["params"] = p.clone();
        }
        let mut call: Method = serde_json::from_value(wire)
            .map_err(|e| error("ipc.bad_request", "ipc.error.bad_request", &[method.into(), e.to_string()]))?;
        if let Method::DiagSubscribe { logs, link, receiver } = call {
            let merged = {
                let mut subs = lock(&self.subscriptions);
                subs.insert(window.into(), Subscription { logs, link, receiver: receiver.unwrap_or(false) });
                merge(subs.values())
            };
            call = merged.method();
        }
        let timeout = timeout_for(method, params.as_ref().unwrap_or(&Value::Null));
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        lock(&self.pending).insert(id, tx);
        let sent =
            lock(&self.out).as_ref().map(|out| out.send(ClientMessage::Request { id, call }).is_ok()).unwrap_or(false);
        if !sent {
            lock(&self.pending).remove(&id);
            return Err(error("ipc.not_connected", "ipc.error.not_connected", &[]));
        }
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(result)) => result,
            // The sender was dropped: the connection broke
            Ok(Err(_)) => Err(error("ipc.disconnected", "ipc.error.disconnected", &[])),
            Err(_) => {
                lock(&self.pending).remove(&id);
                Err(error("ipc.timeout", "ipc.error.timeout", &[method.into()]))
            }
        }
    }

    /// Forgets a window's subscription (window closed for good)
    pub fn drop_window(&self, window: &str) {
        let merged = {
            let mut subs = lock(&self.subscriptions);
            if subs.remove(window).is_none() {
                return;
            }
            merge(subs.values())
        };
        self.send_subscription(merged);
    }

    fn send_subscription(&self, sub: Subscription) {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        if let Some(out) = lock(&self.out).as_ref() {
            let _ = out.send(ClientMessage::Request { id, call: sub.method() });
        }
    }

    fn on_response(&self, msg: &Value) {
        #[derive(Deserialize)]
        struct Response {
            id: u64,
            result: Option<Value>,
            error: Option<ErrorInfo>,
        }
        let Ok(r) = serde_json::from_value::<Response>(msg.clone()) else { return };
        let Some(p) = lock(&self.pending).remove(&r.id) else { return };
        let result = match r.error {
            Some(e) => Err(e),
            None => Ok(r.result.unwrap_or(Value::Null)),
        };
        let _ = p.send(result);
    }

    fn fail_pending(&self) {
        // Dropping the senders wakes every waiter with "disconnected"
        lock(&self.pending).clear();
    }
}

fn merge<'a>(subs: impl Iterator<Item = &'a Subscription>) -> Subscription {
    subs.fold(Subscription::default(), |acc, s| Subscription {
        // LogLevel orders from Error (fewest lines) to Trace (most): the
        // most detailed level any window wants
        logs: acc.logs.max(s.logs),
        link: acc.link || s.link,
        receiver: acc.receiver || s.receiver,
    })
}

/// Starts the connection task (runs for the life of the process)
pub fn start(app: AppHandle, ipc: Arc<Ipc>) {
    tauri::async_runtime::spawn(async move { run(app, ipc).await });
}

enum Ended {
    /// The pipe broke or the core closed it
    Broken,
    /// The core said Bye: it is shutting down on purpose (tray Exit,
    /// app.quit), so the UI ends too instead of starting it again
    Bye,
    Rejected,
}

async fn run(app: AppHandle, ipc: Arc<Ipc>) {
    let name = match win_sys::pipe::current_user_sid() {
        Ok(sid) => ipc::pipe_name(&sid),
        Err(e) => {
            ipc.set_status(&app, Status::CoreMissing { detail: format!("user SID: {e}") });
            return;
        }
    };
    loop {
        ipc.set_status(&app, Status::Connecting);
        let pipe = connect(&app, &ipc, &name).await;
        let ended = session(&app, &ipc, pipe).await;
        *lock(&ipc.out) = None;
        ipc.fail_pending();
        match ended {
            // The update's installer runs next and starts the UI again
            Ended::Bye if ipc.updating() => tokio::time::sleep(Duration::from_millis(300)).await,
            Ended::Bye => {
                app.exit(0);
                return;
            }
            // Another major version: retrying soon will not help, but the
            // user may update the core while the window stays open
            Ended::Rejected => tokio::time::sleep(Duration::from_secs(10)).await,
            Ended::Broken => tokio::time::sleep(Duration::from_millis(300)).await,
        }
    }
}

/// Opens the pipe, starting the core when it does not exist
async fn connect(app: &AppHandle, ipc: &Ipc, name: &str) -> NamedPipeClient {
    let mut spawned: Option<Instant> = None;
    // The last start failed (its reason is the status already)
    let mut spawn_failed = false;
    loop {
        match ClientOptions::new().open(name) {
            Ok(pipe) => return pipe,
            Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY) => {
                tokio::time::sleep(Duration::from_millis(50)).await;
                continue;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && ipc.updating() => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let due = spawned.is_none_or(|t| t.elapsed() >= CORE_RESPAWN_INTERVAL);
                if due {
                    spawned = Some(Instant::now());
                    spawn_failed = match spawn_core() {
                        Ok(()) => false,
                        Err(detail) => {
                            // Keep looking: the user may start it by hand
                            ipc.set_status(app, Status::CoreMissing { detail });
                            true
                        }
                    };
                } else if !spawn_failed && spawned.is_some_and(|t| t.elapsed() >= CORE_START_WAIT) {
                    ipc.set_status(app, Status::CoreMissing { detail: "a2dpwb-core.exe did not open its pipe".into() });
                }
            }
            Err(_) => {}
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// Starts `a2dpwb-core.exe --background` from a2dpwb.exe's folder (the
/// installer puts it there as a Tauri sidecar)
fn spawn_core() -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    // The core outlives the UI: no console, not in our process group (a
    // Ctrl+C in a developer's terminal must not reach it)
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let core = exe.with_file_name("a2dpwb-core.exe");
    if !core.is_file() {
        return Err(format!("{} not found", core.display()));
    }
    std::process::Command::new(&core)
        .arg("--background")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("{}: {e}", core.display()))
}

async fn session(app: &AppHandle, ipc: &Ipc, pipe: NamedPipeClient) -> Ended {
    let (mut rd, mut wr) = tokio::io::split(pipe);
    let hello =
        ClientMessage::Hello { protocol: PROTOCOL_VERSION, client: format!("a2dpwb-ui/{}", env!("CARGO_PKG_VERSION")) };
    if ipc::write_frame(&mut wr, &hello).await.is_err() {
        return Ended::Broken;
    }
    let (tx, mut rx) = mpsc::unbounded_channel::<ClientMessage>();
    *lock(&ipc.out) = Some(tx);

    let writer = async move {
        while let Some(m) = rx.recv().await {
            if ipc::write_frame(&mut wr, &m).await.is_err() {
                break;
            }
        }
    };
    let reader = async {
        loop {
            // Read as plain JSON: a newer minor version of the core may send
            // event kinds this build does not know; the frontend ignores
            // them instead of the connection failing here
            let msg: Value = match ipc::read_frame(&mut rd).await {
                Ok(Some(m)) => m,
                Ok(None) | Err(_) => return Ended::Broken,
            };
            match msg.get("type").and_then(Value::as_str) {
                Some("welcome") => {
                    let server = msg.get("server").and_then(Value::as_str).unwrap_or("").to_string();
                    ipc.set_status(app, Status::Connected { server });
                    // A new connection has no subscription yet
                    let merged = merge(lock(&ipc.subscriptions).values());
                    if merged != Subscription::default() {
                        ipc.send_subscription(merged);
                    }
                }
                Some("reject") => {
                    let reason = msg.get("reason").and_then(Value::as_str).unwrap_or("").to_string();
                    ipc.set_status(app, Status::Rejected { reason });
                    return Ended::Rejected;
                }
                Some("response") => {
                    ipc.on_response(&msg);
                    continue;
                }
                _ => {}
            }
            let bye = msg.pointer("/event/kind").and_then(Value::as_str) == Some("bye");
            let _ = app.emit(MESSAGE_EVENT, msg);
            if bye {
                return Ended::Bye;
            }
        }
    };
    tokio::select! {
        () = writer => Ended::Broken,
        ended = reader => ended,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscriptions_merge_to_the_most_detailed() {
        let subs = [
            Subscription { logs: Some(LogLevel::Info), link: false, receiver: false },
            Subscription { logs: None, link: true, receiver: false },
            Subscription { logs: Some(LogLevel::Debug), link: false, receiver: true },
        ];
        assert_eq!(merge(subs.iter()), Subscription { logs: Some(LogLevel::Debug), link: true, receiver: true });
        assert_eq!(merge([].iter()), Subscription::default());
    }

    #[test]
    fn long_calls_get_long_timeouts() {
        assert_eq!(timeout_for("stream.stop", &Value::Null), Duration::from_secs(5));
        assert_eq!(timeout_for("device.scan", &serde_json::json!({ "seconds": 3 })), Duration::from_secs(30));
        assert_eq!(timeout_for("device.scan", &serde_json::json!({ "seconds": 30 })), Duration::from_secs(55));
        assert!(timeout_for("adapter.assign_winusb", &Value::Null) >= Duration::from_secs(60));
        // Two 30 s fetches and the adapter list
        assert!(timeout_for("adapter.download_firmware", &Value::Null) > Duration::from_secs(60));
        // Discovery waits 1.5 s; connecting ends an older connection first
        assert!(timeout_for("receiver.discover", &Value::Null) >= Duration::from_secs(10));
        assert!(timeout_for("receiver.connect", &Value::Null) >= Duration::from_secs(10));
    }
}
