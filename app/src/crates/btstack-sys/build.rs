//! Builds BTstack (classic: HCI, L2CAP, SDP, AVDTP / A2DP source, AVRCP; LE:
//! SM, ATT / GATT, LE device DB in a TLV file, ISO), the Windows run loop and
//! TLV store, micro-ecc, the chipset drivers and the C glue in csrc/shim.c
//! with the MSVC compiler found by `cc`.
//!
//! The BTstack sources come from the git submodule extern/btstack of the
//! repository (override with the BTSTACK_DIR environment variable).

use std::env;
use std::path::{Path, PathBuf};

/// src/*.c not needed by BlueCaster (audio playback helpers, LC3 - LE Audio
/// encodes with crate lc3-sys -, other HCI transports, HID)
const CORE_EXCLUDE: &[&str] = &[
    "btstack_audio.c",
    "btstack_audio_generator.c",
    "btstack_audio_player.c",
    "btstack_audio_song.c",
    "btstack_lc3.c",
    "btstack_lc3_google.c",
    "btstack_lc3plus_fraunhofer.c",
    "btstack_resample.c",
    "btstack_sample_rate_compensation.c",
    "btstack_slip.c",
    "btstack_uart_slip_wrapper.c",
    "hci_transport_em9304_spi.c",
    "hci_transport_h4.c",
    "hci_transport_h5.c",
    "btstack_hid.c",
    "btstack_hid_parser.c",
    "btstack_base64_decoder.c",
    "btstack_base64_encoder.c",
];

const CLASSIC: &[&str] = &[
    "a2dp.c",
    "a2dp_source.c",
    "a2dp_sink.c",
    "avdtp.c",
    "avdtp_acceptor.c",
    "avdtp_initiator.c",
    "avdtp_sink.c",
    "avdtp_source.c",
    "avdtp_util.c",
    "avrcp.c",
    "avrcp_controller.c",
    "avrcp_target.c",
    "rfcomm.c",
    "hfp.c",
    "hfp_ag.c",
    "hfp_gsm_model.c",
    "sdp_client.c",
    "sdp_client_rfcomm.c",
    "sdp_server.c",
    "sdp_util.c",
    "btstack_link_key_db_memory.c",
];

/// src/ble/*.c not built: LE bonds are kept in a TLV file
/// (le_device_db_tlv.c), as in v1; sm.c is built from a patched copy
/// ([`patched_sm`])
const BLE_EXCLUDE: &[&str] = &["le_device_db_memory.c", "sm.c"];

