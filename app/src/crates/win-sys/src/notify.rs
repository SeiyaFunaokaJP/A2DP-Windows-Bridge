//! Balloon notifications from the resident core's tray icon (v1:
//! wxTaskBarIcon::ShowBalloon and wxEVT_TASKBAR_BALLOON_CLICK in
//! wx_main_frame.cpp: the "still running in the tray" hint and "update
//! available", whose click opened the release page)
//!
//! Windows 10 / 11 show a balloon as a toast (and keep it in the
//! notification centre), attributed to the program; no AppUserModelID,
//! Start-menu shortcut or COM activator is needed, which a WinRT toast from
//! an unpackaged program would require. Clicking it sends
//! NIN_BALLOONUSERCLICK to the icon's window.
//!
//! The icon is the one the `tray-icon` crate created: `TrayIcon::
//! window_handle()` gives its hidden window. Each tray-icon window holds
//! exactly one notification icon, whose uID is an internal counter value;
//! [`TrayNotifier`] finds it by trying uIDs (NIM_MODIFY fails for an icon
//! that does not exist) and remembers it. An icon built with
//! `TrayIconBuilder::with_guid` is addressed by that GUID instead
//! ([`TrayNotifier::with_guid`]).
//!
//! The click is seen by subclassing the tray-icon window (SetWindowSubclass),
//! which must happen on the thread that owns that window: the thread running
//! the tao event loop that built the tray icon. [`TrayNotifier`] is
//! therefore neither Send nor Sync, and the click action runs on that thread.
//! tray-icon itself ignores balloon notifications.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::marker::PhantomData;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Shell::{
    DefSubclassProc, NIF_GUID, NIF_INFO, NIIF_ERROR, NIIF_INFO, NIIF_NONE, NIIF_WARNING, NIM_MODIFY,
    NIN_BALLOONUSERCLICK, NOTIFY_ICON_INFOTIP_FLAGS, NOTIFYICONDATAW, RemoveWindowSubclass, SetWindowSubclass,
    Shell_NotifyIconW, ShellExecuteW,
};
use windows::Win32::UI::WindowsAndMessaging::{ASFW_ANY, AllowSetForegroundWindow, SW_SHOWNORMAL, WM_NCDESTROY};
use windows::core::{GUID, HSTRING, w};

/// The icon's callback message in tray-icon (`WM_USER_TRAYICON` in its
/// platform_impl/windows/mod.rs, 0.26): the shell posts balloon events with
/// it. Checked against tray-icon 0.26.1; a tray-icon update that changes it
/// stops click actions (not the balloons).
pub const TRAY_ICON_CALLBACK_MESSAGE: u32 = 6002;

/// uIDs tried when looking for the tray-icon's icon (tray-icon numbers
/// icons and ids from one process-wide counter starting at 1)
const UID_PROBE_LIMIT: u32 = 1024;

/// Our subclass of the tray-icon window
const SUBCLASS_ID: usize = 0xA2D9_0001;

/// The small icon in the balloon's title (the toast shows the program's
/// icon in any case)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BalloonIcon {
    None,
    #[default]
    Info,
    Warning,
    Error,
}

impl BalloonIcon {
    fn flags(self) -> NOTIFY_ICON_INFOTIP_FLAGS {
        match self {
            BalloonIcon::None => NIIF_NONE,
            BalloonIcon::Info => NIIF_INFO,
            BalloonIcon::Warning => NIIF_WARNING,
            BalloonIcon::Error => NIIF_ERROR,
        }
    }
}

/// What a click on the balloon (or on its toast in the notification
/// centre, while the core runs) does
pub enum ClickAction {
    /// Opens a URL (or a file) with its default program, as v1 opened the
    /// release page of an update
    OpenUrl(String),
    /// Runs on the tray thread, inside the window procedure: keep it short
    /// (e.g. send a tao user event)
    Run(Box<dyn FnOnce()>),
}

impl std::fmt::Debug for ClickAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClickAction::OpenUrl(u) => f.debug_tuple("OpenUrl").field(u).finish(),
            ClickAction::Run(_) => f.write_str("Run(..)"),
        }
    }
}

impl ClickAction {
    fn run(self) {
        match self {
            ClickAction::OpenUrl(url) => open_url(&url),
            ClickAction::Run(f) => f(),
        }
    }
}

