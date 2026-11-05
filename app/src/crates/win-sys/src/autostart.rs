//! Starting with Windows: the value under
//! HKCU\Software\Microsoft\Windows\CurrentVersion\Run, as v1
//! system_integration.cpp writes it (value name "A2DPWB", REG_SZ with the
//! quoted program path and its arguments).
//!
//! Also holds the small registry helpers the driver module uses.

use std::path::{Path, PathBuf};

use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_EXPAND_SZ, REG_OPTION_NON_VOLATILE, REG_SAM_FLAGS,
    REG_SZ, REG_VALUE_TYPE, RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
    RegSetValueExW,
};
use windows::core::{HSTRING, PCWSTR};

/// The Run key, under HKEY_CURRENT_USER
pub const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
/// The value name v1 uses; v2 keeps it so that installing v2 over v1
/// replaces the entry instead of starting both
pub const VALUE_NAME: &str = "A2DPWB";

/// The command line registered under `value_name`, or None if there is none
pub fn get(value_name: &str) -> windows::core::Result<Option<String>> {
    let r = get_in(RUN_KEY, value_name);
    tracing::debug!("autostart: read HKCU\\{RUN_KEY}\\{value_name}: {r:?}");
    r
}

/// Registers `command` (see `command_line`) under `value_name`
pub fn set(value_name: &str, command: &str) -> windows::core::Result<()> {
    let r = set_in(RUN_KEY, value_name, command);
    tracing::debug!("autostart: write HKCU\\{RUN_KEY}\\{value_name} = {command}: {r:?}");
    r
}

/// Removes the entry; Ok(false) if there was none
pub fn remove(value_name: &str) -> windows::core::Result<bool> {
    let r = remove_in(RUN_KEY, value_name);
    tracing::debug!("autostart: remove HKCU\\{RUN_KEY}\\{value_name}: {r:?} (true: it was there)");
    r
}

/// `"C:\path\a2dpwb.exe" --minimized`: the program path quoted (it may
/// contain spaces), then the arguments, as v1 RegisterStartup builds it
pub fn command_line(exe: &Path, args: &str) -> String {
    let exe = exe.display();
    if args.is_empty() { format!("\"{exe}\"") } else { format!("\"{exe}\" {args}") }
}

/// The program a Run command line starts: the quoted path, or up to the
/// first space when unquoted (as Windows reads it); None if empty
pub fn program_of(command: &str) -> Option<PathBuf> {
    let command = command.trim_start();
    let program = match command.strip_prefix('"') {
        Some(rest) => rest.split('"').next().unwrap_or(rest),
        None => command.split(' ').next().unwrap_or(command),
    };
    (!program.trim().is_empty()).then(|| PathBuf::from(program))
}

/// The REG_SZ value `value_name` of HKCU\`subkey`, or None if there is none
pub(crate) fn get_in(subkey: &str, value_name: &str) -> windows::core::Result<Option<String>> {
    let Some(key) = RegKey::open_hkcu(subkey, KEY_QUERY_VALUE)? else {
        return Ok(None);
    };
    match key.query(value_name)? {
        Some((ty, data)) if ty == REG_SZ || ty == REG_EXPAND_SZ => Ok(Some(utf16_string(&data))),
        Some(_) => Ok(None),
        None => Ok(None),
    }
}

fn set_in(subkey: &str, value_name: &str, command: &str) -> windows::core::Result<()> {
    let key = RegKey::create_hkcu(subkey, KEY_SET_VALUE)?;
    let mut data: Vec<u16> = command.encode_utf16().collect();
    data.push(0);
    key.set(value_name, REG_SZ, &data)
}

fn remove_in(subkey: &str, value_name: &str) -> windows::core::Result<bool> {
    let Some(key) = RegKey::open_hkcu(subkey, KEY_SET_VALUE)? else {
        return Ok(false);
    };
    key.delete_value(value_name)
}

/// A registry key handle, closed on drop
pub(crate) struct RegKey(pub(crate) HKEY);

