//! Keeping the DLLs of WinRT classes loaded for the process' life.
//!
//! windows-rs caches a class's activation factory for the whole process
//! (an agile factory, never released). COM may still unload the DLL that
//! implements it: WRL-based DLLs do not count their factories as objects
//! in use, and COM frees idle libraries as apartments come and go. The
//! next call through the cache then reads the vtable of an unloaded DLL
//! (bluecaster-core crashed twice on 2026-10-22 in
//! GlobalSystemMediaTransportControlsSessionManager::RequestAsync, with
//! Windows.Media.MediaControl.dll in the unloaded-module list).
//!
//! Pinning one DLL is not always enough: Windows.Devices.Radios.dll keeps
//! the radio managers it loads (WlanRadioManager.dll, BthRadioMedia.dll)
//! for the process, and COM unloads them with every other DLL when the
//! last thread in the multithreaded apartment leaves it. [`keep_mta`] keeps
//! that apartment, and with it those DLLs, for the process' life.

use core::ffi::c_void;
use std::sync::Once;

use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::Com::CoIncrementMTAUsage;
use windows::Win32::System::LibraryLoader::{
    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_PIN, GetModuleFileNameW, GetModuleHandleExW,
};
use windows::core::{Interface, PCWSTR};

/// Keeps the multithreaded apartment for the process' life, so that COM
/// never tears it down and unloads the DLLs loaded in it. bluecaster-core
/// crashed twice on 2026-11-02 in Radio::GetRadiosAsync, through the cached
/// factory of an unloaded Windows.Devices.Radios.dll: a thread that had
/// joined the apartment (ComApartment) left it last
pub fn keep_mta() {
    static KEEP: Once = Once::new();
    KEEP.call_once(|| {
        // SAFETY: plain call; the usage it adds is never released
        match unsafe { CoIncrementMTAUsage() } {
            Ok(_) => tracing::debug!("com: the multithreaded apartment is kept for the process' life"),
            Err(e) => tracing::warn!("com: cannot keep the multithreaded apartment: {e}"),
        }
    });
}

/// Pins the DLL that implements `object` (the module its vtable is in), so
/// it stays loaded until the process ends; its file name, None if the
/// module could not be found
pub fn pin_module_of<I: Interface>(object: &I) -> Option<String> {
    // SAFETY: a COM interface pointer points at its vtable pointer
    let vtable = unsafe { *(object.as_raw() as *const *const c_void) };
    let mut module = HMODULE::default();
    // SAFETY: FROM_ADDRESS takes any address inside a module as the name;
    // PIN keeps no handle to release
    unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
            PCWSTR(vtable as *const u16),
            &mut module,
        )
    }
    .ok()?;
    let mut name = [0u16; 260];
    // SAFETY: a module handle of this process and a buffer of its length
    let len = unsafe { GetModuleFileNameW(Some(module), &mut name) } as usize;
    let path = String::from_utf16_lossy(&name[..len]);
    Some(path.rsplit('\\').next().unwrap_or_default().to_string())
}
