//! LE Audio (experimental), MCP: the Windows side of the media control
//! server (v1 le_audio_mcp_media.cpp). The MCP module of bt-host (GMCS on
//! the PC) asks it what plays and hands it the headset's Media Control
//! Point commands; [`windows_backend`] goes into `LeConfig::media` unless
//! the MCP dry run is on (`--mcp-dry-run`, implied by `--hci-tcp`: then
//! le-audio's simulated player answers).
//!
//! One thread of its own (never the BTstack thread):
//! - reads the current Windows media session (System Media Transport
//!   Controls, Windows.Media.Control: the player of the volume flyout)
//!   about once a second: player, state, title, duration, position;
//! - carries out the commands through that session (Play / Pause / Stop /
//!   Next / Previous / seek), or as media keys (as AVRCP buttons are) when
//!   Windows has no media session.

use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use le_audio::mcp_model::{
    self, ChangedFn, DoneFn, MediaBackend, MediaInfo, OP_MOVE_RELATIVE, OP_NEXT_TRACK, OP_PAUSE, OP_PLAY,
    OP_PREVIOUS_TRACK, OP_STOP, RESULT_COMMAND_CANNOT_BE_COMPLETED, RESULT_MEDIA_PLAYER_INACTIVE,
    RESULT_OPCODE_NOT_SUPPORTED, RESULT_SUCCESS, STATE_INACTIVE, STATE_PAUSED, STATE_PLAYING,
};
use win_sys::media_keys::MediaKey;
use windows::ApplicationModel::AppInfo;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session, GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
};
use windows::core::HSTRING;

/// Session state read every
const POLL: Duration = Duration::from_secs(1);
/// ... and this long after a command (the player reacts first)
const POLL_AFTER_COMMAND: Duration = Duration::from_millis(300);
/// SMTC not available: tried again after
const RETRY_MANAGER: Duration = Duration::from_secs(30);
/// An async WinRT call answers within
const WINRT_TIMEOUT: Duration = Duration::from_secs(2);
/// 100 ns units per 0.01 s
const TICKS_PER_CENTISECOND: i64 = 100_000;

/// The backend for a real adapter; None: the simulated player (dry run)
pub(crate) fn windows_backend() -> Option<Box<dyn MediaBackend>> {
    Some(Box::new(WindowsMedia::default()))
}

/// The clock of [`MediaInfo::updated_ms`], shared with the MCP module
fn now_ms() -> u64 {
    mcp_model::now_ms()
}

/// "SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify" -> "SpotifyMusic",
/// "C:\...\foobar2000.exe" / "Spotify.exe" -> "foobar2000" / "Spotify"
fn name_from_aumid(aumid: &str) -> String {
    let mut s = aumid;
    if let Some((package, _)) = s.split_once('!') {
        s = package.split('_').next().unwrap_or(package);
        if let Some((_, last)) = s.rsplit_once('.')
            && !last.is_empty()
        {
            s = last;
        }
    }
    if let Some((_, file)) = s.rsplit_once(['\\', '/']) {
        s = file;
    }
    if s.len() > 4 && s[s.len() - 4..].eq_ignore_ascii_case(".exe") {
        s = &s[..s.len() - 4];
    }
    if s.is_empty() { "Windows".into() } else { s.into() }
}

fn fold_hash(s: &str) -> u32 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    let h = h.finish();
    (h ^ (h >> 32)) as u32
}

/// The result of an async WinRT call, waited for with a timeout (the
/// backend thread is in the MTA, blocking is allowed there)
macro_rules! wait_result {
    ($op:expr) => {{
        match $op {
            Ok(op) => {
                let until = Instant::now() + WINRT_TIMEOUT;
                loop {
                    match op.Status() {
                        // Started
                        Ok(s) if s.0 == 0 && Instant::now() < until => std::thread::sleep(Duration::from_millis(10)),
                        // Completed
                        Ok(s) if s.0 == 1 => break op.GetResults().ok(),
                        _ => break None,
                    }
                }
            }
            Err(_) => None,
        }
    }};
}

enum Task {
    Command(u8, i32, DoneFn),
    Seek(i32),
}

#[derive(Default)]
struct State {
    stopping: bool,
    tasks: VecDeque<Task>,
    info: MediaInfo,
}

