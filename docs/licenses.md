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

## Third-Party Libraries

| Library | License | Usage |
|:--------|:--------|:------|
| [BTstack](https://github.com/bluekitchen/btstack) | BSD-3-Clause (non-commercial) / Commercial | User-mode Bluetooth stack |
| [libldac (AOSP)](https://android.googlesource.com/platform/external/libldac) | Apache-2.0 | LDAC encoder |
| [libopenaptx](https://github.com/pali/libopenaptx) | LGPL-2.1+ | aptX / aptX HD / aptX LL encoder |
| [fdk-aac](https://github.com/mstorsjo/fdk-aac) | FDK AAC License | AAC-LC encoder |
| [wxWidgets](https://www.wxwidgets.org/) | wxWindows Library Licence | GUI framework |
| [nlohmann/json](https://github.com/nlohmann/json) | MIT | JSON parser |

For full license texts and compliance details, see [THIRD_PARTY_LICENSES.md](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/THIRD_PARTY_LICENSES.md).

## Important Notes

### BTstack Dual License

BTstack is dual-licensed. The BSD-3-Clause license permits **non-commercial use** only. Commercial use requires a separate license from [BlueKitchen GmbH](https://bluekitchen-gmbh.com/).

### LDAC Certification

Commercial products using LDAC must complete Sony's certification process. See [sony.net/Products/LDAC/aosp](https://www.sony.net/Products/LDAC/aosp/).

### aptX Patent Notice

aptX (classic), aptX HD, aptX Low Latency and aptX Adaptive are trademarks of Qualcomm. The libopenaptx library is a clean-room implementation, used here for aptX, aptX HD and aptX Low Latency. aptX encoding may be covered by patents in some jurisdictions. A2DPWB does not include an aptX Adaptive encoder.

### AAC Patent Notice

AAC is covered by patent licenses managed by Via Licensing. Commercial products should obtain appropriate licenses.

### Bluetooth Qualification

Commercial Bluetooth products must undergo the [Bluetooth Qualification Process](https://www.bluetooth.com/) managed by the Bluetooth SIG.
