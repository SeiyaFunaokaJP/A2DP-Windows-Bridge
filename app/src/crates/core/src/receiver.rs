//! Peer receiver test: statistics from a measuring receiver on another PC —
//! v1's remote_sink_client.cpp.
//!
//! tools/linux_sink/bluecaster_sink.py runs on a Linux PC as an ordinary A2DP
//! sink and measures what arrives: lost / late packets, jitter, gaps,
//! dropouts of a modelled playout buffer, frame and decode errors, RSSI. It
//! serves one JSON object per line over TCP (a "hello" line, then "stats"
//! about once per second) and takes "configure" lines back; a
//! [`ReceiverClient`] keeps that connection (reconnecting every few seconds)
//! and holds the latest statistics for the link quality view and the CLI.
//!
//! The receiver also answers a UDP "discover" broadcast on the same port, so
//! it can be found on the local network: target "auto", or [`discover`].
//!
//! Plain std sockets on one thread per client: one line a second does not
//! need an async runtime, and a blocking read with a timeout notices a
//! receiver that went away.

use std::io::{ErrorKind, Read, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;

/// TCP (statistics) and UDP (discovery) port of bluecaster_sink.py
pub const DEFAULT_PORT: u16 = 51201;
/// Target meaning: find the receiver on the local network
pub const AUTO: &str = "auto";
/// How long [`discover`] waits for answers by default
pub const DISCOVER_TIMEOUT: Duration = Duration::from_millis(1500);

const RECONNECT_DELAY: Duration = Duration::from_secs(3);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
/// The receiver sends once per second; silence this long means it is gone
const RECEIVE_TIMEOUT: Duration = Duration::from_secs(5);
/// Reads wake this often to see a stop: on Windows, shutting the socket down
/// from another thread does not end a blocked receive (v1 waited for the
/// receive timeout)
const READ_POLL: Duration = Duration::from_millis(200);
/// A line longer than this is not the receiver talking
const MAX_LINE: usize = 1 << 20;
const DISCOVER_REQUEST: &[u8] = br#"{"type":"discover","version":1,"tool":"A2DPWB"}"#;

/// A receiver: from a discovery reply or the hello line of a connection
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ReceiverInfo {
    /// Address it answered from (or was connected to)
    pub ip: String,
    pub port: u16,
    /// Its host name
    pub host: String,
    /// Its Bluetooth adapter: the device to stream to
    pub bt_address: String,
    pub bt_name: String,
    pub codecs: Vec<String>,
    /// L2CAP MTU it offers for AVDTP
    pub mtu: u32,
    /// Its playout buffer model (0 = not said)
    pub buffer_ms: u32,
}

impl ReceiverInfo {
    /// "Receiver: host (ip:port), Bluetooth XX:.. "name", L2CAP MTU n" (the
    /// CLI's line after discovery)
    pub fn describe(&self, first_of_several: bool) -> String {
        format!(
            "Receiver: {} ({}:{}), Bluetooth {} \"{}\", L2CAP MTU {}{}",
            self.host,
            self.ip,
            self.port,
            self.bt_address,
            self.bt_name,
            self.mtu,
            if first_of_several { " (first of several)" } else { "" }
        )
    }
}

/// The stream the receiver measures. Counters are cumulative for that
/// stream (`id`); they restart with a new stream.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ReceivedStream {
    pub id: u32,
    pub codec: String,
    pub config: String,
    /// The receiver decodes (not only header checks)
    pub decoding: bool,
    pub packets: u64,
    pub bytes: u64,
    pub frames: u64,
    pub lost: u64,
    pub late: u64,
    pub ts_errors: u64,
    pub frame_errors: u64,
    pub decode_errors: u64,
    /// Dropouts of the modelled playout buffer, and skips (overflows)
    pub underruns: u64,
    pub overflows: u64,
    /// The source paused sending
    pub pauses: u64,
    /// The source sends nothing right now
    pub idle: bool,
    pub audio_ms: f64,
    pub underrun_ms: f64,
    pub overflow_ms: f64,
    pub max_gap_ms: f64,
    pub jitter_ms: f64,
    /// Modelled playout buffer, None = not playing
    pub buffer_ms: Option<f64>,
    /// Last reporting interval (about one second)
    pub interval_packets: u64,
    pub interval_bytes: u64,
    pub interval_max_gap_ms: f64,
}

impl ReceivedStream {
    /// Share of the packets that were lost, in percent
    pub fn loss_percent(&self) -> f64 {
        let expected = self.packets + self.lost;
        if expected == 0 { 0.0 } else { 100.0 * self.lost as f64 / expected as f64 }
    }

