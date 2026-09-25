---
title: Building
layout: default
nav_order: 4
---

# Building from Source
{: .no_toc }

## Table of contents
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## Requirements

- **Visual Studio 2022 or later** with "Desktop development with C++" workload
- **CMake** 3.16+
- **Git** (for submodules and wxWidgets FetchContent)

## Quick Build

```bash
git clone --recursive https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge.git
cd A2DP-Windows-Bridge
cmake -B build -A x64
cmake --build build --config Release
```

The executable is output to `build/app/Release/A2DPWB.exe`.

{: .note }
The first build takes several minutes because CMake FetchContent downloads and compiles wxWidgets (v3.2.6).

## If You Already Cloned Without `--recursive`

```bash
git submodule update --init --recursive
```

## Project Structure

```
A2DP-Windows-Bridge/
├── app/                    Application source
│   ├── src/                C++ source files
│   ├── lang/               Localization (en.json, ja.json)
│   ├── resources/          Icon, manifest, resource script
│   └── CMakeLists.txt      App build config
├── compat/                 MSVC compatibility headers for AOSP code
├── docs/                   Documentation (this site)
├── extern/                 Third-party libraries (git submodules)
│   ├── btstack/            BTstack Bluetooth stack
│   ├── libldac/            AOSP LDAC encoder
│   ├── libopenaptx/        aptX / aptX HD / aptX LL encoder
│   ├── fdk-aac/            Fraunhofer AAC encoder
│   └── json/               nlohmann/json (header-only)
├── tools/
│   ├── a2dp_decode/        a2dpwb_decode: checks / decodes the media stream in an HCI capture (.pklg)
│   └── emu/                End-to-end test against a virtual Bluetooth sink (Python)
├── CMakeLists.txt          Root build config
└── build.bat               Build helper script
```

## Build Options

| CMake Option | Default | Description |
|:-------------|:--------|:------------|
| `LDAC_SOFT_FLOAT` | `OFF` | Use software floating point for libldac |
| `A2DPWB_BUILD_TOOLS` | `ON` | Build developer tools (`a2dpwb_decode`, see [Usage](usage#verify-stream)). Not part of the release package |

```bash
cmake -B build -A x64 -DA2DPWB_BUILD_TOOLS=OFF
```

## Testing Without Hardware (tools/emu)

`tools/emu` streams every codec from A2DPWB to a virtual Bluetooth sink on the same PC, with no adapter or headphones. It runs on Windows only and needs **Python 3.11 or later** in addition to the build requirements.

```bash
python tools/emu/setup_env.py     # once: creates tools/emu/.venv with pinned packages
python tools/emu/run_test.py      # after a Release build; about 35 s for all codecs
```

How it works:

- `emu_sink.py` runs two [Bumble](https://github.com/google/bumble) virtual controllers on an in-process link. One is served as H4 over TCP (`127.0.0.1`) for A2DPWB. The other is used by a Bumble A2DP sink that offers SBC, AAC, aptX, aptX HD, aptX LL and LDAC, and records its HCI traffic.
- `run_test.py` runs `A2DPWB.exe --cli` for each codec with development options: `--hci-tcp` (virtual controller instead of WinUSB), `--test-tone` (1 kHz left / 1.5 kHz right instead of system audio), `--duration`, `--hci-capture`. The environment variable `A2DPWB_CONFIG_DIR` points A2DPWB at a separate config folder, so pairing with the virtual sink never touches your real link keys.
- Both HCI captures are then checked with `a2dpwb_decode` (the sink side with `--received`).

A codec passes when all of these hold:

- the requested codec was negotiated
- `a2dpwb_decode` finds no problem on the sent side or the received side
- the sink received every media packet and frame A2DPWB sent, and at least 90% of the requested audio duration was sent
- except LDAC (no open-source decoder): the decoded audio is identical on both sides and is the test tone

| Option | Description |
|:-------|:------------|
| `--codecs sbc aac ...` | Test only these codecs (`sbc aac aptx aptxhd aptxll ldac`, default: all) |
| `--duration <s>` | Seconds of streaming per codec (default 5) |
| `--max-packet <bytes>` | Passed to A2DPWB (max media packet size, see [Usage](usage)) |
| `--build-dir`, `--config` | Where to find `A2DPWB.exe` / `a2dpwb_decode.exe` (default `build`, `Release`) |
| `--out <dir>` | Output folder (default `tools/emu/out`) |

For each codec, `tools/emu/out/<codec>/` holds the captures (`a2dpwb.pklg`, `sink.pklg`), logs, the `a2dpwb_decode` reports and the decoded WAV files. The exit code is 0 when every codec passed.

{: .note }
The virtual link has no radio: it checks encoding, packetization, AVDTP / L2CAP signalling and interoperability with an independent Bluetooth stack (Bumble), but not radio conditions, timing on real hardware or how real headphones play the audio. The Bumble packages are test tooling only and are not part of A2DPWB (see THIRD_PARTY_LICENSES.md §13).

## Dependencies

All dependencies are included as git submodules or fetched at build time. No manual installation required.

| Library | Method | Pinned version | License |
|:--------|:-------|:---------------|:--------|
| BTstack | git submodule | v1.8.1-6-g5bc5cbdbe | BTstack License (BSD-3-Clause-style with a non-commercial clause) |
| Bluedroid SBC codec | bundled in BTstack (`3rd-party/bluedroid`) | as in BTstack | Apache-2.0 |
| rijndael | bundled in BTstack (`3rd-party/rijndael`) | as in BTstack | Public domain |
| libldac (AOSP) | git submodule | android-15.0.0_r36-4-geeee1a3 | Apache-2.0 |
| libopenaptx | git submodule | 0.2.0 (do not update) | LGPL-2.1+ |
| fdk-aac | git submodule | v2.0.3-158-gd8e6b1a | FDK AAC License |
| nlohmann/json | git submodule (header-only) | 3.12.0 | MIT |
| wxWidgets | CMake FetchContent | v3.2.6 | wxWindows Library Licence 3.1 |
| zlib / libpng / nanosvg | built-in copies in wxWidgets | 1.2.13.1 / 1.6.37 / as in wxWidgets | zlib / PNG Reference Library License v2 / zlib |

{: .warning }
Because BTstack is linked in, any `A2DPWB.exe` you build may only be used and redistributed for personal, non-commercial purposes, even though the A2DPWB source code itself is MIT-licensed.

See [THIRD_PARTY_LICENSES.md](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/THIRD_PARTY_LICENSES.md) for full license details.
