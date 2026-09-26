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

In the GUI, **Debug > Debug Console...** (Ctrl+Shift+D) shows the same log live. Its **Connection Flow** list marks each stage of the latest connection attempt (adapter init, ACL / AVDTP connect, capability discovery, codec selection, WASAPI, stream configuration, encoder, stream start, audio sending, disconnects / reconnects) as OK, Warning or Failed; double-click a step to jump to the log line behind it. The log below can be filtered by level and text. **Copy Diagnostics** puts version, OS, adapter, profile, the flow, warnings / errors and the recent log on the clipboard, ready to paste into a bug report (Bluetooth addresses are masked to their vendor part by default).

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

### Max Media Packet Size (per Profile)

**Max media packet size** in each profile (**Edit Profile**; CLI: `--max-packet <bytes>`) sets the upper limit for the audio data in each Bluetooth packet, from 679 to 1679 bytes, so headphones that need a different value can have their own. The headphones' own limit (MTU) always applies too; A2DPWB uses the smaller of the two. The change applies from the next connection. Profiles saved by older versions take the value that was set globally before.

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

### Link Quality Window and AFH {#link-quality}

**Actions > Link Quality...** shows what A2DPWB sends while streaming: codec and bitrate, packets sent, packets dropped before sending (send queue full: the radio link could not keep up), audio the encoder could not keep up with, the send queue, and **Radio**: the RSSI of the connection, relative to the controller's golden receive range (0 = fine, negative = too weak; not an absolute level), and how many of the 79 Bluetooth channels the link hops on (AFH).

Bluetooth avoids busy channels by itself (AFH), but it only learns about Wi-Fi from errors. A2DPWB also tells its controller which channels to avoid (AFH host channel classification), for every connection:

| `afh` | |
|:--|:--|
| `auto` (default) | The 2.4 GHz Wi-Fi networks this PC hears strongly (Windows Wi-Fi scan, repeated while connected) |
| `off` | Nothing: the controller decides alone |
| `6,11` | These Wi-Fi channels |

At least 20 Bluetooth channels always stay in use. Set it with `"afh"` in `settings.json` (config folder), in the Peer Receiver Test window (it is saved there), or with `--afh` in CLI mode. The Radio row shows which Wi-Fi channels are avoided. Whether it helps depends on the environment: measure with the peer receiver test.

### Peer Receiver Test (tools/linux_sink) {#receiver}

The link quality window shows what A2DPWB sends. To see what actually arrives, run `tools/linux_sink/a2dpwb_sink.py` on a second PC with Linux. It drives that PC's ordinary Bluetooth adapter - the built-in one is fine - directly (Bumble over the HCI user channel) as an A2DP sink for every codec including LDAC, measures every media packet that arrives and serves the statistics over the network:

Copy the `tools/linux_sink` folder to the Linux PC (the folder alone is enough), then:

```bash
sh setup.sh      # once: decoder libraries, Python environment (Ubuntu / Debian, Python 3.11+)
sudo sh run.sh   # start the receiver (root: it takes the Bluetooth adapter)
```

In debug mode, **Debug > Peer Receiver Test...** finds the receiver on the local network by itself. **Start** streams a steady test tone (or the system audio) to its Bluetooth adapter with the chosen codec, quality, sample rate and bit depth - no profile or pairing step needed - and the window shows what was sent next to what arrived: bitrate, lost and late packets (RTP sequence numbers), jitter and gaps, dropouts and skips of a modelled playout buffer, stream / decode errors and the RSSI at the receiver (relative to its golden receive range: 0 = fine, negative = too weak). Each step (find receiver, statistics, receiver settings, Bluetooth connection, codec, audio arrives) is shown as OK / NG, with a log that can be copied. In CLI mode, `--remote-sink auto` does the same: it finds the receiver, streams to it unless `-d` names another device, and prints its statistics every 5 seconds and at the end.

Both sides count the packets of a stream from its first packet, and after **Stop** the final counts stay shown ("at stop"; the receiver's arrive about a second later), so the packets sent and received can be compared: they should be equal. Bluetooth retransmits lost radio packets inside the controllers, and A2DPWB does not give up on a packet while the link is up, so a poor radio link shows as gaps, jitter and dropouts at the receiver rather than as lost packets; when the link cannot keep up, packets are dropped before sending on the sending side. The number of retransmissions is not available to either side.

The receiver's adapter is not available to its own Bluetooth stack while the tool runs. Discovery uses a UDP broadcast on port 51201, so both PCs must be on the same network segment (otherwise enter the address). See `tools/linux_sink/README.md` for all options and the statistics protocol.

## Troubleshooting

### Pairing Problems After Updating

A2DPWB pairs using SSP (Just Works) with **General Bonding**, and the link keys are saved so the headphones can reconnect without pairing again. If you paired the headphones with an older A2DPWB build, remove the A2DPWB pairing on the headphones (see the headphones' manual for clearing the pairing list) and pair again.

### Connection Timeouts

A2DPWB registers SDP records (A2DP Source, AVRCP Controller / Target) so the headphones can identify it as an audio source. If a connection attempt times out, the half-open connection is torn down, so you can simply retry in the same session without restarting A2DPWB. Make sure the headphones are powered on, in range, and not connected to another device.

A failed connection is tried again by itself (up to 3 attempts) when the link came up and then carried nothing, and when the headphones did not answer. If the headphones no longer know the pairing (reset, or paired with another PC), the stored key is dropped and pairing is done once more. The error then says what happened:

| Message | Meaning |
|:--|:--|
| The device did not answer | Off, out of range, or not connectable (a new device: pairing mode) |
| The connection came up but then carried nothing | Radio conditions: try again, or move the adapter / the device |
| The device answered but then ended the connection itself | It does not accept a connection now: turning off, charging, or connected to another device |
| Authentication failed, also after pairing again | Put the device into pairing mode, or remove its pairing list entry, and try again |
