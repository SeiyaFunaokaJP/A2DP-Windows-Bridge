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
│   └── a2dp_decode/        a2dpwb_decode: checks / decodes the media stream in an HCI capture (.pklg)
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
