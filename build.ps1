#Requires -Version 7.0
<#
.SYNOPSIS
One-step build of BlueCaster: the UI, then bluecaster-core.exe, bluecaster-driver.exe
and bluecaster.exe; optionally the checks, the decoder tool, the emulator test or
the release files.

.DESCRIPTION
Run from any folder (PowerShell 7):

    pwsh ./build.ps1                  # release profile: target/release/*.exe
    pwsh ./build.ps1 -Debug           # dev profile: target/debug/*.exe
    pwsh ./build.ps1 -Test            # build, then the checks of a pull request
    pwsh ./build.ps1 -Decoder         # build, and tools/a2dp_decode -> build/a2dp_decode
    pwsh ./build.ps1 -Emu             # build, the decoder, then tools/emu/run_test.py
    pwsh ./build.ps1 -Emu -Codecs sbc,ldac
    pwsh ./build.ps1 -Release -Arch x64   # installer/build-release.ps1 only

Switches combine (e.g. -Debug -Test -Emu); -Release takes only -Test and -Arch.

Steps:
  - The UI (app/src/ui): `pnpm install --frozen-lockfile` when node_modules
    is missing, then `pnpm build` (bluecaster.exe embeds app/src/ui/dist).
  - `cargo build [--release] -p bluecaster-core-exe -p bluecaster-driver -p bluecaster-ui`.
  - -Test: cargo fmt --check, clippy -D warnings, cargo test --workspace,
    tsc --noEmit and vitest run in app/src/ui, python tools/check_lang.py.
  - -Decoder (also -Emu): configures and builds tools/a2dp_decode (Release)
    in build/a2dp_decode with cmake from PATH, or else the CMake bundled with
    Visual Studio (found with vswhere).
  - -Emu: tools/emu/run_test.py with tools/emu/.venv (create it once with
    `python tools/emu/setup_env.py`) against the core just built, output
    in tools/emu/out; -Codecs sbc,ldac tests only these codecs (default: all).
  - -Release: only installer/build-release.ps1 -Arch <x64|arm64> (it builds
    everything itself; with -Test the checks run first).

