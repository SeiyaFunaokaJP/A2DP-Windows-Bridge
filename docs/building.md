---
title: Building
layout: default
nav_order: 4
english_only: true
---

# Building from Source
{: .no_toc }

## Table of contents
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## Requirements

- **Rust** (stable, 1.90 or later) via [rustup](https://rustup.rs/)
- **Visual Studio 2022 or later** (or its Build Tools) with the "Desktop development with C++" workload: the C libraries (BTstack, the codecs) are compiled with MSVC
- **Node.js** 22 and **pnpm** for the UI (`corepack enable` provides pnpm)
- **Git** (for the submodules)
- **PowerShell 7** (`pwsh`) for `build.ps1` / `build.bat` (`winget install Microsoft.PowerShell`)
- For ARM64 builds: the Visual Studio components "MSVC v143 - VS 2022 C++ ARM64 build tools" and, on an x64 PC, "C++ Clang Compiler for Windows" (rustls' `ring` assembles its ARM64 code with clang; `installer/build-release.ps1` finds Visual Studio's clang itself), and `rustup target add aarch64-pc-windows-msvc`

## Quick Build

```bash
git clone --recursive https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge.git
cd A2DP-Windows-Bridge
pwsh ./build.ps1
```

Or double-click `build.bat` in Explorer: it runs `build.ps1` and waits for a key at the end (`build.bat -Test` passes the switches on). `build.ps1` first checks that Rust, Node.js, pnpm and the submodules are there, runs `pnpm install` when `node_modules` is missing, builds the window's page and then the three programs into `target/release/`: `a2dpwb-core.exe` (the resident core: Bluetooth, audio, tray), `a2dpwb.exe` (the window) and `a2dpwb-driver.exe` (the elevated helper that assigns / restores the WinUSB driver). Keep the three in one folder; `a2dpwb-core.exe --cli` streams without the window.

Switches: `-Debug` (dev profile, `target/debug/`), `-Test` (then the checks under Tests), `-Decoder` (also the decoder tool), `-Emu` (also the [emulator test](#emu)), `-Release -Arch x64|arm64` (only the release script, see Release Packages).

**Step by step** (what `build.ps1` does; the window's page must be built before `a2dpwb.exe`, which embeds it):

```bash
cargo build --release -p a2dpwb-core-exe -p a2dpwb-driver
cd app/src/ui
pnpm install --frozen-lockfile
pnpm build
cd ../../..
cargo build --release -p a2dpwb-ui
```

## If You Already Cloned Without `--recursive`

```bash
git submodule update --init --recursive
```

## Project Structure

```
A2DP-Windows-Bridge/
├── app/
│   ├── src/
│   │   ├── core/           a2dpwb-core.exe: resident core, tray, IPC server, --cli
│   │   ├── ui/             a2dpwb.exe: Tauri 2 + SolidJS (pnpm dev runs it in a browser with a mock core)
│   │   ├── driver/         a2dpwb-driver.exe: WinUSB assign / restore (elevated)
│   │   └── crates/         Libraries: btstack-sys, hci-transport, bt-host, codec, the codec -sys crates,
│   │                       pipeline, audio-capture, win-sys, core (a2dpwb-core), ipc, le-audio, lc3-sys,
│   │                       aptx-adaptive (aptX Adaptive encoder)
│   ├── lang/               Localization (en.json is the source; tools/check_lang.py checks the others)
│   └── resources/          Icon and manifest
├── Cargo.toml              Cargo workspace
├── installer/              Release script, Inno Setup script, updater notes (installer/README.md)
├── compat/                 MSVC compatibility headers for the AOSP code
├── docs/                   Documentation (this site) and the design document (v2-design.md)
├── extern/                 Third-party C libraries (git submodules)
│   ├── btstack/            BTstack Bluetooth stack
│   ├── libldac/            AOSP LDAC encoder
│   ├── libopenaptx/        aptX / aptX HD / aptX LL encoder
│   ├── fdk-aac/            Fraunhofer AAC encoder
│   ├── lhdcv5-enc/         LHDC V5 encoder
│   └── opus/               libopus (Opus encoder)
└── tools/
    ├── a2dp_decode/        a2dpwb_decode: checks / decodes the media stream in an HCI capture (.pklg)
    ├── emu/                End-to-end test against a virtual Bluetooth sink (Python)
    └── linux_sink/         a2dpwb_sink: measuring A2DP receiver on Linux (Python, Bumble)
```

## Tests

```bash
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cd app/src/ui && pnpm exec tsc --noEmit && pnpm exec vitest run
python tools/check_lang.py
```

## Release Packages

```powershell
pwsh installer/build-release.ps1 -Arch x64
```

writes the installer (`A2DPWB-<version>-win64-setup.exe`), the portable zip and the updater manifest to `target/dist`. It needs Inno Setup 6.5 or later (`winget install JRSoftware.InnoSetup`). Signing the in-app updates and the other details are in `installer/README.md`.

## Testing Without Hardware (tools/emu) {#emu}

`tools/emu` streams every codec from A2DPWB to a virtual Bluetooth sink on the same PC, with no adapter or headphones. It runs on Windows only and needs **Python 3.11 or later** and **CMake** (bundled with Visual Studio) for the decoder tool.

```bash
python tools/emu/setup_env.py                       # once: creates tools/emu/.venv with pinned packages
cmake -S tools/a2dp_decode -B build/a2dp_decode     # once: the decoder tool
cmake --build build/a2dp_decode --config Release
cargo build --release -p a2dpwb-core-exe
python tools/emu/run_test.py                        # every codec and negotiation case
```

How it works:

- `emu_sink.py` runs two [Bumble](https://github.com/google/bumble) virtual controllers on an in-process link. One is served as H4 over TCP (`127.0.0.1`) for A2DPWB. The other is used by a Bumble A2DP sink that offers SBC (SBC XQ capable), AAC, aptX, aptX HD, aptX LL, LDAC, LHDC V5, FastStream, Opus, Opus 05 and aptX Adaptive, and records its HCI traffic.
- `run_test.py` runs `a2dpwb-core.exe --cli` for each codec with development options: `--hci-tcp` (virtual controller instead of WinUSB), `--test-tone` (1 kHz left / 1.5 kHz right instead of system audio), `--duration`, `--hci-capture`. The environment variable `A2DPWB_CONFIG_DIR` points A2DPWB at a separate config folder, so pairing with the virtual sink never touches your real link keys.
- Both HCI captures are then checked with `a2dpwb_decode` (the sink side with `--received`).

A codec passes when all of these hold:

- the requested codec was negotiated
- `a2dpwb_decode` finds no problem on the sent side or the received side
- the sink received every media packet and frame A2DPWB sent, and at least 90% of the requested audio duration was sent
- except LDAC, LHDC V5 and aptX Adaptive (no open-source decoder): the decoded audio is identical on both sides and is the test tone

| Option | Description |
|:-------|:------------|
| `--codecs sbc aac ...` | Test only these codecs (`sbc sbcxq aac aptx aptxhd aptxll ldac lhdcv5 faststream opus opus05`) or negotiation cases (sample rates, low bitpools, SBC frame setups, aptX Adaptive, ...; `-h` lists them); default: all |
| `--le-audio [case ...]` | Run the LE Audio (experimental) cases instead (default: all of them) |
| `--scan [case ...]` | Run the device scan cases instead (`eir`, `name-request`) |
| `--duration <s>` | Seconds of streaming per codec (default 5) |
| `--max-packet <bytes>` | Passed to A2DPWB (max media packet size, see [Usage](usage)) |
| `--a2dpwb <exe>` | The core to test (default `target/release/a2dpwb-core.exe`) |
| `--decoder <exe>` | The decoder tool (default `<build-dir>/a2dp_decode/<config>/a2dpwb_decode.exe`) |
| `--build-dir <dir>`, `--config <cfg>` | Where the decoder tool was built (default `build`, `Release`) |
| `--base-port <port>` | First TCP port (default 9101; LE Audio cases use +100); change it to run several tests at once |
| `--out <dir>` | Output folder (default `tools/emu/out`) |

For each codec, `tools/emu/out/<codec>/` holds the captures (`a2dpwb.pklg`, `sink.pklg`), logs, the `a2dpwb_decode` reports and the decoded WAV files. The exit code is 0 when every codec passed.

{: .note }
The virtual link has no radio: it checks encoding, packetization, AVDTP / L2CAP signalling and interoperability with an independent Bluetooth stack (Bumble), but not radio conditions, timing on real hardware or how real headphones play the audio. The Bumble packages are test tooling only and are not part of A2DPWB (see THIRD_PARTY_LICENSES.md).

`emu_vhci.py` does the same towards the Linux Bluetooth stack: on Linux it attaches a virtual adapter to BlueZ through `/dev/vhci`, which the measuring receiver `tools/linux_sink` then takes like a real adapter - it can be tested without hardware, optionally with lost or held-back media packets (`--drop`, `--stall`). See `tools/linux_sink/README.md`.

## Dependencies

The C libraries are git submodules; Rust crates and the UI's npm packages are fetched by Cargo and pnpm (versions pinned in `Cargo.lock` and `app/src/ui/pnpm-lock.yaml`; `cargo deny check` checks their licences).

| Library | Method | Pinned version | License |
|:--------|:-------|:---------------|:--------|
| BTstack | git submodule | v1.8.1-6-g5bc5cbdbe | BTstack License (BSD-3-Clause-style with a non-commercial clause) |
| Bluedroid SBC codec | encoder ported to Rust; the decoder (tests, tools) from BTstack's copy | as in BTstack | Apache-2.0 |
| rijndael | bundled in BTstack (`3rd-party/rijndael`) | as in BTstack | Public domain |
| liblc3 (lc3-google) | bundled in BTstack (`3rd-party/lc3-google`) | as in BTstack | Apache-2.0 |
| micro-ecc | bundled in BTstack (`3rd-party/micro-ecc`) | as in BTstack | BSD-2-Clause |
| libldac (AOSP) | git submodule | android-15.0.0_r36-4-geeee1a3 | Apache-2.0 |
| libopenaptx | git submodule | 0.2.0 (do not update) | LGPL-2.1+ |
| fdk-aac | git submodule | v2.0.3-158-gd8e6b1a | FDK AAC License |
| LHDC V5 encoder | git submodule | as pinned | Apache-2.0 |
| libopus | git submodule | v1.5.2 | BSD-3-Clause |
| Tauri, SolidJS, Rust crates | Cargo / pnpm | `Cargo.lock`, `pnpm-lock.yaml` | MIT / Apache-2.0 and others (`licenses/rust-crates.txt` in the release) |

{: .warning }
Because BTstack is linked in, any A2DPWB you build may only be used and redistributed for personal, non-commercial purposes, even though the A2DPWB source code itself is dual-licensed under Apache-2.0 or MIT.

See [THIRD_PARTY_LICENSES.md](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/THIRD_PARTY_LICENSES.md) for full license details.
