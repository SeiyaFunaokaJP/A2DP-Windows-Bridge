---
title: Chip bring-up and probing (developer note)
layout: default
nav_exclude: true
---

# Chip bring-up and probing - developer note

**Status: step 1 (common loader interface, Intel on it) is done; the rest is
design.**

A2DPWB drives the adapter itself, so every chip family that needs a firmware
download needs its own loader (Realtek in BTstack, Broadcom PatchRAM, Intel
since #261). MediaTek (MT7921 / MT7922) and Qualcomm (QCA61x4) have none and
are shown as unsupported. Whoever wants to add one needs to see what the chip
says, and the maintainers have no MediaTek or Qualcomm hardware.

This note proposes one framework for all of them:

1. a **chip probe** that any adapter on WinUSB can run, which only reads and
   records everything it sends and receives, for the diagnostic bundle;
2. a **common loader interface**, so a new vendor is a state machine plus
   tests, not changes across the stack;
3. **step files** (developer mode) to try a command sequence on a reporter's
   adapter without a new build.

The goal is that "how do I add chip X" has a written answer, and a reporter
with chip X can produce what a contributor needs with one button.

## What exists

| Piece | Where | Reused for |
|:--|:--|:--|
| Intel loader: sans-IO state machine (`start` / `on_event` / `on_timeout` -> `Action`s) | `bt-host/src/intel.rs` | the model of the common interface |
| Action runner | `HostApp::run_intel` in `bt-host/src/app.rs` | becomes the one runner for every loader |
| Raw mode: packets go to the app before `hci_init` | `Stack::raw_begin` / `raw_send` / `raw_end` (`btstack-sys`) | probe and loaders |
| Raw packets in the HCI capture | `log_packet` in `btstack-sys/src/callbacks.rs` | probe traffic in `.pklg` with no extra work |
| Flight recorder and diagnostic bundle | `bt-host/src/capture.rs`, `core/src/bundle.rs` | carry the probe report |
| Bootloader mode (events on bulk IN, no HCI Reset on close) | `hci-transport/src/usb.rs` | kept as an action |
| Vendor detection | `bt-host/src/chips.rs` (`ChipVendor`) | picks the loader and the probe steps |

## 1. Common loader interface

`bt-host/src/bringup.rs` (done): `Action`, `Outcome`, `LoadError` and the
`ChipLoader` trait (`vendor`, `start`, `on_event`, `on_timeout`,
`missing_file_hint`). `HostApp::start_loader` / `run_loader` in `app.rs` carry
out the actions for any loader; `intel::Loader` implements the trait and
`intel` re-exports the shared types, so its tests run as before (only
`Outcome::Loaded` now has a log `detail` instead of Intel's `ddc` flag).

Still to come with the probe and MediaTek:

```rust
    UsbControl(ControlRequest), // vendor / class control transfer (MediaTek registers)
    Note(String),               // a line for the probe report and the capture
    // and ChipLoader::on_control(Result<Vec<u8>, ControlError>)
```

- `UsbControl` needs one new method on the USB backend
  (`control_in` / `control_out` with `ControlType::Vendor`); H4 over TCP
  answers it with `ControlError::NotUsb`.
- Control transfers are not HCI packets: the runner writes each one to the
  capture as a PacketLogger log message, so the `.pklg` still shows the whole
  sequence in order.

## 2. Chip probe

A `Loader` that never writes to the chip, run in place of the normal start
when the user asks for it (developer mode), on any adapter with WinUSB,
including vendors marked unsupported.

Steps, each with its own timeout; a timeout or error is recorded and the probe
goes on:

1. Standard reads: HCI Reset, Read Local Version, Read BD_ADDR, Read Local
   Supported Commands / Features, Read Buffer Size, Read Local Name.
2. The vendor's read-only steps from a table in `bringup/probe_steps.rs`,
   e.g. Intel Read Version (0xFC05, 0xFF), Realtek ROM version, MediaTek
   chip ID and firmware version, Qualcomm ROM version. Each entry says why it
   is safe (read-only per vendor documentation or Linux behaviour).
3. USB descriptors: interfaces, endpoints, alternate settings.

Output: `probe-<time>.json` (steps, raw bytes both ways, timings, decoded
fields where a decoder exists) plus the `.pklg`, both in the diagnostic
bundle. Addresses are masked like the rest of the bundle when masking is on.

UI: a **Probe** button on the adapter row in developer mode. For an
unsupported adapter it also allows switching to WinUSB, with a note that
Windows Bluetooth on that adapter stops until the original driver is put back.

## 3. Step files (developer mode)

A JSON file in the config folder lists steps: send a command (hex), send a
control transfer, wait for an event matching a pattern, wait N ms, upload a
file in chunks with a given command header. The step runner is one more
`Loader`.

- Lets a contributor send a reporter a file instead of a build; the result
  comes back as a probe report.
- Writes are allowed here (loading a patch is a write), so the file is only
  read in developer mode, its steps are shown before it runs, and the log
  names the file and its hash.
- RAM patches are lost at power-off; flash or OTP writes (e.g. CSR
  persistent store) are what can harm an adapter. The runner refuses opcodes
  on a deny list per vendor.

Once a sequence works from a step file, it is written as a proper loader with
tests (step files are not a support path for users).

## Adding a vendor

1. Probe report from real hardware (issue template asks for it).
2. Loader in `bt-host/src/<vendor>.rs` implementing `ChipLoader`, plain data and
   std only, with tests that replay the probe's recorded bytes.
3. Entry in `chips.rs` (`ChipVendor`, VID/PID or Read Local Version company
   ID), firmware file names, docs/setup.md table.
4. No GPL code or close paraphrase: work from vendor documentation, captures
   and probe reports; name sources in the module docs.

## Order of work

| PR | Content | Verifiable here |
|:--|:--|:--|
| 1 (done) | `bringup` interface, Intel ported, runner generalised | yes: tests, AX211 |
| 2 | Probe (HCI steps), report in the bundle, developer-mode button | yes: Realtek, AX211, emulator |
| 3 | `UsbControl` action, USB descriptors in the probe, vendor read steps | partly: no MediaTek / Qualcomm here |
| 4 | Step files | yes: emulator, Realtek |
| 5 | MediaTek loader | needs a reporter with MT7921 / MT7922 |

## Open questions

- MediaTek modules often use OEM VIDs (AzureWave 0x13D3, Foxconn 0x0489) also
  used by Realtek modules: detection may need PID lists or the probe's Read
  Local Version company ID (MediaTek 0x0046) after WinUSB is on.
- Whether the probe should also run once automatically on an unsupported
  adapter that is already on WinUSB.
