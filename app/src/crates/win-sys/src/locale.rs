//! The user's Windows display languages

use windows::Win32::Globalization::{GetUserPreferredUILanguages, MUI_LANGUAGE_NAME};
use windows::core::PWSTR;

/// The user's preferred display languages, first choice first, as BCP 47
/// names ("ja-JP", "zh-Hant-TW"); empty if Windows gives none
pub fn preferred_ui_languages() -> Vec<String> {
    let (mut count, mut len) = (0u32, 0u32);
    // SAFETY: a null buffer asks for the size only
    if unsafe { GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, None, &mut len) }.is_err() || len == 0 {
        return Vec::new();
    }
    let mut buf = vec![0u16; len as usize];
    // SAFETY: buf holds the len UTF-16 units asked for above
    let r =
        unsafe { GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, Some(PWSTR(buf.as_mut_ptr())), &mut len) };
    if r.is_err() {
        return Vec::new();
    }
    // A double-NUL-terminated list of NUL-terminated names
    buf.split(|&c| c == 0).filter(|s| !s.is_empty()).map(String::from_utf16_lossy).collect()
}

/// The language chosen in the installer (installer/A2DPWB.iss writes the
/// app's code, "ja" or "zh-TW", to HKCU\Software\A2DPWB, value
/// SetupLanguage); None without one
pub fn setup_language() -> Option<String> {
    crate::autostart::get_in(r"Software\A2DPWB", "SetupLanguage").ok().flatten().filter(|l| !l.trim().is_empty())
}

#[cfg(test)]
mod tests {
    #[test]
    fn windows_has_a_display_language() {
        let l = super::preferred_ui_languages();
        assert!(!l.is_empty());
        assert!(l.iter().all(|t| !t.is_empty() && !t.contains('\0')), "{l:?}");
    }
}
