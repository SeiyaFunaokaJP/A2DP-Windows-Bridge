# A2DP Windows Bridge (A2DPWB)

Bluetooth A2DP audio streaming for Windows with the codecs Windows lacks: LDAC, aptX HD, aptX Low Latency, LHDC V5 and more, through a USB Bluetooth adapter it takes over in WinUSB mode — no kernel driver or test signing required.

![A2DPWB streaming LDAC 990 kbps / 96 kHz to Sony WH-1000XM4 (Home screen)](docs/images/app_sample.png)

**[Download](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/releases/latest)** (installer `-setup.exe` or `-portable.zip`) | **[Documentation](https://seiyafunaokajp.github.io/A2DP-Windows-Bridge/)**

## Codecs

| Codec | Bitrate | Sample rate | Bit depth | Windows built-in | A2DPWB |
|-------|---------|-------------|-----------|:----------------:|:------:|
| LHDC V5 | 400–1000 kbps, ABR 160–400 | 44.1–192 kHz | 16/24 bit | ❌ | ✅ |
| LDAC | 330/660/990 kbps | 44.1–96 kHz | 16/24/32 bit | ❌ | ✅ |
| aptX Adaptive | 265/427 kbps (96 kHz: 646) | 48/96 kHz | 24 bit | ❌ | ✅ ¹ |
| Opus (Android) | 256 kbps | 48 kHz | 16 bit | ❌ | 🧪 |
| aptX HD | 576 kbps | 44.1/48 kHz | 24 bit | ❌ | 🧪 |
| aptX | 352/384 kbps | 44.1/48 kHz | 16 bit | ✅ | ✅ |
| aptX Low Latency | 352 kbps | 44.1/48 kHz | 16 bit | ❌ | 🧪 |
| AAC | up to 256 kbps | 44.1/48 kHz | 16 bit | Windows 11 | ✅ |
| SBC XQ | up to 552 kbps | 44.1/48 kHz | 16 bit | ❌ | ✅ |
| SBC | up to ~345 kbps | 44.1/48 kHz | 16 bit | ✅ | ✅ |
| FastStream | 198/216 kbps | 44.1/48 kHz | 16 bit | ❌ | 🧪 ² |
| Opus 05 (PipeWire) | 256 kbps | 48 kHz | 16 bit | ❌ | 🧪 ² ³ |
| LC3 (LE Audio) | 32–124 kbps per channel | 16–48 kHz | 16 bit | Windows 11 ⁴ | 🧪 ⁵ |

✅ supported · 🧪 experimental: passes the encoder tests and the end-to-end test against a virtual Bluetooth sink, but not yet confirmed with real headphones that use it ([reports welcome](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/11)) · ❌ not supported. The app offers every codec, the experimental ones too.

¹ Encoded with the MIT-licensed encoder of [AKS-Logic/aptx-adaptive-linux](https://github.com/AKS-Logic/aptx-adaptive-linux), whose tables its authors reconstructed from shipped binaries; confirmed on an EarFun Air Pro 4+ at 48 kHz, and 96 kHz on real headphones ([#86](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/86)). ² Auto never picks it; choose it on **Codecs**. ³ Only PipeWire (Linux) receivers implement it. ⁴ Only on PCs whose Bluetooth hardware and driver support LE Audio. ⁵ LE Audio (BAP unicast over isochronous channels), not A2DP: chosen per device (its LE Audio entry on **Codecs**), and needs an adapter with Bluetooth 5.2 isochronous channels; gaming headsets that support GMAP get its low-latency configuration (7.5/10 ms frames, 64–96 kbps per channel, ~30 ms). Tested only against the Bluetooth emulator ([details](https://seiyafunaokajp.github.io/A2DP-Windows-Bridge/usage#le-audio)).

Auto picks LHDC V5 > LDAC > aptX Adaptive > Opus > aptX HD > aptX > aptX LL > AAC > SBC XQ > SBC. Per-codec details, the Ubuntu / PipeWire comparison and the verified hardware are on the [documentation home](https://seiyafunaokajp.github.io/A2DP-Windows-Bridge/). Third-party apps that work with A2DPWB (not supported by this project): [Works With](https://seiyafunaokajp.github.io/A2DP-Windows-Bridge/works-with).

## Requirements

- **Windows 10 (22H2) or 11**, x64 or ARM64 (ARM64 and Windows 10 not yet tried on real hardware)
- **A USB Bluetooth adapter for A2DPWB**: a separate USB dongle is recommended. The PC's built-in adapter works too when it is a USB device (many laptops' Realtek adapters), but Windows then has no Bluetooth of its own while A2DPWB has it. See [Recommended Adapters](https://seiyafunaokajp.github.io/A2DP-Windows-Bridge/setup#adapters) and the [verified hardware](https://seiyafunaokajp.github.io/A2DP-Windows-Bridge/hardware)

### Why does A2DPWB take over a whole adapter?

Windows 10 / 11 load only kernel drivers signed through Microsoft, which needs an EV code signing certificate (several hundred US dollars a year, normally for companies only). A2DPWB therefore takes a whole USB adapter away from Windows with the Microsoft-signed WinUSB driver and runs the Bluetooth stack itself, in user mode. If you just want something simple, commercial software with a signed driver works with the built-in Bluetooth.

## Setup

1. **Install**: run `A2DPWB-<version>-win64-setup.exe` (per user, no administrator rights), or extract the portable zip and run `a2dpwb.exe`. Installing over v1 removes v1 and keeps its settings, pairings and firmware.
2. **Plug in the adapter** (or use the built-in one, see the warning below).
3. **Follow the first-run setup**: choose the adapter, **Assign WinUSB** (Windows asks for administrator permission), add the firmware files for Realtek adapters, then press **Connect** next to your audio device: A2DPWB picks the best settings for it and connects.
4. **Connect** from **Home**, or from the tray icon.

> **Warning:** While WinUSB is assigned, Windows' own Bluetooth no longer sees the adapter. A built-in adapter can be used too, but **the PC then has no Windows Bluetooth at all** until the driver is restored, so a separate USB adapter is recommended. **Adapters > Restore original driver** gives the adapter back to Windows at any time, and the uninstaller offers it too.

**Realtek adapters** need firmware files such as `rtl8761bu_fw.bin` and `rtl8761bu_config.bin` from [linux-firmware](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/rtl_bt) (proprietary, not included). The **Adapter** screen shows which files are needed; **Download Firmware** fetches them from kernel.org into the settings folder after you confirm, or open the download page and the settings folder to do it by hand. See the [Setup guide](https://seiyafunaokajp.github.io/A2DP-Windows-Bridge/setup).

## Usage

The resident core streams and owns the tray icon; the window only shows its state, so closing it does not stop the audio. **Left-click** the tray icon for the flyout, **right-click** it for the menu (your devices, **Disconnect**, **Exit**).

The core also runs from the command line (quit A2DPWB first: only one program can use the adapter):

```bash
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF                    # Auto codec
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q hq -a   # LDAC 990 kbps with ABR
a2dpwb-core.exe --cli --scan                                  # Find nearby devices
a2dpwb-core.exe --cli -h                                      # All options
```

The sample rate is 48 kHz unless you choose another in a codec's settings on **Codecs** (96 kHz for LDAC, for example). Everything else — codecs, capture modes, volume sync, link quality and AFH, diagnostics, start with Windows, troubleshooting — is in the [Usage guide](https://seiyafunaokajp.github.io/A2DP-Windows-Bridge/usage).

## Features

- **Codecs**: LHDC V5, LDAC, aptX Adaptive, experimental Opus, the rest of the aptX family, AAC and SBC XQ / SBC, with automatic choice and ABR (LHDC V5, LDAC, aptX Adaptive; adaptive bitpool for SBC / SBC XQ)
- **Capture**: all system audio, a virtual audio device (e.g. VB-CABLE), or one application
- **Resident core with tray**: streams with the window closed; reconnects after link loss and after sleep
- **WinUSB in the app**: assigns and restores the adapter's driver, no Zadig needed
- **Headphone controls**: volume sync and play / pause / next / previous (AVRCP)
- **Diagnostics**: link quality, AFH (avoids your Wi-Fi channels), connection flow, log, HCI capture
- **Updates**: daily check, in-app update of the installed version
- **8 languages**: English, Japanese, Chinese (Simplified / Traditional), Korean, German, French, Spanish
- **LE Audio (experimental)**: LC3 to LE Audio headphones and earbuds, tested only against the Bluetooth emulator ([details](https://seiyafunaokajp.github.io/A2DP-Windows-Bridge/usage#le-audio))

## Building

Requirements: **Rust** 1.90+, **Visual Studio 2022+** with the C++ desktop workload, **Node.js 22** with **pnpm** (`corepack enable`), **Git**, **PowerShell 7**.

```bash
git clone --recursive https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge.git
cd A2DP-Windows-Bridge
pwsh ./build.ps1
```

`build.ps1` builds the window's page and the three programs into `target/release/` (`a2dpwb-core.exe`, `a2dpwb.exe`, `a2dpwb-driver.exe`); double-clicking `build.bat` runs the same. Switches: `-Debug` (debug build), `-Test` (formatting, lints, tests), `-Decoder` (the `a2dpwb_decode` tool), `-Emu` (end-to-end test against a virtual sink; `-Codecs` picks codecs), `-Release -Arch x64|arm64` (installer, portable zip). See [Building](https://seiyafunaokajp.github.io/A2DP-Windows-Bridge/building) for the steps one by one, the tests and the release packages, and [Architecture](https://seiyafunaokajp.github.io/A2DP-Windows-Bridge/architecture) for how the programs fit together.

## License

The A2DPWB source code is dual-licensed under the **Apache License 2.0** ([LICENSE-APACHE](LICENSE-APACHE)) or the **MIT License** ([LICENSE-MIT](LICENSE-MIT)), at your option. See [LICENSE](LICENSE).

> **Important:** The distributed programs include [BTstack](https://github.com/bluekitchen/btstack), whose license allows use and redistribution only for **personal, non-commercial purposes** ("solely for personal benefit and not for any commercial purpose or for monetary gain"). Commercial licenses are available from BlueKitchen GmbH.

Third-party libraries (BTstack, libldac, libopenaptx, fdk-aac, the LHDC V5 encoder, libopus, Tauri, SolidJS and others) are used under their own licenses: see [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).
