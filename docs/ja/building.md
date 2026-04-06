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
git clone --recursive https://github.com/SeiyaFunaokaJP/A2DPWB.git
cd A2DPWB
cmake -B build -A x64
cmake --build build --config Release
```

実行ファイルは `build/app/Release/A2DPWB-1.0.0.exe` に出力されます。

{: .note }
初回ビルドは CMake FetchContent が wxWidgets (v3.2.6) をダウンロード・コンパイルするため、数分かかります。

## `--recursive` なしでクローン済みの場合

```bash
git submodule update --init --recursive
```

## プロジェクト構成

```
A2DPWB/
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
│   ├── libopenaptx/        aptX / aptX HD エンコーダー
│   ├── fdk-aac/            Fraunhofer AAC エンコーダー
│   └── json/               nlohmann/json（ヘッダーオンリー）
├── CMakeLists.txt          ルートビルド設定
└── build.bat               ビルドヘルパースクリプト
```

## ビルドオプション

| CMake オプション | 既定値 | 説明 |
|:-----------------|:-------|:-----|
| `BUILD_TESTS` | `ON` | テストプログラムをビルド |
| `LDAC_SOFT_FLOAT` | `OFF` | libldac にソフトウェア浮動小数点を使用 |

```bash
cmake -B build -A x64 -DBUILD_TESTS=OFF
```

## 依存関係

すべての依存関係は git サブモジュールとして含まれるか、ビルド時に取得されます。手動インストールは不要です。

| ライブラリ | 取得方法 | ライセンス |
|:-----------|:---------|:-----------|
| BTstack | git サブモジュール | BSD-3-Clause（非商用） |
| libldac (AOSP) | git サブモジュール | Apache-2.0 |
| libopenaptx | git サブモジュール | LGPL-2.1+ |
| fdk-aac | git サブモジュール | FDK AAC License |
| nlohmann/json | 同梱（ヘッダーオンリー） | MIT |
| wxWidgets v3.2.6 | CMake FetchContent | wxWindows Library Licence |

ライセンスの全文は [THIRD_PARTY_LICENSES.md](https://github.com/SeiyaFunaokaJP/A2DPWB/blob/main/THIRD_PARTY_LICENSES.md) を参照してください。
