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

## The Window

Start **A2DP Windows Bridge** from the Start menu (or `a2dpwb.exe`). The menu on the left has five screens; it folds to icons when the window is narrow. The window only shows what the resident `a2dpwb-core.exe` does: closing it does not stop streaming (see [Tray and Flyout](#tray)).

### Home

- The device of the current profile, its state, and the codec in use with its bitrate. When a different codec than the profile's is used, the codec badge says which one and why
- **Connect** / **Disconnect**, the biggest button on the screen
- **Volume**: the headphones' volume (see [Volume](#volume))
- The profile switcher, the capture source (with **Edit**) and the link quality with a small graph
- An error comes with its cause and, where there is one, the button that fixes it (**Assign WinUSB**, **Open Download Page**, **Restart Bluetooth**, **Try again**, **Edit** the profile, ...)

### Profiles

A profile is a device plus its sound settings. The cards show the device, the codec and the capture source, with **Connect** (or **Disconnect** while it streams), edit and delete. **New Profile** and the edit button open the editor:

- **Device**: **Scan Devices** finds nearby devices with the USB adapter (about 10 seconds; put the headphones into pairing mode). Devices A2DPWB paired with before are tagged **Paired**. If the scan does not find them, enter the address under **Manual:** and click **Use**; typed or pasted addresses are formatted as you go (`aa-bb-cc-dd-ee-ff`, `AABBCCDDEEFF` and `AA BB CC DD EE FF` all become `AA:BB:CC:DD:EE:FF`)
- **Profile Name**: follows the device until you type one
- **Codec**: Auto or one codec, as cards with three bars (sound quality, low latency, stability). **Use another codec if this one is not available** (on by default, see [Explicitly Selected Codec Not Offered](#codec-fallback))
- **Capture Source**: System Loopback, Virtual Device or Application (see [Capture Modes](#capture-modes))
- **Windows Volume Sync** (see [Volume](#volume))
- **Advanced** (quality, sample rate, bit depth, packet size): offers only what the chosen codec can do
  - **Quality**: LDAC HQ (990 kbps) / SQ (660 kbps) / MQ (330 kbps); LHDC V5 High (1000 kbps) / Medium-High (900 kbps) / Standard (500 kbps) / Mobile (400 kbps), the four levels Android offers; AAC and SBC have quality levels too, the aptX family a fixed bitrate
  - **ABR (Adaptive Bit Rate)**: for LDAC and LHDC V5 (see [ABR](#abr))
  - **Sample Rate**: Auto follows the capture device
  - **Bit Depth**: 16 or 24 bit for LDAC and LHDC V5 (Auto: 24 bit); 16 bit for the others
  - **Max media packet size (bytes)** (see [Max Media Packet Size](#max-packet))

The defaults give the best sound; most profiles need nothing under **Advanced**.

### Diagnostics

- **Link Quality** while streaming: actual bitrate, packets, packets dropped before sending, audio dropped at capture, send queue, radio time used, signal (RSSI) and the AFH channels (see [Link Quality and AFH](#link-quality))
- **Connection Flow**: the eleven stages of the latest connection attempt (start, Bluetooth adapter, ACL / AVDTP connect, capability discovery, codec selection, audio capture, stream configuration, encoder, stream start, audio sending, connection upkeep with disconnects / reconnects) as OK, Warning or Failed, with the time it took. **Copy Flow** copies it
- **Log**: live, filtered by level (all, info and above, warnings and errors, errors only) and by text. **Copy Shown Log** copies what is shown
- **Start HCI Capture** / **Stop HCI Capture** (see [Verifying the Media Stream](#verify-stream))
- **Copy Diagnostics**: version, OS, adapter, profile, the flow, warnings / errors and the recent log on the clipboard, ready to paste into a bug report. **Mask Bluetooth addresses** (on by default) keeps only the vendor part (`AA:BB:CC:xx:xx:xx`)

With **Settings > Developer mode**, Diagnostics also has a **Peer Receiver Test** tab (see [Peer Receiver Test](#receiver)).

### Adapter

- The detected USB Bluetooth adapters with their chip, **Driver** and **Firmware**. **Refresh** looks again
- **Assign WinUSB** / **Restore driver** (see [Setup](setup#winusb)); Windows asks for administrator permission each time
- **Bluetooth Chip** for the firmware, with **Open Download Page** and **Open Config Folder** (see [Firmware](setup#firmware))
- **AFH**: Auto, Off or Manual Wi-Fi channels (see [Link Quality and AFH](#link-quality))
- **Bluetooth stack**: stopped (it starts with the first scan or connection), starting, ready or error, and **Restart Bluetooth**

### Settings

| Section | Settings |
|:--------|:---------|
| Appearance | Language, theme (System / Light / Dark) |
| Startup | **Start with Windows** and **Start in the tray** (see [Start with Windows](#autostart)), **Minimize to System Tray** (see [Tray and Flyout](#tray)) |
| Headphones | **Headphone Buttons Control Playback** (see [Headphone buttons](#headphone-buttons)), **Remember Headphone Volume** (see [Volume](#volume)) |
| Updates | **Check for Updates**, **Check for Updates on Startup**, **Include Pre-releases in Update Checks** (also offer beta releases), **Update now** (see [Updates](#updates)) |
| Advanced | **Developer mode** (adds the receiver test to Diagnostics and more details to the log; no restart needed), **Settings folder** with **Open Config Folder**, **Run setup** |
| About A2DPWB | Version, license, third-party licenses, **Quit A2DPWB** |

### First-Run Setup

The first start (no profile yet, or no adapter with WinUSB) opens **Set up A2DPWB**: choose the adapter, driver and firmware, find your headphones, test. See [Setup](setup#first-run). **Settings > Run setup** opens it again.

## Tray and Flyout {#tray}

A2DPWB keeps running in the system tray, with or without the window:

- **Left-click** the tray icon for the flyout: the device and its state, **Connect** / **Disconnect**, the volume, the profile switcher, the codec, the link quality and **Open app**. Esc or a click elsewhere hides it. If an error needs a screen of the main window (a profile, the adapter), its button opens the window there
- **Right-click** for the menu: **Open A2DPWB**, the first 10 profiles (the active one checked; click one to connect with it), **Disconnect** and **Exit**. The menu opens at once, without starting the window
- The tooltip says whether A2DPWB is idle or streaming

With **Settings > Minimize to System Tray** on (the default), minimizing or closing the window only hides it in the tray, and streaming goes on. With it off, the window minimizes to the taskbar, and closing it quits A2DPWB. **Exit** in the tray menu or **Settings > Quit A2DPWB** stops streaming, releases the adapter and closes A2DPWB completely.

## Start with Windows {#autostart}

**Settings > Start with Windows** starts A2DPWB when you sign in. With **Start in the tray** on, it starts in the background: only the tray icon appears, and the window opens from it. With it off (the default), the main window opens. It is the value `A2DPWB` under `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run`, with the data `"<folder>\a2dpwb-core.exe"`, followed by `--background` when **Start in the tray** is on. To turn it off by hand, delete that value. Installing v2 over v1 turns **Start in the tray** on, as v1 always started in the tray.

## CLI Mode

`a2dpwb-core.exe --cli` streams from a console, with the same core as the window. Quit A2DPWB first (tray > **Exit**): only one program can use the adapter at a time. The CLI does not read the profiles; everything is given as options.

### Basic

```bash
# Auto-select best codec
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF
```

Ctrl+C stops streaming. The CLI prints the state, the codec and bitrate, and at the end the number of packets sent and dropped.

### Codec Selection

```bash
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac      # LDAC
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxhd    # aptX HD
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxll    # aptX Low Latency
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptx      # aptX
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aac       # AAC
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c sbc       # SBC
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c lhdcv5    # LHDC V5
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c opus      # Opus (experimental; 48 kHz, 256 kbps)
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c opus05    # PipeWire's Opus 05 (experimental; to PipeWire sinks)
```

Auto-select priority: LDAC > aptX HD > aptX LL > aptX > LHDC V5 > Opus (experimental) > AAC > SBC XQ > SBC (FastStream and Opus 05, both experimental, only when chosen)

If the requested codec is not offered by the headphones, the CLI stops with an error that says why; use `-c auto` (the default) to take the best codec they offer. aptX Adaptive is never selected -- see [aptX Family and aptX Adaptive Compatibility](#aptx-compatibility).

### Quality

```bash
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q hq   # 990 kbps (default)
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q sq   # 660 kbps
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q mq   # 330 kbps

# LHDC V5: hq 1000 / mhq 900 / sq 500 / mq 400 kbps (Android's four levels)
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c lhdcv5 -q mhq  # 900 kbps
```

### ABR (Adaptive Bit Rate) {#abr}

```bash
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -a
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c lhdcv5 -a
```

ABR automatically lowers the bitrate when the Bluetooth connection is unstable, and raises it when conditions improve. LDAC moves between its quality modes; LHDC V5 follows Android's policy and stays between 160 and 400 kbps. In the window, ABR is in the profile editor under **Advanced**.

### Device Discovery

```bash
# Nearby devices, found with the USB adapter (10 seconds; --scan 20 for longer)
a2dpwb-core.exe --cli --scan
```

Put the headphones into pairing mode first.

### All Options {#all-options}

| Option | Description |
|:-------|:------------|
| `-d <addr>` | Bluetooth address of the headphones / receiver (`AA:BB:CC:DD:EE:FF`) |
| `-c <codec>` | `ldac`, `aptxhd`, `aptxll`, `aptx`, `aac`, `sbc`, `lhdcv5`, `opus`, `opus05`, `auto` (default: `auto`) |
| `-q <mode>` | Quality: `hq` (default), `sq`, `mq`; LHDC V5 also `mhq` (900 kbps) |
| `-a` | ABR (LDAC and LHDC V5) |
| `-m <mode>` | Capture mode: `loopback` (default), `virtual`, `app` |
| `--audio-device <id>` | Render device for `-m virtual` |
| `--app <exe>` | Executable for `-m app` (e.g. `spotify.exe`) |
| `--rate <hz>` | Capture sample rate (default: the device's) |
| `--bits <16\|24>` | Bit depth (default: 24 where the codec carries it) |
| `--max-packet <bytes>` | Max media packet size, 679-1679, default 1023 (see [Max Media Packet Size](#max-packet)) |
| `--afh <auto\|off\|6,11>` | AFH host channel classification (see [Link Quality and AFH](#link-quality)) |
| `--remote-sink <host[:port]\|auto>` | Show what the peer receiver gets; without `-d`, stream to that receiver (see [Peer Receiver Test](#receiver)) |
| `--scan [seconds]` | List nearby devices and exit (default 10 seconds) |
| `-h` | Show the help |

For development and testing there are also `--hci-tcp <host:port>` (a virtual controller over TCP instead of the WinUSB adapter), `--test-tone` (1 kHz left / 1.5 kHz right instead of captured audio), `--duration <s>` (stop after this many seconds) and `--hci-capture <file.pklg>` (HCI capture of the whole run); see [Building](building#emu).

Without `--cli`, `a2dpwb-core.exe` is the resident program: it starts the window too, except with `--background` (tray icon only, used by **Start with Windows** when **Start in the tray** is on). `a2dpwb-core.exe --quit` asks a running A2DPWB to stop streaming and exit (the installer uses it).

## Capture Modes {#capture-modes}

A2DPWB supports three audio capture modes. Each records a different point in the audio path:

| Mode | What it records |
|:-----|:----------------|
| System Loopback | What the Windows default output device plays: all apps mixed |
| Virtual Device | What apps play into the selected device (e.g., VB-CABLE) |
| Application | Only what one app (and its child processes) plays, after the app's own processing. Needs Windows 11 or Windows 10 version 2004 (build 19041) or later |

The profile editor shows the same explanation under **Capture Source**.

System Loopback copies the audio; it does not take it away from the default output device. The audio keeps playing on that device (speakers, an HDMI TV, ...) as well as on the Bluetooth headphones. To hear it only on the headphones, use Virtual Device mode with a device that does not play anywhere, such as VB-CABLE.

In Virtual Device mode, **Auto-switch default device** makes the selected device the Windows default output while streaming, so apps play to it without changing Windows settings. When streaming stops, A2DPWB switches back to the previous default device.

- The **Audio Device** list shows only the devices that A2DPWB detects as virtual audio devices. Without one, the editor offers **Download VB-Cable**.
- The default communication device (used by calling apps) is not changed.
- Some apps keep playing on the previous device until they are restarted.
- If A2DPWB exits unexpectedly, the default device is not switched back. Change it in Windows Sound settings.

### Application mode {#application-mode}

Application mode sends the audio of one app, for example a music player, a browser or an effects app. Choose the app in the **App** list. The list shows the apps that are playing audio right now; if yours is missing, start playback in it and click **Refresh**. The app's child processes are included, so a browser that plays audio from a separate process works too.

- The app does not have to be running. A2DPWB connects to the headphones anyway and starts sending once the app is up. When the app quits during streaming, A2DPWB stays connected and picks the app up again when it is restarted.
- The app keeps playing on its own output device as well. To hear it only on the headphones, mute that output device (its master volume): this does not affect what A2DPWB captures. Do not mute the app in the Windows Volume Mixer, though, or the headphones get silence too.
- The app's slider in the Windows Volume Mixer changes the level sent to the headphones (and on its output device). The master volume and the volume keys do not change the audio sent; with the profile's **Windows Volume Sync** on Auto they change the headphones' volume instead (see [Volume](#volume)).
- Windows converts the audio to the sample rate that A2DPWB asks for, so the device format in Sound settings does not limit it.
- On Windows 10 before version 2004 (build 19041), Windows has no per-app capture. Connecting with an Application profile then shows an error that asks for another capture source.

### Volume {#volume}

While streaming, Home and the tray flyout show a **Volume** slider when the headphones support AVRCP absolute volume. It sets the volume on the headphones themselves, so the audio sent keeps its full resolution. The slider also follows the volume buttons and touch controls on the headphones.

**Windows Volume Sync** in the profile makes a Windows volume and the headphones' volume one volume while streaming:

| Choice | Windows volume synced |
|:-------|:----------------------|
| Auto (default) | The device A2DPWB captures: the default output device in System Loopback and Application mode, the selected device in Virtual Device mode |
| Off | None: the Windows volume and the headphones' volume are separate |
| An output device | That device's volume |

- The headphones' volume buttons and gestures, and the **Volume** slider, move the synced Windows volume, and its slider in Sound settings moves the headphones' volume.
- The volume keys and the Windows volume flyout always act on the default output device, so they change the headphones' volume only when the default output device is the one synced (as with Auto, except in Virtual Device mode without **Auto-switch default device**).
- When streaming starts, the Windows volume takes over the headphones' volume; when streaming stops, the device gets back the volume it had before.
- With Auto, the audio is captured before the Windows volume is applied, so it is still lowered only once, on the headphones. If the synced volume also changes the captured audio, the volume applies twice: choose Off then (see [System-wide effects apps](#effects-apps)).
- Mute is not synced: muting the output device keeps the PC silent and does not mute the headphones.

**Settings > Remember Headphone Volume** (off by default) saves the headphones' volume for each device and sets it again as soon as they connect, before any audio is sent. Headphones otherwise start at their own last volume, which may be much louder than the volume used last time with A2DPWB (e.g., after using them with a phone).

### Headphone buttons {#headphone-buttons}

The play/pause, next and previous track controls on the headphones (buttons or touch gestures, e.g. a double tap on the WH-1000XM4) control playback on the PC while streaming: A2DPWB passes them to Windows as media keys, which go to the app shown in the Windows media controls (Spotify, a browser, a media player...). Stop is passed as well.

- Play and pause both act as the Windows Play/Pause key, so a tap always switches between playing and paused, even if the headphones' idea of the state is out of date.
- A2DPWB tells the headphones whether audio is playing (from whether there is audio to capture), so headphones that pause when taken off send the right command.
- Fast forward / rewind (holding a button) have no Windows media key and are ignored.
- Volume gestures change the headphones' own volume; with **Windows Volume Sync** the synced Windows volume follows (see [Volume](#volume)).
- Turn this off with **Settings > Headphone Buttons Control Playback**, e.g. if another program already reacts to the media keys twice.

This needs AVRCP on the headphones, which nearly all have. Without it, streaming works as usual, just without the buttons.

### System-wide effects apps {#effects-apps}

A system-wide effects app (an equalizer, a sound enhancer, a virtual surround app, ...) usually installs its own virtual output device. Other apps play into that device; the effects app processes the audio and plays the result to the output device you choose in the effects app. Which A2DPWB mode you choose decides whether the headphones get the effects:

| Setting | What reaches the headphones |
|:--------|:----------------------------|
| Virtual Device: the effects app's own virtual device | The audio **before** the effects: what apps send into the effects app |
| Application: the effects app | The audio **after** the effects: what the effects app plays (recommended) |
| Virtual Device: the effects app's output device (e.g., VB-CABLE set as its output) | The audio after the effects (for Windows versions without Application mode) |

In Application mode, the effects app's output device can be any device: A2DPWB takes what the app plays, not what the device plays, so the device is only somewhere for the audio to go. So that the PC does not play the audio at the same time, choose an output nobody listens to (e.g., a monitor's HDMI audio) or mute that device's master volume. A virtual device such as VB-CABLE also works as a silent place to send it, but it is not required. The device must be enabled and connected.

If the effects app's own virtual device is the Windows default output device, its volume also lowers what the effects app receives. With **Windows Volume Sync** on Auto, a volume change then applies twice (on the effects app's input and on the headphones). Set it to Off in that profile, and use the headphones' volume or the **Volume** slider.

#### Why A2DPWB has no virtual output device of its own {#no-virtual-device}

A Bluetooth headphones entry in the Windows output device list, which an effects app could select directly, would need a kernel-mode audio driver. A2DPWB does not include one:

- Writing and maintaining a kernel audio driver is a large job, and a bug in it can crash Windows (blue screen).
- Windows 10 and 11 load only drivers signed by Microsoft. Getting that signature needs an EV code signing certificate (several hundred US dollars a year) and a Microsoft Partner Center account, normally registered as a company.
- The only way around the signature is Windows test signing mode, which lowers system security and breaks some software (e.g., games with anti-cheat). A2DPWB will not rely on it.

Application mode, or a free virtual device such as VB-CABLE, covers the same use.

- Windows has no public API for changing the default device. A2DPWB uses the undocumented IPolicyConfig interface, as other audio switching tools do (Windows 10 and 11).

{: .warning }
System Loopback and Virtual Device use WASAPI shared mode, so the capture sample rate is determined by the device's **Default Format** in Windows Sound settings (typically 48 kHz). To use LDAC at 96 kHz, change the output device's format to 96 kHz in **Sound settings > Device properties > Advanced > Default format**.

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

The profile editor shows the same comparison as three bars on each codec card.

## aptX Family and aptX Adaptive Compatibility {#aptx-compatibility}

A2DPWB supports three aptX codecs, all encoded with [libopenaptx](https://github.com/pali/libopenaptx). aptX Adaptive is **not** supported.

{: .warning }
**aptX HD and aptX Low Latency are experimental.** They follow the Android / PipeWire implementations and pass encode/decode round-trip tests, but have not yet been verified with real headphones.

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

Auto-select priority: LDAC > aptX HD > aptX LL > aptX > LHDC V5 > Opus (experimental) > AAC > SBC XQ > SBC (FastStream and Opus 05, both experimental, only when chosen)

### Explicitly Selected Codec Not Offered {#codec-fallback}

When you request a specific codec that the headphones do not list:

| Where | Behavior |
|:------|:---------|
| Profile, **Use another codec if this one is not available** on (default) | Connects with the best codec the headphones offer, in the Auto priority order, and the codec badge says which codec is used instead and why. |
| Profile, option off | The connection stops with an error ("Device does not support ..."). If you requested aptX, aptX HD or aptX LL and the headphones offer only aptX Adaptive, the message says so and suggests Auto, AAC or SBC. |
| CLI (`-c <codec>`) | Stops with the same error. Use `-c auto` to connect with the best codec offered. |

The option is in the profile editor under the codec cards (not used with Auto).

### Codec Missing While the Headphones Are Connected to Another Device {#sep-in-use}

Headphones offer each codec on a stream endpoint (SEP). On a dual (multipoint) connection, an endpoint that streams to the phone or PC is reported as **in use**, and its codec cannot be used until that stream stops. Some headphones also offer high-resolution codecs (LDAC, L2HC, ...) only while a single device is connected. A2DPWB then logs:

```
remote SEP 6: sink audio, IN USE (codec not queried)
remote SEPs in use by another connection: 1 of 6 audio sink SEP(s); ...
capability discovery complete (LDAC=false ... , 1 of 6 audio sink SEP(s) in use)
```

and the error says that endpoints are in use by another device. Disconnect the headphones from the phone / PC (or turn dual connection off in the headphones' app) and connect again. A PC whose built-in Bluetooth is still paired with the headphones counts as another device, too.

### Sample Rate

aptX, aptX HD and aptX LL support only 44.1 kHz and 48 kHz. A2DPWB picks one of the rates the headphones advertise (preferring the current Windows device rate, otherwise 48 kHz) and lets WASAPI resample the capture. You do not need to change the Windows Sound settings for aptX.

### Checking What Your Headphones Offer

1. Connect to the headphones.
2. Open **Diagnostics** and type `remote` into the log filter.

The same lines are in the log files in the config folder (`%APPDATA%\A2DPWB\logs\a2dpwb-core.<date>.log`); in CLI mode they are printed to the console. The capability lines show exactly which codecs the headphones list:

```
remote supports aptX (SEID ..., [...])
remote supports aptX Adaptive (SEID ...) - never selected (no open encoder); classic aptX is used only if the remote lists it
remote vendor codec 0x........./0x.... (SEID ...) - unsupported
capability discovery complete (LDAC=false aptX HD=false aptX LL=false aptX=true aptX Adaptive=true [not encodable] SBC=true AAC=true LHDC V5=false Opus=false)
```

- `aptX=true` -- classic aptX is available (even if `aptX Adaptive=true` too).
- `aptX=false`, `aptX HD=false`, `aptX LL=false` with `aptX Adaptive=true` -- the headphones offer only aptX Adaptive; use Auto, AAC or SBC.
- `remote vendor codec ... - unsupported` -- another vendor codec that A2DPWB does not implement.

## Max Media Packet Size (per Profile) {#max-packet}

**Max media packet size** in each profile (profile editor > **Advanced**; CLI: `--max-packet <bytes>`) sets the upper limit for the audio data in each Bluetooth packet, from 679 to 1679 bytes, so headphones that need a different value can have their own. The headphones' own limit (MTU) always applies too; A2DPWB uses the smaller of the two. The change applies from the next connection. Profiles saved by older versions take the value that was set globally before.

**1023 (default) is recommended.** It fits one Bluetooth baseband packet (3-DH5). Larger values save only 1-2% overhead, and each lost packet then loses more audio. 679 fits one 2-DH5 packet and is the minimum LDAC accepts.

## Verifying the Media Stream (a2dpwb_decode) {#verify-stream}

Most headphones cannot show which codec is in use. **Diagnostics > Start HCI Capture** / **Stop HCI Capture** records an HCI packet log (`hci_<date>_<time>.pklg` in the config folder) with the codec negotiation and every media packet sent. At LDAC 990 kbps it grows by roughly 0.5 GB per hour, so capture only as long as you need. In CLI mode, `--hci-capture <file.pklg>` records the whole run.

You can start a capture before connecting or while already streaming. If it starts during a connection, A2DPWB first writes the connection setup packets it remembered (L2CAP channel setup and AVDTP signaling, including the codec negotiation), so the file can still be analysed. Diagnostics shows the file path while it records and after it stops.

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

## Link Quality and AFH {#link-quality}

**Diagnostics > Link Quality** shows what A2DPWB sends while streaming: the actual bitrate, packets sent, packets dropped before sending (send queue full: the radio link could not keep up), audio dropped at capture, the send queue, the radio time used, and the **Signal**: the RSSI of the connection, relative to the controller's golden receive range (0 = fine, negative = too weak; not an absolute level), and how many of the 79 Bluetooth channels the link hops on (AFH). Home and the tray flyout sum it up as Good, Fair or Poor.

Bluetooth avoids busy channels by itself (AFH), but it only learns about Wi-Fi from errors. A2DPWB also tells its controller which channels to avoid (AFH host channel classification), for every connection:

| AFH | |
|:--|:--|
| Auto (default) | Only the 2.4 GHz Wi-Fi channel this PC is connected on (checked again every 30 seconds while connected). Nothing when the PC is not on 2.4 GHz Wi-Fi: disconnected, on 5 or 6 GHz, or wired |
| Off | Nothing: the controller decides alone |
| Manual, e.g. `6,11` | These Wi-Fi channels (1 to 14) |

Auto leaves the networks the PC only hears to the controller's own AFH: avoiding a neighbour's busy channel takes 22 MHz from the hopping and can make the link worse, not better. To avoid such a channel anyway, enter it as Manual.

At least 20 Bluetooth channels always stay in use. Set it on the **Adapter** screen, with `"afh"` in `settings.json` (config folder), or with `--afh` in CLI mode. The radio line shows which Wi-Fi channels are avoided. Whether it helps depends on the environment: measure with the peer receiver test.

## Peer Receiver Test (tools/linux_sink) {#receiver}

The link quality view shows what A2DPWB sends. To see what actually arrives, run `tools/linux_sink/a2dpwb_sink.py` on a second PC with Linux. It drives that PC's ordinary Bluetooth adapter - the built-in one is fine - directly (Bumble over the HCI user channel) as an A2DP sink for every codec including LDAC, measures every media packet that arrives and serves the statistics over the network:

Copy the `tools/linux_sink` folder to the Linux PC (the folder alone is enough), then:

```bash
sh setup.sh      # once: decoder libraries, Python environment (Ubuntu / Debian, Python 3.11+)
sudo sh run.sh   # start the receiver (root: it takes the Bluetooth adapter)
```

On the Windows PC, quit A2DPWB (tray > **Exit**) and run:

```bash
a2dpwb-core.exe --cli --remote-sink auto -c ldac --test-tone
```

`--remote-sink auto` finds the receiver on the local network, streams to its Bluetooth adapter unless `-d` names another device, and prints the receiver's statistics every 5 seconds and at the end: bitrate, lost and late packets (RTP sequence numbers), jitter and gaps, dropouts and skips of a modelled playout buffer, stream / decode errors and the RSSI at the receiver (relative to its golden receive range: 0 = fine, negative = too weak). Without `--test-tone` it sends the system audio. `--remote-sink <host[:port]>` names the receiver instead of searching for it.

{: .note }
With **Settings > Developer mode**, Diagnostics has a **Peer Receiver Test** tab. In this version it is a preview: it saves the receiver address, but it does not stream yet. Use the command line above.

Both sides count the packets of a stream from its first packet, so the packets sent and received can be compared: they should be equal. Bluetooth retransmits lost radio packets inside the controllers, and A2DPWB does not give up on a packet while the link is up, so a poor radio link shows as gaps, jitter and dropouts at the receiver rather than as lost packets; when the link cannot keep up, packets are dropped before sending on the sending side. The number of retransmissions is not available to either side.

The receiver's adapter is not available to its own Bluetooth stack while the tool runs. Discovery uses a UDP broadcast on port 51201, so both PCs must be on the same network segment (otherwise enter the address). See `tools/linux_sink/README.md` for all options and the statistics protocol.

## LE Audio (experimental) {#le-audio}

A2DPWB can also stream to LE Audio headphones and earbuds (LC3 over connected isochronous streams, BAP unicast), with headset volume (VCP), media buttons (MCP), both earbuds of a pair (CSIP) and a low-latency mode for gaming headsets (GMAP). This is **experimental**, chosen per profile: it is only used by a profile for an LE Audio device. It has only been tested against a Bluetooth emulator, **not yet with real LE Audio headphones**, so it may not work with yours; use the classic A2DP profiles for everyday listening.

What it needs:

- A USB Bluetooth adapter with **Bluetooth 5.2 or later that supports isochronous channels (ISO)**, with WinUSB assigned as for A2DP. An adapter without ISO support is refused with "adapter does not support LE Audio (no ISO buffers)".
- Headphones or earbuds that support LE Audio (LC3); some need LE Audio turned on in their own app first.

How to use it:

1. Put the headphones into pairing mode and scan for devices in a profile. LE Audio devices are listed with an **LE Audio** tag, a pair of earbuds as **LE Audio set** (pick either one: A2DPWB finds the other). A headphone that also does A2DP is listed twice, once per kind.
2. Tick **LE Audio device (experimental, LC3)** in the profile. There is no codec or quality choice (A2DPWB picks the best LC3 configuration the headphones accept), and only System Loopback and Virtual Device capture are available.
3. Under it, **Low latency for gaming headsets (GMAP, experimental)** is on by default: with headsets that support GMAP's game role it uses shorter LC3 frames; other headsets get the media configuration either way.

There is no LE Audio switch in Settings. If the LE side ever disturbs an adapter, `"le_audio_experimental": false` in `settings.json` (**Settings > Settings folder**) turns it off altogether.

In CLI mode: `--le-audio -d <address>` streams to an LE Audio device, `--le-device <address>` adds another device of the group (the second earbud), `--le-scan` lists the LE Audio devices nearby, and `--le-low-latency` / `--le-preset <name>` choose the configuration (`a2dpwb-core.exe --cli -h` lists all LE options).

## Updates {#updates}

A2DPWB checks GitHub Releases at most once a day (**Settings > Check for Updates on Startup**; **Include Pre-releases in Update Checks** also offers beta releases). When a new version is available, Settings says so:

- **Installed copy**: **Update now** downloads the signed update. While streaming, it waits and installs when streaming stops, so an update never cuts the audio. A2DPWB then closes, installs the new version and starts again by itself.
- **Portable copy** (or a build without update signing): **Open download page** opens the release page.

## Troubleshooting

Sound that drops out or stutters, a connection that fails, or a bit rate lower than expected: see **[Dropouts and Connection Problems](wireless)**, which sums up what the reports so far turned out to be and what to check first.

### The Window Says the Core Is Not Running

The window needs `a2dpwb-core.exe` next to it and starts it by itself. "The A2DPWB core is not running" means it could not be started: reinstall A2DPWB (or extract the portable zip again). "The core is a different version" means the two programs come from different versions: install the same version of both (the latest release does this).

### Assigning or Restoring the Driver Fails

- "The administrator prompt was declined": the driver was not changed. Click the button again and allow the prompt.
- "Changing the driver failed": see **Diagnostics > Log** for the details.
- "The adapter did not return to its previous driver": if the adapter was set up with Zadig, Windows may pick Zadig's WinUSB driver package again. Remove that driver package in Device Manager (adapter > Uninstall device, with "Attempt to remove the driver for this device"), then unplug and replug the adapter.

### The Bluetooth Adapter Does Not Respond {#adapter-no-answer}

If the adapter sends nothing back after it is opened (it does not answer HCI Reset), A2DPWB gives up after 5 seconds, opens it once more and, if it still does not answer, shows "The Bluetooth adapter does not respond". Unplug the adapter, plug it back in and try again (or **Restart Bluetooth**). Restarting A2DPWB is not needed: a failed start releases the adapter and every new start initializes it again.

To keep an adapter from getting stuck, A2DPWB resets its USB pipes when it opens it and sends HCI Reset when it closes it.

### Pairing Problems After Updating

A2DPWB pairs using SSP (Just Works) with **General Bonding**, and the link keys are saved so the headphones can reconnect without pairing again (v2 reads the link keys of v1). If you paired the headphones with an older A2DPWB build and they no longer connect, remove the A2DPWB pairing on the headphones (see the headphones' manual for clearing the pairing list) and pair again.

### Connection Timeouts

A2DPWB registers SDP records (A2DP Source, AVRCP Controller / Target) so the headphones can identify it as an audio source. If a connection attempt times out, the half-open connection is torn down, so you can simply retry without restarting A2DPWB. Make sure the headphones are powered on, in range, and not connected to another device.

A failed connection is tried again by itself (up to 3 attempts) when the link came up and then carried nothing, and when the headphones did not answer. If the headphones no longer know the pairing (reset, or paired with another PC), the stored key is dropped and pairing is done once more. The error then says what happened:

| Message | Meaning |
|:--|:--|
| The device did not answer | Off, out of range, or not connectable (a new device: pairing mode) |
| The connection came up but then carried nothing | Radio conditions: try again, or move the adapter / the device |
| The device answered but then ended the connection itself | It does not accept a connection now: turning off, charging, or connected to another device |
| Authentication failed, also after pairing again | Put the device into pairing mode, or remove its pairing list entry, and try again |

A lost connection is reconnected by itself (up to 10 attempts). When the PC goes to sleep while streaming, A2DPWB reconnects after it resumes; these attempts are not counted.
