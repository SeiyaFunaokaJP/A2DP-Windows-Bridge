---
title: 日本語
layout: default
nav_order: 7
has_children: true
---

# A2DP Windows Bridge (A2DPWB)

Windows 向け Bluetooth A2DP オーディオストリーミングツール（全コーデック対応）
{: .fs-6 .fw-300 }

USB Bluetooth アダプターを WinUSB モードで使用し、**LDAC、aptX HD、aptX Low Latency、AAC、SBC** でシステム音声をストリーミングします。カーネルドライバーやテスト署名は不要です。

[セットアップ](setup){: .btn .btn-primary .fs-5 .mb-4 .mb-md-0 .mr-2 }
[GitHub](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge){: .btn .fs-5 .mb-4 .mb-md-0 }

---

## 対応コーデック

| コーデック | ビットレート | サンプルレート | ビット深度 | レイテンシー |
|:-----------|:-------------|:---------------|:-----------|:-------------|
| LDAC | 330/660/990 kbps | 44.1--96 kHz | 16/24/32 bit | 約 200 ms |
| aptX HD | 576 kbps | 44.1/48 kHz | 24 bit | 約 150 ms |
| aptX Low Latency | 352 kbps | 44.1/48 kHz | 16 bit | 約 32 ms |
| AAC | 128/192/256 kbps | 44.1/48 kHz | 16 bit | 約 150 ms |
| SBC | 最大約 345 kbps | 44.1/48 kHz | 16 bit | 約 150 ms |

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
オーディオエンコーダー (LDAC / aptX HD / aptX LL / AAC / SBC)
  |
  v
BTstack (A2DP Source -> AVDTP -> L2CAP -> HCI)
  |
  v
WinUSB -> USB Bluetooth アダプター -> ヘッドホン
```

A2DPWB は Windows の Bluetooth スタックを完全にバイパスします。[BTstack](https://github.com/bluekitchen/btstack) を使用して WinUSB 経由で USB Bluetooth アダプターと直接通信し、Bluetooth プロトコルスタック全体をユーザーモードで実装します。

## 主な機能

- **マルチコーデック**: LDAC、aptX HD、aptX Low Latency、AAC、SBC（自動ネゴシエーション対応）
- **2 つのキャプチャモード**: システムループバックまたは仮想オーディオデバイスルーティング
- **LDAC ABR**: 不安定な接続時のアダプティブビットレート
- **自動再接続**: 切断時に自動再接続（最大 10 回）
- **プロファイル管理**: デバイス + コーデック設定の保存・読み込み
- **GUI + CLI**: wxWidgets グラフィカルインターフェースまたはコマンドライン操作
- **多言語対応**: 英語 / 日本語
- **Realtek ファームウェア**: Realtek アダプター向けガイド付きファームウェアダウンロード