/// A balloon to show
#[derive(Debug, Default)]
pub struct Balloon {
    /// Up to 63 UTF-16 units, longer is cut
    pub title: String,
    /// Up to 255 UTF-16 units, longer is cut
    pub text: String,
    pub icon: BalloonIcon,
    /// None: a click does nothing (and forgets the previous balloon's action)
    pub on_click: Option<ClickAction>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum NotifyError {
    #[error("the tray icon was not found (no taskbar, or the icon was removed)")]
    NoTrayIcon,
    #[error("cannot watch the tray icon's window for clicks")]
    Subclass,
}

thread_local! {
    /// Click action of the last balloon, per tray window (subclass proc
    /// and notifier run on the window's thread)
    static PENDING: RefCell<HashMap<isize, ClickAction>> = RefCell::new(HashMap::new());
}

/// Shows balloons from a tray-icon icon. Create and use it on the tray's
/// thread (the tao event loop's); see the module documentation.
///
/// ```ignore
/// // resident.rs, after TrayIconBuilder::build():
/// let notifier = win_sys::notify::TrayNotifier::new(tray.window_handle() as isize);
/// notifier.show(Balloon { title, text, on_click: Some(ClickAction::OpenUrl(url)), ..Default::default() })?;
/// ```
#[derive(Debug)]
pub struct TrayNotifier {
    hwnd: isize,
    guid: Option<GUID>,
    uid: Cell<Option<u32>>,
    subclassed: Cell<bool>,
    /// Bound to the window's thread
    _thread: PhantomData<*const ()>,
}

impl TrayNotifier {
    /// For the icon of the tray-icon window `tray_hwnd`
    /// (`TrayIcon::window_handle() as isize`)
    pub fn new(tray_hwnd: isize) -> TrayNotifier {
        TrayNotifier {
            hwnd: tray_hwnd,
            guid: None,
            uid: Cell::new(None),
            subclassed: Cell::new(false),
            _thread: PhantomData,
        }
    }

    /// For an icon built with `TrayIconBuilder::with_guid(guid)`
    pub fn with_guid(tray_hwnd: isize, guid: u128) -> TrayNotifier {
        TrayNotifier { guid: Some(GUID::from_u128(guid)), ..TrayNotifier::new(tray_hwnd) }
    }

    fn hwnd(&self) -> HWND {
        HWND(self.hwnd as *mut core::ffi::c_void)
    }

    /// Shows `balloon` (replacing one still shown) and arms its click
    /// action. Windows may queue or drop it (Focus assist, quiet hours,
    /// notifications turned off for the program); that is not an error.
    pub fn show(&self, balloon: Balloon) -> Result<(), NotifyError> {
        let mut nid = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd(),
            uFlags: NIF_INFO,
            dwInfoFlags: balloon.icon.flags(),
            ..Default::default()
        };
        copy_wide(&mut nid.szInfoTitle, &balloon.title);
        copy_wide(&mut nid.szInfo, &balloon.text);
        if let Some(guid) = self.guid {
            nid.uFlags |= NIF_GUID;
            nid.guidItem = guid;
            if !modify(&nid) {
                tracing::debug!("notify: no tray icon with GUID {guid:?} on window 0x{:X}", self.hwnd);
                return Err(NotifyError::NoTrayIcon);
            }
        } else {
            let known = self.uid.get();
            let shown = known.is_some_and(|uid| {
                nid.uID = uid;
                modify(&nid)
            }) || (0..=UID_PROBE_LIMIT).filter(|&uid| Some(uid) != known).any(|uid| {
                nid.uID = uid;
                let ok = modify(&nid);
                if ok {
                    self.uid.set(Some(uid));
                }
                ok
            });
            if !shown {
                tracing::debug!("notify: no tray icon on window 0x{:X} (uIDs 0..={UID_PROBE_LIMIT})", self.hwnd);
                return Err(NotifyError::NoTrayIcon);
            }
        }
        tracing::debug!(
            "notify: balloon {:?} shown (uID {:?}, GUID {}, click action {})",
            balloon.title,
            self.uid.get(),
            self.guid.is_some(),
            balloon.on_click.is_some()
        );
        self.arm(balloon.on_click)
    }

