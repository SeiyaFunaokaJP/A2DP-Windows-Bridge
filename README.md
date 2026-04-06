# A2DPWB (A2DP Windows Bridge)

Bluetooth A2DP audio streaming for Windows with full codec support.
Streams system audio via LDAC, aptX HD, aptX Low Latency, AAC, or SBC using a USB Bluetooth adapter in WinUSB mode — no kernel driver or test signing required.

## Supported Codecs

| Codec | Bitrate | Sample Rate | Bit Depth | Latency |
|-------|---------|-------------|-----------|---------|
| LDAC | 330/660/990 kbps | 44.1–96 kHz | 16/24/32 bit | ~200 ms |
| aptX HD | 576 kbps | 44.1/48 kHz | 24 bit | ~150 ms |
| aptX Low Latency | 352 kbps | 44.1/48 kHz | 16 bit | ~32 ms |
| AAC | 128/192/256 kbps | 44.1/48 kHz | 16 bit | ~150 ms |
| SBC | up to ~345 kbps | 44.1/48 kHz | 16 bit | ~150 ms |

## Architecture

```
+-----------------------------------------------+
|  A2DPWB.exe                                   |
|                                               |
|  +- GUI (wxWidgets) ------+  +- CLI ---------+|
|  | Device / adapter select |  | --cli mode    ||
|  | Codec / quality config  |  | Same core     ||
|  | Profile management      |  |               ||
|  | Status display          |  |               ||
|  +-------------------------+  +---------------+|
|                                               |
|  +- Core -------------------------------------+|
|  | WASAPI Capture (Loopback / Virtual Device) ||
|  | Encoder (LDAC / aptX HD / aptX LL / AAC / SBC) |
|  | A2DP Service (connection lifecycle)        ||
|  | BTstack (HCI / L2CAP / AVDTP / A2DP)      ||
|  +--------------------------------------------+|
|                 WinUSB API                     |
+-----------------------+-----------------------+
                USB Bluetooth Adapter
```

### Components

| Directory | Description |
|-----------|-------------|
| `app/src/` | Application: GUI (wxWidgets) + CLI, audio capture, encoding, Bluetooth transport |
| `app/lang/` | Localization files (JSON, embedded at build time) |
| `app/resources/` | Application icon, manifest, resource script |
| `extern/btstack/` | BTstack — user-mode Bluetooth stack (git submodule) |
| `extern/libldac/` | AOSP libldac — LDAC encoder (git submodule) |
| `extern/libopenaptx/` | libopenaptx — aptX / aptX HD encoder (git submodule) |
| `extern/fdk-aac/` | Fraunhofer FDK AAC — AAC-LC encoder (git submodule) |
| `extern/json/` | nlohmann/json — JSON parser for settings, profiles, localization |

## Prerequisites

