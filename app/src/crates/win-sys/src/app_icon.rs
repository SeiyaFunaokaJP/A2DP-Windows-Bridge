//! The app icon at the sizes Windows draws it (issue #86). Given one image,
//! as tray-icon's default and Tauri's window icon do, Windows takes the
//! large one and shrinks it: the fine lines blur into a blue square. Loaded
//! at the size it is drawn at, the .ico's own image for that size is used
//! (app/resources/make_icon.py: redrawn at 16 and 20 px).

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{GetDpiForSystem, GetDpiForWindow, GetSystemMetricsForDpi};
use windows::Win32::UI::WindowsAndMessaging::{
    ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_SHARED, LoadImageW, SM_CXICON, SM_CXSMICON, SYSTEM_METRICS_INDEX,
    SendMessageW, WM_SETICON,
};
use windows::core::PCWSTR;

fn metric(index: SYSTEM_METRICS_INDEX, dpi: u32, least: u32) -> u32 {
    // SAFETY: a plain query without pointers
    let size = unsafe { GetSystemMetricsForDpi(index, dpi) };
    u32::try_from(size).ok().filter(|&s| s >= least).unwrap_or(least)
}

/// The size the taskbar shows a tray icon at: the small icon size at the
/// system DPI (16 px at 100 %, 20 at 125 %, 24 at 150 %)
pub fn tray_icon_size() -> u32 {
    // SAFETY: a plain query
    metric(SM_CXSMICON, unsafe { GetDpiForSystem() }, 16)
}

/// Sets a window's title bar (small) and taskbar (big) icon to this
/// program's icon resource `id` at the window's DPI; again after the window
/// moved to a screen of another scale. False when the icon is not there.
pub fn set_window_icons(hwnd: isize, id: u16) -> bool {
    let hwnd = HWND(hwnd as *mut _);
    // SAFETY: GetDpiForWindow takes any handle (0 for one that is not a window)
    let dpi = match unsafe { GetDpiForWindow(hwnd) } {
        0 => return false,
        d => d,
    };
    // SAFETY: the module of this process, which stays loaded
    let Ok(module) = (unsafe { GetModuleHandleW(None) }) else {
        return false;
    };
    let mut done = true;
    for (which, index, least) in [(ICON_SMALL, SM_CXSMICON, 16), (ICON_BIG, SM_CXICON, 32)] {
        let size = metric(index, dpi, least) as i32;
        // SAFETY: an integer resource ID (MAKEINTRESOURCE); LR_SHARED icons
        // are owned by the system, so the window may keep using them
        let icon = unsafe {
            LoadImageW(Some(module.into()), PCWSTR(id as usize as *const u16), IMAGE_ICON, size, size, LR_SHARED)
        };
        match icon {
            // SAFETY: WM_SETICON with an icon handle
            Ok(h) => unsafe {
                SendMessageW(hwnd, WM_SETICON, Some(WPARAM(which as usize)), Some(LPARAM(h.0 as isize)));
            },
            Err(_) => done = false,
        }
    }
    done
}
