//! Embeds the app icon (ID 101, also the tray icon; drawn by
//! app/resources/make_icon.py), the manifest (per-monitor DPI awareness,
//! Windows 10/11) and the version information. Its FileDescription is the
//! name Windows heads the tray's notifications with: the app's own name, not
//! "... core" (issue #86)

include!("../../resources/version_rc.rs");

fn main() {
    embed_resource::compile("resources/core.rc", embed_resource::NONE).manifest_required().unwrap();
    let rc = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("version.rc");
    std::fs::write(&rc, version_rc("bluecaster-core.exe", "BlueCaster")).unwrap();
    embed_resource::compile(&rc, embed_resource::NONE).manifest_optional().unwrap();
    println!("cargo:rerun-if-changed=resources/core.rc");
    println!("cargo:rerun-if-changed=../../resources/version_rc.rs");
}
