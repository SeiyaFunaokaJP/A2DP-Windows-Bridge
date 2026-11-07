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

Start **BlueCaster** from the Start menu (or `bluecaster.exe`). The menu on the left has the screens below (Home, Codecs, Audio Devices, Adapters, Diagnostics, Settings, About, and in developer mode Developer and, when turned on there, Volume Mixer); it folds to icons when the window is narrow. The window only shows what the resident `bluecaster-core.exe` does: closing it does not stop streaming (see [Tray and Flyout](#tray)).

### Home

- A card for each saved audio device, side by side: its state, battery and, for the one connected, the codec in use (a list in place of its name, see below) with its bitrate. When a different codec than the one chosen is used, the codec line says why beside its name for a few seconds, e.g. **aptX Adaptive (Opus is unsupported)** (or *is in use*, *is experimental*), then only its warning mark stays (hover for why); the status line just says **Connected**
- **Connect** / **Disconnect** under each card. One device is connected at a time: **Connect** on another device disconnects the one in use and connects that one
- **Volume**: the headphones' volume (see [Volume](#volume)); click the speaker to mute (silence is sent) and again to unmute. The cards of the other devices keep the slider, disabled
- The codec of the device in use, chosen from the list on its card to switch quickly (say, one for games and one for music): the codecs the device supports, as on [Codecs](#codecs), by name only. It shows the codec in use, also under **Auto-select**; choosing a codec turns Auto-select off (turn it back on on Codecs). A choice applies at once (the device connects again with it; the next choice waits until it is connected, as on Codecs)
- For the device in use, the link quality with a small graph, on its card while it streams. The output source is chosen on **Audio Devices**; while an application set as the output source is not running yet, the device's card says so
- An error comes with its cause and, where there is one, the button that fixes it (**Replace with WinUSB driver**, **Open Download Page**, **Restart Bluetooth**, **Try again**, **Codecs**, **Audio Devices**, ...)
- Above the cards, until closed: a warning when another Bluetooth adapter is active in Windows, and when OPPO earbuds turned Hi-Res on (see [Codecs](#codecs))

The sidebar shows the same for every device on every screen, and the tray flyout for the device in use.

### Codecs {#codecs}

The codec of each saved audio device, one section per device (a dot that is green while the device is connected, its name, **Paired** / **Audio** tags; the address is on **Audio Devices**) with its codec cards, highest sound quality first unless you put them in your own order, each with three bars (sound quality, low latency, stability) and whether the device offered it when it last connected: **Supported** (green mark) or **Unsupported** (red mark). With **Show only supported codecs** on (the default), the cards shown need no **Supported** tag and have none. Huawei and Honor earbuds keep their **LDAC** card, as **Unconfirmed**, until they list it: they may do so only once it is chosen. OPPO earbuds that list a peak bit rate for LHDC V5 are asked once for Hi-Res when LHDC V5 streams, chosen or by Auto (not with quality **Low** or ABR): they connect again once for it, and Home says so when it raised their bit rate; Hi-Res then stays on in the earbuds (earbuds it gained nothing are set back). In developer mode, **Ask for Hi-Res again** in LHDC V5's settings forgets what the device listed and that it was asked, so its next connection asks again. Every codec is offered; the experimental ones (🧪 in the README's codec table) are marked only in the documentation.

- **Auto-select the codec with the highest quality** (one setting for every device, at the top of the page; on by default): each device uses the codec with the highest sound quality it supports, in this order: LHDC V5 > LDAC > aptX Adaptive > Opus > aptX HD > aptX > aptX LL > AAC > SBC XQ > SBC, with its default settings: high quality, 24-bit where the codec carries it and 48 kHz (another rate the device lists if it has no 48 kHz, 44.1 kHz first; Windows' own rate is resampled), as **Auto** in a codec's settings. The cards are read-only, in the default order, and the codec in use (or the one Auto will pick) is highlighted; clicking a card or its **...** asks whether to turn Auto-select off (**Disable Auto-select** / **Cancel**); once it is off, the card clicked is chosen (its **...** opens its settings). Turn it off to choose each device's codec and its settings yourself; a device with none chosen yet keeps Auto's pick. The codec chosen for a device is kept while Auto-select is on. Settings from before 2.0.2 that chose a codec for any device start with it off
- With it off, click a card to choose that device's codec; it is saved at once, and a device streaming connects again with it. A card the device does not support (shown with **Show only supported codecs** off) is chosen too: BlueCaster asks the device for it anyway, and if the device refuses it, the connection stops with an error saying so (choose another card or turn Auto-select on). A codec chosen while the device offered it and no longer offered later (earbuds switched to another codec) falls back to the first card in the device's order that it supports: the best codec it supports unless you changed the order (see [Explicitly Selected Codec Not Offered](#codec-fallback))
- With it off, drag a device's cards (the hand shows while the button is held; or press Alt with an arrow key on a card) to put its codecs in your own order; each device keeps its own order. A device with no codec chosen yet uses the first card it supports, and so does a chosen codec it does not support. FastStream and Opus 05 (the **Opus (PipeWire)** card) are never used unless chosen, wherever they stand. **Reset codecs order** resets it; it shows only while the cards shown are out of their default order (moved back, they are in the default order again). Under Auto-select the order is not used. A device streaming with a codec this changes connects again
- Choosing another codec for the device that is streaming connects it again with that codec. The cards (and the codec list on Home and in the tray flyout) then wait until it is connected again before the next choice, so a quick series of clicks does not break off a connection still being set up. Switching codecs shows no connection notification; connecting a device does
- **Show only supported codecs** (on by default; turn it off to show both supported and unsupported codecs): the codecs a device did not offer are left out. Before a device's first connection, all of them are shown
- **...** on a card opens that codec's settings. A change applies at once: for the codec in use it is saved and the device connects again with it, except the **Bitpool**, which changes while you listen; for another codec it is kept for when that codec is chosen. Each codec keeps its own settings: choosing another codec and coming back brings them back. Only what that codec can do, and what the device listed at its last connection, is offered. At the top, **Show only options *device* supports** (on by default; shown once the device has listed them) turned off offers everything the codec has, such as a sample rate, bit depth, quality or bitpool the device did not list (marked *(unsupported)*). The device may refuse them: the connection then stops with an error saying so. Turned on again, what the device did not list goes back to Auto. Each codec has its own; Auto-select never uses them:
  - **Quality**: LDAC **High** / **Medium** / **Low** (990 / 660 / 330 kbps), LHDC V5 **High** / **Medium-High** / **Medium** / **Low** (1000 / 900 / 500 / 400 kbps, Android's four levels); aptX Adaptive **High** / **Low Latency** (see [aptX Adaptive](#aptx-adaptive)); with LHDC V5's ABR on, the quality is greyed out (ABR stays at 400 kbps or below). AAC, Opus, Opus 05, SBC and SBC XQ have one, **High**, shown rather than chosen: the highest bit rate the device takes (ABR or VBR, where offered, still varies it; SBC and SBC XQ show the bit rate their bitpool and frame setup give). aptX, aptX HD, aptX LL and FastStream have a bit rate fixed by the codec. A quality saved by an earlier version that the codec no longer has (aptX Adaptive's Standard / Mobile, AAC's lower levels, ...) loads as High
  - **Adaptive Bit Rate (ABR)**: for LDAC, LHDC V5, aptX Adaptive, SBC and SBC XQ (see [ABR](#abr)); **VBR** for AAC
  - **Sample Rate**: Auto, shown with the rate it gives (e.g. *Auto (48000 Hz)*), is 48 kHz, or another rate the device lists if it has no 48 kHz, 44.1 kHz first (96 kHz adds latency and is less compatible, with no audible gain, and Windows mixes at 48 kHz); choose 96 kHz here to use it. Only the rates the device lists are offered (LHDC V5 has no 88.2 kHz; aptX Adaptive's 96 kHz only to a device that lists it, see [aptX Adaptive](#aptx-adaptive)); a device that lists only one rate shows it, without Auto
  - **Bit Depth**: 16 or 24 bit for LDAC and LHDC V5 (Auto: 24 bit; for LHDC V5 only when the device lists it), as far as the device lists them; 24 bit for aptX HD and aptX Adaptive, 16 bit for the others
  - **Bitpool** (SBC, SBC XQ): one slider with two handles, the minimum and the maximum bitpool, within the range the device lists. The maximum is the bitpool used; the minimum is the lowest one used (the bottom of the range the connection is set up with, and how far ABR may go down). **Auto** sets both back: the maximum by quality, the minimum the device's and ABR's own. The line below it shows the channel mode, fixed for each codec: joint stereo for SBC, dual channel for SBC XQ.
  - **Frame setup** (SBC and SBC XQ, developer mode only): blocks, subbands, bit allocation and, for SBC, the channel mode. Auto keeps 16 blocks, 8 subbands, Loudness and Joint stereo. A value the device does not take falls back to Auto when connecting. With a Sony device a user tested, 12 blocks garbles the sound and may disconnect (the encoder's output decodes correctly; the other block counts play). The choices stay saved but are not used once developer mode is off. AAC's and LHDC V5's bit rates stay within the maximum the device lists

The defaults give the best sound; most devices need nothing but **Auto**. An LE Audio device (experimental) has one card, LC3, and **Low latency for gaming audio devices (experimental)** (see [LE Audio](#le-audio)).

Behind the scenes BlueCaster keeps one entry per device in `profiles.json` (its codec, each codec's settings and its codec order), named "*device* | *codec*" there; the window and the tray menu show the device's name. Earlier versions let you make several profiles for one device: the first start of a version with the Codecs screen keeps the one last used (otherwise the first) and folds the others into it, keeping their codec settings (quality, sample rate, bit depth, ABR, SBC setup) for their codecs. Their own names, output sources and packet sizes are not kept; `profiles.json` as it was is saved once as `profiles-before-codecs.json` in the settings folder. The 2.0.2 pre-releases' **Auto's order…** (one order for every device, in developer mode) is no longer used: each device has its own order now.

### Audio Devices

The headphones and speakers BlueCaster can stream to, each with its own settings, and below them **Add an audio device**: **Scan Devices** finds nearby devices with the USB adapter (30 seconds, listed as they answer; **Stop Scanning** ends it; put the headphones into pairing mode; while a device is connected, it asks to disconnect it first). **Add** saves a device found and connects it. Devices BlueCaster paired with before are tagged **Paired**. If the scan does not find them, enter the address under **Manual:** and click **Add**; typed or pasted addresses are formatted as you go (`aa-bb-cc-dd-ee-ff`, `AABBCCDDEEFF` and `AA BB CC DD EE FF` all become `AA:BB:CC:DD:EE:FF`). Headphones see the adapter under your computer's name with BlueCaster beside it, e.g. *Taro-PC (BlueCaster)*, so it differs from Windows' own Bluetooth. A device Windows' own Bluetooth has paired too asks first (**Paired in Windows**: remove it there, or **Add anyway**). A saved device can be renamed or removed (removing it also removes its settings and pairing, so it cannot connect again until it is paired again), and has:

- **Output Source**: System Loopback (a new device's default), Virtual Device or Application (see [Capture Modes](#capture-modes)). Beside it, the format Windows mixes the captured device in (sample rate and bit depth, read only), in amber when the device streams (or, not connected, its codec settings ask for) another one, with a line below giving both formats (it still streams, converted); below it, what the source does
- **Set as Windows default output while connected**, with a virtual device

**All audio devices**, above the list, has the switches that apply to every device: **Windows Volume Sync** (see [Volume](#volume)), **Control playback with the audio device's buttons** and **Reconnect automatically**. Each device's **Max media packet size** is on the **Developer** screen (see [Max Media Packet Size](#max-packet)).

### Diagnostics

- **Link Quality** while streaming: actual bitrate, packets, packets dropped before sending, audio dropped at capture, send queue, the delay the device reports (see [Latency](#latency)), radio time used, signal (RSSI) and the AFH channels (see [Link Quality and AFH](#link-quality))
- **Radio**: the 79 Bluetooth channels on the 2.4 GHz band, which ones the link uses and which the adapter skips, with the Wi-Fi channels underneath, and a few tips for a cleaner link. In developer mode, **Scan** adds the 2.4 GHz Wi-Fi networks around (see [The Radio tab](#radio-tab))
- **Codec details** while streaming: the media payload, headers, codec frame and frames per packet, and what the device's endpoint lists beside what is in use: for SBC the bitpool, sample rates, channel modes, blocks, subbands and bit allocation; for AAC the object types, sample rates, channels, peak bit rate and VBR; for the other codecs the sample rates, bit depths and peak bit rate
- **Connection Flow**: the eleven stages of the latest connection attempt (start, Bluetooth adapter, ACL / AVDTP connect, capability discovery, codec selection, audio capture, stream configuration, encoder, stream start, audio sending, connection upkeep with disconnects / reconnects) as OK, Warning or Failed, with the time it took. **Copy Flow** copies it
- **Log**: live, filtered by level (all, info and above, warnings and errors, errors only), by text and to the current connection attempt (**Current attempt only**). **Copy Shown Log** copies what is shown. **Open log folder** opens the folder of the log files; unlike **Copy Diagnostics**, those files are not masked (they include Bluetooth addresses and your Windows user name)
- **Start HCI Capture** / **Stop HCI Capture** (see [Verifying the Media Stream](#verify-stream))
- **Copy Diagnostics**: version, OS, adapter, audio device (codec, its settings and order), the flow, warnings / errors and the recent log on the clipboard, ready to paste into a bug report. **Mask Bluetooth addresses** (on by default) replaces addresses with `<addr-1>`, `<addr-2>`... (the adapter's own with `<adapter>`) and device names with `<device-1>`, `<device-2>`..., and hides USB serial numbers, file paths above the app's own folder (`<path>\BlueCaster\logs\...`) and your Windows account (user name and SID). The same masking applies to the crash report and error texts copied for a bug report
- **Report a problem**:
  - **Verbose logging**: **Detailed (debug)** or **Everything (trace, 1 hour)** logs far more about Bluetooth, USB, audio capture and encoding. Debug stays on for 24 hours, also across restarts (for problems at start-up), then turns itself off. Turn it on, reproduce the problem, then save a diagnostic bundle
  - **Diagnostic bundle > Save…**: choose whether HCI captures go in (off unless you were asked for them: they keep the addresses and device names) and whether Bluetooth addresses are masked in the text files, then save one `.zip` (in `logs\bundles\` of the settings folder, the newest 3 kept): the diagnostic info, the log files of the core, the window and the driver helper, crash reports, system information (USB Bluetooth adapters and their drivers, Windows' paired devices, audio devices), settings and each device's codec settings (never the link keys), and the HCI flight recorder. The HCI captures keep the addresses (not the pairing keys). It is kept under GitHub's 25 MB attachment limit where possible (the newest logs first, up to 48 MB of text, the newest 5 recordings and 2 minidumps)
  - **Report on GitHub** (after saving): opens a new bug report with the version, Windows build, adapter, headphones, codec, capture mode and the diagnostic info (addresses masked) filled in, and shows the bundle in Explorer to drag into it

BlueCaster also records some things all the time, so that a problem that happened once can still be looked at:

- **HCI flight recorder**: the last few minutes of Bluetooth traffic are kept in memory (media payloads cut short, pairing keys removed) and saved to `logs\hci\` whenever a stream fails or a connection is lost (the newest 10 files are kept)
- **Crash reports**: if BlueCaster crashes, a `.dmp` minidump and a `.txt` file go to `logs\crash\` (the newest 5 per program)

Disk use stays bounded: a normal day's log is well under 1 MB and 7 days are kept; with verbose logging a day's file takes at most 100 MB of debug and trace lines (then only info and above go on until the next day); the flight recorder files are a few MB at most, 10 kept.

### Developer {#developer}

Shown while **Settings > Developer mode** is on; most users never need it. It is for testing devices and troubleshooting: some of its settings work only with some devices. Six tabs:

- **App settings**: **Use the Volume Mixer** (off by default): the **Volume Mixer** screen, the flyout's app list and the output mute (see [Capture Modes](#capture-modes)). Not recommended; VB-CABLE is the better way to keep the PC silent. **Include Alphas in Update Checks** (on by default in an alpha build) also offers alpha releases (see [Updates](#updates))
- **Codec settings**: switches for finding out why a device stays silent (RTP marker bit, logging the first media packets, AAC configuration and encoder options), each saved device's **Max media packet size** (see [Max Media Packet Size](#max-packet)). **Use each codec's settings when the codec is auto-selected**: with Auto-select on, a codec card's **…** opens without turning Auto-select off, and the codec Auto picks connects with those settings instead of Auto's. **Match the sample rate and bit depth to the PC's output**: a sample rate or bit depth on Auto follows the format of the output device in Windows' sound settings instead of 48 kHz: of the rates the device lists for the codec, the PC's own, else the nearest of the same family (44.1 or 48 kHz multiples), else the nearest above it; a 16-bit output streams 16-bit. **Ignore the device's peak bit rate**: every codec runs at the bit rate of the chosen quality even when the device lists a lower maximum (the bit rate of LHDC V5, AAC or Opus 05, the SBC bitpool), and the **Codecs** screen offers every quality and bitpool. A device may drop out or go silent above its maximum. **Send queue limit**: how long a media packet may wait in the send queue before it is dropped, 50 to 2000 ms (empty = 350 ms); it does not lower the delay on a good link (see [Latency](#latency)). **Encode at send time**: while the link stalls, audio waits unencoded instead of as packets already encoded, so with ABR on (SBC, SBC XQ, LDAC, LHDC V5 and aptX Adaptive) it goes out at the bit rate ABR lowers to meanwhile rather than the one in use when the stall began. It does not make a stall itself shorter: nothing goes out until the radio sends again, and only the catch-up afterwards takes less data. Audio that waits longer than the send queue limit is dropped either way; Diagnostics counts the waiting audio in the send queue wait. Everything is off by default and applies when the next stream starts; while one is on, a banner says so. **Turn all off** resets them
- **Vendors**: when BlueCaster speaks earbuds' own protocols (Huawei: LDAC, OPPO: Hi-Res; see [Codecs](#codecs)), each with **Own protocol**: **Auto** (earbuds whose name says they are that vendor's; the default), **Always** (any device) or **Never**. OPPO also has **Ask for Hi-Res on every connection (reconnects each time)**. Changes apply when the next stream starts; while one is changed, a banner says so, and **Turn all off** resets them. They are not used once developer mode is off
- **Logging**: how long log files are kept (default 7 days), the daily size limit (default 100 MB), verbose logging that stays on (no 24-hour or 1-hour limit, also from the core's start), the stream health line every 10 seconds, and **Add marker** (a numbered line in the log, to find a moment again). Turning developer mode off goes back to the defaults
- **Peer Receiver Test**: streams to the measuring receiver on another PC and compares what was sent with what arrived (see [Peer Receiver Test](#receiver))
- **Chip**: runs a chip script on an adapter with WinUSB: requests to the chip, typed in or opened from a file, for whoever adds support for a chip (see [Adding Chip Support](chip-support#chip-scripts)). Lines that may change the chip run only after **Run them**; Bluetooth stops for it and starts again when next used

### Adapters

- The detected USB Bluetooth adapters with their chip, address, USB ID, Bluetooth version, **Driver** and **Firmware**; the list updates by itself when an adapter is plugged in or out. The driver is **Original driver** (the one Windows or the manufacturer installed; hover for its name) or **WinUSB driver** (the one BlueCaster uses). A Realtek adapter shows its address, from its USB serial number, even before it was used; for the others the address and Bluetooth version show once Windows' Bluetooth or BlueCaster has used the adapter (hover over the line for why). An adapter BlueCaster cannot use (some MediaTek and Qualcomm) keeps its driver: its WinUSB and firmware buttons are disabled. Intel, MediaTek MT79xx, Qualcomm ROME / WCN6855 and Broadcom adapters are tagged **Experimental**. In developer mode WinUSB can be assigned to it anyway, and **Read chip info** on an adapter with WinUSB saves what its chip answers to a few read commands to `logs\probe\`, for whoever adds support for it (Bluetooth stops for it and starts again when next used); **Report on GitHub** then opens an issue with it filled in. While an adapter whose firmware BlueCaster loads itself (Intel, MediaTek, Qualcomm) starts, the screen shows the loading and whether it failed
- **Replace with WinUSB driver** / **Restore original driver** (see [Setup](setup#winusb)); Windows asks for administrator permission each time. Before restoring, the dialog names the saved devices Windows has paired too: BlueCaster's pairing gave them a new key, so they may need pairing again in Windows
- **Bluetooth Chip** for the firmware, with **Download Firmware**, **Open Download Page** and **Open Firmware Folder** (see [Firmware](setup#firmware))
- **Avoid Wi-Fi channels**: Auto, Off or Manual, with the Wi-Fi channels beside it: the ones Auto avoids now and nothing for Off (both read only), the ones you type for Manual (see [Link Quality and AFH](#link-quality))
- **Bluetooth stack**: stopped (it starts with the first scan or connection), starting, ready or error, and **Restart Bluetooth**

### Settings

| Section | Settings |
|:--------|:---------|
| Appearance | Language, theme (System / Light / Dark) |
| Startup | **Start with Windows** and **Start in the tray** (see [Start with Windows](#autostart)), **Minimize to System Tray** and **Open the tray window from** (see [Tray and Flyout](#tray)) |
| Notifications | **Notify when a device connects** (the device, its battery, the codec and what it runs with; on by default), **Notify when the battery is low**: **Below 20%** (the default), **Below 10%**, **Below 5%** or **Never**, once per connection (again only after the battery was back 5 points above) |
| Updates | **Check for Updates**, **Check for Updates on Startup**, **Include Pre-releases in Update Checks** (also offer beta releases), **Update now** (see [Updates](#updates)) |
| Advanced | **Developer mode** (adds the [Developer](#developer) screen, the SBC **Frame setup** in the codec settings, the **Services** tag on **Audio Devices** (see [Services and Pairing Keys](#shared-pairing)), **Read chip info** on **Adapters**, and more details to the log; no restart needed), **Settings folder** with **Open Settings Folder**, **Run setup** |

**Quit BlueCaster** is at the bottom. The version, the licenses and **Third-party licenses** are on the **About** screen.

### First-Run Setup

The first start (no audio device yet, or no adapter with WinUSB) opens the **Setup Wizard**: choose the adapter, driver and firmware, find your audio devices (Connect saves the device with the best settings for it and connects; **Import pairings from Windows** shows here too, see [Services and Pairing Keys](#shared-pairing)), then **General Settings**: theme, **Start with Windows** and **Start in the tray** (a first setup turns both on), notifications and **Check for Updates on Startup**. See [Setup](setup#first-run). **Settings > Run setup** opens it again.

## Tray and Flyout {#tray}

BlueCaster keeps running in the system tray, with or without the window:

- **Left-click** the tray icon for the flyout: the device and its state, **Connect** / **Disconnect**, the volume, the device switcher, the codec (a list to choose it, as on Home), the link quality and **Open app**. It is just as tall as what it shows. Esc or a click elsewhere hides it. If an error needs a screen of the main window (Codecs, Audio Devices, the adapter), its button opens the window there
- **Settings > Open the tray window from** (**Tray icon** by default): on Windows 11, with **Quick Settings (the volume button)**, the tray icon opens the main window and the flyout opens with Quick Settings instead, on top of the panels there, and closes with them. It works there without closing them. If the screen is too short, it opens to their left. With **Nowhere**, no flyout opens and the tray icon opens the main window.
- **Right-click** for the menu: **Open BlueCaster**, the first 10 devices by name (an LE Audio entry tagged "(LE Audio)"; the one streaming checked; click one to connect it), **Disconnect** and **Exit**. The menu opens at once, without starting the window
- The tooltip says whether BlueCaster is idle or connected

With **Settings > Minimize to System Tray** on (the default), minimizing or closing the window only hides it in the tray, and streaming goes on. With it off, the window minimizes to the taskbar, and closing it quits BlueCaster. **Exit** in the tray menu or **Settings > Quit BlueCaster** stops streaming, releases the adapter and closes BlueCaster completely.

## Start with Windows {#autostart}

**Settings > Start with Windows** starts BlueCaster when you sign in. With **Start in the tray** on, it starts in the background: only the tray icon appears, and the window opens from it. With it off (the default), the main window opens. It is the value `BlueCaster` under `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run`, with the data `"<folder>\bluecaster-core.exe"`, followed by `--background` when **Start in the tray** is on. To turn it off by hand, delete that value. Installing v2 over v1 turns **Start in the tray** on, as v1 always started in the tray.

## CLI Mode

`bluecaster-core.exe --cli` streams from a console, with the same core as the window. Quit BlueCaster first (tray > **Exit**): only one program can use the adapter at a time. The CLI does not read the saved devices' settings; everything is given as options.

### Basic

```bash
# Auto-select best codec
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF
```

Ctrl+C stops streaming. The CLI prints the state, the codec and bitrate, and at the end the number of packets sent and dropped.

### Codec Selection

```bash
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac      # LDAC
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxhd    # aptX HD
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxll    # aptX Low Latency
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptx      # aptX
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aac       # AAC
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c sbc       # SBC
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c lhdcv5    # LHDC V5
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c opus      # Opus (experimental; 48 kHz, 256 kbps)
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c opus05    # PipeWire's Opus 05 (experimental; to PipeWire sinks)
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxad    # aptX Adaptive (48 kHz; --rate 96000 for 96 kHz)
```

Auto-select priority: LHDC V5 > LDAC > aptX Adaptive > Opus (experimental) > aptX HD > aptX > aptX LL > AAC > SBC XQ > SBC (FastStream and Opus 05, both experimental, only when chosen)

If the requested codec is not offered by the headphones, the CLI stops with an error that says why; use `-c auto` (the default) to take the best codec they offer, or `--fallback` to take the best codec they offer instead of the requested one, as the app does -- see [aptX Family and aptX Adaptive](#aptx-compatibility). `--no-experimental` never uses the experimental codecs (aptX HD, aptX LL, Opus, Opus 05, FastStream); the app always uses them.

### Quality

```bash
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q hq   # High, 990 kbps (default)
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q sq   # Medium, 660 kbps
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q mq   # Low, 330 kbps

# LHDC V5: hq 1000 / mhq 900 / sq 500 / mq 400 kbps
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c lhdcv5 -q sq   # Medium, 500 kbps

# aptX Adaptive: hq 427 / ll 265 kbps
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxad -q ll   # Low Latency
```

The other codecs have no quality choice: they run as `hq`, the highest bit rate the headphones take (aptX, aptX HD, aptX LL and FastStream: fixed by the codec). A level a codec does not have (`mhq` for any codec but LHDC V5, `sq` / `mq` for aptX Adaptive, `ll` for any codec but aptX Adaptive) runs as `hq`.

### ABR (Adaptive Bit Rate) {#abr}

```bash
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -a
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c lhdcv5 -a
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxad -a
```

ABR automatically lowers the bitrate when the Bluetooth connection is unstable, and raises it when conditions improve. LDAC moves between its quality modes; LHDC V5 follows Android's policy and stays between 160 and 400 kbps; aptX Adaptive steps between its levels in High (see [aptX Adaptive](#aptx-adaptive)); SBC and SBC XQ lower the bitpool by 2 and raise it by 1 each second. With SBC, SBC XQ and aptX Adaptive, a bitpool or level the connection backed up at is tried again only after 10 s, and twice as long each time it backs up there again (up to 2 minutes), so a device that copes one step lower settles there; a minute without trouble at it makes it a first time again. In the window, ABR is in a codec's settings (**...** on its card on **Codecs**).

### Device Discovery

```bash
# Nearby devices, found with the USB adapter (10 seconds; --scan 20 for longer)
bluecaster-core.exe --cli --scan
```

Put the headphones into pairing mode first.

### All Options {#all-options}

| Option | Description |
|:-------|:------------|
| `-d <addr>` | Bluetooth address of the headphones / receiver (`AA:BB:CC:DD:EE:FF`; default: the first audio device paired with Windows) |
| `--device-name <name>` | Its name, with `-d`, as a saved device's (Huawei earbuds are told by it) |
| `-l` | List the Bluetooth devices paired with Windows and exit |
| `-u <adapter>` | The USB adapter to use when several have WinUSB: instance ID, device path or `VID:PID` (e.g. `0bda:8771`) |
| `-c <codec>` | `ldac`, `aptxhd`, `aptxll`, `aptx`, `aptxad`, `aac`, `sbcxq`, `sbc`, `lhdcv5`, `opus`, `opus05`, `faststream`, `auto` (default: `auto`) |
| `-q <mode>` | Quality: `hq` (High, default); LDAC and LHDC V5 also `sq` (Medium) and `mq` (Low), LHDC V5 also `mhq` (Medium-High); aptX Adaptive also `ll` (Low Latency). Other codecs run any level as `hq` |
| `-a` | ABR (LDAC, LHDC V5 and aptX Adaptive; SBC / SBC XQ: adaptive bitpool) |
| `--fallback` | A codec `-c` the headphones do not offer falls back to the best one they offer, as the app does, instead of an error |
| `--no-experimental` | Never use the experimental codecs (aptX HD, aptX LL, Opus, Opus 05, FastStream) |
| `-m <mode>` | Capture mode: `loopback` (default), `virtual`, `app` |
| `--audio-device <id>` | Render device for `-m virtual` |
| `--app <exe>` | Executable for `-m app` (e.g. `spotify.exe`) |
| `--rate <hz>` | Capture sample rate (default: 48 kHz, or another rate the device lists if it has no 48 kHz) |
| `--bits <16\|24>` | Bit depth (default: 24 where the codec carries it) |
| `--max-packet <bytes>` | Max media packet size, 679-1679, default 1023 (see [Max Media Packet Size](#max-packet)) |
| `--sbc-blocks <n>`, `--sbc-subbands <n>`, `--sbc-alloc <method>`, `--sbc-bitpool <n>` | SBC / SBC XQ frame setup: blocks 4/8/12/16 (default 16), subbands 4/8 (default 8), allocation `loudness` (default) or `snr`, bitpool (default: by quality) |
| `--sbc-mode <mode>` | SBC channel mode: `joint_stereo` (default), `stereo`, `mono` (dual channel is `-c sbcxq`) |
| `--afh <auto\|off\|6,11>` | AFH host channel classification (see [Link Quality and AFH](#link-quality)) |
| `--remote-sink <host[:port]\|auto>` | Show what the peer receiver gets; without `-d`, stream to that receiver (see [Peer Receiver Test](#receiver)) |
| `--scan [seconds]` | List nearby devices and exit (default 10 seconds) |
| `--probe` | Save what the adapter answers to read commands to `logs\probe\` and exit, for adapters BlueCaster cannot use yet (see [Adding Chip Support](chip-support)) |
| `--script <file>`, `--allow-writes` | Run a chip script on the adapter, save its report and exit; `--allow-writes` lets lines that may change the chip run (see [Adding Chip Support](chip-support#chip-scripts)) |
| `-h` | Show the help |

For development and testing there are also `--hci-tcp <host:port>` (a virtual controller over TCP instead of the WinUSB adapter), `--test-tone` (1 kHz left / 1.5 kHz right instead of captured audio), `--duration <s>` (stop after this many seconds), `--sessions <n>` (stream n times in a row, as Disconnect / Connect in the app), `--restart` (with `--sessions`: restart Bluetooth between them), `--restart-on-link` (start the session again as soon as its link is up, as a setting changed while connecting does), `--wait-off` (headphones turned off do not end the run) and `--hci-capture <file.pklg>` (HCI capture of the whole run); see [Building](building#emu). The LE Audio options are listed by `-h` (see [LE Audio](#le-audio)). `--services` lists the services the device gives in SDP once streaming.

Without `--cli`, `bluecaster-core.exe` is the resident program: it starts the window too, except with `--background` (tray icon only, used by **Start with Windows** when **Start in the tray** is on). `bluecaster-core.exe --quit` asks a running BlueCaster to stop streaming and exit (the installer uses it).

## Capture Modes {#capture-modes}

BlueCaster supports three audio capture modes. Each records a different point in the audio path:

| Mode | What it records |
|:-----|:----------------|
| System Loopback | What the Windows default output device plays: all apps mixed |
| Virtual Device | What apps play into the selected device (e.g., VB-CABLE) |
| Application | Only what one app (and its child processes) plays, after the app's own processing. Needs Windows 11 or Windows 10 version 2004 (build 19041) or later |

**Audio Devices** shows the same explanation under **Output Source**. A new device captures System Loopback, which works with nothing else installed; choose Virtual Device or Application for it there.

System Loopback copies the audio; it does not take it away from the default output device, so the audio also plays there (speakers, wired headphones, an HDMI TV, ...). To hear it only on the Bluetooth headphones, use a virtual device such as VB-CABLE.

In developer mode with **Developer > App settings > Use the Volume Mixer** on, **Mute Windows default output while connected** (on the **Volume Mixer** screen; off by default) mutes that device while streaming and unmutes it when streaming stops, for every capture source but the test tone. This does not affect what BlueCaster captures. Windows' mute key then mutes the headphones instead (silence is sent; press it again to play) and the device stays muted; the volume keys, which also unmute in Windows, leave it muted and the headphones playing. With an LE Audio device the mute key only keeps the device muted. A device you had muted stays muted, and if BlueCaster exits unexpectedly, the device stays muted: unmute it in Windows. With developer mode off, the mute stays off whatever this setting says.

With that switch on, the **Volume Mixer** screen and the flyout set each app's volume, with or without that mute. They work while the output is muted, when the Windows mixer may cut the sound. If the Windows default output is a virtual device, muting it can silence the stream too: turn the mute off.

In Virtual Device mode, **Set as Windows default output while connected** makes the selected device the Windows default output while streaming, so apps play to it without changing Windows settings. When streaming stops, BlueCaster switches back to the previous default device.

- The **Output Source** list offers, under Virtual Device, only the devices that BlueCaster detects as virtual audio devices. Without one, **Audio Devices** says why one is wanted and offers **Download VB-Cable**.
- The default communication device (used by calling apps) is not changed.
- Some apps keep playing on the previous device until they are restarted.
- If BlueCaster exits unexpectedly, the default device is not switched back. Change it in Windows Sound settings.

In developer mode, the **Output Source** list also has **Output Device**: each output device Windows has (speakers, a USB headset, a TV, ...). BlueCaster then records what that device plays, whichever device is the Windows default output, and never makes it the default; **Windows Volume Sync** syncs that device's volume. If it is missing when connecting (unplugged or disabled), BlueCaster sends System Loopback instead and the connection status says so; **Audio Devices** shows it as not connected. With developer mode off, such a device captures System Loopback: the choice stays saved and counts again once developer mode is back on.

### Application mode {#application-mode}

Application mode sends the audio of one app, for example a music player, a browser or an effects app. Choose the app in the **Output Source** list, under Application. It shows the apps that are playing audio right now; if yours is missing, start playback in it and click **Refresh** beside the list. The app's child processes are included, so a browser that plays audio from a separate process works too.

- The app does not have to be running. BlueCaster connects to the headphones anyway and starts sending once the app is up. When the app quits during streaming, BlueCaster stays connected and picks the app up again when it is restarted.
- The app keeps playing on its own output device as well. To hear it only on the headphones, mute that output device (its master volume): this does not affect what BlueCaster captures. Do not mute the app in the Windows Volume Mixer, though, or the headphones get silence too.
- The app's slider in the Windows Volume Mixer changes the level sent to the headphones (and on its output device). The master volume and the volume keys do not change the audio sent; with **Windows Volume Sync** on they change the headphones' volume instead (see [Volume](#volume)).
- Windows converts the audio to the sample rate that BlueCaster asks for, so the device format in Sound settings does not limit it.
- On Windows 10 before version 2004 (build 19041), Windows has no per-app capture. Connecting a device set to Application capture then shows an error that asks for another output source.

### Volume {#volume}

While streaming, the **Volume** slider on Home and in the tray flyout works when the headphones support AVRCP absolute volume (otherwise it stays disabled). It sets the volume on the headphones themselves, so the audio sent keeps its full resolution. The speaker beside it mutes (volume 0 on the headphones) and unmutes (back to the volume before). At volume 0, however it was set (the speaker, the slider, Windows or the headphones' buttons), BlueCaster sends silence, since many headphones still play faintly at their lowest volume. The slider also follows the volume buttons and touch controls on the headphones.

**Windows Volume Sync** (on **Audio Devices**, for every device; on by default) makes a Windows volume and the headphones' volume one volume while streaming. The Windows volume synced is that of the device BlueCaster captures: the default output device in System Loopback and Application mode, the selected device in Virtual Device mode. Off, the Windows volume and the headphones' volume are separate.

- The headphones' volume buttons and gestures, and the **Volume** slider, move the synced Windows volume, and its slider in Sound settings moves the headphones' volume.
- The volume keys and the Windows volume flyout always act on the default output device, so they change the headphones' volume only when the default output device is the one synced (always, except in Virtual Device mode without **Set as Windows default output while connected**).
- When streaming starts, the Windows volume takes over the headphones' volume; when streaming stops, the device gets back the volume it had before.
- The audio is captured before the Windows volume is applied, so it is still lowered only once, on the headphones. If the synced volume also changes the captured audio, the volume applies twice: turn the sync off then (see [System-wide effects apps](#effects-apps)).
- Mute is not synced: muting the output device keeps the PC silent and does not mute the headphones.

Earlier versions let a device follow another output device's volume (in developer mode); that choice is no longer used.

BlueCaster saves each audio device's own volume and sets it again as soon as it connects, before any audio is sent, so one device's volume does not carry over to another.

### Headphone buttons {#headphone-buttons}

The play/pause, next and previous track controls on the headphones (buttons or touch gestures, e.g. a double tap on the WH-1000XM4) control playback on the PC while streaming: BlueCaster passes them to Windows as media keys, which go to the app shown in the Windows media controls (Spotify, a browser, a media player...). Stop is passed as well.

- Play and pause both act as the Windows Play/Pause key, so a tap always switches between playing and paused, even if the headphones' idea of the state is out of date.
- BlueCaster tells the headphones whether audio is playing (from whether there is audio to capture), so headphones that pause when taken off send the right command.
- Fast forward / rewind (holding a button) have no Windows media key and are ignored.
- Volume gestures change the headphones' own volume; with **Windows Volume Sync** the synced Windows volume follows (see [Volume](#volume)).
- Turn this off with **Audio Devices > Control playback with the audio device's buttons** (one switch for every device), e.g. if another program already reacts to the media keys twice.

This needs AVRCP on the headphones, which nearly all have. Without it, streaming works as usual, just without the buttons.

### System-wide effects apps {#effects-apps}

A system-wide effects app (an equalizer, a sound enhancer, a virtual surround app, ...) usually installs its own virtual output device. Other apps play into that device; the effects app processes the audio and plays the result to the output device you choose in the effects app. Which BlueCaster mode you choose decides whether the headphones get the effects:

| Setting | What reaches the headphones |
|:--------|:----------------------------|
| Virtual Device: the effects app's own virtual device | The audio **before** the effects: what apps send into the effects app |
| Application: the effects app | The audio **after** the effects: what the effects app plays (recommended) |
| Virtual Device: the effects app's output device (e.g., VB-CABLE set as its output) | The audio after the effects (for Windows versions without Application mode) |

In Application mode, the effects app's output device can be any device: BlueCaster takes what the app plays, not what the device plays, so the device is only somewhere for the audio to go. So that the PC does not play the audio at the same time, choose an output nobody listens to (e.g., a monitor's HDMI audio) or mute that device's master volume. A virtual device such as VB-CABLE also works as a silent place to send it, but it is not required. The device must be enabled and connected.

If the effects app's own virtual device is the Windows default output device, its volume also lowers what the effects app receives. With **Windows Volume Sync** on, a volume change then applies twice (on the effects app's input and on the headphones). Turn it off, and use the headphones' volume or the **Volume** slider.

#### Why BlueCaster has no virtual output device of its own {#no-virtual-device}

A Bluetooth headphones entry in the Windows output device list, which an effects app could select directly, would need a kernel-mode audio driver. BlueCaster does not include one:

- Writing and maintaining a kernel audio driver is a large job, and a bug in it can crash Windows (blue screen).
- Windows 10 and 11 load only drivers signed by Microsoft. Getting that signature needs an EV code signing certificate (several hundred US dollars a year) and a Microsoft Partner Center account, normally registered as a company.
- The only way around the signature is Windows test signing mode, which lowers system security and breaks some software (e.g., games with anti-cheat). BlueCaster will not rely on it.

Application mode, or a free virtual device such as VB-CABLE, covers the same use.

- Windows has no public API for changing the default device. BlueCaster uses the undocumented IPolicyConfig interface, as other audio switching tools do (Windows 10 and 11).

{: .warning }
System Loopback and Virtual Device use WASAPI shared mode: with the sample rate on Auto (Auto-select too), BlueCaster asks for 48 kHz, or another rate the headphones list if they have no 48 kHz (44.1 kHz first), and WASAPI resamples the device's **Default Format** to it. To use LDAC at 96 kHz, choose 96 kHz in the codec's settings; the **Default format** in **Sound settings > Device properties > Advanced** does not need to change.

## Codec Comparison

| Codec | Best For | Trade-off |
|:------|:---------|:----------|
| LHDC V5 | Highest bit rate and sample rate (up to 1000 kbps, 192 kHz) | Few devices; needs a stable connection |
| LDAC (HQ) | Highest audio quality on many Sony and Android-oriented devices | Higher latency, needs stable connection |
| LDAC (ABR) | Quality + stability | Bitrate fluctuates with connection |
| aptX Adaptive | High quality with a low-latency mode, on Qualcomm-based devices | Bit rate varies with the link |
| Opus 🧪 | Efficient, good quality at 256 kbps | Few sinks (Android's A2DP Opus, PipeWire) |
| aptX HD | High quality, wider device support | Fixed 576 kbps |
| aptX | Widest aptX device support | 16-bit, 352/384 kbps |
| aptX LL | Gaming, video (low latency) | Lower audio quality (16-bit) |
| AAC | Apple devices, good efficiency | Moderate quality |
| SBC XQ | Much better than SBC on any device whose SBC allows it | Needs dual channel and bitpool ≥ 29; ~385-552 kbps |
| SBC | Maximum compatibility | Lowest audio quality |

The **Codecs** screen shows the same comparison as three bars on each codec card.

## aptX Family and aptX Adaptive {#aptx-compatibility}

BlueCaster supports three aptX codecs encoded with [libopenaptx](https://github.com/pali/libopenaptx), and aptX Adaptive, encoded with the encoder of [AKS-Logic/aptx-adaptive-linux](https://github.com/AKS-Logic/aptx-adaptive-linux).

{: .warning }
**aptX HD and aptX Low Latency are experimental.** They follow the Android / PipeWire implementations and pass encode/decode round-trip tests, but have not yet been verified with real headphones.

{: .note }
**Only need classic aptX?** Windows 10 already supports classic aptX in its built-in Bluetooth stack (not aptX HD, aptX LL or aptX Adaptive), so BlueCaster is not required for it. BlueCaster is mainly useful for LDAC, aptX HD and aptX Low Latency.

| Codec | Vendor ID / Codec ID | RTP header | Status |
|:------|:---------------------|:-----------|:-------|
| aptX Adaptive | 0x000000D7 / 0x00AD | Yes | Supported (265-427 kbps, 48 / 96 kHz, 24-bit) |
| aptX HD | 0x000000D7 / 0x0024 | Yes | Supported (576 kbps, 24-bit input) |
| aptX | 0x0000004F / 0x0001 | No | Supported (352/384 kbps at 44.1/48 kHz) |
| aptX Low Latency | 0x0000000A or 0x000000D7 / 0x0002 | No | Supported (352 kbps) |

### aptX Adaptive {#aptx-adaptive}

{: .note }
**aptX Adaptive is confirmed with real headphones at 48 kHz:** a user streamed it to an EarFun Air Pro 4+ on 2.0.2-beta.6 with no drops or stutter, as stable as classic aptX ([#86](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/86)). It also passes the unit tests, upstream's bit-exact test suite and the end-to-end test against the virtual Bluetooth sink (`tools/emu`, which checks the RTP header, the packet sizes and the frame types; the audio is not decoded, as there is no open decoder). Upstream reports it working on Linux / PipeWire with Bose QuietComfort Ultra (gen 1) and B&W Px7 S2e at 48 kHz in High and Low Latency. 96 kHz (High, 646 kbps) is confirmed with real headphones too ([#86](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/86)): it is used when chosen in the codec's settings (CLI `--rate 96000`), if the headphones are an R2 sink that lists it (the settings offer it only then), never in Low Latency. Auto, Auto-select too, streams 48 kHz. [Reports welcome](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/11).

The encoder is the pure-Rust aptX Adaptive encoder of [AKS-Logic/aptx-adaptive-linux](https://github.com/AKS-Logic/aptx-adaptive-linux) (MIT). Upstream describes it as a clean-room reimplementation whose numeric tables were reconstructed from lawfully obtained shipped Qualcomm binaries and on-air captures; no Qualcomm code, SDK or binary is included (see [Licenses](licenses)).

- **Choosing it**: Auto picks it when the headphones offer neither LHDC V5 nor LDAC. Or choose **aptX Adaptive** on **Codecs**, or `-c aptxad`.
- **Format**: 48 kHz stereo, 24-bit; 96 kHz in High when chosen, to a device that lists it (aptX Adaptive R2); a 44.1 kHz request is captured at 48 kHz.
- **Headphones**: R1 sinks (no capability extension, e.g. B&W Px7 S2e) and R2 sinks (R2 bitstream).

| Quality (**...** / `-q`) | R1 sinks | R2 sinks | Packets |
|:-------------------------|:---------|:---------|:--------|
| High (`hq`) | 427 kbps | 427 kbps | 12 ms, 656 bytes |
| Low Latency (`ll`) | 265 kbps | 270 kbps | 10 ms, 348 bytes |

`ll` is used only by aptX Adaptive; other codecs treat it as High. The Standard and Mobile levels (R1 364 / 318 kbps, R2 270 / 204 kbps) are no longer offered as qualities: a device saved with one, and `-q sq` / `-q mq`, run High; ABR still steps through them.

**ABR** (codec settings **ABR**, CLI `-a`): in High, the bit rate steps down one level (Standard, then Mobile, not below) while the send queue holds 3 or more packets, at most every ~0.5 s, and back up one level (up to High) after ~3 s with at most one packet queued. Low Latency has no ABR.

### There is no "fallback" from aptX Adaptive to aptX

When BlueCaster connects, the headphones advertise a **list of codecs** (AVDTP stream endpoints). BlueCaster chooses one codec from that list. aptX Adaptive and classic aptX are separate entries in it, so nothing "falls back" from one to the other inside the codec:

- **The headphones also list classic aptX** -- Many aptX Adaptive headphones do, because Qualcomm markets aptX Adaptive as backward compatible with aptX / aptX HD. Auto picks aptX Adaptive (above aptX HD and aptX); classic aptX is used when you choose it, as a separate codec in the headphones' list, not a reduced mode of aptX Adaptive.
- **The headphones list only aptX Adaptive** (no classic aptX, aptX HD or aptX LL) -- choosing aptX falls back to the best codec they offer (in the device's order on the **Codecs** screen), usually aptX Adaptive itself.

Auto-select priority: LHDC V5 > LDAC > aptX Adaptive > Opus (experimental) > aptX HD > aptX > aptX LL > AAC > SBC XQ > SBC (FastStream and Opus 05, both experimental, only when chosen)

### Explicitly Selected Codec Not Offered {#codec-fallback}

When you request a specific codec that the headphones do not list:

| Where | Behavior |
|:------|:---------|
| App (**Codecs**, Auto off) | Connects with the first codec in the device's order (Auto's priority order unless you dragged the cards) that the headphones offer, as Auto would (Opus chosen, headphones with aptX Adaptive, aptX, AAC and SBC: aptX Adaptive, not aptX below Opus; with SBC XQ dragged first: SBC XQ), and the codec badge says which codec is used instead and why for a few seconds. This is for a codec chosen while the headphones offered it: the **Codecs** screen shows the one used as chosen. A card chosen while they did not offer it (at their last connection) is asked for anyway, with no fallback: if they refuse it, the connection stops with an error. |
| CLI (`-c <codec>`) | Stops with an error ("Device does not support ..."); if you requested aptX, aptX HD or aptX LL and the headphones offer only aptX Adaptive, the message says so. Add `--fallback` to fall back as the app does, or use `-c auto`. |

Earlier versions had the option **Use another supported codec if this one is not available** instead; the fallback is now always on. Some 2.0.2 pre-releases took the next codec below the chosen one rather than the best one.

### Codec Missing While the Headphones Are Connected to Another Device {#sep-in-use}

Headphones offer each codec on a stream endpoint (SEP). On a dual (multipoint) connection, an endpoint that streams to the phone or PC is reported as **in use**, and its codec cannot be used until that stream stops. Some headphones also offer high-resolution codecs (LDAC, L2HC, ...) only while a single device is connected. BlueCaster then logs:

```
remote SEP 6: sink audio, IN USE (codec not queried)
remote SEPs in use by another connection: 1 of 6 audio sink SEP(s); ...
capability discovery complete (LDAC=false ... , 1 of 6 audio sink SEP(s) in use)
```

and the error says that endpoints are in use by another device. Disconnect the headphones from the phone / PC (or turn dual connection off in the headphones' app) and connect again. A PC whose built-in Bluetooth is still paired with the headphones counts as another device, too.

Sony headphones such as the WH-1000XM4 do not offer LDAC while **Connect to 2 devices simultaneously** (multipoint) is on: turn it off in the Sony Sound Connect app, then connect again. Some models, such as the WH-1000XM6, support LDAC together with multipoint.

The headphones' own app (battery, noise control, EQ) needs a link of its own to the headphones, from the phone or from this PC's built-in Bluetooth. While it is connected, the headphones are on a dual connection, and codec changes in BlueCaster can take longer or fail. Disconnect the phone, or close the app, before changing codecs. [EncoDeck](works-with) controls the headphones over BlueCaster's own link instead, so it adds no second connection.

### Sample Rate

aptX, aptX HD and aptX LL support only 44.1 kHz and 48 kHz (aptX Adaptive 48 kHz; 96 kHz when chosen, if the headphones list it). BlueCaster picks one of the rates the headphones advertise (on Auto 48 kHz, else 44.1 kHz) and lets WASAPI resample the capture. You do not need to change the Windows Sound settings for aptX.

### Checking What Your Headphones Offer

1. Connect to the headphones.
2. Open **Diagnostics** and type `remote` into the log filter.

The same lines are in the log files in the config folder (`%APPDATA%\BlueCaster\logs\<YYYY-MM-DD>\core.log`; not masked, so check them before attaching one to a report); in CLI mode they are printed to the console. The capability lines show exactly which codecs the headphones list:

```
remote supports aptX (SEID ..., [...])
remote supports aptX Adaptive (SEID ..., [...])
remote vendor codec 0x........./0x.... (SEID ...) - unsupported
capability discovery complete (LDAC=false aptX HD=false aptX LL=false aptX=true aptX Adaptive=true SBC=true SBC XQ=true AAC=true LHDC V5=false Opus=false Opus 05=false FastStream=false)
```

- `aptX=true` -- classic aptX is available (even if `aptX Adaptive=true` too); Auto uses it.
- `aptX=false`, `aptX HD=false`, `aptX LL=false` with `aptX Adaptive=true` -- the headphones offer only aptX Adaptive; choose aptX Adaptive, or Auto, AAC or SBC.
- `aptX Adaptive=true [not usable]` -- the headphones offer aptX Adaptive without 48 kHz stereo, which BlueCaster needs.
- `remote vendor codec ... - unsupported` -- another vendor codec that BlueCaster does not implement.

## Max Media Packet Size (per Device) {#max-packet}

**Max media packet size** of each device (**Developer** screen, one field per saved device; CLI: `--max-packet <bytes>`) sets the upper limit for the audio data in each Bluetooth packet, from 679 to 1679 bytes, so audio devices that need a different value can have their own. The Developer screen is shown in developer mode (**Settings > Developer mode**); most users never need it. The audio device's own limit (MTU) always applies too; BlueCaster uses the smaller of the two. The change applies from the next connection. It was a profile setting before (under **Advanced**): a device keeps the value of the profile kept for it (see [Codecs](#codecs)); profiles saved by v1 before 1.1 take the value that was set globally then.

**1023 (default) is recommended.** It fits one Bluetooth baseband packet (3-DH5). Larger values save only 1-2% overhead, and each lost packet then loses more audio. 679 fits one 2-DH5 packet and is the minimum LDAC accepts.

## Verifying the Media Stream (bluecaster_decode) {#verify-stream}

Most headphones cannot show which codec is in use. **Diagnostics > Start HCI Capture** / **Stop HCI Capture** records an HCI packet log (`logs\hci\hci_<date>_<time>.pklg` in the config folder) with the codec negotiation and every media packet sent. At LDAC 990 kbps it grows by roughly 0.5 GB per hour, so capture only as long as you need. In CLI mode, `--hci-capture <file.pklg>` records the whole run.

You can start a capture before connecting or while already streaming. If it starts during a connection, BlueCaster first writes the connection setup packets it remembered (L2CAP channel setup and AVDTP signaling, including the codec negotiation), so the file can still be analysed. Diagnostics shows the file path while it records and after it stops.

Open the capture in Wireshark, or check it with the `bluecaster_decode` developer tool (built from source, see [Building](building)):

```
bluecaster_decode "%APPDATA%\BlueCaster\logs\hci\hci_20260925_120000.pklg" -o C:\temp\check
```

The tool follows the AVDTP signaling (SET_CONFIGURATION / RECONFIGURE and whether the headphones accepted it), extracts the media packets BlueCaster sent and checks them against the negotiated configuration:

| Codec | What is checked | Output |
|:------|:----------------|:-------|
| SBC, SBC XQ, FastStream, AAC, aptX, aptX HD, aptX LL, Opus, Opus 05 | Full decode with the reference decoders; frame parameters vs configuration; media payload header frame count | `<prefix>.<n>.<codec>.wav` to listen to |
| LDAC | Frame headers only (sync word, sampling rate, channel mode, frame length), frame count, bitrate. No open-source LDAC decoder exists | Report only |
| LHDC V5 | Frame headers only (media payload header frame count, sequence numbers), bitrate. No open-source decoder exists | Report only |
| aptX Adaptive | Its own RTP header (sample count timestamp, latency mode) and the codec packet header only. No open-source decoder exists | Report only |

`bluecaster_decode --le-audio` does the same for an LE Audio capture (LC3 over CIS).

For every stream it also reports RTP sequence gaps, how fast the RTP timestamp advances, the bitrate, and how many ACL packets the Bluetooth controller reported as completed. The last line is `RESULT: OK` or `RESULT: PROBLEMS FOUND` (exit code 0 or 1).

{: .note }
This shows what BlueCaster sent and that it is valid for the negotiated codec. Since a sink can only decode the codec that was negotiated, correct audio from the headphones together with a clean report is strong evidence that the codec is really in use.

## Link Quality and AFH {#link-quality}

**Diagnostics > Link Quality** shows what BlueCaster sends while streaming: the actual bitrate, packets sent, packets dropped before sending (they waited too long in the send queue: the radio link could not keep up), audio dropped at capture (the PC fell behind), the send queue, the radio time used, and the **Signal vs. ideal**: the RSSI of the connection, relative to the controller's golden receive range (0 = fine, negative = too weak; not an absolute level; below the range, the most the actual level can be follows in dBm), and how many of the 79 Bluetooth channels the link hops on (AFH). Home and the tray flyout sum it up as Good, Fair or Poor, or PC busy when the link is fine but the PC dropped audio.

Windows itself can also skip audio before BlueCaster gets it, when its audio engine glitches (heavy load, or other audio software holding it up). **Copy Diagnostics** counts these gaps, and the audio they lost, apart from PC busy and from the radio: a dropout counted there is not the link's fault.

Bluetooth avoids busy channels by itself (AFH), but it only learns about Wi-Fi from errors. BlueCaster also tells its controller which channels to avoid (AFH host channel classification), for every connection:

| AFH | |
|:--|:--|
| Auto (default) | Only the 2.4 GHz Wi-Fi channel this PC is connected on (checked again every 30 seconds while connected). Nothing when the PC is not on 2.4 GHz Wi-Fi: disconnected, on 5 GHz, or wired. It reads only the channel number, which needs no location permission; a 6 GHz channel numbered 1 to 14 counts as 2.4 GHz |
| Off | Nothing: the controller decides alone |
| Manual, e.g. `6,11` | These Wi-Fi channels (1 to 14) |

Auto leaves the networks the PC only hears to the controller's own AFH: avoiding a neighbour's busy channel takes 22 MHz from the hopping and can make the link worse, not better. To avoid such a channel anyway, enter it as Manual.

At least 20 Bluetooth channels always stay in use. Set it on the **Adapters** screen, with `"afh"` in `settings.json` (config folder), or with `--afh` in CLI mode. The radio line shows which Wi-Fi channels are avoided. Whether it helps depends on the environment: measure with the peer receiver test.

### The Radio tab {#radio-tab}

**Diagnostics > Radio** draws the channel map the adapter reports for the link (Read AFH Channel Map), so you can see where it meets interference even on a PC without Wi-Fi. Red channels are the ones the adapter's own AFH left out; when most of one Wi-Fi channel's width is red, the tab names that channel. Channels BlueCaster asked the adapter to avoid (Auto or Manual above) have an outline.

In developer mode, **Scan** asks Windows for a fresh Wi-Fi scan and draws each 2.4 GHz network over the band, higher the louder it is. On Windows 11 24H2 and later the list counts as location data: Windows may ask for location access, and with **Let desktop apps access your location** off it gives no list. The scan does not change AFH: Auto still avoids only the PC's own channel (see above why).

### Blend (experimental) {#afh-blend}

In developer mode, **Avoid Wi-Fi channels** also offers **Blend**, with **Mild**, **Medium** or **Strong**. It avoids what Auto avoids, plus the Bluetooth channels where two signals point at interference: the Wi-Fi networks heard within 11 MHz (counting from -90 dBm, in full at -50 dBm) and how often the adapter itself left the channel out of its recent channel maps. A loud network the adapter keeps using without trouble is not enough on its own at Mild or Medium. The stricter the setting, the more readily a channel is avoided and the fewer always stay in use: 56 (Mild), 48 (Medium) or 40 (Strong) of 79.

While Blend runs, BlueCaster scans for Wi-Fi every 2 minutes, which needs location access as **Scan** does; without it, the adapter's map alone decides. Blend has not been measured on real adapters yet and may make the link worse than Auto: compare it with the [peer receiver test](#receiver) before keeping it. Without developer mode it runs as Auto. In `settings.json` and with `--afh` it is `blend-mild`, `blend-medium` or `blend-strong`.

## Latency {#latency}

What you hear lags behind the PC by the sum of a few delays. On the PC: the audio engine's 10 ms period, the encoder's 10 ms rounds and the codec's own frames (AAC's 1024 samples are 21 ms, LDAC packets about 5 ms), and the send queue, which stays near empty on a good link (**Diagnostics > Link Quality** shows it). The largest part is in the audio device: it buffers the stream before playing it, to ride out radio gaps, and buffers more for the bit rates of LDAC and LHDC V5. That part is the same whatever sends the audio, a phone included.

Where a device supports Delay Reporting, it tells BlueCaster that delay once the stream is set up and again when it changes; **Diagnostics > Link Quality** shows it as **Device delay (reported)** (and **Copy Diagnostics** includes it). A device that does not support it shows a dash. Android uses the same report to delay video so that it stays in sync with the sound; Windows has no such mechanism, so a video player keeps its picture ahead of the sound by that delay. Most players can delay the audio or the video by a chosen amount (an audio offset or audio delay setting): set it to the reported delay, or try values until the lip sync is right. Games cannot be helped that way: a lower bit rate (LDAC Medium or Low, a codec with smaller frames, aptX Adaptive's Low Latency, aptX LL) lets some devices buffer less.

Developer mode's **Send queue limit** (**Developer > Codec settings**) is a limit on how long a packet may wait before it is dropped, not a delay in itself: on a good link, no packet comes near it. It matters only when the radio stalls, where a shorter limit trades a gap in the sound for less delay afterwards.

## Peer Receiver Test (tools/linux_sink) {#receiver}

The link quality view shows what BlueCaster sends. To see what actually arrives, run `tools/linux_sink/bluecaster_sink.py` on a second PC with Linux. It drives that PC's ordinary Bluetooth adapter - the built-in one is fine - directly (Bumble over the HCI user channel) as an A2DP sink for every codec including LDAC, measures every media packet that arrives and serves the statistics over the network:

Copy the `tools/linux_sink` folder to the Linux PC (the folder alone is enough), then:

```bash
sh setup.sh      # once: decoder libraries, Python environment (Ubuntu / Debian, Python 3.11+)
sudo sh run.sh   # start the receiver (root: it takes the Bluetooth adapter)
```

On the Windows PC, quit BlueCaster (tray > **Exit**) and run:

```bash
bluecaster-core.exe --cli --remote-sink auto -c ldac --test-tone
```

`--remote-sink auto` finds the receiver on the local network, streams to its Bluetooth adapter unless `-d` names another device, and prints the receiver's statistics every 5 seconds and at the end: bitrate, lost and late packets (RTP sequence numbers), jitter and gaps, dropouts and skips of a modelled playout buffer, stream / decode errors and the RSSI at the receiver (relative to its golden receive range: 0 = fine, negative = too weak). Without `--test-tone` it sends the system audio. `--remote-sink <host[:port]>` names the receiver instead of searching for it.

The same test runs in the window: with **Settings > Developer mode** on, open **Developer > Peer Receiver Test**. Leave **Receiver** empty to find it on the local network (or enter its address), **Connect** connects to its statistics; choose the codec, its settings, the source (**Test tone** or **System audio**) and, if needed, the L2CAP MTU and playout buffer the receiver offers, then **Start**. It checks each step and shows what this PC sent beside what the receiver counted. Quit nothing first: the window uses the running BlueCaster.

Both sides count the packets of a stream from its first packet, so the packets sent and received can be compared: they should be equal. Bluetooth retransmits lost radio packets inside the controllers, and BlueCaster does not give up on a packet while the link is up, so a poor radio link shows as gaps, jitter and dropouts at the receiver rather than as lost packets; when the link cannot keep up, packets are dropped before sending on the sending side. Neither side sees how often the controllers retransmit; the adapter's failed contacts count only packets it gave up on, so they normally stay at 0, as BlueCaster does not give up on packets.

The receiver's adapter is not available to its own Bluetooth stack while the tool runs. Discovery uses a UDP broadcast on port 51201, so both PCs must be on the same network segment (otherwise enter the address). See `tools/linux_sink/README.md` for all options and the statistics protocol.

## Services and Pairing Keys {#shared-pairing}

In developer mode, a saved classic device has a **Services** tag on **Audio Devices**. It opens:

- the services the device lists, as a table of **Name** (the Bluetooth name, else the device's own, else a known vendor's name such as Qualcomm GAIA or Google Fast Pair; **Unknown service** for one that has none) and **GUID**. Asked while the device is connected; when it is not, the list from its last connection since BlueCaster started is shown
- below them, **Pairing keys**: **LinkKey**, the link key BlueCaster keeps for the device, and its type; **LongTermKey** (with its size) and **IdentityResolvingKey**, the device's LE keys, with its **Identity address** (**Random** when it is a random static one). The LE keys come from the link key: in developer mode BlueCaster asks the device for them right after pairing, or at its next connection when it was paired before (once per device each time Bluetooth starts); both sides derive the LongTermKey from the link key (cross-transport key derivation) and the device gives its IdentityResolvingKey. Only devices that support this give them, only on a connection BlueCaster leads (usually one it opened), and not with `"le_audio_experimental": false`. A key BlueCaster does not have shows **Not available**, never a made-up value. Keys are shown in the order Bluetooth sends them (least significant byte first); the **Reverse byte order** switch above them shows them all the other way round, as some systems keep them. **Copy key** copies one as shown, for example to give another system on the same PC the same pairing with the same adapter. Anyone with these keys can pose as the device: do not share them

**Import pairings from Windows**, on **Audio Devices** and in the setup's device step (an adapter on WinUSB; shown while Windows paired a device through it that BlueCaster has not saved), takes over the link keys Windows' Bluetooth kept for that adapter before it was given WinUSB: headphones paired with Windows through it then connect to BlueCaster without pairing again. Windows asks for administrator permission, since only the system may read those keys. A device BlueCaster already has a key for keeps its own (it was paired with BlueCaster later, so Windows' key is the old one). Keys Windows made with another adapter, such as the PC's built-in Bluetooth, do not work: a pairing belongs to the adapter's address. Windows does not keep a key's type, so an imported key gets type 4. LE devices are not imported. Headphones it imports are added to the saved devices with the names Windows knows, ready to connect.

## LE Audio (experimental) {#le-audio}

BlueCaster can also stream to LE Audio headphones and earbuds (LC3 over connected isochronous streams, BAP unicast), with headset volume (VCP), media buttons (MCP), both earbuds of a pair (CSIP) and a low-latency mode for gaming headsets (GMAP). This is **experimental**, chosen per device: it is only used for a device's LE Audio entry. It has only been tested against a Bluetooth emulator, **not yet with real LE Audio headphones**, so it may not work with yours; use classic A2DP for everyday listening.

What it needs:

- A USB Bluetooth adapter with **Bluetooth 5.2 or later that supports isochronous channels (ISO)**, with WinUSB assigned as for A2DP. An adapter without ISO support is refused with "adapter does not support LE Audio (no ISO buffers)".
- Headphones or earbuds that support LE Audio (LC3); some need LE Audio turned on in their own app first.

How to use it:

1. Put the headphones into pairing mode and scan for devices on **Audio Devices**. LE Audio devices are listed with an **LE Audio** tag, a pair of earbuds as **LE Audio set** (pick either one: BlueCaster finds the other). A headphone that also does A2DP is listed twice, once per kind.
2. On **Codecs**, choose the LC3 card of the LE Audio entry. There is no codec or quality choice (BlueCaster picks the best LC3 configuration the headphones accept), and only System Loopback and Virtual Device capture are available.
3. Under it, **Low latency for gaming audio devices (experimental)** is on by default: with headsets that support GMAP's game role it uses shorter LC3 frames; other headsets get the media configuration either way.

There is no LE Audio switch in Settings. If the LE side ever disturbs an adapter, `"le_audio_experimental": false` in `settings.json` (**Settings > Settings folder**) turns it off altogether.

In CLI mode: `--le-audio -d <address>` streams to an LE Audio device, `--le-device <address>` adds another device of the group (the second earbud), `--le-scan` lists the LE Audio devices nearby, and `--le-low-latency` / `--le-preset <name>` choose the configuration (`bluecaster-core.exe --cli -h` lists all LE options).

## Updates {#updates}

BlueCaster checks GitHub Releases at most once a day (**Settings > Check for Updates on Startup**; **Include Pre-releases in Update Checks** also offers beta releases). When a new version is available, a dialog shows a few lines on what is new (the full notes are a link away), with **Update now** (**Open download page** for a portable copy) and **Later**; **Later** leaves it to Settings until the next version. Settings says so too:

- **Installed copy**: **Update now** downloads the signed update. While an audio device is connected, it waits and installs when the device disconnects, so an update never cuts the audio. BlueCaster then closes, installs the new version and starts again by itself.
- **Portable copy** (or a build without update signing): **Open download page** opens the release page.

In developer mode, **Developer > App settings > Include Alphas in Update Checks** also offers alpha releases: builds between betas, for example with new chip support, that come out more often and are tested less. Outside developer mode they are not offered.

## Troubleshooting

Sound that drops out or stutters, a connection that fails, or a bit rate lower than expected: see **[Dropouts and Connection Problems](wireless)**, which sums up what the reports so far turned out to be and what to check first.

### The Window Says the Core Is Not Running

The window needs `bluecaster-core.exe` next to it and starts it by itself. "The BlueCaster core is not running" means it could not be started: reinstall BlueCaster (or extract the portable zip again). "The core is a different version" means the two programs come from different versions: install the same version of both (the latest release does this).

### Assigning or Restoring the Driver Fails

- "The administrator prompt was declined": the driver was not changed. Click the button again and allow the prompt.
- "Changing the driver failed": see **Diagnostics > Log** for the details.
- "The adapter did not return to its previous driver": if the adapter was set up with Zadig, Windows may pick Zadig's WinUSB driver package again. Remove that driver package in Device Manager (adapter > Uninstall device, with "Attempt to remove the driver for this device"), then unplug and replug the adapter.
- "The driver is back, but Windows runs one Bluetooth adapter at a time": another Bluetooth adapter (such as the PC's built-in one) is in use in Windows. To use this one in Windows instead, disable the other in Device Manager, then unplug and replug this one.
- Devices paired in BlueCaster may not connect in Windows once the adapter is back on its driver: remove them in Windows' settings and pair them again.

### The Bluetooth Adapter Does Not Respond {#adapter-no-answer}

If the adapter sends nothing back after it is opened (it does not answer HCI Reset), BlueCaster gives up after 5 seconds, opens it once more and, if it still does not answer, shows "The Bluetooth adapter does not respond". Unplug the adapter, plug it back in and try again (or **Restart Bluetooth**). Restarting BlueCaster is not needed: a failed start releases the adapter and every new start initializes it again.

To keep an adapter from getting stuck, BlueCaster resets its USB pipes when it opens it and sends HCI Reset when it closes it.

### Pairing Problems After Updating

BlueCaster pairs using SSP (Just Works) with **General Bonding**, and the link keys are saved so the headphones can reconnect without pairing again (v2 reads the link keys of v1). If you paired the headphones with an older BlueCaster build and they no longer connect, remove the BlueCaster pairing on the headphones (see the headphones' manual for clearing the pairing list) and pair again.

### Connection Timeouts

BlueCaster registers SDP records (A2DP Source, AVRCP Controller / Target) so the headphones can identify it as an audio source. If a connection attempt times out, the half-open connection is torn down, so you can simply retry without restarting BlueCaster. Make sure the headphones are powered on, in range, and not connected to another device.

A failed connection is tried again by itself (up to 3 attempts) when the link came up and then carried nothing, and when the headphones did not answer. If the headphones no longer know the pairing (reset, or paired with another PC), the stored key is dropped and pairing is done once more. The error then says what happened:

| Message | Meaning |
|:--|:--|
| The device did not answer | Off, out of range, or not connectable (a new device: pairing mode) |
| The connection came up but then carried nothing | Radio conditions: try again, or move the adapter / the device |
| The device answered but then ended the connection itself | It does not accept a connection now: turning off, charging, or connected to another device |
| Authentication failed, also after pairing again | Put the device into pairing mode, or remove its pairing list entry, and try again |

**The first connection** to a headphone needs pairing mode; later ones reconnect with the saved link key. If it fails although the headphones are in pairing mode, they may still be connected to a phone or another PC (also through LE Audio): disconnect them there, turn LE Audio off in the headphones' app if it has the option, put them into pairing mode and connect once more. The log then shows `signaling connection failed (0x66)` (refused for security: the headphones kept the key of an earlier pairing, for example with this PC's Windows Bluetooth) or `(0x67)` (refused for lack of resources: busy with another device).

A lost connection is reconnected by itself (up to 10 attempts). When the PC goes to sleep while streaming, BlueCaster reconnects after it resumes; these attempts are not counted.

Turn off **Audio Devices > Reconnect automatically** if you move the headphones between the PC and a phone by turning them off and on. A connection that is lost then stays disconnected and BlueCaster lets go of the Bluetooth adapter, so the headphones can connect to the phone. Connect them to the PC again with **Connect** on Home.
