---
title: v2 設計書（Rust / Tauri）
layout: default
nav_exclude: true
---

# BlueCaster 次期版 設計書（Rust / Tauri）

Oct 17, 2026 · @seiya ／ 改訂: 未決事項の決定を反映（同日）

> **この文書は更新を終えた設計の記録です**（2.0.2-beta.12 時点、2026-10-30）。v2 を作ったときの判断とその理由を残すために置いています。いまの構成と動作は [Architecture](architecture)、外部ツール向けの IPC は [Integration (IPC)](integration)、使い方は [Usage](usage) を見てください。ここに書かれた計画やメソッドの一覧は、実装と違っていることがあります。以降の変更はプルリクエストとリリースノートに残します。

## 概要

次期版（v2.0）では、BlueCaster を **Rust製のコアサービス** と **Tauri製のUI** の2つに分けて作り直します。自前のC++コード（約2.3万行）はすべてRustに書き換えます。btstackや各コーデックのように置き換えの効かないCライブラリは、FFIでそのまま使い続けます。

**目的**

- 安全性: 自前のコードからメモリ破壊とデータ競合をなくします。`unsafe` はFFI用の決まったクレートの中だけに閉じ込めます。
- 操作性・見た目: 初めて使う人でも「つなぐ」「音を流す」「状態を見る」が迷わずできる、モダンで見やすいUIにします。
- 構造: コアとUIを分けて、UIを閉じても音が止まらないようにします。常駐中はメモリも軽くします。

**前提**

