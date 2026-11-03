---
title: Dropouts and Connection Problems
layout: default
nav_order: 3.4
english_only: true
---

# Dropouts and Connection Problems
{: .no_toc }

Sound that cuts out, stutters or stops, a connection that will not come up, or a bit rate lower than you expected. This page sums up what the reports in the [issues](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues) turned out to be, so you can check the common causes before you report, and tells you what to send if it is not one of them.

## Table of contents
{: .no_toc .text-delta }

1. TOC
{:toc}

## Check these first {#checklist}

Bluetooth shares the 2.4 GHz band with Wi-Fi, wireless mice and keyboards, headset dongles and every other Bluetooth radio in the room. A USB adapter has a small antenna, and earbuds have an even smaller one, with your head between them and the PC. Most "poor connection" reports so far came down to one of these; each takes a minute to try. The bug report form has a box for each, so please tick the ones you tried.

{: .note }
Do one at a time and listen for a few minutes in between: if the dropouts stop, you have found the cause.

### 1. Turn off the other Bluetooth adapters in this PC

Windows' own Bluetooth (the built-in adapter) keeps transmitting while A2DPWB streams through the USB adapter next to it, and the two share one band and often one antenna area. It is also where the headphones' app on the PC connects, which gives the headphones a second link (see the next item). Disable the built-in adapter in Device Manager while A2DPWB runs, or at least for the test. A2DPWB shows a notice on Home when it sees another active adapter.