/// Bluedroid SBC encoder (Apache-2.0), feature "sbc-reference"
const SBC_ENCODER: &[&str] = &[
    "sbc_analysis.c",
    "sbc_dct.c",
    "sbc_dct_coeffs.c",
    "sbc_enc_bit_alloc_mono.c",
    "sbc_enc_bit_alloc_ste.c",
    "sbc_enc_coeffs.c",
    "sbc_encoder.c",
    "sbc_packing.c",
];

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let btstack =
        env::var_os("BTSTACK_DIR").map(PathBuf::from).unwrap_or_else(|| manifest.join("../../../../extern/btstack"));
    // std::path::absolute, not canonicalize: MSVC does not take \\?\ paths
    let btstack = std::path::absolute(&btstack).unwrap();
    assert!(
        btstack.join("src/hci.c").exists(),
        "BTstack not found at {} (git submodule update --init extern/btstack)",
        btstack.display()
    );
    let csrc = manifest.join("csrc");

    let mut build = cc::Build::new();
    build
        .include(&csrc)
        .include(btstack.join("src"))
        .include(btstack.join("platform/windows"))
        .include(btstack.join("platform/posix"))
        .include(btstack.join("chipset/realtek"))
        .include(btstack.join("chipset/bcm"))
        .include(btstack.join("3rd-party/tinydir"))
        .include(btstack.join("3rd-party/rijndael"))
        .include(btstack.join("3rd-party/micro-ecc"))
        .define("_WIN32_WINNT", "0x0A00")
        .define("NOMINMAX", None)
        .define("WIN32_LEAN_AND_MEAN", None)
        .define("_CRT_SECURE_NO_WARNINGS", None)
        .warnings(false)
        .flag_if_supported("/utf-8")
        .flag_if_supported("/W1")
        // A function called without a declaration (C4013) links against
        // nothing when it is a static inline of a header: an error
        .flag_if_supported("/we4013");

    // The Realtek driver prints why it loads which firmware ("Realtek:
    // Using firmware ...", "Firmware already exists") with printf, to a
    // stdout the resident process does not have: BTstack's
    // ENABLE_PRINTF_TO_LOG sends those lines to the HCI dump log instead
    // (HCI_DUMP_LOG_LEVEL_PRINT), for this file only
    let mut realtek = build.clone();
    // csrc/realtek.c includes btstack_chipset_realtek.c (reads its firmware table)
    realtek.define("ENABLE_PRINTF_TO_LOG", None).file(csrc.join("realtek.c"));
    realtek.compile("btstack_realtek");

    let mut core: Vec<PathBuf> = std::fs::read_dir(btstack.join("src"))
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "c"))
        .filter(|p| !CORE_EXCLUDE.contains(&p.file_name().unwrap().to_str().unwrap()))
        .collect();
    core.sort();
    build.files(&core);
    let mut ble: Vec<PathBuf> = std::fs::read_dir(btstack.join("src/ble"))
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "c"))
        .filter(|p| !BLE_EXCLUDE.contains(&p.file_name().unwrap().to_str().unwrap()))
        .collect();
    ble.sort();
    build.files(&ble);
    build.file(patched_sm(&btstack));
    build.files(CLASSIC.iter().map(|f| match *f {
        "a2dp.c" => patched_a2dp(&btstack),
        f => btstack.join("src/classic").join(f),
    }));
    build.file(btstack.join("platform/windows/btstack_run_loop_windows.c"));
    // LE bonds (le_device_db_tlv) and the GATT server's client
    // configurations in a TLV file, readable by v1's le_audio_bonds.tlv
    build.file(btstack.join("platform/windows/btstack_tlv_windows.c"));
    // Software P-256 for LE Secure Connections (ENABLE_MICRO_ECC_P256)
    build.file(btstack.join("3rd-party/micro-ecc/uECC.c"));
    build.file(btstack.join("chipset/bcm/btstack_chipset_bcm.c"));
    build.file(btstack.join("3rd-party/rijndael/rijndael.c"));
    build.file(csrc.join("shim.c"));

    if env::var_os("CARGO_FEATURE_SBC_REFERENCE").is_some() {
        let enc = btstack.join("3rd-party/bluedroid/encoder");
        build.include(enc.join("include"));
        build.files(SBC_ENCODER.iter().map(|f| enc.join("srce").join(f)));
        build.file(csrc.join("sbc_reference.c"));
        build.define("BLUECASTER_SBC_REFERENCE", None);
    }

    build.compile("btstack");

    for lib in ["setupapi", "winusb", "ws2_32", "advapi32", "user32"] {
        println!("cargo:rustc-link-lib={lib}");
    }
    println!("cargo:rerun-if-changed=csrc");
    println!("cargo:rerun-if-env-changed=BTSTACK_DIR");
    rerun_dir(&btstack.join("src"));
    rerun_dir(&btstack.join("src/ble"));
    // For dependents (e.g. tests that need the include paths)
    println!("cargo:root={}", btstack.display());
}

/// src/classic/a2dp.c with its SEP discovery bounded, written to OUT_DIR.
///
/// BTstack 1.8.1 keeps the free sink SEPs a headset lists in a static array
/// of AVDTP_MAX_SEP_NUM (10) without a bounds check: an 11th overwrites the
/// statics after it (the HFP callbacks; BlueCaster then calls a NULL
/// hfp_ag_callback). Upstream fixed it in fc208fcce0 ("a2dp: ignore
/// additional stream endpoints", 2026-07-22, not in a release) by ignoring
/// SEPs beyond the array; that check is added here, and the array holds 62
/// SEPs, every SEID a remote can list (0x01 - 0x3E), so no codec of a
/// headset with many endpoints is ignored.
fn patched_a2dp(btstack: &Path) -> PathBuf {
    fn replace_once(source: &mut String, from: &str, to: &str) {
        assert_eq!(source.matches(from).count(), 1, "a2dp.c patch: `{from}` not found once (BTstack changed?)");
        *source = source.replacen(from, to, 1);
    }
    let mut source = std::fs::read_to_string(btstack.join("src/classic/a2dp.c")).unwrap();
    // Built from OUT_DIR: the header next to it by its path under src/
    replace_once(&mut source, "#include \"a2dp.h\"", "#include \"classic/a2dp.h\"");
    replace_once(&mut source, "#define AVDTP_MAX_SEP_NUM 10", "#define AVDTP_MAX_SEP_NUM 62");
    if !source.contains("discovery limit reached") {
        let store = "a2dp_config_process_sep_discovery_seps[a2dp_config_process_sep_discovery_count++] = sep;";
        replace_once(
            &mut source,
            store,
            &format!(
                "if (a2dp_config_process_sep_discovery_count == AVDTP_MAX_SEP_NUM) {{\n\
                 log_info(\"A2DP: ignoring SEP %u, discovery limit reached\", sep.seid);\n\
                 break;\n\
                 }}\n\
                 {store}"
            ),
        );
    }
    let path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("a2dp.c");
    std::fs::write(&path, source).unwrap();
    path
}