- 対応OSは Windows 10 (22H2) / 11 です。**x64 と ARM64 の両方を v2.0 で出します**（[ARM64](#arm64) を参照）。
- 使い方は今と同じです。WinUSBを割り当てたUSBドングルを btstack で直接制御して、WASAPIで取り込んだシステム音声をA2DPで送ります。
- コーデックは今と同じものを用意します（SBC / AAC / aptX / aptX HD / aptX LL / LDAC / LHDC V5）。v2 では SBC XQ、FastStream と Android の A2DP Opus（試験的）を加えます（[v2 で加えたコーデック](#v2-で加えたコーデック)、[Opus](#opus)）。aptX Adaptive も試験的に加えました（[aptX Adaptive](#aptx-adaptive)）。
- アプリ名（BlueCaster / BlueCaster）とアイコンは v1 のまま変えません。

**今回やらないこと**

- Bluetoothスタック本体（btstack）をRustで書き直すこと
- コーデックのエンコーダをRustで作り直すこと（SBCだけは小さいので例外としてRustに移植します）
- LE Audio の正式対応。v1 の実験的な LE Audio（BAP ユニキャスト、CSIP / VCP / MCP / CCP / TMAP / GMAP）は 2026-10-18 に v2 へ移植しましたが、実験的な機能（既定でオフ）のままです。実機での確認が済むまで正式な機能にはしません（[LE Audio（実験的）](#le-audio実験的)を参照）。
- macOS / Linux 版のホストアプリ

## 現行機能の棚卸し

v1.1.0の機能は、開発者向けのものも含めてすべて次期版に引き継ぎます。引き継がないのは、使われていない `wx_zadig_dialog` だけです（ドライバ導入はアプリ内の新しい仕組みに置き換えます。[ドライバ](#ドライバwinusb)を参照）。設定フォルダ（`%APPDATA%\BlueCaster`、`BLUECASTER_CONFIG_DIR` で変更可）と中のファイルはそのまま読みます。

| 機能 | 現行の場所 | 次期の場所 |
| --- | --- | --- |
| プロファイル（デバイス、コーデック、品質、ABR、サンプルレート、ビット深度、取り込み元、音量同期、最大パケットサイズ） | メイン画面の一覧、プロファイルダイアログ | `core`、UIの「プロファイル」画面 |
| デバイス検索と登録（保存済み、Windowsでペアリング済み、検索結果） | プロファイルダイアログ | `bt-host`、プロファイル編集画面 |
| 接続・切断、自動再接続、最大3回の再試行、リンクキー紛失時の再ペアリング | A2dpService、BtStackTransport | `core` の状態機械 |
| スリープ復帰時のBTstack再起動 | A2dpService | `core` + `win-sys`（電源通知） |
| 7コーデックとAuto選択、代替コーデックへの切り替え、使えない理由の表示 | 各エンコーダ、メイン画面のコーデック行 | `codec`、ホームのコーデックバッジ |
| 取り込み（システムループバック / 仮想デバイスと既定デバイスの自動切り替え / アプリ単位で終了と再起動に追従） | wasapi\_capture、audio\_device\_enum | `audio-capture`、`win-sys` |
| AVRCP: 絶対音量の双方向同期、音量の記憶、ヘッドホンのボタンをメディアキーに変換 | volume\_sync、BtStackTransport | `bt-host` + `core` |
| AFH（Wi-Fiのチャネルを避ける。auto / off / 手動） | afh | `bt-host` + `win-sys`（WLAN API）、UIの「アダプタ」画面 |
| ファームウェア（Realtek 12チップ、IntelとBroadcomは実験的、カスタム） | ファームウェアダイアログ | `bt-host`、初回セットアップと「アダプタ」画面 |
| 接続品質（RSSI、AFHチャネル、3-DH5帯域、エアタイム、送信前ドロップ、取り込みドロップ） | リンク品質ダイアログ | `pipeline` のカウンタ、「診断」画面 |
| デバッグコンソール（接続の流れ11ステップ、ログ、アドレスの伏せ字、診断情報のコピー） | デバッグコンソール | `tracing` + IPC、「診断」画面 |
| HCIキャプチャ（`.pklg`。接続途中からでも接続時のパケットを含める） | hci\_capture | `bt-host` |
| 受信機テスト（`tools/linux_sink` とTCP / UDP 51201で連携して測定） | 受信機ダイアログ、remote\_sink\_client | `core`（tokio）、開発者モードの診断タブ |
| テストトーン（左 1 kHz / 右 1.5 kHz） | test\_tone | `audio-capture` |
| トレイ（プロファイル10件までのクイック接続、切断、通知）、二重起動防止 | wx\_app、wx\_main\_frame | `bluecaster-core.exe` のトレイ、トレイフライアウト |
| 自動起動（HKCU Run、`--minimized`）、更新チェック（GitHub Releases、プレリリースも選べる） | system\_integration、update\_checker | `win-sys`、`core` + `tauri-plugin-updater` |
| 8言語（en / ja / de / fr / es / ko / zh-CN / zh-TW）、テーマ（ダーク / ライト / システム） | localization、theme\_manager | UI（フロントエンド） |
| CLI（`--cli`、`--hci-tcp`、`--test-tone`、`--duration` など） | main.cpp（A2dpServiceを使わない重複ロジック） | `bluecaster-core.exe --cli`（同じ `core` を使う） |
| 開発ツール（`tools/emu`、`tools/linux_sink`、`tools/a2dp_decode`） | tools/ | そのまま使う。結合テストのCIに組み込む |
| **（新規）WinUSBドライバの割り当てと解除** | Zadig（外部ツール） | `win-sys` の `driver` + 昇格ヘルパー `bluecaster-driver.exe`、初回セットアップと「アダプタ」画面 |

## 全体アーキテクチャ

アプリは、音を流す常駐プロセス `bluecaster-core.exe` と、開いているときだけ動く画面プロセス `bluecaster.exe` の2つに分かれます。UIを閉じても、UIが落ちても、音は止まりません。（設定「システムトレイに最小化」をオフにしているときだけは、v1 と同じくメインウィンドウを閉じるとトレイの「終了」と同じくコアごと終了します。）ドライバの割り当てと解除だけは管理者権限が要るので、その時だけ起動する小さな昇格ヘルパー `bluecaster-driver.exe` を別に持ちます。

```
┌─────────────────────────────┐        Named Pipe         ┌──────────────────────────────┐
│ bluecaster.exe (Tauri 2 / UI)   │ ◀──── \\.\pipe\bluecaster-<SID> ────▶ │ bluecaster-core.exe (常駐)         │
│  SolidJS + TypeScript       │   hello / snapshot /      │  tray-icon・IPCサーバー・CLI   │
│  バックエンド: ipc クライアント │   event / request          │  ┌ core（状態機械・設定）       │
└─────────────────────────────┘                           │  ├ pipeline（エンコード経路）   │
                                                          │  ├ bt-host（btstackスレッド）  │
       ┌──────────────────────────┐  UAC昇格で一時起動      │  ├ audio-capture（WASAPI）     │
       │ bluecaster-driver.exe        │ ◀──────────────────── │  └ win-sys / *-sys（FFI）      │
       │ WinUSBの割り当て・解除      │                        └──────────────────────────────┘
       └──────────────────────────┘
```

上の段ほどアプリ寄り、下の段ほどOSやCライブラリ寄りで、依存は上から下への一方向です。

- **コアはWindowsサービスにしません。** ログオンしたユーザーの普通のプロセスとして動かします。WASAPIループバックで取り込むのはそのユーザーのセッションの音で、サービスのセッション0からは扱いにくいためです。
- **トレイはコア側に置きます。** 右クリックのメニュー（プロファイルへの接続、切断、終了）は、UIを起動しなくてもすぐに開きます。左クリックでUIのフライアウトを開きます。
- **UIは表示だけを担当します。** 状態はすべてコアが持ち、UIは起動のたびにsnapshotを受け取って画面を組み立てます。
- **CLIも同じコアを使います。** `bluecaster-core.exe --cli` はトレイとIPCを起動せずに `core` を動かします。エミュレータの結合テストもこの経路で回します。

## Rustクレート構成

ひとつのCargo workspace（リポジトリ直下の `Cargo.toml`）を、依存が下から上へ一方向になる5つの層に分けます。ライブラリのクレートは `app/src/crates/` に、実行ファイルは役割ごとに `app/src/core`（コア）、`app/src/ui`（UI）、`app/src/driver`（昇格ヘルパー）に置きます。翻訳は `app/lang/`、アイコンとマニフェストは `app/resources/` です。`unsafe` を書いてよいのは `*-sys` クレートと、Win32 APIを直接呼ぶ `win-sys` だけです。それ以外のクレートには `#![forbid(unsafe_code)]` を付けます。

| 層 | クレート（ディレクトリ） | 責務 | unsafe |
| --- | --- | --- | --- |
| 1. FFI | `btstack-sys` | btstackを `cc` でビルドし、FFI宣言と安全なラッパーを提供。`btstack_config.h` もここに置く。btstackを呼べるのはランループのスレッドだけ、という制約を型（`!Send` の `Stack` トークン）で強制する | 可 |
| 1. FFI | `ldac-sys` / `openaptx-sys` / `fdkaac-sys` / `lhdcv5-sys` / `opus-sys` | 残すCコーデックをビルドしてFFIを提供。ライセンスの異なるライブラリをクレート単位で分けて管理する | 可 |
| 1. FFI | `win-sys` | `windows` クレートで、Rustのライブラリが無いWin32 APIを包む。MMCSS、IPolicyConfig（既定デバイスの切り替え）、WLAN API（AFH用）、電源通知、レジストリのRunキー、メディアキー送出、**WinUSBドライバの割り当てと解除（SetupAPI / DiInstallDevice）** | 可 |
| 2. 部品 | `hci-transport` | HCI H2（`nusb` でWinUSBを直接制御）とH4 over TCP（エミュレータ用）。現行の改造版 `hci_transport_h2_winusb.c` の復旧処理（パイプリセット、読み取り失敗からの復帰、コマンドタイムアウト、終了時のHCI Reset）を移植し、btstackには `btstack-sys` のコールバック表で接続する | 禁止 |
| 2. 部品 | `codec` | `trait Encoder`、Rustで書き直したSBCエンコーダ、その他コーデックの安全なラッパー。Codec Informationの生成・解析と能力の交渉（Autoの優先順位 LDAC > aptX HD > aptX LL > aptX > LHDC V5 > Opus > AAC > SBC XQ > SBC。FastStream、Opus 05、aptX Adaptive は明示的に選んだときだけ）も担当 | 禁止 |
| 2. 部品 | `audio-capture` | `wasapi` クレートで取り込む。システムループバック、仮想デバイス、アプリ単位のループバック（build 19041以降）。デバイス一覧、エンドポイント音量、テストトーン | 禁止 |
| 3. エンジン | `bt-host` | btstackのランループを専用スレッドに閉じ込め、外からはコマンドチャネルと型付きイベントだけで操作する。GAP / A2DP Source / AVRCP（絶対音量、ボタン操作）/ SDP / リンクキー / AFH / ファームウェア読み込み / pklgキャプチャ | 禁止 |
| 3. エンジン | `pipeline` | リングバッファ、形式変換、リサンプル、ダウンミックス、エンコード、メディアペイロードの組み立て、送信ペースの制御、ABR | 禁止 |
| 4. コア | `core`（パッケージ名 `bluecaster-core`） | 状態機械（アダプタ・デバイス・ストリーム）、自動再接続、スリープ復帰、プロファイル、設定、音量同期、更新チェック、受信機テストのクライアント、統計の集計 | 禁止 |
| 4. コア | `ipc` | コアとUIで共有するプロトコル型（serde）とNamed Pipe通信。`ts-rs` でTypeScriptの型を自動生成する（`cargo test -p ipc export_bindings`） | 禁止 |
| 5. 実行ファイル | `bluecaster-core.exe`（`app/src/core`） | 常駐プロセス。コア、タスクトレイ（`tray-icon`）、IPCサーバー。`--cli` でも同じ `core` を使うので、現行版のような重複ロジックはなくなる | 禁止 |
| 5. 実行ファイル | `bluecaster.exe`（`app/src/ui`） | Tauri 2のUI。バックエンドはIPCクライアントだけを持ち、フロントは **SolidJS** + TypeScript + Vite | 禁止 |
| 5. 実行ファイル | `bluecaster-driver.exe`（`app/src/driver`） | 管理者権限で一時起動する昇格ヘルパー。`win-sys::driver` を呼んでWinUSBを割り当て / 解除して終了する | 禁止 |

> `core` は Rust 標準の `core` クレートと名前が衝突するため、パッケージ名は `bluecaster-core` にします（ディレクトリは `app/src/crates/core`）。

**FFIの作り方（`btstack-sys` ほか）**

- 当初案の `bindgen` は使いません。`bindgen` は libclang を要求し、ビルド環境の前提が増えるためです。使う関数とデータ構造は限られている（btstack で約100関数）ので、FFI宣言は手で書き、Cの構造体の中身に触る必要がある所（`hci_connection_t` の状態、`avdtp_connection_t` の状態など）は小さなCシム（`csrc/shim.c`）に関数として置きます。
- 手書きの宣言とCの実体がずれないように、構造体のサイズとオフセットをシムが返し、`btstack-sys` の初期化時とテストで照合します。
- イベントの解析（btstack の `btstack_event.h` の static inline ゲッター相当）は、`bt-host` 側で境界チェック付きのスライス読み取りとして書きます。

**依存ライブラリの方針**

- 非同期の管理系（IPC、更新チェック、設定保存）は `tokio` で動かします。音声とBluetoothのスレッドはtokioに載せず、専用のOSスレッドにします。
- Rust製のライブラリで機能・性能・メモリのどれかが良くなるものは置き換えます（次の表）。置き換えると弱くなるものだけCのまま残します。
- ビルドは `cargo` だけで完結させ、CMakeへの依存はなくします。残すCのソースは今のようにgit submoduleで取り込み、`build.rs` のMSVCでコンパイルします。

**Rust製に置き換えるライブラリ**

| 現行 | 次期 | 良くなる点 |
| --- | --- | --- |
| wxWidgets 3.2.6 | Tauri 2 + SolidJS | 見た目の自由度。UIを閉じればそのプロセスごとメモリが空く。SolidJSは仮想DOMを持たず、細かい状態更新（4Hzの統計など）でも再描画が軽い |
| 改造版 `hci_transport_h2_winusb.c` + SetupAPIによる列挙 | `nusb` | Pure RustのUSBライブラリ。非同期のバルク / インタラプト転送を複数本並行で投げられ、ホットプラグも扱える。ドングルの抜き差しへの追従が良くなる |
| WASAPIのCOMを直接呼ぶコード | `wasapi` クレート | ループバックとイベント駆動に対応した安全なラッパー。COMの参照計数ミスがなくなる |
| WASAPIの自動リサンプル | `rubato`（選択式） | 44.1 kHz ↔ 48 kHzの変換を高品質なシンク補間で行える。軽いモードと高品質モードを選べる |
| BluedroidのSBCエンコーダ（C） | Rustへ移植 | 小さな固定小数点コードで、Apache-2.0なので移植できる。必ず使われるコーデックから `unsafe` がなくなる。出力が元の実装とビット単位で一致することをテストで確かめる |
| 自作のリングバッファ（256 KB） | `rtrb` | 検証済みのロックフリーSPSC。待機なし、メモリ確保なし |
| nlohmann/json | `serde` + `serde_json` | 設定とプロファイルを型で読み書きできる。今の `settings.json` / `profiles.json` の形式はそのまま読める |
| WinHTTPと自前のバージョン比較 | `ureq`（rustls）+ `semver` | 一日一回の更新チェックには同期型の軽いクライアントで十分。プレリリースを含むバージョン比較を標準の実装に任せられる |
| 更新はリリースページを開くだけ | `tauri-plugin-updater` | 署名付きの更新をアプリ内でダウンロードして入れられる（機能追加） |
| stderrを `debug.log` に向けてUIがファイルを読む | `tracing` + `tracing-appender` | 構造化ログを別スレッドで書き出す。ローテーション付きで、UIにはIPCで直接届く |
| wxのトレイと二重起動防止のmutex | `tray-icon` + `tauri-plugin-single-instance` | トレイは常駐するコア側に置き、UIの二重起動は既存のウィンドウを前に出す |
| Inno Setup 6 | Inno Setup 6（2.0.2-beta.5 までは TauriのNSISバンドラ） | 台本が人に読みやすく、開発が活発で、小さいうちに移しておけば後が楽（改訂履歴 2026-10-22）。ユーザー単位のインストール、8言語のセットアップ画面、アンインストール時のRunキー削除は `installer/BlueCaster.iss` の [Code] で行う |
| Zadig（外部ツール、手作業） | `win-sys::driver`（SetupAPI） | アプリの中で割り当てと解除ができる。コピーレフトのライブラリを使わない（[ドライバ](#ドライバwinusb)） |

**Cのまま残すライブラリ**

| ライブラリ | 残す理由 |
| --- | --- |
| btstack | Windowsで使えるClassic Bluetooth（A2DP / AVDTP / AVRCP）のホストスタックで、実用水準のRust実装はない |
| libldac / lhdcv5-enc | 仕様が公開されておらず、参照実装を使う以外にない |
| fdk-aac | AACエンコーダでこれを超える品質のRust実装はない |
| libopus | Opusの参照実装。Rustの純粋な実装は品質・速度ともに及ばない（v2で追加、BSD-3-Clause） |
| libopenaptx 0.2.0 | Rustへの移植自体は難しくない。ただし移植版もLGPL-2.1の派生物になり、ライセンスの扱いは変わらない。なので当面は独立した `-sys` クレートに閉じ込めておく |

### v2 で加えたコーデック

**SBC XQ**（キー `sbcxq`、表示名 "SBC XQ"。PipeWire の `sbc_xq` と同じ考え方）

- SBC を dual channel、16ブロック、8サブバンド、loudness で、チャネルごとに高い bitpool を使って送ります。メディアペイロードヘッダ（フレーム数1バイト）とRTPヘッダはSBCと同じです。ローカルのSEPはSBCのもの（全チャネルモード、bitpool 2〜53）を共用し、SET_CONFIGURATION で dual channel を選びます。
- bitpool（`codec::sbc::xq_bitpool`）: High は 44.1 kHz で 47（200バイトのフレーム、約551 kbps。Androidのカスタムロムや pulseaudio-modules-bt の "SBC XQ" と同じ）、48 kHz で 43（約552 kbps、同じビットレート）。Standard は PipeWire の SBC-XQ の既定値（`a2dp-codec-sbc.c` の `default_bitpool`）の 43 / 39（約505 kbps、A2DP 1.2 のシンクが必ず受けられる 512 kbps 以内）、Mobile は PipeWire の XQ でない dual channel の既定値 32 / 29（約385 kbps）。High のフレームは、ヘッダ込みで 2-DH5 に3つ、3-DH5 に5つ入ります（1023バイトのパケット上限で1001 / 921バイト）。dual channel の bitpool の上限はチャネルごとに 16 × サブバンド数 = 128 なので、どれも制限の内側です。
- 交渉: シンクのSBCの能力（BTstack の SBC capability イベントのビットマップと bitpool の範囲）に dual channel、16ブロック、8サブバンド、loudness、44.1 / 48 kHz があり、最大 bitpool が 29 以上のときだけ SBC XQ を提供していると見なします（2026-10 まで 43。PipeWire は下限なしで min(43 / 39, シンクの最大) に抑えるだけで、最大 39 の Moto buds+ でも XQ になる。issue #86）。設定する bitpool の範囲は max(2, シンクの最小) 〜 min(そのレートの High の bitpool, シンクの最大)、エンコーダの bitpool は品質の値をシンクの範囲に収めたものです。通常の SBC も同じようにシンクの範囲を使います（範囲は max(2, シンクの最小) 〜 min(53, シンクの最大)。能力が分からないときは 2 〜 53）。条件を満たさないシンクでは SBC XQ は出ず、Auto は SBC を選び、明示的に選んだ場合は代替コーデックの設定に従って SBC に切り替わるか、エラーになります。
- 能力では dual channel を出しているのに、SET_CONFIGURATION で dual channel を拒否するシンクがあります。SBC XQ の設定が拒否された（または応答がなかった）ときは、Auto か代替コーデックがオンなら一度だけ通常の SBC でやり直します。BTstack はこの接続の A2DP の設定手順を拒否で終えてしまう（2回目の SET_CONFIGURATION を受け付けない）ので、いったん切断して接続し直し、能力を読み直してから SBC を設定します。レート、チャネル数、ビット数は SBC XQ と同じなので、取り込みはそのままです。明示的に SBC XQ を選び代替コーデックがオフなら、これまでどおりエラーです。拒否された SET_CONFIGURATION と START は、BTstack の `A2DP_SUBEVENT_COMMAND_REJECTED` を見て待ちをすぐ終えます（以前は10秒のタイムアウトまで待っていました）。
- Auto の優先順位は AAC の下、SBC の上です。ビット数は AAC 256 kbps より多いものの、AAC を出すシンクは AAC 向けに調整されていることが多く、ビットあたりの音質も AAC のほうが上だからです。
- 検証: 単体テスト（フレーム長とビットレート、パケットへの収まり、交渉とフォールバック）と `tools/emu`（`sbcxq` と、SBCだけのシンクでの Auto の2ケース `auto-sbcxq` / `auto-sbc-no-xq`、dual channel を拒否するシンクで SBC に戻る `auto-sbcxq-refused`、最大 bitpool の低いシンクの `auto-sbc-bp35` / `sbcxq-bp45`）。実機では未確認です。

**FastStream**（キー `faststream`、表示名 "FastStream"。CSR のベンダーコーデック、vendor ID 0x0000000A / codec ID 0x0001）

- PipeWire の `a2dp-codec-faststream.c` に合わせています。Codec Information は8バイトで、byte 6 が方向（0x01 シンク = ヘッドセットへの音楽、0x02 ソース = ヘッドセットからの音声）、byte 7 の下位4ビットがシンクのレート（0x2 44.1 kHz、0x1 48 kHz）、上位4ビットがソースのレート（0x2 16 kHz）です。
- 音楽の方向だけを実装しました。ローカルのSEPは方向 0x01、レート 44.1 / 48 kHz を出し、SET_CONFIGURATION も方向 0x01 と1つのレートだけを送ります。ヘッドセットからの音声（16 kHz の SBC、PipeWire の duplex）は実装していません。
- 中身は固定の SBC です。44.1 / 48 kHz、joint stereo、16ブロック、8サブバンド、loudness、bitpool 29（71バイトのフレーム）。各フレームを0の1バイトで偶数長の72バイトにし、1パケットに3フレーム（216バイト、48 kHz で 8 ms 分）を入れます。メディアペイロードヘッダもRTPヘッダも付けません。約 198 / 216 kbps（44.1 / 48 kHz）。品質の設定は効きません。
- パケットはいつも3フレームにします（PipeWire の `max_frames` と同じ）。そのため FastStream だけは、エンコードの1回り（約10 ms）の終わりでも、まだ入る余地のあるパケットを送りません（`Packetizer::end_round`）。
- シンクの能力に音楽の方向と 44.1 / 48 kHz のどちらかがあるときだけ使えます。SBC より音質が低いので Auto は選びません（SBC がないシンクでも）。明示的に選んだときだけ使います。
- SEP が1つ増えるので、BTstack の `MAX_NR_AVDTP_STREAM_ENDPOINTS` を8にしました。
- 検証: 単体テスト（交渉、ローカルのSEP、フレームとパディング、モノラルの複製、3フレームのパケット）と `tools/emu`（`faststream`、`faststream-44k`、FastStream と SBC だけのシンクで Auto が SBC を選ぶ `auto-not-faststream`）。`tools/a2dp_decode` は FastStream のパケットを SBC として復号し、フレームごとの設定、パディング、1パケット3フレームを確かめます。実機（CSR のヘッドセットやドングル）では未確認です。

## 音声パイプライン・スレッド設計

音が通る経路は「取り込み → エンコード → BT送信」の3スレッドを、ロックフリーのSPSCキューでつなぎます。この経路では、メモリ確保、mutex、ログの書き出し、システムコールによる待機を一切しません。現行版の良いところ（MMCSS、約10msずつの処理、バッファの使い回し）はそのまま引き継ぎます。

| スレッド | 優先度 | 仕事 | 入力 → 出力 |
| --- | --- | --- | --- |
| capture | MMCSS "Audio" | WASAPIループバックをイベント駆動で読む。無音区間は無音フレームで埋める | WASAPI → PCMリング（f32、容量200ms） |
| encode | MMCSS "Pro Audio" | 形式変換、ダウンミックス、エンコード、メディアペイロードとRTPヘッダの組み立て。ABRの判断もここで行う | PCMリング → パケットキュー（固定長スロット） |
| bt-runloop | 通常より高い | btstackのランループ。L2CAPが送信可能になったらパケットキューから取り出して送る | パケットキュー → HCI ACL |
| usb-io | 通常より高い | WinUSBの非同期転送。イベント / ACL受信を複数本並行で待ち受ける | HCI ↔ USB |
| core（tokio） | 通常 | 状態機械、IPC、設定保存、統計の集計（4Hz）、更新チェック | コマンド / イベント |

**設計上の決まり**

- リアルタイム経路の設定変更（コーデック、ビットレート、チャンネル数）は、`arc-swap` で差し替える不変の `StreamConfig` で渡します。encodeスレッドはフレームの切れ目でだけ読み直します。
- 統計（送信数、ドロップ数、キュー深さ、送信間隔のゆらぎ、RSSI）は `AtomicU64` のカウンタに書きます。coreが4Hzで読んでUIに配信するので、経路側は待たされません。
- バッファの深さは3段階のプリセット（低遅延 / 標準 / 安定）から選べるようにします。現行の固定値は「標準」に当てます。
- ストリームが開始していない間（サスペンド中、再接続中）、encodeスレッドはエンコードを続けますが、パケットはキューに入れずに捨てます（キューのドロップには数えません。v1 の `send_media` と同じ）。BTstack のスレッドがストリームの開始・停止で共有のフラグを切り替え、開始のときはそれより前に残ったパケットを捨てます。ABR もこの間は止めます。誰も取り出さないキューが満ちて、LDAC が MQ に、LHDC V5 が最低のビットレートに下がり、再開後に古い音を送るのを防ぐためです。再接続後の `Pipeline::reset` では、残りのPCMとタイムスタンプに加えて ABR の状態もストリームの始めに戻します（LDAC は品質の設定値と新しい ABR モジュール、LHDC V5 は表の最上段 400 kbps）。
- 再接続では、デバイスがそのとき出す能力から設定を作り直します（SEID やコーデックの設定が変わっていることがあるため。v1 も能力を読み直していました）。そのコーデックが出なくなっていたら、再試行せずにエラー（`error.codec_not_supported`）で終えます。`BtHost::reconnect` / `configure_and_start` は新しい設定、能力、MTU を返し、MTU やシンクの設定（SBC の bitpool の範囲など）でエンコーダの設定が変わったときはパイプラインを作り直します。変わらなければ今のエンコーダのまま `reset` します。
- UI に出す ABR は、エンコーダが実際に ABR を動かしているか（`Encoder::abr_active`）です。LDAC の ABR モジュールの準備に失敗したときは固定の品質で流れ、ABR はオフと表示します。
- ログ: 取り込みリングのあふれ（エンコードが遅れて捨てた取り込みバッファの数）を5秒ごとに、WASAPI のデータの不連続（開始直後の500 msを除く）を増えたときに最大2秒ごとに出します（v1 と同じ）。
- 仮想デバイスを既定の出力にしたときは、切り替えに失敗しても元の既定のデバイスを最後に戻します。Windows は Console と Multimedia の片方だけ切り替えてから失敗することがあるためです。
- デバイスの切断やUSBの抜けで経路を止めるときは、coreが各スレッドに停止を伝えてjoinします。スレッドの持ち主は常に `StreamSession` 型ひとつで、`Drop` で必ず片付きます。現行版にある `Sleep` を使ったポーリングは、イベント待ちに置き換えます。
- btstackはスレッド安全ではないので、呼ぶのは bt-runloop スレッドだけにします。`bt-host` の公開型は `!Send` のハンドルを外に出さず、他スレッドからはコマンドを送って `btstack_run_loop_execute_on_main_thread` 経由で実行します。この制約は型で強制します（`btstack-sys` の `Stack` トークンは `!Send` で、ランループのスレッドの中でしか作れません。btstack を呼ぶ安全な関数はすべて `&Stack` を引数に取ります）。

## コアAPI / IPC 設計

UIはコアの状態を持たず、「スナップショットを受け取り、その後は差分イベントで更新する」だけの表示層にします。通信はユーザーごとのNamed Pipe `\\.\pipe\bluecaster-<ユーザーSID>` で、長さ付きフレームのJSONをやり取りします。パイプのDACLはそのユーザーだけに絞ります。

**接続の流れ**

1. UIが `hello { protocol: 1, client: "bluecaster-ui/2.0.0" }` を送ります。メジャー版が合わなければ、コアは理由を付けて拒否します。
2. コアが `snapshot`（状態全体とリビジョン番号）を返します。
3. 以降、コアは `event { rev, ... }` を送り続けます。リビジョンが飛んだら、UIはスナップショットを取り直します。
4. UIからの操作は `request { id, method, params }` で送り、`response { id, result | error }` で結果を受け取ります。

**状態モデル（snapshotの中身）**

| キー | 内容 |
| --- | --- |
| `adapter` | 使用中のドングル（VID/PID、チップ、ファームウェア、ドライバの状態）と状態（未接続 / 初期化中 / 準備完了 / エラー） |
| `devices[]` | ペアリング済み・検出したデバイス。名前、アドレス、対応コーデック、接続状態 |
| `stream` | 配信状態、コーデックと設定、音量、入力デバイス |
| `profiles[]` | 保存済みプロファイルと、使用中のプロファイル |
| `settings` | アプリ設定（言語、テーマ、自動起動、更新チャネルなど） |
| `update` | 更新の有無とバージョン |

**メソッドの群**

- `adapter.*`: 一覧の取得、選択、再初期化、ドライバの割り当てと解除（昇格ヘルパーを起動）
- `device.*`: 検索の開始と停止、ペアリング、接続、切断、削除
- `stream.*`: 開始、停止、コーデック変更、音量、テストトーン
- `profile.*` / `settings.*`: 読み書き、エクスポート、インポート
- `diag.*`: ログの購読（レベル指定）、btsnoopキャプチャの開始と停止、接続品質ストリームの購読
- `app.*`: 更新チェック、終了

エラーは文言ではなくコード（例: `adapter.driver_missing`）とパラメータで返し、翻訳はUI側で行います。接続品質のような高頻度のデータは、購読しているときだけ送ります。

## UI/UXデザイン

「開いた瞬間に、今どうなっていて次に何を押せばよいかが分かる」ことを最優先にします。普段の操作はタスクトレイのフライアウトで済ませ、メインウィンドウは設定と診断のときに開く場所にします。

### デザイン原則

1. **ワンクリックで音が出る**: 前回使ったプロファイルでつなぐ操作を、画面で一番大きなボタンにします。
2. **状態は常に見える**: コーデック、ビットレート、接続品質を、どの画面にも小さく出し続けます。
3. **詳しい設定は奥に置く**: 品質モード、ビット深度、最大パケットサイズのような項目は「詳細」を開いたときだけ出します。初期値のままでも一番良い音が出るようにします。
4. **失敗したら次の一手を出す**: エラーには必ず原因と直し方のボタンを付けます。例えば「ドライバが入っていません」には「ドライバを入れる」（アプリ内で割り当て）、「ファームウェアがありません」には「入手先を開く」を付けます。
5. **Windows 11に馴染む**: Micaの背景、角丸、Fluentアイコンを使い、その上に独自のアクセントを積みます。

### 画面構成

メインウィンドウ（標準 960×640、最小 720×520）は、左のナビゲーションと右の内容の2列です。ナビゲーションは幅が狭いときアイコンだけに畳みます。現行版でメニューやダイアログに散らばっていた機能は、すべてこのどこかに収めます。

| 画面 | 用途 | 主な要素 |
| --- | --- | --- |
| トレイフライアウト（360×440） | 毎日の操作 | 接続先、接続 / 切断ボタン、プロファイルの切り替え、音量、コーデックのバッジ、品質インジケータ、「アプリを開く」 |
| 初回セットアップ | 初めて使うとき | 4ステップ: ドングルを選ぶ → ドライバとファームウェアを準備（**ボタン1つでWinUSBを割り当て**） → イヤホンを探して登録 → テスト音で確認 |
| ホーム | 今の状態とよく使う操作 | デバイスの大きなカード、接続状態のリング、大きな接続ボタン、音量、取り込み元、接続品質のミニグラフ |
| プロファイル | デバイスと音質の組み合わせ | プロファイルのカード一覧。編集画面ではデバイス検索、コーデックをカードで選ぶ（音質・遅延・安定性を3本のバーで比較）、取り込み元（システム / 仮想デバイス / アプリ）、音量同期、詳細設定 |
| 診断 | 不具合の調査 | 接続品質のライブグラフ（RSSI、帯域、キュー、ドロップ）、接続の流れ11ステップ、ログ（レベルと検索で絞り込み）、HCIキャプチャ、診断情報のコピー |
| アダプタ | ドングルの管理 | 検出したアダプタ、チップの選択、ドライバとファームウェアの状態、**WinUSBの割り当て / 元のドライバに戻す**、AFHの設定 |
| 設定 | アプリ全体 | 言語、テーマ（システム / ライト / ダーク）、自動起動、更新、ヘッドホンのボタン操作、音量の記憶、開発者モード、このアプリについて |

開発者モードを有効にしたときだけ、診断画面に「受信機テスト」（現行の Peer Receiver Test）のタブが出ます。現行版と違い、開発者モードの切り替えに再起動は必要ありません。

どちらの画面（ホームとトレイフライアウト）も、一番大きい要素は接続 / 切断のボタンです。そのすぐ下に音量があり、コーデックと接続品質は小さく常に表示します。

### ビジュアル言語

- **色**: ダークを基本にして、ライトも同じトークンから作ります。アクセントは「信号」を表す青から紫へのグラデーション（`#4F8CFF` → `#8B5CF6`）で、使うのは接続ボタンと配信中のリングだけにします。状態色は良好 `#34D399`、注意 `#FBBF24`、異常 `#F87171` の3色だけです。
- **文字**: Segoe UI Variable、日本語はYu Gothic UIを使います。サイズは 12 / 14 / 16 / 20 / 28 px の5段階だけです。ビットレートなどの数字は等幅数字にして、値が変わっても揺れないようにします。
- **形**: 角丸は8 px（カードは12 px）、余白は4 px単位、カードの境界は影ではなく1 pxの線で表します。
- **動き**: 状態の変化は150〜250 msのease-outです。接続中はリングがゆっくり脈打ち、配信中はリングが音量に合わせてわずかに光ります。OSで「アニメーションを減らす」が有効なら、動きは止めます。
- **アイコン**: Fluent UI System Icons（MIT）を使います。アプリのアイコンは v1 のものをそのまま使います。

### アクセシビリティと多言語

- すべての操作をキーボードで行えるようにし、フォーカスリングを常に表示します。文字のコントラストはWCAG AA以上にします。
- Windowsのハイコントラストモード（`forced-colors`）と、100〜200%のDPIスケーリングに対応します。
- 翻訳は今と同じフラットなJSON（8言語、約410キー）をそのままフロントエンドで使います。`tools/check_lang.py` も引き続き使えます。翻訳のない文言は英語で表示します（現行版と同じ動き）。
- 言語の既定値: settings.json に `language` がないとき（新規インストール、ポータブル版の初回）は、Windows の表示言語（`GetUserPreferredUILanguages` の順）のうち8言語にある最初のものを使い、なければ英語にします。v1 はインストーラがセットアップの言語を書き込んでいました。v2 のセットアップ画面も Windows の言語で表示されるため、同じ結果になります。

## 安全性・品質

`unsafe` は `*-sys` と `win-sys` の6クレートだけに置き、CIで他のクレートに入り込んだら失敗させます。実機がなくても、エミュレータで接続から音の確認までを自動で試せる状態を保ちます。

**`unsafe` の決まり**

- `unsafe` ブロックには必ず `// SAFETY:` コメントを付けます（`clippy::undocumented_unsafe_blocks`）。
- Cから呼ばれるコールバックはすべて `catch_unwind` で囲い、panicがC側へ漏れないようにします。現行版のSEHガードの代わりです。
- Cに渡すポインタの寿命は、持ち主の型の `Drop` で管理します。btstackに登録したコールバックは、登録を解除するまで解放できない型にします。
- 依存クレートは `cargo-deny`（ライセンスと脆弱性）と `cargo-geiger`（`unsafe` の量）で監視します。

**テスト**

| 種類 | 対象 | 方法 |
| --- | --- | --- |
| 単体テスト | 形式変換、ダウンミックス、パケット化、状態機械、設定の移行 | `cargo test`。状態機械は偽の `bt-host` で試す |
| 一致テスト | Rust版SBCエンコーダ | 同じ入力でBluedroid版とビット単位で一致するか確かめる |
| プロパティテスト | リングバッファ、残りフレームの扱い、MTU境界 | `proptest` |
| ファジング | Codec Informationの解析、AVRCPの入力、IPCのフレーム、設定ファイル | `cargo-fuzz` |
| 結合テスト | 7コーデックそれぞれで、接続から音の確認まで | 今の `tools/emu/run_test.py` をH4 over TCPで動かし、`bluecaster_decode` で判定する（`--bluecaster` で v2 の `bluecaster-core.exe` を指定） |
| 障害テスト | パケットの取りこぼし、停滞、無反応のリンク、ドングルの抜け | `emu_vhci.py` の障害注入 |
| UIテスト | 主要な画面の操作 | フロントは偽のIPCで Vitest、Tauriを通した操作は WebDriver |
| 実機テスト | リリース前 | 手元のRealtekドングルとイヤホンでチェックリストを回す。`tools/linux_sink` で測定もする |

**CI**: 今は `.github` にCIがありません。GitHub ActionsのWindowsランナー（x64 と ARM64 の両方）で、`fmt` / `clippy -D warnings` / 単体テスト / `cargo-deny` / フロントの型チェックを全PRで回します。エミュレータの結合テストは、developへのマージ時と毎晩回します（x64のみ）。

## 性能・メモリ目標と計測方法

常駐中のメモリは現行版以下、音の遅延と途切れは現行版と同じかそれより良くすることを、リリースの条件にします。現行版の実測値は、一度だけ測った値です（配信中かどうかは未確認）。移行前に同じ手順で測り直して、基準値にします。

| 項目 | 現行版 | 目標 | 測り方 |
| --- | --- | --- | --- |
| 常駐中のプライベートメモリ（UIを閉じた状態） | 約61 MB（UIを含む） | 40 MB以下 | パフォーマンスカウンターを1時間の配信中に記録する。他のソフトが注入するDLLの分は除く（下の注） |
| UIを開いたときに増える分 | — | 120 MB以下。閉じたらすべて解放する | WebView2の子プロセスも含めて合計する |
| 配信中のCPU（LDAC 990 kbps） | 未計測 | 現行版以下 | ETWとWPAでスレッドごとのCPU時間を見る |
| エンコード経路のメモリ確保 | バッファの拡張時のみ | 配信開始後は0回 | テスト用のアロケータで数える |
| 送信間隔のゆらぎ（99パーセンタイル） | 未計測 | 現行版以下 | `pipeline` のカウンタと `tools/linux_sink` のジッター測定 |
| パケットロスと途切れ | 未計測 | 現行版以下 | `tools/linux_sink` で同じ場所・同じイヤホンで比べる |
| 起動から接続完了まで | 未計測 | 現行版以下 | 接続の流れ11ステップの経過時間 |

> **計測の注意（2026-10-17）**: 開発機では、待機中の `bluecaster-core.exe` のプライベートメモリ（コミット）が約60 MBと出ました。そのうち約52 MBは RivaTuner Statistics Server（MSI Afterburner のオーバーレイ）が全GUIプロセスに注入する `RTSSHooks64.dll` の書き込み可能セクションで、コア自身の分は約4 MBでした（tracing-appender の書き出しキューを既定の12.8万行から8192行にして、約4 MB減らした後の値）。プライベートメモリを比べるときは、VMMap などでモジュールごとに見て、注入されたDLLを除きます。v1の約61 MBにも同じ分が含まれている可能性があります。

メモリは主に次の3点で削ります。

1. UIを別プロセスにして、閉じている間は起動しないようにします。
2. エンコーダは使うコーデックのものだけ、配信を始めたときに作ります。
3. ログはメモリに貯めずファイルに書き出し、UIには購読中のものだけ送ります。

## 配布・更新・設定移行

v1から上書きインストールするだけで、プロファイル、ペアリング、ファームウェアがそのまま使えるようにします。イヤホンの再ペアリングは不要です。

**配布物**

- `BlueCaster-<版>-win64-setup.exe` / `BlueCaster-<版>-arm64-setup.exe`: Inno Setup で作ります（`installer/BlueCaster.iss`、`installer/build-release.ps1`）。中身はポータブル版と同じで、`bluecaster-core.exe`、`bluecaster.exe`、`bluecaster-driver.exe`、ライセンス文書（Rustのクレートは `cargo-about` で `licenses/rust-crates.txt` にまとめる）、`SOURCE.txt` です。インストールはユーザー単位だけにします（管理者権限なし、8言語のセットアップ画面）。全ユーザー向けは外しました。両方を選べるようにすると、管理者のアカウントでは起動時と更新のたびにUACの確認が出るためです。
- `BlueCaster-<版>-win64-portable.zip` / `BlueCaster-<版>-arm64-portable.zip`: 今と同じく用意します。ポータブル版では、更新機能は通知だけにします。
- WebView2ランタイムは、無ければインストーラが入れます（ブートストラッパーを同梱。Windows 11には標準で入っています）。

**v1からの乗り換え**

1. v2のインストーラは、Inno Setupで入ったv1をレジストリから見つけて、設定フォルダを残したままアンインストールします。v1が動いていれば、同じmutex（`A2DPWB_SingleInstance`）を見て終了を促します。動いているv2のコアは `bluecaster-core.exe --quit` で終了させ（配信を止めてアダプタを放す）、インストール後に起動し直します（`installer/BlueCaster.iss`）。2.0.0〜2.0.2-beta.5 の NSIS でのインストールは、ファイルを入れる前に NSIS の更新モード（`uninstall.exe /S /UPDATE`）でアンインストールし、残ったレジストリキーを消します。更新モードなので、アダプタの WinUSB、設定、ショートカット、Runキーは残ります。その後でインストールが失敗・中止されると何も入っていない状態になりますが、セットアップをもう一度実行すれば入ります。
2. 自動起動のRunキー（値の名前は v1 と同じ `BlueCaster`）が有効だったときは、`bluecaster-core.exe --background` に書き換えます。アンインストールでは、そのインストール先を指しているときだけ消します。
   - アンインストールでは、BlueCaster が WinUSB を割り当てたアダプターを Windows に戻すかを尋ねます（[ドライバ](#ドライバwinusb)の「アンインストール時の復元」）。v1 は Zadig に任せていたのでこの心配がありませんでしたが、v2 はアプリの中で割り当てるため、何もしないとアプリを消した後もドングルが Windows の Bluetooth から消えたままになり、デバイスマネージャーでしか戻せません。
3. 初回起動時、コアは設定フォルダを `backup-v1\` に丸ごと（v2の `logs\` を除いて）コピーしてから読みます。コピーは一度だけで、`settings.json` に `schema_version` が付いた後はしません（`app/src/crates/core/src/migrate.rs`）。
4. v1 が動いている間（mutex `A2DPWB_SingleInstance`。コアが3秒ごとに見て、スナップショットの `v1_running` とイベント `v1_running` で知らせる）は、ホームと初回セットアップに「BlueCaster v1 が起動しています。先に終了してください」のバナーを出します。両方が同じアダプタを使うことはできないためです。

**設定ファイルの扱い**

| ファイル | 扱い |
| --- | --- |
| `settings.json` | そのまま読む。`schema_version` を追加し、未知のキーは消さずに残す。v1の古い `max_media_payload` の移行処理も引き継ぐ |
| `profiles.json` | そのまま読む。実行時だけの `test_tone` は保存しない。v1 は名前を「<デバイス> <コーデック>」で自動で付け、位置で区別していたので同名が普通にあった。v2 は名前で探すため、同じ名前の2つ目以降に「 (2)」「 (3)」…を付ける（移行時はファイルにも書く。その後に v1 が書いたファイルも、読むときにメモリ上で付け、次の保存で書く） |
| `devices.txt` / `link_keys.txt` | 形式を変えない。リンクキーが読めれば、イヤホンは再ペアリングなしでつながる |
| `le_audio_devices.txt` / `le_audio_bonds.tlv` / `le_audio_csip.txt` | LE Audio（実験的）のデバイス、ボンド、コーディネートセット。v1 の形式のまま読み書きする |
| ファームウェアと `.pklg` | 置き場所も名前もそのまま |
| `debug.log` | v2は `logs\` に日付ごとのファイルで書く。古いものは残す |

**更新**

- `tauri-plugin-updater` を使い、GitHub Releasesに置いた署名付きの `latest.json` を参照します。マニフェストはリリースごとに添付するので、プレリリースのマニフェストは正式版のものと別になり、プレリリースのチャネルにだけ届きます。どのリリースのマニフェストを読むかは、コアの一日一回の更新チェック（GitHub API）が決めます。x64とARM64は同じマニフェストの別プラットフォームとして載せます。
- 自動チェックは今と同じく一日一回までです。「今すぐ更新」でダウンロードし、配信中なら配信を止めたときに入れます。コアは配信中・接続中の `app.quit_for_update` を断るので、更新で音が切れることはありません。
- 更新を入れる前に、UIがコアに終了を頼み、終わるのを待ってからインストーラ（passive）を起動します。インストーラが終わるとUIが起動し直し、UIがコアを起動します。
- 署名鍵はリポジトリに含めず、リリースを作る環境にだけ置きます（環境変数で渡す。手順は `installer/README.md`）。署名鍵なしのビルドとポータブル版は、リリースページを開くだけです。

## ドライバ（WinUSB）

**決定: アプリから直接、WinUSBの割り当てと解除（元のドライバに戻す）ができるようにします。ただし libwdi は使わず、Windows標準の SetupAPI で自前に実装します。** これでコピーレフトの問題は起きません。

### libwdi（LGPL）を使う場合の整理

検討の記録として残します。

- libwdi のライセンスは LGPL-3.0 です（Zadig本体は GPL-3.0）。
- LGPL のライブラリでも、**別ファイルのDLLとして動的リンク**すれば、アプリ本体（Apache-2.0 / MIT）にコピーレフトは及びません。条件は次のとおりで、満たすこと自体は難しくありません。
  - libwdi のライセンス文と著作権表示を同梱する。
  - libwdi のソース（使った版、改変していればそれも）を入手できるようにする（同梱か、書面での提供の申し出）。
  - 利用者が DLL を自分でビルドしたものに差し替えられるようにする（静的リンクやDLLの埋め込みをしない）。
  - 差し替えのための解析（リバースエンジニアリング）を禁じない。
- ただし libwdi には別の重さがあります。ドライバのINFを生成して、その場で作った自己署名の証明書でカタログに署名し、その証明書を「信頼されたルート証明機関」と「信頼された発行元」に入れる仕組みです（Zadigと同じ）。インストーラがルート証明書を入れるのは、セキュリティソフトに警戒されやすく、利用者にも説明しにくい動きです。また WDK の co-installer を同梱するビルドが必要で、cargoだけで完結させる方針にも合いません。

### 採用する方法: SetupAPI で inbox の winusb.inf を割り当てる

Windows 8 以降は WinUSB ドライバ（`winusb.inf`）が OS に標準で入っていて、Microsoft の署名付きです。デバイスマネージャーの「ドライバーの更新 → コンピューターを参照 → 一覧から選択 → ユニバーサル シリアル バス デバイス → WinUsb デバイス」と同じことを、プログラムから行います。

1. `SetupDiGetClassDevs` / `SetupDiOpenDeviceInfo` で対象ドングル（VID:PID とインスタンスID）を開く。
2. デバイスの `Device Parameters` に `DeviceInterfaceGUIDs`（BlueCaster 固有のGUID）を書き込む（Zadigが生成するINFが書いている値と同じ役割。WinUSB はこれを見てデバイスインターフェースを登録する）。
3. `SP_DEVINSTALL_PARAMS` に `DI_ENUMSINGLEINF` と `DriverPath = %SystemRoot%\INF\winusb.inf` を指定して `SetupDiBuildDriverInfoList(SPDIT_CLASSDRIVER)` を呼び、"WinUsb Device" のモデルを選ぶ（`SetupDiSetSelectedDriver`）。
4. `DiInstallDevice` で割り当てる。署名済みの inbox ドライバなので、証明書のインストールは要らない。
5. **解除（元に戻す）**: `DiUninstallDevice` でデバイスを削除してから `CM_Reenumerate_DevNode` で再列挙すると、Windows は順位の一番高いドライバ（標準の Bluetooth ドライバ `BthUSB` や、メーカーのドライバ）を選び直す。WinUSB は互換IDでの一致ではない手動の割り当てなので、再び選ばれることはない。割り当て前のドライバ（INF名とバージョン）は記録しておき、戻した後に一致しているかを確かめる。

- 管理者権限が要るので、常駐のコアは昇格させず、`bluecaster-driver.exe` を UAC 昇格（`ShellExecuteEx` の `runas`）で一時起動します。ヘルパーは引数で受け取った1台だけを処理し（アンインストール時の `restore --records` を除く）、結果を終了コードとJSONで返します。
- 割り当て中は Windows の Bluetooth からそのドングルが消えることを、実行前の確認画面で説明します。
- **アンインストール時の復元**: コアは割り当てのたびに `assign` の結果（割り当て前のドライバを含む）を設定フォルダの `driver\<インスタンスIDの英数字以外を _ にした名前>.json` に残し、「元のドライバに戻す」が成功すると消します。アンインストーラ（`installer/BlueCaster.iss` の `RestoreDrivers`。更新はアンインストーラを通らない）は、コアを終了させた後にこの記録があれば戻すかを尋ね、「はい」なら `bluecaster-driver.exe restore --records <フォルダ>` を UAC 昇格で1回だけ起動します。ヘルパーは記録ごとに `restore --previous @<記録>` と同じ処理をし、戻せたアダプタの記録を消します。終了コードは全体で一番重いもの（未昇格 3 > 失敗 1 > 未確認 5 > 抜かれている 4 > 成功 0）で、0 以外ならアンインストーラがデバイスマネージャーでの手順を案内します。決めたこと:
  - インストーラで JSON を読まずに済むよう、記録の解釈と複数台の処理はヘルパーに置く。UAC の確認は台数によらず1回。
  - 割り当てを頼まれた時点ですでに WinUSB だったアダプタ（`already_winusb`。Zadig など）は BlueCaster が変えたものではないので戻さず、記録だけ消す。読めない記録は消さずに残す。
  - サイレント（`/S`）では尋ねる相手がおらず、UAC の確認を出すべきでもないので WinUSB のまま残す（既定の答えは「いいえ」）。v1 が動いているかインストールされているときは、v1 も WinUSB を使うので既定のボタンを「いいえ」にして一言添える。
  - 抜かれているアダプタは戻せないので記録を残す（次にアプリを入れ直したときの「元のドライバに戻す」でも使える）。
  - アプリのボタンや一覧が使えない個体のために、利用者向けのセットアップガイドにデバイスマネージャーで手で割り当てる / 戻す手順を載せる（`docs/setup.md` の「By hand in Device Manager」と各言語版）。WinUSB の割り当て方によらず、コアは WinUSB のアダプタを開ける（BlueCaster のインターフェース GUID は自分の割り当てを見分けるためだけに使う）。
- **検証（フェーズ0）**: 手元の RTL8761BU で、割り当て → `nusb` で開ける → 解除 → Windows の Bluetooth に戻る、を確認します。`DiInstallDevice` が手動選択のドライバを拒む環境があれば、代わりに「`winusb.inf` を Include / Needs する小さなINF + カタログ」を作る方法を検討します（その場合は署名が要るので、libwdi と同じ問題が出ます。最後の手段として libwdi を DLL で同梱し、上の LGPL の条件を守ります）。

## Opus

Android 13 で入った A2DP の Opus ベンダーコーデック（AOSP `system/stack/a2dp/a2dp_vendor_opus*.cc`、ベンダー ID 0x000000E0 Google、コーデック ID 0x0001）を、試験的に v2 で加えます。エンコーダは libopus 1.5.2（`extern/opus`、BSD-3-Clause）で、`opus-sys` クレートが `*.mk` のソース一覧から浮動小数点版を `cc` でビルドします。SIMD 用のソースと実行時の CPU 判定は使わないので、x64 と ARM64 で同じビルドになります。

| 項目 | 値 | 根拠 |
| --- | --- | --- |
| Codec Information | 7 バイト。バイト 6 のビット 2..0 がチャネルモード（0x01 モノラル、0x02 ステレオ、0x04 デュアルモノ）、ビット 4..3 がフレーム長（0x08 10 ms、0x10 20 ms）、ビット 7 が 48 kHz | `a2dp_vendor_opus_constants.h`、PipeWire `OPUS_G_*` |
| 提供する能力 | 48 kHz、10 / 20 ms、モノラル / ステレオ（0x9B） | |
| 設定 | 48 kHz、20 ms（シンクが 10 ms だけのときは 10 ms）、ステレオ（モノラル入力でシンクがモノラルを持つときだけモノラル）。デュアルモノだけのシンクは使わない | Android の既定値 0x92。PipeWire も 20 ms とステレオを優先 |
| ビットレート | 高音質 256 kbps、標準 192 kbps、モバイル 128 kbps（ステレオ。モノラルは半分）。1 フレームがメディアペイロードに収まるよう上限を掛ける | Android `A2DP_VendorGetBitRateOpus` は 256 / 128 kbps。PipeWire（`a2dp-codec-opus-g.c`）も 1 チャネル 128 kbps |
| エンコーダ設定 | `OPUS_APPLICATION_AUDIO`、制約付き VBR（libopus の既定。Android は変更しない）、複雑度 10（libopus と PipeWire の既定。Android は 5） | `a2dp_vendor_opus_encoder.cc` |
| パケット | RTP ヘッダー + 1 バイトのメディアペイロードヘッダー（下位 4 ビットがフレーム数 = 1）+ Opus パケット 1 つ（1 フレーム）。`opus_encode` にはペイロードの残りを上限として渡す | `A2DP_BuildMediaPayloadHeaderOpus`。PipeWire のデコーダはフレーム数 1 以外を受け付けない |

Android の `tA2DP_OPUS_ENCODER_PARAMS::bitrate` は `uint16_t` なので、256000 は 59392（約 59 kbps）に切り詰められて `OPUS_SET_BITRATE` に渡っています。BlueCaster はこの不具合を真似せず、Android が意図した値を使います。PipeWire 独自の「Opus 05」は、ステレオの音楽の方向だけを別のコーデック `opus05` として後から加えました（[Opus 05](#opus-05)）。

Auto の優先順位は LHDC V5 の下、AAC の上です。256 kbps の Opus は同じビットレートの AAC と同等以上で、コーデックの遅延も小さい（20 ms のフレームと 6.5 ms の先読みで 26.5 ms。AAC-LC は約 43 ms）一方、シンク側の実装は新しく数も少ないため、実績のある LDAC / aptX 系 / LHDC V5 よりは下に置きます。

### Opus 05

PipeWire 独自の A2DP Opus（`spa/plugins/bluez5/a2dp-codec-opus.c` の `opus_05`、ベンダー ID 0x000005F1 The Linux Foundation、コーデック ID 0x1005、PipeWire 0.3.57 以降）を、試験的に `opus05`（表示名 "Opus 05"）として加えます。実装しているのは PipeWire だけなので、相手は A2DP シンクとして動く PipeWire（Linux の PC）に限られます。PipeWire のサラウンド（`opus_05_51` / `opus_05_71`）、双方向（`opus_05_duplex`）、Pro Audio（`opus_05_pro`）は実装せず、ソース側のステレオの音楽の方向だけです。

| 項目 | 値 | 根拠 |
| --- | --- | --- |
| Codec Information | 24 バイト。ベンダーヘッダーの後に 9 バイトの方向が 2 つ（main = 音楽、bidi = 戻りの音声。0 チャネルなら無し）。各方向はチャネル数、coupled stream 数、オーディオロケーション（LE 4 バイト、Bluetooth のロケーションのビット）、フレーム長のビット（0x01 2.5 ms、0x02 5 ms、0x04 10 ms、0x08 20 ms、0x10 40 ms）、ビットレート（LE 2 バイト、1024 bps 単位、0 は上限なし） | PipeWire `a2dp-codec-caps.h` の `a2dp_opus_05_t` |
| 提供する能力 | main: 2 チャネル、ロケーション FL + FR（0x3）、10 / 20 ms、上限なし。bidi: なし | PipeWire `codec_fill_caps`（`opus_05`）。フレーム長は BlueCaster のエンコーダが扱う 10 / 20 ms に絞る |
| 設定 | main: ステレオ = 2 チャネル・coupled 1・FL + FR（シンクが 1 チャネルだけならモノラル = 1 チャネル・coupled 0・ロケーション 0）、10 ms（シンクが持たなければ 20 ms）、ビットレートはシンクの上限と 312（320000 / 1024）の小さいほう（シンクが 0 なら 312）。bidi は 0。サンプリング周波数は設定に含まれないので 48 kHz で送る | PipeWire `select_config` / `set_channel_conf` の `opus_05`。10 ms を優先するのも PipeWire と同じ |
| SEP の選択 | 双方向の SEP（bidi のチャネル数が 0 でない。PipeWire の `opus_05_duplex`）は、ほかに Opus 05 の SEP がないときだけ使う | PipeWire `caps_preference_cmp`（双方向が要らなければ bidi のない SEP を優先） |
| ビットレート | Opus と同じ高音質 256 / 標準 192 / モバイル 128 kbps を、設定したビットレート（1024 bps 単位）とメディアペイロードで頭打ちにする。ABR はしない | PipeWire は 96〜320 kbps、初期値 192 kbps で、送信キューを見て変える |
| エンコーダとパケット | Opus と同じ（`codec::opus::OpusEncoder` を `CodecKind` 付きで共用）。ステレオは coupled stream 1 つ、モノラルは stream 1 つの multistream なので、パケットは普通の Opus パケットと同じ。RTP ヘッダー + 1 バイトのヘッダー（PipeWire の `rtp_payload`。フレーム数 1、分割なし）+ Opus パケット 1 つ | PipeWire の `codec_encode` は MTU を超えると分割するが、BlueCaster はペイロードに収まるように符号化するので分割しない。デコーダは分割なしならフレーム数 1 だけを受け付ける |

Auto は Opus 05 を選びません（FastStream と同じく、明示的に選んだときだけ）。相手が PipeWire に限られ、実機の PipeWire では確かめておらず、PipeWire 1.2 以降のシンクなら Android の Opus（`opus_g`）も出すからです。PipeWire がシンクのときに受けられるのは、Opus 05 が 0.3.57 以降、Android の Opus が 1.2.0 以降です（Ubuntu 24.04 の PipeWire 1.0.5 は Opus 05 だけ、Ubuntu 25.10 の 1.4.7 と 26.04 の 1.6.2 は両方）。

ストリームエンドポイントが 1 つ増えるので、BTstack の `MAX_NR_AVDTP_STREAM_ENDPOINTS` を 10 にしました。24 バイトの能力と設定は、シムのバッファ（32 バイト）に収まります。

検証: 単体テスト（能力の解析、設定、双方向の SEP の扱い、ローカルの SEP、エンコードと libopus での復号、ビットレートの上限）と `tools/emu`（PipeWire の `opus_05` と同じ能力を出すシンクでの `opus05`、44.1 kHz を指定しても 48 kHz で送る `opus05-from-44k`、SBC・AAC・Opus 05 のシンクで Auto が AAC を選ぶ `auto-not-opus05`）。`tools/a2dp_decode` は Opus 05 の設定を解析して、Opus と同じように復号します。実機の PipeWire では未確認です。

## aptX Adaptive

aptX Adaptive（ベンダー ID 0x000000D7 Qualcomm、コーデック ID 0x00AD）を `aptxad`（表示名 "aptX Adaptive"）として加えます。エンコーダは [AKS-Logic/aptx-adaptive-linux](https://github.com/AKS-Logic/aptx-adaptive-linux) の Rust だけで書かれたエンコーダ（MIT、Copyright (c) 2026 Prateek Mittal (AKS-Logic)）で、上流のコミット 25099004b2d3（2026-10-22）を `app/src/crates/aptx-adaptive` に取り込みました。上流はこれをクリーンルームでの再実装とし、数値テーブルは合法に入手した Qualcomm の出荷済みバイナリと無線のキャプチャから再構成したと説明しています（クレートの横に写した上流の `LEGAL.md`）。Qualcomm のコード、SDK、バイナリは含みません。aptX / aptX Adaptive は Qualcomm の商標で、国や地域によっては特許の対象になりえます。使ってよいかどうかは利用者の責任です（上流の注意書き）。

| 項目 | 値 |
| --- | --- |
| 形式 | 48 kHz ステレオ、24 ビット。プロファイルで 96 kHz を選び、シンクが R2 で 96 kHz を出しているときは、高音質 / 標準 / モバイルを 96 kHz で送る（上流のプラグインと同じく、構成 3 と 96 kHz の表の行。低遅延には 96 kHz がないので 48 kHz）。Auto（0）は Windows の再生デバイスのレートによらず 48 kHz。44.1 kHz を指定しても 48 kHz で取り込む |
| シンク | R1（能力の拡張がない。例: B&W Px7 S2e）と R2（R2 のビットストリーム） |
| 品質（R1） | 高音質 `hq` 427 kbps（12 ms、656 バイト）、標準 `sq` 364 kbps（14 ms。参照実装の既定値）、モバイル `mq` 318 kbps（16 ms）、低遅延 `ll` 265 kbps（10 ms、348 バイト）。R2 のシンクでは 427 / 270 / 204 / 270 kbps、96 kHz では 646（8 ms）/ 340（15 ms）/ 255（20 ms）kbps（656 バイト）。`ll` は aptX Adaptive だけが使う新しい品質キーで、ほかのコーデックは高音質として扱う |
| ABR | 高音質のとき、送信キューに 3 パケット以上ある間は 1 段下げ（モバイルより下げない。最短でも約 0.5 秒おき）、約 3 秒間キューが 1 パケット以下なら 1 段上げる（選んだ品質まで）。低遅延は ABR なし |
| パケット | メディアパケット 1 つにコーデックのパケット 1 つ。BlueCaster 自身の 12 バイトの RTP ヘッダー（BTstack は付けない）: ペイロードタイプ 96、シーケンス番号はパケットごとに +1、タイムスタンプはパケットのチャネルあたりのサンプル数（時刻ではない）、SSRC はシンクの遅延モード（HQ 0x000000A3、LL 0x000000A1）。ペイロードは 16 ビット単位でバイトを入れ替える |
| 設定 | SET_CONFIGURATION は 48 kHz（レートのビット 0x10。または 96 kHz: Android と同じ 0xA0 = 0x80 | 0x20 を送り、シンクがどちらかのビットを出していれば 96 kHz 可とみなす。issue #86）、ソースタイプ 2（ホストのエンコーダ: 再生のタイミングはシンクが決める）、ステレオ。R1 のシンクにはシンク自身の目標遅延のバイトを返し、LL0 は 80 ms 以上に上げ、HQ の遅延が 0 なら 200 ms にする。R2 のシンクにはソース側の R2 ブロックを送る |
| 能力 | Codec Information は 40 バイト。BTstack の能力のイベントでは 30 バイトに切られるが、使う値はすべてその中にある |

Auto は LHDC V5、LDAC の次に aptX Adaptive を選びます（issue #86 の順位。加えた当初は FastStream、Opus 05 と同じく、明示的に選んだときだけでした）。aptX / aptX HD / aptX LL を選んだのにシンクが aptX Adaptive しか出していないときのエラーは、aptX Adaptive、Auto、AAC、SBC を選ぶように案内します。

ストリームエンドポイントが 1 つ増えるので、BTstack の `MAX_NR_AVDTP_STREAM_ENDPOINTS` を 11 にしました。シムのバッファは 40 バイトにしました。

検証: 単体テスト、上流のビット単位で一致するテスト（Windows でも通る）、`tools/emu` の仮想シンクとの end-to-end テスト（RTP ヘッダー、パケットの大きさ、フレームの種類を確かめる。公開されたデコーダがないので音は復号しない）。実機では、EarFun Air Pro 4+ で 48 kHz が途切れずに動くと報告がありました（2.0.2-beta.6、issue #86）。96 kHz はエミュレーターだけで確認しています。上流は Linux / PipeWire で Bose QuietComfort Ultra（第1世代）と B&W Px7 S2e での 48 kHz の HQ と LL の動作を報告しています。

## ARM64

**決定: v2.0 で x64 と ARM64 の両方を出します。**

- Rust 側は `aarch64-pc-windows-msvc` ターゲットでそのままビルドできます。`nusb`、`wasapi`、`windows`、Tauri（WebView2）は ARM64 に対応しています。
- C のライブラリ（btstack、fdk-aac、libldac、libopenaptx、lhdcv5-enc、libopus）は `cc` で MSVC の ARM64 ツールチェーン（VS の「MSVC v143 ARM64 ビルドツール」）を使ってコンパイルします。x86 専用の組み込み関数を使っている箇所がないかをフェーズ1で確認します。
- CI は GitHub Actions の `windows-11-arm` ランナーでビルドと単体テストを回します。エミュレータの結合テスト（Python + Bumble）は x64 だけで回します。
- 実機での確認は ARM64 の PC と USB ドングルが要るので、ベータ版の利用者に協力してもらいます（リリース前のチェックリストに入れます）。

## 移行計画

一度に全部を入れ替えず、フェーズごとに動くものを出しながら進めます。次のフェーズには、完了条件を満たしてから進みます。

| フェーズ | 中身 | 完了条件 |
| --- | --- | --- |
| 0. 土台の試作 | workspace、`btstack-sys`、`hci-transport`（H4 over TCP / nusb）、`bt-host`（A2DP Source + AVRCP）、Rust版SBC、最小の `pipeline` と CLI | エミュレータで SBC の接続から音の確認まで通る。Rust版SBCがBluedroid版とビット単位で一致する。実機の RTL8761BU で SBC の配信ができる。WinUSB の割り当てと解除が実機で動く |
| 1. CLIアルファ | 全コーデックの `-sys` と `codec`、`audio-capture`（WASAPI 3モード）、`core` の状態機械（再接続、スリープ復帰、リンクキー再ペアリング）、設定・プロファイルの読み書き、AFH、ファームウェア、HCIキャプチャ | エミュレータで7コーデック全部が通る（`run_test.py` 7/7）。v1 の CLI の機能がそろう。ARM64 でビルドが通る |
| 2. UIベータ | `ipc`、トレイ、Tauri + SolidJS のUI（全画面）、初回セットアップ、ドライバの割り当て画面、8言語、診断画面、受信機テスト | v1 の全機能が UI から使える。UIを閉じても配信が続く。メモリ目標を満たす |
| 3. 正式版 | NSIS インストーラ、v1 からの移行、`tauri-plugin-updater`、性能の計測と調整、実機チェックリスト | 性能・メモリの目標を満たす。v1 から上書きで乗り換えられる |

フェーズ1の終わりにCLIだけのアルファ版を、フェーズ2からはUI付きのベータ版を、それぞれGitHubのプレリリースとして出します。フェーズ0で完了条件を満たせなければ、この設計のトランスポート部分を見直します。

作業は `develop-v2.x` ブランチ（旧 `develop-2.0`、`feat/le-audio` から分岐）で進め、`main` にマージします。v1 は `develop-v1.x` に残しています。初めはリポジトリ直下の `v2/` にv1のコードと並べて置いていましたが、2026-10-17 に v2 を `app/src/` へ移し、v1 のコードは削除しました。

**v1の保守**: 2026-10-17 に終了しました（当初は v2.0 を出すまで続ける予定でした）。v1 は `develop` ブランチとリリースのタグにだけ残ります（`main` は v2 です）。v1 にあった実験的な LE Audio（`feat/le-audio`）は 2026-10-18 に v2 へ移植しました（次の節）。

### LE Audio（実験的）

v1 の `feat/le-audio`（b819893）にあった実験的な LE Audio を、動作とログの文言を変えずに v2 へ移しました。設定「LE Audio（実験的）」（`le_audio_experimental`、既定はオフ）か CLI の `--le-audio` で有効になり、オフのときは LE のコードは何も動かず、HCI の初期化も LE なしの v2 と同じです（`btstack-sys` の `transport.rs` が LE の機能ビットを隠す。LE Set Scan Parameters だけはその場で応答する）。確認はエミュレータ（`tools/emu`、39 ケース）だけで、実機の LE Audio イヤホンではまだ確かめていません。

| 層 | クレート / ファイル | 中身 |
| --- | --- | --- |
| 1. FFI | `btstack-sys`（`le.rs`、`att.rs`、`events.rs`） | BTstack の `src/ble`（SM、GATT クライアント / サーバー、LE デバイス DB）、micro-ecc、`btstack_tlv_windows.c`。CIG / CIS、ISO 送信、ATT サーバーの同期読み書き |
| 1. FFI | `lc3-sys` | liblc3（`extern/btstack/3rd-party/lc3-google`、Apache-2.0）のエンコーダ |
| 2. 部品 | `le-audio` | 純粋なロジック（PACS / ASCS、プリセット、ストリーム計画、CSIP の暗号、VCP / CCP / MCP のモデル、Database Hash、SDU のジッターバッファ） |
| 2. 部品 | `hci-transport`（`usb.rs`） | HCI ISO データを ACL のバルクエンドポイントで送る（v1 / btusb と同じ） |
| 3. エンジン | `bt-host`（`le/`、`le/modules/`） | LE コア（スキャン、接続、ペアリング、GATT キュー、GATT サーバー、BAP ユニキャスト、ISO 送信）と、プロファイルのモジュール（CSIP、VCP、MCP、CCP、TMAP / GMAP） |
| 3. エンジン | `pipeline`（`le.rs`） | 取り込み → ステレオへのダウンミックス → LC3 のレートへのリサンプル → LC3 → SDU セット |
| 4. コア | `core`（`le_stream.rs`、`le_media.rs`、`devices.rs`） | LE Audio のセッション（v1 の `a2dp_service_le_audio.cpp`）、SMTC のメディア操作、`le_audio_devices.txt` |

設定フォルダには v1 と同じ `le_audio_bonds.tlv`（LE のボンド）、`le_audio_csip.txt`（コーディネートセット）、`le_audio_devices.txt`（LE Audio のデバイス）を置きます。開発者向けの説明は [le-audio-dev.md](le-audio-dev.md) にあります。

## リスクと未決事項

一番大きいリスクは、btstackとWinUSBの周りに実機で積み上げた対策を取りこぼすことです。そのため最初のフェーズで、この2つだけを先に試作して確かめます。

| リスク | 起きるとどうなるか | 対策 |
| --- | --- | --- |
| `nusb` では、一部のドングルでv1の復旧処理を再現できない | 特定のチップで初期化や通信が不安定になる | フェーズ0で、手元のドングルを使って試す。駄目なら `win-sys` でWinUSB APIを直接呼ぶ実装に切り替える |
| btstackのランループとRustのスレッドの組み合わせが複雑になる | デッドロックや、コールバック寿命の不具合 | btstackを呼ぶのは1スレッドだけという制約を型で強制する。フェーズ0でSBC配信まで通す |
| `wasapi` クレートが、アプリ単位のループバックに必要なものを全部は持っていない | アプリモードが作れない | 使うバージョンで対応を確かめる。足りない部分だけ `win-sys` で補う |
| トレイフライアウトを開くたびにWebView2の起動を待つ | 毎日の操作が遅く感じる | トレイの右クリックはコア側のネイティブメニューにして、すぐに開くようにする（接続、切断、終了）。UIプロセスは閉じてから数分は隠して残す |
| ライセンス（libopenaptxのLGPL、fdk-aac、LHDC）の条件を満たせない | 配布できない | v1と同じ条件を守る。`cargo-deny` と `THIRD_PARTY_LICENSES.md` を自動で更新する |
| Rust版SBCの出力が元の実装とずれる | 一部のイヤホンで音がおかしくなる | ビット単位の一致テストをCIで回す。一致するまではBluedroid版をFFIで使う |
| 作り直しが長引き、その間v1の不具合が直せない | 利用者が困る | v1の保守は終了した（2026-10-17）。v2をフェーズごとにプレリリースとして出し、早く乗り換えてもらう |
| inbox の winusb.inf を手動で割り当てる方法が、一部の環境で拒まれる | アプリ内でドライバを入れられない | フェーズ0で実機確認。駄目な環境では Zadig の案内に戻す。最後の手段として libwdi を DLL で同梱（LGPLの条件を守る） |
| ARM64 で一部のCライブラリが組めない／遅い | ARM64版が出せない、または音が途切れる | フェーズ1でビルドを通し、エンコード速度を計測する。問題のあるコーデックだけ ARM64 版で無効にする選択肢を残す |

**決定した事項（元の未決事項）**

- [x] アプリから直接WinUSBドライバを入れるか → **入れる。解除（元に戻す）も簡単にする。** libwdi は使わず SetupAPI で自前に実装する（[ドライバ](#ドライバwinusb)）。
- [x] ARM64版をいつ出すか → **v2.0 で x64 と一緒に出す**（[ARM64](#arm64)）。
- [x] v1の保守をいつまで続けるか → **2026-10-17 に終了し、v2 に一本化する**（当初の決定は「v2.0 を正式に出すまで」）。
- [x] フロントエンドを何にするか → **Tauri + SolidJS**（Svelte 5 から変更）。
- [x] アプリ名とアイコンをv2で新しくするか → **しない。** v1 のまま。

**決めた事項（v2.0 の前）**

- [x] `DiInstallDevice` による割り当てが使えない環境の扱い → **Zadig の案内に戻す。** 手元の UB500（RTL8761BU）では割り当てと解除の往復が通った（2026-10-18、解除後は TP-Link のドライバに戻り、割り当て後に LDAC で配信できた）。拒まれた環境では、割り当ての失敗のメッセージ（`error.driver_assign_failed`）がセットアップガイドの「Zadig を使う場合」を案内する。libwdi の同梱はしない。

**残っている未決事項**: なし。

## 改訂履歴

- 2026-10-27（サンプルレートの既定は 48 kHz、aptX Adaptive の 96 kHz のスイッチを廃止）: サンプルレートが自動（プロファイルの 0。自動選択も）のときは、デバイスが通知する最も高いレートや Windows のミックスのレートではなく 48 kHz を求め、各コーデックがデバイスの通知するレートから選ぶ（48 kHz がなければ 44.1 kHz など）。96 kHz は遅延と相性の問題が増えるわりに聞き分けられる差がなく、Windows のミキサーも通常 48 kHz のため（issue #86 第9回）。96 kHz はコーデックの設定で選んだときだけ。aptX Adaptive の 96 kHz が実機で確認できたので、開発者の「aptX Adaptive を 96 kHz で」（`dev_overrides.aptxad_96k`、CLI `--aptxad-96k`）をやめ、96 kHz はデバイスが通知していれば選べる。コーデックの設定は aptX Adaptive の 96 kHz を、デバイスの通知が分からないときも含め、通知したデバイスにだけ出す（Bose QC Ultra の 0x51 には出さない）。「コーデック」画面のスイッチは「対応しているコーデックのみ表示」（既定でオン）。カードのドラッグでは、持ち上げたカードがポインターについて動き、ほかのカードは新しい位置へ滑って移り（FLIP）、離したカードは自分の位置へ収まる（prefers-reduced-motion では動かさない）。
- 2026-10-26（デバイスごとのコーデックの順番）: 開発者モードの「自動選択の順番…」と settings.json の `auto_order` をやめ、「コーデック」画面で自動選択がオフのとき、デバイスごとにコーデックのカードをドラッグ（またはカード上で Alt＋矢印キー）で並べ替えられるようにした（issue #86 第8回）。順番はプロファイルの `codec_order`（コーデックのキー、空なら音質順）に保存する。自動選択がオンのときは既定の順番で読み取り専用にし、カードや「…」をクリックすると自動選択をオフにするか尋ねる。あわせて、選んだコーデックにデバイスが対応していないときのフォールバックを、その下の次のコーデックではなく、デバイスの順番で最初に対応するコーデック（既定なら最も音質の高いもの）にした（Opus を選んだ EarFun では aptX ではなく aptX Adaptive）。Auto とフォールバックは FastStream と Opus 05 を選ばない。順番だけを変えた保存では、コーデックが変わらない限り使用中のストリームを接続し直さない。古い `auto_order` は読み込んでも使わない（ファイルにはそのまま残る）。各デバイスの名前の横に、接続中は緑の点を出す。
- 2026-10-26（自動選択の順番）: 「コーデック」画面に「自動選択の順番…」を加えた（issue #86）。ポップアップでコーデックをドラッグ（または矢印ボタン）で並べ替え、「音質順に戻す」で既定に戻せる。settings.json の `auto_order`（コーデックのキー、空なら音質順）に保存し、コアは `codec::AutoOrder` で Auto の選択と、選んだコーデックのフォールバックの両方にこの順番を使う。カードもこの順番で並べる。使用中のプロファイルが Auto かフォールバック中なら接続し直す。
- 2026-10-26（自動選択をデバイス共通に）: 「コーデック」画面の「対応するコーデックのうち最も音質の高いものを自動で選ぶ」を、デバイスごとのスイッチ（プロファイルの codec "auto"）から、画面上部のすべてのデバイス共通の設定（settings.json の `auto_codec`、既定はオン）にした（issue #86）。オンのときは各プロファイルを Auto（Auto の既定の設定）で配信し、デバイスごとに選んだコーデックはプロファイルに残す。オフのときは選んだコーデックを使い、まだ選んでいないデバイスは Auto のまま。schema 5 への移行では、いずれかのプロファイルがコーデックを選んでいればオフで始める（v1 のフォルダも同じ）。使用中のストリームのコーデックが変わるときは接続し直す。
- 2026-10-25（実験的コーデックの設定）: 「コーデック」画面に「実験的コーデックを有効にする」（settings.json の `experimental_codecs`、既定はオン。CLI では `--no-experimental` でオフ）を加えた（issue #86）。実験的コーデックは aptX HD、aptX LL、Opus、Opus 05、FastStream で、カードに「実験的」のタグを付ける。オフのときはそのカードを隠し、Auto もフォールバックもそれらを選ばず、それらを選んでいたデバイスは Auto の順位で次に低い、デバイスが出しているコーデックを使う（画面にその旨を出す）。使用中のストリームのコーデックが変わるときは接続し直す。あわせて、EarFun Air Pro 4+ での 48 kHz の動作報告を受けて aptX Adaptive を試験的な扱いから外した（96 kHz はエミュレーターでだけ確認）。
- 2026-10-23（NSIS 版は先にアンインストール）: 2.0.2-beta.5 までの NSIS でのインストールを引き継ぐのをやめ、ファイルを入れる前に NSIS のアンインストーラを更新モード（`/S /UPDATE`）で実行するようにした（issue #86）。古いインストーラの登録やファイルに手を入れないほうが、後で困らない。失敗・中止すると何も残らないが、まだ利用者が少なく、セットアップをもう一度実行すれば済むので受け入れる（リリースノートに書く）。
- 2026-10-22（インストーラを Inno Setup に）: Tauri の NSIS バンドラから Inno Setup 6（`installer/BlueCaster.iss`）に移した（issue #86）。NSIS で困っていたことはないが、(1) 台本が人に読みやすい（NSIS のフックは Tauri の生成する `installer.nsi` の流れと `$0`〜`$9` のレジスタを追わないと読めず、`System::Call` で構造体を組んでいた）、(2) 開発が活発で将来性がある、(3) 利用者とインストール済みの環境が少ない今のうちに移すほうが楽、の3点で決めた。`tauri build --no-bundle` で `bluecaster.exe` だけを作り、ポータブル版のフォルダをそのまま Inno Setup に渡す。更新は引き続き `tauri-plugin-updater`（署名は `tauri signer sign --app-version`）で、プラグインが付ける `/UPDATE` に `/SILENT /NORESTART` を足す（`installMode: basicUi`）。beta.5 以前の更新機能が付ける NSIS の `/P` では、ページをすべて飛ばして進行状況だけを出す（セットアップの .exe はローダーが開いたままなので、`/SILENT` で起動し直すことはできなかった）。NSIS でのインストールは、ファイルを入れ終えてからフォルダを引き継ぐ（先にアンインストールすると、失敗や中止で何も残らないため）。インストール済みの判定は `uninstall.exe` から `unins000.exe` に。v1 とは別の AppId。中国語（簡体・繁体）の翻訳は Inno Setup 6 に同梱されないので `installer/languages/` に置く。
- 2026-10-24（aptX Adaptive の 96 kHz）: プロファイルで 96 kHz を選び、シンクが R2 で 96 kHz を出しているとき、高音質 / 標準 / モバイルを 96 kHz で送るようにした（issue #86）。R1 のシンクと低遅延は 48 kHz のまま。エンコーダの呼び出しは 2 ms（96 kHz では 192 フレーム）。Codecs の設定でサンプリングレートに 96 kHz を加え、ビット深度は aptX Adaptive と aptX HD で 24 ビットと表示する（常に 24 ビットで送っているのに 16 ビットと出ていた）。上流（f8a0908、2026-10-23）を見直したが、エンコーダのクレートは変わっていない。上流のプラグインでの変更のうち、ソースタイプ 0（シンクのリンクの時計に合わせたタイムスタンプ。ヘッドホンによってはこちらでないと切れる）はリンクの時計の値が要るので移していない。仮想シンクでのみ確認、実機では未確認。
- 2026-10-22（aptX Adaptive）: AKS-Logic/aptx-adaptive-linux の Rust のエンコーダ（MIT、上流のコミット 25099004b2d3）を `app/src/crates/aptx-adaptive` に取り込み、aptX Adaptive を試験的に `aptxad` として追加した。R1 と R2 のシンク、48 kHz ステレオ、品質は高音質 / 標準 / モバイル / 低遅延（新しいキー `ll`）、高音質では ABR。Auto は選ばない。ストリームエンドポイントは 11 個になった。実機のヘッドホンでは未確認。[aptX Adaptive](#aptx-adaptive)を参照。
- 2026-10-18（ブランチ名）: `develop-2.0` を `develop-v2.x`、v1 の `develop` を `develop-v1.x` に改名した。CI のプッシュ時のテストは `develop-v2.x` と `main` で動く。
- 2026-10-18（未決事項の解消、ARM64）: `DiInstallDevice` が使えない環境では Zadig の案内に戻すと決めた（割り当ての失敗に専用の文言 `error.driver_assign_failed`、8言語）。実機（UB500）で割り当て → 解除 → 割り当ての往復を確認した。x64 の PC での ARM64 のビルドには、rustls の `ring` のために Visual Studio の clang が要る（`installer/build-release.ps1` が探して PATH に足す）。手元で ARM64 の3つの exe が組めることを確かめた（ARM64 の実機では未確認）。
- 2026-10-18（Opus 05）: PipeWire 独自の A2DP Opus（ベンダー 0x000005F1 / 0x1005、`opus_05`）のステレオの音楽の方向を、試験的に `opus05` として追加した。Auto は選ばない。エンコーダは Opus のものを共用し、設定したビットレートで頭打ちにする。ストリームエンドポイントは 10 個になった。サラウンド、双方向、Pro Audio は未実装。[Opus 05](#opus-05)を参照。
- 2026-10-18（アンインストール時のドライバ復元）: v1 との差の洗い出しで見つかった「v2 をアンインストールしてもアダプタが WinUSB のまま残る」を直した。アンインストーラが確認のうえで `bluecaster-driver.exe restore --records` を1回昇格して、記録のあるアダプタを Windows に戻す。8言語の文言、セットアップガイドにデバイスマネージャーでの手順。[ドライバ](#ドライバwinusb)の「アンインストール時の復元」を参照。
- 2026-10-18（v1 との差の修正: UI）: v1 の同名プロファイルに「 (2)」「 (3)」…を付ける（移行時と読み込み時）。settings.json に言語がないときは Windows の表示言語を使う。使用中のプロファイルを保存すると新しい設定でつなぎ直す。「システムトレイに最小化」がオフならメインウィンドウを閉じるとアプリ全体が終了する。v1 が動いている間はホームとセットアップにバナー（スナップショットの `v1_running`）。このアプリについてにサードパーティの一覧を戻し、ライセンスはプログラム横の `licenses` フォルダ（開発ビルドではリリースタグのページ）を開く。Ctrl+Shift+D で診断。トレイのツールチップに配信中のデバイス名。接続フローの列見出し。プロファイル一覧に品質とビットレート。
- 2026-10-18（ストリームの v1 との差）: サスペンド中・再接続中はパケットを捨てて ABR を止め、再接続後に ABR を戻すようにした。再接続ではデバイスの能力から設定を作り直し、MTU などが変わればパイプラインを作り直す。SBC XQ の設定を拒否するシンクでは SBC で接続し直す。UI の ABR はエンコーダが実際に動かしているかを表示する。既定の出力デバイスは切り替えの一部失敗でも戻す。取り込みリングのあふれと WASAPI の不連続をログに出す。[音声パイプライン・スレッド設計](#音声パイプラインスレッド設計)の「設計上の決まり」と [v2 で加えたコーデック](#v2-で加えたコーデック)の SBC XQ を参照。

- 2026-10-18（LE Audio）: v1 の実験的な LE Audio を v2 に移植した（クレート `le-audio` と `lc3-sys`、`btstack-sys` の LE、`bt-host` の `le/`、`pipeline` の `le.rs`、`core` の `le_stream.rs`、UI の設定・プロファイル・デバイス一覧・診断）。既定はオフで、実験的な扱いは v1 と同じ。オフのときの HCI コマンドは LE なしのビルドと同じ（LE Set Scan Parameters をトランスポートが応答する点を除く）。[LE Audio（実験的）](#le-audio実験的)を参照。「今回やらないこと」の LE Audio は「正式対応」に書き換えた。

- 2026-10-17（Opus）: Android の A2DP Opus（ベンダー 0x000000E0 / 0x0001）を試験的に `opus` として追加した。エンコーダは libopus 1.5.2（`opus-sys`）。パラメータとその根拠、Auto の位置（LHDC V5 の下、AAC の上）は[Opus](#opus)に記した。ストリームエンドポイントは 9 個（SBC、AAC、aptX、aptX HD、aptX LL、LDAC、LHDC V5、FastStream、Opus）になり、BTstack の静的プールは 1 個あたり約 0.6 KB（`avdtp_stream_endpoint_t` 528 バイトとシムの能力 / 設定 64 バイト）増える。

- 2026-10-17（FastStream）: CSR FastStream の音楽の方向（ソース側）を `faststream` として追加した。Auto は選ばない。音声のバックチャネルは未実装。[v2 で加えたコーデック](#v2-で加えたコーデック)を参照。

- 2026-10-17（SBC XQ）: SBC XQ（dual channel、高 bitpool）を独立したコーデック `sbcxq` として追加した。交渉、品質ごとの bitpool、Auto の位置は[v2 で加えたコーデック](#v2-で加えたコーデック)に記した。

- 2026-10-17（v2 に一本化）: v1 の保守を終了し、develop-2.0 から v1 のコード（C++ / wxWidgets、Inno Setup のインストーラ、CMake のビルド、実験的な LE Audio）を削除した。v2 を `v2/` から `app/src/` に移し、`core` / `ui` / `driver` / `crates` に分けた。Cargo workspace はリポジトリ直下、インストーラの台本は `installer/`、この設計書は `docs/v2-design.md`。デコーダの開発ツール `tools/a2dp_decode` は単独の CMake プロジェクトにした。
- 2026-10-17（フェーズ3）: NSISのインストールはユーザー単位だけにした（全ユーザー向けを選べるとUACが毎回出るため）。セットアップ画面は8言語。更新のマニフェストはリリースごとに添付し、コアの更新チェックがURLを決める。コアに `--quit`、IPCに `app.quit_for_update`（配信中は断る）を追加。v1からの移行（`backup-v1\`、`schema_version`、古い `max_media_payload` をプロファイルへ）をコアの初回起動に入れた。待機中メモリの計測結果と、注入DLLを除く計り方を追記。開発用に `BLUECASTER_INSTANCE`（別のパイプとmutexで2つ目のコアを動かす）。
- 2026-10-17（実装に合わせた変更）: TypeScriptの型生成は `specta`（2.0 がまだ RC）ではなく `ts-rs` を使う。Named Pipe は tokio の非同期パイプ（同じ接続で読み書きを同時に行うため。同期ハンドルでは読み取り中の書き込みが待たされる）で、ユーザーだけに絞るDACLは `win-sys::pipe` で付ける。トレイは `tray-icon` を `tao` のイベントループで動かす（右クリックのネイティブメニュー、左クリックでUIのフライアウト）。`bluecaster-core.exe` は GUI サブシステムで、`--cli` のときだけ呼び出し元のコンソールにつなぐ。
- 2026-10-17: 未決事項の決定を反映。フロントエンドを SolidJS に変更。ARM64 を v2.0 の対象に追加。アプリ内でのWinUSB割り当て / 解除（SetupAPI、昇格ヘルパー `bluecaster-driver.exe`）を追加し、libwdi を使わない理由を記録。作業ブランチを `feat/v2-rust` から `develop-2.0`（`feat/le-audio` から分岐）に変更。FFI を bindgen から手書き + Cシムに変更（libclang を不要にするため）。`core` クレートのパッケージ名を `bluecaster-core` に。移行計画の4フェーズと完了条件を表にした。
