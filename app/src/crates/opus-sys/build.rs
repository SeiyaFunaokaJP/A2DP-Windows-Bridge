//! Builds libopus (Xiph.Org, BSD-3-Clause) as a static library with the
//! compiler found by `cc`: the floating-point build with the default
//! options of its CMakeLists.txt (no DNN features: no DRED, OSCE or deep
//! PLC), and no SIMD-specific sources. That plain C build is the same on
//! x64 and ARM64; Opus at 48 kHz stereo needs a few percent of one core
//! even so, and no run-time CPU detection or intrinsics flags are involved.
//!
//! The sources come from the git submodule extern/opus of the repository
//! (override with OPUS_DIR). The source lists are read from its *.mk files
//! (the lists its CMake and autotools builds use too), so they follow the
//! submodule when it is updated.

use std::env;
use std::path::{Path, PathBuf};

/// Source lists of the float build (opus_sources.mk, celt_sources.mk,
/// silk_sources.mk)
const LISTS: &[(&str, &str)] = &[
    ("opus_sources.mk", "OPUS_SOURCES"),
    ("opus_sources.mk", "OPUS_SOURCES_FLOAT"),
    ("celt_sources.mk", "CELT_SOURCES"),
    ("silk_sources.mk", "SILK_SOURCES"),
    ("silk_sources.mk", "SILK_SOURCES_FLOAT"),
];

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let dir = env::var_os("OPUS_DIR").map(PathBuf::from).unwrap_or_else(|| manifest.join("../../../../extern/opus"));
    // std::path::absolute, not canonicalize: MSVC does not take \\?\ paths
    let dir = std::path::absolute(&dir).unwrap();
    assert!(
        dir.join("include/opus.h").exists() && dir.join("opus_sources.mk").exists(),
        "libopus not found at {} (git submodule update --init extern/opus)",
        dir.display()
    );

    let mut sources = Vec::new();
    for (file, list) in LISTS {
        let text = std::fs::read_to_string(dir.join(file)).unwrap();
        let files = mk_list(&text, list);
        assert!(!files.is_empty(), "{list} not found in {}", dir.join(file).display());
        sources.extend(files.into_iter().map(|f| dir.join(f)));
    }

    // opus_get_version_string(): the release's package_version file, else
    // `git describe` of the submodule (as cmake/OpusPackageVersion.cmake)
    let version = package_version(&dir);

    let mut build = cc::Build::new();
    build
        .define("PACKAGE_VERSION", Some(format!("\"{version}\"").as_str()))
        .includes(["include", "celt", "silk", "silk/float", "dnn", "."].iter().map(|d| dir.join(d)))
        // As its CMakeLists.txt defines them for MSVC (no VLAs: alloca)
        .define("OPUS_BUILD", None)
        .define("USE_ALLOCA", None)
        .define("DISABLE_DEBUG_FLOAT", None)
        // lrintf / lrint for float -> int (celt/float_cast.h takes them
        // with C99 or later; x64 MSVC uses an SSE intrinsic before that,
        // ARM64 needs them)
        .define("HAVE_LRINT", None)
        .define("HAVE_LRINTF", None)
        .warnings(false)
        .cargo_warnings(false)
        // The encoder runs in real time on the encode thread: keep it
        // optimized in debug builds of BlueCaster too
        .opt_level(2);
    if build.get_compiler().is_like_msvc() {
        build.flag("/std:c11").flag("/W1").flag("/utf-8");
    } else {
        build.std("c11").flag("-w");
    }
    compile_parallel(&build, &sources, "opus");

    println!("cargo:rerun-if-env-changed=OPUS_DIR");
    for d in ["include", "src", "celt", "silk"] {
        println!("cargo:rerun-if-changed={}", dir.join(d).display());
    }
    for (file, _) in LISTS {
        println!("cargo:rerun-if-changed={}", dir.join(file).display());
    }
    // For dependents that need the headers (DEP_OPUS_INCLUDE)
    println!("cargo:include={}", dir.join("include").display());
}

fn package_version(dir: &Path) -> String {
    if let Ok(text) = std::fs::read_to_string(dir.join("package_version"))
        && let Some(v) = text.lines().find_map(|l| l.strip_prefix("PACKAGE_VERSION="))
    {
        return v.trim().trim_matches('"').to_owned();
    }
    std::process::Command::new("git")
        .args(["describe", "--tags", "--match", "v*"])
        .current_dir(dir)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|v| v.trim().trim_start_matches('v').to_owned())
        .filter(|v| !v.is_empty() && v.chars().all(|c| c.is_ascii_alphanumeric() || ".-_".contains(c)))
        .unwrap_or_else(|| "unknown".to_owned())
}

/// The .c files of `NAME = \` ... in a *.mk file
fn mk_list(text: &str, name: &str) -> Vec<String> {
    let mut lines = text.lines();
    let start = format!("{name} =");
    if !lines.any(|l| l.trim_end() == start || l.trim_end() == format!("{start} \\")) {
        return Vec::new();
    }
    let mut files = Vec::new();
    for line in lines {
        let entry = line.trim().trim_end_matches('\\').trim();
        if entry.ends_with(".c") {
            files.push(entry.to_owned());
        }
        if !line.trim_end().ends_with('\\') {
            break;
        }
    }
    files
}

/// Compiles `sources` on several threads (about 150 C files) and archives
/// them as lib`name`
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
