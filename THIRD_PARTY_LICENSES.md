# Third-Party Licenses

This document lists all third-party libraries used in A2DP Windows Bridge (A2DPWB),
along with their license information and compliance obligations.

> **Important — non-commercial use only for the binary**
>
> The A2DPWB source code is MIT-licensed. The distributed `A2DPWB.exe` includes
> BTstack and therefore may only be used and redistributed for personal,
> non-commercial purposes. See [§4 BTstack](#4-btstack-bluetooth-stack).

---

## Summary

All versions below are the ones pinned in this repository (git submodules under
`extern/`, or CMake FetchContent for wxWidgets).

| Library | Pinned version | License (SPDX) | Copyright | Usage |
|---------|----------------|----------------|-----------|-------|
| libldac (AOSP) | `android-15.0.0_r36-4-geeee1a3` (commit `eeee1a3`) | Apache-2.0 | Sony Corporation | LDAC audio encoding |
| libopenaptx | 0.2.0 (tag `0.2.0`, commit `2459ed4`) | LGPL-2.1-or-later | Aurelien Jacobs, Pali Rohár | aptX / aptX HD / aptX LL audio encoding (decoding in `a2dpwb_decode`, §12) |
| fdk-aac | `v2.0.3-158-gd8e6b1a` (commit `d8e6b1a`) | FDK-AAC | Fraunhofer-Gesellschaft | AAC-LC audio encoding (decoding in `a2dpwb_decode`, §12) |
| BTstack | `v1.8.1-6-g5bc5cbdbe` (commit `5bc5cbdbe`) | LicenseRef-BTstack (BSD-3-Clause-style **with a non-commercial clause**) | BlueKitchen GmbH | User-mode Bluetooth stack |
| Bluedroid SBC codec (bundled in BTstack) | as in BTstack `5bc5cbdbe` (`3rd-party/bluedroid/`) | Apache-2.0 | Broadcom Corporation; The Android Open Source Project; Open Interface North America, Inc. | SBC audio encoding (decoder also compiled; decoding in `a2dpwb_decode`, §12) |
| rijndael (bundled in BTstack) | as in BTstack `5bc5cbdbe` (`3rd-party/rijndael/`) | Public domain | Philip J. Erdelsky | AES (used by BTstack) |
| wxWidgets | v3.2.6 (FetchContent tag `v3.2.6`) | LGPL-2.0-or-later WITH WxWindows-exception-3.1 | Julian Smart, Robert Roebling et al | GUI framework |
| zlib (built-in copy in wxWidgets) | 1.2.13.1 (`1.2.13.1-motley`, from wxWidgets 3.2.6) | Zlib | Jean-loup Gailly and Mark Adler | Used by wxWidgets (PNG support) |
| libpng (built-in copy in wxWidgets) | 1.6.37 (from wxWidgets 3.2.6) | libpng-2.0 | The PNG Reference Library Authors et al. | PNG image loading in wxWidgets |
| nanosvg (bundled in wxWidgets) | as bundled in wxWidgets 3.2.6 (`3rdparty/nanosvg/`) | Zlib | Mikko Mononen | SVG rendering in wxWidgets |
| nlohmann/json | 3.12.0 (commit `3946872`) | MIT | Niels Lohmann | JSON parsing (settings, profiles, localization) |
| Windows SDK | (build machine) | Microsoft EULA | Microsoft Corporation | System APIs (WASAPI, COM, Bluetooth) |

---

## What ships in the release zip

The release zip contains:

| File | Contents |
|------|----------|
| `A2DPWB.exe` | The application (all libraries above statically linked) |
| `LICENSE` | MIT License for the A2DPWB source code |
| `THIRD_PARTY_LICENSES.md` | This document |
| `SOURCE.txt` | Where to get the complete corresponding source: the tagged A2DPWB commit and the exact commits of every submodule / fetched dependency |
| `licenses/BTstack-LICENSE.txt` | BTstack license (`extern/btstack/LICENSE`) |
| `licenses/libldac-LICENSE.txt` | libldac license (Apache-2.0 text, `extern/libldac/LICENSE`) |
| `licenses/libldac-NOTICE.txt` | libldac NOTICE (`extern/libldac/NOTICE`) |
| `licenses/Apache-2.0.txt` | Apache License 2.0 text (covers the Bluedroid SBC codec) |
| `licenses/fdk-aac-NOTICE.txt` | Fraunhofer FDK AAC Codec Library license (`extern/fdk-aac/NOTICE`) |
| `licenses/libopenaptx-LGPL-2.1.txt` | GNU LGPL 2.1 text for libopenaptx |
| `licenses/nlohmann-json-LICENSE.MIT.txt` | nlohmann/json MIT license (`extern/json/LICENSE.MIT`) |
| `licenses/wxWidgets-licence.txt` | wxWindows Library Licence 3.1 (`docs/licence.txt` in wxWidgets) |
| `licenses/wxWidgets-lgpl.txt` | GNU Library General Public License 2 (`docs/lgpl.txt` in wxWidgets) |
| `licenses/zlib.txt` | zlib license (`src/zlib/LICENSE` in wxWidgets) |
| `licenses/libpng-LICENSE.txt` | libpng license (`src/png/LICENSE` in wxWidgets) |
| `licenses/nanosvg-LICENSE.txt` | nanosvg license (`3rdparty/nanosvg/LICENSE.txt` in wxWidgets) |

Not distributed:

- **Realtek Bluetooth firmware** — proprietary Realtek binaries from the
  linux-firmware project. A2DPWB only links to the download page; the user
  downloads the files. See
  [linux-firmware WHENCE](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/WHENCE).
- **Zadig** — not bundled; the documentation only links to <https://zadig.akeo.ie/>.
- **Microsoft Visual C++ runtime** — `A2DPWB.exe` links the MSVC runtime
  dynamically; the runtime DLLs are not shipped in the zip (they are installed
  on Windows or via Microsoft's Visual C++ Redistributable).

---

## 1. libldac (AOSP LDAC Encoder)

- **Source**: <https://android.googlesource.com/platform/external/libldac>
- **Version**: `android-15.0.0_r36-4-geeee1a3` (commit `eeee1a3`)
- **Path in project**: `extern/libldac/` (git submodule)
- **License**: Apache License 2.0
- **Copyright**: Copyright (C) 2003-2017 Sony Corporation (file headers carry
  2003-2016, 2003-2017, 2013-2016 or 2013-2017)
- **SPDX**: `Apache-2.0`

### License Text

See `extern/libldac/LICENSE` for the full Apache 2.0 license text
(shipped as `licenses/libldac-LICENSE.txt`).

### NOTICE (Required by Apache 2.0 Section 4d)

Verbatim contents of `extern/libldac/NOTICE` (shipped as `licenses/libldac-NOTICE.txt`):

```
---------------
 Certification
---------------
   Taking the certification process is required to use LDAC in your products.
   For the detail of certification process, see the following URL:
      https://www.sony.net/Products/LDAC/aosp/
```

### Obligations

- Include a copy of the Apache 2.0 license when distributing
- Retain all copyright, patent, trademark, and attribution notices
- Include the NOTICE file contents with binary distributions
- Mark modified files with prominent notices if changes are made

### Certification

The NOTICE says "Taking the certification process is required to use LDAC in
your products." It does not distinguish commercial and non-commercial use.
**It has not been verified whether (or how) Sony's certification process
applies to a free, non-commercial application such as A2DPWB.** A2DPWB has not
gone through Sony's LDAC certification.

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
  and relink (see [LGPL Compliance Notes](#lgpl-compliance-notes))
- Give prominent notice that the library is used and that it is covered by the
  LGPL (LGPL-2.1 §6)

### Patent Notice

aptX, aptX HD, aptX Low Latency and aptX Adaptive are trademarks of Qualcomm
Technologies International, Ltd. A2DPWB does not include an aptX Adaptive
encoder.
According to its README, libopenaptx is an "Open Source implementation of Audio
Processing Technology codec (aptX) derived from ffmpeg 4.0 project". It is not
Qualcomm code. However, aptX encoding/decoding may be covered by patents in
some jurisdictions. Users should evaluate patent implications independently.

---

## 3. fdk-aac (Fraunhofer FDK AAC Codec Library)

- **Source**: <https://github.com/mstorsjo/fdk-aac>
- **Version**: `v2.0.3-158-gd8e6b1a` (commit `d8e6b1a`)
- **Path in project**: `extern/fdk-aac/` (git submodule)
- **License**: Software License for The Fraunhofer FDK AAC Codec Library for Android
  (`extern/fdk-aac/NOTICE`, shipped as `licenses/fdk-aac-NOTICE.txt`)
- **Copyright**: © Copyright 1995 - 2018 Fraunhofer-Gesellschaft zur Förderung der
  angewandten Forschung e.V.
- **SPDX**: `FDK-AAC`

### Obligations (section 2, "Copyright License")

Redistribution and use in source and binary forms are permitted without payment
of copyright license fees, provided that:

- The complete text of the license is retained in the documentation and/or other
  materials provided with binary redistributions (A2DPWB ships it as
  `licenses/fdk-aac-NOTICE.txt`)
- "You must make available free of charge copies of the complete source code of
  the FDK AAC Codec and your modifications thereto to recipients of copies in
  binary form." (The exact fdk-aac commit is listed in `SOURCE.txt` and is
  available from the A2DPWB repository's submodule at the release tag.)
- The name of Fraunhofer may not be used to endorse or promote products derived
  from the library without prior written permission
- No copyright license fees may be charged for anyone to use, copy or distribute
  the FDK AAC Codec or modifications thereto
- Modified versions must carry prominent notices stating the change and its date,
  and must be called "Third-Party Modified Version of the Fraunhofer FDK AAC
  Codec Library for Android" (A2DPWB does not modify fdk-aac)

### Patent Notice (section 3, "NO PATENT LICENSE")

The fdk-aac license grants **no** express or implied license to any patent
claims, including Fraunhofer's, and states: "You may use this FDK AAC Codec
software or modifications thereto only for purposes that are authorized by
appropriate patent licenses." A2DPWB does not hold an AAC patent license;
obtaining any required AAC patent license (e.g. via Via Licensing, as named in
the NOTICE, or from the patent owners) is the user's responsibility.

---

## 4. BTstack (Bluetooth Stack)

- **Source**: <https://github.com/bluekitchen/btstack>
- **Version**: `v1.8.1-6-g5bc5cbdbe` (commit `5bc5cbdbe`)
- **Path in project**: `extern/btstack/` (git submodule)
- **License**: BTstack License (BSD-3-Clause-style with a non-commercial clause);
  commercial licenses are available separately from BlueKitchen GmbH
- **Copyright**: Copyright (C) 2009 BlueKitchen GmbH
- **SPDX**: `LicenseRef-BTstack`

The BTstack license is **not** the BSD-3-Clause license and is not an
OSI-approved open-source license. Its clauses 1–3 match BSD-3-Clause, but it
adds clause 4 (quoted verbatim from `extern/btstack/LICENSE`):

> 4. Any redistribution, use, or modification is done solely for
>    personal benefit and not for any commercial purpose or for
>    monetary gain.

The license file ends with: "Please inquire about commercial licensing options
at contact@bluekitchen-gmbh.com".

### Consequence for A2DPWB

**The A2DPWB source code is MIT-licensed. The distributed `A2DPWB.exe` includes
BTstack and therefore may only be used and redistributed for personal,
non-commercial purposes.** Any commercial use requires a commercial BTstack
license from BlueKitchen GmbH.

### Obligations

- Use, redistribution and modification only for personal benefit, not for any
  commercial purpose or monetary gain (clause 4)
- Binary redistributions must reproduce the copyright notice, the list of
  conditions and the disclaimer in the documentation (shipped as
  `licenses/BTstack-LICENSE.txt`)
- Do not use the names of the copyright holders or contributors to endorse or
  promote derived products without written permission

---

## 5. Bluedroid SBC Codec (bundled in BTstack)

- **Source**: `extern/btstack/3rd-party/bluedroid/` (originally from AOSP Bluedroid)
- **Version**: as contained in BTstack commit `5bc5cbdbe`
- **License**: Apache License 2.0 (per file headers)
- **Copyright** (per file headers):
  - Copyright (C) 1999-2012 Broadcom Corporation (encoder)
  - Copyright (C) 2014 The Android Open Source Project (decoder)
  - Copyright 2002-2007 Open Interface North America, Inc. (decoder; individual
    headers state 2002-2004, 2003-2004 or 2006, and `oi_codec_version.c` states 2002-2007)
- **SPDX**: `Apache-2.0`
- **Usage**: SBC encoding; the decoder is also compiled because BTstack's SBC
  glue code references it

The Bluedroid SBC code is compiled into `A2DPWB.exe`. It is licensed under
Apache-2.0, not under the BTstack license. No NOTICE file is present in
`3rd-party/bluedroid/`.

### Obligations

- Include a copy of the Apache 2.0 license when distributing (shipped as
  `licenses/Apache-2.0.txt`)
- Retain all copyright, patent, trademark, and attribution notices
- Mark modified files with prominent notices if changes are made

---

## 6. rijndael (bundled in BTstack)

- **Source**: `extern/btstack/3rd-party/rijndael/` (from <http://www.efgh.com/software/rijndael.htm>)
- **License**: Public domain (per the `rijndael.c` header: "License: Public Domain")
- **Author**: Philip J. Erdelsky

No obligations.

---

## 7. wxWidgets (GUI Framework)

- **Source**: <https://github.com/wxWidgets/wxWidgets>
- **Version used**: v3.2.6 (fetched via CMake FetchContent at build time)
- **License**: wxWindows Library Licence, Version 3.1 (LGPL-2.0 with exception)
- **Copyright**: Copyright (c) 1998-2005 Julian Smart, Robert Roebling et al
  (as stated in wxWidgets `docs/licence.txt`)
- **SPDX**: `LGPL-2.0-or-later WITH WxWindows-exception-3.1`

### License Summary

The wxWindows Library Licence is the LGPL-2.0 with an additional exception
clause that permits distribution of works that use the library in binary form
under the user's own terms, without requiring the entire application to be
released under LGPL. This effectively makes it similar to a permissive license
for binary distribution.

### Obligations

- Include the wxWindows Library Licence and copyright notice in distributions
  (shipped as `licenses/wxWidgets-licence.txt` and `licenses/wxWidgets-lgpl.txt`)
- If wxWidgets source code is modified, modified source must be made available
  under the wxWindows Library Licence
- The exception clause means **no obligation to release application source code**
  when distributing binaries linked with wxWidgets

### Built-in libraries statically linked through wxWidgets

wxWidgets is built with its bundled copies of the following libraries, which
end up in `A2DPWB.exe`:

| Library | Version | License | Copyright | License file in wxWidgets 3.2.6 |
|---------|---------|---------|-----------|-------------------------------|
| zlib | 1.2.13.1 (`ZLIB_VERSION "1.2.13.1-motley"`) | zlib license (`Zlib`) | (C) 1995-2022 Jean-loup Gailly and Mark Adler | `src/zlib/LICENSE` |
| libpng | 1.6.37 | PNG Reference Library License version 2 (`libpng-2.0`); the LICENSE file also contains the libpng License version 1 for libpng 0.5 through 1.6.35 | Copyright (c) 1995-2019 The PNG Reference Library Authors; (c) 2018-2019 Cosmin Truta; (c) 2000-2002, 2004, 2006-2018 Glenn Randers-Pehrson; (c) 1996-1997 Andreas Dilger; (c) 1995-1996 Guy Eric Schalnat, Group 42, Inc. | `src/png/LICENSE` |
| nanosvg | as bundled in wxWidgets 3.2.6 | zlib license (`Zlib`) | Copyright (c) 2013-14 Mikko Mononen | `3rdparty/nanosvg/LICENSE.txt` |

All three licenses require that the copyright/license notice not be removed
from source distributions and that altered versions be marked as such; A2DPWB
ships each license text in `licenses/`.

---

## 8. nlohmann/json (JSON for Modern C++)

- **Source**: <https://github.com/nlohmann/json>
- **Path in project**: `extern/json/` (header-only, git submodule)
- **Version**: 3.12.0 (commit `3946872`; version per `json.hpp`)
- **License**: MIT License
- **Copyright**: Copyright (c) 2013-2026 Niels Lohmann
- **SPDX**: `MIT`

### Obligations

- Include the MIT license and copyright notice in distributions (shipped as
  `licenses/nlohmann-json-LICENSE.MIT.txt`)
- No other restrictions

---

## 9. Windows SDK / Windows APIs

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

## 10. Bluetooth Specifications

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

## 11. Project License

The A2DP Windows Bridge (A2DPWB) project code itself (excluding third-party libraries) is licensed
under the **MIT License**. See [`LICENSE`](LICENSE) in the project root.

The distributed `A2DPWB.exe` is a combined work that also contains the
libraries listed above. Because it includes BTstack, the binary may only be
used and redistributed for personal, non-commercial purposes.

---

## 12. Developer tool: a2dpwb_decode

`tools/a2dp_decode/` builds `a2dpwb_decode.exe`, which checks and decodes the
media stream recorded in an A2DPWB debug-mode HCI dump. It is a developer tool:
**it is not part of the release zip** and is only built from source
(CMake option `A2DPWB_BUILD_TOOLS`).

Its own source is MIT (§11). It statically links only these libraries, at the
same pinned versions as above:

| Library | Pinned version | License (SPDX) | Parts used | License verified in the pinned source |
|---------|----------------|----------------|------------|---------------------------------------|
| Bluedroid SBC decoder | as in BTstack `5bc5cbdbe` | Apache-2.0 | `3rd-party/bluedroid/decoder` (CMake target `sbc_decoder`) | Every `decoder/srce/*.c` and `decoder/include/*.h` carries the Apache-2.0 header |
| fdk-aac | commit `d8e6b1a` | FDK-AAC | AAC decoder (`libAACdec`, `libMpegTPDec` and shared modules) | `NOTICE` covers "encoding and decoding"; every `libAACdec` / `libMpegTPDec` source carries the same license header |
| libopenaptx | 0.2.0 (commit `2459ed4`) | LGPL-2.1-or-later | `aptx_decode_sync()` | `COPYING` is LGPL 2.1; `openaptx.c` / `openaptx.h` headers state LGPL 2.1 or later |

It does **not** link BTstack (HCI / L2CAP / AVDTP parsing is its own code),
so the BTstack non-commercial clause (§4) does not apply to it. The Bluedroid
SBC decoder is built as its own library (`sbc_decoder`), separate from
BTstack's SBC glue code, for this reason.

It does not link libldac either. LDAC frame headers are parsed by the tool's
own code, based on the frame header layout (sync word, sampling rate, channel
config, frame length, frame status). No libldac code is copied.

If you redistribute an `a2dpwb_decode.exe` binary, the obligations of §3
(fdk-aac: license text, source code offer, no patent license; AAC decoding
may also need a patent license), §2 (libopenaptx: LGPL-2.1 text, prominent
notice, relinking via the source at the same commit) and §5 (Apache-2.0 text,
notices) apply, in the same way as for `A2DPWB.exe`.

---

## 13. Test environment: tools/emu (Python)

`tools/emu/` runs A2DPWB against a virtual Bluetooth sink (see
[docs/building.md](docs/building.md)). It is test tooling only:

- **Nothing from it is linked into or shipped with `A2DPWB.exe`** or the
  release zip. A2DPWB only talks to the virtual controller over a local TCP
  socket (development CLI option `--hci-tcp`).
- The packages are **not included in this repository**. `setup_env.py`
  downloads them from PyPI into `tools/emu/.venv` (git-ignored), at the exact
  versions pinned in `tools/emu/requirements.txt`. Their licenses apply to
  that local environment.
- The repository's own tools/emu scripts are MIT (§11). `emu_sink.py`
  subclasses Bumble's `Controller` but does not copy Bumble code.

This is the complete set that `setup_env.py` installs (`pip freeze` of
`tools/emu/.venv` equals `requirements.txt`). It follows Bumble 0.0.234's own
`Requires-Dist`; later Bumble versions have different dependencies.

| Package | Pinned version | License (SPDX) | Determined from | Required by |
|---------|----------------|----------------|-----------------|-------------|
| [Bumble](https://github.com/google/bumble) | 0.0.234 | Apache-2.0 | metadata `License-Expression`; Apache License 2.0 text in the wheel; sources state "Copyright Google LLC" | tools/emu (virtual controllers, link, A2DP sink) |
| cffi | 2.1.1 | MIT-0 | metadata `License-Expression` | cryptography |
| click | 8.5.0 | BSD-3-Clause | metadata `License-Expression` | Bumble |
| cryptography | 50.0.1 | Apache-2.0 OR BSD-3-Clause | metadata `License-Expression` | Bumble |
| importlib_resources | 7.1.0 | Apache-2.0 | metadata `License-Expression` | libusb-package |
| libusb-package | 1.0.26.4 | Apache-2.0 (Python code); the bundled `libusb-1.0.dll` is LGPL-2.1 per the package ("licensed with LGPLv2.1"; upstream libusb sources say version 2.1 or later) | metadata `License: Apache 2.0`; Apache LICENSE file in the wheel; "License" section of the package description | Bumble (USB transport, not used by tools/emu) |
| libusb1 | 3.4.0 | LGPL-2.1-or-later | metadata `License-Expression`; COPYING / COPYING.LESSER in the wheel | Bumble (USB transport, not used by tools/emu) |
| platformdirs | 4.11.12 | MIT | metadata `License-Expression` | Bumble |
| prompt_toolkit | 3.0.53 | BSD-3-Clause \* | LICENSE file in the wheel (metadata: classifier "BSD License" only) | Bumble |
| pycparser | 3.0 | BSD-3-Clause | metadata `License-Expression` | cffi |
| pyee | 14.0.0 | MIT | metadata `License: MIT` | Bumble |
| pyserial | 3.5 | BSD \*\* | metadata `License: BSD` only; the wheel ships no license file | Bumble, pyserial-asyncio |
| pyserial-asyncio | 0.6 | BSD-3-Clause \* | LICENSE.txt in the wheel (metadata: `License: BSD`) | Bumble |
| pyusb | 1.3.1 | BSD-3-Clause \* | LICENSE file in the wheel (metadata: classifier "BSD License" only) | Bumble |
| typing_extensions | 4.16.0 | PSF-2.0 | metadata `License-Expression` | cryptography, pyee |
| wcwidth | 0.9.1 | MIT \* | LICENSE file in the wheel (metadata: classifier "MIT License" only) | prompt_toolkit |
| websockets | 17.1 | BSD-3-Clause | metadata `License-Expression` | Bumble |

\* The package metadata gives no SPDX expression; the SPDX identifier was
determined from the license text shipped in the wheel (BSD-3-Clause: the three
conditions including "Neither the name ..."; MIT: the MIT permission notice).

\*\* Not normalized to an SPDX identifier: the metadata only says "BSD" and
the wheel contains no license text. The upstream pySerial repository's
LICENSE.txt is BSD-3-Clause.

When changing `requirements.txt`, re-check and update this table.

---

## License Compatibility Matrix

| Component | License | Compatible with MIT? | Notes |
|-----------|---------|---------------------|-------|
| A2DPWB (project) | MIT | — | Project license |
| libldac | Apache-2.0 | Yes | Permissive; NOTICE must accompany binaries |
| Bluedroid SBC | Apache-2.0 | Yes | Permissive |
| libopenaptx | LGPL-2.1+ | Yes (with care) | Must allow library replacement / relinking |
| fdk-aac | FDK AAC License | Yes | Source of fdk-aac must be offered free of charge; no patent license |
| BTstack | LicenseRef-BTstack | Source: yes / Binary: **restricts it** | Non-commercial clause 4 applies to the whole distributed binary; commercial license from BlueKitchen |
| rijndael | Public domain | Yes | — |
| wxWidgets | wxWindows Lib Licence 3.1 | Yes | LGPL + exception (effectively permissive for binaries) |
| zlib / libpng / nanosvg | Zlib / libpng-2.0 / Zlib | Yes | Permissive |
| nlohmann/json | MIT | Yes | Same license |
| Windows SDK | Proprietary | Yes (system library) | Platform dependency |

### LGPL Compliance Notes

Both **libopenaptx** (LGPL-2.1+) and **wxWidgets** (LGPL-2.0 + exception) have
LGPL obligations. For wxWidgets, the exception clause removes the relinking
requirement for binary distributions.

libopenaptx is **statically linked** into `A2DPWB.exe`. A2DPWB addresses
LGPL-2.1 §6 as follows:

1. **License text** — the release zip ships the LGPL-2.1 text as
   `licenses/libopenaptx-LGPL-2.1.txt`.
2. **Prominent notice** — the About dialog shows the libopenaptx copyright
   (© 2017 Aurelien Jacobs, © 2018-2020 Pali Rohár) and its license, and refers
   to the license texts in the `licenses/` folder and to this document.
3. **Relinking** — the complete corresponding source of A2DPWB (MIT) and of
   every library, including libopenaptx, is available from the A2DPWB GitHub
   repository at the release's tagged commit together with its submodules.
   `SOURCE.txt` in the zip lists the exact commits. With it, a user can rebuild
   `A2DPWB.exe` against a modified libopenaptx. The source is offered from the
   same place (GitHub) as the binary, in the sense of LGPL-2.1 §6(d).
