//! Processes: find one by executable name (Toolhelp), tell when one has
//! exited, read an executable's description, and the Windows build number.
//!
//! Used by Application capture mode (per-process loopback), which follows an
//! app by its executable name and attaches again when it is restarted.

use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};

use windows::Win32::Foundation::{HANDLE, WAIT_OBJECT_0};
use windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::System::SystemInformation::OSVERSIONINFOW;
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
    QueryFullProcessImageNameW, WaitForSingleObject,
};
use windows::core::{PCWSTR, PWSTR, s, w};

/// One running process, from a Toolhelp snapshot
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessEntry {
    pub pid: u32,
    pub parent_pid: u32,
    /// Executable file name, e.g. "player.exe" (no path)
    pub exe: String,
}

/// Wrap a handle the caller owns so that it is closed on drop
fn owned(handle: HANDLE) -> OwnedHandle {
    // SAFETY: every caller passes a handle it just opened and owns, valid
    // and not closed elsewhere, so OwnedHandle may close it exactly once.
    unsafe { OwnedHandle::from_raw_handle(handle.0) }
}

/// All running processes (Toolhelp snapshot)
pub fn processes() -> windows::core::Result<Vec<ProcessEntry>> {
    // SAFETY: plain call; the snapshot handle is owned and closed below.
    let snap = owned(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)? });
    let snap_handle = HANDLE(snap.as_raw_handle());
    let mut out = Vec::new();
    let mut pe = PROCESSENTRY32W { dwSize: size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
    // SAFETY: `snap_handle` is a live snapshot handle and `pe` is a valid
    // PROCESSENTRY32W with dwSize set, as the API requires.
    let mut ok = unsafe { Process32FirstW(snap_handle, &mut pe) }.is_ok();
    while ok {
        out.push(ProcessEntry {
            pid: pe.th32ProcessID,
            parent_pid: pe.th32ParentProcessID,
            exe: from_wide_nul(&pe.szExeFile),
        });
        // SAFETY: as for Process32FirstW above.
        ok = unsafe { Process32NextW(snap_handle, &mut pe) }.is_ok();
    }
    tracing::trace!("process: {} running", out.len());
    Ok(out)
}

/// Full path of a process's executable, or None (exited, or not accessible:
/// PROCESS_QUERY_LIMITED_INFORMATION is enough for most processes,
/// including elevated ones, but not for protected system processes)
pub fn image_path(pid: u32) -> Option<String> {
    // SAFETY: plain call; the handle is owned and closed on drop.
    let process = match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) } {
        Ok(h) => owned(h),
        Err(e) => {
            tracing::trace!("process: cannot open {pid} for its image path: {e}");
            return None;
        }
    };
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    // SAFETY: `buf` is writable for `len` UTF-16 units, and the call writes
    // the length it used back into `len`.
    unsafe {
        QueryFullProcessImageNameW(
            HANDLE(process.as_raw_handle()),
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
    }
    .ok()?;
    let len = (len as usize).min(buf.len());
    Some(String::from_utf16_lossy(&buf[..len]))
}

/// FileDescription from an executable's version resource, e.g.
/// "Music Player" (the name Windows shows for it), or None
pub fn file_description(path: &str) -> Option<String> {
    let wpath: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
    let wpath = PCWSTR(wpath.as_ptr());
    // SAFETY: `wpath` is NUL-terminated and outlives the call.
    let size = unsafe { GetFileVersionInfoSizeW(wpath, None) };
    if size == 0 {
        return None;
    }
    let mut data = vec![0u8; size as usize];
    // SAFETY: `data` is writable for `size` bytes, as passed.
    unsafe { GetFileVersionInfoW(wpath, None, size, data.as_mut_ptr().cast()) }.ok()?;

    // The first translation (language, code page) names the string table
    let translation = query_value(&data, "\\VarFileInfo\\Translation")?;
    if translation.len() < 4 {
        return None;
    }
    let lang = u16::from_le_bytes([translation[0], translation[1]]);
    let code_page = u16::from_le_bytes([translation[2], translation[3]]);
    let key = format!("\\StringFileInfo\\{lang:04x}{code_page:04x}\\FileDescription");
    let desc = query_value(&data, &key)?;
    let units: Vec<u16> = desc.as_chunks::<2>().0.iter().map(|&c| u16::from_le_bytes(c)).collect();
    let s = from_wide_nul(&units);
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_owned())
}

/// VerQueryValueW over a version-info block: the bytes of `sub_block`,
/// bounds-checked against `block`. String values come back as UTF-16 bytes.
fn query_value<'a>(block: &'a [u8], sub_block: &str) -> Option<&'a [u8]> {
    let wsub: Vec<u16> = sub_block.encode_utf16().chain(Some(0)).collect();
    let mut ptr: *mut core::ffi::c_void = core::ptr::null_mut();
    let mut len = 0u32;
    // SAFETY: `block` is the buffer GetFileVersionInfoW filled, `wsub` is
    // NUL-terminated, and both out pointers are valid.
    let found = unsafe { VerQueryValueW(block.as_ptr().cast(), PCWSTR(wsub.as_ptr()), &mut ptr, &mut len) };
    if !found.as_bool() || ptr.is_null() || len == 0 {
        return None;
    }
    // The value points into `block`. String values report their length in
    // UTF-16 units, binary ones in bytes: take the larger reading and clamp
    // it to the block, so a malformed resource cannot make us read past it.
    let start = (ptr as usize).checked_sub(block.as_ptr() as usize)?;
    if start >= block.len() {
        return None;
    }
    let bytes = if sub_block.starts_with("\\StringFileInfo") { len as usize * 2 } else { len as usize };
    let end = (start + bytes).min(block.len());
    Some(&block[start..end])
}