#[derive(Default)]
struct Shared {
    state: Mutex<State>,
    cv: Condvar,
}

impl Shared {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// The Windows media session backend (v1 mcp::MediaBackend, not dry run)
#[derive(Default)]
pub(crate) struct WindowsMedia {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl MediaBackend for WindowsMedia {
    fn start(&mut self, changed: ChangedFn) {
        tracing::debug!("MCP media: starting the Windows backend");
        self.stop();
        self.shared = Arc::new(Shared::default());
        let shared = self.shared.clone();
        self.thread = std::thread::Builder::new()
            .name("mcp-media".into())
            .spawn(move || Worker::new(shared, changed).run())
            .map_err(|e| tracing::warn!("MCP: cannot start the media thread: {e}"))
            .ok();
    }

    fn stop(&mut self) {
        let Some(t) = self.thread.take() else { return };
        {
            let mut s = self.shared.lock();
            tracing::debug!("MCP media: stopping, {} task(s) dropped", s.tasks.len());
            s.stopping = true;
            s.tasks.clear();
        }
        self.shared.cv.notify_all();
        let _ = t.join();
    }

    fn dry_run(&self) -> bool {
        false
    }

    fn info(&self) -> MediaInfo {
        self.shared.lock().info.clone()
    }

    fn command(&mut self, opcode: u8, param: i32, done: DoneFn) {
        tracing::debug!(
            "MCP media: command {} (param {param}), backend running {}",
            mcp_model::opcode_name(opcode),
            self.thread.is_some()
        );
        if self.thread.is_none() {
            return done(RESULT_MEDIA_PLAYER_INACTIVE);
        }
        self.post(Task::Command(opcode, param, done));
    }

    fn seek(&mut self, position: i32) {
        tracing::debug!("MCP media: seek to {position} cs, backend running {}", self.thread.is_some());
        if self.thread.is_some() {
            self.post(Task::Seek(position));
        }
    }
}

impl WindowsMedia {
    fn post(&self, task: Task) {
        {
            let mut s = self.shared.lock();
            if s.stopping {
                return;
            }
            s.tasks.push_back(task);
        }
        self.shared.cv.notify_all();
    }
}

impl Drop for WindowsMedia {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The backend thread's own state
struct Worker {
    shared: Arc<Shared>,
    changed: ChangedFn,
    manager: Option<Manager>,
    manager_tried: Option<Instant>,
    manager_logged: bool,
    /// AUMID -> display name
    names: HashMap<String, String>,
}

impl Worker {
    fn new(shared: Arc<Shared>, changed: ChangedFn) -> Worker {
        Worker { shared, changed, manager: None, manager_tried: None, manager_logged: false, names: HashMap::new() }
    }

    fn current(&self) -> MediaInfo {
        self.shared.lock().info.clone()
    }

    /// New state; `changed` unless only the position moved on as expected
    fn publish(&self, next: MediaInfo) {
        let prev = std::mem::replace(&mut self.shared.lock().info, next.clone());
        if !prev.differs_from(&next, now_ms()) {
            return;
        }
        tracing::debug!(
            "MCP media: changed: state {}, track {:08x} -> {:08x}, duration {} cs, position {} cs, rate {}, seek {}",
            mcp_model::state_name(next.state),
            prev.track,
            next.track,
            next.duration,
            next.position,
            next.rate,
            next.can_seek
        );
        if prev.state != next.state || prev.player != next.player {
            tracing::info!(
                "MCP: media player \"{}\": {}",
                if next.session { next.player.as_str() } else { "(none)" },
                mcp_model::state_name(next.state)
            );
        }
        (self.changed)();
    }

    // ---- Windows media session (SMTC) ----

    fn ensure_manager(&mut self) {
        if self.manager.is_some() || self.manager_tried.is_some_and(|t| t.elapsed() < RETRY_MANAGER) {
            return;
        }
        self.manager_tried = Some(Instant::now());
        pin_factory_dll::<Manager>("session manager factory");
        self.manager = wait_result!(Manager::RequestAsync());
        tracing::debug!("MCP media: session manager request: available {}", self.manager.is_some());
        if let Some(m) = &self.manager {
            pin_dll_of("session manager", m);
        }
        match &self.manager {
            Some(_) => tracing::info!("MCP: reading the Windows media session (System Media Transport Controls)"),
            None if !self.manager_logged => {
                tracing::warn!("MCP: Windows media sessions not available: media keys only, no player state");
                self.manager_logged = true;
            }
            None => {}
        }
    }

