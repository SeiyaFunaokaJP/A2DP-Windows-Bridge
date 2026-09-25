---
title: Licenses
layout: default
nav_order: 6
---

# Licenses
{: .no_toc }

## Table of contents
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## Project License

A2DP Windows Bridge (A2DPWB) is licensed under the **MIT License**. See [LICENSE](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/LICENSE).

{: .warning }
The A2DPWB source code is MIT-licensed. The distributed `A2DPWB.exe` includes BTstack and therefore may only be used and redistributed for personal, non-commercial purposes.

## Third-Party Libraries

| Library | Pinned version | License | Usage |
|:--------|:---------------|:--------|:------|
| [BTstack](https://github.com/bluekitchen/btstack) | v1.8.1-6-g5bc5cbdbe | BTstack License (BSD-3-Clause-style with a non-commercial clause) / Commercial | User-mode Bluetooth stack |
| Bluedroid SBC codec (bundled in BTstack) | as in BTstack | Apache-2.0 | SBC encoder |
| rijndael (bundled in BTstack) | as in BTstack | Public domain | AES (used by BTstack) |
| [libldac (AOSP)](https://android.googlesource.com/platform/external/libldac) | android-15.0.0_r36-4-geeee1a3 | Apache-2.0 | LDAC encoder |
| [libopenaptx](https://github.com/pali/libopenaptx) | 0.2.0 | LGPL-2.1+ | aptX / aptX HD / aptX LL encoder |
| [fdk-aac](https://github.com/mstorsjo/fdk-aac) | v2.0.3-158-gd8e6b1a | FDK AAC License | AAC-LC encoder |
| [wxWidgets](https://www.wxwidgets.org/) | 3.2.6 | wxWindows Library Licence 3.1 | GUI framework |
| zlib (built into wxWidgets) | 1.2.13.1 | zlib license | Compression (PNG support) |
| libpng (built into wxWidgets) | 1.6.37 | PNG Reference Library License v2 | PNG images |
| nanosvg (bundled in wxWidgets) | as in wxWidgets 3.2.6 | zlib license | SVG images |
| [nlohmann/json](https://github.com/nlohmann/json) | 3.12.0 | MIT | JSON parser |

For full license texts and compliance details, see [THIRD_PARTY_LICENSES.md](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/THIRD_PARTY_LICENSES.md). The release zip contains the license texts in its `licenses/` folder and a `SOURCE.txt` listing the exact source commits. Realtek firmware and Zadig are not distributed with A2DPWB (links only).

## Important Notes

### BTstack License (non-commercial)

BTstack is **not** BSD-3-Clause and is not an OSI-approved open-source license. Its license (SPDX `LicenseRef-BTstack`) has the three BSD-3-Clause conditions plus a fourth clause:

> 4. Any redistribution, use, or modification is done solely for personal benefit and not for any commercial purpose or for monetary gain.

Because `A2DPWB.exe` contains BTstack, this restriction applies to the distributed binary. Commercial use requires a commercial license from [BlueKitchen GmbH](https://bluekitchen-gmbh.com/) (contact@bluekitchen-gmbh.com).

### LDAC Certification

The libldac NOTICE states: "Taking the certification process is required to use LDAC in your products." See [sony.net/Products/LDAC/aosp](https://www.sony.net/Products/LDAC/aosp/). It has not been verified whether Sony's certification process applies to a free, non-commercial application such as A2DPWB; A2DPWB has not been certified.

### libopenaptx Version

libopenaptx is pinned to **0.2.0**, the last LGPL-2.1+ release. From 0.2.1 on it is licensed under GPL-3.0-or-later with additional restrictions, which is not compatible with the other libraries linked into A2DPWB. Releases up to v1.0.4 were mistakenly built with 0.2.1; please use v1.0.5 or later.

libopenaptx is statically linked. The release zip ships the LGPL-2.1 text, the About dialog shows the libopenaptx copyright and refers to the license texts, and the complete corresponding source (for rebuilding/relinking with a modified libopenaptx) is available from the repository at the release's tagged commit with its submodules.

### aptX Patent Notice

aptX (classic), aptX HD, aptX Low Latency and aptX Adaptive are trademarks of Qualcomm. According to its README, libopenaptx is an open-source aptX implementation "derived from ffmpeg 4.0 project"; it is used here for aptX, aptX HD and aptX Low Latency. aptX encoding may be covered by patents in some jurisdictions. A2DPWB does not include an aptX Adaptive encoder.

### fdk-aac License and AAC Patent Notice

The fdk-aac license requires that the complete license text accompany binary redistributions and that the complete source code of the FDK AAC Codec (and any modifications) be made available free of charge to recipients of the binary. It also grants **no patent license**: AAC may be covered by patents (the NOTICE refers to Via Licensing), and obtaining any required AAC patent license is the user's responsibility.

### Developer Tool a2dpwb_decode

`a2dpwb_decode` (see [Usage](usage#verify-stream)) is built from source only and is not in the release zip. It links the Bluedroid SBC decoder (Apache-2.0), fdk-aac (FDK AAC License) and libopenaptx 0.2.0 (LGPL-2.1+) at the pinned versions above, and does **not** link BTstack or libldac. See THIRD_PARTY_LICENSES.md §12.

### Bluetooth Qualification

Commercial Bluetooth products must undergo the [Bluetooth Qualification Process](https://www.bluetooth.com/) managed by the Bluetooth SIG.
