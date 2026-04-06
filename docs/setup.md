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

- **Windows 10/11** (x64)
- **Dedicated USB Bluetooth adapter** (separate from built-in Bluetooth)
- **Zadig** ([https://zadig.akeo.ie/](https://zadig.akeo.ie/))

## Step 1: Install WinUSB Driver

A2DPWB communicates directly with a USB Bluetooth adapter via WinUSB. You need a **dedicated adapter** -- your built-in Bluetooth continues to work normally for regular Windows Bluetooth.

1. Plug in a USB Bluetooth adapter
2. Download and open [Zadig](https://zadig.akeo.ie/)
3. Go to **Options > List All Devices**
4. Select your USB Bluetooth adapter from the dropdown
5. Select **WinUSB** as the target driver
6. Click **Replace Driver**

{: .warning }
While the adapter is in WinUSB mode, Windows cannot use it for regular Bluetooth. Use your built-in Bluetooth for normal peripherals (keyboard, mouse, etc.).

## Step 2: Find Your Headphone's Bluetooth Address

You need the Bluetooth MAC address of your headphones/speakers.

**From Windows Settings:**
1. Open **Settings > Bluetooth & devices**
2. Click on your audio device
3. Click **Properties**
4. The Bluetooth address is shown (format: `AA:BB:CC:DD:EE:FF`)

**From A2DPWB GUI:**
- Launch `A2DPWB.exe` and paired Bluetooth audio devices are listed in the device dropdown

**From CLI:**
```
A2DPWB.exe --cli -l
```

## Step 3: Run A2DPWB

**GUI:**
```
A2DPWB.exe
```

**CLI:**
```
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF
```

See [Usage](usage) for full details.

---

## Firmware (Realtek Adapters)

Realtek-based USB Bluetooth adapters (e.g., TP-Link UB500, RTL8761BU dongles) require proprietary firmware to operate. **Intel and CSR adapters do not need this step.**

### GUI (Guided Download)

Launch `A2DPWB.exe` and open **Firmware** dialog. If firmware is missing, a warning is displayed. Use the **Open Download Page** button to open the [linux-firmware/rtl_bt](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/rtl_bt) page in your browser, then use **Open Config Folder** to open the destination folder. Download the required `.bin` files and place them in the config folder.

The dialog auto-detects your adapter chipset and shows which firmware files are needed.

### Manual Download

1. Download the firmware and config `.bin` files for your chipset from [linux-firmware/rtl_bt](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/rtl_bt) (e.g., `rtl8761bu_fw.bin` and `rtl8761bu_config.bin`)
2. Place both files in the A2DPWB config folder (same directory as `A2DPWB.exe`, or the path shown in the Firmware dialog)

{: .note }
These firmware files are proprietary Realtek binaries distributed via the linux-firmware project. They are not included in this repository. See [WHENCE](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/WHENCE) for redistribution terms.

---

## Recommended Adapters

| Chipset | Example Products | Notes |
|:--------|:-----------------|:------|
| Intel | Intel AX200/AX210 | Works out of the box, no firmware needed |
| CSR | Generic CSR8510 dongles | Works out of the box, no firmware needed |
| Realtek | TP-Link UB500, RTL8761BU | Requires firmware download (see above) |

USB Bluetooth 5.0+ adapters generally work best for high-bitrate codecs like LDAC.
