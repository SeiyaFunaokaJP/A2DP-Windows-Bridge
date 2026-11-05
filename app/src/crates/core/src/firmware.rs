//! Realtek firmware download: the files of a chip in the table, fetched from
//! linux-firmware (kernel.org) when the user asks for them (issue #81).
//!
//! A2DPWB does not ship or mirror the files: they are proprietary Realtek
//! binaries, redistributable under the terms in linux-firmware's WHENCE. The
//! user's PC fetches them from kernel.org itself, at a pinned linux-firmware
//! commit, and each file is checked against its SHA-256 before it is saved.
//!
//! linux-firmware names some files differently from BTstack (rtl8822b_fw.bin
//! for BTstack's rtl8822bu_fw.bin) and has no config file for most chips.
//! The files are saved under BTstack's names; a chip without a config file
//! gets an empty one, which BTstack takes as "use the efuse settings" (as it
//! does a missing one, but the firmware check wants the file there).
//!
//! Blocking (ureq with rustls; the system proxy, as update.rs): call from a
//! worker thread.

use std::path::{Path, PathBuf};
use std::time::Duration;

use bt_host::chips;
use sha2::{Digest, Sha256};

/// The linux-firmware commit the files and hashes are from
pub const LINUX_FIRMWARE_COMMIT: &str = "9a80a04181713da7f8187ec928c15160137f3262";
const PLAIN: &str = "https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/plain/rtl_bt";
/// The firmware files are 40-130 kB
const MAX_FILE: u64 = 1 << 20;
const TIMEOUT: Duration = Duration::from_secs(30);

/// A file in linux-firmware's rtl_bt/
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Source {
    name: &'static str,
    sha256: &'static str,
}

/// Where a chip's files come from
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ChipSource {
    /// Its PID in the chip table (bt_host::chips::REALTEK_CHIPS)
    pid: u16,
    firmware: Source,
    /// None: linux-firmware has none, an empty file is saved
    config: Option<Source>,
}

const fn src(name: &'static str, sha256: &'static str) -> Source {
    Source { name, sha256 }
}

/// Every chip of the table but Custom (linux-firmware 9a80a04)
const SOURCES: &[ChipSource] = &[
    ChipSource {
        pid: 0x8771, // RTL8761BU
        firmware: src("rtl8761bu_fw.bin", "1d7a9597349ad89344fa16c1913d3e39e9a12e966e417ca16871bc79bbe59edb"),
        config: Some(src("rtl8761bu_config.bin", "6c28a3f07c6a30ed208c4b64862a23f02b7d93543ea980edd24df16bab45095f")),
    },
    ChipSource {
        pid: 0xB82C, // RTL8822BU
        firmware: src("rtl8822b_fw.bin", "eecef2c3b2ff03c3a41292a93a25eca6f4ec38eb4a7491a3651b530be89a04fc"),
        config: Some(src("rtl8822b_config.bin", "ea082136d5285655372e3805cd50d961bd7d42085c6fc1058a115a7e844df9c1")),
    },
    ChipSource {
        pid: 0xC821, // RTL8821CU
        firmware: src("rtl8821c_fw.bin", "dfe7ef772d078ec09490c02190a32fcd40b3eaf3b040049d8930eb80e6d5743c"),
        config: Some(src("rtl8821c_config.bin", "292717b0610ce920f66b734c299d349ea59a4c4400044078d8fc65625f17e382")),
    },
    ChipSource {
        pid: 0xC82C, // RTL8822CU
        firmware: src("rtl8822cu_fw.bin", "bb8b1221b62749baebfa3e60271d6d90b1db5e3a480e3255be82c9fb58fe9577"),
        config: None,
    },
    ChipSource {
        pid: 0xD723, // RTL8723DU
        firmware: src("rtl8723d_fw.bin", "6b70723b2db9b658cd9535fe36f1494b4494699e1ad07920c9356f8bd90d9263"),
        config: None,
    },
    ChipSource {
        pid: 0xC852, // RTL8852AU
        firmware: src("rtl8852au_fw.bin", "fb52f72e986d4c96dbf7263650d3766abcbcf10c0a4f23a065ba2c388b19c125"),
        config: None,
    },
    ChipSource {
        pid: 0xB85B, // RTL8852BU
        firmware: src("rtl8852bu_fw.bin", "c2b0d34a4ff8a8ea136993f0dc4ba657522d84231b4094c2a53cda39afc91c57"),
        config: None,
    },
    ChipSource {
        pid: 0xC85A, // RTL8852CU
        firmware: src("rtl8852cu_fw.bin", "9b8b27741a914c1cecc1ab9837194236890522101a57ba1898eb55f8d538dae0"),
        config: None,
    },
    ChipSource {
        pid: 0xB851, // RTL8851BU
        firmware: src("rtl8851bu_fw.bin", "7fcf88c00b2548a5ff157102a35722db5e4be655dab1ce8d9bddb6df85f88f24"),
        config: None,
    },
];

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("no download for {0}")]
    Unknown(String),
    #[error("HTTP status {0}")]
    Status(u16),
    #[error("request failed: {0}")]
    Http(String),
    #[error("{0}: SHA-256 does not match")]
    Hash(&'static str),
    #[error("saving {0} failed: {1}")]
    Save(String, std::io::Error),
}

