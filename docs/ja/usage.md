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
- **コーデック選択** -- Auto / LDAC / aptX HD / aptX LL / aptX / SBC / AAC
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
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -c aptx      # aptX
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -c aac       # AAC
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF -c sbc       # SBC
```

自動選択の優先順位: LDAC > aptX HD > aptX LL > aptX > AAC > SBC

指定したコーデックをヘッドホンが提供していない場合、CLI は Auto の優先順位でフォールバックします。aptX Adaptive は選択されません -- [aptX ファミリーと aptX Adaptive の互換性](#aptx-compatibility)を参照してください。

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
| aptX | aptX 対応機器が最も多い | 16-bit、352/384 kbps |
| AAC | Apple デバイス、良い効率 | 中程度の音質 |
| SBC | 最大の互換性 | 最低の音質 |

## aptX ファミリーと aptX Adaptive の互換性 {#aptx-compatibility}

A2DPWB は 3 種類の aptX コーデックに対応しており、いずれも [libopenaptx](https://github.com/pali/libopenaptx) でエンコードします。aptX Adaptive には**対応していません**。

| コーデック | Vendor ID / Codec ID | RTP ヘッダー | 対応状況 |
|:-----------|:---------------------|:-------------|:---------|
| aptX | 0x0000004F / 0x0001 | なし | 対応（44.1/48 kHz で 352/384 kbps） |
| aptX HD | 0x000000D7 / 0x0024 | あり | 対応（576 kbps、24-bit 入力） |
| aptX Low Latency | 0x0000000A または 0x000000D7 / 0x0002 | なし | 対応（352 kbps） |
| aptX Adaptive | 0x000000D7 / 0x00AD | -- | **非対応** -- 検出してログに記録するのみ |

### aptX Adaptive に対応していない理由

オープンソースの aptX Adaptive エンコーダーは存在しません。libopenaptx が実装しているのは aptX と aptX HD のみです（A2DPWB は aptX Low Latency にも使用しています）。[arkq/openaptx](https://github.com/arkq/openaptx) も aptX / aptX HD を再実装しているだけで aptX Adaptive エンコーダーは含まれておらず、バイナリの再配布も認められていません。そのため A2DPWB は aptX Adaptive を検出してログに記録するだけです。

### aptX Adaptive から aptX への「フォールバック」ではありません

接続時、ヘッドホンは対応コーデックの**一覧**（AVDTP ストリームエンドポイント）を通知し、A2DPWB はその中から 1 つを選びます。aptX Adaptive は最初から選択候補に入らないため、そこから「フォールバック」するわけではありません。

- **ヘッドホンがクラシック aptX も通知している場合** -- Qualcomm は aptX Adaptive を aptX / aptX HD と下位互換があると案内しているため、多くの aptX Adaptive 対応ヘッドホンがこれに該当します。この場合 A2DPWB はクラシック aptX を直接選択します。aptX はヘッドホンの一覧にある独立したコーデックであり、aptX Adaptive の縮退モードではありません。
- **ヘッドホンが aptX Adaptive のみを通知している場合**（クラシック aptX・aptX HD・aptX LL がない） -- A2DPWB では aptX を使えません。Auto は優先順位に従って次のコーデック（通常は AAC か SBC）を選びます。

自動選択の優先順位: LDAC > aptX HD > aptX LL > aptX > AAC > SBC

### 指定したコーデックがない場合の GUI と CLI の違い

ヘッドホンが通知していないコーデックを明示的に指定した場合、GUI と CLI で動作が異なります。

| モード | 動作 |
|:-------|:-----|
| GUI（Auto 以外のコーデックを指定） | エラー（「デバイスは ... に対応していません」）で接続を中止します。aptX・aptX HD・aptX LL を指定し、ヘッドホンが aptX Adaptive しか提供していない場合は、その旨を表示し Auto・AAC・SBC の使用を案内します。 |
| CLI（`-c <codec>`） | `Requested codec ... not available, falling back...` と表示し、Auto の優先順位で選択を続けます。ヘッドホンが aptX Adaptive を提供している場合は、それが選択されないことを示す行も表示します。 |

### サンプルレート

aptX・aptX HD・aptX LL が対応するのは 44.1 kHz と 48 kHz のみです。A2DPWB はヘッドホンが通知したサンプルレートから 1 つを選び（現在の Windows デバイスのレートを優先し、使えなければ 48 kHz）、WASAPI にリサンプリングさせます。aptX のために Windows のサウンド設定を変更する必要はありません。

### ヘッドホンが提供するコーデックの確認方法

1. **設定**で**デバッグモード（debug.logを出力）**を有効にし、A2DPWB を再起動します。
2. ヘッドホンに接続します。
3. 設定フォルダー（`%APPDATA%\A2DPWB`）の `debug.log` を開きます。CLI モードでは同じ内容が標準エラー出力に書き出されます。

以下の行で、ヘッドホンが通知しているコーデックを正確に確認できます。

```
BTstack: Remote supports aptX (SEID=..., caps=0x..)
BTstack: Remote supports aptX Adaptive (SEID=...) — never selected (no open encoder); classic aptX is used only if the remote lists it
BTstack: Remote vendor codec vid=0x........ cid=0x.... (SEID=...) — unsupported
BTstack: Capability discovery complete (LDAC=0, aptXHD=0, aptXLL=0, aptX=1, aptXAdaptive=1 [not encodable], SBC=1, AAC=1)
```

- `aptX=1` -- クラシック aptX を使用できます（`aptXAdaptive=1` であっても同様）。
- `aptX=0`・`aptXHD=0`・`aptXLL=0` かつ `aptXAdaptive=1` -- ヘッドホンは aptX Adaptive しか提供していません。Auto・AAC・SBC を使ってください。
- `Remote vendor codec ... — unsupported` -- A2DPWB が実装していないその他のベンダーコーデックです。

## トラブルシューティング

### アップデート後にペアリングがうまくいかない

A2DPWB は SSP（Just Works）と **General Bonding** でペアリングし、リンクキーを保存するため、次回以降はペアリングし直さずに再接続できます。以前のバージョンの A2DPWB でペアリングしていた場合は、ヘッドホン側で A2DPWB のペアリング情報を削除し（ペアリング履歴の消去方法はヘッドホンの説明書を参照）、改めてペアリングしてください。

### 接続がタイムアウトする

A2DPWB は SDP レコード（A2DP Source、AVRCP Controller / Target）を登録し、ヘッドホンがオーディオソースとして認識できるようにしています。接続がタイムアウトした場合は中途半端な接続が破棄されるため、A2DPWB を再起動せずにそのまま再試行できます。ヘッドホンの電源が入っていること、通信範囲内にあること、他の機器に接続されていないことを確認してください。
