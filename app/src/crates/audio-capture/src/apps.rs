//! Application capture helpers (v1 app_audio): the apps that have an audio
//! session (for the UI's app picker), finding a running app by executable
//! name, telling when it exited, and whether Windows supports the mode.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, OnceLock};

use win_sys::audio_policy::{self, ComApartment};
use win_sys::process::{self, ProcessWatch};

/// Per-process loopback (AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK).
/// Microsoft documents build 20348, but it works from Windows 10 2004
/// (build 19041), which covers 20H2-22H2 (19042-19045). OBS's Application
/// Audio Capture uses the same threshold.
pub const APP_CAPTURE_MIN_BUILD: u32 = 19041;

/// Windows build number (RtlGetVersion, not subject to manifest lies);
/// 0 if unknown. Read once.
pub fn windows_build() -> u32 {
    static BUILD: OnceLock<u32> = OnceLock::new();
    *BUILD.get_or_init(process::windows_build)
}

/// True when this Windows supports Application capture mode
pub fn app_capture_supported() -> bool {
    windows_build() >= APP_CAPTURE_MIN_BUILD
}

/// An app with an audio session
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioApp {
    /// One of its processes that has a session (the app is followed by
    /// `exe`; [`find_process`] picks the process to capture)
    pub process_id: u32,
    /// Executable file name, e.g. "player.exe"
    pub exe: String,
    /// Display name as v1 shows it: "Music Player (player.exe)", or just
    /// the executable name when it has no description
    pub name: String,
}

/// Process IDs with an audio session (COM set up for the call)
fn session_pids() -> Vec<u32> {
    let _com = match ComApartment::init_mta() {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!("audio apps: CoInitializeEx failed: HRESULT 0x{:08x}", e.code().0 as u32);
            return Vec::new();
        }
    };
    audio_policy::session_process_ids().unwrap_or_else(|e| {
        tracing::debug!("audio apps: listing audio sessions failed: {e}");
        Vec::new()
    })
}

/// Apps with an audio session on any active output device, one entry per
/// executable name, sorted by display name (case-insensitive). Leaves out
/// this process and the system sounds session.
pub fn audio_apps() -> Vec<AudioApp> {
    let me = std::process::id();
    let mut by_exe: BTreeMap<String, AudioApp> = BTreeMap::new(); // key: lowercase exe
    for pid in session_pids() {
        if pid == me {
            continue;
        }
        let Some(path) = process::image_path(pid) else {
            continue;
        };
        let exe = file_name(&path).to_owned();
        let key = exe.to_lowercase();
        if by_exe.contains_key(&key) {
            continue;
        }
        let name = match process::file_description(&path) {
            Some(desc) => format!("{desc} ({exe})"),
            None => exe.clone(),
        };
        by_exe.insert(key, AudioApp { process_id: pid, exe, name });
    }
    let mut out: Vec<AudioApp> = by_exe.into_values().collect();
    out.sort_by_key(|a| a.name.to_lowercase());
    if tracing::enabled!(tracing::Level::DEBUG) {
        tracing::debug!("audio apps: {} apps with an audio session", out.len());
        for a in &out {
            tracing::debug!("audio apps: {} (pid {})", a.name, a.process_id);
        }
    }
    out
}

