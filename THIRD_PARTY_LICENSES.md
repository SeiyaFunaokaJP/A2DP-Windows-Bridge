# Third-Party Licenses

This document lists all third-party libraries used in A2DP Windows Bridge (A2DPWB),
along with their license information and compliance obligations.

---

## Summary

| Library | License | Copyright | Usage |
|---------|---------|-----------|-------|
| libldac (AOSP) | Apache-2.0 | Sony Corporation | LDAC audio encoding |
| libopenaptx 0.2.0 | LGPL-2.1+ | Aurelien Jacobs, Pali Rohár | aptX / aptX HD / aptX LL audio encoding |
| fdk-aac | FDK AAC License | Fraunhofer IIS | AAC-LC audio encoding |
| BTstack | BSD-3-Clause (dual) | BlueKitchen GmbH | User-mode Bluetooth stack (incl. SBC encoder) |
| wxWidgets | wxWindows Library Licence | wxWidgets Team | GUI framework |
| nlohmann/json | MIT | Niels Lohmann | JSON parsing (settings, profiles, localization) |
| Windows SDK | Microsoft EULA | Microsoft Corporation | System APIs (WASAPI, COM, Bluetooth) |

---

## 1. libldac (AOSP LDAC Encoder)

- **Source**: <https://android.googlesource.com/platform/external/libldac>
- **Path in project**: `extern/libldac/` (git submodule)
- **License**: Apache License 2.0
- **Copyright**: Copyright (C) 2003-2016 Sony Corporation
- **SPDX**: `Apache-2.0`

### License Text

See `extern/libldac/LICENSE` for the full Apache 2.0 license text.

### NOTICE (Required by Apache 2.0 Section 4d)

```
Certification
Taking the certification process is required to use LDAC in your products.
For the detail of certification process, see the following URL:
   https://www.sony.net/Products/LDAC/aosp/
```

### Obligations

- Include a copy of the Apache 2.0 license when distributing
- Retain all copyright, patent, trademark, and attribution notices
- Include the NOTICE file contents in distributions
- Mark modified files with prominent notices if changes are made
- **Product certification required** from Sony for commercial use of LDAC

---

## 2. libopenaptx (Open Source aptX / aptX HD Encoder)

- **Source**: <https://github.com/pali/libopenaptx>
- **Version**: 0.2.0 (tag `0.2.0`, commit `2459ed4`)
- **Path in project**: `extern/libopenaptx/` (git submodule)
- **License**: GNU Lesser General Public License v2.1 or later (LGPL-2.1+)
- **Copyright**: Copyright (C) 2017 Aurelien Jacobs, Copyright (C) 2018-2020 Pali Rohár
- **SPDX**: `LGPL-2.1-or-later`
- **Usage**: aptX, aptX HD and aptX Low Latency encoding

> **Do not update this submodule to 0.2.1 or later.** Starting with 0.2.1,
> libopenaptx is licensed under GPL-3.0-or-later with additional usage
> restrictions in its README. That is not compatible with the other libraries
> linked into A2DPWB (e.g. fdk-aac, BTstack). Version 0.2.0 is the last
> LGPL-2.1+ release; its encoder code is identical to 0.2.1 apart from the
> license headers. Releases up to and including v1.0.4 were mistakenly built
> with 0.2.1.

### Obligations

- The LGPL permits linking with proprietary software as a shared/static library
- If libopenaptx source code is modified, modified source must be made available
  under LGPL-2.1+
- Include a copy of the LGPL-2.1 license when distributing
- Users must be able to replace the libopenaptx library with their own version
  (dynamic linking satisfies this; for static linking, provide object files
  or source code to enable re-linking)
- Include prominent notice of use of LGPL-licensed library

### Patent Notice

aptX, aptX HD, aptX Low Latency and aptX Adaptive are trademarks of Qualcomm
Technologies International, Ltd. A2DPWB does not include an aptX Adaptive
encoder.
The libopenaptx library is a clean-room reverse-engineered implementation and
does not use any Qualcomm proprietary code. However, aptX encoding/decoding
may be covered by patents in some jurisdictions. Users should evaluate patent
implications independently.

---

## 3. fdk-aac (Fraunhofer FDK AAC Codec Library)

- **Source**: <https://github.com/mstorsjo/fdk-aac>
- **Path in project**: `extern/fdk-aac/` (git submodule)
- **License**: Fraunhofer FDK AAC Codec Library for Android (Software License)
- **Copyright**: Copyright (C) Fraunhofer-Gesellschaft zur Foerderung der angewandten Forschung e.V.
- **SPDX**: `FDK-AAC`

### Obligations

- May be used for non-commercial purposes and for development purposes
- Redistribution and use in source and binary forms permitted with conditions
- Include the license and copyright notice in distributions
- Modified versions must be clearly identified as such
- No use of Fraunhofer name to endorse derived products without permission

### Patent Notice

AAC is covered by patent licenses managed by Via Licensing. Commercial products
using AAC encoding should obtain appropriate patent licenses.

---

## 4. BTstack (Bluetooth Stack)

- **Source**: <https://github.com/bluekitchen/btstack>
- **Path in project**: `extern/btstack/` (git submodule)
- **License**: Dual-licensed: BSD-3-Clause (non-commercial) / Commercial
- **Copyright**: Copyright (C) BlueKitchen GmbH
- **SPDX**: `BSD-3-Clause`

