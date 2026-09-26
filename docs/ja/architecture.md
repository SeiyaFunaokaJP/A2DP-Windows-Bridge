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

A2DP Windows Bridge (A2DPWB) は Windows で LDAC、aptX HD、aptX Low Latency、aptX、AAC、SBC の Bluetooth オーディオを実現します。Windows 標準の Bluetooth スタックが A2DP で対応するのは SBC、AAC、クラシック aptX のみですが、このツールはカーネルドライバーなしで LDAC、aptX HD、aptX Low Latency を追加します。

**BTstack + WinUSB** を使用 -- 完全にユーザーモードで動作し、ドライバー署名は不要です。

## トランスポート: BTstack + WinUSB

Windows の Bluetooth スタックを完全にバイパスし、WinUSB（Microsoft 署名済み汎用 USB ドライバー）経由で USB Bluetooth アダプターと直接通信します。ソース公開の Bluetooth スタック（非商用ライセンス）である BTstack が HCI、L2CAP、AVDTP、A2DP をユーザーモードで実装します。

**利点**:
- ドライバー署名コスト不要
- テスト署名モード不要
- Secure Boot の無効化不要
- すべてのコードがユーザーモードで動作（デバッグが容易）

**トレードオフ**:
- 専用の USB Bluetooth アダプターが必要（Windows 内蔵 Bluetooth とは別）
- WinUSB モードのアダプターは Windows の通常の Bluetooth として使用不可
- デバイスアドレスは手動入力、内蔵アダプター経由で取得した Windows ペアリング済みデバイス一覧から選択、または USB アダプターでの検索（GAP inquiry）で指定

**動作の流れ**:
1. Zadig で USB Bluetooth アダプターに WinUSB ドライバーをインストール
2. BTstack が WinUSB API 経由で USB デバイスを開く
3. BTstack が HCI コマンドで Bluetooth コントローラーを初期化
4. (Realtek、Intel、Broadcom アダプター) 必要に応じてファームウェアをアップロード
5. BTstack が ACL 接続を確立し、L2CAP チャネル (PSM 0x0019) を開く
6. AVDTP シグナリングでリモート SEP を検出し、コーデックをネゴシエーション
7. エンコード済み音声が L2CAP 経由の AVDTP メディアパケットとして送信

## モジュール構成

