//! bluecaster-core.exe is a GUI-subsystem program (no console window for the
//! resident process); `--cli` attaches to the console of the shell that
//! started it, so its output appears there.

use windows::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};

/// Attaches to the parent process's console. False when there is none
/// (started from Explorer) or one is attached already; output that is
/// redirected to a file or pipe works either way.
pub fn attach_parent() -> bool {
    // SAFETY: no pointers; fails harmlessly when there is no parent console.
    unsafe { AttachConsole(ATTACH_PARENT_PROCESS) }.is_ok()
}
