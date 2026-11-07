---
title: 連携するツール
layout: default
parent: ホーム
nav_order: 2.6
---

# 連携するツール
{: .no_toc }

BlueCaster と組み合わせて動く他のアプリやツールです。作者や利用者が issue で知らせてくれたものを載せています。

{: .warning }
> これらは **BlueCaster の一部ではなく**、BlueCaster プロジェクトが作ったり、テストしたり、サポートしたりしているものではありません。BlueCaster と組み合わせて動くと誰かが報告したので載せているだけです。動かない場合、どちらかのプログラムの更新で動かなくなった場合、何らかの問題が起きた場合も、責任は負いません。各プロジェクトのライセンスとドキュメントを確かめ、ツールの問題はまずそのツールのプロジェクトに報告してください。

## 目次
{: .no_toc .text-delta }

1. TOC
{:toc}

## アプリとツール {#apps-and-tools}

| ツール | BlueCaster と組み合わせてできること | 必要なもの | ライセンス | 報告者 |
|:--|:--|:--|:--|:--|
| [EncoDeck](https://github.com/TIANLI0/EncoDeck) | OPPO、Sony、HUAWEI、Xiaomi などのイヤホンやヘッドホンの操作（バッテリー、ノイズコントロール、EQ、ジェスチャー）。コアのパイプに 2 つ目のクライアントとしてつなぎ、ストリームの状態とコーデックを表示し、プロファイルの開始と停止、AVRCP の音量の設定をし、WinUSB の割り当ても手伝います。BlueCaster 自身の Bluetooth のリンク経由でのイヤホンの設定には RFCOMM パススルーを使います | EncoDeck 1.11.0 以降。RFCOMM パススルー: BlueCaster 2.0.2-beta.10 以降（今のところエミュレーターでのみ試験済み） | GPL-3.0 | 作者、[#194](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/194) |

## ツールを追加するには {#adding-a-tool}

BlueCaster と組み合わせて動くツールを作った、または見つけたら、名前、リンク、ライセンス、BlueCaster と組み合わせてできること、試したバージョンを添えて [issue](https://github.com/SeiyaFunaokaJP/BlueCaster/issues) で教えてください。ここに載ることは、レビューや推薦を意味しません。

## ツールの作者の方へ {#for-tool-authors}

ツールは、BlueCaster のウィンドウと同じユーザーごとの名前付きパイプでコアと通信します。プロトコル、ツール向けのメソッド、RFCOMM パススルー、互換性のルール（機能の確認、3.0.0 でのパイプ名の変更）は **[連携（IPC）](integration)** にあります。2.0.2 のベータの間は、プロトコルとコアがまだ変わることがあります。
