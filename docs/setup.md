---
title: Setup
layout: default
nav_order: 2
---

# Setup
{: .no_toc }

## Table of contents
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## Prerequisites

- **Windows 10 (22H2) / 11** (x64 or ARM64)
- **A USB Bluetooth adapter for BlueCaster**: a separate dongle is recommended; a built-in USB adapter works too, but Windows then has no Bluetooth of its own (see [Step 2](#winusb))

BlueCaster assigns the WinUSB driver to the adapter itself. [Zadig](https://zadig.akeo.ie/) is not needed any more (it still works, see [Using Zadig instead](#zadig)).

## Installing BlueCaster

Each [release](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/releases/latest) comes in two forms with the same files:

- **Installer** (`BlueCaster-<version>-win64-setup.exe`): installs for the current user without administrator rights, adds a Start menu entry (**BlueCaster**) and an uninstaller, and installs the Microsoft Edge WebView2 Runtime if it is missing (Windows 11 includes it). Updates install themselves from the app (**Settings > Updates > Update now**). The programs go to `%LOCALAPPDATA%\BlueCaster\` (the three programs, the uninstaller and a `licenses\` folder with the license texts)
- **Portable** (`BlueCaster-<version>-win64-portable.zip`): extract anywhere and run `bluecaster.exe`. Keep the three programs (`bluecaster.exe`, `bluecaster-core.exe`, `bluecaster-driver.exe`) together. The portable copy only tells you about updates; it needs the WebView2 Runtime installed

Both keep settings, profiles, link keys and firmware in `%APPDATA%\BlueCaster`, so you can switch between them. Uninstalling deletes that folder unless you turn **Delete user data** off (a silent uninstall keeps it).

### Upgrading from v1 {#from-v1}

Run the installer over v1. Its **Ready** page says which earlier versions it uninstalls first (v1, or any 2.0.x). It asks you to close v1 if it is running, then uninstalls v1 and keeps the settings folder: profiles, the pairing with your headphones and the firmware files carry over, and the headphones do not need to be paired again. An adapter that already has WinUSB (from Zadig) keeps it. If **Start with Windows** was on, it now starts v2.

At its first start v2 copies the v1 settings to `%APPDATA%\BlueCaster\backup\v1\` before it reads them.

### Upgrading from 2.0.x (A2DP Windows Bridge) {#from-2x}

BlueCaster 3.0.0 is the new name of A2DP Windows Bridge. Run the 3.0.0 installer over the installed 2.0.x: the setup uninstalls the old version by itself and keeps your settings. The Start menu entry and the **Start with Windows** entry move to "BlueCaster".

At its first start 3.0.0 renames the folder `%APPDATA%\A2DPWB` to `%APPDATA%\BlueCaster` and sorts its files into subfolders (see [Settings and Files](architecture#settings-and-files)). `backup\v2\` holds a copy of the 2.0.x settings and pairings, so that 2.0.2 can be installed again: copy those files back to `%APPDATA%\A2DPWB`.

The headphones may keep showing the old Bluetooth name "A2DPWB" until you pair them again.

## Step 1: First-Run Setup {#first-run}

Plug in the USB Bluetooth adapter and start BlueCaster. When there is no audio device yet, or no adapter has WinUSB, BlueCaster opens **Set up BlueCaster**, three steps:

1. **Choose the adapter**: the USB Bluetooth adapters found. If yours is missing, plug it in and press **Refresh**. A green **Supported** or a red **Unsupported** says whether BlueCaster can use each one; an unsupported one is listed but cannot be chosen
2. **Driver and firmware**: **Replace with WinUSB driver** (see [Step 2](#winusb)), and for Realtek adapters the firmware files (see [Firmware](#firmware))
3. **Find your audio devices**: the scan starts by itself. Put the audio device into pairing mode and press **Connect** next to it (or type its address and press **Use**): BlueCaster saves it with the best codec it has (Auto) and connects; it leaves the list (also after **Back** and **Next**) and its row gets **Disconnect** (**Cancel** while it connects): a wrong device goes back to the list and the scan resumes, so that you can connect another one. Then **Finish** (see [Step 3](#headphones)). Its codec can be changed later on **Codecs**. The step's **Startup** block has **Start with Windows** and **Start in the tray**, the same switches as in Settings, saved at once

**Set up later** skips the rest; **Settings > Run setup** opens it again.

## Step 2: Assign WinUSB to the Adapter {#winusb}

BlueCaster communicates directly with a USB Bluetooth adapter via WinUSB, the generic USB driver included in Windows. A **dedicated adapter** is recommended -- your built-in Bluetooth then continues to work normally for regular Windows Bluetooth.

1. Open the **Adapter** screen (or the setup's **Driver and firmware** step)
2. The adapter's **Driver** reads **Original driver** (or **No driver**): the driver Windows or the manufacturer installed, which WinUSB replaces. Click **Replace with WinUSB driver**
3. Read the confirmation and continue: Windows asks for administrator permission (UAC)
4. The driver now reads **WinUSB driver**

If Windows could not load WinUSB right away (the adapter was in use), BlueCaster says so and the driver line shows **Replug or restart Windows**. Unplug the adapter and plug it back in; for a built-in adapter, restart Windows. Until then, connecting fails with `failed to initialize WinUSB (error 50)` in the log.

{: .warning }
While WinUSB is assigned, Windows' own Bluetooth no longer sees the adapter: devices paired with Windows through it stop working until the driver is restored. Use your built-in Bluetooth for normal peripherals (keyboard, mouse, etc.). A built-in adapter that is a USB device (a Realtek one on many laptops) can be given to BlueCaster too, but then **the PC has no Windows Bluetooth at all** (Bluetooth settings, mice, keyboards, other audio devices) until you restore its driver.

**Restore original driver** on the same screen gives the adapter back to Windows: Windows picks its own driver again (normally its Bluetooth driver) and the adapter works with Windows' Bluetooth. BlueCaster cannot stream through it until WinUSB is assigned again. The uninstaller offers the same: a page of the uninstall wizard has **Restore the original Bluetooth driver** (when BlueCaster put adapters on WinUSB; one administrator prompt) and **Delete user data** (settings, codecs, pairings and firmware in `%APPDATA%\BlueCaster`, and what the window remembers, such as **Show only supported codecs**), both on by default. Updates keep WinUSB, and so does a silent uninstall (`/S`).

### Using Zadig instead {#zadig}

A WinUSB driver installed with [Zadig](https://zadig.akeo.ie/) works just as well:

1. Open Zadig, go to **Options > List All Devices**
2. Select your USB Bluetooth adapter from the dropdown
3. Select **WinUSB** as the target driver and click **Replace Driver**

### By hand in Device Manager {#device-manager}

If the adapter is missing from BlueCaster's list, a button on the **Adapter** screen fails, or your adapter behaves differently from others of its kind, Windows' Device Manager (right-click **Start** > **Device Manager**) does the same by hand. Both need administrator rights. Menu names may differ slightly between Windows versions.

**Replace with WinUSB driver**

1. Find the adapter: normally under **Bluetooth** with its own name (for example "TP-Link Bluetooth USB Adapter"), or with a yellow mark when its driver failed to start. **View > Devices by connection** helps tell it apart from the PC's built-in Bluetooth, which you leave alone unless you mean to give it to BlueCaster
2. Right-click it > **Update driver** > **Browse my computer for drivers** > **Let me pick from a list of available drivers on my computer**
3. If Windows asks for a device type, choose **Universal Serial Bus devices**. Clear **Show compatible hardware**, choose the manufacturer **WinUsb Device** and the model **WinUsb Device**, click **Next** and confirm the warning
4. The adapter now shows as **WinUsb Device** under **Universal Serial Bus devices**. Restart BlueCaster (or open the **Adapter** screen again): it uses the adapter as with Zadig

**Restore the original driver**

1. Under **Universal Serial Bus devices**, right-click the adapter (**WinUsb Device**) > **Uninstall device**. If the WinUSB driver came from Zadig, also tick **Attempt to remove the driver for this device**, or Windows picks Zadig's driver again
2. Unplug the adapter and plug it in again (or **Action > Scan for hardware changes**). Windows installs its own driver and the adapter appears under **Bluetooth**
3. If it shows a yellow mark there instead (its driver failed to start, for example Code 31), the manufacturer's driver does not work with it: right-click > **Update driver** > **Browse my computer for drivers** > **Let me pick from a list of available drivers on my computer** > **Generic Bluetooth Adapter**, or assign WinUSB again as above to use it with BlueCaster

## Step 3: Find Your Headphones {#headphones}

**Scan Devices** (on **Audio Devices**) finds nearby devices with the USB adapter for 30 seconds, listing them as they answer. In the setup the scan starts by itself and goes on until a device is connected (it then leaves the list; **Disconnect** on its row puts it back and resumes the scan), **Stop** is pressed or the setup is closed. Put the headphones into pairing mode first. Devices BlueCaster has paired with before are listed as **Paired**.

If the scan does not find them, enter the Bluetooth address under **Manual:** and click **Use**. Typed or pasted addresses are formatted as you go: `AA:BB:CC:DD:EE:FF`, `aa-bb-cc-dd-ee-ff`, `AABBCCDDEEFF` and `AA BB CC DD EE FF` all become `AA:BB:CC:DD:EE:FF`.

**From Windows Settings** (if the headphones were paired with the PC's built-in Bluetooth):
1. Open **Settings > Bluetooth & devices**
2. Click on your audio device
3. Click **Properties**
4. The Bluetooth address is shown (format: `AA:BB:CC:DD:EE:FF`)

**From the command line** (quit BlueCaster first, tray > **Exit**):
```
bluecaster-core.exe --cli --scan
```

The headphones start with **Auto** as the codec (**Codecs**) and the system audio as the output source (**Audio Devices**); the defaults give the best sound. See [Usage](usage) for everything else.

---

## Firmware (Realtek Adapters) {#firmware}

Realtek-based USB Bluetooth adapters (e.g., TP-Link UB500, RTL8761BU dongles) require proprietary firmware to operate. CSR adapters need no firmware. Intel and Broadcom adapters (experimental) may need firmware files of their own: see [Recommended Adapters](#adapters).

### In the App (Guided Download)

The **Adapter** screen (and the setup's **Driver and firmware** step) shows the firmware of the adapter: **Firmware OK**, **No firmware files needed**, or **Firmware not found** with the required file names. Use the **Open Download Page** button to open the [linux-firmware/rtl_bt](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/rtl_bt) page in your browser, then use **Open Config Folder** to open the destination folder. Download the required `.bin` files and place them in the config folder.

For the chips in the **Bluetooth Chip** list, **Download Firmware** does this for you: after you confirm, BlueCaster downloads the files from linux-firmware (kernel.org), checks each one against the SHA-256 it expects and saves it in the config folder. A chip that linux-firmware has no config file for gets an empty `_config.bin` (the chip then uses its own settings). The files are not part of BlueCaster; they are downloaded only when you ask for them.

BlueCaster detects the adapter's chipset and shows which firmware files are needed.

Without the firmware, a Realtek adapter still opens and answers on its ROM code, so it looks fine, but connecting or streaming fails. BlueCaster then says that the firmware is not loaded. A module keeps firmware uploaded in an earlier session until it loses power, so with the files gone it may keep working until the PC is restarted or the adapter is unplugged.

### Bluetooth Chip setting

The **Bluetooth Chip** list on the **Adapter** screen decides whether BlueCaster runs Realtek initialization (firmware upload) on the adapter:

| Choice | Meaning |
|:-------|:--------|
| **Auto** (default) | Decided from the detected adapter each time BlueCaster starts. A known Realtek adapter gets its chip; any other adapter gets no Realtek initialization |
| **Not Realtek** | Never run Realtek initialization. Use it for Intel, Broadcom, CSR and other adapters if Auto does not recognize yours |
| **RTL8761BU**, **RTL8822CU**, ... | That Realtek chip. Needed for a Realtek-based adapter that Auto does not recognize (an OEM product under its own VID:PID). The status line shows whether its `.bin` files are in the config folder |
| **Custom** | Uses `custom_fw.bin` and `custom_config.bin` from the config folder |

Do not choose a Realtek chip or Custom for a non-Realtek adapter: BlueCaster would try to upload Realtek firmware to it, and the adapter may fail to start.

### Manual Download

1. Download the firmware and config `.bin` files for your chipset from [linux-firmware/rtl_bt](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/rtl_bt) (e.g., `rtl8761bu_fw.bin` and `rtl8761bu_config.bin`)
2. Place both files in the `firmware` folder of the BlueCaster config folder (`%APPDATA%\BlueCaster\firmware`; the config folder is also under **Settings > Settings folder**)

{: .note }
These firmware files are proprietary Realtek binaries distributed via the linux-firmware project. They are not included in this repository. See [WHENCE](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/WHENCE) for redistribution terms.

---

## Recommended Adapters {#adapters}

Which adapters and headphones have actually been used with BlueCaster, with the codecs and the reports: [Verified Hardware](hardware).

| Chipset | Example Products | Notes |
|:--------|:-----------------|:------|
| Realtek | TP-Link UB500, RTL8761BU | Requires firmware download (see above). Best tested |
| CSR | Generic CSR8510 dongles | Works out of the box, no firmware needed |
| Broadcom | ASUS BT400 (BCM20702) | *Experimental.* Adapters that report the vendor class `FF/01/01` instead of Bluetooth `E0/01/01` (BCM20702 OEM dongles) are detected too. Runs on ROM firmware; an optional PatchRAM `.hcd` (e.g. `BCM20702A1-0b05-17cb.hcd`) in the config folder is uploaded at startup |
| Intel | Intel 8265 / 9260 / AX200 / AX201 | *Experimental.* In bootloader mode (e.g. after a cold boot) needs the matching `ibt-*.sfi` + `ibt-*.ddc` from linux-firmware `intel/` in the config folder. AX210 and newer (TLV bootloader) are not supported yet |
| MediaTek, Qualcomm | MT7921/MT7922, QCA61x4 | Not supported: need firmware loaders BlueCaster does not have |

USB Bluetooth 5.0+ adapters generally work best for high-bitrate codecs like LDAC.

When several adapters have WinUSB, BlueCaster uses the first one it finds. Keep WinUSB on only the adapter you use for BlueCaster.
