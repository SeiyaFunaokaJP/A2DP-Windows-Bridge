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

A2DP Windows Bridge (A2DPWB) enables LDAC, aptX HD, aptX Low Latency, aptX,
AAC, and SBC Bluetooth audio on Windows. Windows' own Bluetooth stack supports
only SBC, AAC and classic aptX for A2DP — this tool adds LDAC, aptX HD and aptX
Low Latency without requiring a kernel driver.

Uses **BTstack + WinUSB** — entirely user-mode, no driver signing needed.

## Transport: BTstack + WinUSB

Bypasses the Windows Bluetooth stack entirely by communicating directly with a
USB Bluetooth adapter via WinUSB (Microsoft-signed generic USB driver). BTstack,
a source-available Bluetooth stack (non-commercial license), implements HCI, L2CAP, AVDTP, and A2DP in
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
- Device address is entered manually, selected from the Windows paired device
  list (read via the built-in adapter), or found by a scan (GAP inquiry) with
  the USB adapter

**How it works**:
1. User installs WinUSB driver on a USB Bluetooth adapter via Zadig
2. BTstack opens the USB device via WinUSB API
3. BTstack sends HCI commands to initialize the Bluetooth controller
4. (Realtek, Intel and Broadcom adapters) Firmware is uploaded if needed
5. BTstack establishes an ACL connection and opens L2CAP channels (PSM 0x0019)
6. AVDTP signaling discovers remote SEPs and negotiates codecs
7. Encoded audio is sent as AVDTP media packets over L2CAP

## Module Structure

