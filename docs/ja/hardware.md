---
title: 検証済みの機器
layout: default
parent: ホーム
nav_order: 2.5
---

# 検証済みの機器
{: .no_toc }

A2DPWB で使われたことのあるアダプターとヘッドホンです。メンテナー自身のテストと、issue でのユーザーからの報告をもとにしています。✅ 動作 · ⚠️ 制限付き、または一部だけ動作 · ❌ 動作しない · ❓ 未確認。

ほかの機器で A2DPWB を試しましたか？ **診断 > 診断情報をコピー** のテキストを添えて、[issue](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues)（コーデックについては [#11](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/11)）で教えてください。このテキストにはアダプター、コーデック、ビットレートが入っています。

## 目次
{: .no_toc .text-delta }

1. TOC
{:toc}

## Bluetooth アダプター {#bluetooth-adapters}

| アダプター | チップ | USB ID | 種類 | 結果 | 備考 | 出典 |
|:--|:--|:--|:--|:--|:--|:--|
| TP-Link UB500 | Realtek RTL8761BU | 2357:0604 | USB ドングル | ✅ 最も検証済み | ファームウェア `rtl8761bu`。LE Audio は不可（アイソクロナスチャネルなし） | メンテナー、[#84](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/84)、[#87](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/87) |
| 汎用 RTL8761B ドングル | Realtek RTL8761BU | 0BDA:A729 | USB ドングル | ✅ | ファームウェア `rtl8761bu`。2.0.2 から自動で認識（それより前はチップに RTL8761BU を選ぶ）。LE Audio は不可 | [#66](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/66)、[#86](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/86) |
| ノート PC 内蔵の Realtek RTL8852BE | Realtek RTL8852BU | 0BDA:4853 | 内蔵 | ✅ | ファームウェア `rtl8852bu`。2.0.2-beta.1 から認識。A2DPWB が使っている間、Windows 自身の Bluetooth はなくなる | [#81](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/81) |
| ASUS BT400 | Broadcom BCM20702 | 0B05:17CB | USB ドングル | ✅ 試験的 | ROM のファームウェアで動き、ファイルは不要。v1（1.0.12–1.0.17）での報告 | [#18](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/18)、[#27](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/27)、[#30](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/30) |
| 「Realtek Bluetooth 6.0」ドングル | Realtek RTL8761B | 0BDA:A760 | USB ドングル | ✅ | まだ自動では認識しない（LMP サブバージョン 0x8761）。LE Audio は不可 | [#208](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/208) |
| CSR8510 A10 ドングル | CSR8510 | 0A12:0001 | USB ドングル | ✅ | ファームウェアのファイルは不要 | [#32](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/32) |
| Intel AX211（内蔵の Wi-Fi / Bluetooth カード） | Intel AX211（TLV ブートローダー） | 8087:0033 | 内蔵 | ✅ 試験的 | ファームウェアは linux-firmware の `intel/` から、ブートローダーから 1.5–1.7 秒で読み込む: `ibt-0180-0041`（バリアント 0x1B）または `ibt-1040-0041`（バリアント 0x19）。ファイル名はログに出る。ペアリング、AAC、LDAC が動作。A2DPWB が使っている間、Windows 自身の Bluetooth はなくなる | [#261](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/pull/261), [#237](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/237) |
| Intel AX200（内蔵の Wi-Fi / Bluetooth カード） | Intel AX200 | 8087:0029 | 内蔵 | ✅ 試験的 | 報告の時点でファームウェアがすでに動いていたので、ファイルは読み込まなかった。LHDC V5 1000 kbps / 192 kHz が動作。A2DPWB が使っている間、Windows 自身の Bluetooth はなくなる | [#237](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/237) |
| 「MA530」アダプター | 不明 | – | USB ドングル | ✅ | v1.0.9 での報告 | [#3](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/3)、[#9](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/9) |
| UGREEN Bluetooth 5.4 / 6.0 ドングル | 不明 | – | USB ドングル | ✅ | 2.0.1 での報告 | [#11](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/11#issuecomment-6563908711) |

MediaTek と Qualcomm のアダプターはまだ報告がありません。どう動くと見込まれるかは [推奨アダプター](setup#adapters) を参照してください。

## ヘッドホン {#headphones}

| ヘッドホン | アダプター | コーデック | 備考 | バージョン | 出典 |
|:--|:--|:--|:--|:--|:--|
| Sony WH-1000XM4 | TP-Link UB500 | ✅ LDAC 990 kbps（最大 96 kHz） · ✅ AAC · ✅ SBC · ✅ SBC XQ 552 kbps | この機種に aptX はない。ヘッドホンのマルチポイントがオンの間は LDAC がオフになる | 1.0 – 2.0 | メンテナー |
| Sony WH-1000XM4 | ASUS BT400 | ✅ LDAC 990 kbps、48 kHz / 24 bit | Windows 10 22H2 | 1.0.12、1.0.15 | [#18](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/18)、[#27](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/27) |
| Sony WH-1000XM6 | 内蔵 RTL8852BU | ✅ LDAC 990 kbps、48 kHz / 24 bit（5 分間、パケット損失 0） | ヘッドホンの LE Audio はオフ。最初の接続にはペアリングモードが必要 | 2.0.1、2.0.2-beta.1 | [#81](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/81) |
| EarFun Air Pro 4+ | RTL8761B（0BDA:A729） | ✅ LDAC · ✅ AAC · ✅ SBC · ✅ SBC XQ · ✅ aptX · ✅ aptX Adaptive | | 2.0.2-beta.1、2.0.2-beta.6 | [#86](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/86) |
| Grado GW100x | 「MA530」 | ✅ aptX | aptX Adaptive も提示する。1.0.9 には aptX Adaptive のエンコーダーがなく、クラシック aptX を送っていた（aptX Adaptive はこの機種ではまだ試していない） | 1.0.9 | [#9](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/9) |
| LHDC V5 対応ヘッドホン（機種不明） | RTL8761B | ✅ LHDC V5 1000 kbps / 192 kHz | ヘッドホンのアプリで 192 kHz モードにする。96 kHz モードでは 400 kbps しか受け付けない | 1.1.1-beta.1 | [#50](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/50) |
| OPPO Enco Free4 | Intel AX200 | ✅ LHDC V5 1000 kbps / 192 kHz（10 分間、パケットの損失 0） | Hi-Res でないときは、アプリで Hi-Res をオンにしていても最大 48 kHz、400 kbps しか提示しない。A2DPWB が Hi-Res を求めて接続し直すと、最大 192 kHz、ビットレートの制限なしを提示する。2.0.2-beta.18 の一度だけの要求は、このイヤホンではまだ確認していない | 2.0.2-beta.17 | [#237](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/237) |
| OPPO Enco X4 | Intel AX211 | ⚠️ LHDC V5 1000 kbps | アプリで Hi-Res をオンにした状態で最大 192 kHz、ビットレートの制限なしを提示したので、A2DPWB が求める必要はなかった。このリンクでは 1000 kbps でパケットが落ちた（79 チャンネル中 20 が使用可能、近くの Wi-Fi が原因とみられる）。中音質 500 kbps を長時間使っての確認はまだ | 2.0.2-beta.18 | [#237](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/237) |
| JBL Tune 510BT | TP-Link UB500 | ✅ SBC · ⚠️ AAC（無音） | AAC の修正（[#89](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/pull/89)）はこの機種ではまだ確認していない | 2.0.1 | [#43](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/43)、[#84](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/84)、[#87](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/87) |
| Huawei FreeBuds Pro 4 | ASUS BT400 | ✅ AAC 256 kbps · ❓ LDAC | 1.0.16 では SBC と AAC しか提示しなかった。LDAC を選ぶと（自動選択はオフ）、A2DPWB は今ではイヤホンの「音質優先」モードを設定して接続し直し、それで LDAC にならなければモードを元に戻す。このイヤホンではまだ確認していない。L2HC は独自仕様 | 1.0.16 | [#30](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/30) |
| Huawei FreeBuds 5i | RTL8761B（0BDA:A760） | ✅ LDAC 990 / 660 kbps · ✅ AAC 256 kbps · ✅ SBC | LDAC を提示したのは一部の接続だけ。A2DPWB が対応していない Huawei のコーデック（0x027D/0x4C36）も提示する | 2.0.1 | [#208](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/208) |
| Huawei FreeBuds Neo | RTL8761B（0BDA:A760） | ✅ LDAC 990 kbps · ✅ AAC 256 kbps · ✅ SBC | 2.0.1 では SBC と AAC しか提示しなかった。2.0.2-beta.13 では、プロファイルで LDAC を選べば（自動選択はオフ）LDAC が動作。混雑した 2.4 GHz の Wi-Fi では、AFH をルーターのチャネルに合わせて手動にするまで途切れた | 2.0.2-beta.13 | [#208](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/208)、[#30](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/30) |
| EDIFIER W820NB（デュアルゴールドラベル） | CSR8510 A10 | ✅ SBC · ❓ LDAC | 1.0.16 で LDAC が失敗し、1.0.17 で修正。その後は未確認 | 1.0.16 | [#32](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/32) |
| ROSESELSA Ceramics Ultra | UGREEN Bluetooth 5.4 / 6.0 | ✅ LDAC · ⚠️ LHDC V5 · ✅ AAC · ✅ SBC | 通信品質とビットレートが最も安定していたのは LDAC。LHDC V5 は不規則に途切れることがある | 2.0.1 | [#11](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/11#issuecomment-6563908711) |

## テスト用の機材 {#test-equipment}

| 機材 | 構成 | コーデック | 結果 |
|:--|:--|:--|:--|
| 測定用受信機 [`tools/linux_sink`](usage#receiver) | TP-Link UB500 → Ubuntu 26.04（Intel AX201）、無線経由 | SBC、AAC、aptX、aptX HD、aptX LL、LDAC | ✅ 欠落なく受信 |
| 仮想リンク `tools/emu`（Bumble） | 無線なし | すべてのコーデックと LE Audio | ✅ エンドツーエンドテストに合格 |

## まだ検証していないもの {#not-verified-yet}

- **実際のヘッドホンでのコーデック**: aptX HD、aptX Low Latency、Opus、Opus 05、FastStream
- **LE Audio**: エミュレーターでしかテストしていません。上記のアダプターにはどれもアイソクロナスチャネルがありません
- **ARM64 版 Windows** と **v2 での Windows 10**（v1 は Windows 10 22H2 で動きました: [#18](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/18)、[#30](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/30)）