/// UTF-16 up to the first NUL
fn from_wide_nul(units: &[u16]) -> String {
    let end = units.iter().position(|&c| c == 0).unwrap_or(units.len());
    String::from_utf16_lossy(&units[..end])
}

/// A process being watched for its exit. Keeps a SYNCHRONIZE handle open,
/// so the answer stays right even if Windows reuses the process ID for a new
/// process after this one exits.
#[derive(Debug)]
pub struct ProcessWatch {
    pid: u32,
    handle: OwnedHandle,
}

impl ProcessWatch {
    /// Start watching `pid`; None when there is no such process (or it
    /// cannot be opened, which SYNCHRONIZE access almost never prevents)
    pub fn open(pid: u32) -> Option<ProcessWatch> {
        // SAFETY: plain call; the handle is owned and closed on drop.
        let handle = match unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) } {
            Ok(h) => h,
            Err(e) => {
                tracing::debug!("process: cannot watch {pid}: {e}");
                return None;
            }
        };
        tracing::debug!("process: watching {pid}");
        Some(ProcessWatch { pid, handle: owned(handle) })
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// True once the process has exited (a process handle is signaled then)
    pub fn exited(&self) -> bool {
        // SAFETY: the handle is open for the lifetime of `self`; a zero
        // timeout only polls its state.
        let r = unsafe { WaitForSingleObject(HANDLE(self.handle.as_raw_handle()), 0) };
        r == WAIT_OBJECT_0
    }
}

/// Windows build number, e.g. 19045 or 26100. Uses RtlGetVersion, which
/// (unlike GetVersionEx) is not subject to the manifest's compatibility
/// shims. 0 if it cannot be read.
pub fn windows_build() -> u32 {
    type RtlGetVersionFn = unsafe extern "system" fn(*mut OSVERSIONINFOW) -> i32;
    // SAFETY: ntdll.dll is mapped into every process; the module handle is
    // not reference counted and needs no release.
    let Ok(ntdll) = (unsafe { GetModuleHandleW(w!("ntdll.dll")) }) else {
        return 0;
    };
    // SAFETY: plain lookup in a loaded module, with a NUL-terminated name.
    let Some(proc) = (unsafe { GetProcAddress(ntdll, s!("RtlGetVersion")) }) else {
        return 0;
    };
    // SAFETY: RtlGetVersion has exactly this signature:
    // NTSTATUS RtlGetVersion(PRTL_OSVERSIONINFOW), with RTL_OSVERSIONINFOW
    // laid out like OSVERSIONINFOW and NTSTATUS an i32.
    let rtl_get_version: RtlGetVersionFn = unsafe { core::mem::transmute(proc) };
    let mut vi = OSVERSIONINFOW { dwOSVersionInfoSize: size_of::<OSVERSIONINFOW>() as u32, ..Default::default() };
    // SAFETY: `vi` is a valid OSVERSIONINFOW with its size field set.
    let status = unsafe { rtl_get_version(&mut vi) };
    if status != 0 {
        tracing::debug!("process: RtlGetVersion failed (0x{:08X})", status as u32);
        return 0;
    }
    tracing::debug!("process: Windows {}.{} build {}", vi.dwMajorVersion, vi.dwMinorVersion, vi.dwBuildNumber);
    vi.dwBuildNumber
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_number_is_plausible() {
        // Windows 10 RTM is build 10240
        assert!(windows_build() >= 10240, "build {}", windows_build());
    }

    #[test]
    fn finds_own_process() {
        let me = std::process::id();
        let list = processes().unwrap();
        let entry = list.iter().find(|p| p.pid == me).expect("own process listed");
        assert!(entry.exe.to_ascii_lowercase().ends_with(".exe"));
        let path = image_path(me).expect("own image path");
        assert!(path.to_ascii_lowercase().ends_with(&entry.exe.to_ascii_lowercase()));
    }

    #[test]
    fn watch_own_process() {
        let w = ProcessWatch::open(std::process::id()).expect("open self");
        assert!(!w.exited());
        // PID 0 (System Idle Process) cannot be opened
        assert!(ProcessWatch::open(0).is_none());
    }

    #[test]
    fn watch_exited_process() {
        let mut child = std::process::Command::new("cmd").args(["/C", "exit 0"]).spawn().unwrap();
        let w = ProcessWatch::open(child.id());
        child.wait().unwrap();
        // The child may have exited before we opened it; if we did open it,
        // the kept handle must report the exit.
        if let Some(w) = w {
            assert!(w.exited());
        }
    }

    #[test]
    fn description_of_a_system_exe() {
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
        let desc = file_description(&format!("{windir}\\System32\\notepad.exe"));
        // Localised, but present on every Windows with notepad installed
        if let Some(d) = desc {
            assert!(!d.is_empty());
        }
        assert_eq!(file_description("Z:\\no\\such\\file.exe"), None);
    }
}
