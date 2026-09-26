---
title: 日本語
layout: default
nav_order: 7
has_children: true
---

# A2DP Windows Bridge (A2DPWB)

Windows 向け Bluetooth A2DP オーディオストリーミングツール（全コーデック対応）
{: .fs-6 .fw-300 }

USB Bluetooth アダプターを WinUSB モードで使用し、**LDAC、aptX HD、aptX Low Latency、aptX、AAC、SBC** でシステム音声をストリーミングします。カーネルドライバーやテスト署名は不要です。

[セットアップ](setup){: .btn .btn-primary .fs-5 .mb-4 .mb-md-0 .mr-2 }
[GitHub](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge){: .btn .fs-5 .mb-4 .mb-md-0 }

---

## 対応コーデック

| コーデック | ビットレート | サンプルレート | ビット深度 | レイテンシー |
|:-----------|:-------------|:---------------|:-----------|:-------------|
| LDAC | 330/660/990 kbps | 44.1--96 kHz | 16/24/32 bit | 約 200 ms |
| aptX HD | 576 kbps | 44.1/48 kHz | 24 bit | 約 150 ms |
| aptX Low Latency | 352 kbps | 44.1/48 kHz | 16 bit | 約 32 ms |
| aptX | 352/384 kbps | 44.1/48 kHz | 16 bit | -- |
| AAC | 128/192/256 kbps | 44.1/48 kHz | 16 bit | 約 150 ms |
| SBC | 最大約 345 kbps | 44.1/48 kHz | 16 bit | 約 150 ms |

### プラットフォーム別のコーデック対応（ヘッドホンへの送信）

| コーデック | Windows 10 標準 | Windows 11 標準 | Ubuntu 標準（PipeWire） | **A2DPWB**（Windows 10 / 11） |
|:-----------|:---------------:|:---------------:|:-----------------------:|:-----------------------------:|
| LDAC | ❌ | ❌ | ✅ | ✅ |
| aptX HD | ❌ | ❌ | ✅ | 🧪 実験的 |
| aptX Low Latency | ❌ | ❌ | ✅ | 🧪 実験的 |
| aptX | ✅ | ✅ | ✅ | 🧪 実験的 |
| AAC | ✅ | ✅ | ❌ Ubuntu のパッケージに含まれない | ✅ |
| SBC | ✅ | ✅ | ✅ | ✅ |
| aptX Adaptive | ❌ | ❌ | ❌ | ❌ |

✅ 対応 · 🧪 実験的 · ❌ 非対応。どの列も送信側（PC → ヘッドホン）です。「標準」は A2DPWB を使わない OS 自身の Bluetooth オーディオで、Ubuntu の列は Ubuntu 26.04 の PipeWire（`libspa-0.2-bluetooth`）が持つコーデックです。

{: .warning }
**aptX・aptX HD・aptX Low Latency は実験的な対応です。** Android / PipeWire の実装に合わせ、エンコード→デコードの往復テストは通っていますが、実機のヘッドホンでの検証はまだです。表のレイテンシーは一般的な目安で、A2DPWB で測定した値ではありません。

{: .note }
**クラシック aptX だけが目的なら**、Windows 10 標準の Bluetooth スタックがクラシック aptX に対応しています（aptX HD・aptX LL・aptX Adaptive は非対応）。その場合 A2DPWB は不要です。A2DPWB が主に役立つのは LDAC・aptX HD・aptX Low Latency です。

