---
title: 使い方
layout: default
parent: 日本語
nav_order: 2
---

# 使い方
{: .no_toc }

## 目次
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## GUI モード（既定）

```
A2DPWB.exe
```

グラフィカルインターフェースでは以下の操作が可能です:

- **Bluetooth アダプター選択** -- 使用する WinUSB アダプターを選択
- **オーディオデバイス選択** -- WASAPI ループバックキャプチャのソースを選択
- **デバイスアドレス** -- 手動入力またはペアリング済みデバイスから選択
- **コーデック選択** -- Auto / LDAC / aptX HD / aptX LL / AAC / SBC
- **LDAC 品質** -- HQ (990 kbps) / SQ (660 kbps) / MQ (330 kbps)
- **LDAC ABR** -- 不安定な接続時のアダプティブビットレート切替
- **プロファイル管理** -- デバイス + コーデック設定の保存・読み込み
- **リアルタイムステータス** -- コーデック、ビットレート、接続状態

## CLI モード

`--cli` オプションで GUI なしで実行します。

### 基本

```bash
# 最適なコーデックを自動選択
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF
```

### コーデック選択

```bash
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac      # LDAC
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxhd    # aptX HD
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxll    # aptX Low Latency
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -c aac       # AAC
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -c sbc       # SBC
```

自動選択の優先順位: LDAC > aptX HD > aptX LL > AAC > SBC

### LDAC 品質

```bash
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q hq   # 990 kbps（既定）
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q sq   # 660 kbps
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q mq   # 330 kbps
```

### LDAC ABR（アダプティブビットレート）

```bash
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -a
```

ABR は Bluetooth 接続が不安定な場合にビットレートを自動的に下げ、安定すると引き上げます。

### デバイス検出

```bash
# ペアリング済みの Bluetooth オーディオデバイスを一覧表示
A2DPWB.exe --cli -l
```

{: .note }
デバイス一覧の取得には WinUSB アダプターではなく、**内蔵** Bluetooth アダプター経由の Windows Bluetooth API を使用します。

## キャプチャモード

A2DPWB は 2 つのオーディオキャプチャモードに対応しています:

| モード | 説明 |
|:-------|:-----|
| システムループバック | 既定の出力デバイスからシステム音声をすべてキャプチャ |
| 仮想デバイス | 特定の仮想オーディオデバイス（VB-CABLE 等）からキャプチャ（アプリ単位のルーティングに使用） |

{: .warning }
どちらのモードも WASAPI 共有モードを使用するため、キャプチャのサンプルレートは Windows サウンド設定の「既定の形式」に依存します（通常 48 kHz）。LDAC の 96 kHz を利用するには、**サウンド設定 → デバイスのプロパティ → 詳細 → 既定の形式**で出力デバイスのサンプルレートを 96 kHz に変更してください。

## コーデック比較

| コーデック | 最適な用途 | トレードオフ |
|:-----------|:-----------|:-------------|
| LDAC (HQ) | 最高音質 | レイテンシーが高い、安定した接続が必要 |
| LDAC (ABR) | 音質 + 安定性 | 接続状況に応じてビットレートが変動 |
| aptX HD | 高音質、幅広いデバイス対応 | 固定 576 kbps |
| aptX LL | ゲーム、動画（低レイテンシー） | 音質が低い (16-bit) |
| AAC | Apple デバイス、良い効率 | 中程度の音質 |
| SBC | 最大の互換性 | 最低の音質 |
