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
- **Codec selection** -- Auto / LDAC / aptX HD / aptX LL / aptX / SBC / AAC
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
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -c aptx      # aptX
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -c aac       # AAC
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -c sbc       # SBC
```

Auto-select priority: LDAC > aptX HD > aptX LL > aptX > AAC > SBC

If the requested codec is not offered by the headphones, the CLI falls back to the Auto priority order. aptX Adaptive is never selected -- see [aptX Family and aptX Adaptive Compatibility](#aptx-compatibility).

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
| aptX | Widest aptX device support | 16-bit, 352/384 kbps |
| AAC | Apple devices, good efficiency | Moderate quality |
| SBC | Maximum compatibility | Lowest audio quality |

## aptX Family and aptX Adaptive Compatibility {#aptx-compatibility}

A2DPWB supports three aptX codecs, all encoded with [libopenaptx](https://github.com/pali/libopenaptx). aptX Adaptive is **not** supported.

{: .warning }
**aptX, aptX HD and aptX Low Latency are experimental.** They follow the Android / PipeWire implementations and pass encode/decode round-trip tests, but have not yet been verified with real headphones.

{: .note }
**Only need classic aptX?** Windows 10 already supports classic aptX in its built-in Bluetooth stack (not aptX HD, aptX LL or aptX Adaptive), so A2DPWB is not required for it. A2DPWB is mainly useful for LDAC, aptX HD and aptX Low Latency.

| Codec | Vendor ID / Codec ID | RTP header | Status |
|:------|:---------------------|:-----------|:-------|
| aptX | 0x0000004F / 0x0001 | No | Supported (352/384 kbps at 44.1/48 kHz) |
| aptX HD | 0x000000D7 / 0x0024 | Yes | Supported (576 kbps, 24-bit input) |
| aptX Low Latency | 0x0000000A or 0x000000D7 / 0x0002 | No | Supported (352 kbps) |
| aptX Adaptive | 0x000000D7 / 0x00AD | -- | **Not supported** -- detected and logged only |

### Why aptX Adaptive is not supported

There is no open-source aptX Adaptive encoder. libopenaptx implements aptX and aptX HD only (A2DPWB also uses it for aptX Low Latency). [arkq/openaptx](https://github.com/arkq/openaptx) only reimplements aptX / aptX HD, contains no aptX Adaptive encoder, and does not permit binary redistribution. A2DPWB therefore only detects aptX Adaptive and writes it to the log.

### There is no "fallback" from aptX Adaptive to aptX

When A2DPWB connects, the headphones advertise a **list of codecs** (AVDTP stream endpoints). A2DPWB chooses one codec from that list. aptX Adaptive is simply never a candidate, so nothing "falls back" from it:

- **The headphones also list classic aptX** -- Many aptX Adaptive headphones do, because Qualcomm markets aptX Adaptive as backward compatible with aptX / aptX HD. In that case A2DPWB selects classic aptX directly; it is a separate codec in the headphones' list, not a reduced mode of aptX Adaptive.
- **The headphones list only aptX Adaptive** (no classic aptX, aptX HD or aptX LL) -- aptX is not possible with A2DPWB. Auto picks the next codec in the priority order, typically AAC or SBC.

Auto-select priority: LDAC > aptX HD > aptX LL > aptX > AAC > SBC

### Explicitly Selected Codec Not Offered: GUI vs CLI

The GUI and the CLI behave differently when you request a specific codec that the headphones do not list:

| Mode | Behavior |
|:-----|:---------|
| GUI (codec other than Auto) | The connection stops with an error ("Device does not support ..."). If you requested aptX, aptX HD or aptX LL and the headphones offer only aptX Adaptive, the message says so and suggests Auto, AAC or SBC. |
| CLI (`-c <codec>`) | Prints `Requested codec ... not available, falling back...` and continues with the Auto priority order. If the headphones offer aptX Adaptive, an extra line notes that it is never selected. |

### Sample Rate

aptX, aptX HD and aptX LL support only 44.1 kHz and 48 kHz. A2DPWB picks one of the rates the headphones advertise (preferring the current Windows device rate, otherwise 48 kHz) and lets WASAPI resample the capture. You do not need to change the Windows Sound settings for aptX.

### Checking What Your Headphones Offer

1. In the **Settings** menu, check **Debug Mode (debug.log / HCI Log)**, then restart A2DPWB.
2. Connect to the headphones.
3. Open `debug.log` in the config folder (`%APPDATA%\A2DPWB`). In CLI mode the same lines are written to standard error.

The capability lines show exactly which codecs the headphones list:

```
BTstack: Remote supports aptX (SEID=..., caps=0x..)
BTstack: Remote supports aptX Adaptive (SEID=...) — never selected (no open encoder); classic aptX is used only if the remote lists it
BTstack: Remote vendor codec vid=0x........ cid=0x.... (SEID=...) — unsupported
BTstack: Capability discovery complete (LDAC=0, aptXHD=0, aptXLL=0, aptX=1, aptXAdaptive=1 [not encodable], SBC=1, AAC=1)
```

- `aptX=1` -- classic aptX is available (even if `aptXAdaptive=1` too).
- `aptX=0`, `aptXHD=0`, `aptXLL=0` with `aptXAdaptive=1` -- the headphones offer only aptX Adaptive; use Auto, AAC or SBC.
- `Remote vendor codec ... — unsupported` -- another vendor codec that A2DPWB does not implement.

### Max Media Packet Size (Advanced)

**Settings > Advanced Settings...** (CLI: `--max-packet <bytes>`) sets the upper limit for the audio data in each Bluetooth packet, from 679 to 1679 bytes. The headphones' own limit (MTU) always applies too; A2DPWB uses the smaller of the two. The change applies from the next connection.

**1023 (default) is recommended.** It fits one Bluetooth baseband packet (3-DH5). Larger values save only 1-2% overhead, and each lost packet then loses more audio. 679 fits one 2-DH5 packet and is the minimum LDAC accepts.

### Verifying the Media Stream (a2dpwb_decode) {#verify-stream}

Most headphones cannot show which codec is in use. In debug mode, the GUI has a **Debug** menu with **Start HCI Capture** / **Stop HCI Capture**. A capture is an HCI packet log (`hci_<date>_<time>.pklg` in the config folder) with the codec negotiation and every media packet sent. At LDAC 990 kbps it grows by roughly 0.5 GB per hour, so capture only as long as you need.

You can start a capture before connecting or while already streaming. If it starts during a connection, A2DPWB first writes the connection setup packets it remembered (L2CAP channel setup and AVDTP signaling, including the codec negotiation), so the file can still be analysed. After stopping, A2DPWB shows the file path.

Open the capture in Wireshark, or check it with the `a2dpwb_decode` developer tool (built from source, see [Building](building)):

```
a2dpwb_decode "%APPDATA%\A2DPWB\hci_20260925_120000.pklg" -o C:\temp\check
```

The tool follows the AVDTP signaling (SET_CONFIGURATION / RECONFIGURE and whether the headphones accepted it), extracts the media packets A2DPWB sent and checks them against the negotiated configuration:

| Codec | What is checked | Output |
|:------|:----------------|:-------|
| SBC, AAC, aptX, aptX HD, aptX LL | Full decode with the reference decoders; frame parameters vs configuration; media payload header frame count | `<prefix>.<n>.<codec>.wav` to listen to |
| LDAC | Frame headers only (sync word, sampling rate, channel mode, frame length), frame count, bitrate. No open-source LDAC decoder exists | Report only |

For every stream it also reports RTP sequence gaps, how fast the RTP timestamp advances, the bitrate, and how many ACL packets the Bluetooth controller reported as completed. The last line is `RESULT: OK` or `RESULT: PROBLEMS FOUND` (exit code 0 or 1).

{: .note }
This shows what A2DPWB sent and that it is valid for the negotiated codec. Since a sink can only decode the codec that was negotiated, correct audio from the headphones together with a clean report is strong evidence that the codec is really in use.

## Troubleshooting

### Pairing Problems After Updating

A2DPWB pairs using SSP (Just Works) with **General Bonding**, and the link keys are saved so the headphones can reconnect without pairing again. If you paired the headphones with an older A2DPWB build, remove the A2DPWB pairing on the headphones (see the headphones' manual for clearing the pairing list) and pair again.

### Connection Timeouts

A2DPWB registers SDP records (A2DP Source, AVRCP Controller / Target) so the headphones can identify it as an audio source. If a connection attempt times out, the half-open connection is torn down, so you can simply retry in the same session without restarting A2DPWB. Make sure the headphones are powered on, in range, and not connected to another device.
