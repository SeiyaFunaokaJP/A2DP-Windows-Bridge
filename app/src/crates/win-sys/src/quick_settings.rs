//! Windows 11's Quick Settings (the volume button's panels): when they open
//! and where they are, for the flyout to stand above them.
//!
//! No API covers them. Their window has a class of its own and is the
//! foreground window as they open (the input method can take that over while
//! they stay open), but it is a column as tall as the screen, mostly
//! transparent; where the panels are comes from UI Automation.

use std::cell::Cell;
use std::mem::ManuallyDrop;
use std::sync::OnceLock;

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::System::Variant::{VARIANT, VT_BSTR, VariantClear};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, HWINEVENTHOOK, IUIAutomation, IUIAutomationCondition, SetWinEventHook, TreeScope_Descendants,
    UIA_AutomationIdPropertyId,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EVENT_SYSTEM_FOREGROUND, GetClassNameW, GetForegroundWindow, GetWindowRect, IsWindowVisible, WINEVENT_OUTOFCONTEXT,
    WINEVENT_SKIPOWNPROCESS,
};
use windows::core::BSTR;

/// The window's class, the same in every language
const WINDOW_CLASS: &str = "ControlCenterWindow";
/// The panels (automation ids, the same in every language): the media card,
/// shown while something plays, and the one with the tiles and the volume
const PANELS: [&str; 2] = ["MediaTransportControls", "ControlCenterRegion"];

static ON_OPEN: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

fn is_quick_settings(hwnd: HWND) -> bool {
    let mut name = [0u16; 32];
    // SAFETY: a buffer of its length; an invalid window gives 0
    let len = unsafe { GetClassNameW(hwnd, &mut name) } as usize;
    String::from_utf16_lossy(&name[..len]) == WINDOW_CLASS
}

unsafe extern "system" fn on_foreground(_: HWINEVENTHOOK, _: u32, hwnd: HWND, _: i32, _: i32, _: u32, _: u32) {
    if is_quick_settings(hwnd)
        && let Some(on_open) = ON_OPEN.get()
    {
        on_open();
    }
}

/// Calls `on_open` each time Quick Settings open, on this thread: it must
/// run a message loop. Once per process; it lasts until the process ends.
pub fn watch_opened(on_open: impl Fn() + Send + Sync + 'static) {
    if ON_OPEN.set(Box::new(on_open)).is_err() {
        return;
    }
    // SAFETY: the callback is a function of this program, called on this
    // thread's message loop (OUTOFCONTEXT); it needs no module
    let hook = unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None,
            Some(on_foreground),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        )
    };
    if hook.is_invalid() {
        tracing::warn!("quick_settings: no foreground hook");
    }
}

/// Where the panels are (physical pixels, for a DPI-aware process)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Panels {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// Reads the panels of the Quick Settings open now; keeps its COM objects,
/// so one thread can read often. That thread needs COM initialized.
pub struct Reader {
    automation: IUIAutomation,
    panels: IUIAutomationCondition,
    window: Cell<Option<HWND>>,
}

impl Reader {
    pub fn new() -> windows::core::Result<Self> {
        // SAFETY: the class and interface belong together
        let automation: IUIAutomation = unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)? };
        let id = |automation: &IUIAutomation, id: &str| -> windows::core::Result<IUIAutomationCondition> {
            let mut value = VARIANT::default();
            // SAFETY: tagging the VARIANT as the BSTR it now holds, which
            // VariantClear frees
            unsafe {
                let v = &mut *value.Anonymous.Anonymous;
                v.vt = VT_BSTR;
                v.Anonymous.bstrVal = ManuallyDrop::new(BSTR::from(id));
                let condition = automation.CreatePropertyCondition(UIA_AutomationIdPropertyId, &value);
                let _ = VariantClear(&mut value);
                condition
            }
        };
        let (a, b) = (id(&automation, PANELS[0])?, id(&automation, PANELS[1])?);
        // SAFETY: two valid conditions
        let panels = unsafe { automation.CreateOrCondition(&a, &b)? };
        Ok(Self { automation, panels, window: Cell::new(None) })
    }

    /// The panels, None when Quick Settings are closed. If UI Automation
    /// does not find them, the whole window.
    pub fn read(&self) -> Option<Panels> {
        let hwnd = self.window.get().or_else(|| {
            // SAFETY: no arguments; the window is only looked at
            Some(unsafe { GetForegroundWindow() }).filter(|h| is_quick_settings(*h))
        })?;
        self.window.set(Some(hwnd));
        // SAFETY: as above; a window that is gone is not visible
        if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
            return None;
        }
        self.bounds(hwnd).or_else(|| {
            let mut r = RECT::default();
            // SAFETY: a RECT to fill in
            unsafe { GetWindowRect(hwnd, &mut r) }.ok()?;
            Some(Panels { left: r.left, top: r.top, right: r.right, bottom: r.bottom })
        })
    }

    fn bounds(&self, hwnd: HWND) -> Option<Panels> {
        // SAFETY: COM calls on objects this struct holds or that they return
        let rects = unsafe {
            let found =
                self.automation.ElementFromHandle(hwnd).ok()?.FindAll(TreeScope_Descendants, &self.panels).ok()?;
            let mut rects = Vec::new();
            for i in 0..found.Length().ok()? {
                rects.push(found.GetElement(i).ok()?.CurrentBoundingRectangle().ok()?);
            }
            rects
        };
        union(&rects)
    }
}

/// The smallest rectangle around the non-empty `rects`
fn union(rects: &[RECT]) -> Option<Panels> {
    rects.iter().filter(|r| r.right > r.left && r.bottom > r.top).fold(None, |all, r| {
        Some(match all {
            None => Panels { left: r.left, top: r.top, right: r.right, bottom: r.bottom },
            Some(a) => Panels {
                left: a.left.min(r.left),
                top: a.top.min(r.top),
                right: a.right.max(r.right),
                bottom: a.bottom.max(r.bottom),
            },
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn union_takes_both_panels_and_skips_empty_ones() {
        // As measured: the media card above the tiles
        let media = RECT { left: 3282, top: 1281, right: 3822, bottom: 1548 };
        let tiles = RECT { left: 3282, top: 1566, right: 3822, bottom: 2070 };
        let empty = RECT::default();
        assert_eq!(union(&[tiles, empty, media]), Some(Panels { left: 3282, top: 1281, right: 3822, bottom: 2070 }));
        assert_eq!(union(&[tiles]), Some(Panels { left: 3282, top: 1566, right: 3822, bottom: 2070 }));
        assert_eq!(union(&[empty]), None);
    }
}
