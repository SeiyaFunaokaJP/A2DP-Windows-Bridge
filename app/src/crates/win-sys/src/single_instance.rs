//! One running instance: a named mutex
//!
//! v1 (GUI mode) creates the mutex "A2DPWB_SingleInstance" at start and
//! quits if it already exists (main.cpp). v2 uses its own mutex the same
//! way. v2 replaces v1 as a newer version would: it asks a running v1 to
//! exit ([`ask_v1_to_exit`]) and then holds v1's mutex too, so that a v1
//! started later quits at once (both cannot use the same USB adapter).

use windows::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HWND, LPARAM, WPARAM,
};
use windows::Win32::System::Threading::{CreateMutexW, OpenMutexW, SYNCHRONIZATION_SYNCHRONIZE};
use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowTextW, PostMessageW, WM_COMMAND};
use windows::core::{BOOL, HSTRING};

/// The mutex v1 holds while its GUI runs
pub const V1_MUTEX_NAME: &str = "A2DPWB_SingleInstance";

/// v1's main window title starts with this ("A2DPWB v1.1.1", wx_main_frame.cpp)
const V1_TITLE_PREFIX: &str = "A2DPWB v1.";
/// The menu command v1's main window exits on (wxID_EXIT: File > Exit, and
/// the tray menu's Exit)
const V1_EXIT_COMMAND: usize = 5006;

/// Asks a running v1 to exit as its own Exit command does (the stream
/// stops and the adapter is released): posts that command to v1's main
/// window, also while it is hidden in the tray. The number of windows asked
/// (0: none found). Wait for [`V1_MUTEX_NAME`] to go to know it has exited.
pub fn ask_v1_to_exit() -> usize {
    unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let mut buf = [0u16; 128];
        // SAFETY: a window handle EnumWindows passes, and a buffer the
        // call writes at most buf.len() units into.
        let n = unsafe { GetWindowTextW(hwnd, &mut buf) };
        let title = String::from_utf16_lossy(&buf[..n.max(0) as usize]);
        if title.starts_with(V1_TITLE_PREFIX) {
            // SAFETY: lparam is the &mut Vec ask_v1_to_exit passes, alive
            // for the whole EnumWindows call.
            unsafe { &mut *(lparam.0 as *mut Vec<HWND>) }.push(hwnd);
        }
        BOOL(1)
    }
    let mut windows: Vec<HWND> = Vec::new();
    // SAFETY: the callback only uses `windows` through lparam during the call.
    let _ = unsafe { EnumWindows(Some(collect), LPARAM(&mut windows as *mut Vec<HWND> as isize)) };
    let found = windows.len();
    let asked = windows
        .into_iter()
        // SAFETY: posting a message to a window handle; a stale one fails.
        .filter(|&w| unsafe { PostMessageW(Some(w), WM_COMMAND, WPARAM(V1_EXIT_COMMAND), LPARAM(0)) }.is_ok())
        .count();
    tracing::debug!("single_instance: {found} v1 window(s) found, {asked} asked to exit");
    asked
}

/// Holds the named mutex; another `acquire` of the same name fails while
/// this lives. Released on drop (and by Windows when the process ends).
#[derive(Debug)]
pub struct SingleInstance {
    handle: HANDLE,
}

// SAFETY: a mutex handle is a kernel object reference usable from any
// thread; the guard is never used for locking, only for its existence.
unsafe impl Send for SingleInstance {}
// SAFETY: no method uses the handle through &self.
unsafe impl Sync for SingleInstance {}

impl SingleInstance {
    /// Creates the mutex `name` (session-local, like v1's). Ok(None) if it
    /// already exists, i.e. another instance runs.
    pub fn acquire(name: &str) -> windows::core::Result<Option<SingleInstance>> {
        // Not taking ownership: only the mutex's existence matters, and an
        // owned mutex would be "abandoned" when the creating thread ends
        // SAFETY: default security, NUL-terminated name.
        let handle = unsafe { CreateMutexW(None, false, &HSTRING::from(name)) }
            .inspect_err(|e| tracing::debug!("single_instance: CreateMutex({name}) failed: {e}"))?;
        // SAFETY: reads this thread's last error, set by CreateMutexW just
        // above (it reports an existing mutex with a valid handle).
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            // SAFETY: the handle was just opened here.
            let _ = unsafe { CloseHandle(handle) };
            tracing::debug!("single_instance: {name} exists, another instance runs");
            return Ok(None);
        }
        tracing::debug!("single_instance: {name} acquired");
        Ok(Some(SingleInstance { handle }))
    }
}

impl Drop for SingleInstance {
    fn drop(&mut self) {
        // SAFETY: the handle is owned by this guard and closed only here.
        let _ = unsafe { CloseHandle(self.handle) };
    }
}

/// True if a mutex `name` exists, i.e. the program holding it runs. A
/// mutex that exists but cannot be opened (another user, higher integrity)
/// counts as held.
pub fn is_held(name: &str) -> bool {
    // SAFETY: NUL-terminated name; the handle is closed right away.
    match unsafe { OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, false, &HSTRING::from(name)) } {
        Ok(handle) => {
            // SAFETY: just opened here.
            let _ = unsafe { CloseHandle(handle) };
            tracing::trace!("single_instance: {name} is held");
            true
        }
        Err(e) => {
            tracing::trace!("single_instance: OpenMutex({name}): {e}");
            e.code() == ERROR_ACCESS_DENIED.to_hresult()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_acquire_fails_until_released() {
        let name = format!("A2DPWB_test_single_instance_{}", std::process::id());
        assert!(!is_held(&name));
        let first = SingleInstance::acquire(&name).unwrap().expect("first instance");
        assert!(is_held(&name));
        assert!(SingleInstance::acquire(&name).unwrap().is_none());
        drop(first);
        assert!(!is_held(&name));
        assert!(SingleInstance::acquire(&name).unwrap().is_some());
    }

    #[test]
    fn v1_check_runs() {
        // Read-only; v1 may or may not be running here
        let _ = is_held(V1_MUTEX_NAME);
    }
}