Needs: Rust (rustup), Visual Studio 2022 or later with the C++ desktop
workload, Node.js 22 and pnpm, the submodules under extern/; Python for
-Test and -Emu. See docs/building.md.
#>
# No [CmdletBinding()]: it would add the common parameter -Debug
param(
    [switch]$Debug,
    [switch]$Test,
    [switch]$Decoder,
    [switch]$Emu,
    [switch]$Release,
    [ValidateSet('x64', 'arm64')]
    [string]$Arch = 'x64',
    [string[]]$Codecs = @()
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if ($args) { throw "unknown arguments: $args (see Get-Help $PSCommandPath)" }
if ($Release -and ($Debug -or $Decoder -or $Emu)) {
    throw '-Release builds by itself: it cannot be combined with -Debug, -Decoder or -Emu'
}
if ($Codecs -and -not $Emu) { throw '-Codecs is an option of -Emu' }

$repo = $PSScriptRoot
$ui = Join-Path $repo 'app/src/ui'

function Invoke-Checked([string]$what, [scriptblock]$cmd) {
    & $cmd
    if ($LASTEXITCODE -ne 0) { throw "$what failed (exit code $LASTEXITCODE)" }
}

function Invoke-InUi([string]$what, [scriptblock]$cmd) {
    Push-Location $ui
    try { Invoke-Checked $what $cmd } finally { Pop-Location }
}

function Write-Step([string]$text) {
    Write-Host ''
    Write-Host "==> $text" -ForegroundColor Cyan
}

# --- Prerequisites: fail before building anything ---
$missing = @()
if (-not (Get-Command cargo -ErrorAction Ignore)) {
    $missing += 'Rust (cargo) is not on PATH: install it with rustup (https://rustup.rs), then open a new terminal'
}
$node = Get-Command node -ErrorAction Ignore
if (-not $node) {
    $missing += 'Node.js is not on PATH: install Node.js 22 or later (https://nodejs.org)'
} else {
    $nodeVersion = (& node --version).Trim()
    if ($nodeVersion -match '^v(\d+)\.' -and [int]$Matches[1] -lt 22) {
        Write-Warning "Node.js $nodeVersion found; the UI is built and tested with Node.js 22 or later"
    }
}
if (-not (Get-Command pnpm -ErrorAction Ignore)) {
    $missing += 'pnpm is not on PATH: run "corepack enable" (comes with Node.js) or "npm install -g pnpm"'
}
$submodules = Select-String -Path (Join-Path $repo '.gitmodules') -Pattern '^\s*path\s*=\s*(.+?)\s*$' |
    ForEach-Object { $_.Matches[0].Groups[1].Value }
$emptySubmodules = @($submodules | Where-Object {
        $dir = Join-Path $repo $_
        -not (Test-Path -LiteralPath $dir -PathType Container) -or
        -not (Get-ChildItem -LiteralPath $dir -Force | Where-Object Name -ne '.git' | Select-Object -First 1)
    })
if ($emptySubmodules) {
    $missing += "submodules missing ($($emptySubmodules -join ', ')): run ""git submodule update --init --recursive"""
}
if ($Test -and -not (Get-Command python -ErrorAction Ignore)) {
    $missing += 'Python is not on PATH (needed by -Test for tools/check_lang.py): install Python 3.11 or later'
}
$emuPython = Join-Path $repo 'tools/emu/.venv/Scripts/python.exe'
if ($Emu -and -not (Test-Path -LiteralPath $emuPython)) {
    $missing += 'tools/emu/.venv is missing (needed by -Emu): run "python tools/emu/setup_env.py" once'
}
if ($missing) {
    $missing | ForEach-Object { Write-Host "error: $_" -ForegroundColor Red }
    throw 'missing prerequisites (see above)'
}

$cmake = $null
if ($Decoder -or $Emu) {
    $cmake = (Get-Command cmake -ErrorAction Ignore)?.Source
    if (-not $cmake) {
        $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
        if (Test-Path -LiteralPath $vswhere) {
            $cmake = & $vswhere -latest -prerelease -products * -find 'Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe' |
                Select-Object -First 1
        }
    }
    if (-not $cmake) {
        throw 'CMake not found (needed by -Decoder and -Emu): install it, or the Visual Studio component "C++ CMake tools for Windows"'
    }
}

# --- The UI's packages (the release script and the UI build need them) ---
if (-not (Test-Path -LiteralPath (Join-Path $ui 'node_modules'))) {
    Write-Step 'UI packages: pnpm install --frozen-lockfile'
    Invoke-InUi 'pnpm install' { pnpm install --frozen-lockfile }
}

$profileDir = if ($Debug) { 'debug' } else { 'release' }
$targetDir = $null
if (-not $Release) {
    # --- The UI, then the three programs ---
    Write-Step 'UI: pnpm build'
    Invoke-InUi 'pnpm build' { pnpm build }

    Write-Step "Programs: cargo build ($profileDir)"
    $cargoArgs = @('build', '--manifest-path', (Join-Path $repo 'Cargo.toml'),
        '-p', 'bluecaster-core-exe', '-p', 'bluecaster-driver', '-p', 'bluecaster-ui')
    if (-not $Debug) { $cargoArgs += '--release' }
    Invoke-Checked 'cargo build' { cargo @cargoArgs }

    $meta = cargo metadata --format-version 1 --no-deps --manifest-path (Join-Path $repo 'Cargo.toml') | ConvertFrom-Json
    $targetDir = Join-Path $meta.target_directory $profileDir
}

# --- The decoder tool (a CMake project of its own) ---
$decoderExe = Join-Path $repo 'build/a2dp_decode/Release/bluecaster_decode.exe'
if ($Decoder -or $Emu) {
    Write-Step "Decoder: tools/a2dp_decode -> build/a2dp_decode ($cmake)"
    $decoderBuild = Join-Path $repo 'build/a2dp_decode'
    Invoke-Checked 'cmake configure' { & $cmake -S (Join-Path $repo 'tools/a2dp_decode') -B $decoderBuild }
    Invoke-Checked 'cmake build' { & $cmake --build $decoderBuild --config Release --parallel }
}

# --- Checks (as CI and CONTRIBUTING.md) ---
if ($Test) {
    $manifest = Join-Path $repo 'Cargo.toml'
    if ($Release) {
        # The release script builds the UI later; clippy and the tests of
        # bluecaster-ui need app/src/ui/dist now
        Write-Step 'UI: pnpm build'
        Invoke-InUi 'pnpm build' { pnpm build }
    }
    Write-Step 'cargo fmt --check'
    Invoke-Checked 'cargo fmt' { cargo fmt --all --check --manifest-path $manifest }
    Write-Step 'cargo clippy -D warnings'
    Invoke-Checked 'cargo clippy' { cargo clippy --workspace --all-targets --manifest-path $manifest -- -D warnings }
    Write-Step 'cargo test --workspace'
    Invoke-Checked 'cargo test' { cargo test --workspace --manifest-path $manifest }
    Write-Step 'UI: tsc --noEmit'
    Invoke-InUi 'tsc' { pnpm exec tsc --noEmit }
    Write-Step 'UI: vitest run'
    Invoke-InUi 'vitest' { pnpm exec vitest run }
    Write-Step 'Language files: tools/check_lang.py'
    Invoke-Checked 'check_lang.py' { python (Join-Path $repo 'tools/check_lang.py') }
}

# --- Emulator end-to-end test ---
if ($Emu) {
    Write-Step 'Emulator test: tools/emu/run_test.py'
    $core = Join-Path $targetDir 'bluecaster-core.exe'
    # "sbc,ldac" arrives as one string with pwsh -File: split it here
    $more = @($Codecs -split '[,\s]+' | Where-Object { $_ })
    if ($more) { $more = @('--codecs') + $more }
    Invoke-Checked 'run_test.py' {
        & $emuPython (Join-Path $repo 'tools/emu/run_test.py') --bluecaster $core --decoder $decoderExe @more
    }
}

# --- Release files ---
if ($Release) {
    Write-Step "Release: installer/build-release.ps1 -Arch $Arch"
    & (Join-Path $repo 'installer/build-release.ps1') -Arch $Arch
    return
}

Write-Host ''
Write-Host "Programs ($profileDir):" -ForegroundColor Green
foreach ($name in 'bluecaster-core.exe', 'bluecaster.exe', 'bluecaster-driver.exe') {
    $exe = Get-Item -LiteralPath (Join-Path $targetDir $name)
    Write-Host ("  {0}  ({1:N1} MB)" -f $exe.FullName, ($exe.Length / 1MB))
}
if ($Decoder -or $Emu) {
    Write-Host "  $((Get-Item -LiteralPath $decoderExe).FullName)"
}