    pub fn errors(&self) -> u64 {
        self.ts_errors + self.frame_errors + self.decode_errors
    }
}

/// A "stats" message
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ReceiverStats {
    /// When it arrived (staleness)
    #[serde(skip)]
    pub received: Option<Instant>,
    /// A source is connected to the receiver
    pub connected: bool,
    /// Media is flowing right now
    pub streaming: bool,
    /// Bluetooth address of that source
    pub device: String,
    pub buffer_target_ms: u32,
    /// L2CAP MTU the receiver offers for new streams
    pub mtu: u32,
    /// Codecs it offers
    pub codecs: Vec<String>,
    /// BR/EDR RSSI relative to the golden receive range (not dBm)
    pub rssi: Option<i32>,
    pub tx_power: Option<i32>,
    /// Why RSSI is missing, if the receiver says
    pub rssi_note: String,
    /// Channels the link hops over, seen from the receiver
    pub afh_channels: Option<u32>,
    pub stream: Option<ReceivedStream>,
}

impl ReceiverStats {
    /// The CLI's line about what the receiver measured
    pub fn summary(&self) -> String {
        let Some(s) = &self.stream else {
            return format!(
                "Receiver: {}, no stream yet",
                if self.connected { "source connected" } else { "no source connected" }
            );
        };
        let mut out = format!(
            "Receiver: {} {} packets ({:.1} s), lost {} ({:.2}%), late {}, jitter {:.1} ms, max gap {:.0} ms, \
             dropouts {} ({:.0} ms), skips {} ({:.0} ms), errors {}",
            s.codec,
            s.packets,
            s.audio_ms / 1000.0,
            s.lost,
            s.loss_percent(),
            s.late,
            s.jitter_ms,
            s.max_gap_ms,
            s.underruns,
            s.underrun_ms,
            s.overflows,
            s.overflow_ms,
            s.errors()
        );
        if s.pauses > 0 {
            out.push_str(&format!(", source paused {} time(s)", s.pauses));
        }
        // BR/EDR RSSI is relative to the controller's golden receive range
        if let Some(rssi) = self.rssi {
            out.push_str(&format!(", RSSI {rssi:+} dB vs. golden range"));
        }
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiverStatus {
    Off,
    Connecting,
    Connected,
    Failed,
}

fn str_of(j: &Value, key: &str) -> String {
    j.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn u64_of(j: &Value, key: &str) -> Option<u64> {
    let v = j.get(key)?;
    v.as_u64().or_else(|| v.as_f64().filter(|f| *f >= 0.0).map(|f| f as u64))
}

fn u32_of(j: &Value, key: &str) -> u32 {
    u64_of(j, key).map(|v| v.min(u32::MAX as u64) as u32).unwrap_or(0)
}

fn i32_of(j: &Value, key: &str) -> Option<i32> {
    let v = j.get(key)?;
    v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)).map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
}

fn f64_of(j: &Value, key: &str) -> f64 {
    j.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}

fn bool_of(j: &Value, key: &str) -> bool {
    j.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn strings_of(j: &Value, key: &str) -> Vec<String> {
    j.get(key)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default()
}

/// A discovery reply ("announce") or the hello line of a connection; None if
/// it is neither (or another tool's)
pub fn parse_info(text: &str) -> Option<ReceiverInfo> {
    let j: Value = serde_json::from_str(text.trim()).ok()?;
    let kind = j.get("type")?.as_str()?;
    if !matches!(kind, "announce" | "hello") || j.get("tool")?.as_str()? != "a2dpwb_sink" {
        return None;
    }
    Some(ReceiverInfo {
        ip: String::new(),
        port: u64_of(&j, "port").and_then(|p| u16::try_from(p).ok()).filter(|&p| p != 0).unwrap_or(DEFAULT_PORT),
        host: str_of(&j, "host"),
        bt_address: str_of(&j, "bt_address"),
        bt_name: str_of(&j, "bt_name"),
        codecs: strings_of(&j, "codecs"),
        mtu: u32_of(&j, "mtu"),
        buffer_ms: u32_of(&j, "buffer_ms"),
    })
}

/// One "stats" line of bluecaster_sink.py; None if it is no stats message
pub fn parse_stats(line: &str) -> Option<ReceiverStats> {
    let j: Value = serde_json::from_str(line.trim()).ok()?;
    if j.get("type")?.as_str()? != "stats" {
        return None;
    }
    let stream = j.get("stream").filter(|t| t.is_object()).map(|t| {
        let iv = t.get("interval").filter(|v| v.is_object());
        ReceivedStream {
            id: u32_of(t, "id"),
            codec: str_of(t, "codec"),
            config: str_of(t, "config"),
            decoding: bool_of(t, "decoding"),
            packets: u64_of(t, "packets").unwrap_or(0),
            bytes: u64_of(t, "bytes").unwrap_or(0),
            frames: u64_of(t, "frames").unwrap_or(0),
            lost: u64_of(t, "lost").unwrap_or(0),
            late: u64_of(t, "late").unwrap_or(0),
            ts_errors: u64_of(t, "ts_errors").unwrap_or(0),
            frame_errors: u64_of(t, "frame_errors").unwrap_or(0),
            decode_errors: u64_of(t, "decode_errors").unwrap_or(0),
            underruns: u64_of(t, "underruns").unwrap_or(0),
            overflows: u64_of(t, "overflows").unwrap_or(0),
            pauses: u64_of(t, "pauses").unwrap_or(0),
            idle: bool_of(t, "idle"),
            audio_ms: f64_of(t, "audio_ms"),
            underrun_ms: f64_of(t, "underrun_ms"),
            overflow_ms: f64_of(t, "overflow_ms"),
            max_gap_ms: f64_of(t, "max_gap_ms"),
            jitter_ms: f64_of(t, "jitter_ms"),
            buffer_ms: t.get("buffer_ms").and_then(Value::as_f64),
            interval_packets: iv.and_then(|v| u64_of(v, "packets")).unwrap_or(0),
            interval_bytes: iv.and_then(|v| u64_of(v, "bytes")).unwrap_or(0),
            interval_max_gap_ms: iv.map(|v| f64_of(v, "max_gap_ms")).unwrap_or(0.0),
        }
    });
    Some(ReceiverStats {
        received: Some(Instant::now()),
        connected: bool_of(&j, "connected"),
        streaming: bool_of(&j, "streaming"),
        device: str_of(&j, "device"),
        buffer_target_ms: u32_of(&j, "buffer_target_ms"),
        mtu: u32_of(&j, "mtu"),
        codecs: strings_of(&j, "codecs"),
        rssi: i32_of(&j, "rssi"),
        tx_power: i32_of(&j, "tx_power"),
        rssi_note: str_of(&j, "rssi_note"),
        afh_channels: u64_of(&j, "afh_channels").map(|v| v.min(u32::MAX as u64) as u32),
        stream,
    })
}

/// "host", "host:port", "[v6]:port", a bare IPv6 address, "auto[:port]"
/// (host "auto"; "" is "auto"). None on a bad port or an empty host.
pub fn split_target(target: &str) -> Option<(String, u16)> {
    let t = if target.trim().is_empty() { AUTO } else { target.trim() };
    let (host, port) = if let Some(rest) = t.strip_prefix('[') {
        let (host, after) = rest.split_once(']')?;
        match after {
            "" => (host, None),
            p => (host, Some(p.strip_prefix(':')?)),
        }
    } else {
        match t.split_once(':') {
            // More than one colon: a bare IPv6 address
            Some((_, rest)) if rest.contains(':') => (t, None),
            Some((h, p)) => (h, Some(p)),
            None => (t, None),
        }
    };
    let port = match port {
        None => DEFAULT_PORT,
        Some(p) => p.parse::<u16>().ok().filter(|&p| p != 0)?,
    };
    (!host.is_empty()).then(|| (host.to_string(), port))
}

/// Broadcast address of every IPv4 network this PC is on, plus the limited
/// broadcast (which Windows sends on one interface only)
fn broadcast_addresses() -> Vec<Ipv4Addr> {
    let mut out = vec![Ipv4Addr::BROADCAST];
    let Ok(ifaces) = if_addrs::get_if_addrs() else { return out };
    for iface in ifaces {
        let if_addrs::IfAddr::V4(v4) = &iface.addr else { continue };
        if iface.is_loopback() || !iface.is_oper_up() || v4.prefixlen == 0 || v4.prefixlen >= 31 {
            continue;
        }
        let b = v4.broadcast.unwrap_or_else(|| Ipv4Addr::from(u32::from(v4.ip) | !u32::from(v4.netmask)));
        if !out.contains(&b) {
            out.push(b);
        }
    }
    out
}

/// Asks for receivers: a UDP broadcast on every IPv4 network, or only `host`
/// if given. Blocks for up to `timeout` (less when `host` answers).
pub fn discover(host: Option<&str>, port: u16, timeout: Duration) -> Vec<ReceiverInfo> {
    let mut found: Vec<ReceiverInfo> = Vec::new();
    let socket = match UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)) {
        Ok(s) => s,
        Err(e) => {
            tracing::debug!("receiver discovery: cannot bind a UDP socket: {e}");
            return found;
        }
    };
    let _ = socket.set_broadcast(true);
    let targets: Vec<SocketAddr> = match host {
        None => broadcast_addresses().into_iter().map(|ip| SocketAddr::from((ip, port))).collect(),
        Some(h) => {
            (h, port).to_socket_addrs().map(|a| a.filter(SocketAddr::is_ipv4).take(1).collect()).unwrap_or_default()
        }
    };
    tracing::debug!("receiver discovery: asking {targets:?} (timeout {} ms)", timeout.as_millis());
    for t in &targets {
        if let Err(e) = socket.send_to(DISCOVER_REQUEST, t) {
            tracing::debug!("receiver discovery: send to {t} failed: {e}");
        }
    }
    let deadline = Instant::now() + timeout;
    let mut buf = [0u8; 4096];
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() || socket.set_read_timeout(Some(left)).is_err() {
            break;
        }
        let (n, from) = match socket.recv_from(&mut buf) {
            Ok(r) => r,
            // Windows reports an ICMP port unreachable of an earlier send as
            // a reset on the next receive: keep listening
            Err(e) if e.kind() == ErrorKind::ConnectionReset => continue,
            Err(_) => break,
        };
        let Some(mut info) = std::str::from_utf8(&buf[..n]).ok().and_then(parse_info) else {
            tracing::trace!("receiver discovery: {n} byte(s) from {from} not a receiver's reply");
            continue;
        };
        info.ip = from.ip().to_string();
        if !found.iter().any(|f| f.ip == info.ip && f.port == info.port) {
            tracing::debug!("receiver discovery: {}", info.describe(false));
            found.push(info);
        }
        if host.is_some() {
            break; // the one asked for answered
        }
    }
    tracing::debug!("receiver discovery: {} receiver(s) found", found.len());
    found
}