- **Windows 10/11** (x64)
- **Visual Studio 2022 or later** with C++ desktop workload
- **CMake** 3.16+
- **Git** (for submodules and wxWidgets FetchContent)
- **Dedicated USB Bluetooth adapter** (separate from built-in Bluetooth)
- **Zadig** (<https://zadig.akeo.ie/>) to install WinUSB driver on the adapter

## Setup

1. **Plug in a dedicated USB Bluetooth adapter** (keep built-in Bluetooth for normal Windows use)
2. **Install Zadig** from <https://zadig.akeo.ie/>
3. Open Zadig → Options → List All Devices
4. Select your USB Bluetooth adapter from the dropdown
5. Select **WinUSB** as the target driver and click **Replace Driver**
6. **Find your headphone's Bluetooth address** in Windows Settings → Bluetooth & devices → your device → Properties
7. Run `A2DPWB.exe` (GUI) or `A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF`

> **Warning**: Replacing the driver with WinUSB makes the adapter invisible to Windows Bluetooth. **Never replace the driver on your PC's built-in Bluetooth adapter** — doing so will disable all normal Bluetooth functionality (keyboards, mice, audio devices, etc.) and may require Device Manager or system recovery to restore. Always use a **separate, dedicated USB Bluetooth adapter** for A2DPWB.

### Firmware Files (Realtek adapters)

Realtek-based USB Bluetooth adapters (e.g., TP-Link UB500, RTL8761BU dongles) require proprietary firmware to operate. Intel and CSR adapters do not need this step.

**GUI (Guided):** Launch `A2DPWB.exe` and open the **Firmware** dialog. If firmware is missing, a warning is displayed with the required filenames. Use **Open Download Page** to open the [linux-firmware/rtl_bt](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/rtl_bt) page in your browser, and **Open Config Folder** to open the destination folder. Download the `.bin` files and place them in the config folder.

**Manual:**

1. Download the firmware and config `.bin` files for your chipset from <https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/rtl_bt> (e.g., `rtl8761bu_fw.bin` and `rtl8761bu_config.bin`)
2. Place both files in the A2DPWB config folder (`%APPDATA%\A2DPWB`, or the path shown in the Firmware dialog)

> **License**: These firmware files are proprietary Realtek binaries distributed via the linux-firmware project. They are not included in this repository. See [linux-firmware WHENCE](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/WHENCE) for redistribution terms.

## Building

```bash
git clone --recursive https://github.com/SeiyaFunaokaJP/A2DPWB.git
cd A2DPWB
cmake -B build -A x64
cmake --build build --config Release
```

The executable is output to `build/app/Release/A2DPWB-1.0.0.exe`.

> **Note**: The first build takes several minutes because CMake FetchContent downloads and compiles wxWidgets.

## Usage

### GUI Mode (default)

```
A2DPWB.exe
```

The graphical interface provides:
- Bluetooth adapter selection (WinUSB-attached adapters)
- Audio device selection (WASAPI loopback capture source)
- Codec selection (Auto / LDAC / aptX HD / aptX LL / AAC / SBC)
- LDAC quality mode (HQ 990 kbps / SQ 660 kbps / MQ 330 kbps)
- LDAC ABR (Adaptive Bit Rate) toggle
- Connection profile management (save / load device + codec settings)
- Real-time status display (codec, bitrate, connection state)

### CLI Mode

```bash
# Auto-select best codec
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF

# Specify codec
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxhd
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxll
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -c aac
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -c sbc

# LDAC quality modes
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q hq   # 990 kbps (default)
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q sq   # 660 kbps
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q mq   # 330 kbps

# Enable LDAC ABR (Adaptive Bit Rate)
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -a

# Virtual audio device capture (e.g. VB-CABLE)
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -m virtual --audio-device "{device-id}"

# Specify USB adapter path (when multiple adapters are connected)
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -u "\\?\usb#..."

# List paired Bluetooth audio devices
A2DPWB.exe --cli -l
```

## Features

- **Multi-codec support**: LDAC, aptX HD, aptX Low Latency, AAC, SBC with automatic negotiation
- **Two capture modes**: System loopback (all system audio) or virtual audio device (per-app routing via VB-CABLE etc.)
- **LDAC ABR**: Adaptive Bit Rate for unstable connections
- **Auto-reconnect**: Reconnects on Bluetooth disconnection (up to 10 attempts)
- **Profile management**: Save and load device + codec configurations
- **Localization**: English / Japanese UI
- **Realtek firmware**: Guided firmware download for Realtek adapters
- **MTU-aware framing**: Optimal packet utilization for each codec
- **PCM residual buffering**: Prevents audio data loss at encoder boundaries

## License

This project is licensed under the **MIT License**. See [LICENSE](LICENSE) for the full text.

Third-party libraries are used under their respective licenses. See [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md) for details.

> **BTstack** is dual-licensed: BSD-3-Clause for non-commercial use, commercial license available from BlueKitchen GmbH.

## References

- [BTstack](https://github.com/bluekitchen/btstack) — Open-source Bluetooth stack with WinUSB support
- [libldac (AOSP)](https://android.googlesource.com/platform/external/libldac) — LDAC encoder library
- [libopenaptx](https://github.com/pali/libopenaptx) — Open-source aptX / aptX HD encoder
- [fdk-aac](https://github.com/mstorsjo/fdk-aac) — Fraunhofer FDK AAC codec library
- [wxWidgets](https://www.wxwidgets.org/) — Cross-platform GUI library
- [nlohmann/json](https://github.com/nlohmann/json) — JSON for Modern C++
- [Zadig](https://zadig.akeo.ie/) — USB driver installer for WinUSB
