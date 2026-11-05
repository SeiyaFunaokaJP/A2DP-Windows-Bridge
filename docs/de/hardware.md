---
title: Geprüfte Hardware
layout: default
parent: Startseite
nav_order: 2.5
---

# Geprüfte Hardware
{: .no_toc }

Adapter und Kopfhörer, mit denen BlueCaster verwendet wurde, aus den eigenen Tests des Maintainers und aus Nutzerberichten in den Issues. ✅ funktioniert · ⚠️ funktioniert mit einer Einschränkung oder teilweise · ❌ funktioniert nicht · ❓ nicht bestätigt.

Haben Sie BlueCaster mit anderer Hardware ausprobiert? Bitte berichten Sie davon in einem [Issue](https://github.com/SeiyaFunaokaJP/BlueCaster/issues) (oder für Codecs in [#11](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/11)), mit dem Text von **Diagnose > Diagnose kopieren**: Er nennt den Adapter, den Codec und die Bitrate.

## Inhalt
{: .no_toc .text-delta }

1. TOC
{:toc}

## Bluetooth-Adapter

| Adapter | Chip | USB-ID | Art | Ergebnis | Hinweise | Quelle |
|:--|:--|:--|:--|:--|:--|:--|
| TP-Link UB500 | Realtek RTL8761BU | 2357:0604 | USB-Dongle | ✅ Am besten getestet | Firmware `rtl8761bu`. Kein LE Audio (keine isochronen Kanäle) | Maintainer; [#84](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/84), [#87](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/87) |
| Generischer RTL8761B-Dongle | Realtek RTL8761BU | 0BDA:A729 | USB-Dongle | ✅ | Firmware `rtl8761bu`. Seit 2.0.2 automatisch erkannt (davor: RTL8761BU als Chip wählen). Kein LE Audio | [#66](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/66), [#86](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/86) |
| Eingebauter Realtek RTL8852BE eines Notebooks | Realtek RTL8852BU | 0BDA:4853 | Eingebaut | ✅ | Firmware `rtl8852bu`, seit 2.0.2-beta.1 erkannt. Windows hat kein eigenes Bluetooth, solange BlueCaster ihn verwendet | [#81](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/81) |
| ASUS BT400 | Broadcom BCM20702 | 0B05:17CB | USB-Dongle | ✅ Experimentell | Läuft mit seiner ROM-Firmware, keine Dateien nötig. Gemeldet unter v1 (1.0.12–1.0.17) | [#18](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/18), [#27](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/27), [#30](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/30) |
| Dongle „Realtek Bluetooth 6.0“ | Realtek RTL8761B | 0BDA:A760 | USB-Dongle | ✅ | Noch nicht automatisch erkannt (LMP-Subversion 0x8761). Kein LE Audio | [#208](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/208) |
| CSR8510-A10-Dongle | CSR8510 | 0A12:0001 | USB-Dongle | ✅ | Keine Firmware-Dateien nötig | [#32](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/32) |
| Intel AX211 (eingebaute WLAN- / Bluetooth-Karte) | Intel AX211 (TLV-Bootloader) | 8087:0033 | Eingebaut | ✅ Experimentell | Firmware aus `intel/` von linux-firmware, in 1,5–1,7 s aus dem Bootloader geladen: `ibt-0180-0041` (Variante 0x1B) oder `ibt-1040-0041` (Variante 0x19), das Protokoll nennt die Datei. Kopplung, AAC und LDAC funktionieren. Windows hat kein eigenes Bluetooth, solange BlueCaster sie verwendet | [#261](https://github.com/SeiyaFunaokaJP/BlueCaster/pull/261) |
| Adapter „MA530“ | Nicht angegeben | – | USB-Dongle | ✅ | Gemeldet unter v1.0.9 | [#3](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/3), [#9](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/9) |
| UGREEN-Dongles Bluetooth 5.4 und 6.0 | Nicht angegeben | – | USB-Dongle | ✅ | Gemeldet unter 2.0.1 | [#11](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/11#issuecomment-6563908711) |

Für MediaTek- und Qualcomm-Adapter gibt es noch keine Berichte: Was von ihnen zu erwarten ist, steht unter [Empfohlene Adapter](setup#adapters).

## Kopfhörer

| Kopfhörer | Adapter | Codecs | Hinweise | Version | Quelle |
|:--|:--|:--|:--|:--|:--|
| Sony WH-1000XM4 | TP-Link UB500 | ✅ LDAC 990 kbps (bis 96 kHz) · ✅ AAC · ✅ SBC · ✅ SBC XQ 552 kbps | Kein aptX bei diesem Modell. LDAC ist aus, solange sein Multipoint an ist | 1.0 – 2.0 | Maintainer |
| Sony WH-1000XM4 | ASUS BT400 | ✅ LDAC 990 kbps, 48 kHz / 24 Bit | Windows 10 22H2 | 1.0.12, 1.0.15 | [#18](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/18), [#27](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/27) |
| Sony WH-1000XM6 | Eingebauter RTL8852BU | ✅ LDAC 990 kbps, 48 kHz / 24 Bit (5 Minuten, 0 Pakete verloren) | LE Audio an den Kopfhörern aus. Die erste Verbindung braucht den Kopplungsmodus | 2.0.1, 2.0.2-beta.1 | [#81](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/81) |
| EarFun Air Pro 4+ | RTL8761B (0BDA:A729) | ✅ LDAC · ✅ AAC · ✅ SBC · ✅ SBC XQ · ✅ aptX · ✅ aptX Adaptive | | 2.0.2-beta.1, 2.0.2-beta.6 | [#86](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/86) |
| Grado GW100x | „MA530“ | ✅ aptX | Bietet auch aptX Adaptive an; 1.0.9 hatte keinen aptX-Adaptive-Encoder und sendete klassisches aptX (aptX Adaptive ist an ihm noch nicht ausprobiert) | 1.0.9 | [#9](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/9) |
| LHDC-V5-Kopfhörer (Modell nicht angegeben) | RTL8761B | ✅ LHDC V5 1000 kbps / 192 kHz | Die App der Kopfhörer auf ihren 192-kHz-Modus stellen; im 96-kHz-Modus nehmen sie nur 400 kbps an | 1.1.1-beta.1 | [#50](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/50) |
| OPPO Enco Free4 | Intel AX200 | ⚠️ LHDC V5 400 kbps | Sie boten BlueCaster höchstens 48 kHz und 400 kbps an, auch mit eingeschaltetem Hi-Res in ihrer App. Wenn LHDC V5 streamt (gewählt oder von der Automatik, ABR aus), fragt BlueCaster sie jetzt einmal nach Hi-Res und verbindet sich dafür sofort neu; an diesen Ohrhörern noch nicht bestätigt | 2.0.1 | [#237](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/237) |
| OPPO Enco X4 | Intel AX211 | ⚠️ LHDC V5 400 kbps | Sie boten BlueCaster höchstens 48 kHz und 400 kbps an, mit ausgeschaltetem Hi-Res in ihrer App. 2.0.2-beta.17 fragte nur die Enco Free4 nach Hi-Res; jetzt werden alle OPPO-Ohrhörer gefragt, die die Einstellung anbieten. Noch nicht bestätigt | 2.0.2-beta.17 | [#237](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/237) |
| JBL Tune 510BT | TP-Link UB500 | ✅ SBC · ⚠️ AAC (stumm) | Die AAC-Korrektur ([#89](https://github.com/SeiyaFunaokaJP/BlueCaster/pull/89)) ist an diesem Modell noch nicht bestätigt | 2.0.1 | [#43](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/43), [#84](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/84), [#87](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/87) |
| Huawei FreeBuds Pro 4 | ASUS BT400 | ✅ AAC 256 kbps · ❓ LDAC | Unter 1.0.16 boten sie nur SBC und AAC an. Mit gewähltem LDAC (automatische Auswahl aus) stellt BlueCaster jetzt ihren Modus „Klangqualität zuerst“ ein und verbindet sich neu, und stellt den Modus zurück, wenn daraus kein LDAC wird; an diesen Ohrhörern noch nicht bestätigt. L2HC ist proprietär | 1.0.16 | [#30](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/30) |
| Huawei FreeBuds 5i | RTL8761B (0BDA:A760) | ✅ LDAC 990 / 660 kbps · ✅ AAC 256 kbps · ✅ SBC | Boten LDAC nur bei manchen Verbindungen an. Bieten außerdem einen Huawei-Codec (0x027D/0x4C36) an, den BlueCaster nicht unterstützt | 2.0.1 | [#208](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/208) |
| Huawei FreeBuds Neo | RTL8761B (0BDA:A760) | ✅ LDAC 990 kbps · ✅ AAC 256 kbps · ✅ SBC | Unter 2.0.1 boten sie nur SBC und AAC an. Unter 2.0.2-beta.13 funktioniert LDAC, sobald LDAC im Profil gewählt ist (automatische Auswahl aus). In einem stark belegten 2,4-GHz-WLAN setzte es aus, bis AFH auf Manuell mit dem Kanal des Routers gestellt war | 2.0.2-beta.13 | [#208](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/208), [#30](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/30) |
| EDIFIER W820NB (Dual Gold Label) | CSR8510 A10 | ✅ SBC · ❓ LDAC | LDAC schlug unter 1.0.16 fehl und wurde in 1.0.17 behoben; seitdem nicht bestätigt | 1.0.16 | [#32](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/32) |
| ROSESELSA Ceramics Ultra | UGREEN Bluetooth 5.4 / 6.0 | ✅ LDAC · ⚠️ LHDC V5 · ✅ AAC · ✅ SBC | LDAC hatte die stabilste Verbindungsqualität und Bitrate. LHDC V5 kann zufällig aussetzen | 2.0.1 | [#11](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/11#issuecomment-6563908711) |

## Testausrüstung

| Was | Aufbau | Codecs | Ergebnis |
|:--|:--|:--|:--|
| Messempfänger [`tools/linux_sink`](usage#receiver) | TP-Link UB500 → Ubuntu 26.04 (Intel AX201) über Funk | SBC, AAC, aptX, aptX HD, aptX LL, LDAC | ✅ Ohne Verluste empfangen |
| Virtuelle Verbindung `tools/emu` (Bumble) | Kein Funk | Jeder Codec und LE Audio | ✅ End-to-End-Test besteht |

## Noch nicht geprüft

- **Codecs an echten Kopfhörern**: aptX HD, aptX Low Latency, Opus, Opus 05, FastStream
- **LE Audio**: nur gegen den Emulator getestet; keiner der obigen Adapter hat isochrone Kanäle
- **Windows auf ARM64** und **Windows 10 mit v2** (v1 lief unter Windows 10 22H2: [#18](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/18), [#30](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/30))