fn connect(host: &str, port: u16) -> Result<TcpStream, String> {
    let addrs: Vec<SocketAddr> =
        (host, port).to_socket_addrs().map_err(|_| format!("cannot resolve {host}"))?.collect();
    if addrs.is_empty() {
        return Err(format!("cannot resolve {host}"));
    }
    let mut error = "cannot connect".to_string();
    for addr in addrs {
        tracing::debug!("receiver: connecting to {addr}");
        match TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT) {
            Ok(s) => {
                tracing::debug!("receiver: connected to {addr}");
                let _ = s.set_read_timeout(Some(READ_POLL));
                let _ = s.set_nodelay(true);
                return Ok(s);
            }
            Err(e) => {
                tracing::debug!("receiver: {addr}: {e}");
                error = match e.kind() {
                    ErrorKind::ConnectionRefused => "connection refused".into(),
                    ErrorKind::TimedOut => "connection timed out".into(),
                    _ => format!("cannot connect ({e})"),
                }
            }
        }
    }
    Err(error)
}

#[derive(Default)]
struct State {
    stop: bool,
    /// A clone of the live connection: stop() shuts it down (the peer sees
    /// the end), configure() writes to it
    socket: Option<TcpStream>,
    target: String,
    status: Option<(ReceiverStatus, String)>,
    stats: Option<ReceiverStats>,
    info: Option<ReceiverInfo>,
}