fn source_for(firmware_file: &str) -> Option<(&'static ChipSource, &'static chips::RealtekChip)> {
    SOURCES.iter().find_map(|s| {
        let chip = chips::realtek_chip(s.pid)?;
        firmware_file.eq_ignore_ascii_case(&format!("{}.bin", chip.fw_name)).then_some((s, chip))
    })
}

/// A2DPWB can fetch the files of the chip whose firmware file this is
/// ("rtl8761bu_fw.bin", as AdapterInfo.firmware_files lists it)
pub fn available(firmware_file: &str) -> bool {
    source_for(firmware_file).is_some()
}

/// The page of a file at the pinned commit
fn url(name: &str) -> String {
    format!("{PLAIN}/{name}?id={LINUX_FIRMWARE_COMMIT}")
}

fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect()
}

/// Downloaded files are kept here and used again (checked by their SHA-256)
/// for a later download, also after a reinstall: kernel.org is asked once
/// per file and machine. Never removed by A2DPWB; Windows' temp clean-up
/// may.
pub fn cache_dir() -> PathBuf {
    std::env::temp_dir().join("A2DPWB_cache")
}

/// The file from the cache when its SHA-256 is the one expected
fn cached(cache: &Path, source: &Source) -> Option<Vec<u8>> {
    let path = cache.join(source.name);
    let data = match std::fs::read(&path) {
        Ok(d) => d,
        Err(e) => {
            tracing::debug!("firmware cache: {} not used: {e}", path.display());
            return None;
        }
    };
    let hash = sha256_hex(&data);
    if hash != source.sha256 {
        tracing::debug!(
            "firmware cache: {} ({} bytes) has SHA-256 {hash}, expected {}, not used",
            path.display(),
            data.len(),
            source.sha256
        );
    }
    (hash == source.sha256).then_some(data)
}

/// A checked download into the cache (a failure only costs the next
/// download)
fn store(cache: &Path, source: &Source, data: &[u8]) {
    if let Err(e) = save(cache, source.name, data) {
        tracing::warn!("firmware cache: {e}");
    }
}

/// From the cache, else downloaded (and then cached)
fn fetch_cached(agent: &ureq::Agent, cache: &Path, source: &Source) -> Result<Vec<u8>, DownloadError> {
    if let Some(data) = cached(cache, source) {
        tracing::info!("firmware: {} from the cache {}", source.name, cache.display());
        return Ok(data);
    }
    let data = fetch(agent, source)?;
    store(cache, source, &data);
    Ok(data)
}

fn fetch(agent: &ureq::Agent, source: &Source) -> Result<Vec<u8>, DownloadError> {
    let started = std::time::Instant::now();
    tracing::debug!("firmware: GET {}", url(source.name));
    let mut response = agent.get(url(source.name)).call().map_err(|e| {
        tracing::debug!("firmware: {} request failed: {e}", source.name);
        match e {
            ureq::Error::StatusCode(code) => DownloadError::Status(code),
            e => DownloadError::Http(e.to_string()),
        }
    })?;
    tracing::debug!("firmware: {} status {}", source.name, response.status());
    let data = response
        .body_mut()
        .with_config()
        .limit(MAX_FILE)
        .read_to_vec()
        .map_err(|e| DownloadError::Http(e.to_string()))?;
    let hash = sha256_hex(&data);
    tracing::debug!(
        "firmware: {} {} bytes in {} ms, SHA-256 {hash} (expected {}, match {})",
        source.name,
        data.len(),
        started.elapsed().as_millis(),
        source.sha256,
        hash == source.sha256
    );
    if hash != source.sha256 {
        return Err(DownloadError::Hash(source.name));
    }
    Ok(data)
}

/// Writes through a temporary file, so a failed write leaves no half file
fn save(dir: &Path, name: &str, data: &[u8]) -> Result<(), DownloadError> {
    let fail = |e| DownloadError::Save(name.to_string(), e);
    std::fs::create_dir_all(dir).map_err(fail)?;
    let tmp = dir.join(format!("{name}.download"));
    std::fs::write(&tmp, data).map_err(fail)?;
    std::fs::rename(&tmp, dir.join(name)).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        fail(e)
    })
}

