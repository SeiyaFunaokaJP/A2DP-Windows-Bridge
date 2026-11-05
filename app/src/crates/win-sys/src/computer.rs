//! The computer's name

use windows::Win32::System::SystemInformation::{ComputerNamePhysicalDnsHostname, GetComputerNameExW};
use windows::core::PWSTR;

/// The computer's name as Settings shows it under "Device name" (and
/// Windows' own Bluetooth uses), in its own case; None if Windows gives none
pub fn computer_name() -> Option<String> {
    let mut len = 0u32;
    // SAFETY: a null buffer asks for the size only (it fails with
    // ERROR_MORE_DATA and sets len)
    let _ = unsafe { GetComputerNameExW(ComputerNamePhysicalDnsHostname, None, &mut len) };
    if len == 0 {
        return None;
    }
    let mut buf = vec![0u16; len as usize];
    // SAFETY: buf holds the len UTF-16 units asked for above
    unsafe { GetComputerNameExW(ComputerNamePhysicalDnsHostname, Some(PWSTR(buf.as_mut_ptr())), &mut len) }.ok()?;
    let name = String::from_utf16_lossy(&buf[..len as usize]);
    (!name.trim().is_empty()).then_some(name)
}

#[cfg(test)]
mod tests {
    #[test]
    fn windows_has_a_computer_name() {
        let n = super::computer_name().expect("a computer name");
        assert!(!n.contains('\0'), "{n:?}");
    }
}