    fn session(&mut self) -> Option<Session> {
        self.ensure_manager();
        self.manager.as_ref()?.GetCurrentSession().ok()
    }

    fn display_name(&mut self, aumid: &str) -> String {
        if let Some(n) = self.names.get(aumid) {
            return n.clone();
        }
        pin_factory_dll::<AppInfo>("app info factory");
        let name = AppInfo::GetFromAppUserModelId(&HSTRING::from(aumid))
            .inspect(|a| pin_dll_of("app info", a))
            .and_then(|a| a.DisplayInfo())
            .and_then(|d| d.DisplayName())
            .map(|n| n.to_string_lossy())
            .ok()
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| name_from_aumid(aumid));
        tracing::debug!("MCP media: player {aumid} is \"{name}\"");
        self.names.insert(aumid.to_string(), name.clone());
        name
    }

    fn read_session(&mut self) -> MediaInfo {
        let Some(s) = self.session() else { return MediaInfo::default() };
        self.read(&s).unwrap_or_default()
    }

    /// None: the session went away while reading
    fn read(&mut self, s: &Session) -> Option<MediaInfo> {
        let mut mi = MediaInfo::default();
        let aumid = s.SourceAppUserModelId().ok()?.to_string_lossy();
        mi.session = true;
        mi.player = self.display_name(&aumid);
        let pb = s.GetPlaybackInfo().ok()?;
        mi.state = match pb.PlaybackStatus().ok()? {
            PlaybackStatus::Playing => STATE_PLAYING,
            PlaybackStatus::Closed => STATE_INACTIVE,
            // paused, stopped, opened, changing
            _ => STATE_PAUSED,
        };
        if let Ok(rate) = pb.PlaybackRate().and_then(|r| r.Value()) {
            mi.rate = rate;
        }
        mi.can_seek = pb.Controls().and_then(|c| c.IsPlaybackPositionEnabled()).unwrap_or(false);
        let mut artist = String::new();
        if let Some(props) = wait_result!(s.TryGetMediaPropertiesAsync()) {
            mi.title = props.Title().map(|t| t.to_string_lossy()).unwrap_or_default();
            artist = props.Artist().map(|t| t.to_string_lossy()).unwrap_or_default();
        }
        let tl = s.GetTimelineProperties().ok()?;
        let start = tl.StartTime().ok()?.Duration;
        let end = tl.EndTime().ok()?.Duration;
        let pos = tl.Position().ok()?.Duration;
        if end > start {
            mi.duration = ((end - start) / TICKS_PER_CENTISECOND).min(i64::from(i32::MAX - 1)) as i32;
        }
        if end > start || pos > 0 {
            let mut p = pos - start;
            if mi.state == STATE_PLAYING
                && let Ok(updated) = tl.LastUpdatedTime()
            {
                let elapsed = filetime_now() - updated.UniversalTime;
                if elapsed > 0 && elapsed < 36_000_000_000 {
                    p += (elapsed as f64 * mi.rate) as i64;
                }
            }
            mi.position = (p / TICKS_PER_CENTISECOND).clamp(0, i64::from(i32::MAX - 1)) as i32;
            if mi.duration > 0 {
                mi.position = mi.position.min(mi.duration);
            }
            mi.updated_ms = now_ms();
        }
        mi.track = fold_hash(&format!("{aumid}\n{}\n{artist}\n{}", mi.title, mi.duration));
        Some(mi)
    }

    fn seek_session(s: &Session, position: i32) -> bool {
        let Some(start) = s.GetTimelineProperties().and_then(|t| t.StartTime()).ok().map(|t| t.Duration) else {
            return false;
        };
        wait_result!(s.TryChangePlaybackPositionAsync(start + i64::from(position) * TICKS_PER_CENTISECOND))
            .unwrap_or(false)
    }