/// Fetches the files of the chip whose firmware file this is into `dir`
/// (blocking): those missing or invalid there, a valid one is kept. Both
/// are fetched before either is saved. The names of the files saved.
pub fn download(dir: &Path, firmware_file: &str) -> Result<Vec<String>, DownloadError> {
    let (source, chip) = source_for(firmware_file).ok_or_else(|| DownloadError::Unknown(firmware_file.to_string()))?;
    let fw_name = format!("{}.bin", chip.fw_name);
    let cfg_name = format!("{}.bin", chip.cfg_name);
    let need_fw = chips::firmware_file_state(&dir.join(&fw_name)) != chips::FileState::Ok;
    let need_cfg = chips::config_file_state(&dir.join(&cfg_name)) != chips::FileState::Ok;
    tracing::debug!(
        "firmware download: {} into {}: {fw_name} needed {need_fw} (from {}), {cfg_name} needed {need_cfg} (from {:?})",
        chip.chip_name,
        dir.display(),
        source.firmware.name,
        source.config.map(|c| c.name)
    );

    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .user_agent(concat!("A2DPWB/", env!("CARGO_PKG_VERSION")))
        .build()
        .into();
    let cache = cache_dir();
    let fw = if need_fw { Some(fetch_cached(&agent, &cache, &source.firmware)?) } else { None };
    let cfg = match (need_cfg, &source.config) {
        (false, _) => None,
        (true, Some(c)) => Some(fetch_cached(&agent, &cache, c)?),
        (true, None) => Some(Vec::new()),
    };

    let mut saved = Vec::new();
    for (name, data) in [(fw_name, fw), (cfg_name, cfg)] {
        if let Some(data) = data {
            save(dir, &name, &data)?;
            tracing::debug!("firmware download: saved {} ({} bytes)", dir.join(&name).display(), data.len());
            saved.push(name);
        }
    }
    tracing::info!(
        "firmware download for {}: saved {saved:?} (linux-firmware {LINUX_FIRMWARE_COMMIT:.7})",
        chip.chip_name
    );
    Ok(saved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cache_gives_back_only_files_with_the_expected_hash() {
        let cache = std::env::temp_dir().join(format!("a2dpwb-fwcache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&cache);
        // SHA-256 of "abc"
        let abc = src("abc.bin", "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(cached(&cache, &abc), None);
        store(&cache, &abc, b"abc");
        assert_eq!(cached(&cache, &abc).as_deref(), Some(&b"abc"[..]));
        // A changed or damaged file is not used
        std::fs::write(cache.join("abc.bin"), b"abd").unwrap();
        assert_eq!(cached(&cache, &abc), None);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn every_table_chip_but_custom_has_a_source() {
        for c in chips::REALTEK_CHIPS.iter().filter(|c| c.pid != chips::CUSTOM_CHIP_PID) {
            assert!(available(&format!("{}.bin", c.fw_name)), "{}", c.chip_name);
        }
        assert!(available("RTL8761BU_fw.bin"));
        assert!(!available("custom_fw.bin"));
        assert!(!available("rtl8761bu_config.bin"));
        assert!(!available("rtl8723bu_fw.bin"));
        for s in SOURCES {
            assert_eq!(s.firmware.sha256.len(), 64);
            assert!(s.firmware.sha256.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
        }
    }

    #[test]
    fn hashes_and_urls() {
        assert_eq!(sha256_hex(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(
            url("rtl8822b_fw.bin"),
            "https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/plain/rtl_bt/rtl8822b_fw.bin?id=9a80a04181713da7f8187ec928c15160137f3262"
        );
    }

    #[test]
    fn unknown_chip_fetches_nothing() {
        let dir = std::env::temp_dir().join("a2dpwb-fw-unknown");
        assert!(matches!(download(&dir, "custom_fw.bin"), Err(DownloadError::Unknown(_))));
        assert!(!dir.exists());
    }

    /// Valid files are kept: nothing to fetch, nothing saved
    #[test]
    fn present_files_are_kept() {
        let dir = std::env::temp_dir().join(format!("a2dpwb-fw-kept-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut fw = b"Realtech".to_vec();
        fw.extend_from_slice(&[0, 0, 0, 0, 0x51, 0x04, 0xFD, 0x77]);
        std::fs::write(dir.join("rtl8852bu_fw.bin"), &fw).unwrap();
        std::fs::write(dir.join("rtl8852bu_config.bin"), b"").unwrap();
        assert_eq!(download(&dir, "rtl8852bu_fw.bin").unwrap(), Vec::<String>::new());
        assert_eq!(std::fs::read(dir.join("rtl8852bu_fw.bin")).unwrap(), fw);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Really asks kernel.org: `cargo test -p a2dpwb-core firmware -- --ignored`
    #[test]
    #[ignore = "network"]
    fn downloads_every_chip() {
        let dir = std::env::temp_dir().join(format!("a2dpwb-fw-net-{}", std::process::id()));
        for s in SOURCES {
            let chip = chips::realtek_chip(s.pid).unwrap();
            let fw = format!("{}.bin", chip.fw_name);
            let saved = download(&dir, &fw).unwrap();
            assert_eq!(saved.len(), 2, "{saved:?}");
            assert!(chips::firmware_check(&dir, s.pid, "").unwrap().present, "{}", chip.chip_name);
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