BTstack bundles a **Bluedroid SBC encoder/decoder** (`3rd-party/bluedroid/`)
originally from AOSP, licensed under Apache-2.0.

### Obligations

- Non-commercial use is permitted under the BSD-3-Clause license
- Commercial use requires a separate license from BlueKitchen GmbH
- Include the BSD license and copyright notice in distributions
- Do not use the name "BlueKitchen" or "BTstack" to endorse derived products

---

## 5. wxWidgets (GUI Framework)

- **Source**: <https://github.com/wxWidgets/wxWidgets>
- **Version used**: v3.2.6 (fetched via CMake FetchContent at build time)
- **License**: wxWindows Library Licence (LGPL-2.0 with exception)
- **Copyright**: Copyright (C) 1992-2024 wxWidgets Team
- **SPDX**: `LGPL-2.0-or-later WITH WxWindows-exception-3.1`

### License Summary

The wxWindows Library Licence is the LGPL-2.0 with an additional exception
clause that permits distribution of works that use the library in binary form
under the user's own terms, without requiring the entire application to be
released under LGPL. This effectively makes it similar to a permissive license
for binary distribution.

### Obligations

- Include the wxWindows Library Licence and copyright notice in distributions
- If wxWidgets source code is modified, modified source must be made available
  under the wxWindows Library Licence
- The exception clause means **no obligation to release application source code**
  when distributing binaries linked with wxWidgets

---

## 6. nlohmann/json (JSON for Modern C++)

- **Source**: <https://github.com/nlohmann/json>
- **Path in project**: `extern/json/` (header-only, git submodule)
- **Version**: 3.11.3
- **License**: MIT License
- **Copyright**: Copyright (C) 2013-2023 Niels Lohmann
- **SPDX**: `MIT`

### Obligations

- Include the MIT license and copyright notice in distributions
- No other restrictions

---

## 7. Windows SDK / Windows APIs

- **Provider**: Microsoft Corporation
- **License**: Microsoft Software License Terms (included with Windows SDK)
- **Used APIs**:

| API | Header | Library | Purpose |
|-----|--------|---------|---------|
| COM | `windows.h` | `ole32.lib` | Component Object Model runtime |
| WASAPI | `audioclient.h`, `mmdeviceapi.h` | (COM-based) | Audio loopback capture |
| AVRT | `avrt.h` | `avrt.lib` | Multimedia thread scheduling |
| Bluetooth | `bluetoothapis.h`, `ws2bth.h` | `bthprops.lib`, `ws2_32.lib` | Device discovery |
| Property Store | `propsys.h` | `propsys.lib` | Device property access |
| SetupAPI | `setupapi.h` | `setupapi.lib` | USB device enumeration (Zadig) |
| Shell | `shellapi.h` | `shell32.lib` | System tray integration |
| DPI | `shellscalingapi.h` | `shcore.lib` | High-DPI awareness |
| UUID | `windows.h` | `uuid.lib` | COM interface UUIDs (`__uuidof`) |
| Version | `winver.h` | `version.lib` | `GetFileVersionInfo` (friendly app names) |
| Multimedia Timer | `timeapi.h` | `winmm.lib` | `timeBeginPeriod`/`timeEndPeriod` (timer resolution) |

### Obligations

- Windows SDK is licensed for use in developing Windows applications
- Distributed binaries must run on licensed copies of Windows
- No redistribution of SDK headers or libraries

---

## 8. Bluetooth Specifications

This project implements protocols defined in the following Bluetooth SIG
specifications. Implementation does not require licensing fees for open-source
projects, but commercial products must obtain Bluetooth qualification:

- **A2DP v1.2** — Advanced Audio Distribution Profile
- **AVDTP v1.3** — Audio/Video Distribution Transport Protocol
- **L2CAP** — Logical Link Control and Adaptation Protocol

### Bluetooth Qualification

Commercial products using Bluetooth must undergo the Bluetooth Qualification
Process (BQP) managed by the Bluetooth SIG. See <https://www.bluetooth.com/>
for details.

---

## 9. Project License

The A2DP Windows Bridge (A2DPWB) project code itself (excluding third-party libraries) is licensed
under the **MIT License**. See [`LICENSE`](LICENSE) in the project root.

---

## License Compatibility Matrix

| Component | License | Compatible with MIT? | Notes |
|-----------|---------|---------------------|-------|
| A2DPWB (project) | MIT | — | Project license |
| libldac | Apache-2.0 | Yes | Permissive |
| libopenaptx | LGPL-2.1+ | Yes (with care) | Must allow library replacement |
| fdk-aac | FDK AAC License | Yes | Permissive with conditions |
| BTstack | BSD-3-Clause | Yes | Non-commercial use |
| wxWidgets | wxWindows Lib Licence | Yes | LGPL + exception (effectively permissive for binaries) |
| nlohmann/json | MIT | Yes | Same license |
| Windows SDK | Proprietary | Yes (system library) | Platform dependency |

### LGPL Compliance Notes

Both **libopenaptx** (LGPL-2.1+) and **wxWidgets** (LGPL-2.0 + exception) have
LGPL obligations. For wxWidgets, the exception clause removes the relinking
requirement for binary distributions. For libopenaptx, to comply with LGPL-2.1+
when statically linking:

1. Provide the application object files (`.obj`) alongside the binary, OR
2. Build libopenaptx as a DLL (dynamic linking), OR
3. Provide the complete application source code

Since this project is open-source (MIT), option 3 is inherently satisfied.
