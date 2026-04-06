---
title: アーキテクチャ
layout: default
parent: 日本語
nav_order: 4
---

# アーキテクチャ
{: .no_toc }

## 目次
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## 概要

A2DPWB (A2DP Windows Bridge) は Windows で LDAC、aptX HD、aptX Low Latency、AAC、SBC の Bluetooth オーディオを実現します。Windows は Bluetooth A2DP で SBC と AAC のみネイティブ対応ですが、このツールはカーネルドライバーなしで高音質コーデックを追加します。

**BTstack + WinUSB** を使用 -- 完全にユーザーモードで動作し、ドライバー署名は不要です。

## トランスポート: BTstack + WinUSB

Windows の Bluetooth スタックを完全にバイパスし、WinUSB（Microsoft 署名済み汎用 USB ドライバー）経由で USB Bluetooth アダプターと直接通信します。オープンソースの Bluetooth スタックである BTstack が HCI、L2CAP、AVDTP、A2DP をユーザーモードで実装します。

**利点**:
- ドライバー署名コスト不要
- テスト署名モード不要
- Secure Boot の無効化不要
- すべてのコードがユーザーモードで動作（デバッグが容易）

**トレードオフ**:
- 専用の USB Bluetooth アダプターが必要（Windows 内蔵 Bluetooth とは別）
- WinUSB モードのアダプターは Windows の通常の Bluetooth として使用不可
- デバイスアドレスは手動入力か、内蔵アダプター経由で取得した Windows ペアリング済みデバイス一覧から選択

**動作の流れ**:
1. Zadig で USB Bluetooth アダプターに WinUSB ドライバーをインストール
2. BTstack が WinUSB API 経由で USB デバイスを開く
3. BTstack が HCI コマンドで Bluetooth コントローラーを初期化
4. (Realtek アダプター) 必要に応じてファームウェアをアップロード
5. BTstack が ACL 接続を確立し、L2CAP チャネル (PSM 0x0019) を開く
6. AVDTP シグナリングでリモート SEP を検出し、コーデックをネゴシエーション
7. エンコード済み音声が L2CAP 経由の AVDTP メディアパケットとして送信

## モジュール構成

```
A2DPWB.exe
├── GUI レイヤー (wxWidgets)
│   ├── wx_app              アプリエントリーポイント、イベントループ
│   ├── wx_main_frame       メインウィンドウ（デバイス、コーデック、ステータス）
│   ├── wx_profile_dialog   接続プロファイル管理
│   ├── wx_settings_dialog  アプリケーション設定
│   ├── wx_firmware_dialog  Realtek ファームウェアダウンロード
│   ├── wx_about_dialog     バージョン情報 / ライセンス
│   ├── wx_zadig_dialog     Zadig WinUSB インストールガイド
│   ├── theme_manager       ライト / ダークテーマ対応
│   └── localization        多言語化 (en, ja -- 埋め込み JSON)
│
├── コアレイヤー
│   ├── a2dp_service        A2DP 接続ライフサイクル & ステートマシン
│   ├── btstack_transport   BTstack 統合 (HCI, L2CAP, AVDTP, A2DP)
│   ├── wasapi_capture      WASAPI ループバックオーディオキャプチャ
│   ├── audio_encoder       エンコーダーインターフェース（抽象基底）
│   │   ├── ldac_encoder        LDAC (libldac, ABR 対応)
│   │   ├── aptxhd_encoder      aptX HD (libopenaptx)
│   │   ├── aptxll_encoder      aptX Low Latency (libopenaptx)
│   │   ├── aac_encoder         AAC-LC (fdk-aac, LATM トランスポート)
│   │   └── a2dp_sbc_encoder    SBC (BTstack Bluedroid)
│   │
│   ├── bt_device           Bluetooth デバイス情報（アドレス、名前、コーデック）
│   ├── bt_adapter_enum     USB Bluetooth アダプター列挙 (WinUSB)
│   ├── audio_device_enum   WASAPI オーディオデバイス列挙
│   └── profile_manager     接続プロファイル永続化 (JSON)
│
├── サポート
│   ├── app_settings        永続アプリケーション設定 (JSON)
│   ├── config_path         設定ファイルパス解決
│   ├── system_integration  システムトレイ、自動起動
│   ├── debug_log           デバッグログマクロ
│   └── capture_mode        オーディオキャプチャモード定義
│
└── CLI モード
    └── main.cpp            CLI 引数パース、ヘッドレスストリーミング
```

## データフロー

