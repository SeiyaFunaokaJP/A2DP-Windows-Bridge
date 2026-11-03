---
title: Dropouts and Connection Problems
layout: default
nav_order: 3.4
---

# Dropouts and Connection Problems
{: .no_toc }

Sound that cuts out, stutters or stops, a connection that will not come up, or a bit rate lower than you expected. This page sums up what such reports have turned out to be, so you can check the common causes before you report, and tells you what to send if it is not one of them.

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

A second active adapter is one of the two causes a long-time tester named for unstable connections and codec switching.

### 2. Give the headphones one connection only

Dual connection (multipoint) lets the headphones stay connected to a phone and the PC at once. Every connection costs air time, and some headphones also offer fewer codecs, refuse the hands-free link (so no battery level) or hold a codec endpoint for the other device while it is on. The headphones' companion app counts too: it keeps its own link open, on the phone or on this PC's Bluetooth, even when you only use it for the battery or the equalizer.

For the test: turn dual connection off in the headphones' app, turn Bluetooth off on the phone, close the companion app on the phone and on the PC, then connect from A2DPWB again.

Dual connection is the other cause that tester named; codec switching became much smoother once the companion app was disconnected. A first connection was refused while the headphones were still connected to a phone with LE Audio on, and several earbuds offered A2DPWB fewer codecs or a lower bit rate while a phone or their app was involved. Sony's LDAC is off while the headphones' multipoint is on.

### 3. Move the adapter

- Use a **USB 2.0 port**, or a short USB extension cable. USB 3 ports and cables radiate noise in the 2.4 GHz band, right where the adapter sits.
- Keep it **away from other wireless receivers**: gaming mouse and keyboard dongles, wireless headset dongles, Wi-Fi antennas. A receiver in the neighbouring port is a common cause of exactly this pattern.
- Keep it **in sight of the headphones**. A PC under the desk, with you and the desk in between, costs a lot of signal. A USB extension cable up to the desk helps more than anything else.

A gaming-mouse receiver next to the adapter, on a PC with no Wi-Fi adapter to steer AFH, left a link that carried about 750 kbps and an adapter that marked up to 46 of the 79 channels as bad. Earbuds a metre from the adapter still showed a weak signal, which was enough for a blackout every few minutes near the bit rate limit.

### 4. Keep 2.4 GHz Wi-Fi away from the link