    fn execute(&mut self, op: u8, param: i32) -> u8 {
        let cur = self.current();
        let Some(s) = self.session() else {
            // No media session: the media keys AVRCP buttons use
            let key = match op {
                OP_PLAY | OP_PAUSE => MediaKey::PlayPause,
                OP_STOP => MediaKey::Stop,
                OP_NEXT_TRACK => MediaKey::Next,
                OP_PREVIOUS_TRACK => MediaKey::Previous,
                _ => {
                    tracing::info!("MCP: {}: no media session in Windows", mcp_model::opcode_name(op));
                    return RESULT_MEDIA_PLAYER_INACTIVE;
                }
            };
            tracing::info!("MCP: {} -> media key {key:?} (no media session in Windows)", mcp_model::opcode_name(op));
            if let Err(e) = win_sys::media_keys::send(key) {
                tracing::warn!("MCP: media key {key:?}: {e}");
            }
            return RESULT_SUCCESS;
        };
        let ok: Option<bool> = match op {
            OP_PLAY if cur.state == STATE_PLAYING => Some(true),
            OP_PLAY => wait_result!(s.TryPlayAsync()),
            OP_PAUSE if cur.state == STATE_PAUSED => Some(true),
            OP_PAUSE => wait_result!(s.TryPauseAsync()),
            OP_STOP => {
                // MCS: Stop = paused at the start of the track
                match wait_result!(s.TryStopAsync()) {
                    Some(true) => Some(true),
                    _ => {
                        let paused = wait_result!(s.TryPauseAsync());
                        if paused == Some(true) && cur.can_seek {
                            Worker::seek_session(&s, 0);
                        }
                        paused
                    }
                }
            }
            OP_NEXT_TRACK => wait_result!(s.TrySkipNextAsync()),
            OP_PREVIOUS_TRACK => wait_result!(s.TrySkipPreviousAsync()),
            OP_MOVE_RELATIVE => {
                let now = cur.position_now(now_ms());
                if !cur.can_seek || now < 0 {
                    tracing::debug!(
                        "MCP media: move relative {param} not possible (seek {}, position {now})",
                        cur.can_seek
                    );
                    return RESULT_COMMAND_CANNOT_BE_COMPLETED;
                }
                let mut target = (i64::from(now) + i64::from(param)).max(0);
                if cur.duration > 0 {
                    target = target.min(i64::from(cur.duration));
                }
                Some(Worker::seek_session(&s, target as i32))
            }
            _ => {
                tracing::debug!("MCP media: opcode 0x{op:02x} not supported");
                return RESULT_OPCODE_NOT_SUPPORTED;
            }
        };
        tracing::info!(
            "MCP: {} -> \"{}\": {}",
            mcp_model::opcode_name(op),
            cur.player,
            match ok {
                None => "no answer",
                Some(true) => "done",
                Some(false) => "refused by the player",
            }
        );
        if ok == Some(true) { RESULT_SUCCESS } else { RESULT_COMMAND_CANNOT_BE_COMPLETED }
    }

    fn seek(&mut self, mut position: i32) {
        let cur = self.current();
        let Some(s) = self.session() else {
            tracing::debug!("MCP media: seek ignored, no media session");
            return;
        };
        if !cur.can_seek {
            tracing::debug!("MCP media: seek ignored, \"{}\" cannot seek", cur.player);
            return;
        }
        if cur.duration > 0 {
            position = position.min(cur.duration);
        }
        let done = Worker::seek_session(&s, position);
        tracing::info!("MCP: seek -> \"{}\": {}", cur.player, if done { "done" } else { "refused" });
    }