fn replace_once(source: &mut String, file: &str, from: &str, to: &str) {
    assert_eq!(source.matches(from).count(), 1, "{file} patch: `{from}` not found once (BTstack changed?)");
    *source = source.replacen(from, to, 1);
}

/// src/ble/sm.c with Cross-Transport Key Derivation from BR/EDR (issue #86),
/// written to OUT_DIR.
///
/// ENABLE_CROSS_TRANSPORT_KEY_DERIVATION is defined for this file only: the
/// rest of BTstack (L2CAP's fixed channel mask) stays as before, so a
/// device is never told BlueCaster takes SMP over BR/EDR. Of what the flag
/// brings, BlueCaster uses one direction: after a classic link is encrypted
/// with Secure Connections, BlueCaster as Central asks the device over the
/// BR/EDR security manager channel (CID 7) for its IRK and identity
/// address, and both sides derive the LE LTK from the link key. That runs
/// only while the shim says so (bluecaster_ctkd_enabled: developer mode;
/// SM itself runs only with LE Audio on), after a new pairing (BTstack's
/// own trigger) or on a connection of a device with no keys from it yet
/// (bluecaster_ctkd_wanted). A device that does not list CID 7 is not
/// asked; a failure ends SMP on that link only, never the link.
///
/// The other direction (a BR/EDR link key derived from an LE pairing) stays
/// off, as before: BTstack would derive and store it whether or not the
/// device distributes one, replacing the device's classic key. LE pairing
/// keeps its key distribution and AuthReq of today (no LinkKey, no CT2),
/// and SM never stores a link key.
fn patched_sm(btstack: &Path) -> PathBuf {
    const FILE: &str = "sm.c";
    let mut source = std::fs::read_to_string(btstack.join("src/ble/sm.c")).unwrap();
    source.insert_str(
        0,
        "/* BlueCaster: patched by btstack-sys build.rs (patched_sm) */\n\
         #define ENABLE_CROSS_TRANSPORT_KEY_DERIVATION\n\
         int bluecaster_ctkd_enabled(void);\n\
         int bluecaster_ctkd_wanted(const unsigned char *addr);\n",
    );
    // Only while enabled; off, the state stays SM_BR_EDR_W4_ENCRYPTION_COMPLETE,
    // where a Pairing Request from the device is refused
    let handler =
        "static void sm_event_handle_classic_encryption_event(sm_connection_t * sm_conn, hci_con_handle_t con_handle){";
    replace_once(&mut source, FILE, handler, &format!("{handler}\n    if (!bluecaster_ctkd_enabled()) return;"));
    // Also for a device paired before (BTstack asks only after a pairing)
    replace_once(
        &mut source,
        FILE,
        "} else if (sm_conn->sm_pairing_requested){",
        "} else if (sm_conn->sm_pairing_requested || bluecaster_ctkd_wanted(sm_conn->sm_peer_address)){",
    );
    // No link key derived from an LE pairing, none offered in it
    let from_le = "static bool sm_ctkd_from_le(void) {";
    replace_once(&mut source, FILE, from_le, &format!("{from_le}\n    if (setup != NULL) return false;"));
    replace_once(&mut source, FILE, "flags |= SM_KEYDIST_LINK_KEY;", "/* BlueCaster: no LinkKey over LE */");
    replace_once(&mut source, FILE, "auth_req |= SM_AUTHREQ_CT2;", "/* BlueCaster: no CT2 over LE */");
    // Nor stored: after a derivation from BR/EDR, BTstack 1.8.1 stores the
    // link key it read for it (setup->sm_link_key, most significant octet
    // first for h6 / h7) back as a link key, byte-reversed and under the
    // device's identity address. With a public identity that replaces the
    // device's link key with a wrong one (the next connection fails
    // authentication); with a random one it adds a stray key.
    let store = "static void sm_store_classic_bonding_information(sm_connection_t * sm_conn) {";
    replace_once(&mut source, FILE, store, &format!("{store}\n    if (sm_conn != NULL) return;"));
    let path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("sm.c");
    std::fs::write(&path, source).unwrap();
    path
}

fn rerun_dir(dir: &Path) {
    println!("cargo:rerun-if-changed={}", dir.display());
}
