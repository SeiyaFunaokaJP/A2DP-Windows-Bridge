//! Embeds the app icon (ID 101, also the tray icon; drawn by
//! app/resources/make_icon.py), the manifest (per-monitor DPI awareness,
//! Windows 10/11) and the version information. Its FileDescription is the
//! name Windows heads the tray's notifications with: the app's own name, not
//! "... core" (issue #86)
//!
//! wlanapi.dll is delay-loaded: Windows Server without the Wireless LAN
//! Service and trimmed Windows images lack it, and a load-time import would
//! keep the process from starting at all. win-sys::wlan checks for the DLL
//! before its first call.

include!("../../resources/version_rc.rs");

fn main() {
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bins=/DELAYLOAD:wlanapi.dll");
        println!("cargo:rustc-link-arg-bins=delayimp.lib");
    }
    embed_resource::compile("resources/core.rc", embed_resource::NONE).manifest_required().unwrap();
    let rc = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("version.rc");
    std::fs::write(&rc, version_rc("a2dpwb-core.exe", "A2DP Windows Bridge")).unwrap();
    embed_resource::compile(&rc, embed_resource::NONE).manifest_optional().unwrap();
    println!("cargo:rerun-if-changed=resources/core.rc");
    println!("cargo:rerun-if-changed=../../resources/version_rc.rs");
}
