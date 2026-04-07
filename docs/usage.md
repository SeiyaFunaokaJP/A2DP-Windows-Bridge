---
title: Usage
layout: default
nav_order: 3
---

# Usage
{: .no_toc }

## Table of contents
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## GUI Mode (Default)

```
A2DPWB.exe
```

The graphical interface provides:

- **Bluetooth adapter selection** -- choose which WinUSB adapter to use
- **Audio device selection** -- choose the WASAPI loopback capture source
- **Device address** -- enter manually or select from paired devices
- **Codec selection** -- Auto / LDAC / aptX HD / aptX LL / AAC / SBC
- **LDAC quality** -- HQ (990 kbps) / SQ (660 kbps) / MQ (330 kbps)
- **LDAC ABR** -- Adaptive Bit Rate toggle for unstable connections
- **Profile management** -- save and load device + codec configurations
- **Real-time status** -- codec, bitrate, connection state

## CLI Mode

Add `--cli` to run without a GUI window.

### Basic

```bash
# Auto-select best codec
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF
```

### Codec Selection

```bash
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac      # LDAC
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxhd    # aptX HD
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxll    # aptX Low Latency
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -c aac       # AAC
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -c sbc       # SBC
```

Auto-select priority: LDAC > aptX HD > aptX LL > AAC > SBC

### LDAC Quality

```bash
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q hq   # 990 kbps (default)
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q sq   # 660 kbps
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q mq   # 330 kbps
```

### LDAC ABR (Adaptive Bit Rate)

```bash
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -a
```

ABR automatically lowers the bitrate when the Bluetooth connection is unstable, and raises it when conditions improve.

### Device Discovery

```bash
# List paired Bluetooth audio devices
A2DPWB.exe --cli -l
```

{: .note }
Device listing uses the Windows Bluetooth API via your **built-in** Bluetooth adapter, not the WinUSB adapter.

## Capture Modes

A2DPWB supports two audio capture modes:

| Mode | Description |
|:-----|:------------|
| System Loopback | Captures all system audio from the default output device |
| Virtual Device | Captures from a specific virtual audio device (e.g., VB-CABLE) for per-app routing |

{: .warning }
Both modes use WASAPI shared mode, so the capture sample rate is determined by the device's **Default Format** in Windows Sound settings (typically 48 kHz). To use LDAC at 96 kHz, change the output device's format to 96 kHz in **Sound settings > Device properties > Advanced > Default format**.

## Codec Comparison

| Codec | Best For | Trade-off |
|:------|:---------|:----------|
| LDAC (HQ) | Highest audio quality | Higher latency, needs stable connection |
| LDAC (ABR) | Quality + stability | Bitrate fluctuates with connection |
| aptX HD | High quality, wider device support | Fixed 576 kbps |
| aptX LL | Gaming, video (low latency) | Lower audio quality (16-bit) |
| AAC | Apple devices, good efficiency | Moderate quality |
| SBC | Maximum compatibility | Lowest audio quality |
