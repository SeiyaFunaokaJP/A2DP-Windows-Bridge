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
├── CMakeLists.txt          Root build config
└── build.bat               Build helper script
```

## Build Options

| CMake Option | Default | Description |
|:-------------|:--------|:------------|
| `BUILD_TESTS` | `ON` | Build test programs |
| `LDAC_SOFT_FLOAT` | `OFF` | Use software floating point for libldac |

```bash
cmake -B build -A x64 -DBUILD_TESTS=OFF
```

## Dependencies

All dependencies are included as git submodules or fetched at build time. No manual installation required.

| Library | Method | License |
|:--------|:-------|:--------|
| BTstack | git submodule | BSD-3-Clause (non-commercial) |
| libldac (AOSP) | git submodule | Apache-2.0 |
| libopenaptx | git submodule (pinned to 0.2.0) | LGPL-2.1+ |
| fdk-aac | git submodule | FDK AAC License |
| nlohmann/json | vendored (header-only) | MIT |
| wxWidgets v3.2.6 | CMake FetchContent | wxWindows Library Licence |

See [THIRD_PARTY_LICENSES.md](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/THIRD_PARTY_LICENSES.md) for full license details.
