//! Link keys in link_keys.txt of the settings folder, in v1's format, so
//! headphones paired with v1 connect without pairing again:
//!
//! ```text
//! # BTstack link key database — do not edit while app is running
//! XX-XX-XX-XX-XX-XX <32 hex digits of the key> <key type>
//! ```
//!
//! Kept in memory and written in full (temp file + rename) on every change.

use std::path::{Path, PathBuf};

use btstack_sys::LinkKeyStore;

/// v1 kept at most 32 keys; the oldest goes first
const MAX_LINK_KEYS: usize = 32;
const FILE_NAME: &str = "link_keys.txt";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkKeyEntry {
    pub addr: [u8; 6],
    pub key: [u8; 16],
    pub key_type: i32,
}

pub struct FileLinkKeyStore {
    path: PathBuf,
    entries: Vec<LinkKeyEntry>,
}

fn parse_line(line: &str) -> Option<LinkKeyEntry> {
    let line = line.trim_end_matches(['\r', '\n']);
    if line.starts_with('#') || line.is_empty() {
        return None;
    }
    let mut parts = line.split_whitespace();
    let addr = crate::events::parse_addr(parts.next()?)?;
    let hex = parts.next()?;
    // 32 hex digits, read as bytes: a line with other characters is skipped
    // (slicing the str at byte offsets could split a character and panic)
    let hex = hex.as_bytes();
    if hex.len() != 32 {
        return None;
    }
    let mut key = [0u8; 16];
    let digit = |c: u8| char::from(c).to_digit(16);
    for (i, k) in key.iter_mut().enumerate() {
        *k = (digit(hex[i * 2])? * 16 + digit(hex[i * 2 + 1])?) as u8;
    }
    let key_type = parts.next()?.parse().ok()?;
    Some(LinkKeyEntry { addr, key, key_type })
}

fn format_entry(e: &LinkKeyEntry) -> String {
    let addr: Vec<String> = e.addr.iter().map(|b| format!("{b:02X}")).collect();
    let key: String = e.key.iter().map(|b| format!("{b:02x}")).collect();
    format!("{} {} {}\n", addr.join("-"), key, e.key_type)
}

impl FileLinkKeyStore {
    /// Loads `dir`/link_keys.txt (missing or unreadable: empty)
    pub fn open(dir: &Path) -> FileLinkKeyStore {
        let path = dir.join(FILE_NAME);
        let entries: Vec<LinkKeyEntry> = std::fs::read_to_string(&path)
            .map(|text| text.lines().filter_map(parse_line).take(MAX_LINK_KEYS).collect())
            .unwrap_or_default();
        tracing::info!("link keys: {} loaded from {}", entries.len(), path.display());
        FileLinkKeyStore { path, entries }
    }

    pub fn entries(&self) -> &[LinkKeyEntry] {
        &self.entries
    }

    fn save(&self) {
        let mut text = String::from("# BTstack link key database \u{2014} do not edit while app is running\n");
        for e in &self.entries {
            text.push_str(&format_entry(e));
        }
        let tmp = self.path.with_extension("txt.tmp");
        let result = std::fs::write(&tmp, text).and_then(|()| std::fs::rename(&tmp, &self.path));
        if let Err(e) = result {
            tracing::error!("link keys: cannot write {}: {e}", self.path.display());
        }
    }
}

impl LinkKeyStore for FileLinkKeyStore {
    fn get(&mut self, addr: &[u8; 6]) -> Option<([u8; 16], i32)> {
        let found = self.entries.iter().find(|e| &e.addr == addr).map(|e| (e.key, e.key_type));
        tracing::debug!(
            "link keys: lookup for {}: {}",
            crate::events::addr_str(addr),
            match &found {
                Some((_, t)) => format!("found (type {t})"),
                None => "none".into(),
            }
        );
        found
    }

    fn put(&mut self, addr: &[u8; 6], key: &[u8; 16], key_type: i32) {
        match self.entries.iter_mut().find(|e| &e.addr == addr) {
            Some(e) => {
                e.key = *key;
                e.key_type = key_type;
            }
            None => {
                if self.entries.len() >= MAX_LINK_KEYS {
                    self.entries.remove(0);
                }
                self.entries.push(LinkKeyEntry { addr: *addr, key: *key, key_type });
            }
        }
        self.save();
        tracing::info!("link keys: stored the key of {}", crate::events::addr_str(addr));
        tracing::debug!("link keys: key type {key_type}, {} stored in all", self.entries.len());
    }

    fn delete(&mut self, addr: &[u8; 6]) {
        let before = self.entries.len();
        self.entries.retain(|e| &e.addr != addr);
        if self.entries.len() != before {
            self.save();
            tracing::info!("link keys: deleted the key of {}", crate::events::addr_str(addr));
        }
    }

    fn at(&self, index: usize) -> Option<([u8; 6], [u8; 16], i32)> {
        self.entries.get(index).map(|e| (e.addr, e.key, e.key_type))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v1_file_round_trip() {
        let dir = std::env::temp_dir().join(format!("bluecaster-lk-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(FILE_NAME),
            "# BTstack link key database \u{2014} do not edit while app is running\n\
             00-A2-D0-00-00-02 00112233445566778899aabbccddeeff 4\n\
             garbage line\n",
        )
        .unwrap();
        let mut store = FileLinkKeyStore::open(&dir);
        let addr = [0x00, 0xA2, 0xD0, 0x00, 0x00, 0x02];
        let (key, t) = store.get(&addr).unwrap();
        assert_eq!(key[0], 0x00);
        assert_eq!(key[15], 0xFF);
        assert_eq!(t, 4);
        store.put(&[1, 2, 3, 4, 5, 6], &[7; 16], 5);
        let reread = FileLinkKeyStore::open(&dir);
        assert_eq!(reread.entries().len(), 2);
        assert_eq!(reread.entries()[1].addr, [1, 2, 3, 4, 5, 6]);
        store.delete(&addr);
        assert_eq!(FileLinkKeyStore::open(&dir).entries().len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn bad_lines_are_skipped() {
        let addr = "00-A2-D0-00-00-02";
        let good = parse_line(&format!("{addr} 00112233445566778899aabbccddeeFF 4")).unwrap();
        assert_eq!(good.key[1], 0x11);
        assert_eq!(good.key[15], 0xFF);
        // 32 bytes but not ASCII: no panic, no entry
        for key in [
            "\u{e9}112233445566778899aabbccddeeff",
            "0\u{e9}112233445566778899aabbccddeef",
            "0011223344556677889\u{3042}bccddeeff0",
        ] {
            assert_eq!(key.len(), 32, "{key}");
            assert!(parse_line(&format!("{addr} {key} 4")).is_none(), "{key}");
        }
        for bad in [
            "+0112233445566778899aabbccddeeff 4",
            "0g112233445566778899aabbccddeeff 4",
            "00112233445566778899aabbccddeeff",
            "00112233445566778899aabbccddeef 4",
        ] {
            assert!(parse_line(&format!("{addr} {bad}")).is_none(), "{bad}");
        }
    }
}
