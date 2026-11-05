//! Administrator rights: whether this process has them, and running another
//! program with them (the UAC prompt).
//!
//! Only the WinUSB driver helper (`a2dpwb-driver.exe`) needs administrator
//! rights. The core service stays unelevated and starts the helper through
//! `run_elevated` for the one operation, so a UAC prompt appears only when
//! the user asked to assign or restore a driver.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows::Win32::Foundation::{CloseHandle, ERROR_CANCELLED, HANDLE, WAIT_FAILED};
use windows::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation};
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetExitCodeProcess, INFINITE, OpenProcessToken, WaitForSingleObject,
};
use windows::Win32::UI::Shell::{SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
use windows::core::{PCWSTR, w};

/// Why an elevated program could not be run
#[derive(Debug, thiserror::Error)]
pub enum ElevationError {
    /// The user answered "No" on the UAC prompt
    #[error("the administrator prompt was declined")]
    Cancelled,
    #[error("could not start the program as administrator: {0}")]
    Start(windows::core::Error),
    #[error("could not wait for the elevated program: {0}")]
    Wait(windows::core::Error),
}

/// True if this process runs with an elevated (administrator) token. False
/// also when the token cannot be read: callers use it to refuse work that
/// would fail later with a less clear error.
pub fn is_elevated() -> bool {
    let mut token = HANDLE::default();
    // SAFETY: GetCurrentProcess returns a pseudo handle that needs no
    // closing; `token` is a valid out pointer.
    if let Err(e) = unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } {
        tracing::debug!("elevation: OpenProcessToken failed: {e}");
        return false;
    }
    let mut elevation = TOKEN_ELEVATION::default();
    let mut returned = 0u32;
    // SAFETY: `token` was opened with TOKEN_QUERY; the buffer is a
    // TOKEN_ELEVATION and its exact size is passed.
    let ok = unsafe {
        GetTokenInformation(
            token,
            TokenElevation,
            Some(std::ptr::from_mut(&mut elevation).cast()),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        )
    }
    .is_ok();
    // SAFETY: `token` is a handle this function opened and owns.
    let _ = unsafe { CloseHandle(token) };
    tracing::debug!("elevation: token read {ok}, elevated {}", elevation.TokenIsElevated != 0);
    ok && elevation.TokenIsElevated != 0
}

/// Runs `exe` with `args` as administrator (the UAC prompt, ShellExecuteEx
/// "runas"), waits for it to exit and returns its exit code. The program
/// gets no visible console window.
///
/// The elevated program's stdout cannot be read by the caller (ShellExecute
/// gives no pipes), so a program that reports results must write them to a
/// file the caller names: `a2dpwb-driver.exe` takes `--out <file>` for this.
/// Waits without a time limit: the user may take a while on the prompt.
pub fn run_elevated<S: AsRef<OsStr>>(exe: &Path, args: &[S]) -> Result<u32, ElevationError> {
    let file = wide(exe.as_os_str());
    let line = join_args(args);
    tracing::debug!("elevation: running {} {line} as administrator", exe.display());
    let started = std::time::Instant::now();
    let params = wide(OsStr::new(&line));
    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        // NOCLOSEPROCESS: hand back the process handle to wait on.
        // NOASYNC: finish the launch before returning (no DDE leftovers).
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
        lpVerb: w!("runas"),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(params.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    // SAFETY: `info` is fully initialised with its size; the strings it
    // points to (`file`, `params`, the static verb) outlive the call.
    if let Err(e) = unsafe { ShellExecuteExW(&mut info) } {
        tracing::debug!("elevation: ShellExecuteEx failed after {:?}: {e}", started.elapsed());
        if e.code() == ERROR_CANCELLED.to_hresult() {
            return Err(ElevationError::Cancelled);
        }
        return Err(ElevationError::Start(e));
    }
    let process = info.hProcess;
    if process.is_invalid() {
        // Happens only when ShellExecute handed the file to another program
        // (not for an .exe): there is nothing to wait on
        return Err(ElevationError::Start(windows::core::Error::from_thread()));
    }
    // SAFETY: `process` is the valid handle SEE_MASK_NOCLOSEPROCESS returned.
    let waited = unsafe { WaitForSingleObject(process, INFINITE) };
    let result = if waited == WAIT_FAILED {
        Err(ElevationError::Wait(windows::core::Error::from_thread()))
    } else {
        let mut code = 0u32;
        // SAFETY: valid process handle and out pointer.
        match unsafe { GetExitCodeProcess(process, &mut code) } {
            Ok(()) => Ok(code),
            Err(e) => Err(ElevationError::Wait(e)),
        }
    };
    // SAFETY: the process handle is ours to close (SEE_MASK_NOCLOSEPROCESS).
    let _ = unsafe { CloseHandle(process) };
    match &result {
        Ok(code) => tracing::debug!("elevation: exited with code {code} (0x{code:08X}) after {:?}", started.elapsed()),
        Err(e) => tracing::debug!("elevation: {e}"),
    }
    result
}

/// A NUL-terminated UTF-16 copy of `s`
fn wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}