Reports: a second active adapter was one of the two causes a long-time tester named for unstable connections ([#86](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/86)).

### 2. Give the headphones one connection only

Dual connection (multipoint) lets the headphones stay connected to a phone and the PC at once. Every connection costs air time, and some headphones also offer fewer codecs, refuse the hands-free link (so no battery level) or hold a codec endpoint for the other device while it is on. The headphones' companion app counts too: it keeps its own link open, on the phone or on this PC's Bluetooth, even when you only use it for the battery or the equalizer.

For the test: turn dual connection off in the headphones' app, turn Bluetooth off on the phone, close the companion app on the phone and on the PC, then connect from A2DPWB again.

Reports: dual connection and a running device-manager app made connecting and codec switching unstable, and switching became "way smoother" once the manager was disconnected ([#86](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/86)); a first connection was refused while the headphones were still connected to a phone with LE Audio on ([#81](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/81)); earbuds offered A2DPWB fewer codecs or a lower bit rate while a phone or their app was involved ([#30](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/30), [#208](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/208), [#237](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/237)). Sony's LDAC is off while the headphones' multipoint is on.

### 3. Move the adapter

- Use a **USB 2.0 port**, or a short USB extension cable. USB 3 ports and cables radiate noise in the 2.4 GHz band, right where the adapter sits.
- Keep it **away from other wireless receivers**: gaming mouse and keyboard dongles (Logitech LIGHTSPEED, for example), wireless headset dongles, Wi-Fi antennas. A receiver in the neighbouring port is a common cause of exactly this pattern.
- Keep it **in sight of the headphones**. A PC under the desk, with you and the desk in between, costs a lot of signal. A USB extension cable up to the desk helps more than anything else.

Reports: a LIGHTSPEED receiver next to the adapter and no way to avoid Wi-Fi channels made a UB500 carry only about 750 kbps, with the adapter marking up to 46 of the 79 channels as bad ([#252](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/252)); earbuds sitting a metre from the adapter still showed a weak signal, which was enough for a blackout every few minutes near the bit rate limit ([#279](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/279)).

### 4. Keep 2.4 GHz Wi-Fi away from the link

Put this PC, and if you can the phone and the other devices near it, on 5 or 6 GHz Wi-Fi. If 2.4 GHz has to stay on, A2DPWB tells the adapter to avoid the channel this PC is connected on (AFH, Auto). It cannot see the Wi-Fi when the PC has no Wi-Fi adapter, is wired, or is on another band than the router's 2.4 GHz network: then find the router's channel (its admin page, or a Wi-Fi analyzer app on the phone), fix it there if it hops, and enter it under **Adapter > Avoid Wi-Fi channels > Manual**, for example `6` or `11`. See [Link Quality and AFH](usage#link-quality).

Reports: LDAC 990 kbps lost about 1,700 packets in 5 minutes on a busy 2.4 GHz network, 33 in 15 minutes once the router was fixed on channel 7 and AFH set to Manual on it ([#30](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/30)).

### 5. Give the link headroom

A Bluetooth link carries a little over 1 Mbps on a perfect day; LDAC 990 kbps, LHDC V5 900 and 1000 kbps and SBC XQ at a high bitpool leave almost nothing for retransmissions. True-wireless earbuds relay the audio from one bud to the other, which costs air time as well, so 900 kbps and above is hard for them even on a clean link.

- Try **ABR** (adaptive bit rate; LDAC, LHDC V5, aptX Adaptive, adaptive bitpool for SBC), or one step lower: LDAC 660, LHDC V5 500, SBC XQ bitpool 29 to 36.
- Check whether the dropouts stay with **SBC or AAC**. If they do, it is not the bit rate.
- As a check on the headphones: pair them with a phone, set the same codec and bit rate there and listen in the same spot. If they drop out there too, the limit is on their side.

Reports: anything above 500 kbps cut out on a link that carried 750 kbps, while ABR had zero drops ([#252](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/252)); SBC XQ above bitpool 36 went silent on budget earbuds whose SBC capability allowed 53 ([#279](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/279)); LHDC V5 dropped at random where LDAC was steady on the same earbuds ([#11](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/11#issuecomment-6563908711)). A clean link does carry it: a laptop's built-in RTL8852BU streamed LDAC 990 kbps for 5 minutes with 0 packets lost ([#81](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/81)).

### 6. Update A2DPWB

Several stutters that sounded like radio were the app. 2.0.1 sent audio faster than real time when VB-Cable delivered it late, which the headphones hear as one second of sound and one of silence ([#279](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/279)); a stream after Disconnect > Connect stuttered until the app was restarted ([#84](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/84)); a stale Bluetooth stack refused to connect after sleep, a long session or many restarts ([#30](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/30), [#42](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/42), [#43](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/43)). All are fixed in the current release ([Updates](usage#updates)). If the problem is gone after updating, no report is needed.

## Reading Link Quality {#link-quality}

**Diagnostics > Link Quality** (and the graph on Home) tells the radio, the PC and the app apart. Take a screenshot while the problem happens and attach it to a report.

| What you see | What it means | Points to |
|:--|:--|:--|
| **Signal** well below 0 (about -25 dB or less) | The headphones' signal reaches the adapter far below the adapter's ideal range. It is not an absolute level: 0 is ideal, -30 is 30 dB below it, so -30 can still play but has no margin | Radio: distance, your head and body, placement of the adapter |
| **Send queue** full, **packets dropped before sending** growing, bitrate below the codec's | Packets waited too long for the radio; the link cannot carry this bit rate right now | Radio or interference, or too high a bit rate |
| **AFH channels in use** jumping, or stuck at 79 while packets are lost | The adapter marks channels as bad because of errors on them; at 79 it has given up telling them apart | Interference: Wi-Fi, other receivers, a second Bluetooth adapter |
| **Audio dropped at capture** growing, Home says **PC busy** | The PC did not deliver the audio in time | The PC: load, the capture device, a virtual cable |
| Everything flat and empty, but silence or stutter | The link is fine and nothing is dropped on the PC | The app or the headphones' decoder: report it with the diagnostic bundle |

The receiving side cannot be seen from the PC. To measure what actually arrives, the [peer receiver test](usage#receiver) runs a Linux PC as the headphones.

## What the headphones decide {#headphones}

Some "it only gives me 400 kbps" or "LDAC is missing" reports are not the link at all: the headphones tell A2DPWB which codecs and bit rates they accept, and A2DPWB follows that, as Android does. The log line `remote supports ...` shows what they offered.

- **Huawei FreeBuds** offer LDAC to a PC only after their AI Life app has set "prioritize sound quality" with multi-connect off; otherwise they list SBC and AAC only. Since 2.0.2 A2DPWB asks for that mode itself when LDAC is chosen with Auto-select off ([#30](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/30), [#208](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/208)).
- **LHDC V5 headphones** advertise a maximum bit rate. In their app's 96 kHz mode one pair listed 400 kbps; in its 192 kHz mode, 1000 kbps ([#50](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/50)). OPPO Enco Free4 list 400 kbps and 48 kHz to a PC whatever the app says; 2.0.2 asks them for Hi-Res over their own protocol, not yet confirmed on real earbuds ([#237](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/237)).
- **ABR** keeps LHDC V5 at 400 kbps or below, as on Android ([#50](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/50)).
- A codec the headphones **hold for another device** is missing while that device is connected ([Codec missing while connected to another device](usage#sep-in-use)).

How to see what they offer: [Checking What Your Headphones Offer](usage#codec-fallback).

## Connecting fails {#connecting}

The error on Home says what happened; the table in [Connection Timeouts](usage#connection-timeouts) explains each message. The radio cases:

- **The device did not answer** (Page Timeout): off, out of range, not in pairing mode for a first connection, or the adapter is still on its ROM firmware. A Realtek adapter without its firmware finds nothing in a scan and times out on every connection, or opens fine and then never streams; one report's firmware file turned out to be a saved web page ([#66](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/66), [#81](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/81)). The Adapter screen shows whether the firmware is loaded.
- **The connection came up but then carried nothing**: radio conditions, or the headphones are busy with another device. Try again, move the adapter.
- **Authentication failed**, or the log says `signaling connection failed (0x66)`: the headphones kept a key from an earlier pairing, usually this PC's own Windows Bluetooth. `(0x67)`: they are busy with another device, for example a phone with LE Audio on. Disconnect the other devices, turn LE Audio off in the headphones' app, put them into pairing mode and connect once; later connections use the saved key ([#81](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/81)).
- The headphones **cannot connect to the phone** after Disconnect: 2.0.2 lets the link go on Disconnect ([#270](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/270)); turn off **Reconnect automatically** if you move them between the PC and a phone by turning them off and on.

## The reports so far {#reports}

Every issue to date that concerned the wireless side, and what it turned out to be. "Environment" means the radio, the placement or another device; "Headphones" means a limit on their side; "App" means a bug in A2DPWB.

| Issue | Symptom | Found to be | Outcome |
|:--|:--|:--|:--|
| [#279](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/279) Tozo O2, RTL8761BU, Windows 10 | 1 s of sound, 1 s stuck | App: 2.0.1 sent VB-Cable audio faster than real time. Then: blackouts every few minutes at SBC XQ bitpool 35 with a weak signal (-30 dB) | Stutter fixed in 2.0.2; the blackouts are the link's headroom (environment) |
| [#252](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/252) realme Buds Pro 7, TP-Link UB500 | "Poor connection" at any distance, LHDC V5 above 500 kbps cuts out | Environment: the link carried about 750 kbps, the adapter marked up to 46 channels bad, a LIGHTSPEED receiver on the PC, no Wi-Fi adapter for AFH Auto, true-wireless earbuds | Open; 500 kbps or ABR streams cleanly |
| [#270](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/270) LHDC V5 headphones, RTL8761BU | Phone cannot connect after Disconnect; "connected, connected, disconnected" prompts on auto-reconnect | App: the link was kept after Disconnect (fixed 2.0.2-beta.17); the reconnect still tears down the stream the headphones opened. The radio was clean (signal -6 to -16 dB) | Partly fixed, reconnect prompts open |
| [#237](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/237) OPPO Enco Free4, Intel AX200 | LHDC V5 stays at 400 kbps, 1000 on the phone | Headphones: they list 400 kbps and 48 kHz to a PC; Hi-Res is set over OPPO's own protocol | Open; 2.0.2 asks for Hi-Res, unconfirmed |
| [#208](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/208) Huawei FreeBuds Neo, RTL8761B | LDAC not offered | Headphones: only SBC and AAC listed until AI Life sets sound-quality mode | Duplicate of #30 |
| [#86](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/86) EarFun Air Pro 4+, Moto Buds, RTL8761B | Reconnect errors after power-off, "Authentication failed" when the headphones connect first, unstable connection and codec switching | App: device-initiated connections (fixed through 2.0.2-beta.13). Environment: dual connection, a second active adapter, the device-manager app, 2.4 GHz Wi-Fi, as the reporter found | Fixes released; a long "Connecting" after power-on and crackle on a codec switch open |
| [#84](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/84) JBL Tune 510BT, RTL8761BU | Stutter after Disconnect > Connect | App: the second stream's timing (capture splice, thread priority) | Fixed 2.0.2-beta.5 |
| [#81](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/81) Sony WH-1000XM6, built-in RTL8852BU | WinUSB error 50, then an adapter that opens but never streams; first connection refused | App: the chip was unknown, the WinUSB binding was not live until the device restarted, and ROM firmware serves HCI but cannot stream (fixed 2.0.2-beta.1 to beta.5, with the in-app firmware download). Headphones: still held Windows' link key and a phone's LE Audio link | Fixed; LDAC 990 kbps with 0 packets lost |
| [#66](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/66) RTL8761BU 0BDA:A729 | Scan finds nothing, every connection times out | Environment: the firmware file was a saved web page, the adapter ran on ROM code | Resolved; the log now says why an upload was skipped |
| [#43](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/43), [#42](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/42) JBL Tune 510BT, WH-1000XM4 | No reconnect after sleep or a long session until restart | App: a stale Bluetooth stack was reused | Fixed 1.0.17 |
| [#32](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/32) EDIFIER W820NB, CSR8510 | LDAC fails to start, SBC works | App: the encoder was given the packet size minus the RTP header | Fixed 1.0.17 |
| [#30](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/30) Huawei FreeBuds Pro 4 and Neo, ASUS BT400, RTL8761B | LDAC not offered; the adapter stopped answering after many restarts; later LDAC 990 kbps dropouts | Headphones: LDAC only after AI Life's sound-quality mode (multipoint was suspected, the log showed no endpoint in use). App: the adapter was not closed properly (fixed 1.0.17), and a codec cache hid LDAC after one discovery without it (2.0.2-beta.13). Environment: the dropouts were 2.4 GHz Wi-Fi, gone with the router fixed on channel 7 and AFH Manual | Closed |
| [#18](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/18) ASUS BT400 | Adapter not seen after Zadig | App: the Broadcom vendor class was skipped | Fixed 1.0.12 |
| [#11](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/11) ROSESELSA Ceramics Ultra, UGREEN dongles | LHDC V5 drops at random, LDAC steady | Headphones or link headroom | Report only |
| [#50](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/50) LHDC V5 headphones, RTL8761B | 400 kbps whatever the quality | Headphones: their app's 96 kHz mode lists 400 kbps; 192 kHz mode lists 1000 | Resolved on the headphones' side |

Not the radio, although they looked like it: silence with AAC on a JBL Tune 510BT (the AAC frame format, [#87](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/87), fixed 2.0.2-beta.5); Windows output left on the virtual cable after the headphones went off ([#132](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/132), fixed); a few seconds of audio on the speakers while a codec change reconnects ([#203](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/203), fixed); sound on wired headphones too, which is how System Loopback works ([#223](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/223)).

### What stands out

- **Stutter and dropouts split about evenly** between the app (timing bugs in 2.0.1 and the first 2.0.2 betas, all fixed) and the environment (2.4 GHz Wi-Fi, a second radio next to the adapter, a second connection of the headphones, a bit rate without headroom). The two measured interference cases both show the same picture: the sent bitrate falls to 550 to 800 kbps, the send queue fills, and the adapter's channel map goes to 79 of 79.
- **Connecting problems were mostly the app**, and nearly all on one path: the headphones opening the connection themselves after being turned on. The Bluetooth emulator cannot drive that path, so these fixes rest on users' logs.
- **"Codec missing" and "bit rate stuck" were the headphones** deciding what to offer a PC, usually steered by their companion app; once it was A2DPWB's own codec cache.
- **Adapters that do not start** were support gaps (a chip, a USB class, a firmware file, a driver binding), each fixed once with the log in hand.

## Reporting {#reporting}

If the checks above did not find it, open a [bug report](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/new?template=bug_report.yml) and tick what you tried. What settles a wireless report quickly:

1. **Diagnostics > Report a problem**, **Verbose logging** on **Detailed (debug)**, then reproduce the problem.
2. The **diagnostic bundle** (**Diagnostic bundle > Save…**, then **Report on GitHub**). It holds the stream health lines (send queue, dropped packets, signal, AFH channels over time) and the HCI capture of a failed connection, which show where the packets stopped.
3. A **screenshot of Link Quality** while it happens.
4. Whether it also happens with **SBC or AAC**, and at a **lower bit rate**.
5. Your answers to the form's Dual connection, Another Bluetooth adapter and Wi-Fi questions.