#[derive(Default)]
struct Shared {
    state: Mutex<State>,
    wake: Condvar,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn set_status(&self, status: ReceiverStatus, detail: &str) {
        let mut s = self.lock();
        s.status = Some((status, detail.to_string()));
        if status != ReceiverStatus::Connected {
            s.stats = None;
        }
    }
}

/// Connection to one receiver, kept until [`stop`](ReceiverClient::stop)
/// (or drop). v1 had one process-wide instance; the owner here decides
/// (the service keeps one so the connection outlives a UI window).
#[derive(Default)]
pub struct ReceiverClient {
    shared: Arc<Shared>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl ReceiverClient {
    pub fn new() -> ReceiverClient {
        ReceiverClient::default()
    }

    /// Connects to "host[:port]" (default port [`DEFAULT_PORT`]), or with
    /// "auto" / "" to the first receiver found on the local network, and
    /// keeps reconnecting until stop(). Replaces a previous target.
    pub fn start(&self, target: &str) {
        self.stop();
        let mut s = self.shared.lock();
        s.target = if target.trim().is_empty() { AUTO.into() } else { target.trim().into() };
        s.stats = None;
        s.info = None;
        let Some((host, port)) = split_target(target) else {
            tracing::debug!("receiver: invalid target {target:?}");
            s.status = Some((ReceiverStatus::Failed, "invalid address (host or host:port)".into()));
            return;
        };
        tracing::debug!("receiver: start, host {host} port {port}");
        s.stop = false;
        s.status = Some((ReceiverStatus::Connecting, String::new()));
        drop(s);
        let shared = self.shared.clone();
        let t = std::thread::Builder::new().name("receiver".into()).spawn(move || run(&shared, &host, port));
        match t {
            Ok(t) => *self.thread.lock().unwrap_or_else(|e| e.into_inner()) = Some(t),
            Err(e) => self.shared.set_status(ReceiverStatus::Failed, &format!("cannot start a thread: {e}")),
        }
    }

    /// Disconnects; waits for the connection thread (bounded by the connect
    /// and discovery timeouts)
    pub fn stop(&self) {
        let thread = self.thread.lock().unwrap_or_else(|e| e.into_inner()).take();
        {
            let mut s = self.shared.lock();
            s.stop = true;
            if let Some(sock) = &s.socket {
                let _ = sock.shutdown(Shutdown::Both); // tells the receiver; the read sees the stop within READ_POLL
            }
        }
        self.shared.wake.notify_all();
        if let Some(t) = thread {
            let _ = t.join();
        }
        let mut s = self.shared.lock();
        s.status = None;
        s.stats = None;
        s.info = None;
    }

    /// Status and its detail (why it failed)
    pub fn status(&self) -> (ReceiverStatus, String) {
        self.shared.lock().status.clone().unwrap_or((ReceiverStatus::Off, String::new()))
    }

    pub fn target(&self) -> String {
        self.shared.lock().target.clone()
    }

    /// The latest statistics; None while nothing was received from the
    /// current connection
    pub fn latest(&self) -> Option<ReceiverStats> {
        self.shared.lock().stats.clone()
    }

    /// The receiver of the current connection, once it introduced itself
    pub fn receiver(&self) -> Option<ReceiverInfo> {
        self.shared.lock().info.clone()
    }

    /// Receiver settings for the next stream (0 = leave unchanged); false
    /// if not connected. The receiver confirms them in its statistics.
    pub fn configure(&self, mtu: u32, buffer_ms: u32) -> bool {
        let mut msg = serde_json::json!({ "type": "configure", "version": 1 });
        if mtu != 0 {
            msg["mtu"] = mtu.into();
        }
        if buffer_ms != 0 {
            msg["buffer_ms"] = buffer_ms.into();
        }
        let line = format!("{msg}\n");
        let s = self.shared.lock();
        let sent = match s.socket.as_ref() {
            // &TcpStream writes: the reader thread keeps its own handle
            Some(mut sock) => sock.write_all(line.as_bytes()).is_ok(),
            None => false,
        };
        tracing::debug!("receiver: configure {msg}: sent {sent}");
        sent
    }

    /// The CLI's line: what the receiver measured, or why there is nothing
    pub fn summary(&self) -> String {
        if let Some(stats) = self.latest() {
            return stats.summary();
        }
        let (status, detail) = self.status();
        format!(
            "Receiver {}: {}{}{}",
            self.target(),
            match status {
                ReceiverStatus::Connecting => "connecting",
                ReceiverStatus::Connected => "connected, waiting for data",
                _ => "not connected",
            },
            if detail.is_empty() { "" } else { " - " },
            detail
        )
    }
}

impl Drop for ReceiverClient {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The connection thread
fn run(shared: &Shared, host: &str, port: u16) {
    let automatic = host.eq_ignore_ascii_case(AUTO);
    loop {
        if shared.lock().stop {
            return;
        }
        shared.set_status(ReceiverStatus::Connecting, "");
        let mut target = Some((host.to_string(), port));
        if automatic {
            target = discover(None, port, DISCOVER_TIMEOUT).into_iter().next().map(|info| {
                let t = (info.ip.clone(), info.port);
                shared.lock().info = Some(info);
                t
            });
        }
        let error = match target {
            None => "no receiver found on the local network".to_string(),
            Some((h, p)) => match connect(&h, p) {
                Ok(stream) => match read_lines(shared, stream, &h) {
                    Some(e) => e,
                    None => return, // stopped
                },
                Err(e) => e,
            },
        };
        let mut s = shared.lock();
        if s.stop {
            return;
        }
        tracing::debug!("receiver: {error}, trying again in {} ms", RECONNECT_DELAY.as_millis());
        s.status = Some((ReceiverStatus::Failed, error));
        s.stats = None;
        let (s, _) = shared.wake.wait_timeout_while(s, RECONNECT_DELAY, |s| !s.stop).unwrap_or_else(|e| e.into_inner());
        if s.stop {
            return;
        }
    }
}

/// Reads the receiver's lines until the connection ends: Some(why), or None
/// when stopped
fn read_lines(shared: &Shared, mut stream: TcpStream, host: &str) -> Option<String> {
    {
        let mut s = shared.lock();
        if s.stop {
            return None;
        }
        s.socket = stream.try_clone().ok();
        s.status = Some((ReceiverStatus::Connected, String::new()));
    }
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 4096];
    let mut last_data = Instant::now();
    let error = loop {
        let n = match stream.read(&mut chunk) {
            Ok(0) => break "closed by the receiver".to_string(),
            Ok(n) => n,
            Err(e) if matches!(e.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) => {
                if shared.lock().stop {
                    break String::new();
                }
                if last_data.elapsed() >= RECEIVE_TIMEOUT {
                    break "no data from the receiver".to_string();
                }
                continue;
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => {
                tracing::debug!("receiver: read failed: {e}");
                break "connection lost".to_string();
            }
        };
        last_data = Instant::now();
        buf.extend_from_slice(&chunk[..n]);
        while let Some(nl) = buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = buf.drain(..=nl).collect();
            let Ok(line) = std::str::from_utf8(&line) else { continue };
            if let Some(stats) = parse_stats(line) {
                tracing::trace!("receiver: {}", stats.summary());
                shared.lock().stats = Some(stats);
            } else if let Some(mut info) = parse_info(line) {
                info.ip = host.to_string();
                tracing::debug!("receiver: hello, {}", info.describe(false));
                shared.lock().info = Some(info);
            } else {
                tracing::trace!("receiver: line not understood: {}", line.trim());
            }
        }
        if buf.len() > MAX_LINE {
            break "protocol error".to_string();
        }
    };
    let mut s = shared.lock();
    s.socket = None;
    if s.stop { None } else { Some(error) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;

    const ANNOUNCE: &str = r#"{"type":"announce","version":1,"tool":"a2dpwb_sink","host":"pi5","port":51201,
        "bt_address":"DC:A6:32:00:11:22","bt_name":"bluecaster-sink","codecs":["SBC","AAC","LDAC"],"mtu":1005,
        "buffer_ms":200}"#;

