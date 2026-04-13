---
title: Architecture
layout: default
nav_order: 5
---

# Architecture
{: .no_toc }

## Table of contents
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## Overview

A2DP Windows Bridge (A2DPWB) enables LDAC, aptX HD, aptX Low Latency, AAC, and
SBC Bluetooth audio on Windows. Windows natively only supports SBC and AAC for
Bluetooth A2DP — this tool adds high-quality codecs without requiring a kernel
driver.

Uses **BTstack + WinUSB** — entirely user-mode, no driver signing needed.

## Transport: BTstack + WinUSB

Bypasses the Windows Bluetooth stack entirely by communicating directly with a
USB Bluetooth adapter via WinUSB (Microsoft-signed generic USB driver). BTstack,
an open-source Bluetooth stack, implements HCI, L2CAP, AVDTP, and A2DP in
user-mode.

**Advantages**:
- No driver signing cost
- No test signing mode required
- No Secure Boot disabling
- All code runs in user-mode (easier debugging)

**Trade-offs**:
- Requires a dedicated USB Bluetooth adapter (separate from Windows's built-in
  Bluetooth)
- While claimed by WinUSB, the adapter is unavailable for normal Windows
  Bluetooth
- Device address must be entered manually or selected from Windows paired
  device list (discovered via built-in adapter)

**How it works**:
1. User installs WinUSB driver on a USB Bluetooth adapter via Zadig
2. BTstack opens the USB device via WinUSB API
3. BTstack sends HCI commands to initialize the Bluetooth controller
4. (Realtek adapters) Firmware is uploaded if needed
5. BTstack establishes an ACL connection and opens L2CAP channels (PSM 0x0019)
6. AVDTP signaling discovers remote SEPs and negotiates codecs
7. Encoded audio is sent as AVDTP media packets over L2CAP

## Module Structure

```
A2DPWB.exe
├── GUI Layer (wxWidgets)
│   ├── wx_app              App entry point, event loop
│   ├── wx_main_frame       Main window (device, codec, status)
│   ├── wx_profile_dialog   Connection profile management
│   ├── wx_settings_dialog  Application settings
│   ├── wx_firmware_dialog  Realtek firmware download
│   ├── wx_about_dialog     About / license info
│   ├── wx_zadig_dialog     Zadig WinUSB installation guide
│   ├── theme_manager       Light / dark theme support
│   └── localization        i18n (en, ja — embedded JSON)
│
├── Core Layer
│   ├── a2dp_service        A2DP connection lifecycle & state machine
│   ├── btstack_transport   BTstack integration (HCI, L2CAP, AVDTP, A2DP)
│   ├── wasapi_capture      WASAPI loopback audio capture
│   ├── audio_encoder       Encoder interface (abstract base)
│   │   ├── ldac_encoder        LDAC (libldac, ABR support)
│   │   ├── aptxhd_encoder      aptX HD (libopenaptx)
│   │   ├── aptxll_encoder      aptX Low Latency (libopenaptx)
│   │   ├── aac_encoder         AAC-LC (fdk-aac, LATM transport)
│   │   └── a2dp_sbc_encoder    SBC (BTstack Bluedroid)
│   │
│   ├── bt_device           Bluetooth device info (address, name, codecs)
│   ├── bt_adapter_enum     USB Bluetooth adapter enumeration (WinUSB)
│   ├── audio_device_enum   WASAPI audio device enumeration
│   └── profile_manager     Connection profile persistence (JSON)
│
├── Support
│   ├── app_settings        Persistent application settings (JSON)
│   ├── config_path         Config file path resolution
│   ├── system_integration  System tray, autostart
│   ├── debug_log           Debug logging macros
│   └── capture_mode        Audio capture mode definitions
│
└── CLI Mode
    └── main.cpp            CLI argument parsing, headless streaming
```

## Data Flow

