//! Sleep and resume notifications without a window
//!
//! The core service has no window to receive WM_POWERBROADCAST, so it
//! subscribes with PowerRegisterSuspendResumeNotification and a callback
//! (DEVICE_NOTIFY_CALLBACK), as v1 a2dp_service.cpp does. After a resume the
//! USB adapter has been reset and Bluetooth must be restarted.

use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};

use windows::Win32::Foundation::{ERROR_SUCCESS, HANDLE};
use windows::Win32::System::Power::{
    DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS, HPOWERNOTIFY, PowerRegisterSuspendResumeNotification,
    PowerUnregisterSuspendResumeNotification,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DEVICE_NOTIFY_CALLBACK, PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND, PBT_APMSUSPEND,
};

/// A change of the PC's power state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerEvent {
    /// The PC is about to sleep or hibernate
    Suspend,
    /// The PC woke up. Windows sends two notifications for one resume
    /// (PBT_APMRESUMEAUTOMATIC, and PBT_APMRESUMESUSPEND when a user is
    /// present); both arrive as `Resume`, and the caller keeps only the
    /// first, as v1 does.
    Resume,
}

type Callback = Box<dyn Fn(PowerEvent) + Send + Sync + 'static>;

/// A registration for sleep / resume notifications; unregisters on drop
pub struct PowerNotifier {
    handle: *mut c_void,
    // Boxed twice so the context pointer handed to Windows stays the same
    // wherever the PowerNotifier moves. Freed only after unregistering.
    callback: *mut Callback,
}

// SAFETY: the registration handle is process-wide (not tied to a thread)
// and may be unregistered from any thread; the callback it points to is
// Send + Sync.
unsafe impl Send for PowerNotifier {}
// SAFETY: PowerNotifier has no methods taking &self that touch its fields.
unsafe impl Sync for PowerNotifier {}

impl PowerNotifier {
    /// Registers `callback`. It runs on a thread of Windows' power manager,
    /// so it should return quickly (set a flag, send on a channel).
    pub fn new(callback: impl Fn(PowerEvent) + Send + Sync + 'static) -> windows::core::Result<PowerNotifier> {
        let callback: *mut Callback = Box::into_raw(Box::new(Box::new(callback)));
        let params = DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS { Callback: Some(on_power_event), Context: callback.cast() };
        let mut handle: *mut c_void = std::ptr::null_mut();
        // SAFETY: with DEVICE_NOTIFY_CALLBACK the recipient is a pointer to
        // the subscribe parameters, valid for the call; the context stays
        // valid until Drop has unregistered.
        let err = unsafe {
            PowerRegisterSuspendResumeNotification(
                DEVICE_NOTIFY_CALLBACK,
                HANDLE(std::ptr::from_ref(&params).cast_mut().cast()),
                &mut handle,
            )
        };
        if err != ERROR_SUCCESS {
            tracing::debug!("power: PowerRegisterSuspendResumeNotification failed (error {})", err.0);
            // SAFETY: registration failed, so Windows holds no copy of the
            // pointer; it came from Box::into_raw above.
            drop(unsafe { Box::from_raw(callback) });
            return Err(err.into());
        }
        tracing::debug!("power: sleep / resume notifications registered");
        Ok(PowerNotifier { handle, callback })
    }
}

impl Drop for PowerNotifier {
    fn drop(&mut self) {
        // SAFETY: `handle` came from a successful registration and is
        // unregistered only here.
        let err = unsafe { PowerUnregisterSuspendResumeNotification(HPOWERNOTIFY(self.handle as isize)) };
        if err != ERROR_SUCCESS {
            // A callback might still come: leak the closure rather than
            // free memory Windows may call into
            tracing::warn!("power: unregistering sleep / resume notifications failed ({err:?})");
            return;
        }
        tracing::debug!("power: sleep / resume notifications unregistered");
        // SAFETY: unregistered, so Windows no longer calls with this
        // context (the unregister call waits for a running callback, which
        // v1 relied on as well); the pointer came from Box::into_raw.
        drop(unsafe { Box::from_raw(self.callback) });
    }
}

unsafe extern "system" fn on_power_event(context: *const c_void, kind: u32, _setting: *const c_void) -> u32 {
    let event = match kind {
        PBT_APMSUSPEND => PowerEvent::Suspend,
        PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND => PowerEvent::Resume,
        _ => {
            tracing::debug!("power: notification 0x{kind:04X} ignored");
            return 0;
        }
    };
    tracing::debug!("power: notification 0x{kind:04X} -> {event:?}");
    // SAFETY: the context is the Callback box registered in
    // PowerNotifier::new, alive until it is unregistered.
    let callback = unsafe { &*context.cast::<Callback>() };
    // A panic must not unwind into Windows (it would abort the process)
    if catch_unwind(AssertUnwindSafe(|| callback(event))).is_err() {
        tracing::error!("power: the sleep / resume callback panicked");
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_and_unregisters() {
        let notifier = PowerNotifier::new(|_| {}).expect("register");
        // Moving it must not invalidate the registered context
        let moved = std::thread::spawn(move || notifier).join().unwrap();
        drop(moved);
    }
}
