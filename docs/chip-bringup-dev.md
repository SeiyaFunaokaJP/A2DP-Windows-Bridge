---
title: Chip bring-up and probing (developer note)
layout: default
nav_exclude: true
---

# Chip bring-up and probing - developer note

**Status: the generic foundation is done (steps 1 to 3). Loaders for MediaTek,
Qualcomm and other chips are left to contributors with the hardware: the
procedure is [Adding Chip Support](chip-support).**

A2DPWB drives the adapter itself, so every chip family that needs a firmware
download needs its own loader (Realtek in BTstack, Broadcom PatchRAM, Intel
since #261). MediaTek (MT7921 / MT7922) and Qualcomm (QCA61x4) have none and
are shown as unsupported. Whoever wants to add one needs to see what the chip
says, and the maintainers have no MediaTek or Qualcomm hardware. So A2DPWB
provides the means, not the vendor code:

1. a **common loader interface**, so a new vendor is a state machine plus
   tests, not changes across the stack (#282);
2. a **chip probe** that any adapter on WinUSB can run, which only reads and
   records everything it sends and receives (#287);
3. **USB control transfers** for loaders, the USB descriptors in the probe,
   and a **sample loader** with its tests to start from.

## What it builds on

| Piece | Where | Used for |
|:--|:--|:--|
| Raw mode: packets go to the app before `hci_init` | `Stack::raw_begin` / `raw_send` / `raw_end` (`btstack-sys`) | probe and loaders |
| Raw packets in the HCI capture | `log_packet` in `btstack-sys/src/callbacks.rs` | probe and loader traffic in `.pklg` |
| Flight recorder and diagnostic bundle | `bt-host/src/capture.rs`, `core/src/bundle.rs` | carry the probe reports |
| Bootloader mode (events on bulk IN, no HCI Reset on close) | `hci-transport/src/usb.rs` | `Action::Bootloader` |
| Vendor detection | `bt-host/src/chips.rs` (`ChipVendor`) | picks the loader |

## 1. Common loader interface

`bt-host/src/bringup.rs`: `Action`, `Outcome`, `LoadError` and the
`ChipLoader` trait (`vendor`, `start`, `on_event`, `on_control`,
`on_timeout`, `missing_file_hint`). `HostApp::start_loader` / `run_loader` in
`app.rs` carry out the actions for any loader; `intel::Loader` implements the
trait.

A loader has no I/O of its own. `Action::UsbControl` is the one action with
an immediate result: the runner does the transfer
(`HciBackend::control`, nusb on the USB transport; H4 refuses it) and hands
the result to `on_control` before the next action. Control transfers are not
HCI packets, so `Stack::raw_control` writes each one with its result to the
HCI capture as a log line, keeping the whole sequence in order there.

`bringup/sample.rs` is a complete loader for a made-up chip with tests
against a simulated controller. It is compiled for the tests only.

## 2. Chip probe

`bt-host/src/probe.rs`: a `ChipLoader` that never writes to the chip, run in
place of HCI init (`BtHost::probe`) on any adapter with WinUSB, including
vendors marked unsupported, and over TCP on the emulator's controller. No HCI
Reset: a bootloader may not take it.

Steps, each answered, timed out (1 s) or failed, then the next:

1. HCI: Read Local Version, Read BD_ADDR, Read Local Supported Commands /
   Features, Read Buffer Size, Read Local Name, LE Read Local Supported
   Features, LE Read Buffer Size. Events that answer no step (a bootloader's
   vendor events) are kept too.
2. USB (standard GET_DESCRIPTOR on the default pipe): the device descriptor,
   then the manufacturer and product strings it names.
3. The runner adds the descriptors Windows keeps (`HciBackend::usb_descriptors`):
   one line per interface setting and the raw configuration descriptors.

No vendor command is sent: which ones are harmless differs per chip, and
that is for whoever writes its loader to find out from captures (see the
guide). A vendor's read steps belong in its loader.

Output: `logs\probe\probe-<time>.json` with a decoded summary (company, HCI /
LMP version, address, name, buffers, USB IDs and strings) and every request
in hex as sent and received (HCI: command and event; USB: SETUP packet and
the bytes read); the newest 10 are kept. The diagnostic bundle takes the
reports only with the HCI captures, as they hold the adapter's address.

How to run it: developer mode, Adapters, **Read chip info** on an adapter
with WinUSB (developer mode also offers WinUSB for an unsupported adapter);
or `a2dpwb-core --cli -u <adapter> --probe`, or with `--hci-tcp` on the
emulator (tools/emu).

## Possible later: step files

A JSON file of steps (send a command, a control transfer, wait for an
event, upload a file in chunks) run by one more `ChipLoader`, so a
contributor could try a sequence on a reporter's adapter without a new
build. Not built: it would be the one way to send writes from outside the
code, so it needs a vendor deny list for flash / OTP commands and a
confirmation in developer mode. A loader written from a capture and tested
against the simulated controller has been enough so far.

## Open questions

- MediaTek modules often use OEM VIDs (AzureWave 0x13D3, Foxconn 0x0489) also
  used by Realtek modules: detection may need VID:PID pairs or the probe's
  Read Local Version company ID (MediaTek 0x0046) after WinUSB is on.
- Whether the probe should also run once automatically on an unsupported
  adapter that is already on WinUSB (for now it runs only when asked).
