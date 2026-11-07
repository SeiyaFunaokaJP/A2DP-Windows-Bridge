//! Crash reports: a minidump and a short text file in a folder when the
//! process crashes in native code (an access violation in BTstack or a
//! codec library, the C runtime's abort()), and on request ([`write`], the
//! panic hooks).
//!
//! Files: `<dir>\<prefix>-YYYYMMDD-HHMMSS-mmm.dmp` and `.txt` beside it; the
//! last [`KEEP`] reports of a prefix are kept. The dump holds every thread's
//! stack and the loaded modules (no heap): open it in Visual Studio or WinDbg
//! with the release's .pdb files.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};

use windows::Win32::Foundation::{CloseHandle, GENERIC_WRITE, HANDLE};
use windows::Win32::Storage::FileSystem::{CREATE_ALWAYS, CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_NONE};
use windows::Win32::System::Diagnostics::Debug::{
    EXCEPTION_POINTERS, MINIDUMP_EXCEPTION_INFORMATION, MiniDumpNormal, MiniDumpWithThreadInfo,
    MiniDumpWithUnloadedModules, MiniDumpWriteDump, SetUnhandledExceptionFilter,
};
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::Win32::System::Threading::{GetCurrentProcess, GetCurrentProcessId, GetCurrentThreadId};
use windows::core::PCWSTR;

/// Reports kept per prefix
pub const KEEP: usize = 5;
/// Reports written per process run at most (a panic loop writes no more)
const MAX_PER_RUN: u32 = 3;
/// EXCEPTION_CONTINUE_SEARCH: Windows Error Reporting still sees the crash
const CONTINUE_SEARCH: i32 = 0;
const SIGABRT: i32 = 22;

struct Target {
    dir: PathBuf,
    prefix: String,
    /// "bluecaster-core 2.0.2", the first line of the text file
    about: String,
}

static TARGET: OnceLock<Target> = OnceLock::new();
static WRITTEN: AtomicU32 = AtomicU32::new(0);

unsafe extern "C" {
    /// The C runtime's signal(): abort() raises SIGABRT before it ends the
    /// process
    fn signal(sig: i32, handler: usize) -> usize;
}

/// Writes crash reports to `dir` from now on. Call once, early in main.
pub fn install(dir: &Path, prefix: &str, about: &str) {
    let target = Target { dir: dir.to_path_buf(), prefix: prefix.into(), about: about.into() };
    if TARGET.set(target).is_err() {
        return;
    }
    // SAFETY: plain function pointers that live for the whole process
    unsafe {
        SetUnhandledExceptionFilter(Some(on_exception));
        signal(SIGABRT, on_abort as extern "C" fn(i32) as usize);
    }
}

/// The folder given to [`install`]
pub fn dir() -> Option<&'static Path> {
    TARGET.get().map(|t| t.dir.as_path())
}

/// Writes a report now (a panic): the dump of this process and `reason`
/// with `details` (a backtrace) in the text file. None if [`install`] was
/// not called, the run's reports are used up, or writing failed.
pub fn write(reason: &str, details: &str) -> Option<PathBuf> {
    write_report(reason, details, std::ptr::null())
}

unsafe extern "system" fn on_exception(info: *const EXCEPTION_POINTERS) -> i32 {
    // SAFETY: Windows passes valid pointers (or none)
    let record = unsafe { info.as_ref().and_then(|i| i.ExceptionRecord.as_ref()) };
    let reason = match record {
        Some(r) => {
            format!("unhandled exception 0x{:08X} at 0x{:X}", r.ExceptionCode.0 as u32, r.ExceptionAddress as usize)
        }
        None => "unhandled exception".into(),
    };
    write_report(&reason, "", info);
    CONTINUE_SEARCH
}

extern "C" fn on_abort(_sig: i32) {
    write_report("abort() called", "", std::ptr::null());
}