    fn run(mut self) {
        // WinRT on an MTA thread (blocking waits allowed)
        let _com = win_sys::audio_policy::ComApartment::init_mta().map_err(|e| tracing::warn!("MCP: COM: {e}")).ok();
        let mut wait = Duration::ZERO;
        loop {
            let tasks: Vec<Task> = {
                let guard = self.shared.lock();
                let (mut s, _) = self
                    .shared
                    .cv
                    .wait_timeout_while(guard, wait, |s| !s.stopping && s.tasks.is_empty())
                    .unwrap_or_else(|e| e.into_inner());
                if s.stopping {
                    break;
                }
                s.tasks.drain(..).collect()
            };
            if !tasks.is_empty() {
                for t in tasks {
                    match t {
                        Task::Command(op, param, done) => {
                            let r = self.execute(op, param);
                            done(r);
                        }
                        Task::Seek(p) => self.seek(p),
                    }
                }
                // Let the player react, then read it
                wait = POLL_AFTER_COMMAND;
                continue;
            }
            let info = self.read_session();
            self.publish(info);
            wait = POLL;
        }
        self.manager = None;
    }
}

/// True the first time it is asked for `what`
fn first_pin(what: &'static str) -> bool {
    static PINNED: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
    let mut pinned = PINNED.lock().unwrap_or_else(|e| e.into_inner());
    if pinned.contains(&what) {
        return false;
    }
    pinned.push(what);
    true
}

/// Keeps the DLL of a WinRT class's activation factory loaded for good,
/// before the first static call caches the factory: windows-rs caches it
/// for the process, and a media thread that started after COM unloaded the
/// DLL crashed in it (win_sys::com). Pinning the object a call returns is
/// not enough: a call that fails caches the factory too, and returns
/// nothing to pin (GetFromAppUserModelId for Chrome, no packaged app,
/// left Windows.ApplicationModel.dll to be unloaded: crash on 2026-10-22).
fn pin_factory_dll<C: windows::core::RuntimeName>(what: &'static str) {
    if !first_pin(what) {
        return;
    }
    let pinned =
        windows::core::factory::<C, windows::core::IInspectable>().ok().and_then(|f| win_sys::com::pin_module_of(&f));
    match pinned {
        Some(dll) => tracing::debug!("MCP media: {what}: {dll} kept loaded"),
        None => tracing::warn!("MCP media: {what}: cannot keep its DLL loaded"),
    }
}

/// Keeps the DLL of a WinRT object loaded for good, once per class (see
/// [`pin_factory_dll`])
fn pin_dll_of<I: windows::core::Interface>(what: &'static str, object: &I) {
    if !first_pin(what) {
        return;
    }
    match win_sys::com::pin_module_of(object) {
        Some(dll) => tracing::debug!("MCP media: {what}: {dll} kept loaded"),
        None => tracing::warn!("MCP media: {what}: cannot keep its DLL loaded"),
    }
}

/// Now as a WinRT DateTime (100 ns since 1601-01-01 UTC)
fn filetime_now() -> i64 {
    const UNIX_TO_1601: i64 = 11_644_473_600 * 10_000_000;
    let since_unix = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    UNIX_TO_1601 + (since_unix.as_nanos() / 100) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_names_from_aumids() {
        assert_eq!(name_from_aumid("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"), "SpotifyMusic");
        assert_eq!(name_from_aumid(r"C:\Program Files\foobar2000\foobar2000.exe"), "foobar2000");
        assert_eq!(name_from_aumid("Spotify.exe"), "Spotify");
        assert_eq!(name_from_aumid("chrome"), "chrome");
        assert_eq!(name_from_aumid(""), "Windows");
    }

    #[test]
    fn stopped_backend_answers_inactive() {
        let mut b = WindowsMedia::default();
        assert!(!b.dry_run());
        let (tx, rx) = std::sync::mpsc::channel();
        b.command(OP_PLAY, 0, Box::new(move |r| tx.send(r).unwrap()));
        assert_eq!(rx.recv().unwrap(), RESULT_MEDIA_PLAYER_INACTIVE);
        b.seek(100);
        b.stop();
        assert!(!b.info().session);
    }

    #[test]
    fn filetime_is_after_2020() {
        // 2020-01-01 in 100 ns since 1601
        assert!(filetime_now() > 132_223_104_000_000_000);
    }

    /// The session manager's DLL stays loaded for the cached factory
    /// (two crashes on 2026-10-22 in RequestAsync after COM unloaded it)
    #[test]
    fn the_session_manager_dll_is_kept_loaded() {
        let _com = win_sys::audio_policy::ComApartment::init_mta().unwrap();
        // No media sessions on a build machine: nothing to check there
        let Some(m) = wait_result!(Manager::RequestAsync()) else { return };
        let dll = win_sys::com::pin_module_of(&m).unwrap();
        assert!(dll.to_ascii_lowercase().ends_with(".dll"), "{dll}");
    }
}
