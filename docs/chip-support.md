---
title: Adding Chip Support
layout: default
nav_order: 4.5
english_only: true
---

# Adding Chip Support

A2DP Windows Bridge drives the Bluetooth adapter itself (WinUSB), so it has to do what the vendor's Windows driver does before the adapter works: on many chips, load firmware into it at every start. Realtek, Intel, Broadcom and CSR adapters work today; MediaTek (MT7921, MT7922 and others) and Qualcomm (QCA61x4) do not, because nobody has written their loader yet.

This page is the procedure for adding one. You need an adapter with that chip; the maintainers do not have them. Even without writing code, you can help: steps 1 and 2 give the report a contributor needs.

{: .note }
The design behind this, and what is still open, is in the [developer note](chip-bringup-dev).

## Table of contents
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## What you need

- The adapter, plugged into a Windows 10 / 11 PC
- A2DP Windows Bridge 2.0.2 or later (the steps below only need the app)
- To write the loader: a build setup as in [Building](building), and [USBPcap](https://desowin.org/usbpcap/) with [Wireshark](https://www.wireshark.org/) to record what Windows' own driver does

## Step 1: Give the adapter to A2DP Windows Bridge

1. **Settings > Developer mode**: on
2. **Adapters**: on the adapter's card, **Replace with WinUSB driver** (Windows asks for administrator permission). For a chip A2DP Windows Bridge cannot use yet, this button is only enabled in developer mode
3. If the card says **Replug or restart Windows**, do that

While the adapter has WinUSB, Windows' own Bluetooth cannot use it. On a laptop whose only Bluetooth is that chip, keyboards, mice and headsets connected through Windows stop working until you put the driver back: **Restore original driver** on the same card.

## Step 2: Read what the chip says (chip probe)

On the adapter's card, **Firmware** part: **Read chip info**. The Bluetooth stack stops for a moment, the probe runs, and Explorer opens with the report, `probe-<date>_<time>.json` in `logs\probe\` of the settings folder (`%APPDATA%\A2DPWB`). The newest 10 are kept.

The probe only reads. It sends no HCI Reset and no vendor command:

| Request | What it tells |
|:--|:--|
| Read Local Version Information | HCI / LMP version, the manufacturer (company ID), LMP subversion: often the ROM or firmware version |
| Read BD_ADDR | The adapter's address (zero or a placeholder on some chips before their firmware runs) |
| Read Local Supported Commands / Features, Read Buffer Size | What the running code offers: a ROM often offers much less than the firmware |
| Read Local Name | Some ROMs name themselves |
| LE Read Local Supported Features, LE Read Buffer Size | The same for LE |
| USB device descriptor, manufacturer and product strings | Standard USB requests on the default pipe |

It also records the USB configuration Windows reports (interfaces, endpoints) and any event the chip sent on its own. A request without an answer is recorded too: a chip that answers nothing until its firmware is loaded is a finding in itself.

From the command line, with the adapter's VID:PID or instance ID (the adapter card shows them):

```bat
"%LOCALAPPDATA%\A2DP Windows Bridge\a2dpwb-core.exe" --cli -u 0e8d:7961 --probe
```

Quit A2DP Windows Bridge from the tray first, as both cannot use the adapter at once.

**Sending a report**: open an issue with the chip's name, the probe's JSON file, and the adapter's product name and where it came from (built into a laptop: its model). The report contains the adapter's Bluetooth address and name; it does not contain anything about your headphones. A **diagnostic bundle** (Diagnostics screen) with HCI captures included also carries the probe reports, under `probe/`.

## Step 3: Record what Windows' driver does

A probe shows the chip's state, not how to change it. The vendor's Windows driver knows that: record it once and the loader repeats what it does.

1. **Restore original driver** on the adapter's card, so Windows' driver runs it again
2. Install USBPcap and Wireshark; start Wireshark as administrator and capture the USBPcap interface the adapter is on (USBPcapCMD lists the devices of each root hub)
3. Make the driver load the firmware: unplug and replug the adapter; for a built-in one, **Disable device** and **Enable device** in Device Manager. A firmware that survives a warm restart may only load after a full shutdown
4. Stop the capture once the adapter shows in Windows' Bluetooth settings. Find the adapter's device descriptor (its VID:PID) among the first packets, filter on its address (`usb.bus_id == <bus> && usb.device_address == <address>`) and save the capture

What to look for, in order:

- **Control transfers** before any HCI command: `bmRequestType` 0x40 / 0xC0 (vendor, OUT / IN) with their `bRequest`, `wValue`, `wIndex`. These often read a chip ID or reset part of the chip
- **HCI commands** (class requests, `bmRequestType` 0x20) with opcodes 0xFC00 to 0xFFFF: vendor commands. A long run of the same opcode with large parameters is the firmware being sent; its answers say how each part is acknowledged
- **Events** that are not Command Complete / Status (code 0xFF: vendor events), for example "firmware started"
- **Bulk OUT** data before the first HCI Reset: some chips take the firmware on the ACL pipe
- Where the file comes from: compare the data sent with the files in the driver's folder (Device Manager > driver details, or `C:\Windows\System32\DriverStore\FileRepository\`) and with the chip's files in [linux-firmware](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/) (`mediatek/`, `qca/`), which may be redistributed and which A2DP Windows Bridge users can download

Note any delay the driver keeps between steps (the capture's timestamps): a chip that is still resetting does not answer.

{: .warning }
Work from captures, probe reports and vendor documentation. Do not copy or closely follow GPL code, such as the Linux kernel's `btmtk` or `btqca` drivers: A2DP Windows Bridge is MIT licensed, and a loader derived from them could not be included.

## Step 4: Write the loader

A loader is a state machine with no I/O of its own: it gets the controller's events, the results of its USB requests and its timer, and returns **actions** that the app carries out. That makes it testable without the chip and keeps the USB and threading details out of it.

Start from the sample, a complete loader for a made-up chip with its tests:

1. Copy `app/src/crates/bt-host/src/bringup/sample.rs` to `app/src/crates/bt-host/src/<vendor>.rs` and add `pub mod <vendor>;` to `bt-host/src/lib.rs`
2. Replace the sample's made-up opcodes, requests and file names with what steps 2 and 3 showed
3. Keep the module comment format of `intel.rs`: what the chip needs, the steps in order, and where each fact comes from (your capture, vendor documentation)

The interface (`app/src/crates/bt-host/src/bringup.rs`):

| `ChipLoader` method | Called |
|:--|:--|
| `vendor()` | For log lines ("MediaTek") |
| `start()` | Once, on the open adapter, before HCI init |
| `on_event(event)` | For every HCI event: code, length, parameters |
| `on_control(result)` | With the result of the last `UsbControl`: the bytes read, or why it failed |
| `on_timeout()` | When the timer of the last `Timer` fires |
| `missing_file_hint(file)` | For the log when the loader ended with `MissingFile` ("x.bin from linux-firmware (mediatek/)") |

| `Action` | Does |
|:--|:--|
| `Command(bytes)` | Sends an HCI command (opcode, length, parameters) on the control pipe; logged in the HCI capture |
| `BulkCommand(bytes)` | Sends a command on the bulk OUT pipe, as some bootloaders want |
| `UsbControl(request)` | A control transfer on the default pipe (vendor registers); done at once, `on_control` gets the result before the next action. Logged in the HCI capture as a note |
| `Bootloader(on)` | Events may also come on the bulk IN pipe, and closing the adapter sends no HCI Reset |
| `Timer(ms)` | Starts or restarts the loader's single timer |
| `Done(Ok(outcome))` | Finished: HCI init follows. `AlreadyRunning`, `Loaded { firmware, detail }`, `Rom` |
| `Done(Err(error))` | The adapter cannot be used: `MissingFile`, `BadFile`, `Command { opcode, status }`, `Timeout(step)`, `Rejected(code)`, `Unsupported(why)`, `Host(why)` |

Rules that kept the Intel loader reliable:

- Wait for each acknowledgment before sending the next command, with a timer for every wait; give up with `Timeout(step)` naming the step
- Do nothing when the firmware already runs (`AlreadyRunning`): the adapter keeps it until it loses power, and a second download may fail
- Check the firmware file before sending any of it (`BadFile` with the reason), and name the exact file that is missing
- Log what you learn from the chip at `info` (chip ID, ROM version, the file chosen): it is what reports show
- Only read firmware files through the `FileReader` the app gives you; never write to the chip's flash or OTP memory

## Step 5: Test it against a simulated controller

The sample's tests show the pattern: a small simulated chip answers commands and control transfers the way the real one did in your capture, and `run()` drives the loader the way the app does. Cover at least:

- the full download, checking the commands sent (count, order, chunk sizes)
- firmware already running
- a missing file and a broken file
- a rejected command (error status) and a silent controller (timeout)
- a failed USB transfer

Bytes copied from your capture make the best test data. Then:

```bat
cargo test -p bt-host
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

## Step 6: Hook it into the app

Follow how Intel is wired in (search for `intel_loader`):

1. **`bt-host/src/chips.rs`**: the vendor's USB VIDs in `vendor_for_usb` (modules often carry the laptop maker's VID, for example AzureWave 0x13D3 or Foxconn 0x0489, shared with other chips: use VID:PID pairs then), and `ChipVendor::support_tag` from "unsupported" to "experimental"
2. **`bt-host/src/lib.rs`**: a `HostConfig` field like `intel_loader`, and the branch in `BtHost::launch` that installs the transport and starts the loader instead of HCI init
3. **`bt-host/src/app.rs`**: a `start_<vendor>_loader` like `start_intel_loader`, with the firmware folder's vendor subfolder (`read_firmware_file(&dir, "mediatek", name)`)
4. **`crates/core/src/adapter.rs`** (`configure`): set the field when an adapter of that vendor is on WinUSB and not in use
5. **`core/src/state.rs`** (`vendor_note`): a `firmware.vendor_<vendor>` note instead of `firmware.vendor_unsupported`, in all languages in `app/lang/`; the Adapters screen then offers WinUSB without developer mode
6. **Docs**: the chip table in [Setup](setup#adapters) (all languages) and, once someone used it, [Verified Hardware](hardware)

## Step 7: Try it on the adapter

Build (`cargo build --release -p a2dpwb-core-exe` in the repository root), quit the installed app, and run the core from the command line with an HCI capture:

```bat
target\release\a2dpwb-core.exe --cli -u 0e8d:7961 --scan 5 --hci-capture loader.pklg
```

The log shows each step; `loader.pklg` opens in Wireshark with every command, event and control transfer in order. Unplug the adapter (or shut down) and try again: the download must work from a cold chip, and do nothing when the firmware already runs.

{: .note }
With several adapters on WinUSB, a Realtek one is preferred and its chip setup applied, even with `-u`. Set **Adapters > Bluetooth Chip** to **Not Realtek**, or test with only your adapter on WinUSB.

Then stream to headphones for a while with the window, and check that **Restart Bluetooth** and a replug bring it back.

## Step 8: Open the pull request

- Against the current `develop-v*` branch
- Say which adapters you tested (product, VID:PID, chip), cold start and warm, and attach a probe report and the HCI capture of a download
- Mention where the facts in the loader come from
- The loader stays **experimental** until a few people have used it; the Adapters screen shows that tag