/// One command line from separate arguments, quoted so that the program's
/// CommandLineToArgvW / CRT parsing gives back exactly `args` (JSON passed
/// as an argument has quotes and backslashes in it)
pub fn join_args<S: AsRef<OsStr>>(args: &[S]) -> String {
    args.iter().map(|a| quote_arg(&a.as_ref().to_string_lossy())).collect::<Vec<_>>().join(" ")
}

/// Quotes one argument by the MSVC CRT rules: backslashes are literal
/// unless they precede a quote, so those before a quote (or the closing
/// quote) are doubled, and quotes are escaped with a backslash
fn quote_arg(arg: &str) -> String {
    if !arg.is_empty() && !arg.contains([' ', '\t', '\n', '\u{0B}', '"']) {
        return arg.to_string();
    }
    let mut out = String::with_capacity(arg.len() + 2);
    out.push('"');
    let mut backslashes = 0usize;
    for c in arg.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                out.extend(std::iter::repeat_n('\\', backslashes * 2 + 1));
                out.push('"');
                backslashes = 0;
            }
            _ => {
                out.extend(std::iter::repeat_n('\\', backslashes));
                out.push(c);
                backslashes = 0;
            }
        }
    }
    out.extend(std::iter::repeat_n('\\', backslashes * 2));
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_follows_crt_rules() {
        assert_eq!(quote_arg("list"), "list");
        assert_eq!(quote_arg(""), "\"\"");
        assert_eq!(quote_arg("a b"), "\"a b\"");
        assert_eq!(quote_arg(r"C:\dir\"), r"C:\dir\");
        assert_eq!(quote_arg(r"C:\my dir\"), r#""C:\my dir\\""#);
        // Backslashes stay single unless a quote follows them
        assert_eq!(quote_arg(r#"{"a":"b\\c"}"#), r#""{\"a\":\"b\\c\"}""#);
        assert_eq!(quote_arg(r#"a\"b"#), r#""a\\\"b""#);
        assert_eq!(
            join_args(&["restore", r"USB\VID_0BDA&PID_8771\00E04C", "--previous", r#"{"x":1}"#]),
            r#"restore USB\VID_0BDA&PID_8771\00E04C --previous "{\"x\":1}""#
        );
    }

    /// What the started program sees: Windows' own command line parser
    #[test]
    fn quoted_arguments_parse_back() {
        use windows::Win32::Foundation::{HLOCAL, LocalFree};
        use windows::Win32::UI::Shell::CommandLineToArgvW;
        let args = [
            "restore",
            r"USB\VID_0BDA&PID_8771\00E04C",
            "--previous",
            r#"{"inf_name":"bth.inf","path":"C:\\dir\\","q":"a \"b\""}"#,
            "",
            r"trailing\",
            r"C:\my dir\",
        ];
        let line = wide(OsStr::new(&format!("a2dpwb-driver.exe {}", join_args(&args))));
        let mut n = 0i32;
        // SAFETY: NUL-terminated command line and out pointer; the array
        // is freed with LocalFree below.
        let argv = unsafe { CommandLineToArgvW(PCWSTR(line.as_ptr()), &mut n) };
        assert!(!argv.is_null());
        // SAFETY: CommandLineToArgvW returned `n` valid string pointers.
        let parsed: Vec<String> =
            unsafe { std::slice::from_raw_parts(argv, n as usize).iter().map(|p| p.to_string().unwrap()).collect() };
        // SAFETY: allocated by CommandLineToArgvW, freed once.
        unsafe { LocalFree(Some(HLOCAL(argv.cast()))) };
        assert_eq!(&parsed[1..], &args);
    }

    #[test]
    fn elevation_query_does_not_fail() {
        // Whatever the answer, it must come back without panicking
        let _ = is_elevated();
    }
}