```
A2DPWB.exe
├── GUI Layer (wxWidgets)
│   ├── wx_app              App entry point, event loop
│   ├── wx_main_frame       Main window (device, codec, status)
│   ├── wx_profile_dialog   Connection profile management, device scan
│   ├── wx_settings_dialog  Application settings
│   ├── wx_firmware_dialog  Realtek firmware download
│   ├── wx_about_dialog     About / license info
│   ├── wx_zadig_dialog     Zadig WinUSB installation guide
│   ├── wx_link_quality_dialog  Link quality window (what is sent, radio)
│   ├── wx_receiver_dialog  Peer receiver test (sent vs. received, debug mode)
│   ├── wx_debug_console_dialog  Debug console: live log, connection flow
│   ├── wx_radio_text.h     RSSI / AFH as text for both windows
│   ├── theme_manager       Light / dark theme support
│   └── localization        i18n (en, ja — embedded JSON)
│
├── Core Layer
│   ├── a2dp_service        A2DP connection lifecycle & state machine
│   ├── btstack_transport   BTstack integration (HCI, L2CAP, AVDTP, A2DP)
│   ├── btstack_link_key_db_file  Link key storage (file in the config folder)
│   ├── btstack_uart_tcp_windows  H4 over TCP to a virtual controller (--hci-tcp, testing)
│   ├── wasapi_capture      WASAPI loopback audio capture
│   ├── test_tone           Test tone instead of captured audio
│   ├── audio_encoder       Encoder interface (abstract base)
│   │   ├── ldac_encoder        LDAC (libldac, ABR support)
│   │   ├── aptxhd_encoder      aptX HD (libopenaptx)
│   │   ├── aptxll_encoder      aptX Low Latency (libopenaptx)
│   │   ├── aptx_encoder        aptX classic (libopenaptx)
│   │   ├── aptx_pcm_pack.h     Shared PCM → packed 24-bit helper for aptX encoders
│   │   ├── aac_encoder         AAC-LC (fdk-aac, LATM transport)
│   │   └── a2dp_sbc_encoder    SBC (BTstack Bluedroid)
│   │
│   ├── bt_device           Bluetooth device info (address, name, codecs)
│   ├── bt_adapter_enum     USB Bluetooth adapter enumeration (WinUSB)
│   ├── audio_device_enum   WASAPI audio device enumeration
│   ├── profile_manager     Connection profile persistence (JSON)
│   ├── media_payload_limit Max media packet size (range, default)
│   ├── afh                 AFH host channel classification from the Wi-Fi scan
│   ├── link_stats          Counters for the link quality window
│   └── remote_sink_client  Statistics from tools/linux_sink over the network
│
├── Support
│   ├── app_settings        Persistent application settings (JSON)
│   ├── config_path         Config file path resolution
│   ├── system_integration  System tray, autostart
│   ├── debug_log           Debug logging macros
│   ├── debug_log_model     Log lines and connection flow for the debug console
│   ├── hci_capture         On-demand HCI capture (.pklg)
│   ├── update_checker      Update check (GitHub Releases)
│   ├── zadig_helper        Zadig download / launch
│   ├── embedded_langs      Language files embedded at build time
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
 WASAPI Loopback Capture (device mix format, usually float32, 44.1-96 kHz)
       │
       ▼
 Audio Encoder (LDAC / aptX HD / aptX LL / aptX / AAC / SBC)
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
- aptX-family sample rate selection (44.1 / 48 kHz from the rates the remote
  advertises; WASAPI resamples the capture)
- PCM conversion (float32 → 16 / 32-bit integer) for the encoder
- Connection state machine (idle → connecting → streaming → reconnecting, error)
- Auto-reconnect logic on unexpected disconnection
- Media packet sending with codec-specific framing

### BTstack Transport (`btstack_transport.cpp`)

Bridge between the application's synchronous model and BTstack's event-driven
API.

**Responsibilities**:
- Initialize BTstack with WinUSB HCI transport
- Run BTstack event loop in a dedicated thread
- Register stream endpoints: vendor codecs (LDAC, aptX HD, aptX, aptX LL)
  plus SBC and AAC
- Register SDP records (A2DP Source, AVRCP Controller, AVRCP Target)
- Parse remote capabilities, including aptX LL under both vendor IDs and
  aptX Adaptive (detected and logged only)
- Handle A2DP connection lifecycle via async-to-sync wrappers
- Manage SSP pairing (Just Works, General Bonding; link keys persisted)
- Tear down a half-open connection after a connect timeout so retries work
- Retry a failed connection (up to 3 attempts) and tell the causes apart (no
  answer, silent link, remote ended the link, authentication); drop a link key
  the device no longer knows and pair again
- GAP inquiry for the device scan
- AFH host channel classification and RSSI / AFH readings for the link quality
  window
- Provide thread-safe media sending for WASAPI callback
- Firmware loading for Realtek, Intel and Broadcom chipsets

**Key BTstack APIs used**:
- `a2dp_source_create_stream_endpoint()` — Register codec endpoints
- `a2dp_source_establish_stream()` — Connect to A2DP sink
- `a2dp_source_set_config_other()` — Configure vendor-specific codec
- `a2dp_source_stream_send_media_payload_rtp()` — Send encoded audio with an
  RTP header (LDAC, aptX HD, AAC, SBC)
- `a2dp_source_stream_send_media_packet()` — Send raw media packets without
  an RTP header (aptX, aptX LL)

### WASAPI Capture (`wasapi_capture.cpp`)

Captures system audio output in real-time using Windows Audio Session API.
- Uses `IAudioClient` in `AUDCLNT_STREAMFLAGS_LOOPBACK` mode
- Delivers the device's shared-mode mix format (usually float32; A2DP Service
  converts it for the encoder)
- Configurable audio device selection

### Audio Encoders

| Encoder | Library | Bitrate | Features |
|---------|---------|---------|----------|
| LDAC | libldac (AOSP) | 330/660/990 kbps | HQ/SQ/MQ modes, ABR |
| aptX HD | libopenaptx | 576 kbps | 24-bit input, fixed rate, RTP header |
| aptX LL | libopenaptx | 352 kbps | ~32 ms latency, no RTP header, packets ≤ ~7.5 ms |
| aptX | libopenaptx | 352/384 kbps (44.1/48 kHz) | 16-bit stereo, no RTP header |
| AAC | fdk-aac | up to 256 kbps | AAC-LC, LATM transport |
| SBC | BTstack Bluedroid | up to ~345 kbps | Mandatory A2DP baseline |

All encoders implement the `AudioEncoder` interface with `encode()` and
`get_frame_size()` methods.

libopenaptx expects groups of 4 stereo samples as packed 24-bit little-endian
PCM. `aptx_pcm_pack.h` converts the capture's 16-bit or 32-bit (MSB-aligned)
samples into that format for the aptX, aptX HD and aptX LL encoders; mono input
is duplicated to both channels.

## Vendor Codec Information Elements

Non-standard codecs are registered as Vendor Specific in AVDTP:

| Codec | Vendor ID | Codec ID |
|-------|-----------|----------|
| LDAC | Sony (0x0000012D) | 0x00AA |
| aptX | APT (0x0000004F) | 0x0001 |
| aptX HD | Qualcomm (0x000000D7) | 0x0024 |
| aptX Low Latency | CSR (0x0000000A) or Qualcomm (0x000000D7) | 0x0002 |
| aptX Adaptive | Qualcomm (0x000000D7) | 0x00AD (detected only, never selected) |

AAC and SBC use standard A2DP codec IDs defined in the A2DP specification.

Codec information element sizes (including the 6-byte vendor/codec ID):

- **aptX**: 7 bytes (byte 6 = sample rate / channel mode)
- **aptX HD**: 11 bytes (byte 6 as aptX, plus 4 reserved bytes); stereo is
  required
- **aptX LL**: 8 bytes, or 17 bytes when the sink sets the extended
  ("new caps") flag. A2DPWB registers one aptX LL endpoint and configures the
  stream with the vendor ID the sink used

**RTP vs no RTP**: LDAC, aptX HD, AAC and SBC media packets carry the 12-byte
RTP header. aptX and aptX LL are sent **without** an RTP header (as Android and
PipeWire do), so the whole L2CAP MTU is available for aptX frames.

**aptX Adaptive** has no open-source encoder (libopenaptx does not implement
it), so A2DPWB never registers or selects it. When a sink lists it, A2DPWB
only logs it; classic aptX is used if the sink lists that separately.

## Build System

- **CMake** with MSVC (Visual Studio 2022 or later)
- Third-party libraries built as static libraries from source
- wxWidgets fetched at configure time via CMake FetchContent (v3.2.6)
- Language files (JSON) embedded into the executable at configure time
- Application manifest and icon compiled via Windows resource script

## Important Considerations

- **Adapter compatibility**: Realtek (best tested) and CSR USB adapters; Intel and
  Broadcom are experimental. Realtek adapters require firmware upload at startup,
  Intel adapters in bootloader mode too
- **Pairing**: Uses SSP Just Works with General Bonding. Link keys are persisted
  to a local file
- **Second adapter recommended**: Keep built-in Bluetooth for Windows, use
  dedicated USB adapter for A2DPWB
- Some Bluetooth adapters' firmware limits achievable bitrate
- USB Bluetooth 5.0+ adapters generally work well for LDAC
- Auto codec selection priority: LDAC > aptX HD > aptX LL > aptX > AAC > SBC
