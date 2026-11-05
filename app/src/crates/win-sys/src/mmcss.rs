//! MMCSS (Multimedia Class Scheduler Service) thread registration.
//!
//! The audio threads register with MMCSS so that a busy PC cannot hold them
//! back for tens of milliseconds: "Audio" for the capture thread (as v1's
//! WASAPI capture and test tone did), "Pro Audio" for the encode thread.
//! Registration belongs to the calling thread, so the guard is `!Send` and
//! reverts on the same thread when it is dropped.

use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Threading::{AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW};
use windows::core::PCWSTR;

/// MMCSS task names (keys under HKLM\...\Multimedia\SystemProfile\Tasks)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MmcssTask {
    /// "Audio": the WASAPI capture thread
    Audio,
    /// "Pro Audio": the encode thread, which has a deadline of its own
    ProAudio,
}

impl MmcssTask {
    /// The task name as MMCSS knows it
    pub fn name(self) -> &'static str {
        match self {
            MmcssTask::Audio => "Audio",
            MmcssTask::ProAudio => "Pro Audio",
        }
    }
}

/// The calling thread's MMCSS registration; dropping it reverts the thread
/// to normal scheduling. Holds a raw handle, so it is neither `Send` nor
/// `Sync`: AvRevertMmThreadCharacteristics must run on the registered thread.
#[derive(Debug)]
pub struct MmcssGuard {
    handle: HANDLE,
    task_index: u32,
}

impl MmcssGuard {
    /// Register the calling thread with `task`. Fails when the MMCSS
    /// service is unavailable (stopped, or a Windows edition without it);
    /// the thread then simply keeps its normal priority.
    pub fn register(task: MmcssTask) -> windows::core::Result<MmcssGuard> {
        let name: Vec<u16> = task.name().encode_utf16().chain(Some(0)).collect();
        let mut task_index = 0u32;
        // SAFETY: `name` is a NUL-terminated UTF-16 string that outlives the
        // call, and `task_index` is a valid out pointer. The task index starts
        // at 0, which asks MMCSS to assign a new one.
        let handle =
            unsafe { AvSetMmThreadCharacteristicsW(PCWSTR(name.as_ptr()), &mut task_index) }.inspect_err(|e| {
                tracing::debug!("MMCSS: registering {:?} as \"{}\" failed: {e}", thread_name(), task.name())
            })?;
        tracing::debug!("MMCSS: {:?} registered as \"{}\", task index {task_index}", thread_name(), task.name());
        Ok(MmcssGuard { handle, task_index })
    }

    /// The task index MMCSS assigned
    pub fn task_index(&self) -> u32 {
        self.task_index
    }
}

impl Drop for MmcssGuard {
    fn drop(&mut self) {
        // SAFETY: `handle` came from AvSetMmThreadCharacteristicsW on this
        // thread (the guard is !Send) and is reverted exactly once, here.
        if let Err(e) = unsafe { AvRevertMmThreadCharacteristics(self.handle) } {
            tracing::debug!("MMCSS revert failed: {e}");
        } else {
            tracing::debug!("MMCSS: {:?} reverted (task index {})", thread_name(), self.task_index);
        }
    }
}

fn thread_name() -> String {
    let t = std::thread::current();
    t.name().map_or_else(|| format!("{:?}", t.id()), str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_revert() {
        // The MMCSS service runs on every desktop Windows; a failure here
        // means the service is stopped, which is not this code's fault.
        match MmcssGuard::register(MmcssTask::Audio) {
            Ok(guard) => drop(guard),
            Err(e) => eprintln!("MMCSS unavailable: {e}"),
        }
    }

    #[test]
    fn task_names() {
        assert_eq!(MmcssTask::Audio.name(), "Audio");
        assert_eq!(MmcssTask::ProAudio.name(), "Pro Audio");
    }
}
