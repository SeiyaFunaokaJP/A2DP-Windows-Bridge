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

## サードパーティライブラリ

| ライブラリ | ライセンス | 用途 |
|:-----------|:-----------|:-----|
| [BTstack](https://github.com/bluekitchen/btstack) | BSD-3-Clause（非商用）/ 商用 | ユーザーモード Bluetooth スタック |
| [libldac (AOSP)](https://android.googlesource.com/platform/external/libldac) | Apache-2.0 | LDAC エンコーダー |
| [libopenaptx](https://github.com/pali/libopenaptx) | LGPL-2.1+ | aptX / aptX HD / aptX LL エンコーダー |
| [fdk-aac](https://github.com/mstorsjo/fdk-aac) | FDK AAC License | AAC-LC エンコーダー |
| [wxWidgets](https://www.wxwidgets.org/) | wxWindows Library Licence | GUI フレームワーク |
| [nlohmann/json](https://github.com/nlohmann/json) | MIT | JSON パーサー |

ライセンス全文とコンプライアンスの詳細は [THIRD_PARTY_LICENSES.md](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/THIRD_PARTY_LICENSES.md) を参照してください。

## 重要な注意事項

### BTstack デュアルライセンス

BTstack はデュアルライセンスです。BSD-3-Clause ライセンスは**非商用利用**のみ許可されています。商用利用には [BlueKitchen GmbH](https://bluekitchen-gmbh.com/) からの別途ライセンスが必要です。

### LDAC 認証

LDAC を使用する商用製品は Sony の認証プロセスを完了する必要があります。[sony.net/Products/LDAC/aosp](https://www.sony.net/Products/LDAC/aosp/) を参照してください。

### aptX 特許に関する注意

aptX（クラシック）、aptX HD、aptX Low Latency、aptX Adaptive は Qualcomm の商標です。libopenaptx ライブラリはクリーンルーム実装で、本ツールでは aptX、aptX HD、aptX Low Latency に使用しています。aptX エンコーディングは一部の法域で特許の対象となる場合があります。A2DPWB は aptX Adaptive エンコーダーを含みません。

### AAC 特許に関する注意

AAC は Via Licensing が管理する特許ライセンスの対象です。商用製品は適切なライセンスを取得する必要があります。

### Bluetooth 認証

商用 Bluetooth 製品は Bluetooth SIG が管理する [Bluetooth 認証プロセス](https://www.bluetooth.com/)を受ける必要があります。
