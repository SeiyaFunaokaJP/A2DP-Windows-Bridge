---
title: 已验证的硬件
layout: default
parent: 首页
nav_order: 2.5
---

# 已验证的硬件
{: .no_toc }

BlueCaster 实际使用过的适配器和耳机，来自维护者自己的测试以及用户在 issue 中的报告。✅ 可用 · ⚠️ 可用但有限制，或部分可用 · ❌ 不可用 · ❓ 未确认。

在其他硬件上试过 BlueCaster？请在 [issue](https://github.com/SeiyaFunaokaJP/BlueCaster/issues) 中告诉我们（编解码器相关请在 [#11](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/11) 中），并附上**诊断 > 复制诊断信息**的文本：其中包含适配器、编解码器和比特率。

## 目录
{: .no_toc .text-delta }

1. TOC
{:toc}

## Bluetooth 适配器

| 适配器 | 芯片 | USB ID | 类型 | 结果 | 说明 | 来源 |
|:--|:--|:--|:--|:--|:--|:--|
| TP-Link UB500 | Realtek RTL8761BU | 2357:0604 | USB 适配器 | ✅ 测试最充分 | 固件 `rtl8761bu`。不支持 LE Audio（没有同步信道） | 维护者；[#84](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/84)、[#87](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/87) |
| 通用 RTL8761B 适配器 | Realtek RTL8761BU | 0BDA:A729 | USB 适配器 | ✅ | 固件 `rtl8761bu`。自 2.0.2 起可自动检测（之前需将芯片选为 RTL8761BU）。不支持 LE Audio | [#66](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/66)、[#86](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/86) |
| 笔记本电脑内置的 Realtek RTL8852BE | Realtek RTL8852BU | 0BDA:4853 | 内置 | ✅ | 固件 `rtl8852bu`，自 2.0.2-beta.1 起可识别。BlueCaster 使用它期间，Windows 没有自己的 Bluetooth 可用 | [#81](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/81) |
| ASUS BT400 | Broadcom BCM20702 | 0B05:17CB | USB 适配器 | ✅ 实验性 | 使用其 ROM 固件运行，无需文件。报告于 v1（1.0.12–1.0.17） | [#18](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/18)、[#27](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/27)、[#30](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/30) |
| “Realtek Bluetooth 6.0”适配器 | Realtek RTL8761B | 0BDA:A760 | USB 适配器 | ✅ | 尚不能自动检测（LMP subversion 0x8761）。不支持 LE Audio | [#208](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/208) |
| CSR8510 A10 适配器 | CSR8510 | 0A12:0001 | USB 适配器 | ✅ | 无需固件文件 | [#32](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/32) |
| Intel AX211（内置 Wi-Fi / Bluetooth 网卡） | Intel AX211（TLV bootloader） | 8087:0033 | 内置 | ✅ 实验性 | 固件来自 linux-firmware 的 `intel/`，在 1.5–1.7 秒内从 bootloader 加载：`ibt-0180-0041`（variant 0x1B）或 `ibt-1040-0041`（variant 0x19），日志中会写明文件名。配对、AAC 和 LDAC 均可用。BlueCaster 使用它期间，Windows 没有自己的 Bluetooth 可用 | [#261](https://github.com/SeiyaFunaokaJP/BlueCaster/pull/261) |
| “MA530”适配器 | 未提供 | – | USB 适配器 | ✅ | 报告于 v1.0.9 | [#3](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/3)、[#9](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/9) |
| UGREEN Bluetooth 5.4 和 6.0 适配器 | 未提供 | – | USB 适配器 | ✅ | 报告于 2.0.1 | [#11](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/11#issuecomment-6563908711) |

MediaTek 和 Qualcomm 适配器尚无报告：对它们的预期请参阅[推荐适配器](setup#adapters)。

## 耳机

| 耳机 | 适配器 | 编解码器 | 说明 | 版本 | 来源 |
|:--|:--|:--|:--|:--|:--|
| Sony WH-1000XM4 | TP-Link UB500 | ✅ LDAC 990 kbps（最高 96 kHz）· ✅ AAC · ✅ SBC · ✅ SBC XQ 552 kbps | 该型号不支持 aptX。开启多点连接时不提供 LDAC | 1.0 – 2.0 | 维护者 |
| Sony WH-1000XM4 | ASUS BT400 | ✅ LDAC 990 kbps，48 kHz / 24 bit | Windows 10 22H2 | 1.0.12、1.0.15 | [#18](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/18)、[#27](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/27) |
| Sony WH-1000XM6 | 内置 RTL8852BU | ✅ LDAC 990 kbps，48 kHz / 24 bit（5 分钟，丢失 0 个数据包） | 耳机上关闭 LE Audio。首次连接需要配对模式 | 2.0.1、2.0.2-beta.1 | [#81](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/81) |
| EarFun Air Pro 4+ | RTL8761B (0BDA:A729) | ✅ LDAC · ✅ AAC · ✅ SBC · ✅ SBC XQ · ✅ aptX · ✅ aptX Adaptive | | 2.0.2-beta.1、2.0.2-beta.6 | [#86](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/86) |
| Grado GW100x | “MA530” | ✅ aptX | 也提供 aptX Adaptive；1.0.9 没有 aptX Adaptive 编码器，发送的是经典 aptX（尚未在它上面试过 aptX Adaptive） | 1.0.9 | [#9](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/9) |
| LHDC V5 耳机（未提供型号） | RTL8761B | ✅ LHDC V5 1000 kbps / 192 kHz | 请在耳机的应用中设为 192 kHz 模式；在 96 kHz 模式下只接受 400 kbps | 1.1.1-beta.1 | [#50](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/50) |
| OPPO Enco Free4 | Intel AX200 | ⚠️ LHDC V5 400 kbps | 即使在其应用中开启了 Hi-Res，它们向 BlueCaster 提供的也最多只有 48 kHz 和 400 kbps。现在 LHDC V5 串流时（手动选择或由“自动”选择，ABR 关闭），BlueCaster 会向其请求一次 Hi-Res，并为此立即重新连接一次；尚未在这款耳塞上确认 | 2.0.1 | [#237](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/237) |
| OPPO Enco X4 | Intel AX211 | ⚠️ LHDC V5 400 kbps | 在其应用中关闭 Hi-Res 时，它们向 BlueCaster 提供的最多只有 48 kHz 和 400 kbps。2.0.2-beta.17 只向 Enco Free4 请求 Hi-Res；现在会向所有提供该设置的 OPPO 耳塞请求。尚未确认 | 2.0.2-beta.17 | [#237](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/237) |
| JBL Tune 510BT | TP-Link UB500 | ✅ SBC · ⚠️ AAC（无声） | AAC 的修复（[#89](https://github.com/SeiyaFunaokaJP/BlueCaster/pull/89)）尚未在该型号上确认 | 2.0.1 | [#43](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/43)、[#84](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/84)、[#87](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/87) |
| Huawei FreeBuds Pro 4 | ASUS BT400 | ✅ AAC 256 kbps · ❓ LDAC | 在 1.0.16 上只提供 SBC 和 AAC。现在选择 LDAC（自动选择关闭）时，BlueCaster 会将其设为“音质优先”模式并重新连接，如果没有因此得到 LDAC，则将该模式改回原样；尚未在这款耳塞上确认。L2HC 为专有编解码器 | 1.0.16 | [#30](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/30) |
| Huawei FreeBuds 5i | RTL8761B (0BDA:A760) | ✅ LDAC 990 / 660 kbps · ✅ AAC 256 kbps · ✅ SBC | 只在部分连接中提供 LDAC。还提供一种 BlueCaster 不支持的 Huawei 编解码器（0x027D/0x4C36） | 2.0.1 | [#208](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/208) |
| Huawei FreeBuds Neo | RTL8761B (0BDA:A760) | ✅ LDAC 990 kbps · ✅ AAC 256 kbps · ✅ SBC | 在 2.0.1 上只提供 SBC 和 AAC。在 2.0.2-beta.13 上，在配置文件中选择 LDAC（自动选择关闭）后 LDAC 可用。在繁忙的 2.4 GHz Wi-Fi 环境下会断音，直到将 AFH 设为手动并填入路由器的信道 | 2.0.2-beta.13 | [#208](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/208)、[#30](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/30) |
| EDIFIER W820NB（双金标版） | CSR8510 A10 | ✅ SBC · ❓ LDAC | LDAC 在 1.0.16 上失败，已在 1.0.17 中修复；此后未确认 | 1.0.16 | [#32](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/32) |
| ROSESELSA Ceramics Ultra | UGREEN Bluetooth 5.4 / 6.0 | ✅ LDAC · ⚠️ LHDC V5 · ✅ AAC · ✅ SBC | LDAC 的链路质量和比特率最稳定。LHDC V5 可能随机断音 | 2.0.1 | [#11](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/11#issuecomment-6563908711) |

## 测试设备

| 项目 | 配置 | 编解码器 | 结果 |
|:--|:--|:--|:--|
| 测量用接收器 [`tools/linux_sink`](usage#receiver) | TP-Link UB500 → Ubuntu 26.04（Intel AX201），通过无线 | SBC、AAC、aptX、aptX HD、aptX LL、LDAC | ✅ 无丢失接收 |
| 虚拟链路 `tools/emu`（Bumble） | 无无线电 | 所有编解码器以及 LE Audio | ✅ 端到端测试通过 |

## 尚未验证

- **真实耳机上的编解码器**：aptX HD、aptX Low Latency、Opus、Opus 05、FastStream
- **LE Audio**：只针对模拟器测试过；上述适配器都没有同步信道
- **ARM64 上的 Windows**，以及 **Windows 10 上的 v2**（v1 曾在 Windows 10 22H2 上运行：[#18](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/18)、[#30](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/30)）
