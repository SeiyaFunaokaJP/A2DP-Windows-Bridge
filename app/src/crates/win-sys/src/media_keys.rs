//! Media keys sent to Windows, for the headphones' buttons (AVRCP)
//!
//! A press and release of the multimedia key goes through SendInput, as v1
//! btstack_transport.cpp send_media_key does: whatever player owns the
//! media session (or listens for the keys) reacts as to a keyboard key.

use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, SendInput,
    VIRTUAL_KEY, VK_MEDIA_NEXT_TRACK, VK_MEDIA_PLAY_PAUSE, VK_MEDIA_PREV_TRACK, VK_MEDIA_STOP,
};

/// A multimedia key
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKey {
    /// Windows has one Play/Pause key; headphones send PLAY or PAUSE by
    /// what they believe the state is, so the caller maps both here
    PlayPause,
    Stop,
    Next,
    Previous,
}

impl MediaKey {
    fn virtual_key(self) -> VIRTUAL_KEY {
        match self {
            MediaKey::PlayPause => VK_MEDIA_PLAY_PAUSE,
            MediaKey::Stop => VK_MEDIA_STOP,
            MediaKey::Next => VK_MEDIA_NEXT_TRACK,
            MediaKey::Previous => VK_MEDIA_PREV_TRACK,
        }
    }
}

/// Sends a press and a release of `key`. Fails when the input was blocked
/// (another desktop is active, e.g. the lock screen or a UAC prompt, or a
/// higher-integrity window has the focus).
pub fn send(key: MediaKey) -> windows::core::Result<()> {
    let input = |flags: KEYBD_EVENT_FLAGS| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: key.virtual_key(), dwFlags: flags, ..Default::default() } },
    };
    let inputs = [input(KEYEVENTF_EXTENDEDKEY), input(KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYUP)];
    // SAFETY: `inputs` is a valid slice of keyboard INPUTs and the size
    // passed is that of one INPUT, as SendInput requires.
    let sent = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
    if sent as usize != inputs.len() {
        let e = windows::core::Error::from_thread();
        tracing::debug!("media_keys: {key:?}: SendInput sent {sent} of {} ({e})", inputs.len());
        return Err(e);
    }
    tracing::debug!("media_keys: {key:?} sent");
    Ok(())
}
