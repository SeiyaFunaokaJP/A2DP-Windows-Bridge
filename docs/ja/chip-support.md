---
title: チップ対応の追加
layout: default
parent: ホーム
nav_order: 3.5
---

# チップ対応の追加

A2DP Windows Bridge は Bluetooth アダプターを自分で（WinUSB を通して）動かすため、アダプターが使えるようになるまでにメーカーの Windows ドライバーがしていることを、自分で行う必要があります。多くのチップでは、起動のたびにファームウェアを読み込ませることです。Realtek、Intel、Broadcom、CSR のアダプターはすでに使えますが、MediaTek（MT7921、MT7922 など）と Qualcomm（QCA61x4）は、そのローダーをまだ誰も書いていないため使えません。

このページは、それを追加する手順です。そのチップのアダプターが必要です（メンテナーは持っていません）。コードを書かなくても協力できます。手順 1 と 2 で、コントリビューターに必要なレポートが作れます。

{: .note }
この仕組みの設計と、まだ決まっていないことは[開発者向けノート](../chip-bringup-dev)にあります。

## 目次
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## 必要なもの

- そのアダプターと、それを挿した Windows 10 / 11 の PC
- A2DP Windows Bridge 2.0.2 以降（下の手順 1 と 2 はアプリだけで行えます）
- ローダーを書くには: [ビルド](building)のとおりのビルド環境と、Windows 自身のドライバーの動きを記録するための [USBPcap](https://desowin.org/usbpcap/) と [Wireshark](https://www.wireshark.org/)

## 手順 1: アダプターを A2DP Windows Bridge に渡す

1. **設定 > 開発者モード**: オン
2. **アダプタ**: そのアダプターのカードで **WinUSB ドライバに置き換える**（Windows が管理者の許可を求めます）。A2DP Windows Bridge がまだ使えないチップでは、このボタンは開発者モードのときだけ押せます
3. カードに **差し直すか Windows を再起動** と出たら、そのとおりにします

アダプターが WinUSB の間は、Windows 自身の Bluetooth はそのアダプターを使えません。Bluetooth がそのチップだけのノート PC では、Windows で接続しているキーボード、マウス、ヘッドセットが、ドライバーを戻すまで使えなくなります。戻すには、同じカードの **元のドライバに戻す** を押します。

## 手順 2: チップの応答を読む（チッププローブ）

アダプターのカードの **ファームウェア** の欄で **チップ情報を読む** を押します。Bluetooth スタックが少しの間止まってプローブが実行され、エクスプローラーでレポートが開きます。レポートは設定フォルダー（`%APPDATA%\A2DPWB`）の `logs\probe\` にある `probe-<date>_<time>.json` で、新しいものから 10 個が残ります。

プローブは読むだけです。HCI Reset もベンダーコマンドも送りません。

| 要求 | わかること |
|:--|:--|
| Read Local Version Information | HCI / LMP のバージョン、メーカー（company ID）、LMP サブバージョン（多くは ROM かファームウェアのバージョン） |
| Read BD_ADDR | アダプターのアドレス（チップによっては、ファームウェアが動くまではゼロか仮の値） |
| Read Local Supported Commands / Features、Read Buffer Size | 動いているコードが提供する機能。ROM はファームウェアよりずっと少ないことがよくあります |
| Read Local Name | 自分の名前を返す ROM もあります |
| LE Read Local Supported Features、LE Read Buffer Size | LE について同じ内容 |
| USB デバイスディスクリプター、メーカーと製品の文字列 | デフォルトパイプでの標準の USB 要求 |

ほかに、Windows が報告する USB 構成（インターフェイス、エンドポイント）と、チップが自分から送ってきたイベントも記録します。応答のなかった要求も記録します。ファームウェアを読み込むまで何も応答しないチップなら、それ自体が手がかりになります。

コマンドラインからは、アダプターの VID:PID かインスタンス ID（アダプターのカードに出ています）を指定します。

```bat
"%LOCALAPPDATA%\A2DP Windows Bridge\a2dpwb-core.exe" --cli -u 0e8d:7961 --probe
```

アダプターは両方から同時には使えないので、先にトレイから A2DP Windows Bridge を終了してください。

**レポートを送る**: **チップ情報を読む** のあと、アダプターのカードに読み取った内容と **GitHub で報告** ボタンが表示されます。このボタンは、バージョン、アダプター、その概要、Windows のビルドを記入した新しい issue を開き、エクスプローラーでレポートのファイルを表示します。そのファイルを issue にドラッグし、アダプターの入手元（ノート PC 内蔵ならその機種）を書き足してください。レポートにはアダプターの Bluetooth アドレスと名前が入りますが、ヘッドホンについての情報は入りません。HCI キャプチャを含めた **診断パッケージ**（診断画面）にも、プローブのレポートが `probe/` に入ります。

## 手順 3: Windows のドライバーの動きを記録する

プローブでわかるのはチップの状態で、それを変える方法ではありません。それを知っているのはメーカーの Windows ドライバーです。一度記録すれば、ローダーで同じことを繰り返せます。

1. アダプターのカードで **元のドライバに戻す** を押し、Windows のドライバーでもう一度動かします
2. USBPcap と Wireshark をインストールし、Wireshark を管理者として起動して、アダプターがつながっている USBPcap のインターフェイスをキャプチャします。`USBPcapCMD.exe`（USBPcap のフォルダーにあります）で、ルートハブごとのデバイスを一覧できます。ノート PC 内蔵のアダプターは内部のハブにつながっていて、「USB 複合デバイス」（"USB Composite Device"）か VID:PID だけで出ることがよくあります。わからなければ、USBPcap のインターフェイスをすべて同時にキャプチャします
3. ドライバーにファームウェアを読み込ませます。アダプターを抜いて挿し直します。内蔵のものは、デバイス マネージャーで **デバイスを無効にする** と **デバイスを有効にする** を行います。再起動では消えないファームウェアは、完全にシャットダウンしたあとでないと読み込まれないことがあります
4. Windows の Bluetooth 設定にアダプターが出たら、キャプチャを止めます。最初のほうのパケットからアダプターのデバイスディスクリプター（VID:PID）を探し、そのアドレスで絞り込んで（`usb.bus_id == <bus> && usb.device_address == <address>`）キャプチャを保存します

見るところ（この順に）:

- HCI コマンドより前の **コントロール転送**: `bmRequestType` 0x40 / 0xC0（ベンダー、OUT / IN）と、その `bRequest`、`wValue`、`wIndex`。チップ ID を読んだり、チップの一部をリセットしたりしていることがよくあります
- オペコード 0xFC00〜0xFFFF の **HCI コマンド**（クラス要求、`bmRequestType` 0x20）: ベンダーコマンドです。同じオペコードが大きなパラメーター付きで長く続いていれば、ファームウェアを送っているところです。その応答から、各部分がどう確認されるかがわかります
- Command Complete / Status 以外の **イベント**: ベンダーイベント（コード 0xFF、または 0xE4 のようなベンダー独自のコード）。たとえば「ファームウェアが起動した」という通知や、Command Complete の代わりにファームウェアの各部分を確認するものです
- 最初の HCI Reset より前の **Bulk OUT** のデータ: ACL パイプでファームウェアを受け取るチップもあります
- ファイルの出どころ: 送られたデータを、ドライバーのフォルダー（デバイス マネージャー > ドライバーの詳細、または `C:\Windows\System32\DriverStore\FileRepository\`）のファイルや、[linux-firmware](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/) にあるそのチップのファイル（`mediatek/`、`qca/`）と比べます。linux-firmware のファイルは再配布でき、A2DP Windows Bridge のユーザーもダウンロードできます

ドライバーが手順の間に置いている待ち時間（キャプチャのタイムスタンプ）も書き留めてください。リセット中のチップは応答しません。

{: .warning }
キャプチャ、プローブのレポート、メーカーの資料をもとに作業してください。Linux カーネルの `btmtk` や `btqca` ドライバーのような GPL のコードを、コピーしたりなぞって書いたりしないでください。A2DP Windows Bridge は MIT ライセンスで、それらから派生したローダーは取り込めません。

## ビルドせずに試す: チップスクリプト {#chip-scripts}

ローダーを書く前に、キャプチャでわかったことをアダプターで試してみます。**チップスクリプト** は手順をテキストにしたもので、1 行に 1 つの要求を書き、Bluetooth スタックの代わりに実行します。アダプターを持っていない人が書いて、持っている人に送ることもできます。結果はレポートとして返ってくるので、新しいビルドは要りません。

**実行する**: 開発者モードで **開発者 > チップ** を開き、アダプター（WinUSB のもの）を選んで、スクリプトを入力するかファイルを開き、**実行** を押します。各行の応答が下に表示され、**レポートを表示** で `logs\probe\` の `script-<date>_<time>.json` が開きます（スクリプトも入っています）。コマンドラインからは `a2dpwb-core --cli -u <adapter> --script <file>` です。

```text
# 1 行に 1 つの要求。数値は 16 進数（0x は省略可）。ただし wait、timeout=、chunk= は 10 進数
hci 1001                           # HCI コマンド: オペコード、続いてパラメーターのバイト
hci fc05 ff
bulk fc09 00 01 02                 # 同じものを Bulk OUT パイプで
usb in vendor device 63 0 0 4      # コントロール IN: 種類、受信者、要求、値、インデックス、長さ
usb out vendor device 64 0 0 01    # コントロール OUT: …、続いてデータのバイト
wait 100                           # ミリ秒
expect e4 02 01 timeout=5000       # イベント: そのコードと、パラメーターの先頭のバイト
bootloader off                     # ブートローダーモード（最初からオン）
upload mediatek/patch.bin fc6f 01 01 chunk=240 ack=event:e4:02,01
upload-bulk patch.bin fc09 01      # 同じものを Bulk OUT パイプで
```

- `hci` と `bulk` は、そのコマンドの Command Complete か Command Status を待ち（2 秒）、ステータスにかかわらず記録します
- `upload` は、ファームウェアのフォルダーにあるファイルを、そのオペコードのコマンドに分けて送ります。各コマンドにはヘッダーのバイトと、ファイルの最大 `chunk` バイト（既定は 240）が入ります。コマンドごとに確認を待ちます: `ack=complete`（既定。そのオペコードの、ステータス 0 の Command Complete か Command Status）または `ack=event:<code>[:<bytes>]`。エラーのステータスが返ると止まります
- スクリプトは最初に失敗した行（応答がない、エラー、ファイルがない）で止まり、どの行かを表示します
- どの行への応答でもないイベントも、レポートに残ります

**安全のために**: そのまま実行されるのは、読み取りだけだとわかっている要求です: 標準の HCI の読み取り（情報パラメーターとステータスパラメーター、Read Local Name、LE の読み取り）、標準の USB ディスクリプター要求、`wait`、`expect`、`bootloader`。それ以外の行があるスクリプトは、許可したときだけ実行されます（その行の一覧の下にある **実行を許可**。コマンドラインでは `--allow-writes`）。チップが電源を切っても保持するメモリーに書き込めるベンダーコマンドは、何を許可しても実行されません（`app/src/crates/bt-host/src/chips.rs` の `denied_opcodes`。今のところ CSR の BCCMD）。ブートローダーモードは最初からオンなので、終わったあとアダプターを閉じても HCI Reset は送られません。チップの RAM に読み込んだファームウェアは、差し直すと消えます。

## 手順 4: ローダーを書く

ここからのパスは、リポジトリのルートからです。関係するクレートは 3 つです。

| クレート（パッケージ） | フォルダー | 中身 |
|:--|:--|:--|
| `bt-host` | `app/src/crates/bt-host` | Bluetooth ホスト: ローダー、プローブ、チップの検出 |
| `a2dpwb-core`（ライブラリ） | `app/src/crates/core` | 設定とアダプター: どのローダーを動かすか |
| `a2dpwb-core-exe` | `app/src/core` | サービスの実行ファイル（`a2dpwb-core.exe`）: ウィンドウからの要求、アダプターの説明 |

下の 2 つはどちらも名前に "core" が付きます。`cargo -p` に渡すのは上のパッケージ名です。

ローダーは、自分では I/O を行わないステートマシンです。コントローラーのイベント、自分の USB 要求の結果、タイマーを受け取り、アプリが実行する **アクション** を返します。そのためチップがなくてもテストでき、USB やスレッドの細かい処理はローダーに入りません。

1. 架空のチップ用の完全なローダーとそのテストである `app/src/crates/bt-host/src/bringup/sample.rs` を、`app/src/crates/bt-host/src/<vendor>.rs`（例: `mediatek.rs`）にコピーします。`app/src/crates/bt-host/src/lib.rs` に、ほかのモジュールとアルファベット順になる位置で `pub mod <vendor>;` を追加します
2. 名前はそのままにします。型は `Loader`（`<vendor>::Loader` として使います）で、`vendor()` はベンダー名を返します。テストは、サンプルと同じくそのファイルに置きます
3. サンプルが "made up" としているところを、手順 2 と 3 でわかったことにすべて置き換えます: バージョンの確認、USB 要求、ファイル名、ダウンロードのコマンドとその確認のされ方、ファームウェアの起動のしかた
4. 先頭のモジュールコメントを書き直します: チップに必要なこと、手順の順番、それぞれの事実の出どころ（Windows のドライバーのキャプチャ、メーカーの資料、プローブのレポート）。手順の出どころとして、ほかのドライバーのソースコードを挙げないでください

### インターフェイス

すべて `app/src/crates/bt-host/src/bringup.rs` にあります。

| `ChipLoader` のメソッド | 呼ばれるとき |
|:--|:--|
| `vendor()` | ログの行に使う（"MediaTek"） |
| `start()` | 開いたアダプターで、HCI の初期化の前に 1 回 |
| `on_event(event)` | HCI イベントごと: コード、長さ、パラメーター |
| `on_control(result)` | 最後の `UsbControl` の結果（読んだバイト、または失敗の理由）を渡すとき |
| `on_timeout()` | 最後の `Timer` のタイマーが切れたとき |
| `missing_file_hint(file)` | ローダーが `MissingFile` で終わったときのログ用: 何をどこに置くか（"x.bin from linux-firmware (mediatek/)"） |
| `step()`、`progress()` | アクションをまとめて返すたび: 手順の名前（"download"）と進み具合（全体のうち送ったコマンドの数）。アダプターの起動中にアダプタ画面に表示します |

`Done` のあと、アプリはローダーを破棄します。遅れて届いた応答がローダーに渡ることはありません。

| `Action` | 動作 |
|:--|:--|
| `Command(bytes)` | HCI コマンド（オペコード、長さ、パラメーター）をコントロールパイプで送ります。HCI キャプチャに記録されます |
| `BulkCommand(bytes)` | `Command` と同じバイトのコマンドを Bulk OUT パイプで送ります。ここでファームウェアを受け取るブートローダーもあります |
| `UsbControl(request)` | デフォルトパイプでのコントロール転送（ベンダーのレジスター）。すぐに実行され、次のアクションの前に `on_control` が結果を受け取ります。HCI キャプチャにはメモとして記録されます |
| `Bootloader(on)` | ローダーが Bulk IN パイプで応答するチップや、HCI Reset を送ってはいけないチップ（受けると止まるものがあります）のためのものです。パターンを参照してください。HCI に普通に応答する ROM のチップには不要です |
| `Timer(ms)` | ローダーのただ 1 つのタイマーを開始、または開始し直します |
| `Done(Ok(outcome))` | 完了。続いて HCI の初期化が行われます。`AlreadyRunning`、`Loaded { firmware, detail }`、`Rom`（`Probed` はプローブ専用） |
| `Done(Err(error))` | アダプターは使えません: `MissingFile(name)`、`BadFile { file, why }`、`Command { opcode, status }`、`Timeout(step)`、`Rejected(code)`、`BadVersion`、`UnsupportedVariant(v)`、`Unsupported(why)`、`Host(why)` |

バイトを手で解析しなくて済むように、ヘルパーがあります。

- `hci_command(opcode, params)` はコマンドを組み立てます
- `parse_event(event)` は `HciEvent::Complete { opcode, status, ret }`（ステータスのあとの戻りパラメーター）、`Status { opcode, status }`、`Vendor(params)`（イベント 0xFF）、`Other { code, params }`（ほかのすべてのコード。0xE4 のようなベンダー独自のものも含む）を返します

**ファームウェアのファイル**: ローダーは `FileReader` を受け取り、**フォルダーを含まないファイル名**（`"BT_RAM_CODE_MT7961_1_2_hdr.bin"`）でファイルを求めます。アプリはファームウェアのフォルダー（`%APPDATA%\A2DPWB\firmware`）と、その中のベンダーのサブフォルダー（`firmware\mediatek\`）を、linux-firmware と同じ配置で探します。このサブフォルダーは手順 6 で決めます。

### パターン

どれもサンプルに入っています。チップに不要なものは削ってください。

- **1 つのコマンドに 1 つの確認**: 送ってタイマーを開始し、待ち状態ではそのオペコードの確認だけを受け付けます。Command Complete か、結果があとから来るコマンドならステータス 0 の Command Status です。それ以外は無視するので、遅れて届いた応答でローダーが先に進むことはありません
- **ベンダーイベントで確認するチップ**: ファームウェアの各部分に、Command Complete ではなく独自のイベントで応答するチップがあります。その場合はそのイベントが確認です。そのイベント（コードと、それを見分けるパラメーター）だけを待ち、その状態ではそのコマンドの Command Complete やステータス 0 の Command Status を無視します。ただし、どちらでもエラーのステータスなら終了します。サンプルは Command Complete と Command Status を同じ確認として扱うので、このためには `on_event` で状態とイベントの両方を match する必要があります
- **複数のファイル**（パッチのあとに設定ファイル）: どれかを送る前にすべて読み込んで確認し、順に送り、最後のものを `Loaded { firmware, .. }` に入れます（ほかのものはログの行に出します）
- **意図した待ち時間**（ドライバーが次の手順の前に 100 ms 待つ）: そのための状態と `Timer(100)`。その状態での `on_timeout` は、失敗にせず次の手順を送ります。テストでは `SimRun::timeouts` がこうした待ちも数えます
- **2 回応答するコマンド**（確認のあとに、「ファームウェアが起動した」のようなベンダーイベント）: 状態を 2 つにし、2 つ目には専用のタイマーを付けます。サンプルの `Launching` と `Starting` がこれです。イベントに結果コードがあり、それが不正なら `Rejected(code)` でローダーを終えます
- Command Complete か Command Status の **エラーのステータス** は、`Command { opcode, status }` でローダーを終えます
- **ブートローダーモード**: 最初のコマンドの前（バージョンの確認より前でも）に `Bootloader(true)`。成功したとき（`Loaded` と `AlreadyRunning`）は `Done` の直前に `Bootloader(false)` を送り、HCI の初期化が普通に動くようにします。失敗したときはチップがまだブートローダーにいるので、オンのままにします。サンプルの `finish()` がこれを行います
- **すでに動いている**: バージョンやレジスターが動作用のファームウェアを示していれば、すぐに `AlreadyRunning` で終えます。アダプターは電源が切れるまでファームウェアを保持していて、2 回目のダウンロードは失敗することがあります
- **先にファイルを確認する**: どれかを送る前にファイル全体を解析して確認し（理由付きの `BadFile`）、見つからないときはそのファイル名を正確に示します
- **自分のチップだけ**: ローダーがベンダーのチップの一部だけに対応するときは、何かを送る前に、アダプターがそのどれかであることを確認し（チップ自身のチップ ID のレジスター、または `ControlRequest::device_descriptor()` で得る USB ID）、違えば `Unsupported` で終えます
- チップからわかったこと（チップ ID、ROM のバージョン、選んだファイル）は `info` で **ログに出します**。レポートに出るのはこれです
- チップのフラッシュや OTP メモリーには決して書き込まず、ファイルは `FileReader` を通してだけ読みます

## 手順 5: シミュレートしたコントローラーでテストする

テストには `app/src/crates/bt-host/src/bringup/sim.rs` を使います。小さな模擬チップ（`impl SimController`）を書き、キャプチャで実物がしたとおりにコマンドとコントロール転送に応答させます。`sim::run(&mut loader, &mut chip)` は、タイマーも含めてアプリとまったく同じようにローダーを動かします。その結果（`SimRun`）には、終わり方、送ったすべてのコマンドとそのパイプ（`commands[i].bulk`）、すべてのコントロール転送、最後のブートローダーモード（`bootloader`）、タイマーが切れた回数が入っています。`sim::command_complete`、`command_status`、`vendor_event`、`event`（任意のコード）、`opcode`、`params` でバイトを組み立てたり読んだりできます。バルクのコマンドには `bulk_command` で応答します。

サンプルと同じく、少なくとも次を確かめます。

- ダウンロード全体と、送ったコマンド（数、順番、サイズ、パイプ）
- ローダーがブートローダーモードを使うなら、どの場合も最後のブートローダーモード
- ファームウェアがすでに動いている場合
- ファイルがない場合と、壊れたファイル（何も送らないこと）
- 拒否されたコマンド（エラーのステータス）、応答しないコントローラー（タイムアウト）、タイムアウトしうる待ちのそれぞれ
- ローダーが USB 転送を使うなら、その失敗

テストデータには、キャプチャからコピーしたバイトが一番です。実行するには:

```bat
cargo test -p bt-host
```

## 手順 6: アプリに組み込む

変更するファイルをすべて挙げます。すでに未対応として載っているベンダー（MediaTek、Qualcomm）でも、新しいベンダーでも同じです。新しいベンダーなら、そのバリアントが必要な `match` はコンパイラーが示します。それ以外の箇所と、すでに載っているベンダーの場合のすべては、この一覧でしか見つかりません。

**一部のチップか、ベンダー全体か**: ローダーは普通、ベンダーのチップの一部に対応します（MT7921 には対応しても MT7925 には対応しない、など）。その場合、"unsupported" から外してよいのはそのチップだけです。その VID:PID を `app/src/crates/bt-host/src/chips.rs` の `LOADER_CHIPS` に並べ、ベンダーの `support_tag()` はそのままにします。すると `chips::chip_supported(vendor, vid, pid)` と `chip_support_tag()` がアダプターごとに答え、アダプターの名前のタグ、アダプタ画面での説明、そこで WinUSB を勧めるかどうか、"not implemented" の警告がそれに従います。`support_tag()` を変えるのは、（Intel のように）ベンダーのすべてのチップに対応するローダーの場合だけです。

{: .note }
各ファイルの改行コードはそのままにしてください。ソースファイルには CRLF のものも LF のものもあります。エディターや Git Bash の `sed -i` には、黙って書き換えるものがあり、`cargo fmt` では元に戻りません。`git diff --stat` で気づけます。数か所しか変えていないファイルで、すべての行が数えられていれば、それです。その場合は、コミットの前に（たとえば VS Code のステータスバーにある改行コードの切り替えで）元に戻してください。

**検出**（`app/src/crates/bt-host/src/chips.rs`）:

1. `ChipVendor`: 新しいベンダーにはバリアント（doc の行付き）と、`name()` のアームを追加します。どのベンダーでも、enum の doc コメント（ローダーのあるベンダー）とバリアントの doc の行（"not supported"）を今の状態に合わせます
2. 一部のチップ: その VID:PID を `LOADER_CHIPS` に入れ、ローダー自身のチップの確認（チップ ID、バージョン）がちょうどそれらのエントリーを受け入れることを確かめるテストを、ローダーのモジュールに追加します。ベンダー全体: `support_tag()` を `"unsupported"` から `"experimental"` に変えます
3. `vendor_for_usb()`: ベンダーの USB VID。ほかのメーカーも使う VID は、全体ではなく必ず `VID if pid == PID` で一致させます。モジュールメーカーの VID で出ているノート PC 用モジュール（AzureWave 0x13D3、Foxconn 0x0489）や、pid.codes 0x1209 のような共有の VID です。ここに並べた VID はどれも、`is_bluetooth_vendor_vid` がそのメーカーのデバイスを Bluetooth アダプターとして扱うことにもつながります。BTstack の Realtek の表か `OEM_TABLE`（どちらもこのファイルの `resolve_realtek_pid` にあります）にある VID:PID は、`vendor_for_usb` が呼ばれる前に Realtek と判定されます。自分の VID:PID がどちらにもないことを確かめてください
4. `company_name()`: Read Local Version で得られる Bluetooth SIG の company ID。まだなければ追加します
5. テスト: `app/src/crates/bt-host/tests/chips.rs`（VID:PID とタグ）

**アダプターを開く**: Bluetooth クラスのデバイス（クラス E0/01/01）はそのまま開けます。ROM でベンダークラス（FF）を名乗るチップは、`app/src/crates/core/src/adapter.rs` の `BUILTIN_VENDOR_CLASS` にもその VID:PID が必要です。クラスはプローブのレポートの `usb.interfaces` の行でわかります。

**ローダーを起動する**（Intel と同じように。`intel_loader` で検索してください）:

6. `app/src/crates/bt-host/src/lib.rs`: `intel_loader` のような `HostConfig` のフィールド（`HostConfig::new` での既定値、`BtHost::launch` での分割代入、debug の行も）と、`BtHost::launch` の中の分岐。H4 でないとき（Intel と同じく `&& !tcp`。エミュレーターのコントローラーは実際のチップではありません）だけ、トランスポートを設定して、`setup_hci_and_power_on` の代わりにアプリの起動関数を呼びます。起動ごとに動くローダーは 1 つです。2 つのベンダーのフラグが立っているとき（両方のアダプターが WinUSB）は最初の分岐が優先されるので、テストは自分のアダプターだけを WinUSB にして行ってください
7. `app/src/crates/bt-host/src/app.rs`: `start_intel_loader` のような `start_<vendor>_loader`（`use crate::<vendor>;` 付き）。ローダーに `read_firmware_file(&dir, "<vendor>", name)`（サブフォルダー名は linux-firmware と同じ）を渡し、`start_loader` を呼びます
8. `app/src/crates/core/src/adapter.rs` の `configure()`: そのベンダーのアダプターのうち `chips::chip_supported(a.vendor, a.device.vid, a.device.pid)` を満たすものが WinUSB にあり、使用中でないときにフィールドを設定し（`config.intel_loader` と同じように。それ以外には Intel と同じ "in use by another program" のログ）、そこにある `adapter:` の debug の行にも追加します。その下の "firmware loading for this chip is not implemented" の警告は、すでに `chip_supported` に従っています
9. `app/src/crates/core/src/service.rs` の `host_config()`: debug の行にフィールドを追加します

**ウィンドウに出る内容**:

10. `app/src/core/src/state.rs` の `vendor_note()`: `MediaTek | Qualcomm` のアームから外して、新しいキー `firmware.vendor_<vendor>` のアームを作り、そのテストも追加します。この関数は、`chip_supported` が受け入れないチップにはその前で `firmware.vendor_unsupported` を返すので、ベンダーのほかのチップはそのままです。アダプタ画面は、説明が `firmware.vendor_unsupported` でないすべてのアダプターに WinUSB を勧めます
11. `app/lang/*.json`（全 8 言語、それぞれ同じ行）: 新しいキー。Intel のもの（"… adapter (experimental). …"）にならいます。ボタンやフォルダーの名前を書くときは、同じファイルにあるその言語の言葉を使います（ファームウェアのフォルダーを開くボタンは `firmware.open_folder`）。ベンダー全体: `firmware.vendor_unsupported` からそのベンダー名を外します。一部のチップ: ベンダーのほかのチップがまだその文を表示するので、そのまま残します
12. `app/src/ui/src/lib/links.ts`: ベンダーのファームウェアのページ（`LINKS`、`firmwareUrl()`、`vendorFirmwareUrl()` に。Intel の linux-firmware のフォルダーと同じように）。これでアダプタ画面がファイルの入手先を示します。テストは `app/src/ui/src/lib/chips.test.ts` に書きます。アダプタ画面がそのページを示すのは説明が `firmware.vendor_unsupported` でないアダプターだけなので、ベンダーのほかのチップには出ません

**ドキュメント**:

13. `docs/setup.md` と `docs/*/setup.md`（全言語）のチップの表: Intel と同じような行に、対応するチップ、ファームウェアのファイルとその置き場所（ファームウェアのフォルダー `%APPDATA%\A2DPWB\firmware`、またはその `<vendor>` サブフォルダー。古い行にはまだ "config folder" とあります）と *Experimental.*（日本語のページでは *試験的。*）を書きます。ウィンドウのボタンは、各言語の `app/lang/*.json` にあるとおりの名前で書きます（アダプタ画面は `ui.nav.adapter`、そのフォルダーを開くボタンは `firmware.open_folder`）。ベンダー全体: "Not supported" の行から外します。一部のチップ: ベンダーはそこに残し、まだ対応していないチップを書きます。Firmware の節の最初の文（"Intel and Broadcom adapters (experimental) may need firmware files of their own"）にそのベンダーを加えます
14. 誰かが使ったら [動作確認済みハードウェア](hardware)（`docs/hardware.md`）に載せます。またこのページの冒頭（`docs/chip-support.md`、全言語）も直します。ここではローダーのないベンダーを挙げています（一部のチップなら "the other <vendor> chips"）

## 手順 7: アダプターで試す

リポジトリのルートでビルドし（`cargo build --release -p a2dpwb-core-exe`）、インストール済みのアプリを（トレイアイコンから）終了してから、HCI キャプチャを取りながらコアをコマンドラインで実行します。

```bat
target\release\a2dpwb-core.exe --cli -u 0e8d:7961 --scan 5 --hci-capture loader.pklg
```

ログに各手順が出ます。`loader.pklg` は Wireshark で開け、コマンド、イベント、コントロール転送がすべて順に入っています。アダプターを抜いて（またはシャットダウンして）もう一度試してください。ダウンロードは電源が切れていたチップから動き、ファームウェアがすでに動いているときは何もしない必要があります。

{: .note }
WinUSB のアダプターが複数あると、`-u` を指定しても Realtek のものが優先され、そのチップの設定が適用されます。**アダプタ > Bluetooth チップ** を **Realtek 以外** にするか、自分のアダプターだけを WinUSB にしてテストしてください。

そのあと、ウィンドウからしばらくヘッドホンに配信し、**Bluetooth を再起動** と差し直しで元に戻ることを確かめます。アダプターの起動中は、**アダプタ** 画面の Bluetooth の状態の下にローダーの様子（「読み込み中 42%」、または失敗したこと）が表示されます。開発者モードでは準備ができたあとも、`step()` の名前と終わり方が表示されます。

## 手順 8: プルリクエストを出す

出す前に、次を順に実行します（環境は[ビルド](building)を参照）。まず `app\src\ui` で UI のチェックです。手順 6 の変更（項目 12）を確かめるもので、Rust のワークスペースにはウィンドウのクレートが入っていて、そのビルドには UI のビルドが必要です。

```bat
pnpm install
pnpm build
pnpm exec tsc --noEmit
pnpm exec vitest run
```

次に、リポジトリのルートで:

```bat
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
python tools/check_lang.py
```

Node.js がなければ、テストと clippy を上の 3 つのクレートだけで実行し（`-p bt-host -p a2dpwb-core -p a2dpwb-core-exe`）、UI のチェックはプルリクエストの CI に任せて、そのことをプルリクエストに書いてください。`pwsh ./build.ps1 -Test` ですべて実行できます。

Clippy は名前も確かめます。`PatchState` という `State` のバリアントは `enum_variant_names` で失敗するので、`Patch` にします。`check_lang.py` は、変更前からある "long hints left" を報告することがあります。直す必要があるのは、エラーと足りないキーだけです。

ほかにも MediaTek や Qualcomm を未対応として軽く触れている箇所があります（UI のモックデータとテスト、`Adapter.tsx` と `probe.rs` のコメント、"Not Realtek" の選択肢）。これらはそのままにします。

プルリクエストには:

- 現在の `develop-v*` ブランチをベースにします
- テストしたアダプター（製品、VID:PID、チップ）と、コールドスタートとウォームスタートの結果を書き、プローブのレポートとダウンロードの HCI キャプチャを添付します
- ローダーの事実の出どころを書きます
- ローダーは何人かが使うまで **実験的** のままです。アダプタ画面にそのタグが出ます
