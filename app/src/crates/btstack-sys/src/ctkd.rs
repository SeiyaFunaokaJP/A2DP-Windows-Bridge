//! Cross-Transport Key Derivation from BR/EDR (issue #86): the LE keys of a
//! classic device, as Windows and Linux get them when they pair it. After a
//! classic link is encrypted with Secure Connections, SM (the patched sm.c,
//! see build.rs `patched_sm`) asks the device over the BR/EDR security
//! manager channel for its IRK and identity address, and both sides derive
//! the LE LTK from the link key. SM keeps them as an LE bond of the device's
//! identity address (le_device_db, the bond TLV file); which classic device
//! a bond came from is kept beside it in the same file (one tag per bond).
//!
//! Runs only while [`set_ctkd`] is on (developer mode) and SM is set up (LE
//! Audio on, [`Stack::le_init`]): after a new pairing, and on a connection
//! of a device without keys from it yet (once per device and run). A device
//! that does not list the channel is not asked; one that refuses, or does
//! not answer (SM's 30 s timeout), keeps its classic link as it is.

use std::cell::RefCell;
use std::ffi::c_int;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::callbacks::guard;
use crate::consts::*;
use crate::stack::addr_str;
use crate::{ConnectionType, Stack, ffi};

static ENABLED: AtomicBool = AtomicBool::new(false);

thread_local! {
    /// Classic devices asked during this run (each once)
    static ASKED: RefCell<Vec<[u8; 6]>> = const { RefCell::new(Vec::new()) };
}

/// Asks classic devices for their LE keys (from any thread; effective while
/// SM runs, i.e. with LE Audio on)
pub fn set_ctkd(enabled: bool) {
    if ENABLED.swap(enabled, Ordering::AcqRel) != enabled {
        tracing::debug!(target: "le", "CTKD: asking classic devices for LE keys {}", if enabled { "on" } else { "off" });
    }
}

pub fn ctkd_enabled() -> bool {
    ENABLED.load(Ordering::Acquire)
}

pub(crate) fn reset() {
    ASKED.with(|a| a.borrow_mut().clear());
}

/// The LE keys BlueCaster has of a classic device
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeKeys {
    /// The identity address the device gave (BD_ADDR_TYPE_LE_PUBLIC /
    /// _RANDOM), its classic address when it gave none
    pub identity_addr_type: u8,
    pub identity_addr: [u8; 6],
    /// The device's IRK, most significant octet first as BTstack keeps it;
    /// None when it gave none
    pub irk: Option<[u8; 16]>,
    /// The LTK, most significant octet first; None when there is none
    pub ltk: Option<[u8; 16]>,
    /// The LTK's encryption key size (octets)
    pub key_size: u8,
}

/// Ours, beside le_device_db's bond `index`: the classic device it came from
fn tag(index: usize) -> u32 {
    (u32::from(b'A') << 24) | (u32::from(b'W') << 16) | (u32::from(b'K') << 8) | (index as u32 & 0xFF)
}

/// Classic address, identity address type, identity address
const MAPPING_LEN: usize = 13;

fn mapping(classic: &[u8; 6], identity_type: u8, identity: &[u8; 6]) -> [u8; MAPPING_LEN] {
    let mut v = [0u8; MAPPING_LEN];
    v[..6].copy_from_slice(classic);
    v[6] = identity_type;
    v[7..].copy_from_slice(identity);
    v
}

fn nonzero(k: [u8; 16]) -> Option<[u8; 16]> {
    (k != [0u8; 16]).then_some(k)
}

