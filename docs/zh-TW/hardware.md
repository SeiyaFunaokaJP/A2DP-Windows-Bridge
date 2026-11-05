---
title: 已驗證的硬體
layout: default
parent: 首頁
nav_order: 2.5
---

# 已驗證的硬體
{: .no_toc }

曾與 BlueCaster 搭配使用的介面卡與耳機，資料來自維護者自己的測試，以及使用者在 issue 中的回報。✅ 可用 · ⚠️ 可用但有限制，或部分可用 · ❌ 無法使用 · ❓ 尚未確認。

在其他硬體上試過 BlueCaster 嗎？請在 [issue](https://github.com/SeiyaFunaokaJP/BlueCaster/issues)（編解碼器相關請到 [#11](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/11)）告訴我們，並附上 **診斷 > 複製診斷資訊** 的文字：其中會寫明介面卡、編解碼器與位元率。

## 目錄
{: .no_toc .text-delta }

1. TOC
{:toc}

## 藍牙介面卡

| 介面卡 | 晶片 | USB ID | 類型 | 結果 | 備註 | 來源 |
|:--|:--|:--|:--|:--|:--|:--|
| TP-Link UB500 | Realtek RTL8761BU | 2357:0604 | USB 接收器 | ✅ 測試最完整 | 韌體 `rtl8761bu`。不支援 LE Audio（沒有等時通道） | 維護者；[#84](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/84)、[#87](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/87) |
| 通用 RTL8761B 接收器 | Realtek RTL8761BU | 0BDA:A729 | USB 接收器 | ✅ | 韌體 `rtl8761bu`。自 2.0.2 起會自動偵測（之前：晶片請選擇 RTL8761BU）。不支援 LE Audio | [#66](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/66)、[#86](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/86) |
| 筆電內建的 Realtek RTL8852BE | Realtek RTL8852BU | 0BDA:4853 | 內建 | ✅ | 韌體 `rtl8852bu`，自 2.0.2-beta.1 起可辨識。BlueCaster 使用期間，Windows 本身沒有藍牙可用 | [#81](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/81) |
| ASUS BT400 | Broadcom BCM20702 | 0B05:17CB | USB 接收器 | ✅ 實驗性 | 以其 ROM 韌體運作，不需要任何檔案。回報於 v1（1.0.12–1.0.17） | [#18](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/18)、[#27](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/27)、[#30](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/30) |
| 「Realtek Bluetooth 6.0」接收器 | Realtek RTL8761B | 0BDA:A760 | USB 接收器 | ✅ | 尚無法自動偵測（LMP subversion 0x8761）。不支援 LE Audio | [#208](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/208) |
| CSR8510 A10 接收器 | CSR8510 | 0A12:0001 | USB 接收器 | ✅ | 不需要韌體檔案 | [#32](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/32) |
| Intel AX211（內建 Wi-Fi / 藍牙網卡） | Intel AX211（TLV bootloader） | 8087:0033 | 內建 | ✅ 實驗性 | 韌體取自 linux-firmware `intel/`，從 bootloader 載入，需 1.5–1.7 秒：`ibt-0180-0041`（variant 0x1B）或 `ibt-1040-0041`（variant 0x19），記錄中會寫明檔案名稱。配對、AAC 與 LDAC 可用。BlueCaster 使用期間，Windows 本身沒有藍牙可用 | [#261](https://github.com/SeiyaFunaokaJP/BlueCaster/pull/261) |
| 「MA530」介面卡 | 未提供 | – | USB 接收器 | ✅ | 回報於 v1.0.9 | [#3](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/3)、[#9](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/9) |
| UGREEN 藍牙 5.4 與 6.0 接收器 | 未提供 | – | USB 接收器 | ✅ | 回報於 2.0.1 | [#11](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/11#issuecomment-6563908711) |

MediaTek 與 Qualcomm 介面卡尚無回報：對它們的預期請參閱 [建議的介面卡](setup#adapters)。

## 耳機

| 耳機 | 介面卡 | 編解碼器 | 備註 | 版本 | 來源 |
|:--|:--|:--|:--|:--|:--|
| Sony WH-1000XM4 | TP-Link UB500 | ✅ LDAC 990 kbps（最高 96 kHz） · ✅ AAC · ✅ SBC · ✅ SBC XQ 552 kbps | 此型號沒有 aptX。開啟其多點連線時，LDAC 會關閉 | 1.0 – 2.0 | 維護者 |
| Sony WH-1000XM4 | ASUS BT400 | ✅ LDAC 990 kbps、48 kHz / 24 位元 | Windows 10 22H2 | 1.0.12、1.0.15 | [#18](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/18)、[#27](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/27) |
| Sony WH-1000XM6 | 內建 RTL8852BU | ✅ LDAC 990 kbps、48 kHz / 24 位元（5 分鐘，遺失 0 個封包） | 耳機上關閉 LE Audio。第一次連線需要配對模式 | 2.0.1、2.0.2-beta.1 | [#81](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/81) |
| EarFun Air Pro 4+ | RTL8761B (0BDA:A729) | ✅ LDAC · ✅ AAC · ✅ SBC · ✅ SBC XQ · ✅ aptX · ✅ aptX Adaptive | | 2.0.2-beta.1、2.0.2-beta.6 | [#86](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/86) |
| Grado GW100x | 「MA530」 | ✅ aptX | 也提供 aptX Adaptive；1.0.9 沒有 aptX Adaptive 編碼器，因此傳送的是傳統 aptX（尚未在此耳機上試過 aptX Adaptive） | 1.0.9 | [#9](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/9) |
| LHDC V5 耳機（未提供型號） | RTL8761B | ✅ LHDC V5 1000 kbps / 192 kHz | 請在耳機的 App 中設為 192 kHz 模式；在 96 kHz 模式下，它只接受 400 kbps | 1.1.1-beta.1 | [#50](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/50) |
| OPPO Enco Free4 | Intel AX200 | ⚠️ LHDC V5 400 kbps | 即使在其 App 中開啟 Hi-Res，提供給 BlueCaster 的也最多只有 48 kHz 與 400 kbps。現在 LHDC V5 串流時（手動選擇或由自動選擇，ABR 關閉），BlueCaster 會向耳機要求一次 Hi-Res，並為此立即重新連線；尚未在此耳機上確認 | 2.0.1 | [#237](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/237) |
| OPPO Enco X4 | Intel AX211 | ⚠️ LHDC V5 400 kbps | 在其 App 中關閉 Hi-Res 時，提供給 BlueCaster 的最多只有 48 kHz 與 400 kbps。2.0.2-beta.17 只會向 Enco Free4 要求 Hi-Res；現在任何提供此設定的 OPPO 耳機都會被要求。尚未確認 | 2.0.2-beta.17 | [#237](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/237) |
| JBL Tune 510BT | TP-Link UB500 | ✅ SBC · ⚠️ AAC（無聲） | AAC 的修正（[#89](https://github.com/SeiyaFunaokaJP/BlueCaster/pull/89)）尚未在此型號上確認 | 2.0.1 | [#43](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/43)、[#84](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/84)、[#87](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/87) |
| Huawei FreeBuds Pro 4 | ASUS BT400 | ✅ AAC 256 kbps · ❓ LDAC | 在 1.0.16 上只提供 SBC 與 AAC。現在選擇 LDAC 時（自動選擇關閉），BlueCaster 會將耳機設為「音質優先」模式並重新連線，若因此仍未得到 LDAC，則把模式設回原狀；尚未在此耳機上確認。L2HC 為專有格式 | 1.0.16 | [#30](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/30) |
| Huawei FreeBuds 5i | RTL8761B (0BDA:A760) | ✅ LDAC 990 / 660 kbps · ✅ AAC 256 kbps · ✅ SBC | 只在部分連線時提供 LDAC。另外也提供 BlueCaster 不支援的華為編解碼器（0x027D/0x4C36） | 2.0.1 | [#208](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/208) |
| Huawei FreeBuds Neo | RTL8761B (0BDA:A760) | ✅ LDAC 990 kbps · ✅ AAC 256 kbps · ✅ SBC | 在 2.0.1 上只提供 SBC 與 AAC。在 2.0.2-beta.13 上，於設定檔中選擇 LDAC（自動選擇關閉）後 LDAC 即可使用。在繁忙的 2.4 GHz Wi-Fi 環境下會斷音，直到將 AFH 設為手動並指定路由器的通道為止 | 2.0.2-beta.13 | [#208](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/208)、[#30](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/30) |
| EDIFIER W820NB（雙金標） | CSR8510 A10 | ✅ SBC · ❓ LDAC | LDAC 在 1.0.16 上失敗，已於 1.0.17 修正；之後尚未確認 | 1.0.16 | [#32](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/32) |
| ROSESELSA Ceramics Ultra | UGREEN 藍牙 5.4 / 6.0 | ✅ LDAC · ⚠️ LHDC V5 · ✅ AAC · ✅ SBC | LDAC 的連線品質與位元率最穩定。LHDC V5 可能隨機斷音 | 2.0.1 | [#11](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/11#issuecomment-6563908711) |

## 測試設備

| 項目 | 配置 | 編解碼器 | 結果 |
|:--|:--|:--|:--|
| 測量用接收器 [`tools/linux_sink`](usage#receiver) | TP-Link UB500 → Ubuntu 26.04（Intel AX201），經由無線傳輸 | SBC、AAC、aptX、aptX HD、aptX LL、LDAC | ✅ 接收無遺失 |
| 虛擬連結 `tools/emu`（Bumble） | 無無線電 | 所有編解碼器，以及 LE Audio | ✅ 端對端測試通過 |

## 尚未驗證

- **實際耳機上的編解碼器**：aptX HD、aptX Low Latency、Opus、Opus 05、FastStream
- **LE Audio**：只對模擬器測試過；上述介面卡都沒有等時通道
- **ARM64 版 Windows**，以及 **搭配 v2 的 Windows 10**（v1 曾在 Windows 10 22H2 上執行：[#18](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/18)、[#30](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/30)）