```
A2DPWB.exe
├── GUI レイヤー (wxWidgets)
│   ├── wx_app              アプリエントリーポイント、イベントループ
│   ├── wx_main_frame       メインウィンドウ（デバイス、コーデック、ステータス）
│   ├── wx_profile_dialog   接続プロファイル管理、デバイス検索
│   ├── wx_settings_dialog  アプリケーション設定
│   ├── wx_firmware_dialog  Realtek ファームウェアダウンロード
│   ├── wx_about_dialog     バージョン情報 / ライセンス
│   ├── wx_zadig_dialog     Zadig WinUSB インストールガイド
│   ├── wx_link_quality_dialog  通信品質ウィンドウ（送信内容、電波）
│   ├── wx_receiver_dialog  対向受信テスト（送信と受信の比較、デバッグモード）
│   ├── wx_debug_console_dialog  デバッグコンソール: ログ表示、接続フロー
│   ├── wx_radio_text.h     RSSI / AFH の表示文字列（両ウィンドウ共通）
│   ├── theme_manager       ライト / ダークテーマ対応
│   └── localization        多言語化 (en, ja -- 埋め込み JSON)
│
├── コアレイヤー
│   ├── a2dp_service        A2DP 接続ライフサイクル & ステートマシン
│   ├── btstack_transport   BTstack 統合 (HCI, L2CAP, AVDTP, A2DP)
│   ├── btstack_link_key_db_file  リンクキー保存（設定フォルダーのファイル）
│   ├── btstack_uart_tcp_windows  仮想コントローラーへの H4 over TCP（--hci-tcp、テスト用）
│   ├── wasapi_capture      WASAPI ループバックオーディオキャプチャ
│   ├── test_tone           キャプチャ音声の代わりのテストトーン
│   ├── audio_encoder       エンコーダーインターフェース（抽象基底）
│   │   ├── ldac_encoder        LDAC (libldac, ABR 対応)
│   │   ├── aptxhd_encoder      aptX HD (libopenaptx)
│   │   ├── aptxll_encoder      aptX Low Latency (libopenaptx)
│   │   ├── aptx_encoder        aptX クラシック (libopenaptx)
│   │   ├── aptx_pcm_pack.h     aptX 系エンコーダー共通の PCM → パック 24-bit 変換
│   │   ├── aac_encoder         AAC-LC (fdk-aac, LATM トランスポート)
│   │   └── a2dp_sbc_encoder    SBC (BTstack Bluedroid)
│   │
│   ├── bt_device           Bluetooth デバイス情報（アドレス、名前、コーデック）
│   ├── bt_adapter_enum     USB Bluetooth アダプター列挙 (WinUSB)
│   ├── audio_device_enum   WASAPI オーディオデバイス列挙
│   ├── profile_manager     接続プロファイル永続化 (JSON)
│   ├── media_payload_limit 最大メディアパケットサイズ（範囲、既定値）
│   ├── afh                 Wi-Fi スキャンからの AFH ホストチャネル分類
│   ├── link_stats          通信品質ウィンドウ用のカウンター
│   └── remote_sink_client  tools/linux_sink の統計をネットワーク経由で取得
│
├── サポート
│   ├── app_settings        永続アプリケーション設定 (JSON)
│   ├── config_path         設定ファイルパス解決
│   ├── system_integration  システムトレイ、自動起動
│   ├── debug_log           デバッグログマクロ
│   ├── debug_log_model     デバッグコンソール用のログ行と接続フロー
│   ├── hci_capture         随時開始できる HCI キャプチャ (.pklg)
│   ├── update_checker      アップデート確認 (GitHub Releases)
│   ├── zadig_helper        Zadig のダウンロード / 起動
│   ├── embedded_langs      ビルド時に埋め込む言語ファイル
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
 WASAPI ループバックキャプチャ (デバイスのミックス形式、通常 float32、44.1〜96 kHz)
       │
       ▼
 オーディオエンコーダー (LDAC / aptX HD / aptX LL / aptX / AAC / SBC)
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
- aptX 系のサンプルレート選択（リモートが通知する 44.1 / 48 kHz から選び、キャプチャは WASAPI がリサンプリング）
- エンコーダー向けの PCM 変換（float32 → 16 / 32-bit 整数）
- 接続ステートマシン（idle → connecting → streaming → reconnecting、error）
- 予期しない切断時の自動再接続ロジック
- コーデック固有のフレーミングによるメディアパケット送信

### BTstack Transport (`btstack_transport.cpp`)

アプリケーションの同期モデルと BTstack のイベント駆動 API のブリッジ。

**役割**:
- WinUSB HCI トランスポートで BTstack を初期化
- 専用スレッドで BTstack イベントループを実行
- ストリームエンドポイントを登録: ベンダーコーデック (LDAC, aptX HD, aptX, aptX LL) と SBC、AAC
- SDP レコード (A2DP Source, AVRCP Controller, AVRCP Target) を登録
- リモートの対応コーデックを解析（両方の Vendor ID の aptX LL、および検出・ログ出力のみの aptX Adaptive を含む）
- 非同期→同期ラッパーで A2DP 接続ライフサイクルを管理
- SSP ペアリング（Just Works、General Bonding、リンクキー永続化）を処理
- 接続タイムアウト後に中途半端な接続を破棄し、再試行できるようにする
- 失敗した接続を再試行し（最大 3 回）、原因（応答なし、通信のないリンク、相手からの切断、認証）を区別する。デバイスが失ったリンクキーを破棄してペアリングし直す
- デバイス検索のための GAP inquiry
- AFH ホストチャネル分類と、通信品質ウィンドウ用の RSSI / AFH の読み取り
- WASAPI コールバック向けスレッドセーフなメディア送信を提供
- Realtek、Intel、Broadcom チップセットのファームウェアロード

**使用する主な BTstack API**:
- `a2dp_source_create_stream_endpoint()` -- コーデックエンドポイント登録
- `a2dp_source_establish_stream()` -- A2DP シンクに接続
- `a2dp_source_set_config_other()` -- ベンダー固有コーデック設定
- `a2dp_source_stream_send_media_payload_rtp()` -- RTP ヘッダー付きでエンコード済み音声を送信 (LDAC, aptX HD, AAC, SBC)
- `a2dp_source_stream_send_media_packet()` -- RTP ヘッダーなしでメディアパケットを送信 (aptX, aptX LL)

### WASAPI Capture (`wasapi_capture.cpp`)

Windows Audio Session API を使用してシステム音声出力をリアルタイムでキャプチャ。
- `IAudioClient` を `AUDCLNT_STREAMFLAGS_LOOPBACK` モードで使用
- デバイスの共有モードのミックス形式（通常 float32）で提供（エンコーダー向けの変換は A2DP Service が行う）
- オーディオデバイス選択に対応

### オーディオエンコーダー

| エンコーダー | ライブラリ | ビットレート | 機能 |
|:-------------|:-----------|:-------------|:-----|
| LDAC | libldac (AOSP) | 330/660/990 kbps | HQ/SQ/MQ モード、ABR |
| aptX HD | libopenaptx | 576 kbps | 24-bit 入力、固定レート、RTP ヘッダーあり |
| aptX LL | libopenaptx | 352 kbps | 約 32 ms レイテンシー、RTP ヘッダーなし、パケットは約 7.5 ms 以下 |
| aptX | libopenaptx | 352/384 kbps (44.1/48 kHz) | 16-bit ステレオ、RTP ヘッダーなし |
| AAC | fdk-aac | 最大 256 kbps | AAC-LC、LATM トランスポート |
| SBC | BTstack Bluedroid | 最大約 345 kbps | A2DP 必須ベースライン |

すべてのエンコーダーは `AudioEncoder` インターフェースの `encode()` と `get_frame_size()` メソッドを実装しています。

libopenaptx は 4 ステレオサンプル単位のパック済み 24-bit リトルエンディアン PCM を入力として受け取ります。`aptx_pcm_pack.h` はキャプチャした 16-bit または 32-bit（MSB 詰め）のサンプルをこの形式に変換し、aptX・aptX HD・aptX LL エンコーダーに渡します。モノラル入力は両チャネルに複製されます。

## ベンダーコーデック情報要素

非標準コーデックは AVDTP で Vendor Specific として登録:

| コーデック | Vendor ID | Codec ID |
|:-----------|:----------|:---------|
| LDAC | Sony (0x0000012D) | 0x00AA |
| aptX | APT (0x0000004F) | 0x0001 |
| aptX HD | Qualcomm (0x000000D7) | 0x0024 |
| aptX Low Latency | CSR (0x0000000A) または Qualcomm (0x000000D7) | 0x0002 |
| aptX Adaptive | Qualcomm (0x000000D7) | 0x00AD（検出のみ、選択されない） |

AAC と SBC は A2DP 仕様で定義された標準コーデック ID を使用します。

コーデック情報要素のサイズ（6 バイトの Vendor ID / Codec ID を含む）:

- **aptX**: 7 バイト（バイト 6 = サンプルレート / チャネルモード）
- **aptX HD**: 11 バイト（バイト 6 は aptX と同じ、加えて予約 4 バイト）。ステレオ必須
- **aptX LL**: 8 バイト。シンクが拡張（"new caps"）フラグを立てている場合は 17 バイト。A2DPWB は aptX LL エンドポイントを 1 つだけ登録し、シンクが使用した Vendor ID でストリームを設定します

**RTP ヘッダーの有無**: LDAC・aptX HD・AAC・SBC のメディアパケットには 12 バイトの RTP ヘッダーが付きます。aptX と aptX LL は（Android や PipeWire と同様に）RTP ヘッダー**なし**で送信するため、L2CAP MTU 全体を aptX フレームに使えます。

**aptX Adaptive** にはオープンソースのエンコーダーがない（libopenaptx も未実装）ため、A2DPWB は登録も選択もしません。シンクが通知した場合はログに記録するだけで、シンクがクラシック aptX を別途通知していればそちらを使用します。

## ビルドシステム

- **CMake** + MSVC (Visual Studio 2022 以降)
- サードパーティライブラリはソースから静的ライブラリとしてビルド
- wxWidgets はビルド設定時に CMake FetchContent で取得 (v3.2.6)
- 言語ファイル (JSON) はビルド設定時に実行ファイルに埋め込み
- アプリケーションマニフェストとアイコンは Windows リソーススクリプトでコンパイル

## 重要な注意事項

- **アダプター互換性**: Realtek（最も検証済み）と CSR の USB アダプター。Intel と Broadcom は試験的。Realtek アダプターは起動時にファームウェアアップロードが必要（Intel もブートローダーモードでは必要）
- **ペアリング**: SSP Just Works（General Bonding）を使用。リンクキーはローカルファイルに永続化
- **セカンドアダプター推奨**: Windows には内蔵 Bluetooth、A2DPWB には専用 USB アダプターを使用
- 一部の Bluetooth アダプターのファームウェアは達成可能なビットレートを制限する場合がある
- USB Bluetooth 5.0 以上のアダプターは LDAC に適している
- 自動コーデック選択の優先順位: LDAC > aptX HD > aptX LL > aptX > AAC > SBC