impl Stack {
    /// The LE keys of the classic device `addr`: from its key derivation,
    /// else an LE bond of the same public address. None without LE (SM) or
    /// keys.
    pub fn le_keys_of_classic(&self, addr: &[u8; 6]) -> Option<LeKeys> {
        if !self.le_initialized() {
            return None;
        }
        let count = self.le_device_db_max_count();
        let derived = (0..count).find(|&i| {
            let Some(v) = self.tlv_get(tag(i)) else { return false };
            v.len() == MAPPING_LEN
                && v[..6] == addr[..]
                && self.le_device_db_info(i).is_some_and(|e| v[..] == mapping(addr, e.addr_type, &e.addr)[..])
        });
        let index = derived.or_else(|| {
            (0..count).find(|&i| {
                self.le_device_db_info(i).is_some_and(|e| e.addr_type == BD_ADDR_TYPE_LE_PUBLIC && e.addr == *addr)
            })
        })?;
        let entry = self.le_device_db_info(index)?;
        let encryption = self.le_device_db_encryption(index);
        let ltk = encryption.and_then(|e| nonzero(e.ltk));
        Some(LeKeys {
            identity_addr_type: entry.addr_type,
            identity_addr: entry.addr,
            irk: nonzero(entry.irk),
            ltk,
            key_size: encryption.filter(|_| ltk.is_some()).map_or(0, |e| e.key_size),
        })
    }
}

/// SM events: an LE bond created over a classic link is noted with the
/// classic device's address
pub(crate) fn on_sm_event(stack: &Stack, p: &[u8]) {
    if p.first() != Some(&SM_EVENT_IDENTITY_CREATED) || p.len() < 20 {
        return;
    }
    let handle = u16::from_le_bytes([p[2], p[3]]);
    if stack.connection_type(handle) != ConnectionType::Acl {
        return;
    }
    let index = usize::from(u16::from_le_bytes([p[18], p[19]]));
    let mut classic = [0u8; 6];
    // SAFETY: on the BTstack thread; classic has the 6 bytes written.
    if unsafe { ffi::bluecaster_connection_address(handle, classic.as_mut_ptr()) } == 0 {
        return;
    }
    let Some(entry) = stack.le_device_db_info(index) else { return };
    // One bond per classic device: an older one of another identity goes
    for i in (0..stack.le_device_db_max_count()).filter(|&i| i != index) {
        if stack.tlv_get(tag(i)).is_some_and(|v| v.len() == MAPPING_LEN && v[..6] == classic[..]) {
            stack.tlv_delete(tag(i));
        }
    }
    stack.tlv_store(tag(index), &mapping(&classic, entry.addr_type, &entry.addr));
    tracing::info!(
        target: "le",
        "CTKD: {} gave its LE keys (identity {}, type {}, IRK {})",
        addr_str(&classic),
        addr_str(&entry.addr),
        entry.addr_type,
        if entry.irk == [0u8; 16] { "none" } else { "received" }
    );
}

#[unsafe(no_mangle)]
extern "C" fn bluecaster_rs_ctkd_enabled() -> c_int {
    guard(0, || ctkd_enabled() as c_int)
}

/// SM, after a classic link was encrypted with Secure Connections, as
/// Central: whether to ask the device (no pairing just now)
#[unsafe(no_mangle)]
extern "C" fn bluecaster_rs_ctkd_wanted(addr: *const u8) -> c_int {
    guard(0, || {
        if !ctkd_enabled() || addr.is_null() {
            return 0;
        }
        let mut a = [0u8; 6];
        // SAFETY: SM passes its bd_addr_t (6 bytes).
        unsafe { std::ptr::copy_nonoverlapping(addr, a.as_mut_ptr(), 6) };
        let stack = Stack::new();
        if stack.le_keys_of_classic(&a).is_some() || ASKED.with(|s| s.borrow().contains(&a)) {
            return 0;
        }
        ASKED.with(|s| s.borrow_mut().push(a));
        tracing::info!(target: "le", "CTKD: asking {} for its LE keys", addr_str(&a));
        1
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapping_layout() {
        let v = mapping(&[1, 2, 3, 4, 5, 6], 1, &[0xC1, 2, 3, 4, 5, 6]);
        assert_eq!(v, [1, 2, 3, 4, 5, 6, 1, 0xC1, 2, 3, 4, 5, 6]);
        assert_eq!(tag(3), u32::from_be_bytes(*b"AWK\x03"));
        assert_eq!(nonzero([0; 16]), None);
        assert_eq!(nonzero([1; 16]), Some([1; 16]));
    }
}
