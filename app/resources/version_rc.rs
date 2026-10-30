// Shared by the build scripts of bluecaster-core.exe and bluecaster-driver.exe
// (`include!`): the VERSIONINFO resource, so that both programs carry the
// same product name and version as bluecaster.exe (Tauri writes its own).
// Code signing checks these fields (product name, product version).

/// The VERSIONINFO block of `file` (e.g. "bluecaster-core.exe") described as
/// `description`, from the package version Cargo gives the build script
fn version_rc(file: &str, description: &str) -> String {
    let version = std::env::var("CARGO_PKG_VERSION").unwrap();
    let num = |key: &str| std::env::var(key).unwrap().parse::<u16>().unwrap();
    // Numeric fields have no room for "-alpha.1": the string fields carry it
    let bin = format!("{},{},{},0", num("CARGO_PKG_VERSION_MAJOR"), num("CARGO_PKG_VERSION_MINOR"), num("CARGO_PKG_VERSION_PATCH"));
    format!(
        r#"
1 VERSIONINFO
FILEVERSION {bin}
PRODUCTVERSION {bin}
FILEFLAGSMASK 0x3fL
FILEFLAGS 0x0L
FILEOS 0x40004L
FILETYPE 0x1L
FILESUBTYPE 0x0L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904b0"
        BEGIN
            VALUE "CompanyName", "Seiya Funaoka"
            VALUE "FileDescription", "{description}"
            VALUE "FileVersion", "{version}"
            VALUE "InternalName", "{file}"
            VALUE "LegalCopyright", "Copyright (c) Seiya Funaoka. Apache-2.0 OR MIT."
            VALUE "OriginalFilename", "{file}"
            VALUE "ProductName", "BlueCaster"
            VALUE "ProductVersion", "{version}"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x409, 1200
    END
END
"#
    )
}