{: .note }
**aptX Adaptive には対応していません**（オープンソースのエンコーダーが存在しないため）。ヘッドホンがクラシック aptX も通知していれば A2DPWB は aptX を直接使用し、そうでなければ Auto は AAC か SBC を選びます。詳しくは [aptX ファミリーと aptX Adaptive の互換性](usage#aptx-compatibility)を参照してください。

## 対応プラットフォーム {#platform-support}

| コンポーネント | Windows 10 (x64) | Windows 11 (x64) | Linux (Ubuntu) |
|:---------------|:----------------:|:----------------:|:--------------:|
| **A2DPWB**（GUI / CLI） | ✅（実機では未確認） | ✅ | ❌ |
| [ソースからのビルド](building)（Visual Studio 2022 以降） | ✅ | ✅ | ❌ |
| `a2dpwb_decode`（ストリーム検査ツール） | ✅ | ✅ | ❌ |
| エンドツーエンドテスト `tools/emu/run_test.py` | ✅ | ✅ | ❌ |
| 仮想アダプター `tools/emu/emu_vhci.py` | ❌ | ❌ | ✅ |
| [測定用受信機](usage#receiver) `tools/linux_sink` | ❌ | ❌ | ✅ |

A2DPWB 本体は Windows 専用で、x64 ビルドのみです。Linux 側のツールには Python 3.11 以降が必要です（Ubuntu 26.04 で確認済み）。`setup.sh` は Ubuntu / Debian 向けで、他のディストリビューションではパッケージを手動でインストールしてください。

## 検証済みの機器 {#verified-hardware}

| 対象 | 構成 | コーデック | 結果 |
|:-----|:-----|:-----------|:-----|
| Sony WH-1000XM4（ヘッドホン） | Windows 11 + TP-Link UB500 | LDAC、AAC、SBC | ✅ 再生できる（XM4 は aptX 非対応） |
| TP-Link UB500（Realtek RTL8761BU、アダプター） | Windows 11、WinUSB、linux-firmware の `rtl8761bu` | – | ✅ |
| 測定用受信機 `tools/linux_sink` | Windows 11 + UB500 → Ubuntu 26.04（実際の無線） | SBC、AAC、aptX、aptX HD、aptX LL、LDAC | ✅ 受信・測定できる |
| 仮想リンク `tools/emu`（Bumble） | Windows 11、無線なし | SBC、AAC、aptX、aptX HD、aptX LL、LDAC | ✅ エンドツーエンドテストに合格 |

Windows 10 の実機ではまだ試していません。その他のアダプターは[推奨アダプター](setup#adapters)を参照してください。

## オーディオキャプチャモード

A2DPWB は WASAPI でオーディオをキャプチャし、2 つのモードを提供します。

| モード | 説明 | 用途 |
|:-------|:-----|:-----|
| **システムループバック** | WASAPI ループバックで既定の再生デバイスから全システム音声をキャプチャ | シンプルな構成 -- すべての音声がストリーミングされる |
| **仮想デバイス** | ユーザーが選択した仮想オーディオデバイス（VB-CABLE、VoiceMeeter 等）からキャプチャ | 特定のアプリだけ Bluetooth に送り、他の音声はスピーカーに出力 |

**仮想デバイス**モードでは、Windows の既定の再生デバイスを選択した仮想デバイスに切り替え、そのループバック出力をキャプチャします。仮想デバイスに出力するアプリの音声が Bluetooth でストリーミングされます。

## 動作の仕組み

```
オーディオソース
  |
  +-- システムループバック: 既定の再生デバイス（全システム音声）
  +-- 仮想デバイス:  選択した仮想オーディオデバイス（アプリ別ルーティング）
  |
  v
WASAPI ループバックキャプチャ (PCM)
  |
  v
オーディオエンコーダー (LDAC / aptX HD / aptX LL / aptX / AAC / SBC)
  |
  v
BTstack (A2DP Source -> AVDTP -> L2CAP -> HCI)
  |
  v
WinUSB -> USB Bluetooth アダプター -> ヘッドホン
```

A2DPWB は Windows の Bluetooth スタックを完全にバイパスします。[BTstack](https://github.com/bluekitchen/btstack) を使用して WinUSB 経由で USB Bluetooth アダプターと直接通信し、Bluetooth プロトコルスタック全体をユーザーモードで実装します。

## 主な機能

- **マルチコーデック**: LDAC、aptX HD、aptX Low Latency、aptX、AAC、SBC（自動ネゴシエーション対応）
- **2 つのキャプチャモード**: システムループバックまたは仮想オーディオデバイスルーティング
- **LDAC ABR**: 不安定な接続時のアダプティブビットレート
- **自動再接続**: 切断時に自動再接続（最大 10 回）
- **通信品質と AFH**: 送信内容・RSSI・使用中のチャネルを表示し、避けるべき Wi-Fi チャネルをアダプターに伝えます（[使い方](usage#link-quality)を参照）
- **プロファイル管理**: デバイス + コーデック設定の保存・読み込み
- **GUI + CLI**: wxWidgets グラフィカルインターフェースまたはコマンドライン操作
- **多言語対応**: 英語 / 日本語
- **Realtek ファームウェア**: Realtek アダプター向けガイド付きファームウェアダウンロード
- **診断**（デバッグモード）: デバッグコンソール、HCI キャプチャ、Linux PC を使った対向受信テスト（[使い方](usage#receiver)を参照）
