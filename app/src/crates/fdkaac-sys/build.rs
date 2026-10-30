//! Builds the Fraunhofer FDK AAC encoder (and, with the `decoder` feature,
//! the decoder) as a static library with the MSVC compiler found by `cc`.
//!
//! The sources come from the git submodule extern/fdk-aac of the repository
//! (override with the FDK_AAC_DIR environment variable). The source lists and
//! include directories are read from its CMakeLists.txt, the build v1 used
//! (add_subdirectory), so they follow the submodule when it is updated.
//!
//! csrc/layout.c is built into a second small library: it reports sizeof /
//! offsetof of the structs declared by hand in src/ffi.rs, which the unit
//! tests compare with the Rust layout.

use std::env;
use std::path::{Path, PathBuf};

/// CMakeLists.txt source lists of the encoder and of what it calls:
/// transport encoder (LATM), SBR / MPS encoders (aacenc_lib.cpp references
/// them even for AAC-LC), PCM utilities, FDK tools and the system library
const ENCODER_LISTS: &[&str] =
    &["AACENC_SRC", "MPEGTPENC_SRC", "SBRENC_SRC", "SACENC_SRC", "PCMUTILS_SRC", "FDK_SRC", "SYS_SRC"];

/// Added by the `decoder` feature
const DECODER_LISTS: &[&str] =
    &["AACDEC_SRC", "ARITHCODING_SRC", "DRCDEC_SRC", "MPEGTPDEC_SRC", "SACDEC_SRC", "SBRDEC_SRC"];

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let fdk =
        env::var_os("FDK_AAC_DIR").map(PathBuf::from).unwrap_or_else(|| manifest.join("../../../../extern/fdk-aac"));
    // std::path::absolute, not canonicalize: MSVC does not take \\?\ paths
    let fdk = std::path::absolute(&fdk).unwrap();
    let cmake = fdk.join("CMakeLists.txt");
    assert!(
        cmake.exists() && fdk.join("libAACenc/include/aacenc_lib.h").exists(),
        "fdk-aac not found at {} (git submodule update --init extern/fdk-aac)",
        fdk.display()
    );
    let cmake_text = std::fs::read_to_string(&cmake).unwrap();
    let includes = include_dirs(&cmake_text);
    assert!(!includes.is_empty(), "no include directories found in {}", cmake.display());

    let decoder = env::var_os("CARGO_FEATURE_DECODER").is_some();
    let mut lists = ENCODER_LISTS.to_vec();
    if decoder {
        lists.extend_from_slice(DECODER_LISTS);
    }
    let mut sources = Vec::new();
    for list in lists {
        let files = cmake_list(&cmake_text, list);
        assert!(!files.is_empty(), "{list} not found in {}", cmake.display());
        sources.extend(files.into_iter().map(|f| fdk.join(f)));
    }

    let mut build = cc::Build::new();
    build
        .cpp(true)
        .includes(includes.iter().map(|i| fdk.join(i)))
        // As fdk-aac's CMakeLists.txt does for MSVC
        .flag_if_supported("/EHsc")
        .flag_if_supported("/utf-8")
        .warnings(false)
        .flag_if_supported("/w")
        // The encoder runs in real time on the encode thread: keep it
        // optimized in debug builds of BlueCaster too
        .opt_level(2);
    compile_parallel(&build, &sources, "fdkaac");

    // sizeof / offsetof of the hand-written structs (C, not C++)
    cc::Build::new()
        .file(manifest.join("csrc/layout.c"))
        .includes(includes.iter().map(|i| fdk.join(i)))
        .flag_if_supported("/utf-8")
        .warnings(false)
        .compile("fdkaac_layout");

    println!("cargo:rerun-if-env-changed=FDK_AAC_DIR");
    println!("cargo:rerun-if-changed={}", cmake.display());
    println!("cargo:rerun-if-changed=csrc");
    for dir in ["libAACenc/include", "libAACdec/include", "libSYS/include"] {
        println!("cargo:rerun-if-changed={}", fdk.join(dir).display());
    }
    // For dependents that need the headers (DEP_FDKAAC_ROOT)
    println!("cargo:root={}", fdk.display());
}

/// The .cpp files of `set(NAME ...)` in CMakeLists.txt
fn cmake_list(text: &str, name: &str) -> Vec<String> {
    let start = format!("set({name}");
    let Some(pos) = text.find(&start) else { return Vec::new() };
    let body = &text[pos + start.len()..];
    let body = &body[..body.find(')').unwrap_or(body.len())];
    body.split_whitespace().filter(|t| t.ends_with(".cpp")).map(str::to_owned).collect()
}

/// The lib*/include directories of `target_include_directories(fdk-aac ...)`
fn include_dirs(text: &str) -> Vec<String> {
    let start = "target_include_directories(fdk-aac";
    let Some(pos) = text.find(start) else { return Vec::new() };
    let body = &text[pos + start.len()..];
    // Generator expressions ($<...>, ${...}) use no parentheses: the block
    // ends at the first ')'
    let body = &body[..body.find(')').unwrap_or(body.len())];
    body.split_whitespace()
        .map(|t| t.trim_end_matches('>'))
        .filter_map(|t| {
            let mut parts = t.rsplit('/');
            let last = parts.next()?;
            let lib = parts.next()?;
            (last == "include" && lib.starts_with("lib")).then(|| format!("{lib}/include"))
        })
        .collect()
}

/// Compiles `sources` on several threads (about 100 C++ files; `cc` without
/// its `parallel` feature would compile them one by one) and archives them
/// as lib`name`
fn compile_parallel(build: &cc::Build, sources: &[PathBuf], name: &str) {
    let jobs = env::var("NUM_JOBS")
        .ok()
        .and_then(|n| n.parse::<usize>().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()))
        .clamp(1, 16);
    let chunk = sources.len().div_ceil(jobs).max(1);
    let objects: Vec<PathBuf> = std::thread::scope(|s| {
        let handles: Vec<_> = sources
            .chunks(chunk)
            .map(|files| {
                let mut b = build.clone();
                b.files(files.iter().map(|f| f.as_path()));
                s.spawn(move || b.compile_intermediates())
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().expect("compiler thread panicked")).collect()
    });
    let mut archive = build.clone();
    archive.objects(objects.iter().map(Path::new));
    archive.compile(name);
}
