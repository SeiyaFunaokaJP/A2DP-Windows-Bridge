<#
.SYNOPSIS
Builds the release files of A2DPWB for one architecture: the installer
(Inno Setup, installer/A2DPWB.iss), the portable zip and the updater
manifest.

.DESCRIPTION
Run on the release machine from any folder (PowerShell 7):

    pwsh installer/build-release.ps1 -Arch x64
    pwsh installer/build-release.ps1 -Arch arm64   # adds to latest.json

Writes to target/dist (or -OutDir):

    A2DPWB-<version>-win64-setup.exe        (arm64: -arm64-setup.exe)
    A2DPWB-<version>-win64-setup.exe.sig    updater signature
    A2DPWB-<version>-win64-portable.zip
    A2DPWB-<version>-win64-symbols.zip      .pdb files, to read crash dumps
    latest.json                             updater manifest of this release

Updater signing (see installer/README.md): TAURI_SIGNING_PRIVATE_KEY (the
private key file's path or content) and TAURI_SIGNING_PRIVATE_KEY_PASSWORD
sign the installer; A2DPWB_UPDATER_PUBKEY (the .pub file's path or content)
is built into a2dpwb.exe so that it accepts only updates signed with that
key. Without them the build still works, but its in-app update stays off
(the update check only shows the release page, as the portable version).
Neither key is part of the repository.

Needs: Rust (with the target), pnpm, the UI's node_modules (pnpm install),
the submodules under extern/, cargo-about (cargo install cargo-about), and
Inno Setup 6.5 or later (winget install JRSoftware.InnoSetup; ISCC.exe is
found in its default folders, on PATH or in $env:ISCC). The WebView2
bootstrapper is downloaded once into target/installer-cache.
#>
[CmdletBinding()]
param(
    [ValidateSet('x64', 'arm64')]
    [string]$Arch = 'x64',
    [string]$OutDir,
    # Release notes for latest.json (shown nowhere yet; kept for the record)
    [string]$NotesFile
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = Split-Path $PSScriptRoot -Parent
$ui = Join-Path $repo 'app/src/ui'
if (-not $OutDir) { $OutDir = Join-Path $repo 'target/dist' }
$OutDir = [IO.Path]::GetFullPath($OutDir)

$triple = @{ x64 = 'x86_64-pc-windows-msvc'; arm64 = 'aarch64-pc-windows-msvc' }[$Arch]
$platform = @{ x64 = 'windows-x86_64'; arm64 = 'windows-aarch64' }[$Arch]
$archName = @{ x64 = 'win64'; arm64 = 'arm64' }[$Arch]
$releaseUrl = 'https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/releases/download'

function Invoke-Checked([string]$what, [scriptblock]$cmd) {
    & $cmd
    if ($LASTEXITCODE -ne 0) { throw "$what failed (exit code $LASTEXITCODE)" }
}

# --- Version: the workspace's, which package.json must repeat ---
$cargoToml = Get-Content -Raw (Join-Path $repo 'Cargo.toml')
if ($cargoToml -notmatch '(?ms)^\[workspace\.package\].*?^version\s*=\s*"([^"]+)"') {
    throw 'no [workspace.package] version in Cargo.toml'
}
$version = $Matches[1]
$pkgVersion = (Get-Content -Raw (Join-Path $ui 'package.json') | ConvertFrom-Json).version
if ($pkgVersion -ne $version) {
    throw "app/src/ui/package.json has version $pkgVersion, the workspace ${version}: set both"
}
Write-Host "A2DPWB $version for $Arch ($triple)"

# --- Updater keys (from the environment, never from the repository) ---
$signing = [bool]$env:TAURI_SIGNING_PRIVATE_KEY
$pubkey = $env:A2DPWB_UPDATER_PUBKEY
if ($pubkey -and (Test-Path -LiteralPath $pubkey -PathType Leaf)) {
    $pubkey = (Get-Content -Raw -LiteralPath $pubkey).Trim()
}
if ($signing -and -not $pubkey) {
    throw 'TAURI_SIGNING_PRIVATE_KEY is set but A2DPWB_UPDATER_PUBKEY is not: the app could not check what it installs'
}
if (-not $signing) {
    Write-Warning 'TAURI_SIGNING_PRIVATE_KEY is not set: no updater signature, and the in-app update of this build stays off'
    $pubkey = ''
}

# --- ARM64 on an x64 machine: ring (rustls) builds its assembly with clang ---
if ($Arch -eq 'arm64' -and -not (Get-Command clang -ErrorAction SilentlyContinue)) {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
    $clang = if (Test-Path -LiteralPath $vswhere) {
        & $vswhere -latest -prerelease -products * -find 'VC\Tools\Llvm\x64\bin\clang.exe' | Select-Object -First 1
    }
    if (-not $clang) {
        throw 'ARM64 needs clang: add the Visual Studio component "C++ Clang Compiler for Windows" (or put LLVM on PATH)'
    }
    $env:PATH = "$(Split-Path $clang -Parent);$env:PATH"
}

# --- Folders ---
$stage = Join-Path $OutDir "stage-$Arch"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
$docDir = Join-Path $stage 'doc'
New-Item -ItemType Directory -Force (Join-Path $docDir 'licenses') | Out-Null

# --- Inno Setup's compiler ---
$iscc = @(
    $env:ISCC
    (Get-Command ISCC.exe -ErrorAction SilentlyContinue)?.Source
    foreach ($base in $env:LOCALAPPDATA, ${env:ProgramFiles(x86)}, $env:ProgramFiles) {
        if ($base) { Join-Path $base 'Programs/Inno Setup 6/ISCC.exe'; Join-Path $base 'Inno Setup 6/ISCC.exe' }
    }
) | Where-Object { $_ -and (Test-Path -LiteralPath $_ -PathType Leaf) } | Select-Object -First 1
if (-not $iscc) {
    throw 'Inno Setup''s ISCC.exe not found: winget install JRSoftware.InnoSetup (or set $env:ISCC)'
}

# --- The core and the driver helper (the UI is built by tauri build) ---
Invoke-Checked 'cargo build' {
    cargo build --release --locked --manifest-path (Join-Path $repo 'Cargo.toml') --target $triple -p a2dpwb-core-exe -p a2dpwb-driver
}
$release = Join-Path $repo "target/$triple/release"

# --- Licence documents (installed next to the programs, and in the zip) ---
$ext = Join-Path $repo 'extern'
$aptxLicense = Get-Content -Raw (Join-Path $ext 'libopenaptx/COPYING')
if ($aptxLicense -notmatch 'GNU LESSER GENERAL PUBLIC LICENSE\s+Version 2\.1') {
    throw 'extern/libopenaptx/COPYING is not LGPL-2.1: libopenaptx must be pinned to 0.2.0 (THIRD_PARTY_LICENSES.md)'
}
$docs = [ordered]@{
    'LICENSE'                              = Join-Path $repo 'LICENSE'
    'LICENSE-APACHE'                       = Join-Path $repo 'LICENSE-APACHE'
    'LICENSE-MIT'                          = Join-Path $repo 'LICENSE-MIT'
    'THIRD_PARTY_LICENSES.md'              = Join-Path $repo 'THIRD_PARTY_LICENSES.md'
    'licenses/BTstack-LICENSE.txt'         = Join-Path $ext 'btstack/LICENSE'
    'licenses/libldac-LICENSE.txt'         = Join-Path $ext 'libldac/LICENSE'
    'licenses/libldac-NOTICE.txt'          = Join-Path $ext 'libldac/NOTICE'
    # The Bluedroid SBC encoder in BTstack (ported to Rust) is Apache-2.0
    # without a licence file of its own; libldac's LICENSE is the full text
    'licenses/Apache-2.0.txt'              = Join-Path $ext 'libldac/LICENSE'
    'licenses/fdk-aac-NOTICE.txt'          = Join-Path $ext 'fdk-aac/NOTICE'
    'licenses/libopenaptx-LGPL-2.1.txt'    = Join-Path $ext 'libopenaptx/COPYING'
    'licenses/lhdcv5-enc-LICENSE.txt'      = Join-Path $ext 'lhdcv5-enc/LICENSE'
    'licenses/opus-COPYING.txt'            = Join-Path $ext 'opus/COPYING'
    'licenses/aptx-adaptive-LICENSE.txt'   = Join-Path $repo 'app/src/crates/aptx-adaptive/LICENSE'
    # LE Audio (experimental): LC3 encoder and LE Secure Connections' P-256
    'licenses/liblc3-LICENSE.txt'          = Join-Path $ext 'btstack/3rd-party/lc3-google/LICENSE'
    'licenses/micro-ecc-LICENSE.txt'       = Join-Path $ext 'btstack/3rd-party/micro-ecc/LICENSE.txt'
    'licenses/fluentui-system-icons.txt'   = Join-Path $ui 'LICENSES/fluentui-system-icons.txt'
    'licenses/solid-js-LICENSE.txt'        = Join-Path $ui 'node_modules/solid-js/LICENSE'
    'licenses/tauri-apps-api-LICENSE-MIT.txt' = Join-Path $ui 'node_modules/@tauri-apps/api/LICENSE-MIT'
}
foreach ($entry in $docs.GetEnumerator()) {
    if (-not (Test-Path -LiteralPath $entry.Value -PathType Leaf)) {
        throw "required file not found: $($entry.Value) (git submodule update --init --recursive; pnpm install in app/src/ui)"
    }
    Copy-Item -LiteralPath $entry.Value (Join-Path $docDir $entry.Key)
}
Invoke-Checked 'cargo about' {
    cargo about generate --workspace --locked --fail --manifest-path (Join-Path $repo 'Cargo.toml') `
        -c (Join-Path $repo 'about.toml') --target $triple `
        -o (Join-Path $docDir 'licenses/rust-crates.txt') (Join-Path $PSScriptRoot 'about.hbs')
}

# SOURCE.txt: where the corresponding source is (LGPL-2.1 libopenaptx)
$commit = (git -C $repo rev-parse HEAD).Trim()
$dirty = git -C $repo status --porcelain --untracked-files=no -- app Cargo.toml Cargo.lock extern THIRD_PARTY_LICENSES.md
$submodules = (git -C $repo submodule status) -join "`n"
$dirtyNote = if ($dirty) {
    "`nNOTE: this package was built from a working tree with uncommitted`nlocal modifications on top of the commit above.`n"
} else { '' }
@"
A2DP Windows Bridge (A2DPWB) $version ($Arch)

Source code
-----------
Repository: https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge
Commit:     $commit
$dirtyNote
Pinned submodules (git submodule status: commit, path, description):
$submodules

The complete corresponding source code of these programs and of every
library statically linked into them - including the LGPL-2.1-or-later
libopenaptx and the Fraunhofer FDK AAC codec - is available free of charge
at the repository URL and commit above.
It contains everything needed to rebuild and relink the programs (e.g. with
a modified version of libopenaptx):

    git clone --recursive https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge.git
    git checkout $commit
    git submodule update --init --recursive
    pwsh installer/build-release.ps1

See LICENSE, THIRD_PARTY_LICENSES.md and the licenses/ folder for the terms
of each component; licenses/rust-crates.txt lists the Rust crates.
"@ | Set-Content -Encoding utf8NoBOM (Join-Path $docDir 'SOURCE.txt')

# --- a2dpwb.exe: Tauri's configuration plus the updater key ---
$overlayPath = Join-Path $stage 'tauri.release.conf.json'
@{ plugins = @{ updater = @{ pubkey = $pubkey } } } | ConvertTo-Json -Depth 4 |
    Set-Content -Encoding utf8NoBOM $overlayPath
Push-Location $ui
try {
    Invoke-Checked 'tauri build' { pnpm tauri build --target $triple --no-bundle --config $overlayPath }
} finally {
    Pop-Location
}

# --- The package: the portable zip's folder, which the installer installs ---
$pkgName = "A2DPWB-$version"
$pkgDir = Join-Path $stage $pkgName
New-Item -ItemType Directory -Force $pkgDir | Out-Null
foreach ($name in 'a2dpwb', 'a2dpwb-core', 'a2dpwb-driver') {
    Copy-Item (Join-Path $release "$name.exe") $pkgDir
}
Copy-Item -Recurse (Join-Path $docDir '*') $pkgDir
New-Item -ItemType Directory -Force $OutDir | Out-Null
$zip = Join-Path $OutDir "$pkgName-$archName-portable.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path $pkgDir -DestinationPath $zip

# --- WebView2 bootstrapper (Evergreen), embedded for PCs without WebView2 ---
$cache = Join-Path $repo 'target/installer-cache'
$webView2Setup = Join-Path $cache 'MicrosoftEdgeWebview2Setup.exe'
if (-not (Test-Path -LiteralPath $webView2Setup)) {
    New-Item -ItemType Directory -Force $cache | Out-Null
    Invoke-WebRequest 'https://go.microsoft.com/fwlink/p/?LinkId=2124703' -OutFile "$webView2Setup.part"
    Move-Item -Force "$webView2Setup.part" $webView2Setup
}
$sig = Get-AuthenticodeSignature -LiteralPath $webView2Setup
if ($sig.Status -ne 'Valid' -or $sig.SignerCertificate.Subject -notmatch 'O=Microsoft Corporation') {
    throw "$webView2Setup is not signed by Microsoft ($($sig.Status)): delete it and build again"
}

# --- The installer ---
$setupName = "A2DPWB-$version-$archName-setup.exe"
$setup = Join-Path $OutDir $setupName
# VERSIONINFO takes numbers only: 2.0.2-beta.5 -> 2.0.2
$versionNumeric = $version -replace '[-+].*$', ''
Invoke-Checked 'ISCC' {
    & $iscc /Q "/DAppVersion=$version" "/DAppVersionNumeric=$versionNumeric" "/DArch=$Arch" `
        "/DPkgDir=$pkgDir" "/DWebView2Setup=$webView2Setup" "/DOutputDir=$OutDir" `
        "/DOutputBaseFilename=$([IO.Path]::GetFileNameWithoutExtension($setupName))" `
        (Join-Path $PSScriptRoot 'A2DPWB.iss')
}

# --- Updater signature of the installer; the version is part of what is
# signed (plugins.updater.requireSignedVersion) ---
if ($signing) {
    $key = $env:TAURI_SIGNING_PRIVATE_KEY
    if (Test-Path -LiteralPath $key -PathType Leaf) { $key = Get-Content -Raw -LiteralPath $key }
    $savedKey = $env:TAURI_SIGNING_PRIVATE_KEY
    $env:TAURI_SIGNING_PRIVATE_KEY = $key
    Push-Location $ui
    try {
        Invoke-Checked 'tauri signer sign' { pnpm tauri signer sign --app-version $version $setup }
    } finally {
        Pop-Location
        $env:TAURI_SIGNING_PRIVATE_KEY = $savedKey
    }
    if (-not (Test-Path -LiteralPath "$setup.sig")) { throw "no $setup.sig" }
}

# --- Symbols: the crash reports' minidumps (logs\crash) open with them ---
$pdbs = 'a2dpwb', 'a2dpwb_core', 'a2dpwb_driver' | ForEach-Object { Join-Path $release "$_.pdb" } |
    Where-Object { Test-Path $_ }
$symbols = Join-Path $OutDir "$pkgName-$archName-symbols.zip"
if (Test-Path $symbols) { Remove-Item -Force $symbols }
if ($pdbs) { Compress-Archive -Path $pdbs -DestinationPath $symbols }

# --- Updater manifest (one per release; arm64 is added to x64's) ---
if ($signing) {
    $manifestPath = Join-Path $OutDir 'latest.json'
    $manifest = $null
    if (Test-Path $manifestPath) {
        $manifest = Get-Content -Raw $manifestPath | ConvertFrom-Json -AsHashtable
        if ($manifest.version -ne $version) { $manifest = $null }
    }
    if (-not $manifest) {
        $notes = if ($NotesFile) { Get-Content -Raw $NotesFile } else { '' }
        $manifest = [ordered]@{
            version   = $version
            notes     = $notes
            pub_date  = (Get-Date).ToUniversalTime().ToString("yyyy-MM-dd'T'HH:mm:ss'Z'")
            platforms = [ordered]@{}
        }
    }
    $manifest.platforms[$platform] = [ordered]@{
        signature = (Get-Content -Raw "$setup.sig").Trim()
        url       = "$releaseUrl/v$version/$setupName"
    }
    $manifest | ConvertTo-Json -Depth 5 | Set-Content -Encoding utf8NoBOM $manifestPath
}

Write-Host ''
Write-Host 'Release files:'
Get-ChildItem $OutDir -File | Where-Object { $_.Name -like "A2DPWB-$version-*" -or $_.Name -eq 'latest.json' } |
    ForEach-Object { Write-Host ("  {0}  ({1:N1} MB)" -f $_.Name, ($_.Length / 1MB)) }