fn write_report(reason: &str, details: &str, exception: *const EXCEPTION_POINTERS) -> Option<PathBuf> {
    let target = TARGET.get()?;
    if WRITTEN.fetch_add(1, Ordering::Relaxed) >= MAX_PER_RUN {
        return None;
    }
    std::fs::create_dir_all(&target.dir).ok()?;
    let stem = format!("{}-{}", target.prefix, stamp());
    let dump = target.dir.join(format!("{stem}.dmp"));
    let text = format!(
        "{}\nprocess {} thread {}\n{reason}\n{}\n{details}\n",
        target.about,
        std::process::id(),
        // SAFETY: no arguments
        unsafe { GetCurrentThreadId() },
        if write_dump(&dump, exception) { format!("dump: {}", dump.display()) } else { "no dump".into() },
    );
    let _ = std::fs::write(target.dir.join(format!("{stem}.txt")), text);
    prune(&target.dir, &target.prefix);
    Some(dump)
}

/// A few tries: MiniDumpWriteDump of the calling process can fail while its
/// other threads start or end (as on a loaded CI runner)
fn write_dump(path: &Path, exception: *const EXCEPTION_POINTERS) -> bool {
    (0..3).any(|_| write_dump_once(path, exception))
}

fn write_dump_once(path: &Path, exception: *const EXCEPTION_POINTERS) -> bool {
    let wide: Vec<u16> = OsStr::new(path).encode_wide().chain(Some(0)).collect();
    // SAFETY: a NUL-terminated path; the handle is closed below
    let file = unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            GENERIC_WRITE.0,
            FILE_SHARE_NONE,
            None,
            CREATE_ALWAYS,
            FILE_ATTRIBUTE_NORMAL,
            Some(HANDLE::default()),
        )
    };
    let Ok(file) = file else { return false };
    let info = MINIDUMP_EXCEPTION_INFORMATION {
        // SAFETY: no arguments
        ThreadId: unsafe { GetCurrentThreadId() },
        ExceptionPointers: exception as *mut _,
        ClientPointers: false.into(),
    };
    let kind = MiniDumpNormal | MiniDumpWithThreadInfo | MiniDumpWithUnloadedModules;
    // SAFETY: the process' own handles; `info` outlives the call
    let ok = unsafe {
        MiniDumpWriteDump(
            GetCurrentProcess(),
            GetCurrentProcessId(),
            file,
            kind,
            (!exception.is_null()).then_some(&info as *const _),
            None,
            None,
        )
        .is_ok()
    };
    // SAFETY: opened above
    let _ = unsafe { CloseHandle(file) };
    if !ok {
        let _ = std::fs::remove_file(path);
    }
    ok
}

/// "YYYYMMDD-HHMMSS-mmm", local time
fn stamp() -> String {
    // SAFETY: no arguments
    let t = unsafe { GetLocalTime() };
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}-{:03}",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond, t.wMilliseconds
    )
}

/// Keeps the newest [`KEEP`] reports of `prefix` (the names sort by time)
fn prune(dir: &Path, prefix: &str) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut stems: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().to_str().map(str::to_owned))
        .filter(|n| n.starts_with(&format!("{prefix}-")) && n.ends_with(".txt"))
        .map(|n| n.trim_end_matches(".txt").to_owned())
        .collect();
    stems.sort();
    let old = stems.len().saturating_sub(KEEP);
    for stem in &stems[..old] {
        let _ = std::fs::remove_file(dir.join(format!("{stem}.txt")));
        let _ = std::fs::remove_file(dir.join(format!("{stem}.dmp")));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_newest_reports() {
        let dir = std::env::temp_dir().join(format!("bluecaster-crash-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for i in 0..KEEP + 2 {
            let stem = format!("x-20261021-1200{i:02}-000");
            std::fs::write(dir.join(format!("{stem}.txt")), "").unwrap();
            std::fs::write(dir.join(format!("{stem}.dmp")), "").unwrap();
        }
        std::fs::write(dir.join("other-20200101-000000-000.txt"), "").unwrap();
        prune(&dir, "x");
        let mut names: Vec<String> =
            std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        assert_eq!(names.len(), KEEP * 2 + 1);
        assert_eq!(names[0], "other-20200101-000000-000.txt");
        assert!(names[1].starts_with("x-20261021-120002"), "{names:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn writes_a_dump_of_this_process() {
        let path = std::env::temp_dir().join(format!("bluecaster-dump-{}.dmp", std::process::id()));
        assert!(write_dump(&path, std::ptr::null()));
        let data = std::fs::read(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(&data[..4], b"MDMP");
    }
}
