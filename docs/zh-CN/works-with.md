---
title: 配合使用的工具
layout: default
parent: 首页
nav_order: 2.6
---

# 配合使用的工具
{: .no_toc }

可与 BlueCaster 配合使用的第三方应用和工具，由其作者或用户在 issue 中告知。

{: .warning }
> 这些**不属于 BlueCaster**，并非由 BlueCaster 项目制作、测试或支持。列在这里只是因为有人报告它们可以与 BlueCaster 配合使用。如果某个工具无法使用、在任一程序更新后停止工作或引发任何问题，我们概不负责。请查阅各项目自己的许可证和文档，工具的问题请先向该工具的项目报告。

## 目录
{: .no_toc .text-delta }

1. TOC
{:toc}

## 应用和工具

| 工具 | 与 BlueCaster 配合的功能 | 要求 | 许可证 | 报告者 |
|:--|:--|:--|:--|:--|
| [EncoDeck](https://github.com/TIANLI0/EncoDeck) | 适用于 OPPO、Sony、HUAWEI、Xiaomi 等品牌的耳塞和耳机控制（电量、降噪、EQ、手势）。作为第二个客户端连接到核心的管道：显示串流状态和编解码器，启动和停止配置文件，设置 AVRCP 音量，并协助分配 WinUSB。通过 BlueCaster 自己的 Bluetooth 链路进行的耳塞设置使用 RFCOMM 直通 | EncoDeck 1.11.0 或更高版本。RFCOMM 直通：BlueCaster 2.0.2-beta.10 或更高版本（目前只在模拟器中试过） | GPL-3.0 | 其作者，[#194](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/194) |

## 添加工具

制作或发现了可与 BlueCaster 配合使用的工具？请在 [issue](https://github.com/SeiyaFunaokaJP/BlueCaster/issues) 中告诉我们它的名称、链接、许可证、它与 BlueCaster 配合做什么以及你试过的版本。列在这里并不代表评测或认可。

## 致工具作者

工具与核心通信所用的是与 BlueCaster 窗口相同的、每个用户独有的命名管道。协议、供工具使用的方法、RFCOMM 直通以及兼容性规则（功能检查、3.0.0 中管道的改名）请参阅 **[集成（IPC）](integration)**。在 2.0.2 的 beta 期间，协议和核心仍可能发生变化。
