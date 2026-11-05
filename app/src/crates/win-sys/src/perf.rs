//! What this process costs and how the system clock ticks, for the stream
//! health log (issue #84: a stream after the first stuttered until the app
//! was restarted; a thread left running from the first stream, or another
//! program changing the timer resolution, shows here).

use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::time::Duration;

use windows::Win32::Foundation::{FILETIME, HANDLE};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::System::Threading::{GetCurrentProcess, GetCurrentProcessId, GetProcessTimes};
use windows::core::{s, w};

fn filetime(t: FILETIME) -> Duration {
    // 100 ns units
    Duration::from_nanos((((t.dwHighDateTime as u64) << 32) | t.dwLowDateTime as u64) * 100)
}

/// CPU time this process has used (kernel and user, all threads)
pub fn process_cpu_time() -> Option<Duration> {
    let (mut created, mut exited, mut kernel, mut user) = Default::default();
    // SAFETY: the pseudo handle of the current process needs no closing;
    // the four out pointers are valid FILETIMEs.
    unsafe { GetProcessTimes(GetCurrentProcess(), &mut created, &mut exited, &mut kernel, &mut user) }.ok()?;
    Some(filetime(kernel) + filetime(user))
}

/// Threads of this process (Toolhelp snapshot)
pub fn process_threads() -> Option<u32> {
    // SAFETY: plain call
    let pid = unsafe { GetCurrentProcessId() };
    // SAFETY: plain call; the snapshot handle is owned and closed below
    let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }.ok()?;
    // SAFETY: a handle we just opened and own, closed once by OwnedHandle
    let snap = unsafe { OwnedHandle::from_raw_handle(snap.0) };
    let handle = HANDLE(snap.as_raw_handle());
    let mut pe = PROCESSENTRY32W { dwSize: size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
    // SAFETY: a live snapshot handle and a PROCESSENTRY32W with dwSize set
    let mut ok = unsafe { Process32FirstW(handle, &mut pe) }.is_ok();
    while ok {
        if pe.th32ProcessID == pid {
            return Some(pe.cntThreads);
        }
        // SAFETY: as above
        ok = unsafe { Process32NextW(handle, &mut pe) }.is_ok();
    }
    None
}

/// The system timer's current resolution (15.625 ms by default; 1 ms or
/// less while some program asks for it). NtQueryTimerResolution, which no
/// documented API replaces.
pub fn timer_resolution() -> Option<Duration> {
    type NtQueryTimerResolutionFn = unsafe extern "system" fn(*mut u32, *mut u32, *mut u32) -> i32;
    // SAFETY: ntdll.dll is mapped into every process; the module handle is
    // not reference counted and needs no release.
    let ntdll = unsafe { GetModuleHandleW(w!("ntdll.dll")) }.ok()?;
    // SAFETY: plain lookup in a loaded module, with a NUL-terminated name
    let proc = unsafe { GetProcAddress(ntdll, s!("NtQueryTimerResolution")) }?;
    // SAFETY: NTSTATUS NtQueryTimerResolution(PULONG Minimum, PULONG
    // Maximum, PULONG Current), NTSTATUS an i32
    let query: NtQueryTimerResolutionFn = unsafe { core::mem::transmute(proc) };
    let (mut min, mut max, mut current) = (0u32, 0u32, 0u32);
    // SAFETY: three valid out pointers
    if unsafe { query(&mut min, &mut max, &mut current) } != 0 {
        return None;
    }
    Some(Duration::from_nanos(current as u64 * 100))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readings_of_this_process() {
        assert!(process_cpu_time().is_some());
        assert!(process_threads().is_some_and(|n| n >= 1));
        let r = timer_resolution().unwrap();
        assert!(r > Duration::ZERO && r <= Duration::from_millis(16), "{r:?}");
    }
}
