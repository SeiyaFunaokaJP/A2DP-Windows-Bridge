---
title: Verified Hardware
layout: default
nav_order: 3.5
english_only: true
---

# Verified Hardware
{: .no_toc }

Adapters and headphones BlueCaster has been used with, from the maintainer's own tests and from user reports in the issues. ✅ works · ⚠️ works with a limit, or partly · ❌ does not work · ❓ not confirmed.

Tried BlueCaster with other hardware? Please tell us in an [issue](https://github.com/SeiyaFunaokaJP/BlueCaster/issues) (or in [#11](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/11) for codecs), with the text of **Diagnostics > Copy Diagnostics**: it names the adapter, the codec and the bitrate.

## Table of contents
{: .no_toc .text-delta }

1. TOC
{:toc}

## Bluetooth adapters

| Adapter | Chip | USB ID | Kind | Result | Notes | Source |
|:--|:--|:--|:--|:--|:--|:--|
| TP-Link UB500 | Realtek RTL8761BU | 2357:0604 | USB dongle | ✅ Best tested | Firmware `rtl8761bu`. No LE Audio (no isochronous channels) | Maintainer; [#84](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/84), [#87](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/87) |
| Generic RTL8761B dongle | Realtek RTL8761BU | 0BDA:A729 | USB dongle | ✅ | Firmware `rtl8761bu`. Detected automatically since 2.0.2 (before: choose RTL8761BU as the chip). No LE Audio | [#66](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/66), [#86](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/86) |
| Laptop's built-in Realtek RTL8852BE | Realtek RTL8852BU | 0BDA:4853 | Built in | ✅ | Firmware `rtl8852bu`, recognized since 2.0.2-beta.1. Windows has no Bluetooth of its own while BlueCaster uses it | [#81](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/81) |
| ASUS BT400 | Broadcom BCM20702 | 0B05:17CB | USB dongle | ✅ Experimental | Runs on its ROM firmware, no files needed. Reported on v1 (1.0.12–1.0.17) | [#18](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/18), [#27](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/27), [#30](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/30) |
| "Realtek Bluetooth 6.0" dongle | Realtek RTL8761B | 0BDA:A760 | USB dongle | ✅ | Not detected automatically yet (LMP subversion 0x8761). No LE Audio | [#208](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/208) |
| CSR8510 A10 dongle | CSR8510 | 0A12:0001 | USB dongle | ✅ | No firmware files needed | [#32](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/32) |
| "MA530" adapter | Not given | – | USB dongle | ✅ | Reported on v1.0.9 | [#3](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/3), [#9](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/9) |

Intel, MediaTek and Qualcomm adapters have no reports yet: see [Recommended Adapters](setup#adapters) for what is expected of them.

## Headphones

| Headphones | Adapter | Codecs | Notes | Version | Source |
|:--|:--|:--|:--|:--|:--|
| Sony WH-1000XM4 | TP-Link UB500 | ✅ LDAC 990 kbps (up to 96 kHz) · ✅ AAC · ✅ SBC · ✅ SBC XQ 552 kbps | No aptX on this model. LDAC is off while its multipoint is on | 1.0 – 2.0 | Maintainer |
| Sony WH-1000XM4 | ASUS BT400 | ✅ LDAC 990 kbps, 48 kHz / 24 bit | Windows 10 22H2 | 1.0.12, 1.0.15 | [#18](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/18), [#27](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/27) |
| Sony WH-1000XM6 | Built-in RTL8852BU | ✅ LDAC 990 kbps, 48 kHz / 24 bit (5 minutes, 0 packets lost) | LE Audio off on the headphones. The first connection needs pairing mode | 2.0.1, 2.0.2-beta.1 | [#81](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/81) |
| EarFun Air Pro 4+ | RTL8761B (0BDA:A729) | ✅ LDAC · ✅ AAC · ✅ SBC · ✅ SBC XQ · ✅ aptX · ✅ aptX Adaptive | | 2.0.2-beta.1, 2.0.2-beta.6 | [#86](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/86) |
| Grado GW100x | "MA530" | ✅ aptX | Offers aptX Adaptive too; 1.0.9 had no aptX Adaptive encoder and sent classic aptX (aptX Adaptive is not tried on it yet) | 1.0.9 | [#9](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/9) |
| LHDC V5 headphones (model not given) | RTL8761B | ✅ LHDC V5 1000 kbps / 192 kHz | Set the headphones' app to its 192 kHz mode; in 96 kHz mode they accept only 400 kbps | 1.1.1-beta.1 | [#50](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/50) |
| JBL Tune 510BT | TP-Link UB500 | ✅ SBC · ⚠️ AAC (silent) | The AAC fix ([#89](https://github.com/SeiyaFunaokaJP/BlueCaster/pull/89)) is not confirmed on this model yet | 2.0.1 | [#43](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/43), [#84](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/84), [#87](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/87) |
| Huawei FreeBuds Pro 4 | ASUS BT400 | ✅ AAC 256 kbps · ❓ LDAC | They offered only SBC and AAC on 1.0.16. With LDAC chosen (Auto-select off), BlueCaster now sets their "sound quality first" mode and connects again, and sets the mode back if LDAC does not come of it; not confirmed on these earbuds yet. L2HC is proprietary | 1.0.16 | [#30](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/30) |
| Huawei FreeBuds 5i | RTL8761B (0BDA:A760) | ✅ LDAC 990 / 660 kbps · ✅ AAC 256 kbps · ✅ SBC | Offered LDAC on some connections only. Also offers a Huawei codec (0x027D/0x4C36) that BlueCaster does not support | 2.0.1 | [#208](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/208) |
| Huawei FreeBuds Neo | RTL8761B (0BDA:A760) | ✅ AAC 256 kbps · ✅ SBC · ❌ LDAC | They do not offer LDAC to BlueCaster (only SBC and AAC), even with "Prioritize sound quality" and multi connect off, as with the FreeBuds Pro 4 | 2.0.1 | [#208](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/208) |
| EDIFIER W820NB (dual gold label) | CSR8510 A10 | ✅ SBC · ❓ LDAC | LDAC failed on 1.0.16 and was fixed in 1.0.17; not confirmed since | 1.0.16 | [#32](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/32) |

## Test equipment

| What | Setup | Codecs | Result |
|:--|:--|:--|:--|
| Measuring receiver [`tools/linux_sink`](usage#receiver) | TP-Link UB500 → Ubuntu 26.04 (Intel AX201) over the air | SBC, AAC, aptX, aptX HD, aptX LL, LDAC | ✅ Received without loss |
| Virtual link `tools/emu` (Bumble) | No radio | Every codec, and LE Audio | ✅ End-to-end test passes |

## Not verified yet

- **Codecs on real headphones**: aptX HD, aptX Low Latency, Opus, Opus 05, FastStream
- **LE Audio**: tested only against the emulator; none of the adapters above has isochronous channels
- **Windows on ARM64**, and **Windows 10 with v2** (v1 ran on Windows 10 22H2: [#18](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/18), [#30](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/30))
