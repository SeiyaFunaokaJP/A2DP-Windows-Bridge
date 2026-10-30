//! The names A2DP Windows Bridge (A2DPWB: v1, and v2 up to 2.0.x) used
//! before the rename to BlueCaster (3.0.0) that must survive for a while:
//! to find, stop, read or take over what an older version left on a PC, and
//! for third-party IPC clients written for 2.0.x. All in one place so that
//! they can be removed together; each says when.
//!
//! Some live in the lower crates that use them (ipc and win-sys cannot
//! depend on this crate) and are re-exported here.

/// v1's program (wxWidgets GUI). The core asks a running v1 to exit and
/// takes over its Run value. Keep while v1 users may still update to
/// BlueCaster (6–12 months after 3.0.0).
pub const V1_EXE: &str = "A2DPWB.exe";

/// The mutex v1 holds while it runs (defined in win-sys, which uses it).
/// Same lifetime as [`V1_EXE`].
pub use win_sys::single_instance::V1_MUTEX_NAME;

/// 2.0.x core's single-instance mutex, `Local\A2DPWB_v2_Core` (defined in
/// ipc; [`ipc::old_core_mutex_name`] adds the development suffix). A new
/// core asks the old one to exit and then holds it too. Can be removed 6–12
/// months after 3.0.0.
pub use ipc::OLD_CORE_MUTEX;

/// 2.0.x pipe name prefix, `\\.\pipe\a2dpwb-<SID>` (defined in ipc; see
/// [`ipc::old_pipe_name`]). The core still serves this name: the old pipe
/// name was promised for at least 6 months after 3.0.0 for third-party IPC
/// clients (e.g. a Stream Deck plugin).
pub use ipc::OLD_PIPE_PREFIX;

/// The settings folder's name before 3.0.0 (`%APPDATA%\A2DPWB`), migrated
/// to the new folder. Can be removed 6–12 months after 3.0.0.
pub const OLD_SETTINGS_FOLDER: &str = "A2DPWB";

/// The HKCU Run value name of v1 and 2.0.x (defined in win-sys' autostart).
/// The core moves its own entry from it to the new name at start. Can be
/// removed 6–12 months after 3.0.0.
pub use win_sys::autostart::OLD_VALUE_NAME as OLD_RUN_VALUE_NAME;

/// The app's HKCU key before 3.0.0, `Software\A2DPWB` (the installer's
/// SetupLanguage; defined in win-sys' locale, which still reads it). Can be
/// removed 6–12 months after 3.0.0.
pub use win_sys::locale::OLD_REGISTRY_KEY;

/// The firmware download cache before 3.0.0, `%TEMP%\A2DPWB_cache`: still
/// read (never written or removed) so that a file downloaded by 2.0.x is
/// not fetched again. Can be removed 6–12 months after 3.0.0.
pub const OLD_FIRMWARE_CACHE: &str = "A2DPWB_cache";

/// The Tauri identifier before 3.0.0: names the old WebView2 data folder
/// (`%LOCALAPPDATA%\io.github.seiyafunaokajp.a2dpwb`) an uninstaller or
/// migration may clean up. Can be removed 6–12 months after 3.0.0.
pub const OLD_APP_IDENTIFIER: &str = "io.github.seiyafunaokajp.a2dpwb";

/// The install folder's name before 3.0.0 (`%LOCALAPPDATA%\Programs\A2DP
/// Windows Bridge` or under Program Files), for the installer's upgrade
/// from 2.0.x. Can be removed 6–12 months after 3.0.0.
pub const OLD_INSTALL_DIR: &str = "A2DP Windows Bridge";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_old_names() {
        assert_eq!(V1_EXE, "A2DPWB.exe");
        assert_eq!(V1_MUTEX_NAME, "A2DPWB_SingleInstance");
        assert_eq!(OLD_CORE_MUTEX, r"Local\A2DPWB_v2_Core");
        assert_eq!(OLD_PIPE_PREFIX, "a2dpwb");
        assert_eq!(OLD_SETTINGS_FOLDER, "A2DPWB");
        assert_eq!(OLD_RUN_VALUE_NAME, "A2DPWB");
        assert_eq!(OLD_REGISTRY_KEY, r"Software\A2DPWB");
        assert_eq!(OLD_FIRMWARE_CACHE, "A2DPWB_cache");
        assert_eq!(OLD_APP_IDENTIFIER, "io.github.seiyafunaokajp.a2dpwb");
        assert_eq!(OLD_INSTALL_DIR, "A2DP Windows Bridge");
    }
}