    const STATS: &str = r#"{"type":"stats","version":1,"time":1760700000.123,"adapter":"hci0","address":"DC:A6:32:00:11:22",
        "connected":true,"device":"00:1A:7D:DA:71:13","streaming":true,"buffer_target_ms":200,"mtu":1005,
        "codecs":["SBC","LDAC"],"rssi":-3,"tx_power":null,"rssi_note":null,"afh_channels":59,
        "stream":{"id":2,"codec":"LDAC","config":"LDAC 96000 Hz stereo","sample_rate":96000,"channels":2,"rtp":true,
        "decoding":false,"mtu":1005,"starts":1,"packets":9990,"bytes":9000000,"frames":39960,"audio_ms":60123.4,
        "lost":10,"late":2,"ts_errors":1,"frame_errors":0,"decode_errors":0,"underruns":3,"underrun_ms":45.5,
        "overflows":1,"overflow_ms":10.0,"pauses":1,"pause_ms":500.0,"idle":false,"max_gap_ms":48.2,"jitter_ms":1.25,
        "buffer_ms":180.5,"interval":{"packets":166,"bytes":150000,"max_gap_ms":12.0},"issues":{}}}"#;

    fn one_line(s: &str) -> String {
        s.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn parses_announce_and_hello() {
        let i = parse_info(ANNOUNCE).unwrap();
        assert_eq!(i.host, "pi5");
        assert_eq!(i.bt_address, "DC:A6:32:00:11:22");
        assert_eq!(i.codecs, ["SBC", "AAC", "LDAC"]);
        assert_eq!((i.mtu, i.port, i.buffer_ms), (1005, 51201, 200));
        let hello = ANNOUNCE.replace("\"announce\"", "\"hello\"");
        assert!(parse_info(&hello).is_some());
        assert!(parse_info(&ANNOUNCE.replace("bluecaster_sink", "other")).is_none());
        assert!(parse_info(STATS).is_none());
        assert!(parse_info("not json").is_none());
        assert!(parse_info(r#"{"type":"announce"}"#).is_none());
    }

    #[test]
    fn parses_stats_and_summarises() {
        let s = parse_stats(STATS).unwrap();
        assert!(s.connected && s.streaming);
        assert_eq!(s.rssi, Some(-3));
        assert_eq!(s.tx_power, None);
        assert_eq!(s.rssi_note, "");
        assert_eq!(s.afh_channels, Some(59));
        let t = s.stream.as_ref().unwrap();
        assert_eq!((t.id, t.packets, t.lost, t.errors()), (2, 9990, 10, 1));
        assert_eq!(t.buffer_ms, Some(180.5));
        assert_eq!((t.interval_packets, t.interval_bytes), (166, 150000));
        assert!((t.loss_percent() - 0.1).abs() < 1e-9);
        assert_eq!(
            s.summary(),
            "Receiver: LDAC 9990 packets (60.1 s), lost 10 (0.10%), late 2, jitter 1.2 ms, max gap 48 ms, \
             dropouts 3 (46 ms), skips 1 (10 ms), errors 1, source paused 1 time(s), RSSI -3 dB vs. golden range"
        );
        assert!(parse_stats(ANNOUNCE).is_none());
    }

    #[test]
    fn stats_without_a_stream() {
        let s =
            parse_stats(r#"{"type":"stats","connected":false,"streaming":false,"stream":null,"rssi":null}"#).unwrap();
        assert!(s.stream.is_none());
        assert_eq!(s.rssi, None);
        assert_eq!(s.summary(), "Receiver: no source connected, no stream yet");
        let mut no_buffer = STATS.replace("\"buffer_ms\":180.5", "\"buffer_ms\":null");
        no_buffer = one_line(&no_buffer);
        assert_eq!(parse_stats(&no_buffer).unwrap().stream.unwrap().buffer_ms, None);
    }

    #[test]
    fn targets() {
        assert_eq!(split_target(""), Some(("auto".into(), DEFAULT_PORT)));
        assert_eq!(split_target("auto:5000"), Some(("auto".into(), 5000)));
        assert_eq!(split_target("pi5"), Some(("pi5".into(), DEFAULT_PORT)));
        assert_eq!(split_target("192.168.1.5:6000"), Some(("192.168.1.5".into(), 6000)));
        assert_eq!(split_target("[fe80::1]:7000"), Some(("fe80::1".into(), 7000)));
        assert_eq!(split_target("[fe80::1]"), Some(("fe80::1".into(), DEFAULT_PORT)));
        assert_eq!(split_target("fe80::1"), Some(("fe80::1".into(), DEFAULT_PORT)));
        assert_eq!(split_target("host:0"), None);
        assert_eq!(split_target("host:70000"), None);
        assert_eq!(split_target("host:12x"), None);
        assert_eq!(split_target(":5000"), None);
        assert_eq!(split_target("[fe80::1]x"), None);
    }

    #[test]
    fn describes_a_receiver() {
        let mut i = parse_info(ANNOUNCE).unwrap();
        i.ip = "192.168.1.20".into();
        assert_eq!(
            i.describe(true),
            "Receiver: pi5 (192.168.1.20:51201), Bluetooth DC:A6:32:00:11:22 \"bluecaster-sink\", L2CAP MTU 1005 \
             (first of several)"
        );
    }

    #[test]
    fn discovers_a_receiver_on_localhost() {
        let responder = UdpSocket::bind("127.0.0.1:0").unwrap();
        let port = responder.local_addr().unwrap().port();
        let t = std::thread::spawn(move || {
            let mut buf = [0u8; 512];
            let (n, from) = responder.recv_from(&mut buf).unwrap();
            let req: Value = serde_json::from_slice(&buf[..n]).unwrap();
            assert_eq!(req["type"], "discover");
            assert_eq!(req["tool"], "A2DPWB");
            // A foreign reply first, then ours twice (deduplicated)
            responder.send_to(br#"{"type":"announce","tool":"other"}"#, from).unwrap();
            let reply = one_line(ANNOUNCE).replace("51201", &port.to_string());
            responder.send_to(reply.as_bytes(), from).unwrap();
            responder.send_to(reply.as_bytes(), from).unwrap();
        });
        let found = discover(Some("127.0.0.1"), port, Duration::from_secs(3));
        t.join().unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].ip, "127.0.0.1");
        assert_eq!(found[0].port, port);
        assert_eq!(found[0].host, "pi5");
    }

    #[test]
    fn nobody_answers() {
        // A bound socket that never answers: discovery waits out the timeout
        let silent = UdpSocket::bind("127.0.0.1:0").unwrap();
        let port = silent.local_addr().unwrap().port();
        let started = Instant::now();
        assert!(discover(Some("127.0.0.1"), port, Duration::from_millis(300)).is_empty());
        assert!(started.elapsed() >= Duration::from_millis(250));
    }

    // Generous: a refused connect on Windows retries the SYN for about 2 s,
    // up to CONNECT_TIMEOUT, slower still on a loaded CI runner
    fn wait_for(what: impl Fn() -> bool) -> bool {
        let until = Instant::now() + Duration::from_secs(15);
        while Instant::now() < until {
            if what() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    #[test]
    fn client_reads_a_fake_receiver() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let (cfg_tx, cfg_rx) = std::sync::mpsc::channel::<String>();
        let server = std::thread::spawn(move || {
            let (mut conn, _) = listener.accept().unwrap();
            let hello = one_line(ANNOUNCE).replace("\"announce\"", "\"hello\"");
            // The hello and a stats line split across writes
            let stats = one_line(STATS);
            conn.write_all(format!("{hello}\n{}", &stats[..40]).as_bytes()).unwrap();
            std::thread::sleep(Duration::from_millis(50));
            conn.write_all(format!("{}\n", &stats[40..]).as_bytes()).unwrap();
            let mut reader = BufReader::new(conn.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            cfg_tx.send(line).unwrap();
            // Then the receiver goes away
            drop(conn);
        });

        let client = ReceiverClient::new();
        assert_eq!(client.status().0, ReceiverStatus::Off);
        assert!(client.summary().contains("not connected"));
        client.start(&format!("127.0.0.1:{port}"));
        assert_eq!(client.target(), format!("127.0.0.1:{port}"));
        assert!(wait_for(|| client.latest().is_some()), "no stats");
        let info = client.receiver().unwrap();
        assert_eq!((info.ip.as_str(), info.bt_name.as_str()), ("127.0.0.1", "bluecaster-sink"));
        assert_eq!(client.status().0, ReceiverStatus::Connected);
        assert!(client.summary().starts_with("Receiver: LDAC 9990 packets"));

        assert!(client.configure(1005, 0));
        let cfg: Value = serde_json::from_str(&cfg_rx.recv_timeout(Duration::from_secs(5)).unwrap()).unwrap();
        assert_eq!(cfg, serde_json::json!({"type": "configure", "version": 1, "mtu": 1005}));
        server.join().unwrap();

        assert!(wait_for(|| client.status().0 == ReceiverStatus::Failed), "no failure after close");
        assert_eq!(client.status().1, "closed by the receiver");
        assert!(client.latest().is_none());
        assert!(!client.configure(1005, 200));
        assert!(client.summary().contains("not connected - closed by the receiver"));

        // Stop is prompt also while waiting to reconnect
        let started = Instant::now();
        client.stop();
        assert!(started.elapsed() < Duration::from_secs(2));
        assert_eq!(client.status().0, ReceiverStatus::Off);
    }

    #[test]
    fn client_reports_refused_and_bad_targets() {
        // A port nobody listens on
        let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let client = ReceiverClient::new();
        client.start(&format!("127.0.0.1:{port}"));
        assert!(wait_for(|| client.status().0 == ReceiverStatus::Failed));
        assert!(client.status().1.contains("refused") || client.status().1.contains("timed out"));
        client.start("host:99999");
        assert_eq!(client.status(), (ReceiverStatus::Failed, "invalid address (host or host:port)".into()));
        client.stop();
    }

    #[test]
    fn stop_wakes_a_blocked_read() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (conn, _) = listener.accept().unwrap();
            std::thread::sleep(Duration::from_millis(1500));
            drop(conn);
        });
        let client = ReceiverClient::new();
        client.start(&format!("127.0.0.1:{port}"));
        assert!(wait_for(|| client.status().0 == ReceiverStatus::Connected));
        let started = Instant::now();
        client.stop();
        assert!(started.elapsed() < Duration::from_secs(1), "stop waited for the read timeout");
        server.join().unwrap();
    }
}
