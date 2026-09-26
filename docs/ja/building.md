---
title: ビルド
layout: default
parent: 日本語
nav_order: 3
---

# ソースからビルド
{: .no_toc }

## 目次
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## 必要なもの

- **Visual Studio 2022 以降**（「C++ によるデスクトップ開発」ワークロード）
- **CMake** 3.16 以上
- **Git**（サブモジュールおよび wxWidgets FetchContent 用）

## クイックビルド

```bash
git clone --recursive https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge.git
cd A2DP-Windows-Bridge
cmake -B build -A x64
cmake --build build --config Release
```

実行ファイルは `build/app/Release/A2DPWB.exe` に出力されます。

{: .note }
初回ビルドは CMake FetchContent が wxWidgets (v3.2.6) をダウンロード・コンパイルするため、数分かかります。

## `--recursive` なしでクローン済みの場合

```bash
git submodule update --init --recursive
```

## プロジェクト構成

```
A2DP-Windows-Bridge/
├── app/                    アプリケーションソース
│   ├── src/                C++ ソースファイル
│   ├── lang/               ローカライゼーション (en.json, ja.json)
│   ├── resources/          アイコン、マニフェスト、リソーススクリプト
│   └── CMakeLists.txt      アプリビルド設定
├── compat/                 AOSP コード用 MSVC 互換ヘッダー
├── docs/                   ドキュメント（このサイト）
├── extern/                 サードパーティライブラリ（git サブモジュール）
│   ├── btstack/            BTstack Bluetooth スタック
│   ├── libldac/            AOSP LDAC エンコーダー
│   ├── libopenaptx/        aptX / aptX HD / aptX LL エンコーダー
│   ├── fdk-aac/            Fraunhofer AAC エンコーダー
│   └── json/               nlohmann/json（ヘッダーオンリー）
├── tools/
│   ├── a2dp_decode/        a2dpwb_decode: HCI キャプチャ（.pklg）のメディアストリームを検証・デコード
│   ├── emu/                仮想 Bluetooth シンクを相手にしたエンドツーエンドテスト（Python）
│   └── linux_sink/         a2dpwb_sink: Linux で動く測定用 A2DP 受信機（Python、Bumble）
├── CMakeLists.txt          ルートビルド設定
└── build.bat               ビルドヘルパースクリプト
```

## ビルドオプション

