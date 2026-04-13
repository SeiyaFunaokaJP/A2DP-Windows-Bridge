---
title: Home
layout: default
nav_order: 1
---

# A2DP Windows Bridge (A2DPWB)

Bluetooth A2DP audio streaming for Windows with full codec support.
{: .fs-6 .fw-300 }

Streams system audio via **LDAC, aptX HD, aptX Low Latency, AAC, SBC** using a USB Bluetooth adapter in WinUSB mode -- no kernel driver or test signing required.

[Get Started](setup){: .btn .btn-primary .fs-5 .mb-4 .mb-md-0 .mr-2 }
[GitHub](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge){: .btn .fs-5 .mb-4 .mb-md-0 }

---

## Supported Codecs

| Codec | Bitrate | Sample Rate | Bit Depth | Latency |
|:------|:--------|:------------|:----------|:--------|
| LDAC | 330/660/990 kbps | 44.1--96 kHz | 16/24/32 bit | ~200 ms |
| aptX HD | 576 kbps | 44.1/48 kHz | 24 bit | ~150 ms |
| aptX Low Latency | 352 kbps | 44.1/48 kHz | 16 bit | ~32 ms |
| AAC | 128/192/256 kbps | 44.1/48 kHz | 16 bit | ~150 ms |
| SBC | up to ~345 kbps | 44.1/48 kHz | 16 bit | ~150 ms |

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
Audio Encoder (LDAC / aptX HD / aptX LL / AAC / SBC)
  |
  v
BTstack (A2DP Source -> AVDTP -> L2CAP -> HCI)
  |
  v
WinUSB -> USB Bluetooth Adapter -> Headphones
```

A2DPWB bypasses the Windows Bluetooth stack entirely. It communicates directly with a USB Bluetooth adapter through WinUSB, using [BTstack](https://github.com/bluekitchen/btstack) to implement the full Bluetooth protocol stack in user-mode.

## Features

- **Multi-codec**: LDAC, aptX HD, aptX Low Latency, AAC, SBC with automatic negotiation
- **Two capture modes**: System loopback or virtual audio device routing
- **LDAC ABR**: Adaptive Bit Rate for unstable connections
- **Auto-reconnect**: Reconnects on disconnection (up to 10 attempts)
- **Profile management**: Save and load device + codec configurations
- **GUI + CLI**: wxWidgets graphical interface or command-line operation
- **Localization**: English / Japanese
- **Realtek firmware**: Guided firmware download for Realtek adapters