impl RegKey {
    /// Opens HKCU\`subkey`; Ok(None) if it does not exist
    fn open_hkcu(subkey: &str, access: REG_SAM_FLAGS) -> windows::core::Result<Option<RegKey>> {
        let mut key = HKEY::default();
        // SAFETY: valid root key, NUL-terminated subkey and out pointer.
        let err = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, &HSTRING::from(subkey), None, access, &mut key) };
        if err == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        check(err)?;
        Ok(Some(RegKey(key)))
    }

    /// Opens HKCU\`subkey`, creating it if needed
    fn create_hkcu(subkey: &str, access: REG_SAM_FLAGS) -> windows::core::Result<RegKey> {
        let mut key = HKEY::default();
        // SAFETY: valid root key, NUL-terminated subkey, no class, default
        // security, and a valid out pointer.
        let err = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                &HSTRING::from(subkey),
                None,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                access,
                None,
                &mut key,
                None,
            )
        };
        check(err)?;
        Ok(RegKey(key))
    }

    /// The type and raw data of a value; Ok(None) if it does not exist
    pub(crate) fn query(&self, name: &str) -> windows::core::Result<Option<(REG_VALUE_TYPE, Vec<u16>)>> {
        let name = HSTRING::from(name);
        loop {
            let mut ty = REG_VALUE_TYPE::default();
            let mut size = 0u32;
            // SAFETY: size query: no data buffer, valid out pointers.
            let err = unsafe { RegQueryValueExW(self.0, &name, None, Some(&mut ty), None, Some(&mut size)) };
            if err == ERROR_FILE_NOT_FOUND {
                return Ok(None);
            }
            check(err)?;
            // u16 units, rounded up; one spare unit in case the stored
            // string lacks its terminator
            let mut data = vec![0u16; (size as usize).div_ceil(2) + 1];
            let mut got = size;
            // SAFETY: `data` holds at least `size` bytes, as `got` says.
            let err = unsafe {
                RegQueryValueExW(self.0, &name, None, Some(&mut ty), Some(data.as_mut_ptr().cast()), Some(&mut got))
            };
            if err == windows::Win32::Foundation::ERROR_MORE_DATA {
                continue; // the value grew in between
            }
            if err == ERROR_FILE_NOT_FOUND {
                return Ok(None);
            }
            check(err)?;
            data.truncate((got as usize).div_ceil(2));
            return Ok(Some((ty, data)));
        }
    }

    /// Writes a value; `data` in u16 units, terminators included
    pub(crate) fn set(&self, name: &str, ty: REG_VALUE_TYPE, data: &[u16]) -> windows::core::Result<()> {
        // SAFETY: the byte view covers exactly `data`, which lives for the
        // call; u16 has no padding.
        let bytes = unsafe { std::slice::from_raw_parts(data.as_ptr().cast::<u8>(), size_of_val(data)) };
        // SAFETY: open key with KEY_SET_VALUE, NUL-terminated name.
        check(unsafe { RegSetValueExW(self.0, &HSTRING::from(name), None, ty, Some(bytes)) })
    }

    /// Deletes a value; Ok(false) if it did not exist
    pub(crate) fn delete_value(&self, name: &str) -> windows::core::Result<bool> {
        // SAFETY: open key with KEY_SET_VALUE, NUL-terminated name.
        let err = unsafe { RegDeleteValueW(self.0, &HSTRING::from(name)) };
        if err == ERROR_FILE_NOT_FOUND {
            return Ok(false);
        }
        check(err)?;
        Ok(true)
    }
}

impl Drop for RegKey {
    fn drop(&mut self) {
        // SAFETY: the key was opened by this RegKey and is closed only here.
        let _ = unsafe { RegCloseKey(self.0) };
    }
}

pub(crate) fn check(err: WIN32_ERROR) -> windows::core::Result<()> {
    if err == ERROR_SUCCESS { Ok(()) } else { Err(err.into()) }
}

/// A REG_SZ value as text, without its terminator (and anything after it)
pub(crate) fn utf16_string(data: &[u16]) -> String {
    let end = data.iter().position(|&c| c == 0).unwrap_or(data.len());
    String::from_utf16_lossy(&data[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_line_quotes_the_path() {
        assert_eq!(
            command_line(Path::new(r"C:\Program Files\A2DPWB\a2dpwb.exe"), "--minimized"),
            r#""C:\Program Files\A2DPWB\a2dpwb.exe" --minimized"#
        );
        assert_eq!(command_line(Path::new(r"C:\a.exe"), ""), r#""C:\a.exe""#);
    }

    #[test]
    fn program_of_reads_the_path_back() {
        let exe = Path::new(r"C:\Program Files\A2DPWB\a2dpwb-core.exe");
        assert_eq!(program_of(&command_line(exe, "--background")).as_deref(), Some(exe));
        assert_eq!(program_of(&command_line(exe, "")).as_deref(), Some(exe));
        assert_eq!(
            program_of(r"C:\A2DPWB\A2DPWB.exe --minimized").as_deref(),
            Some(Path::new(r"C:\A2DPWB\A2DPWB.exe"))
        );
        assert_eq!(program_of(r#"  "C:\x y\a.exe"#).as_deref(), Some(Path::new(r"C:\x y\a.exe"))); // unclosed quote
        assert_eq!(program_of(""), None);
        assert_eq!(program_of(r#""" --x"#), None);
    }

    /// Writes only under a throwaway HKCU key, deleted afterwards
    #[test]
    fn set_get_remove_roundtrip() {
        let subkey = format!(r"Software\A2DPWB_test_autostart_{}", std::process::id());
        let name = "A2DPWB_test";
        assert_eq!(get_in(&subkey, name).unwrap(), None);
        assert!(!remove_in(&subkey, name).unwrap());
        let cmd = command_line(Path::new(r"C:\Program Files\A2DPWB\a2dpwb.exe"), "--minimized");
        set_in(&subkey, name, &cmd).unwrap();
        assert_eq!(get_in(&subkey, name).unwrap().as_deref(), Some(cmd.as_str()));
        assert!(remove_in(&subkey, name).unwrap());
        assert_eq!(get_in(&subkey, name).unwrap(), None);
        // SAFETY: deletes the empty test key created above.
        let err = unsafe { windows::Win32::System::Registry::RegDeleteKeyW(HKEY_CURRENT_USER, &HSTRING::from(subkey)) };
        assert_eq!(err, ERROR_SUCCESS);
    }

    #[test]
    fn reading_the_real_run_entry_works() {
        // Read-only: whatever is (or is not) registered, the call succeeds
        get(VALUE_NAME).unwrap();
    }
}