| CMake オプション | 既定値 | 説明 |
|:-----------------|:-------|:-----|
| `LDAC_SOFT_FLOAT` | `OFF` | libldac にソフトウェア浮動小数点を使用 |
| `A2DPWB_BUILD_TOOLS` | `ON` | 開発者向けツール（`a2dpwb_decode`。[使い方](usage#verify-stream)を参照）をビルド。リリースパッケージには含まれません |

```bash
cmake -B build -A x64 -DA2DPWB_BUILD_TOOLS=OFF
```

## 実機なしでのテスト（tools/emu）

`tools/emu` は、アダプターやヘッドホンなしで、A2DPWB から同じ PC 上の仮想 Bluetooth シンクへ全コーデックをストリーミングしてテストします。Windows だけで完結し、ビルドに必要なものに加えて **Python 3.11 以降** が必要です。

```bash
python tools/emu/setup_env.py     # 初回のみ: バージョン固定のパッケージで tools/emu/.venv を作成
python tools/emu/run_test.py      # Release ビルド後に実行。全コーデックで約 35 秒
```

仕組み:

- `emu_sink.py` は、[Bumble](https://github.com/google/bumble) の仮想コントローラ 2 台を同じプロセス内の仮想リンクでつなぎます。1 台は A2DPWB 用に H4 over TCP（`127.0.0.1`）で公開し、もう 1 台は Bumble の A2DP シンクが使います。シンクは SBC・AAC・aptX・aptX HD・aptX LL・LDAC を提供し、HCI 通信を記録します。
- `run_test.py` は、コーデックごとに開発用オプション付きで `A2DPWB.exe --cli` を実行します。`--hci-tcp`（WinUSB の代わりに仮想コントローラ）、`--test-tone`（システム音声の代わりに左 1 kHz / 右 1.5 kHz）、`--duration`、`--hci-capture` です。環境変数 `A2DPWB_CONFIG_DIR` で A2DPWB に別の設定フォルダーを使わせるので、仮想シンクとのペアリングが実際のリンクキーに影響することはありません。
- 送信側と受信側の HCI キャプチャを `a2dpwb_decode` で検証します（受信側は `--received`）。

次をすべて満たしたコーデックを合格とします。

- 指定したコーデックで交渉された
- 送信側・受信側とも `a2dpwb_decode` が問題を検出しない
- A2DPWB が送信したメディアパケットとフレームをシンクがすべて受信し、指定した時間の 90% 以上の音声が送信された
- LDAC 以外（オープンソースのデコーダーがないため）は、デコードした音声が両側で完全に一致し、テストトーンである

| オプション | 説明 |
|:-----------|:-----|
| `--codecs sbc aac ...` | 指定したコーデックだけをテスト（`sbc aac aptx aptxhd aptxll ldac`。既定: すべて） |
| `--duration <秒>` | コーデックごとのストリーミング時間（既定 5 秒） |
| `--max-packet <バイト数>` | A2DPWB に渡す最大メディアパケットサイズ（[使い方](usage)を参照） |
| `--build-dir`、`--config` | `A2DPWB.exe` / `a2dpwb_decode.exe` の場所（既定 `build`、`Release`） |
| `--out <フォルダー>` | 出力先（既定 `tools/emu/out`） |

コーデックごとに `tools/emu/out/<codec>/` へ、キャプチャ（`a2dpwb.pklg`、`sink.pklg`）、ログ、`a2dpwb_decode` のレポート、デコードした WAV を出力します。すべて合格すると終了コード 0 になります。

{: .note }
仮想リンクには電波がありません。エンコード、パケット化、AVDTP / L2CAP のシグナリング、独立した Bluetooth スタック（Bumble）との相互接続性は検証できますが、電波の状態、実機でのタイミング、実際のヘッドホンでの再生は検証できません。Bumble 関連のパッケージはテスト用で、A2DPWB には含まれません（THIRD_PARTY_LICENSES.md §13 を参照）。

`emu_vhci.py` は同じことを Linux の Bluetooth スタックに対して行います。Linux 上で `/dev/vhci` を通して仮想アダプターを作るので、測定用受信機 `tools/linux_sink` がそれを実機のアダプターと同じように使い、実機なしでテストできます。メディアパケットを欠落させたり一時的に止めたりもできます（`--drop`、`--stall`）。`tools/linux_sink/README.md` を参照してください。

## 依存関係

すべての依存関係は git サブモジュールとして含まれるか、ビルド時に取得されます。手動インストールは不要です。

| ライブラリ | 取得方法 | 固定バージョン | ライセンス |
|:-----------|:---------|:---------------|:-----------|
| BTstack | git サブモジュール | v1.8.1-6-g5bc5cbdbe | BTstack License（非商用条項付きの BSD-3-Clause 類似ライセンス） |
| Bluedroid SBC コーデック | BTstack に同梱（`3rd-party/bluedroid`） | BTstack に準拠 | Apache-2.0 |
| rijndael | BTstack に同梱（`3rd-party/rijndael`） | BTstack に準拠 | パブリックドメイン |
| libldac (AOSP) | git サブモジュール | android-15.0.0_r36-4-geeee1a3 | Apache-2.0 |
| libopenaptx | git サブモジュール | 0.2.0（更新しないこと） | LGPL-2.1+ |
| fdk-aac | git サブモジュール | v2.0.3-158-gd8e6b1a | FDK AAC License |
| nlohmann/json | git サブモジュール（ヘッダーオンリー） | 3.12.0 | MIT |
| wxWidgets | CMake FetchContent | v3.2.6 | wxWindows Library Licence 3.1 |
| zlib / libpng / nanosvg | wxWidgets 内蔵のコピー | 1.2.13.1 / 1.6.37 / wxWidgets に準拠 | zlib / PNG Reference Library License v2 / zlib |

{: .warning }
BTstack をリンクしているため、A2DPWB のソースコード自体は MIT ライセンスであっても、ビルドした `A2DPWB.exe` は個人的かつ非商用の目的でのみ使用・再配布できます。

ライセンスの全文は [THIRD_PARTY_LICENSES.md](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/THIRD_PARTY_LICENSES.md) を参照してください。