```
システム音声出力
       │
       ▼
 WASAPI ループバックキャプチャ (PCM 16-bit, 44.1/48 kHz)
       │
       ▼
 オーディオエンコーダー (LDAC / aptX HD / aptX LL / AAC / SBC)
       │
       ▼
 A2DP Service → BtStackTransport::send_media()
       │
       ▼
 BTstack A2DP Source → AVDTP → L2CAP → HCI
       │
       ▼
 WinUSB → USB Bluetooth アダプター → Bluetooth 無線
       │
       ▼
 ヘッドホン / スピーカー
```

## 主要モジュール

### A2DP Service (`a2dp_service.cpp`)

接続ライフサイクルの中央管理:
- コーデックネゴシエーション（自動選択またはユーザー指定）
- 全対応コーデックのストリームエンドポイント登録
- 接続ステートマシン（idle → connecting → streaming → disconnecting）
- 予期しない切断時の自動再接続ロジック
- コーデック固有のフレーミングによるメディアパケット送信

### BTstack Transport (`btstack_transport.cpp`)

アプリケーションの同期モデルと BTstack のイベント駆動 API のブリッジ。

**役割**:
- WinUSB HCI トランスポートで BTstack を初期化
- 専用スレッドで BTstack イベントループを実行
- ベンダーコーデックストリームエンドポイント (LDAC, aptX HD, aptX LL) を登録
- 非同期→同期ラッパーで A2DP 接続ライフサイクルを管理
- SSP ペアリング（Just Works モード）を処理
- WASAPI コールバック向けスレッドセーフなメディア送信を提供
- Realtek チップセットファームウェアロード

**使用する主な BTstack API**:
- `a2dp_source_create_stream_endpoint()` -- コーデックエンドポイント登録
- `a2dp_source_establish_stream()` -- A2DP シンクに接続
- `a2dp_source_set_config_other()` -- ベンダー固有コーデック設定
- `a2dp_source_stream_send_media_payload_rtp()` -- エンコード済み音声送信

### WASAPI Capture (`wasapi_capture.cpp`)

Windows Audio Session API を使用してシステム音声出力をリアルタイムでキャプチャ。
- `IAudioClient` を `AUDCLNT_STREAMFLAGS_LOOPBACK` モードで使用
- PCM データ提供（float32 → int16 変換、チャネルダウンミックス）
- オーディオデバイス選択に対応

### オーディオエンコーダー

| エンコーダー | ライブラリ | ビットレート | 機能 |
|:-------------|:-----------|:-------------|:-----|
| LDAC | libldac (AOSP) | 330/660/990 kbps | HQ/SQ/MQ モード、ABR |
| aptX HD | libopenaptx | 576 kbps | 24-bit、固定レート |
| aptX LL | libopenaptx | 352 kbps | 約 32 ms レイテンシー |
| AAC | fdk-aac | 最大 256 kbps | AAC-LC、LATM トランスポート |
| SBC | BTstack Bluedroid | 最大約 345 kbps | A2DP 必須ベースライン |

すべてのエンコーダーは `AudioEncoder` インターフェースの `encode()` と `get_frame_size()` メソッドを実装しています。

## ベンダーコーデック情報要素

非標準コーデックは AVDTP で Vendor Specific として登録:

| コーデック | Vendor ID | Codec ID |
|:-----------|:----------|:---------|
| LDAC | Sony (0x0000012D) | 0x00AA |
| aptX HD | Qualcomm (0x000000D7) | 0x0024 |
| aptX Low Latency | CSR (0x0000000A) | 0x0002 |

AAC と SBC は A2DP 仕様で定義された標準コーデック ID を使用します。

## ビルドシステム

- **CMake** + MSVC (Visual Studio 2022 以降)
- サードパーティライブラリはソースから静的ライブラリとしてビルド
- wxWidgets はビルド設定時に CMake FetchContent で取得 (v3.2.6)
- 言語ファイル (JSON) はビルド設定時に実行ファイルに埋め込み
- アプリケーションマニフェストとアイコンは Windows リソーススクリプトでコンパイル

## 重要な注意事項

- **アダプター互換性**: Intel、CSR、Realtek USB アダプターでテスト済み。Realtek アダプターは起動時にファームウェアアップロードが必要
- **ペアリング**: SSP Just Works を使用。リンクキーはローカルファイルに永続化
- **セカンドアダプター推奨**: Windows には内蔵 Bluetooth、A2DPWB には専用 USB アダプターを使用
- 一部の Bluetooth アダプターのファームウェアは達成可能なビットレートを制限する場合がある
- USB Bluetooth 5.0 以上のアダプターは LDAC に適している
- 自動コーデック選択の優先順位: LDAC > aptX HD > aptX LL > AAC > SBC
