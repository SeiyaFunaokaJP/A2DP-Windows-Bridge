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
│   └── a2dp_decode/        a2dpwb_decode: HCI キャプチャ（.pklg）のメディアストリームを検証・デコード
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
