# Third-Party Licenses

This document lists all third-party components in A2DP Windows Bridge (A2DPWB) 2.x,
along with their license information and compliance obligations.

> **Important — non-commercial use only for the binaries**
>
> The A2DPWB source code is dual-licensed under Apache-2.0 or MIT. The distributed
> `a2dpwb-core.exe` includes BTstack, and A2DPWB is distributed only as the set of its
> programs, so the release may only be used and redistributed for personal,
> non-commercial purposes. See [§4 BTstack](#4-btstack-bluetooth-stack).

---

## Summary

The C libraries are git submodules under `extern/`, at the commits pinned in this
repository. The Rust crates are pinned in `Cargo.lock`, the JavaScript packages in
`app/src/ui/pnpm-lock.yaml`.

| Component | Pinned version | License (SPDX) | Copyright | Usage | In |
|-----------|----------------|----------------|-----------|-------|----|
| libldac (AOSP) | `android-15.0.0_r36-4-geeee1a3` (commit `eeee1a3`) | Apache-2.0 | Sony Corporation | LDAC audio encoding | `a2dpwb-core.exe` |
| libopenaptx | 0.2.0 (tag `0.2.0`, commit `2459ed4`) | LGPL-2.1-or-later | Aurelien Jacobs, Pali Rohár | aptX / aptX HD / aptX LL audio encoding (decoding in `a2dpwb_decode`, §14) | `a2dpwb-core.exe` |
| fdk-aac | `v2.0.3-158-gd8e6b1a` (commit `d8e6b1a`) | FDK-AAC | Fraunhofer-Gesellschaft | AAC-LC audio encoding (decoding in `a2dpwb_decode`, §14) | `a2dpwb-core.exe` |
| BTstack | `v1.8.1-6-g5bc5cbdbe` (commit `5bc5cbdbe`) | LicenseRef-BTstack (BSD-3-Clause-style **with a non-commercial clause**) | BlueKitchen GmbH; Matthias Ringwald | User-mode Bluetooth stack | `a2dpwb-core.exe` |
| Bluedroid SBC encoder, ported to Rust | ported from BTstack `5bc5cbdbe` (`3rd-party/bluedroid/encoder/`) | Apache-2.0 | Broadcom Corporation | SBC audio encoding (`app/src/crates/codec/src/sbc/`) | `a2dpwb-core.exe` |
| rijndael (bundled in BTstack) | as in BTstack `5bc5cbdbe` (`3rd-party/rijndael/`) | Public domain | Philip J. Erdelsky | AES (used by BTstack) | `a2dpwb-core.exe` |
| tinydir (bundled in BTstack) | as in BTstack `5bc5cbdbe` (`3rd-party/tinydir/`) | BSD-2-Clause | tinydir authors | Directory listing in BTstack's Broadcom chipset driver | `a2dpwb-core.exe` |
| liblc3 (lc3-google, bundled in BTstack) | as in BTstack `5bc5cbdbe` (`3rd-party/lc3-google/`) | Apache-2.0 | Google LLC | LC3 encoding for the experimental LE Audio support (decoding in `a2dpwb_decode`, §14), §17 | `a2dpwb-core.exe` |
| micro-ecc (bundled in BTstack) | as in BTstack `5bc5cbdbe` (`3rd-party/micro-ecc/`) | BSD-2-Clause | Kenneth MacKay | P-256 for LE Secure Connections (experimental LE Audio), §18 | `a2dpwb-core.exe` |
| LHDC-V5-Encoder (C port of the AOSP LHDC V5 encoder) | commit `3f9d198` | Apache-2.0; `src/lhdcv5_fft.c` BSD-3-Clause | The Android Open Source Project; Mark Borgerding (KISS FFT) | LHDC V5 audio encoding, §8 | `a2dpwb-core.exe` |
| libopus | 1.5.2 (tag `v1.5.2`, commit `ddbe483`) | BSD-3-Clause | Xiph.Org, Skype Limited, Octasic, Jean-Marc Valin, Timothy B. Terriberry, CSIRO, Gregory Maxwell, Mark Borgerding, Erik de Castro Lopo, Mozilla, Amazon | Opus audio encoding (decoding in `a2dpwb_decode`, §14), §16 | `a2dpwb-core.exe` |
| aptx-adaptive (AKS-Logic/aptx-adaptive-linux, vendored) | upstream commit `25099004b2d3` (2026-10-22), crate 0.1.0 | MIT | Prateek Mittal (AKS-Logic) | aptX Adaptive encoding (experimental), §19 | `a2dpwb-core.exe` |
| HFreebuds (protocol description and test vectors) | commit `dab1f7e` | MIT | N2K02 | Huawei earbuds' sound quality mode (`app/src/crates/core/src/huawei.rs`), §20 | `a2dpwb-core.exe` |
| Rust crates (Tauri 2.12, tokio, serde, windows, nusb, wasapi, tray-icon, ...) | `Cargo.lock` | Apache-2.0, MIT and other permissive licenses; MPL-2.0 for a few (§9) | their authors | Everything written in Rust that is not A2DPWB's own | all three programs |
| SolidJS (`solid-js`) | 1.9.17 | MIT | Ryan Carniato | UI library of the window | `a2dpwb.exe` |
| `@tauri-apps/api` | 2.12.4 | Apache-2.0 OR MIT | Tauri Apps Contributors | JavaScript side of Tauri | `a2dpwb.exe` |
| Fluent UI System Icons | as copied into `app/src/ui/src/components/icons.ts` | MIT | Microsoft Corporation | Icons of the window | `a2dpwb.exe` |
| Microsoft Edge WebView2 Runtime | the one installed on the PC | Microsoft Software License Terms | Microsoft Corporation | Renders the window (not part of A2DPWB, §11) | — |
| Windows SDK / Windows APIs | (build machine) | Microsoft EULA | Microsoft Corporation | System APIs (WASAPI, WinUSB, SetupAPI, ...) | all three programs |

`a2dpwb-driver.exe` contains only Rust code (A2DPWB's own and the crates of §9).

---

## What ships in the release

The installer (`A2DPWB-<version>-win64-setup.exe`) installs, and the portable zip
(`A2DPWB-<version>-win64-portable.zip`) contains, the same files
(`installer/build-release.ps1` collects them):

| File | Contents |
|------|----------|
| `a2dpwb.exe` | The window (Tauri; SolidJS, `@tauri-apps/api` and the icons in its embedded page) |
| `a2dpwb-core.exe` | The resident core (BTstack and all codec libraries statically linked) |
| `a2dpwb-driver.exe` | The elevated WinUSB assignment helper |
| `LICENSE` | License notice for the A2DPWB source code (`Apache-2.0 OR MIT`) |
| `LICENSE-APACHE` | Apache License 2.0 text (A2DPWB source code option 1) |
| `LICENSE-MIT` | MIT License text (A2DPWB source code option 2) |
| `THIRD_PARTY_LICENSES.md` | This document |
| `SOURCE.txt` | Where to get the complete corresponding source: the A2DPWB commit and the exact commits of every submodule |
| `licenses/BTstack-LICENSE.txt` | BTstack license (`extern/btstack/LICENSE`) |
| `licenses/libldac-LICENSE.txt` | libldac license (Apache-2.0 text, `extern/libldac/LICENSE`) |
| `licenses/libldac-NOTICE.txt` | libldac NOTICE (`extern/libldac/NOTICE`) |
| `licenses/Apache-2.0.txt` | Apache License 2.0 text (covers the Bluedroid SBC encoder ported to Rust) |
| `licenses/liblc3-LICENSE.txt` | liblc3 license (Apache-2.0 text, `extern/btstack/3rd-party/lc3-google/LICENSE`) |
| `licenses/micro-ecc-LICENSE.txt` | micro-ecc BSD-2-Clause license (`extern/btstack/3rd-party/micro-ecc/LICENSE.txt`) |
| `licenses/fdk-aac-NOTICE.txt` | Fraunhofer FDK AAC Codec Library license (`extern/fdk-aac/NOTICE`) |
| `licenses/libopenaptx-LGPL-2.1.txt` | GNU LGPL 2.1 text for libopenaptx (`extern/libopenaptx/COPYING`) |
| `licenses/lhdcv5-enc-LICENSE.txt` | LHDC V5 encoder license (Apache-2.0 text, `extern/lhdcv5-enc/LICENSE`); its KISS FFT notice is in §8 of this document |
| `licenses/opus-COPYING.txt` | libopus license (BSD-3-Clause, `extern/opus/COPYING`) |
| `licenses/aptx-adaptive-LICENSE.txt` | aptX Adaptive encoder MIT license (`app/src/crates/aptx-adaptive/LICENSE`) |
| `licenses/fluentui-system-icons.txt` | Fluent UI System Icons MIT license (`app/src/ui/LICENSES/fluentui-system-icons.txt`) |
| `licenses/solid-js-LICENSE.txt` | SolidJS MIT license (from the `solid-js` package) |
| `licenses/tauri-apps-api-LICENSE-MIT.txt` | `@tauri-apps/api` MIT license (from the package; it is also available under Apache-2.0, `licenses/Apache-2.0.txt`) |
| `licenses/rust-crates.txt` | The license texts of all Rust crates in the programs, each followed by the crates it covers (generated by `cargo-about` from `Cargo.lock`) |

The tinydir (§7) and KISS FFT (§8) notices have no file of their own; their
license texts are reproduced in this document.

The installer itself is made by Tauri's NSIS bundler (NSIS, zlib/libpng-style
license) and embeds Microsoft's WebView2 bootstrapper, which it runs when the
WebView2 Runtime is missing (§11). Its uninstaller removes the files above.

Not distributed:

- **Realtek Bluetooth firmware** — proprietary Realtek binaries from the
  linux-firmware project ("Redistributable", LICENCE.rtlwifi_firmware.txt).
  Neither the programs, the installer nor this repository contain them. The
  user downloads the files: through the download page A2DPWB links to, or
  with **Download Firmware**, where the user's PC fetches them from
  kernel.org at a pinned linux-firmware commit after the user confirms
  (`app/src/crates/core/src/firmware.rs`). See
  [linux-firmware WHENCE](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/WHENCE).
- **Zadig** — not bundled; the documentation only links to <https://zadig.akeo.ie/>.
  A2DPWB assigns WinUSB with Windows' own SetupAPI and inbox `winusb.inf`, not
  with libwdi.
- **Microsoft Visual C++ runtime** — the programs link the MSVC runtime
  statically (§11), so no runtime DLL is shipped and the Visual C++
  Redistributable is not needed.
- **WebView2 Runtime** — installed by Windows or by the bootstrapper, not copied
  into A2DPWB's folder.

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
Technologies International, Ltd. (aptX Adaptive is encoded by a separate
encoder, §19.)
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
- **Path in project**: `extern/btstack/` (git submodule), built unmodified by
  `app/src/crates/btstack-sys/build.rs`: `src/` (without the audio, LC3, HID,
  SLIP and UART transport files), the classic profiles A2DP / AVDTP / AVRCP / SDP
  from `src/classic/`, the LE part `src/ble/` (SM, ATT / GATT client and server,
  LE device database; for the experimental LE Audio support), the Windows run
  loop and TLV store (`platform/windows/btstack_tlv_windows.c`), the Realtek,
  Broadcom and Intel chipset drivers, and `3rd-party/rijndael` (§6),
  `3rd-party/tinydir` (§7) and `3rd-party/micro-ecc` (§18).
  `btstack_config.h` and the C glue `csrc/shim.c` in that crate are A2DPWB's own.
  BTstack's HCI transport is A2DPWB's own Rust crate `hci-transport` (USB through
  `nusb`); BTstack's `hci_transport_h2_winusb.c` is not used
- **License**: BTstack License (BSD-3-Clause-style with a non-commercial clause);
  commercial licenses are available separately from BlueKitchen GmbH
- **Copyright**: Copyright (C) 2009 BlueKitchen GmbH (`LICENSE`; file headers carry
  BlueKitchen GmbH or Matthias Ringwald, with years from 2009 on)
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

**The A2DPWB source code is dual-licensed under Apache-2.0 or MIT. The distributed
`a2dpwb-core.exe` includes BTstack, and the window and the driver helper are only
distributed together with it, so the A2DPWB release may only be used and
redistributed for personal, non-commercial purposes.** Any commercial use requires
a commercial BTstack license from BlueKitchen GmbH.

### Obligations

- Use, redistribution and modification only for personal benefit, not for any
  commercial purpose or monetary gain (clause 4)
- Binary redistributions must reproduce the copyright notice, the list of
  conditions and the disclaimer in the documentation (shipped as
  `licenses/BTstack-LICENSE.txt`)
- Do not use the names of the copyright holders or contributors to endorse or
  promote derived products without written permission

---

## 5. Bluedroid SBC Encoder (ported to Rust)

- **Source**: `extern/btstack/3rd-party/bluedroid/encoder/` (originally from AOSP
  Bluedroid), as contained in BTstack commit `5bc5cbdbe`
- **Path in project**: `app/src/crates/codec/src/sbc/` — the fixed-point encoder
  translated from C to Rust. Each ported file carries the Broadcom copyright and
  the Apache-2.0 notice and states that it was ported
- **License**: Apache License 2.0 (per the C file headers)
- **Copyright**: Copyright (C) 1999-2012 Broadcom Corporation
- **SPDX**: `Apache-2.0`
- **Usage**: SBC encoding in `a2dpwb-core.exe`

The Rust port is a derivative work of the Apache-2.0 encoder, not of BTstack;
the BTstack license does not apply to it. The original C encoder is compiled
only by the test that compares the two bit for bit (`btstack-sys` feature
`sbc-reference`, a dev-dependency); it is not in the release programs. The
Bluedroid SBC **decoder** (Apache-2.0; Copyright (C) 2014 The Android Open Source
Project and Copyright 2002-2007 Open Interface North America, Inc.) is used only
by `a2dpwb_decode` (§14). No NOTICE file is present in `3rd-party/bluedroid/`.

### Obligations

- Include a copy of the Apache 2.0 license when distributing (shipped as
  `licenses/Apache-2.0.txt`)
- Retain all copyright, patent, trademark, and attribution notices
- Modified files carry prominent notices that they were changed (the port
  says so in each file)

---

## 6. rijndael (bundled in BTstack)

- **Source**: `extern/btstack/3rd-party/rijndael/` (from <http://www.efgh.com/software/rijndael.htm>)
- **License**: Public domain (per the `rijndael.c` header: "License: Public Domain")
- **Author**: Philip J. Erdelsky

No obligations.

---

## 7. tinydir (bundled in BTstack)

- **Source**: `extern/btstack/3rd-party/tinydir/tinydir.h` (header-only), as
  contained in BTstack commit `5bc5cbdbe`
- **License**: BSD 2-Clause (the header's license comment)
- **Copyright**: Copyright (c) 2013-2021, tinydir authors (Cong Xu, Lautis Sun,
  Baudouin Feildel, Andargor)
- **SPDX**: `BSD-2-Clause`
- **Usage**: BTstack's Broadcom chipset driver (`btstack_chipset_bcm.c`) lists
  the config folder with it to find a PatchRAM `.hcd` file

### License Text

The header ships no separate license file, so its notice is reproduced here for
the binary distribution:

```
Copyright (c) 2013-2021, tinydir authors:
- Cong Xu
- Lautis Sun
- Baudouin Feildel
- Andargor <andargor@yahoo.com>
All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this
   list of conditions and the following disclaimer.
2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND
ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR CONTRIBUTORS BE LIABLE FOR
ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
(INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;
LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND
ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

### Obligations

- Binary redistribution must reproduce the copyright notice, the conditions and
  the disclaimer (above)

---

## 8. LHDC-V5-Encoder (C port of the AOSP LHDC V5 encoder)

- **Source**: <https://github.com/WillyBilly06/LHDC-V5-Encoder>
- **Version**: commit `3f9d198` (2026-07-24; the repository's only release state)
- **Path in project**: `extern/lhdcv5-enc/` (git submodule)
- **Origin**: a C99 translation of the Rust LHDC V5 encoder Google added to
  AOSP for Android 17
  (`platform/packages/modules/Bluetooth`, branch `android17-release`,
  `system/audio/codecs/lhdcv5/`, Apache-2.0, Copyright (C) 2025 The Android
  Open Source Project). Before it was added here, the port was compiled with
  MSVC and compared with the AOSP Rust crate built from that branch:
  identical output over 114 configurations (44.1/48/96/192 kHz, 16/24-bit,
  bit rate indexes 0-13, several MTU / interval settings, mid-stream bit rate
  switches; about 18.5 MB of encoded data).
- **License**: Apache License 2.0 (`LICENSE`; file headers carry
  `SPDX-License-Identifier: Apache-2.0` and the AOSP copyright). The FFT,
  `src/lhdcv5_fft.c`, is derived from KISS FFT and carries
  `Copyright (c) 2003-2010, Mark Borgerding. All rights reserved.` with
  `SPDX-License-Identifier: BSD-3-Clause`.
- **SPDX**: `Apache-2.0 AND BSD-3-Clause`
- **Usage**: LHDC V5 audio encoding. Not linked into
  `a2dpwb_decode`.

### License Texts

The Apache 2.0 text is shipped as `licenses/lhdcv5-enc-LICENSE.txt`. The
port ships no separate notice file for its KISS FFT part, so the BSD-3-Clause
license it refers to is reproduced here for the binary distribution:

```
Copyright (c) 2003-2010, Mark Borgerding. All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice,
   this list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.

3. Neither the name of the copyright holder nor the names of its contributors
   may be used to endorse or promote products derived from this software
   without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE
LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
POSSIBILITY OF SUCH DAMAGE.
```

### Obligations

- Include a copy of the Apache 2.0 license when distributing (shipped as
  `licenses/lhdcv5-enc-LICENSE.txt`)
- Retain all copyright, patent, trademark, and attribution notices; the
  repository has no NOTICE file
- Reproduce the KISS FFT copyright notice and BSD-3-Clause conditions with
  binary distributions (above)
- Mark modified files with prominent notices if changes are made (A2DPWB
  compiles the sources unmodified)

### Trademark, Patents and Certification

LHDC is a codec of Savitech Corp., promoted by the Hi-Res Wireless Audio
(HWA) alliance. Apache-2.0 grants a patent license only for the
contributions of the code's licensors (here Google) and grants no trademark
rights; the port's README notes that codec licensing / certification
requirements should be checked before shipping a commercial product that
advertises LHDC support. A2DPWB is a free, non-commercial application and has
not been certified by Savitech or HWA. Its LHDC V5 support is an
interoperability feature.

---

## 9. Rust Crates

- **Where**: all three programs. The crates and their exact versions are in
  `Cargo.lock`; A2DPWB's own crates (`app/src/crates/*`, `app/src/core`,
  `app/src/ui/src-tauri`, `app/src/driver`) are `Apache-2.0 OR MIT` (§13)
- **License texts**: `licenses/rust-crates.txt`, generated for each release by
  [cargo-about](https://github.com/EmbarkStudios/cargo-about)
  (`installer/build-release.ps1` with `about.toml` and `installer/about.hbs`):
  every license text once, followed by the crates (name, version, repository)
  it covers. Build-only and development dependencies are left out, since they
  are not part of the programs
- **Accepted licenses** (`about.toml`, the same list as `deny.toml`; the build
  fails on any other): Apache-2.0, Apache-2.0 WITH LLVM-exception, MIT,
  BSD-2-Clause, BSD-3-Clause, ISC, Zlib, Unicode-3.0, MPL-2.0, BSL-1.0, CC0-1.0,
  CDLA-Permissive-2.0, Unlicense

Most crates are Apache-2.0 and / or MIT: among them Tauri 2.12 and its plugins
(single instance, updater, opener), tao, wry and webview2-com (the window),
tray-icon and muda (the tray), tokio, serde and serde_json, tracing, the
`windows` crates (Microsoft's Win32 bindings), `nusb` (USB), `wasapi` (audio
capture), rtrb (lock-free rings), ureq and rustls (the update check), semver,
ts-rs. Crates under other licenses:

| License | Crates (at the time of writing) | Note |
|---------|----------------------------------|------|
| Unicode-3.0 | `unicode-ident`, the ICU4X crates (`icu_*`, `zerovec`, ...) used by `idna` | Unicode data files license; notice in `rust-crates.txt` |
| ISC | `ring` (also Apache-2.0 parts), `untrusted`, `rustls-webpki` | Permissive |
| BSD-3-Clause | `brotli` (with MIT), `alloc-no-stdlib`, `alloc-stdlib`, `subtle` | Permissive |
| Zlib | `foldhash` | Permissive |
| CDLA-Permissive-2.0 | `webpki-roots` (Mozilla's list of root certificates, as data) | Permissive data license; the notice is in `rust-crates.txt` |
| MPL-2.0 | `cssparser`, `cssparser-macros`, `selectors`, `dtoa-short` (Tauri's HTML handling), `option-ext` (via `dirs`) | File-level copyleft, see below |

The table changes with `Cargo.lock`; `rust-crates.txt` of a release is the
authoritative list for that release.

### Obligations

- Ship the license texts and copyright notices of the crates with the
  binaries: `licenses/rust-crates.txt`
- **MPL-2.0** (§3.2): the source code of the MPL-covered files must be
  available to recipients of the binaries, and they must be told how to get it.
  A2DPWB uses these crates unmodified, at the versions in `Cargo.lock`; their
  source is on crates.io and in the repositories listed in `rust-crates.txt`,
  and `SOURCE.txt` points to the A2DPWB commit whose `Cargo.lock` pins them. The
  MPL does not extend to A2DPWB's own code

---

## 10. The Window's Page: SolidJS, @tauri-apps/api, Fluent UI System Icons

The window's page (`app/src/ui/src`, TypeScript, built by Vite into
`a2dpwb.exe`) bundles two npm packages and one icon set. The other packages in
`app/src/ui/package.json` (TypeScript, Vite, Vitest, the Tauri CLI, testing
libraries) are build and test tools and are not part of the programs.

| Component | Version | License | Copyright | Shipped as |
|-----------|---------|---------|-----------|------------|
| [SolidJS](https://github.com/solidjs/solid) (`solid-js`) | 1.9.17 | MIT | Copyright (c) 2016-2025 Ryan Carniato | `licenses/solid-js-LICENSE.txt` |
| [`@tauri-apps/api`](https://github.com/tauri-apps/tauri) | 2.12.4 | Apache-2.0 OR MIT (A2DPWB uses it under MIT) | Copyright (c) 2017 - Present Tauri Apps Contributors | `licenses/tauri-apps-api-LICENSE-MIT.txt` |
| [Fluent UI System Icons](https://github.com/microsoft/fluentui-system-icons) | the icons copied into `src/components/icons.ts` | MIT | Copyright (c) 2020 Microsoft Corporation | `licenses/fluentui-system-icons.txt`; also named in **Settings > About A2DPWB** |

### Obligations

- Include the MIT license and copyright notice with the binaries (shipped as
  listed above)

The application icon is A2DPWB's own (the v1 icon).

---

## 11. Microsoft Components: Windows APIs and WebView2

- **Provider**: Microsoft Corporation
- **License**: Microsoft Software License Terms (Windows SDK); the WebView2
  Runtime under its own Microsoft terms

The programs call Windows APIs through Microsoft's `windows` crates (§9) and
the MSVC toolchain's import libraries:

| API | Purpose |
|-----|---------|
| WASAPI, MMDevice, Audio Client (COM) | Audio capture (system, device and process loopback), endpoint volume |
| IPolicyConfig (undocumented COM interface) | Switching the default output device in Virtual Device mode |
| AVRT (MMCSS) | Audio thread priorities |
| WinUSB | HCI over USB (through `nusb`) |
| SetupAPI, Configuration Manager | USB adapter enumeration; assigning and restoring the WinUSB driver (`DiInstallDevice`, `DiUninstallDevice`) |
| WLAN API | The Wi-Fi networks for AFH |
| Power notifications | Sleep and resume |
| Registry | The **Start with Windows** value |
| SendInput | Headphone buttons as media keys |
| Named pipes, security (DACL), ShellExecuteEx | IPC with the window; elevation of the driver helper |
| Winsock | H4 over TCP to the emulator (development), the peer receiver test |
| WebView2 | Rendering the window (Tauri) |

The **Microsoft Edge WebView2 Runtime** renders the window. It is part of
Windows 11 and of most Windows 10 installations; A2DPWB does not ship it. The
installer embeds Microsoft's WebView2 bootstrapper
(`MicrosoftEdgeWebview2Setup.exe`, redistributable under Microsoft's WebView2
distribution terms) and runs it only when the runtime is missing.

All three programs link the Visual C++ runtime statically (`+crt-static` in
`.cargo/config.toml`; the C libraries are compiled with `/MT`), so they do not
need `VCRUNTIME140.dll` or the Visual C++ Redistributable. `a2dpwb-core.exe` and
`a2dpwb-driver.exe` also contain the Universal CRT; `a2dpwb.exe` uses Tauri's
static VC runtime with the Universal CRT that is part of Windows 10 and later.

### Obligations

- Windows SDK is licensed for use in developing Windows applications
- Distributed binaries must run on licensed copies of Windows
- No redistribution of SDK headers or libraries

---

## 12. Bluetooth Specifications

This project implements protocols defined in the following Bluetooth SIG
specifications. Implementation does not require licensing fees for open-source
projects, but commercial products must obtain Bluetooth qualification:

- **A2DP v1.2** — Advanced Audio Distribution Profile
- **AVDTP v1.3** — Audio/Video Distribution Transport Protocol
- **AVRCP** — Audio/Video Remote Control Profile
- **L2CAP** — Logical Link Control and Adaptation Protocol

### Bluetooth Qualification

Commercial products using Bluetooth must undergo the Bluetooth Qualification
Process (BQP) managed by the Bluetooth SIG. See <https://www.bluetooth.com/>
for details.

---

## 13. Project License

The A2DP Windows Bridge (A2DPWB) project code itself (excluding third-party libraries) is licensed
under **either the Apache License 2.0 or the MIT License, at your option**
(SPDX `Apache-2.0 OR MIT`). See [`LICENSE`](LICENSE),
[`LICENSE-APACHE`](LICENSE-APACHE) and [`LICENSE-MIT`](LICENSE-MIT) in the
project root. The Rust SBC encoder in `app/src/crates/codec/src/sbc/` is a port
of Apache-2.0 code and carries its notices (§5).

Versions up to and including v1.0.18-beta.2 were released under the MIT
License only; those releases remain available under MIT.

The distributed programs are combined works that also contain the components
listed above. Because `a2dpwb-core.exe` includes BTstack, the A2DPWB release may
only be used and redistributed for personal, non-commercial purposes.

---

## 14. Developer tool: a2dpwb_decode

`tools/a2dp_decode/` builds `a2dpwb_decode.exe`, which checks and decodes the
media stream recorded in an A2DPWB HCI capture. It is a developer tool:
**it is not part of the release** and is only built from source
(its own CMake project, `tools/a2dp_decode/CMakeLists.txt`).

Its own source is `Apache-2.0 OR MIT` (§13). It statically links only these libraries, at the
same pinned versions as above:

| Library | Pinned version | License (SPDX) | Parts used | License verified in the pinned source |
|---------|----------------|----------------|------------|---------------------------------------|
| Bluedroid SBC decoder | as in BTstack `5bc5cbdbe` | Apache-2.0 | `3rd-party/bluedroid/decoder` (CMake target `sbc_decoder`) | Every `decoder/srce/*.c` and `decoder/include/*.h` carries the Apache-2.0 header |
| fdk-aac | commit `d8e6b1a` | FDK-AAC | AAC decoder (`libAACdec`, `libMpegTPDec` and shared modules) | `NOTICE` covers "encoding and decoding"; every `libAACdec` / `libMpegTPDec` source carries the same license header |
| libopenaptx | 0.2.0 (commit `2459ed4`) | LGPL-2.1-or-later | `aptx_decode_sync()` | `COPYING` is LGPL 2.1; `openaptx.c` / `openaptx.h` headers state LGPL 2.1 or later |
| liblc3 (lc3-google) | as in BTstack `5bc5cbdbe` | Apache-2.0 | LC3 decoder for `--le-audio` (CMake target `lc3`, the whole library) | `LICENSE` is Apache-2.0; every `src/*.c` / `include/*.h` carries the Apache-2.0 header (Google LLC) |
| libopus | 1.5.2 (commit `ddbe483`) | BSD-3-Clause | Opus decoder (its own CMake build, `add_subdirectory(extern/opus)`, target `opus`) | `COPYING` is BSD-3-Clause; the `celt/`, `silk/` and `src/` sources carry the same BSD license header |

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
notice, relinking via the source at the same commit) and §5 (Apache-2.0
text, notices), §17 (liblc3: Apache-2.0 text, Copyright 2022 Google LLC) and
§16 (libopus: the BSD-3-Clause notice) apply, in the same way as for `a2dpwb-core.exe`.

---

## 15. Test environment: tools/emu and tools/linux_sink (Python)

`tools/emu/` runs `a2dpwb-core.exe --cli` against a virtual Bluetooth sink, and
`tools/linux_sink/` is a measuring A2DP receiver run on a second PC (see
[docs/building.md](docs/building.md) and [docs/usage.md](docs/usage.md)).
Both are test tooling only:

- **Nothing from them is linked into or shipped with the A2DPWB programs** or
  the release. A2DPWB only talks to them over the network: to the virtual
  controller over TCP (development CLI option `--hci-tcp`) and to the
  receiver's statistics over TCP / UDP (JSON).
- `tools/linux_sink/requirements.txt` (installed by `setup.sh`) holds the same pinned set for
  `tools/linux_sink/.venv` (git-ignored; verified: `pip freeze` of it equals
  the pinned set, Bumble 0.0.234 on Python 3.14 / Linux). On Linux,
  `a2dpwb_sink.py` also loads the distribution's libsbc, libfreeaptx and
  libfdk-aac at run time if installed (ctypes, see below); they are not part
  of this repository.
- The packages are **not included in this repository**. `setup_env.py`
  downloads them from PyPI into `tools/emu/.venv` (git-ignored), at the exact
  versions pinned in `tools/emu/requirements.txt`. Their licenses apply to
  that local environment.
- The repository's own tools/emu and tools/linux_sink scripts are
  `Apache-2.0 OR MIT` (§13). `emu_sink.py` and `emu_vhci.py` subclass Bumble's `Controller`, and
  `a2dpwb_sink.py` uses Bumble's API, without copying Bumble code.

This is the complete set that `setup_env.py` installs (`pip freeze` of
`tools/emu/.venv` equals `requirements.txt`). It follows Bumble 0.0.234's own
`Requires-Dist`; later Bumble versions have different dependencies.

| Package | Pinned version | License (SPDX) | Determined from | Required by |
|---------|----------------|----------------|-----------------|-------------|
| [Bumble](https://github.com/google/bumble) | 0.0.234 | Apache-2.0 | metadata `License-Expression`; Apache License 2.0 text in the wheel; sources state "Copyright Google LLC" | tools/emu (virtual controllers, link, A2DP sink), tools/linux_sink (host stack, A2DP sink) |
| cffi | 2.1.1 | MIT-0; its extension module statically contains **libffi** (MIT) \*\*\* | metadata `License-Expression`; libffi symbols defined in `_cffi_backend` (Linux `nm -D`: `T ffi_...`; Windows: libffi strings) | cryptography |
| click | 8.5.0 | BSD-3-Clause | metadata `License-Expression` | Bumble |
| cryptography | 50.0.1 | Apache-2.0 OR BSD-3-Clause; its Rust extension statically contains **OpenSSL 4.0.2** (Apache-2.0, as all OpenSSL since 3.0) \*\*\* | metadata `License-Expression`; OpenSSL version string in `_rust.pyd` (Windows) and `_rust.abi3.so` (Linux) | Bumble |
| importlib_resources | 7.1.0 | Apache-2.0 | metadata `License-Expression` | libusb-package |
| libusb-package | 1.0.26.4 | Apache-2.0 (Python code); the bundled `libusb-1.0.dll` (Windows) / `libusb-1.0.so` (Linux) is LGPL-2.1 per the package ("licensed with LGPLv2.1"; upstream libusb sources say version 2.1 or later) | metadata `License: Apache 2.0`; Apache LICENSE file in the wheel; "License" section of the package description | Bumble (USB transport, not used by tools/emu) |
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

\*\*\* Statically linked into the wheel's binary by its upstream build; the
wheel ships no license text of the bundled library. Checked in both the
Windows (tools/emu) and the Linux (tools/linux_sink) wheels of the pinned
versions.

#### Libraries tools/linux_sink loads at run time

`a2dpwb_sink.py` decodes the received audio with the Linux distribution's
shared libraries when they are installed (ctypes); without them it only
checks frame headers. They are neither in this repository nor shipped with
A2DPWB; the script works with whichever version the distribution provides.
Licenses were verified for these versions (Ubuntu 26.04):

| Library | Version checked | License (SPDX) | Determined from | Used for |
|---------|-----------------|----------------|-----------------|----------|
| libsbc (BlueZ SBC) | 2.1 (package 2.1-1build1, `libsbc.so.1.3.1`) | LGPL-2.1-or-later | `SPDX-License-Identifier` in all 17 library sources of the upstream sbc-2.1 tarball (kernel.org), COPYING.LIB. The Debian copyright's `Files: *` GPL-2+ covers the command line tools (sbcdec, sbcenc, ...), not the library | SBC decoding |
| libfreeaptx | 0.2.2 (package 0.2.2-1build1, `libfreeaptx.so.0.2.2`) | LGPL-2.1-or-later | headers of freeaptx.c / freeaptx.h in the upstream 0.2.2 tag; Debian copyright `Files: *` LGPL-2.1+ | aptX, aptX HD, aptX LL decoding |
| libfdk-aac | 2.0.2 (package 2.0.2-3~ubuntu5, multiverse; not installed on the test PC) | Fraunhofer FDK AAC license (LicenseRef-FDK-AAC) | Debian copyright `Files: *` License: Fraunhofer-FDK-AAC-for-Android (read from the downloaded package) | AAC decoding (optional) |

When changing `requirements.txt`, re-check and update this table.

---

## 16. libopus (Xiph.Org Opus Codec)

- **Source**: <https://github.com/xiph/opus> (mirror of <https://gitlab.xiph.org/xiph/opus>)
- **Version**: 1.5.2 (tag `v1.5.2`, commit `ddbe483`)
- **Path in project**: `extern/opus/` (git submodule)
- **License**: BSD-3-Clause (`COPYING`, shipped as `licenses/opus-COPYING.txt`)
- **SPDX**: `BSD-3-Clause`
- **Usage**: Opus audio encoding for Android's A2DP Opus vendor codec. The
  crate `opus-sys` compiles its floating-point C sources (the lists in its
  `*.mk` files; no DNN features) unmodified and links them statically into
  `a2dpwb-core.exe`. `a2dpwb_decode` links its decoder (§14).

### License Text

```
Copyright 2001-2023 Xiph.Org, Skype Limited, Octasic,
                    Jean-Marc Valin, Timothy B. Terriberry,
                    CSIRO, Gregory Maxwell, Mark Borgerding,
                    Erik de Castro Lopo, Mozilla, Amazon

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions
are met:

- Redistributions of source code must retain the above copyright
notice, this list of conditions and the following disclaimer.

- Redistributions in binary form must reproduce the above copyright
notice, this list of conditions and the following disclaimer in the
documentation and/or other materials provided with the distribution.

- Neither the name of Internet Society, IETF or IETF Trust, nor the
names of specific contributors, may be used to endorse or promote
products derived from this software without specific prior written
permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
``AS IS'' AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER
OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL,
EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO,
PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR
PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF
LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

### Obligations

- Reproduce the copyright notice, the conditions and the disclaimer with
  binary distributions (shipped as `licenses/opus-COPYING.txt`, and above)
- Do not use the names of the Internet Society, IETF, IETF Trust or the
  contributors to endorse or promote A2DPWB

### Patents

`COPYING` states that Opus is subject to royalty-free patent licenses from
the Xiph.Org Foundation, Microsoft Corporation and Broadcom Corporation
(IETF IPR declarations 1524, 1914 and 1526; `LICENSE_PLEASE_READ.txt` lists
further declarations). These licenses come with the conditions stated in the
declarations themselves (e.g. a defensive termination clause); A2DPWB adds
no terms of its own.

---

## 17. liblc3 (lc3-google, bundled in BTstack)

- **Source**: `extern/btstack/3rd-party/lc3-google/` (Google's liblc3, as
  vendored by BTstack)
- **Version**: as contained in BTstack commit `5bc5cbdbe`
- **License**: Apache License 2.0 (`LICENSE`, and the header of every source file)
- **Copyright**: Copyright 2022 Google LLC
- **SPDX**: `Apache-2.0`
- **Usage**: LC3 encoding for the experimental LE Audio support (off by
  default). The crate `lc3-sys` compiles all of `src/*.c` unmodified and links
  them statically into `a2dpwb-core.exe`; `src/lc3.c` is compiled through a
  generated wrapper that only drops its `dllexport` (so that the executable
  exports no `lc3_*` functions). BTstack's `btstack_lc3_google.c` glue is not
  used. LC3 decoding in `a2dpwb_decode` (§14)

No NOTICE file is present.

### Obligations

- Include a copy of the Apache 2.0 license when distributing (shipped as
  `licenses/liblc3-LICENSE.txt`)
- Retain all copyright, patent, trademark, and attribution notices
- Mark modified files with prominent notices if changes are made (none are)

LC3 is specified by the Bluetooth SIG for LE Audio; implementing it in a
Bluetooth product is covered by Bluetooth qualification (§12).

---

## 18. micro-ecc (bundled in BTstack)

- **Source**: `extern/btstack/3rd-party/micro-ecc/` (`uECC.c`)
- **Version**: as contained in BTstack commit `5bc5cbdbe`
- **License**: BSD 2-Clause (`LICENSE.txt`)
- **Copyright**: Copyright (c) 2014, Kenneth MacKay
- **SPDX**: `BSD-2-Clause`
- **Usage**: P-256 key generation and Diffie-Hellman for LE Secure
  Connections pairing (BTstack `ENABLE_MICRO_ECC_P256`), compiled unmodified by
  `app/src/crates/btstack-sys/build.rs` into `a2dpwb-core.exe`; used only by
  the experimental LE Audio support

### Obligations

- Binary redistribution must reproduce the copyright notice, the conditions
  and the disclaimer: shipped as `licenses/micro-ecc-LICENSE.txt`

---

## 19. aptx-adaptive (aptX Adaptive encoder from AKS-Logic/aptx-adaptive-linux)

- **Source**: <https://github.com/AKS-Logic/aptx-adaptive-linux>
- **Version**: upstream commit `25099004b2d3` (2026-10-22); crate version 0.1.0
- **Path in project**: `app/src/crates/aptx-adaptive/` (vendored copy of the
  upstream Rust encoder, with upstream's `LICENSE` and `LEGAL.md`)
- **License**: MIT (`LICENSE`, shipped as `licenses/aptx-adaptive-LICENSE.txt`)
- **Copyright**: Copyright (c) 2026 Prateek Mittal (AKS-Logic)
- **SPDX**: `MIT`
- **Usage**: aptX Adaptive encoding (experimental; R1 and R2 sinks, 48 kHz
  stereo), compiled as a Rust crate into `a2dpwb-core.exe`

### License Text

```
MIT License

Copyright (c) 2026 Prateek Mittal (AKS-Logic)

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

### Obligations

- Include the copyright notice and the permission notice with binary
  distributions (shipped as `licenses/aptx-adaptive-LICENSE.txt`, and above)

### Origin, Trademarks and Patents

Upstream describes the encoder as a clean-room reimplementation: its numeric
tables were reconstructed from lawfully obtained shipped Qualcomm binaries and
on-air captures (`LEGAL.md`, copied next to the crate). It contains no Qualcomm
code, SDK or binary. aptX and aptX Adaptive are trademarks of Qualcomm. The
codec may be covered by patents in some jurisdictions; as upstream's notice
says, users are responsible for whether its use is lawful where they are.

---

## 20. HFreebuds (Huawei earbuds protocol)

- **Source**: <https://github.com/N2K02/HFreebuds>
- **Version**: commit `dab1f7e`
- **Path in project**: `app/src/crates/core/src/huawei.rs`, written anew in Rust from
  HFreebuds' description of the protocol; its test vectors are HFreebuds' captures
- **License**: MIT
- **Copyright**: Copyright (c) 2026 N2K02

```text
Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

---

## License Compatibility Matrix

| Component | License | Compatible with Apache-2.0 / MIT? | Notes |
|-----------|---------|---------------------|-------|
| A2DPWB (project) | Apache-2.0 OR MIT | — | Project license (dual; user chooses) |
| libldac | Apache-2.0 | Yes | Permissive; NOTICE must accompany binaries |
| LHDC-V5-Encoder | Apache-2.0 AND BSD-3-Clause | Yes | Permissive; KISS FFT notice reproduced in §8 |
| libopus | BSD-3-Clause | Yes | Permissive; notice shipped as `licenses/opus-COPYING.txt` (§16) |
| aptx-adaptive | MIT | Yes | Permissive; notice shipped as `licenses/aptx-adaptive-LICENSE.txt` (§19) |
| Bluedroid SBC encoder (Rust port) | Apache-2.0 | Yes | Permissive |
| libopenaptx | LGPL-2.1+ | Yes (with care) | Must allow library replacement / relinking. Apache-2.0 is compatible with (L)GPL v3, which "or later" permits |
| fdk-aac | FDK AAC License | Yes | Source of fdk-aac must be offered free of charge; no patent license |
| BTstack | LicenseRef-BTstack | Source: yes / Binary: **restricts it** | Non-commercial clause 4 applies to the distributed `a2dpwb-core.exe` and with it the release; commercial license from BlueKitchen |
| rijndael | Public domain | Yes | — |
| tinydir | BSD-2-Clause | Yes | Permissive; notice reproduced in §7 |
| liblc3 | Apache-2.0 | Yes | Permissive (§17) |
| micro-ecc | BSD-2-Clause | Yes | Permissive; notice shipped as `licenses/micro-ecc-LICENSE.txt` (§18) |
| Rust crates | Apache-2.0, MIT, BSD, ISC, Zlib, Unicode-3.0, CDLA-Permissive-2.0, MPL-2.0 | Yes | Texts in `licenses/rust-crates.txt`; MPL-2.0 files stay under MPL (§9) |
| SolidJS, @tauri-apps/api, Fluent UI System Icons | MIT (Apache-2.0 OR MIT) | Yes | Permissive |
| Windows SDK, WebView2 | Proprietary | Yes (system components) | Platform dependency |

### LGPL Compliance Notes

**libopenaptx** (LGPL-2.1+) is the only LGPL component, and it is **statically
linked** into `a2dpwb-core.exe` (crate `openaptx-sys`). A2DPWB addresses
LGPL-2.1 §6 as follows:

1. **License text** — the installer and the portable zip ship the LGPL-2.1 text
   as `licenses/libopenaptx-LGPL-2.1.txt`, next to the programs.
2. **Prominent notice** — this document, shipped with every copy, states that
   libopenaptx (© 2017 Aurelien Jacobs, © 2018-2020 Pali Rohár) is used and
   that it is covered by the LGPL. **Settings > About A2DPWB** links to the
   third-party licenses.
3. **Relinking** — the complete corresponding source of A2DPWB (Apache-2.0 OR MIT) and of
   every library, including libopenaptx, is available from the A2DPWB GitHub
   repository at the release's commit together with its submodules and
   `Cargo.lock`. `SOURCE.txt` lists the exact commits. With it, a user can
   rebuild `a2dpwb-core.exe` against a modified libopenaptx (the `openaptx-sys`
   crate compiles `extern/libopenaptx/openaptx.c`). The source is offered from
   the same place (GitHub) as the binary, in the sense of LGPL-2.1 §6(d).
