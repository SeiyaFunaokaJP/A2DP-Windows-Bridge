//! Builds liblc3 (Google, Apache-2.0) as a static library with the compiler
//! found by `cc`, from extern/btstack/3rd-party/lc3-google/src (the copy
//! BTstack ships, which v1 built the same way in its CMakeLists.txt: every
//! src/*.c, `_USE_MATH_DEFINES`, /W1 with MSVC). Only liblc3 itself: not
//! BTstack's btstack_lc3_google.c wrapper. Unlike v1, src/lc3.c is compiled
//! through a small generated wrapper that drops liblc3's dllexport.
//!
//! The source directory can be overridden with LC3_DIR (the directory that
//! holds include/lc3.h and src/lc3.c). liblc3's NEON paths (ltpf_neon.h,
//! mdct_neon.h) are taken only where `__ARM_NEON` and `__ARM_ARCH_ISA_A64`
//! are defined (GCC / Clang); otherwise it builds its portable C.

use std::env;
use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let dir = env::var_os("LC3_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("../../../../extern/btstack/3rd-party/lc3-google"));
    // std::path::absolute, not canonicalize: MSVC does not take \\?\ paths
    let dir = std::path::absolute(&dir).unwrap();
    assert!(
        dir.join("include/lc3.h").exists() && dir.join("src/lc3.c").exists(),
        "liblc3 not found at {} (git submodule update --init extern/btstack)",
        dir.display()
    );

    let mut sources: Vec<PathBuf> = std::fs::read_dir(dir.join("src"))
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "c") && !p.ends_with("lc3.c"))
        .collect();
    sources.sort();
    assert!(!sources.is_empty(), "no liblc3 sources in {}", dir.join("src").display());
    // lc3.c (the API) through a wrapper: lc3_private.h makes LC3_EXPORT
    // __declspec(dllexport) on Windows, which would export the lc3_*
    // functions from every BlueCaster executable linking this static library
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let wrapper = out.join("lc3_static.c");
    std::fs::write(
        &wrapper,
        "/* liblc3's API as a static library: no dllexport (lc3-sys build.rs) */\n\
         #include <stdbool.h>\n\
         #include <stdint.h>\n\
         #include \"lc3_private.h\"\n\
         #undef LC3_EXPORT\n\
         #define LC3_EXPORT\n\
         #include \"lc3.c\"\n",
    )
    .unwrap();
    sources.push(wrapper);

    let mut build = cc::Build::new();
    build
        .files(&sources)
        .include(dir.join("include"))
        .include(dir.join("src"))
        // M_PI and friends from <math.h> (MSVC hides them without it)
        .define("_USE_MATH_DEFINES", None)
        .warnings(false)
        .cargo_warnings(false)
        // The encoder runs in real time on the LE encode thread: keep it
        // optimized in debug builds of BlueCaster too
        .opt_level(2);
    if build.get_compiler().is_like_msvc() {
        // stdalign.h (common.h) needs C11
        build.flag("/std:c11").flag("/W1").flag("/utf-8");
        // MSVC on ARM64 defines __ARM_ARCH, for which common.h includes
        // <arm_acle.h>, a header MSVC does not have. Nothing from it is used
        // there (__ARM_FEATURE_SAT / _SIMD32 are GCC / Clang only): an empty one
        if env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("aarch64") {
            let shim = out.join("msvc-arm64");
            std::fs::create_dir_all(&shim).unwrap();
            std::fs::write(shim.join("arm_acle.h"), "/* empty: see lc3-sys build.rs */\n").unwrap();
            build.include(shim);
        }
    } else {
        build.std("c11").flag("-w").flag("-ffast-math");
    }
    build.compile("lc3");

    println!("cargo:rerun-if-env-changed=LC3_DIR");
    for d in ["include", "src"] {
        println!("cargo:rerun-if-changed={}", dir.join(d).display());
    }
    // For dependents that need the headers (DEP_LC3_INCLUDE)
    println!("cargo:include={}", dir.join("include").display());
}
