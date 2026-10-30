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

/// The app's own key under HKEY_CURRENT_USER (the installer's
/// SetupLanguage)
pub const REGISTRY_KEY: &str = r"Software\BlueCaster";
/// The key A2DPWB 2.0.x's installer wrote SetupLanguage to; still read
/// when [`REGISTRY_KEY`] has none (an install before 3.0.0). Can be removed
/// 6–12 months after 3.0.0.
pub const OLD_REGISTRY_KEY: &str = r"Software\A2DPWB";

/// The language chosen in the installer (installer/BlueCaster.iss writes the
/// app's code, "ja" or "zh-TW", to HKCU\Software\BlueCaster, value
/// SetupLanguage; before 3.0.0 to HKCU\Software\A2DPWB); None without one
pub fn setup_language() -> Option<String> {
    [REGISTRY_KEY, OLD_REGISTRY_KEY]
        .into_iter()
        .find_map(|key| crate::autostart::get_in(key, "SetupLanguage").ok().flatten().filter(|l| !l.trim().is_empty()))
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
