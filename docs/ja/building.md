---
title: ビルド
layout: default
parent: ホーム
nav_order: 3
---

# ソースからビルド
{: .no_toc }

## 目次
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## 必要なもの {#requirements}

- **Rust**（stable、1.90 以降）。[rustup](https://rustup.rs/) で入れます
- **Visual Studio 2022 以降**（Build Tools でも可）と「C++ によるデスクトップ開発」ワークロード。C のライブラリ（BTstack、コーデック）を MSVC でコンパイルします
- UI 用に **Node.js** 22 と **pnpm**（`corepack enable` で pnpm が使えます）
- **Git**（サブモジュールの取得）
- `build.ps1` / `build.bat` 用に **PowerShell 7**（`pwsh`。`winget install Microsoft.PowerShell`）
- チェック（`tools/check_lang.py`）と[エミュレーターテスト](#emu)用に **Python 3.11 以降**
- ARM64 版をビルドするとき: Visual Studio のコンポーネント「MSVC v143 - VS 2022 C++ ARM64 ビルド ツール」と、x64 の PC では「Windows 用 C++ Clang コンパイラ」（rustls の `ring` が ARM64 のアセンブリを clang で組むため。`installer/build-release.ps1` は Visual Studio の clang を自分で探します）、それに `rustup target add aarch64-pc-windows-msvc`

## クイックビルド {#quick-build}

```bash
git clone --recursive https://github.com/SeiyaFunaokaJP/BlueCaster.git
cd BlueCaster
pwsh ./build.ps1
```

エクスプローラーで `build.bat` をダブルクリックしても同じです（`build.ps1` を実行し、最後にキー入力を待ちます。`build.bat -Test` のようにスイッチも渡せます）。`build.ps1` は最初に Rust、Node.js、pnpm、サブモジュールがそろっているかを確かめ、`node_modules` がなければ `pnpm install` を実行し、ウィンドウのページをビルドしてから 3 つのプログラムを `target/release/` に作ります。`bluecaster-core.exe`（常駐するコア: Bluetooth、音声、トレイ）、`bluecaster.exe`（ウィンドウ）、`bluecaster-driver.exe`（WinUSB ドライバーの割り当てと復元をする、管理者権限のヘルパー）です。3 つは同じフォルダーに置いてください。`bluecaster-core.exe --cli` ならウィンドウなしで配信できます。

スイッチ: `-Debug`（開発プロファイル、`target/debug/`）、`-Test`（続けて「テスト」のチェック）、`-Decoder`（デコーダーツールも）、`-Emu`（[エミュレーターテスト](#emu)も。`-Emu -Codecs sbc,ldac` でこれらのコーデックだけをテスト）、`-Release -Arch x64|arm64`（リリーススクリプトのみ。「リリースパッケージ」を参照）。

**手順を分けて行う場合**（`build.ps1` がしていること。`bluecaster.exe` はウィンドウのページを埋め込むので、ページを先にビルドします）:

```bash
cargo build --release -p bluecaster-core-exe -p bluecaster-driver
cd app/src/ui
pnpm install --frozen-lockfile
pnpm build
cd ../../..
cargo build --release -p bluecaster-ui
```

## `--recursive` なしでクローン済みの場合 {#if-you-already-cloned-without---recursive}

```bash
git submodule update --init --recursive
```

## プロジェクト構成 {#project-structure}

```
BlueCaster/
├── app/
│   ├── src/
│   │   ├── core/           bluecaster-core.exe: resident core, tray, IPC server, --cli
│   │   ├── ui/             bluecaster.exe: Tauri 2 + SolidJS (pnpm dev runs it in a browser with a mock core)
│   │   ├── driver/         bluecaster-driver.exe: WinUSB assign / restore (elevated)
│   │   └── crates/         Libraries: btstack-sys, hci-transport, bt-host, codec, the codec -sys crates,
│   │                       pipeline, audio-capture, win-sys, core (bluecaster-core), ipc, le-audio, lc3-sys,
│   │                       aptx-adaptive (aptX Adaptive encoder)
│   ├── lang/               Localization (en.json is the source; tools/check_lang.py checks the others)
│   └── resources/          Icon and manifest
├── Cargo.toml              Cargo workspace
├── installer/              Release script, Inno Setup script, updater notes (installer/README.md)
├── compat/                 MSVC compatibility headers for the AOSP code
├── docs/                   Documentation (this site) and the design document (v2-design.md)
├── extern/                 Third-party C libraries (git submodules)
│   ├── btstack/            BTstack Bluetooth stack
│   ├── libldac/            AOSP LDAC encoder
│   ├── libopenaptx/        aptX / aptX HD / aptX LL encoder
│   ├── fdk-aac/            Fraunhofer AAC encoder
│   ├── lhdcv5-enc/         LHDC V5 encoder
│   └── opus/               libopus (Opus encoder)
└── tools/
    ├── a2dp_decode/        bluecaster_decode: checks / decodes the media stream in an HCI capture (.pklg)
    ├── emu/                End-to-end test against a virtual Bluetooth sink (Python)
    └── linux_sink/         bluecaster_sink: measuring A2DP receiver on Linux (Python, Bumble)
```

## テスト {#tests}

```bash
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cd app/src/ui && pnpm exec tsc --noEmit && pnpm exec vitest run
python tools/check_lang.py
```

## リリースパッケージ {#release-packages}

```powershell
pwsh installer/build-release.ps1 -Arch x64
```

は、インストーラー（`BlueCaster-<version>-win64-setup.exe`）、ポータブル版の zip、シンボルの zip（`.pdb` ファイル。クラッシュダンプ用）、更新用のマニフェストを `target/dist` に書き出します。Inno Setup 7.1.0 が必要です（`winget install JRSoftware.InnoSetup.7 --version 7.1.0`）。アプリ内更新の署名などの詳細は `installer/README.md` にあります。

## 実機なしでのテスト（tools/emu） {#emu}

`tools/emu` は、同じ PC 上の仮想 Bluetooth シンクへ BlueCaster からすべてのコーデックを配信します。アダプターもヘッドホンも要りません。Windows 専用で、**Python 3.11 以降**と、デコーダーツール用の **CMake**（Visual Studio に同梱）が必要です。

```bash
python tools/emu/setup_env.py                       # once: creates tools/emu/.venv with pinned packages
cmake -S tools/a2dp_decode -B build/a2dp_decode     # once: the decoder tool
cmake --build build/a2dp_decode --config Release
cargo build --release -p bluecaster-core-exe
python tools/emu/run_test.py                        # every codec and negotiation case
```

仕組み:

- `emu_sink.py` は、プロセス内のリンクでつないだ 2 つの [Bumble](https://github.com/google/bumble) 仮想コントローラーを動かします。1 つは BlueCaster 向けに H4 over TCP（`127.0.0.1`）で公開します。もう 1 つは、SBC（SBC XQ 対応）、AAC、aptX、aptX HD、aptX LL、LDAC、LHDC V5、FastStream、Opus、Opus 05、aptX Adaptive を提示する Bumble の A2DP シンクが使い、その HCI の通信を記録します。
- `run_test.py` はコーデックごとに `bluecaster-core.exe --cli` を開発用のオプション付きで動かします: `--hci-tcp`（WinUSB の代わりに仮想コントローラー）、`--test-tone`（システムの音の代わりに左 1 kHz / 右 1.5 kHz）、`--duration`、`--hci-capture`。環境変数 `BLUECASTER_CONFIG_DIR` で BlueCaster に別の設定フォルダーを使わせるので、仮想シンクとのペアリングが本物のリンクキーに触れることはありません。
- 最後に、両方の HCI キャプチャを `bluecaster_decode` で調べます（シンク側は `--received`）。

次のすべてを満たしたコーデックが合格です。

- 指定したコーデックでネゴシエーションした
- 送信側と受信側のどちらでも `bluecaster_decode` が問題を見つけない
- BlueCaster が送ったメディアパケットとフレームをシンクがすべて受け取り、指定した長さの音声の 90% 以上を送った
- LDAC、LHDC V5、aptX Adaptive 以外（オープンソースのデコーダーがないため）: デコードした音声が両側で一致し、テストトーンである

| オプション | 説明 |
|:-------|:------------|
| `--codecs sbc aac ...` | これらのコーデック（`sbc sbcxq aac aptx aptxhd aptxll ldac lhdcv5 faststream opus opus05`）またはネゴシエーションのケース（サンプルレート、低い bitpool、SBC のフレーム設定、aptX Adaptive、...。`-h` で一覧）だけを試す。既定: すべて |
| `--le-audio [case ...]` | 代わりに LE Audio（実験的）のケースを実行（既定: すべて） |
| `--scan [case ...]` | 代わりにデバイス検索のケースを実行（`eir`、`name-request`） |
| `--link [case ...]` | 代わりにリンクのケースを実行（`disconnect`、`device-link`、`device-a2dp`、`device-stream`、`restart-on-link`） |
| `--duration <s>` | コーデックごとの配信時間（秒。既定 5） |
| `--max-packet <bytes>` | BlueCaster に渡す（最大メディアパケットサイズ。[使い方](usage)を参照） |
| `--bluecaster <exe>` | テストするコア（既定 `target/release/bluecaster-core.exe`） |
| `--decoder <exe>` | デコーダーツール（既定 `<build-dir>/a2dp_decode/<config>/bluecaster_decode.exe`） |
| `--build-dir <dir>`、`--config <cfg>` | デコーダーツールをビルドした場所（既定 `build`、`Release`） |
| `--base-port <port>` | 最初の TCP ポート（既定 9101。LE Audio のケースは +100）。複数のテストを同時に動かすときは変える |
| `--out <dir>` | 出力先のフォルダー（既定 `tools/emu/out`） |

コーデックごとに、`tools/emu/out/<codec>/` にキャプチャ（`bluecaster.pklg`、`sink.pklg`）、ログ、`bluecaster_decode` のレポート、デコードした WAV ファイルが残ります。すべてのコーデックが合格すると終了コードは 0 です。

{: .note }
仮想リンクには無線がありません。エンコード、パケット化、AVDTP / L2CAP のシグナリング、独立した Bluetooth スタック（Bumble）との相互接続は確かめられますが、電波の状態、実機でのタイミング、実際のヘッドホンでの鳴り方は確かめられません。Bumble のパッケージはテスト用の道具で、BlueCaster には含まれません（THIRD_PARTY_LICENSES.md を参照）。

`emu_vhci.py` は同じことを Linux の Bluetooth スタックに向けて行います。Linux 上で `/dev/vhci` から BlueZ に仮想アダプターを付け、計測用受信機 `tools/linux_sink` がそれを本物のアダプターと同じように使うので、実機なしでテストできます。メディアパケットの欠落や遅延も再現できます（`--drop`、`--stall`）。`tools/linux_sink/README.md` を参照してください。

## 依存関係 {#dependencies}

C のライブラリは git サブモジュールです。Rust のクレートと UI の npm パッケージは Cargo と pnpm が取得します（バージョンは `Cargo.lock` と `app/src/ui/pnpm-lock.yaml` で固定。ライセンスは `cargo deny check` で確認）。

| ライブラリ | 取得方法 | 固定バージョン | ライセンス |
|:--------|:-------|:---------------|:--------|
| BTstack | git サブモジュール | v1.8.1-6-g5bc5cbdbe | BTstack License（非商用条項付きの BSD-3-Clause 類似ライセンス） |
| Bluedroid SBC コーデック | エンコーダーは Rust に移植。デコーダー（テスト、ツール）は BTstack 同梱のもの | BTstack に準拠 | Apache-2.0 |
| rijndael | BTstack に同梱（`3rd-party/rijndael`） | BTstack に準拠 | パブリックドメイン |
| liblc3 (lc3-google) | BTstack に同梱（`3rd-party/lc3-google`） | BTstack に準拠 | Apache-2.0 |
| micro-ecc | BTstack に同梱（`3rd-party/micro-ecc`） | BTstack に準拠 | BSD-2-Clause |
| libldac (AOSP) | git サブモジュール | android-15.0.0_r36-4-geeee1a3 | Apache-2.0 |
| libopenaptx | git サブモジュール | 0.2.0（更新しないこと） | LGPL-2.1+ |
| fdk-aac | git サブモジュール | v2.0.3-158-gd8e6b1a | FDK AAC License |
| LHDC V5 エンコーダー | git サブモジュール | 固定したコミット | Apache-2.0 |
| libopus | git サブモジュール | v1.5.2 | BSD-3-Clause |
| Tauri、SolidJS、Rust のクレート | Cargo / pnpm | `Cargo.lock`、`pnpm-lock.yaml` | MIT / Apache-2.0 など（リリースの `licenses/third-party/rust-crates.txt`） |

{: .warning }
BTstack をリンクしているため、BlueCaster のソースコード自体は Apache-2.0 / MIT のデュアルライセンスであっても、自分でビルドした BlueCaster も個人的かつ非商用の目的でのみ使用・再配布できます。

ライセンスの詳細は [THIRD_PARTY_LICENSES.md](https://github.com/SeiyaFunaokaJP/BlueCaster/blob/main/THIRD_PARTY_LICENSES.md) を参照してください。
