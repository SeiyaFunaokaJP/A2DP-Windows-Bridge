//! Builds libopenaptx (extern/libopenaptx/openaptx.c) as its own static
//! library with the MSVC compiler found by `cc`.
//!
//! The source comes from the git submodule extern/libopenaptx of the
//! repository (override with the OPENAPTX_DIR environment variable). It must
//! be release 0.2.0: 0.2.1 and later are GPL-3.0-or-later, which the other
//! libraries linked into BlueCaster are not compatible with (THIRD_PARTY_LICENSES.md),
//! so the build stops on any other version instead of linking it silently.

use std::env;
use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let dir = env::var_os("OPENAPTX_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("../../../../extern/libopenaptx"));
    // std::path::absolute, not canonicalize: MSVC does not take \\?\ paths
    let dir = std::path::absolute(&dir).unwrap();
    let source = dir.join("openaptx.c");
    let header = dir.join("openaptx.h");
    assert!(
        source.exists() && header.exists(),
        "libopenaptx not found at {} (git submodule update --init extern/libopenaptx)",
        dir.display()
    );

    // Releases up to v1.0.4 were mistakenly built with 0.2.1 (GPL-3.0): check
    // the version in the header before compiling anything
    let text = std::fs::read_to_string(&header).unwrap();
    let version = |name: &str| -> Option<u32> {
        text.lines()
            .filter_map(|l| l.trim().strip_prefix("#define "))
            .find_map(|l| l.strip_prefix(name))
            .and_then(|v| v.trim().parse().ok())
    };
    let found = (version("OPENAPTX_MAJOR"), version("OPENAPTX_MINOR"), version("OPENAPTX_PATCH"));
    assert!(
        found == (Some(0), Some(2), Some(0)),
        "libopenaptx at {} is {:?}, not 0.2.0: only 0.2.0 is LGPL-2.1-or-later (0.2.1+ is GPL-3.0-or-later)",
        dir.display(),
        found
    );

    cc::Build::new()
        .file(&source)
        // openaptx.c includes <openaptx.h>
        .include(&dir)
        .warnings(false)
        .flag_if_supported("/utf-8")
        .flag_if_supported("/W1")
        .compile("openaptx");

    println!("cargo:rerun-if-env-changed=OPENAPTX_DIR");
    println!("cargo:rerun-if-changed={}", source.display());
    println!("cargo:rerun-if-changed={}", header.display());
    // For dependents that need the header (DEP_OPENAPTX_ROOT)
    println!("cargo:root={}", dir.display());
}