    /// Sets the action of the next click, subclassing the window once
    fn arm(&self, action: Option<ClickAction>) -> Result<(), NotifyError> {
        if action.is_some() && !self.subclassed.get() {
            // SAFETY: called on the window's thread (the notifier is not
            // Send); the procedure is a plain function, removed again when
            // the window is destroyed.
            if !unsafe { SetWindowSubclass(self.hwnd(), Some(subclass_proc), SUBCLASS_ID, 0) }.as_bool() {
                tracing::debug!("notify: SetWindowSubclass on window 0x{:X} failed", self.hwnd);
                return Err(NotifyError::Subclass);
            }
            self.subclassed.set(true);
        }
        PENDING.with(|p| {
            let mut p = p.borrow_mut();
            match action {
                Some(a) => p.insert(self.hwnd, a),
                None => p.remove(&self.hwnd),
            }
        });
        Ok(())
    }
}

fn modify(nid: &NOTIFYICONDATAW) -> bool {
    // SAFETY: a fully initialised NOTIFYICONDATAW with its cbSize.
    unsafe { Shell_NotifyIconW(NIM_MODIFY, nid) }.as_bool()
}

/// Copies `s` into a NUL-terminated UTF-16 buffer, cutting what does not
/// fit without splitting a surrogate pair
fn copy_wide(dst: &mut [u16], s: &str) {
    let Some(max) = dst.len().checked_sub(1) else { return };
    let mut n = 0;
    for c in s.chars() {
        let mut buf = [0u16; 2];
        let units = c.encode_utf16(&mut buf);
        if n + units.len() > max {
            break;
        }
        dst[n..n + units.len()].copy_from_slice(units);
        n += units.len();
    }
    dst[n] = 0;
}

/// A balloon click in the icon's callback message: lParam is the event
/// (legacy notify-icon version, which tray-icon uses) or has it in its low
/// word (NOTIFYICON_VERSION_4)
fn is_balloon_click(lparam: isize) -> bool {
    (lparam as u32 & 0xFFFF) == NIN_BALLOONUSERCLICK
}

unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    if msg == TRAY_ICON_CALLBACK_MESSAGE && is_balloon_click(lparam.0) {
        // Taken out before running: the action may show another balloon
        let action = PENDING.with(|p| p.borrow_mut().remove(&(hwnd.0 as isize)));
        tracing::debug!(
            "notify: balloon clicked ({})",
            if action.is_some() { "running its action" } else { "no action" }
        );
        if let Some(action) = action {
            action.run();
        }
    } else if msg == WM_NCDESTROY {
        PENDING.with(|p| p.borrow_mut().remove(&(hwnd.0 as isize)));
        // SAFETY: on the window's own thread, removing our own subclass.
        let _ = unsafe { RemoveWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID) };
    }
    // SAFETY: forwards the message unchanged to the next procedure.
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

/// After a click on the tray icon: the shell lets the icon's process take
/// the foreground, and this passes that on, so that the UI process the
/// click opens (or the running one it forwards to) can bring its window to
/// the front (#140)
pub fn allow_foreground_for_click() {
    // SAFETY: plain call without pointers
    if let Err(e) = unsafe { AllowSetForegroundWindow(ASFW_ANY) } {
        tracing::debug!("AllowSetForegroundWindow: {e}");
    }
}