```
System Audio Output
       │
       ▼
 WASAPI Loopback Capture (PCM 16-bit, 44.1/48 kHz)
       │
       ▼
 Audio Encoder (LDAC / aptX HD / aptX LL / AAC / SBC)
       │
       ▼
 A2DP Service → BtStackTransport::send_media()
       │
       ▼
 BTstack A2DP Source → AVDTP → L2CAP → HCI
       │
       ▼
 WinUSB → USB Bluetooth Adapter → Bluetooth Radio
       │
       ▼
 Headphones / Speakers
```

## Key Modules

### A2DP Service (`a2dp_service.cpp`)

Central connection lifecycle manager. Coordinates:
- Codec negotiation (auto-select or user-specified)
- Stream endpoint registration for all supported codecs
- Connection state machine (idle → connecting → streaming → disconnecting)
- Auto-reconnect logic on unexpected disconnection
- Media packet sending with codec-specific framing

### BTstack Transport (`btstack_transport.cpp`)

Bridge between the application's synchronous model and BTstack's event-driven
API.

**Responsibilities**:
- Initialize BTstack with WinUSB HCI transport
- Run BTstack event loop in a dedicated thread
- Register vendor codec stream endpoints (LDAC, aptX HD, aptX LL)
- Handle A2DP connection lifecycle via async-to-sync wrappers
- Manage SSP pairing (Just Works mode)
- Provide thread-safe media sending for WASAPI callback
- Realtek chipset firmware loading

**Key BTstack APIs used**:
- `a2dp_source_create_stream_endpoint()` — Register codec endpoints
- `a2dp_source_establish_stream()` — Connect to A2DP sink
- `a2dp_source_set_config_other()` — Configure vendor-specific codec
- `a2dp_source_stream_send_media_payload_rtp()` — Send encoded audio

### WASAPI Capture (`wasapi_capture.cpp`)

Captures system audio output in real-time using Windows Audio Session API.
- Uses `IAudioClient` in `AUDCLNT_STREAMFLAGS_LOOPBACK` mode
- Provides PCM data (float32 → int16 conversion, channel downmix)
- Configurable audio device selection

### Audio Encoders

| Encoder | Library | Bitrate | Features |
|---------|---------|---------|----------|
| LDAC | libldac (AOSP) | 330/660/990 kbps | HQ/SQ/MQ modes, ABR |
| aptX HD | libopenaptx | 576 kbps | 24-bit, fixed rate |
| aptX LL | libopenaptx | 352 kbps | ~32 ms latency |
| AAC | fdk-aac | up to 256 kbps | AAC-LC, LATM transport |
| SBC | BTstack Bluedroid | up to ~345 kbps | Mandatory A2DP baseline |

All encoders implement the `AudioEncoder` interface with `encode()` and
`get_frame_size()` methods.

## Vendor Codec Information Elements

Non-standard codecs are registered as Vendor Specific in AVDTP:

| Codec | Vendor ID | Codec ID |
|-------|-----------|----------|
| LDAC | Sony (0x0000012D) | 0x00AA |
| aptX HD | Qualcomm (0x000000D7) | 0x0024 |
| aptX Low Latency | CSR (0x0000000A) | 0x0002 |

AAC and SBC use standard A2DP codec IDs defined in the A2DP specification.

## Build System

- **CMake** with MSVC (Visual Studio 2022 or later)
- Third-party libraries built as static libraries from source
- wxWidgets fetched at configure time via CMake FetchContent (v3.2.6)
- Language files (JSON) embedded into the executable at configure time
- Application manifest and icon compiled via Windows resource script

## Important Considerations

- **Adapter compatibility**: Tested with Intel, CSR, and Realtek USB adapters.
  Realtek adapters require firmware upload at startup
- **Pairing**: Uses SSP Just Works. Link keys are persisted to a local file
- **Second adapter recommended**: Keep built-in Bluetooth for Windows, use
  dedicated USB adapter for A2DPWB
- Some Bluetooth adapters' firmware limits achievable bitrate
- USB Bluetooth 5.0+ adapters generally work well for LDAC
- Auto codec selection priority: LDAC > aptX HD > aptX LL > AAC > SBC