fn file_name(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

/// Process ID of a running app with this executable name (case-insensitive).
/// With several processes of that name (browsers), returns the top one of
/// its process tree, since capture includes the child processes; prefers an
/// instance that has an audio session when several are running.
pub fn find_process(exe: &str) -> Option<u32> {
    if exe.is_empty() {
        return None;
    }
    let want = exe.to_lowercase();
    let matches: Vec<(u32, u32)> = process::processes()
        .ok()?
        .into_iter()
        .filter(|p| p.exe.to_lowercase() == want)
        .map(|p| (p.pid, p.parent_pid))
        .collect();
    if matches.is_empty() {
        return None;
    }
    // Sessions are only looked up when there is a choice to make
    let sessions = if matches.len() > 1 { session_pids() } else { Vec::new() };
    let picked = pick_process(&matches, &sessions);
    tracing::trace!(
        "audio apps: {exe}: {} processes {matches:?} (pid, parent), sessions {sessions:?}, picked {picked:?}",
        matches.len()
    );
    picked
}

/// v1's choice among the processes of one executable. `matches`: (pid,
/// parent pid) of each; `sessions`: PIDs with an audio session, in order.
pub(crate) fn pick_process(matches: &[(u32, u32)], sessions: &[u32]) -> Option<u32> {
    let parent: BTreeMap<u32, u32> = matches.iter().copied().collect();
    // The top of a tree: walk up while the parent is another match
    let top = |mut pid: u32| {
        for _ in 0..64 {
            match parent.get(&pid) {
                Some(&pp) if pp != pid && parent.contains_key(&pp) => pid = pp,
                _ => break,
            }
        }
        pid
    };
    // Prefer the instance that is playing audio (several may be running)
    if let Some(&pid) = sessions.iter().find(|p| parent.contains_key(p)) {
        return Some(top(pid));
    }
    // Otherwise the first root (lowest PID whose parent is not a match)
    parent
        .iter()
        .find(|(_, pp)| !parent.contains_key(pp))
        .map(|(&pid, _)| pid)
        .or_else(|| parent.keys().next().copied())
}

/// True once the process has exited (or if there is no such process).
///
/// The first call for a PID opens a SYNCHRONIZE handle and keeps it, so the
/// answer stays right after the process exits: Windows does not reuse a PID
/// while a handle to its process is open.
pub fn process_exited(process_id: u32) -> bool {
    static WATCHES: OnceLock<Mutex<HashMap<u32, ProcessWatch>>> = OnceLock::new();
    /// Above this many kept handles, those of exited processes are closed
    const PRUNE_ABOVE: usize = 32;

    let mut watches = WATCHES.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    if let Some(w) = watches.get(&process_id) {
        return w.exited();
    }
    let Some(w) = ProcessWatch::open(process_id) else {
        return true;
    };
    let exited = w.exited();
    if watches.len() >= PRUNE_ABOVE {
        watches.retain(|_, w| !w.exited());
    }
    watches.insert(process_id, w);
    exited
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_single() {
        assert_eq!(pick_process(&[(100, 4)], &[]), Some(100));
        assert_eq!(pick_process(&[], &[]), None);
    }

    #[test]
    fn pick_tree_top() {
        // A browser: 200 is the main process, 300/400 its children, 500 a
        // child of 300
        let m = [(300, 200), (200, 50), (400, 200), (500, 300)];
        assert_eq!(pick_process(&m, &[]), Some(200));
        // The child playing audio leads to the same top
        assert_eq!(pick_process(&m, &[999, 500]), Some(200));
    }

    #[test]
    fn pick_prefers_the_instance_with_audio() {
        // Two independent instances; the second one plays
        let m = [(100, 10), (700, 20), (800, 700)];
        assert_eq!(pick_process(&m, &[]), Some(100));
        assert_eq!(pick_process(&m, &[800]), Some(700));
    }

    #[test]
    fn pick_survives_cycles() {
        // Parent PIDs can be stale and point at a reused PID: no hang
        let m = [(100, 200), (200, 100)];
        assert!(pick_process(&m, &[]).is_some());
        assert!(pick_process(&m, &[100]).is_some());
        assert_eq!(pick_process(&[(5, 5)], &[]), Some(5));
    }

    #[test]
    fn file_names() {
        assert_eq!(file_name("C:\\Program Files\\App\\player.exe"), "player.exe");
        assert_eq!(file_name("player.exe"), "player.exe");
    }

    #[test]
    fn build_and_support() {
        assert!(windows_build() >= 10240);
        assert_eq!(app_capture_supported(), windows_build() >= APP_CAPTURE_MIN_BUILD);
    }

    #[test]
    fn find_and_watch_a_child() {
        let mut child = std::process::Command::new("cmd").args(["/C", "ping -n 3 127.0.0.1 >NUL"]).spawn().unwrap();
        let pid = child.id();
        // cmd.exe runs elsewhere too; ours is one of the candidates
        assert!(find_process("CMD.EXE").is_some());
        assert!(!process_exited(pid));
        child.wait().unwrap();
        assert!(process_exited(pid));
        assert!(process_exited(pid), "stays exited");
        assert_eq!(find_process("no-such-app-a2dpwb.exe"), None);
        assert_eq!(find_process(""), None);
    }

    #[test]
    fn apps_list_is_well_formed() {
        let apps = audio_apps();
        for a in &apps {
            assert!(!a.exe.is_empty());
            assert!(a.name.contains(&a.exe));
            assert_ne!(a.process_id, std::process::id());
        }
        let mut sorted = apps.clone();
        sorted.sort_by_key(|a| a.name.to_lowercase());
        assert_eq!(apps, sorted);
    }
}
