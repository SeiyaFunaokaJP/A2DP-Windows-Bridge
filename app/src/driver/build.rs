//! Embeds the version information (no manifest: see main.rs, Elevation)

include!("../../resources/version_rc.rs");

fn main() {
    let rc = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("version.rc");
    std::fs::write(&rc, version_rc("bluecaster-driver.exe", "BlueCaster driver helper")).unwrap();
    embed_resource::compile(&rc, embed_resource::NONE).manifest_optional().unwrap();
    println!("cargo:rerun-if-changed=../../resources/version_rc.rs");
}
