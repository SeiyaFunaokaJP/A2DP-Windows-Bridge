//! The link keys Windows' Bluetooth keeps for one radio, so that devices it
//! paired with that radio (an adapter A2DPWB now drives with WinUSB) connect
//! to A2DPWB without pairing again.
//!
//! Windows keeps them in
//! HKLM\SYSTEM\CurrentControlSet\Services\BTHPORT\Parameters\Keys\<radio>,
//! one REG_BINARY value of 16 bytes per classic device, named by its
//! address (12 lowercase hex digits, as is the radio's key). LE devices are
//! subkeys there and are not read. Only SYSTEM may read that key: an
//! administrator gets in with the backup privilege (REG_OPTION_BACKUP_RESTORE),
//! so the caller runs elevated (a2dpwb-driver.exe `windows-keys`).
//!
//! The key bytes are in the order of HCI's Link Key Notification, the order
//! BTstack and BlueZ keep them in too.

use windows::Win32::Foundation::{
    CloseHandle, ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS, ERROR_NOT_ALL_ASSIGNED, GetLastError,
    HANDLE, LUID,
};
use windows::Win32::Security::{
    AdjustTokenPrivileges, LUID_AND_ATTRIBUTES, LookupPrivilegeValueW, SE_BACKUP_NAME, SE_PRIVILEGE_ENABLED,
    TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
};
use windows::Win32::System::Registry::{
    HKEY, HKEY_LOCAL_MACHINE, KEY_READ, REG_BINARY, REG_OPTION_BACKUP_RESTORE, REG_VALUE_TYPE, RegEnumValueW,
    RegOpenKeyExW,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::core::{HSTRING, PCWSTR};

use crate::autostart::{RegKey, check};

const KEYS: &str = r"SYSTEM\CurrentControlSet\Services\BTHPORT\Parameters\Keys";

/// A classic device's link key as Windows keeps it
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsLinkKey {
    /// Display order ("XX" of "XX:.." first)
    pub address: [u8; 6],
    pub key: [u8; 16],
}

/// The classic link keys Windows keeps for the radio `radio` (display
/// order). Ok(None): Windows has no keys for that radio (it never paired a
/// device with it). Needs administrator rights.
pub fn read_link_keys(radio: &[u8; 6]) -> windows::core::Result<Option<Vec<WindowsLinkKey>>> {
    enable_backup_privilege()?;
    let path = format!(r"{KEYS}\{}", hex(radio));
    let mut key = HKEY::default();
    // SAFETY: valid root key, NUL-terminated subkey and out pointer.
    let err = unsafe {
        RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            &HSTRING::from(path.as_str()),
            Some(REG_OPTION_BACKUP_RESTORE.0),
            KEY_READ,
            &mut key,
        )
    };
    if err == ERROR_FILE_NOT_FOUND {
        tracing::info!("bt_keys: Windows has no keys for the radio {}", hex(radio));
        return Ok(None);
    }
    check(err)?;
    let key = RegKey(key);
    let mut out = Vec::new();
    for index in 0.. {
        let mut name = [0u16; 256];
        let mut name_len = name.len() as u32;
        let mut data = [0u8; 64];
        let mut data_len = data.len() as u32;
        let mut ty = 0u32;
        // SAFETY: open key; the name and data buffers are as long as their
        // lengths say, the out pointers are valid.
        let err = unsafe {
            RegEnumValueW(
                key.0,
                index,
                Some(windows::core::PWSTR(name.as_mut_ptr())),
                &mut name_len,
                None,
                Some(&mut ty),
                Some(data.as_mut_ptr()),
                Some(&mut data_len),
            )
        };
        if err == ERROR_NO_MORE_ITEMS {
            break;
        }
        // Longer than a link key: not one
        if err == ERROR_MORE_DATA {
            continue;
        }
        check(err)?;
        let name = String::from_utf16_lossy(&name[..name_len as usize]);
        if let Some(k) = link_key(&name, REG_VALUE_TYPE(ty), &data[..data_len as usize]) {
            out.push(k);
        }
    }
    tracing::info!("bt_keys: {} link key(s) for the radio {}", out.len(), hex(radio));
    Ok(Some(out))
}

/// A value of a radio's key that is a device's link key: named by a
/// device address, 16 bytes (the radio's own values, such as its IRK, have
/// other names)
fn link_key(name: &str, ty: REG_VALUE_TYPE, data: &[u8]) -> Option<WindowsLinkKey> {
    if ty != REG_BINARY || name.len() != 12 {
        return None;
    }
    let key: [u8; 16] = data.try_into().ok()?;
    let mut address = [0u8; 6];
    for (i, a) in address.iter_mut().enumerate() {
        *a = u8::from_str_radix(name.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(WindowsLinkKey { address, key })
}

/// "aabbccddeeff", the form of the registry's key names
fn hex(a: &[u8; 6]) -> String {
    a.iter().map(|b| format!("{b:02x}")).collect()
}

/// SeBackupPrivilege on this process's token (an administrator has it,
/// disabled): opening a key with REG_OPTION_BACKUP_RESTORE then skips its
/// access check
fn enable_backup_privilege() -> windows::core::Result<()> {
    let mut token = HANDLE::default();
    // SAFETY: GetCurrentProcess is a pseudo handle; `token` is an out pointer.
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token) }?;
    let mut luid = LUID::default();
    // SAFETY: SE_BACKUP_NAME is a static NUL-terminated string; `luid` is an
    // out pointer.
    let r = unsafe { LookupPrivilegeValueW(PCWSTR::null(), SE_BACKUP_NAME, &mut luid) }.and_then(|()| {
        let privileges = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES { Luid: luid, Attributes: SE_PRIVILEGE_ENABLED }],
        };
        // SAFETY: `token` was opened with TOKEN_ADJUST_PRIVILEGES; no
        // previous state is asked for.
        unsafe { AdjustTokenPrivileges(token, false, Some(&privileges), 0, None, None) }?;
        // Succeeds also when the token lacks the privilege: then this
        // error is set
        // SAFETY: reads this thread's last error.
        match unsafe { GetLastError() } {
            ERROR_NOT_ALL_ASSIGNED => Err(ERROR_NOT_ALL_ASSIGNED.into()),
            _ => Ok(()),
        }
    });
    // SAFETY: a handle this function opened and owns.
    let _ = unsafe { CloseHandle(token) };
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_device_values_are_link_keys() {
        let key = [0x11u8; 16];
        let k = link_key("a1b2c3d4e5f6", REG_BINARY, &key).unwrap();
        assert_eq!(k.address, [0xA1, 0xB2, 0xC3, 0xD4, 0xE5, 0xF6]);
        assert_eq!(k.key, key);
        assert!(link_key("CentralIRK", REG_BINARY, &key).is_none());
        assert!(link_key("a1b2c3d4e5f6", REG_VALUE_TYPE(4), &key).is_none());
        assert!(link_key("a1b2c3d4e5f6", REG_BINARY, &key[..8]).is_none());
        assert!(link_key("a1b2c3d4e5gg", REG_BINARY, &key).is_none());
        assert_eq!(hex(&[0x00, 0x1A, 0x7D, 0xDA, 0x71, 0x13]), "001a7dda7113");
    }
}
