---
title: Home
layout: default
nav_order: 1
---

# A2DP Windows Bridge (A2DPWB)

Bluetooth A2DP audio streaming for Windows with full codec support.
{: .fs-6 .fw-300 }

Streams system audio via **LDAC, aptX HD, aptX Low Latency, aptX, AAC, SBC** using a USB Bluetooth adapter in WinUSB mode -- no kernel driver or test signing required.

[Get Started](setup){: .btn .btn-primary .fs-5 .mb-4 .mb-md-0 .mr-2 }
[GitHub](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge){: .btn .fs-5 .mb-4 .mb-md-0 }

---

## Supported Codecs

| Codec | Bitrate | Sample Rate | Bit Depth | Latency |
|:------|:--------|:------------|:----------|:--------|
| LDAC | 330/660/990 kbps | 44.1--96 kHz | 16/24/32 bit | ~200 ms |
| aptX HD | 576 kbps | 44.1/48 kHz | 24 bit | ~150 ms |
| aptX Low Latency | 352 kbps | 44.1/48 kHz | 16 bit | ~32 ms |
| aptX | 352/384 kbps | 44.1/48 kHz | 16 bit | -- |
| AAC | 128/192/256 kbps | 44.1/48 kHz | 16 bit | ~150 ms |
| SBC | up to ~345 kbps | 44.1/48 kHz | 16 bit | ~150 ms |

### Codec Support by Platform (Sending to Headphones)

| Codec | Windows 10 built-in | Windows 11 built-in | Ubuntu built-in (PipeWire) | **A2DPWB** (Windows 10 / 11) |
|:------|:-------------------:|:-------------------:|:--------------------------:|:----------------------------:|
| LDAC | ❌ | ❌ | ✅ | ✅ |
| aptX HD | ❌ | ❌ | ✅ | 🧪 experimental |
| aptX Low Latency | ❌ | ❌ | ✅ | 🧪 experimental |
| aptX | ✅ | ✅ | ✅ | 🧪 experimental |
| AAC | ✅ | ✅ | ❌ not in Ubuntu's packages | ✅ |
| SBC | ✅ | ✅ | ✅ | ✅ |
| aptX Adaptive | ❌ | ❌ | ❌ | ❌ |

✅ supported · 🧪 experimental · ❌ not supported. All columns are the sending side (PC → headphones). "Built-in" is the OS's own Bluetooth audio, without A2DPWB; the Ubuntu column is the PipeWire codec set of Ubuntu 26.04 (`libspa-0.2-bluetooth`).

{: .warning }
**aptX, aptX HD and aptX Low Latency are experimental.** They follow the Android / PipeWire implementations and pass encode/decode round-trip tests, but have not yet been verified with real headphones. Latency values in this table are typical figures, not measured with A2DPWB.

{: .note }
**Only need classic aptX?** Windows 10 already supports classic aptX in its built-in Bluetooth stack (not aptX HD, aptX LL or aptX Adaptive), so A2DPWB is not required for it. A2DPWB is mainly useful for LDAC, aptX HD and aptX Low Latency.

{: .note }
**aptX Adaptive is not supported** (there is no open-source encoder). If your headphones also list classic aptX, A2DPWB uses aptX directly; otherwise Auto picks AAC or SBC. See [aptX Family and aptX Adaptive Compatibility](usage#aptx-compatibility).

## Platform Support

| Component | Windows 10 (x64) | Windows 11 (x64) | Linux (Ubuntu) |
|:----------|:----------------:|:----------------:|:--------------:|
| **A2DPWB** (GUI / CLI) | ✅ (not yet tried on real hardware) | ✅ | ❌ |
| [Building from source](building) (Visual Studio 2022+) | ✅ | ✅ | ❌ |
| `a2dpwb_decode` (stream checker) | ✅ | ✅ | ❌ |
| End-to-end test `tools/emu/run_test.py` | ✅ | ✅ | ❌ |
| Virtual adapter `tools/emu/emu_vhci.py` | ❌ | ❌ | ✅ |
| [Measuring receiver](usage#receiver) `tools/linux_sink` | ❌ | ❌ | ✅ |

A2DPWB itself runs on Windows only, as an x64 build. The Linux tools need Python 3.11 or later (verified on Ubuntu 26.04); `setup.sh` targets Ubuntu / Debian, other distributions need the packages installed by hand.

## Verified Hardware

| What | Setup | Codecs | Result |
|:-----|:------|:-------|:-------|
| Sony WH-1000XM4 (headphones) | Windows 11 + TP-Link UB500 | LDAC, AAC, SBC | ✅ Plays (the XM4 has no aptX) |
| TP-Link UB500 (Realtek RTL8761BU, adapter) | Windows 11, WinUSB, linux-firmware `rtl8761bu` | – | ✅ |
| Measuring receiver `tools/linux_sink` | Windows 11 + UB500 → Ubuntu 26.04 over the air | SBC, AAC, aptX, aptX HD, aptX LL, LDAC | ✅ Received and measured |
| Virtual link `tools/emu` (Bumble) | Windows 11, no radio | SBC, AAC, aptX, aptX HD, aptX LL, LDAC | ✅ End-to-end test passes |

Windows 10 has not been tried on real hardware yet. Other adapters: see [Recommended Adapters](setup#adapters).

## Audio Capture Modes

A2DPWB captures audio via WASAPI and offers two modes:

| Mode | Description | Use Case |
|:-----|:------------|:---------|
| **System Loopback** | Captures all system audio output from the default playback device via WASAPI loopback | Simple setup -- all sounds are streamed |
| **Virtual Device** | Captures from a user-selected virtual audio device (e.g., VB-CABLE, VoiceMeeter) | Route specific apps to Bluetooth while keeping other audio on speakers |

In **Virtual Device** mode, A2DPWB switches the Windows default playback device to the selected virtual device, then captures its loopback output. Apps that output to the virtual device are streamed over Bluetooth.

## How It Works

```
Audio Source
  |
  +-- System Loopback: default playback device (all system audio)
  +-- Virtual Device:  selected virtual audio device (per-app routing)
  |
  v
WASAPI Loopback Capture (PCM)
  |
  v
Audio Encoder (LDAC / aptX HD / aptX LL / aptX / AAC / SBC)
  |
  v
BTstack (A2DP Source -> AVDTP -> L2CAP -> HCI)
  |
  v
WinUSB -> USB Bluetooth Adapter -> Headphones
```

A2DPWB bypasses the Windows Bluetooth stack entirely. It communicates directly with a USB Bluetooth adapter through WinUSB, using [BTstack](https://github.com/bluekitchen/btstack) to implement the full Bluetooth protocol stack in user-mode.

## Features

- **Multi-codec**: LDAC, aptX HD, aptX Low Latency, aptX, AAC, SBC with automatic negotiation
- **Two capture modes**: System loopback or virtual audio device routing
- **LDAC ABR**: Adaptive Bit Rate for unstable connections
- **Auto-reconnect**: Reconnects on disconnection (up to 10 attempts)
- **Link quality and AFH**: Shows what is sent, the RSSI and the channels in use; tells the adapter which Wi-Fi channels to avoid (see [Usage](usage#link-quality))
- **Profile management**: Save and load device + codec configurations
- **GUI + CLI**: wxWidgets graphical interface or command-line operation
- **Localization**: English / Japanese
- **Realtek firmware**: Guided firmware download for Realtek adapters
- **Diagnostics** (debug mode): debug console, HCI capture, peer receiver test against a Linux PC (see [Usage](usage#receiver))
