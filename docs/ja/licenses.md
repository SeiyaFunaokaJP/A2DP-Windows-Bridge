---
title: ライセンス
layout: default
parent: 日本語
nav_order: 5
---

# ライセンス
{: .no_toc }

## 目次
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## プロジェクトライセンス

A2DP Windows Bridge (A2DPWB) は **MIT License** の下でライセンスされています。[LICENSE](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/LICENSE) を参照してください。

{: .warning }
A2DPWB のソースコードは MIT ライセンスです。ただし、配布している `A2DPWB.exe` には BTstack が含まれているため、個人的かつ非商用の目的でのみ使用・再配布できます。

## サードパーティライブラリ

| ライブラリ | 固定バージョン | ライセンス | 用途 |
|:-----------|:---------------|:-----------|:-----|
| [BTstack](https://github.com/bluekitchen/btstack) | v1.8.1-6-g5bc5cbdbe | BTstack License（非商用条項付きの BSD-3-Clause 類似ライセンス）/ 商用 | ユーザーモード Bluetooth スタック |
| Bluedroid SBC コーデック（BTstack に同梱） | BTstack に準拠 | Apache-2.0 | SBC エンコーダー |
| rijndael（BTstack に同梱） | BTstack に準拠 | パブリックドメイン | AES（BTstack が使用） |
| [libldac (AOSP)](https://android.googlesource.com/platform/external/libldac) | android-15.0.0_r36-4-geeee1a3 | Apache-2.0 | LDAC エンコーダー |
| [libopenaptx](https://github.com/pali/libopenaptx) | 0.2.0 | LGPL-2.1+ | aptX / aptX HD / aptX LL エンコーダー |
| [fdk-aac](https://github.com/mstorsjo/fdk-aac) | v2.0.3-158-gd8e6b1a | FDK AAC License | AAC-LC エンコーダー |
| [wxWidgets](https://www.wxwidgets.org/) | 3.2.6 | wxWindows Library Licence 3.1 | GUI フレームワーク |
| zlib（wxWidgets 内蔵） | 1.2.13.1 | zlib ライセンス | 圧縮（PNG 対応） |
| libpng（wxWidgets 内蔵） | 1.6.37 | PNG Reference Library License v2 | PNG 画像 |
| nanosvg（wxWidgets に同梱） | wxWidgets 3.2.6 に準拠 | zlib ライセンス | SVG 画像 |
| [nlohmann/json](https://github.com/nlohmann/json) | 3.12.0 | MIT | JSON パーサー |

ライセンス全文とコンプライアンスの詳細は [THIRD_PARTY_LICENSES.md](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/THIRD_PARTY_LICENSES.md) を参照してください。リリース zip には `licenses/` フォルダーに各ライセンス文が、`SOURCE.txt` に正確なソースのコミットが含まれています。Realtek ファームウェアと Zadig は A2DPWB と一緒には配布していません（リンクのみ）。

## 重要な注意事項

### BTstack ライセンス（非商用）

BTstack のライセンスは BSD-3-Clause **ではなく**、OSI 承認のオープンソースライセンスでもありません。このライセンス（SPDX `LicenseRef-BTstack`）は BSD-3-Clause の 3 条件に、次の第 4 条を加えたものです（原文）。

> 4. Any redistribution, use, or modification is done solely for personal benefit and not for any commercial purpose or for monetary gain.

（参考訳: 再配布・使用・改変は、専ら個人的な利益のためにのみ行われ、商業目的や金銭的利益のために行われないこと。）

`A2DPWB.exe` には BTstack が含まれるため、この制限は配布バイナリ全体に及びます。商用利用には [BlueKitchen GmbH](https://bluekitchen-gmbh.com/)（contact@bluekitchen-gmbh.com）から商用ライセンスを取得する必要があります。

### LDAC 認証

libldac の NOTICE には "Taking the certification process is required to use LDAC in your products."（LDAC を製品で使用するには認証プロセスを受ける必要がある）と記載されています。[sony.net/Products/LDAC/aosp](https://www.sony.net/Products/LDAC/aosp/) を参照してください。A2DPWB のような無償・非商用のアプリにこの認証が適用されるかどうかは確認できていません。A2DPWB は認証を受けていません。

### libopenaptx のバージョン

libopenaptx は LGPL-2.1+ の最終版である **0.2.0** に固定しています。0.2.1 以降は GPL-3.0-or-later（追加の制限あり）となり、A2DPWB がリンクしている他のライブラリと両立しません。v1.0.4 までのリリースは誤って 0.2.1 でビルドされていたため、v1.0.5 以降を使用してください。

libopenaptx は静的リンクしています。リリース zip には LGPL-2.1 の全文を同梱し、About ダイアログに libopenaptx の著作権表示とライセンス文への案内を表示しています。改変した libopenaptx で再ビルド（再リンク）するための完全な対応ソースコードは、リリースのタグが付いたコミット（サブモジュールを含む）としてリポジトリから入手できます。

### aptX 特許に関する注意

aptX（クラシック）、aptX HD、aptX Low Latency、aptX Adaptive は Qualcomm の商標です。libopenaptx は README によれば「ffmpeg 4.0 プロジェクトから派生した」（derived from ffmpeg 4.0 project）オープンソースの aptX 実装で、本ツールでは aptX、aptX HD、aptX Low Latency に使用しています。aptX エンコーディングは一部の法域で特許の対象となる場合があります。A2DPWB は aptX Adaptive エンコーダーを含みません。

### fdk-aac のライセンスと AAC 特許に関する注意

fdk-aac のライセンスでは、バイナリを再配布する際にライセンス全文を添付すること、およびバイナリの受領者に FDK AAC Codec（と改変部分）の完全なソースコードを無償で提供することが求められます。また、このライセンスは**特許ライセンスを一切許諾しません**。AAC は特許の対象となる場合があり（NOTICE では Via Licensing に言及）、必要な AAC 特許ライセンスの取得は利用者の責任となります。

### 開発者向けツール a2dpwb_decode

`a2dpwb_decode`（[使い方](usage#verify-stream)を参照）はソースからのみビルドするツールで、リリース zip には含まれません。上記の固定バージョンの Bluedroid SBC デコーダー（Apache-2.0）、fdk-aac（FDK AAC License）、libopenaptx 0.2.0（LGPL-2.1+）をリンクし、BTstack と libldac は**リンクしません**。詳しくは THIRD_PARTY_LICENSES.md の §12 を参照してください。

### テスト環境 tools/emu

`tools/emu`（[ビルド](building)を参照）は、[Bumble](https://github.com/google/bumble) 0.0.234（Apache-2.0）とその依存パッケージを使います。これらは `setup_env.py` が固定バージョンで、git の管理外のローカル環境にダウンロードするもので、A2DPWB にリンクも同梱もされません。パッケージ一覧とライセンスは THIRD_PARTY_LICENSES.md の §13 を参照してください。

### Bluetooth 認証

商用 Bluetooth 製品は Bluetooth SIG が管理する [Bluetooth 認証プロセス](https://www.bluetooth.com/)を受ける必要があります。