/// Opens `url` with its default program (the browser for http/https)
pub fn open_url(url: &str) {
    // SAFETY: NUL-terminated strings that live for the call.
    let r = unsafe { ShellExecuteW(None, w!("open"), &HSTRING::from(url), None, None, SW_SHOWNORMAL) };
    // ShellExecute reports success with a value above 32
    if r.0 as usize <= 32 {
        tracing::warn!("cannot open {url} (ShellExecute {})", r.0 as usize);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, HWND_MESSAGE, SendMessageW, WINDOW_EX_STYLE, WINDOW_STYLE,
    };

    #[test]
    fn wide_copy_cuts_and_terminates() {
        let mut b = [0xFFFFu16; 6];
        copy_wide(&mut b, "abc");
        assert_eq!(&b[..4], &[97, 98, 99, 0]);
        copy_wide(&mut b, "abcdefgh");
        assert_eq!(b, [97, 98, 99, 100, 101, 0]);
        // A surrogate pair that would not fit whole is left out
        let mut b = [0xFFFFu16; 6];
        copy_wide(&mut b, "abcd\u{1F3A7}");
        assert_eq!(b, [97, 98, 99, 100, 0, 0xFFFF]);
        let mut b = [0u16; 0];
        copy_wide(&mut b, "x");
    }

    #[test]
    fn click_events() {
        assert!(is_balloon_click(NIN_BALLOONUSERCLICK as isize));
        // NOTIFYICON_VERSION_4: icon id in the high word
        assert!(is_balloon_click(((7 << 16) | NIN_BALLOONUSERCLICK) as isize));
        assert!(!is_balloon_click(0x0201)); // WM_LBUTTONDOWN
        assert!(!is_balloon_click(1028)); // NIN_BALLOONTIMEOUT
    }

    /// A message-only window standing in for tray-icon's
    struct TestWindow(HWND);

    impl TestWindow {
        fn new() -> TestWindow {
            // SAFETY: the system "STATIC" class, message-only, on this thread
            let hwnd = unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    w!("STATIC"),
                    w!("a2dpwb notify test"),
                    WINDOW_STYLE(0),
                    0,
                    0,
                    0,
                    0,
                    Some(HWND_MESSAGE),
                    None,
                    None,
                    None,
                )
            }
            .expect("test window");
            TestWindow(hwnd)
        }

        fn send(&self, msg: u32, lparam: isize) {
            // SAFETY: our window, on its thread
            unsafe { SendMessageW(self.0, msg, Some(WPARAM(2)), Some(LPARAM(lparam))) };
        }
    }

    impl Drop for TestWindow {
        fn drop(&mut self) {
            // SAFETY: created by this test on this thread
            let _ = unsafe { DestroyWindow(self.0) };
        }
    }

    #[test]
    fn window_without_icon_is_reported() {
        let w = TestWindow::new();
        let n = TrayNotifier::new(w.0.0 as isize);
        let r = n.show(Balloon {
            title: "t".into(),
            text: "x".into(),
            on_click: Some(ClickAction::OpenUrl("https://example.invalid/".into())),
            ..Default::default()
        });
        assert_eq!(r, Err(NotifyError::NoTrayIcon));
        // Nothing armed
        assert!(PENDING.with(|p| p.borrow().is_empty()));
    }

    #[test]
    fn click_runs_the_action_once() {
        let w = TestWindow::new();
        let n = TrayNotifier::new(w.0.0 as isize);
        let clicks = Rc::new(Cell::new(0));
        let c = clicks.clone();
        n.arm(Some(ClickAction::Run(Box::new(move || c.set(c.get() + 1))))).unwrap();
        // Other tray events and other messages leave it armed
        w.send(TRAY_ICON_CALLBACK_MESSAGE, 0x0202);
        w.send(TRAY_ICON_CALLBACK_MESSAGE, 1028);
        w.send(0x0400 + 5, NIN_BALLOONUSERCLICK as isize);
        assert_eq!(clicks.get(), 0);
        w.send(TRAY_ICON_CALLBACK_MESSAGE, NIN_BALLOONUSERCLICK as isize);
        assert_eq!(clicks.get(), 1);
        w.send(TRAY_ICON_CALLBACK_MESSAGE, NIN_BALLOONUSERCLICK as isize);
        assert_eq!(clicks.get(), 1);

        // A balloon without an action disarms the previous one
        let c = clicks.clone();
        n.arm(Some(ClickAction::Run(Box::new(move || c.set(c.get() + 10))))).unwrap();
        n.arm(None).unwrap();
        w.send(TRAY_ICON_CALLBACK_MESSAGE, NIN_BALLOONUSERCLICK as isize);
        assert_eq!(clicks.get(), 1);

        // Destroying the window drops what is armed
        let c = clicks.clone();
        n.arm(Some(ClickAction::Run(Box::new(move || c.set(c.get() + 100))))).unwrap();
        let key = w.0.0 as isize;
        drop(w);
        assert!(PENDING.with(|p| !p.borrow().contains_key(&key)));
        assert_eq!(clicks.get(), 1);
    }
}
