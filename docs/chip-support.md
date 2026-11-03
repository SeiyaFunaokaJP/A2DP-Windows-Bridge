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
2. Install USBPcap and Wireshark; start Wireshark as administrator and capture the USBPcap interface the adapter is on. `USBPcapCMD.exe` (in USBPcap's folder) lists the devices on each root hub; a laptop's built-in adapter sits on an internal hub, often listed only as "USB Composite Device" or by its VID:PID. When unsure, capture every USBPcap interface at once
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

Paths from here on are from the repository root. Three crates matter:

| Crate (package) | Folder | What is there |
|:--|:--|:--|
| `bt-host` | `app/src/crates/bt-host` | The Bluetooth host: loaders, the probe, chip detection |
| `a2dpwb-core` (library) | `app/src/crates/core` | Settings and adapters: which loader runs |
| `a2dpwb-core-exe` | `app/src/core` | The service executable (`a2dpwb-core.exe`): the window's requests, adapter notes |

The last two both have "core" in their name; the package names above are what `cargo -p` takes.

A loader is a state machine with no I/O of its own: it gets the controller's events, the results of its USB requests and its timer, and returns **actions** that the app carries out. That makes it testable without the chip and keeps the USB and threading details out of it.

1. Copy `app/src/crates/bt-host/src/bringup/sample.rs`, a complete loader for a made-up chip with its tests, to `app/src/crates/bt-host/src/<vendor>.rs` (for example `mediatek.rs`). Add `pub mod <vendor>;` to `app/src/crates/bt-host/src/lib.rs` beside `pub mod intel;`
2. Keep the names: the type is `Loader` (used as `<vendor>::Loader`), `vendor()` returns the vendor's name. Keep the tests in the file, as the sample does
3. Replace everything the sample marks "made up" with what steps 2 and 3 showed: the version check, the USB request, the file name, the download command and how it is acknowledged, how the firmware is started
4. Rewrite the module comment at the top: what the chip needs, the steps in order, and where each fact comes from (your capture of Windows' driver, vendor documentation, probe reports). Do not cite other drivers' source code as the source of a step

### The interface

Everything is in `app/src/crates/bt-host/src/bringup.rs`.

| `ChipLoader` method | Called |
|:--|:--|
| `vendor()` | For log lines ("MediaTek") |
| `start()` | Once, on the open adapter, before HCI init |
| `on_event(event)` | For every HCI event: code, length, parameters |
| `on_control(result)` | With the result of the last `UsbControl`: the bytes read, or why it failed |
| `on_timeout()` | When the timer of the last `Timer` fires |
| `missing_file_hint(file)` | For the log when the loader ended with `MissingFile`: what to put where ("x.bin from linux-firmware (mediatek/)") |

After `Done` the app drops the loader: a late answer never reaches it.

| `Action` | Does |
|:--|:--|
| `Command(bytes)` | Sends an HCI command (opcode, length, parameters) on the control pipe; logged in the HCI capture |
| `BulkCommand(bytes)` | Sends a command on the bulk OUT pipe, where some bootloaders take the firmware |
| `UsbControl(request)` | A control transfer on the default pipe (vendor registers); done at once, `on_control` gets the result before the next action. Logged in the HCI capture as a note |
| `Bootloader(on)` | For a chip whose loader answers on the bulk IN pipe, or must not get an HCI Reset (some hang on one): on before the first command, off once the operational firmware runs. A ROM chip that answers HCI normally does not need it |
| `Timer(ms)` | Starts or restarts the loader's single timer |
| `Done(Ok(outcome))` | Finished: HCI init follows. `AlreadyRunning`, `Loaded { firmware, detail }`, `Rom` (`Probed` is the probe's own) |
| `Done(Err(error))` | The adapter cannot be used: `MissingFile(name)`, `BadFile { file, why }`, `Command { opcode, status }`, `Timeout(step)`, `Rejected(code)`, `BadVersion`, `UnsupportedVariant(v)`, `Unsupported(why)`, `Host(why)` |

Helpers, so a loader does not parse bytes by hand:

- `hci_command(opcode, params)` builds a command
- `parse_event(event)` gives `HciEvent::Complete { opcode, status, ret }` (return parameters after the status), `Status { opcode, status }`, `Vendor(params)` (event 0xFF) or `Other { code, params }`

**Firmware files**: the loader gets a `FileReader` and asks it for a file **name without a folder** (`"BT_RAM_CODE_MT7961_1_2_hdr.bin"`). The app looks in the firmware folder (`%APPDATA%\A2DPWB\firmware`) and in the vendor's subfolder there (`firmware\mediatek\`), as linux-firmware lays them out; step 6 sets that subfolder.

### Patterns

- **One command, one acknowledgment**: send, start the timer, and in the waiting state accept only the Command Complete of that opcode. Anything else is ignored, so a late answer does not move the loader on
- **A command answered twice** (Command Complete, then a vendor event such as "firmware started"): two states, the second with its own timer. The sample's `Launching` and `Starting` do this
- **An error status** in a Command Complete or Command Status ends the loader with `Command { opcode, status }`
- **Already running**: when the version or a register shows the operational firmware, finish with `AlreadyRunning` at once. The adapter keeps its firmware until it loses power, and a second download may fail
- **Check the file first**: parse and check the whole file before sending any of it (`BadFile` with the reason), and name the exact file that is missing
- **Log** what you learn from the chip at `info` (chip ID, ROM version, the file chosen): it is what reports show
- Never write to the chip's flash or OTP memory, and read files only through the `FileReader`

## Step 5: Test it against a simulated controller

The tests use `app/src/crates/bt-host/src/bringup/sim.rs`. You write a small simulated chip (`impl SimController`) that answers commands and control transfers the way the real one did in your capture, and `sim::run(&mut loader, &mut chip)` drives the loader exactly as the app does, including the timer. The result has the outcome, every command and control transfer sent, and how often the timer fired. `sim::command_complete`, `command_status`, `vendor_event`, `opcode` and `params` build and read the bytes.

Cover at least, as the sample does:

- the full download, checking the commands sent (count, order, sizes)
- firmware already running
- a missing file, and a broken file (nothing of it sent)
- a rejected command (error status), a silent controller (timeout), and each wait that can time out
- a failed USB transfer, if the loader uses one

Bytes copied from your capture make the best test data. Run them with:

```bat
cargo test -p bt-host
```

## Step 6: Hook it into the app

Every file to change, for a vendor already listed as unsupported (MediaTek, Qualcomm) or a new one. The compiler points at most of the `match`es you miss.

**Detection** (`app/src/crates/bt-host/src/chips.rs`):

1. `ChipVendor`: a new vendor gets a variant (with a doc line) and an arm in `name()`
2. `support_tag()`: the vendor moves from `"unsupported"` to `"experimental"`
3. `vendor_for_usb()`: the vendor's USB VIDs. Laptop modules often carry another maker's VID (for example AzureWave 0x13D3, Foxconn 0x0489), shared with other chips: match those as `VID if pid == PID`, never the whole VID. Every VID listed here also makes `is_bluetooth_vendor_vid` treat the maker's devices as Bluetooth adapters
4. `company_name()`: the Bluetooth SIG company ID from Read Local Version, if it is not there yet
5. Tests: `app/src/crates/bt-host/tests/chips.rs` (the VID:PIDs and the tag)

**Opening the adapter**: Bluetooth-class devices (class E0/01/01) open as they are. A chip that reports a vendor class (FF) in its ROM also needs its VID:PIDs in `BUILTIN_VENDOR_CLASS` in `app/src/crates/core/src/adapter.rs`; the probe report's `usb.interfaces` line shows the class.

**Starting the loader**, as Intel does (search for `intel_loader`):

6. `app/src/crates/bt-host/src/lib.rs`: a `HostConfig` field like `intel_loader` (with its default in `HostConfig::new`, the destructuring in `BtHost::launch`, and the debug line), and the branch in `BtHost::launch` that installs the transport and calls the app's start function instead of `setup_hci_and_power_on`
7. `app/src/crates/bt-host/src/app.rs`: a `start_<vendor>_loader` like `start_intel_loader`, giving the loader `read_firmware_file(&dir, "<vendor>", name)` (the subfolder name as in linux-firmware) and calling `start_loader`
8. `app/src/crates/core/src/adapter.rs`, `configure()`: set the field when an adapter of the vendor is on WinUSB and not in use (as `config.intel_loader`), and take the vendor out of the "firmware loading for this vendor is not implemented" warning below it
9. `app/src/crates/core/src/service.rs`, `host_config()`: add the field to the debug line

**What the window shows**:

10. `app/src/core/src/state.rs`, `vendor_note()`: an arm with a new key `firmware.vendor_<vendor>`, out of the `firmware.vendor_unsupported` arm. The Adapters screen offers WinUSB to every adapter whose note is not `firmware.vendor_unsupported`, so the UI needs no change
11. `app/lang/*.json` (all 8 languages, same line in each): the new key, as the Intel one ("… adapter (experimental). …"); and take the vendor's name out of `firmware.vendor_unsupported` if it names it

**Docs**:

12. The chip table in `docs/setup.md` and `docs/*/setup.md` (all languages): a row like Intel's, with the firmware files and where they go (`firmware` folder, or its `<vendor>` subfolder) and *Experimental.*; take the vendor out of the "Not supported" row
13. [Verified Hardware](hardware) (`docs/hardware.md`) once someone has used it

## Step 7: Try it on the adapter

Build in the repository root (`cargo build --release -p a2dpwb-core-exe`), quit the installed app (tray > Quit), and run the core from the command line with an HCI capture:

```bat
target\release\a2dpwb-core.exe --cli -u 0e8d:7961 --scan 5 --hci-capture loader.pklg
```

The log shows each step; `loader.pklg` opens in Wireshark with every command, event and control transfer in order. Unplug the adapter (or shut down) and try again: the download must work from a cold chip, and do nothing when the firmware already runs.

{: .note }
With several adapters on WinUSB, a Realtek one is preferred and its chip setup applied, even with `-u`. Set **Adapters > Bluetooth Chip** to **Not Realtek**, or test with only your adapter on WinUSB.

Then stream to headphones for a while with the window, and check that **Restart Bluetooth** and a replug bring it back.

## Step 8: Open the pull request

Before opening it, in the repository root (see [Building](building) for the setup):

```bat
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
python tools/check_lang.py
```

The workspace includes the window's crate, which needs the UI built once first (`cd app\src\ui && pnpm install && pnpm build`). Without Node.js, run clippy and the tests on the three crates above instead (`-p bt-host -p a2dpwb-core -p a2dpwb-core-exe`) and say so in the pull request. `pwsh ./build.ps1 -Test` runs everything.

In the pull request:

- Base it on the current `develop-v*` branch
- Say which adapters you tested (product, VID:PID, chip), cold start and warm, and attach a probe report and the HCI capture of a download
- Say where the facts in the loader come from
- The loader stays **experimental** until a few people have used it; the Adapters screen shows that tag