Put this PC, and if you can the phone and the other devices near it, on 5 or 6 GHz Wi-Fi. If 2.4 GHz has to stay on, A2DPWB tells the adapter to avoid the channel this PC is connected on (AFH, Auto). It cannot see the Wi-Fi when the PC has no Wi-Fi adapter, is wired, or is on another band than the router's 2.4 GHz network: then find the router's channel (its admin page, or a Wi-Fi analyzer app on the phone), fix it there if it hops, and enter it under **Adapter > Avoid Wi-Fi channels > Manual**, for example `6` or `11`. See [Link Quality and AFH](usage#link-quality).

LDAC 990 kbps lost about 1,700 packets in 5 minutes on a busy 2.4 GHz network, and 33 in 15 minutes once the router was fixed on one channel and AFH set to Manual on it.

### 5. Give the link headroom

A Bluetooth link carries a little over 1 Mbps on a perfect day; LDAC 990 kbps, LHDC V5 900 and 1000 kbps and SBC XQ at a high bitpool leave almost nothing for retransmissions. True-wireless earbuds relay the audio from one bud to the other, which costs air time as well, so 900 kbps and above is hard for them even on a clean link.

- Try **ABR** (adaptive bit rate; LDAC, LHDC V5, aptX Adaptive, adaptive bitpool for SBC), or one step lower: LDAC 660, LHDC V5 500, SBC XQ bitpool 29 to 36.
- Check whether the dropouts stay with **SBC or AAC**. If they do, it is not the bit rate.
- As a check on the headphones: pair them with a phone, set the same codec and bit rate there and listen in the same spot. If they drop out there too, the limit is on their side.

On a link that carried 750 kbps, anything above 500 kbps cut out while ABR had zero drops. SBC XQ above bitpool 36 went silent on budget earbuds whose SBC capability allowed 53. On the same earbuds LHDC V5 dropped at random where LDAC was steady. A clean link does carry it: a laptop's built-in adapter streamed LDAC 990 kbps for 5 minutes with 0 packets lost.

### 6. Update A2DPWB

Several stutters that sounded like radio were the app. 2.0.1 sent audio faster than real time when a virtual cable delivered it late, which the headphones hear as one second of sound and one of silence; a stream after Disconnect > Connect stuttered until the app was restarted; a stale Bluetooth stack refused to connect after sleep, a long session or many restarts. All are fixed in the current release ([Updates](usage#updates)). If the problem is gone after updating, no report is needed.

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

- **Huawei FreeBuds** offer LDAC to a PC only after their AI Life app has set "prioritize sound quality" with multi-connect off; otherwise they list SBC and AAC only. Since 2.0.2 A2DPWB asks for that mode itself when LDAC is chosen with Auto-select off.
- **LHDC V5 headphones** advertise a maximum bit rate. In their app's 96 kHz mode one pair listed 400 kbps; in its 192 kHz mode, 1000 kbps. OPPO Enco Free4 list 400 kbps and 48 kHz to a PC whatever the app says; 2.0.2 asks them for Hi-Res over their own protocol, not yet confirmed on real earbuds.
- **ABR** keeps LHDC V5 at 400 kbps or below, as on Android.
- A codec the headphones **hold for another device** is missing while that device is connected ([Codec missing while connected to another device](usage#sep-in-use)).

How to see what they offer: [Checking What Your Headphones Offer](usage#codec-fallback).

## Connecting fails {#connecting}

The error on Home says what happened; the table in [Connection Timeouts](usage#connection-timeouts) explains each message. The radio cases:

- **The device did not answer** (Page Timeout): off, out of range, not in pairing mode for a first connection, or the adapter is still on its ROM firmware. A Realtek adapter without its firmware finds nothing in a scan and times out on every connection, or opens fine and then never streams; one firmware file turned out to be a saved web page. The Adapter screen shows whether the firmware is loaded.
- **The connection came up but then carried nothing**: radio conditions, or the headphones are busy with another device. Try again, move the adapter.
- **Authentication failed**, or the log says `signaling connection failed (0x66)`: the headphones kept a key from an earlier pairing, usually this PC's own Windows Bluetooth. `(0x67)`: they are busy with another device, for example a phone with LE Audio on. Disconnect the other devices, turn LE Audio off in the headphones' app, put them into pairing mode and connect once; later connections use the saved key.
- The headphones **cannot connect to the phone** after Disconnect: 2.0.2 lets the link go on Disconnect; turn off **Reconnect automatically** if you move them between the PC and a phone by turning them off and on.

## What the reports turned out to be {#reports}

Every report so far that concerned the wireless side, grouped by what it was in the end.

| Cause | Reports | Examples |
|:--|:--|:--|
| **Environment**: the radio, the placement, another device | About a third | 2.4 GHz Wi-Fi on the same channels, a gaming-mouse receiver next to the adapter, a second active Bluetooth adapter, dual connection and a running companion app, a bit rate without headroom on true-wireless earbuds, a weak signal from small earbuds, a firmware file that was a saved web page |
| **Headphones**: what they offer a PC | About a quarter | LDAC only after Huawei's sound-quality mode, LHDC V5 limited to 400 kbps by the headphones' own mode, Hi-Res set over OPPO's protocol, a codec held for the phone |
| **App**: a bug in A2DPWB, all fixed | Under half | Audio sent faster than real time with a virtual cable, stutter after Disconnect > Connect, a stale Bluetooth stack after sleep or many restarts, the link kept after Disconnect, headphones that connect first mistaken for a wrong key, a codec cache that hid LDAC, adapters not recognized (a chip, a USB class, a driver binding that was not live yet) |

What stands out:

- **Stutter and dropouts split about evenly** between the app (timing bugs in 2.0.1 and the first 2.0.2 betas) and the environment. The two interference cases that were measured show the same picture: the sent bitrate falls to 550 to 800 kbps, the send queue fills, and the adapter's channel map goes to 79 of 79.
- **Connecting problems were mostly the app**, and nearly all on one path: the headphones opening the connection themselves after being turned on. The Bluetooth emulator cannot drive that path, so these fixes rest on users' logs.
- **"Codec missing" and "bit rate stuck" were the headphones** deciding what to offer a PC, usually steered by their companion app.
- Some problems **looked like radio but were not**: silence with AAC on one model (the frame format), Windows output left on the virtual cable after the headphones went off, a few seconds of sound on the speakers while a codec change reconnects, sound on wired headphones too (how System Loopback works).

## Reporting {#reporting}

If the checks above did not find it, open a [bug report](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/new?template=bug_report.yml) and tick what you tried. What settles a wireless report quickly:

1. **Diagnostics > Report a problem**, **Verbose logging** on **Detailed (debug)**, then reproduce the problem.
2. The **diagnostic bundle** (**Diagnostic bundle > Save…**, then **Report on GitHub**). It holds the stream health lines (send queue, dropped packets, signal, AFH channels over time) and the HCI capture of a failed connection, which show where the packets stopped.
3. A **screenshot of Link Quality** while it happens.
4. Whether it also happens with **SBC or AAC**, and at a **lower bit rate**.
5. Your answers to the form's Dual connection, Another Bluetooth adapter and Wi-Fi questions.
