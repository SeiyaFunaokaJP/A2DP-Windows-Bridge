---
title: 連携（IPC）
layout: default
parent: ホーム
nav_order: 2.7
---

# 連携（IPC）
{: .no_toc }

他のツールが BlueCaster のコアと通信する方法です。ストリームの状態の表示、プロファイルの開始と停止、音量の設定、BlueCaster の Bluetooth のリンク経由でのヘッドホン自身のサービスの利用ができます。これを行うツールは [連携するツール](works-with) に載せています。

{: .warning }
> 2.0.2 のベータの間は、プロトコルがまだ変わることがあります。正となるのは [`app/src/crates/ipc/src/protocol.rs`](https://github.com/SeiyaFunaokaJP/BlueCaster/blob/main/app/src/crates/ipc/src/protocol.rs) と、そこから生成した [`app/src/ui/src/ipc/bindings`](https://github.com/SeiyaFunaokaJP/BlueCaster/tree/main/app/src/ui/src/ipc/bindings) の TypeScript の型です。このページはその使い方を説明するもので、すべてのフィールドを挙げてはいません。

## 目次
{: .no_toc .text-delta }

1. TOC
{:toc}

## 接続 {#connecting}

コア（`bluecaster-core.exe`）は、ウィンドウの有無にかかわらず BlueCaster の実行中ずっと動いています。コアは、それを動かしているユーザーの名前付きパイプで待ち受けます。

```
\\.\pipe\bluecaster-<user SID>
```

- `<user SID>` はログオンしているユーザーの SID の文字列（`S-1-5-21-...`）で、たとえば `whoami /user` で分かります。ユーザーごとにコアとパイプがあり、パイプの DACL はそのユーザーだけを許可し、リモートのクライアントは拒否します。
- パイプはバイトモードです。複数のクライアントが同時につなげ（BlueCaster のウィンドウもその 1 つです）、どれも同じ状態とイベントを受け取ります。
- 2.0.x までは、パイプの名前は `\\.\pipe\a2dpwb-<user SID>` でした。BlueCaster 3.0.0 は **両方** の名前で待ち受け、古い名前でも 3.0.0 の後少なくとも 6 か月は待ち受け続けます。クライアントはまず `bluecaster-<user SID>` を試し、なければ古い名前を使ってください。プロトコルのバージョンは 1 のままです。
- パイプがなければ、コアは動いていません。コアが動いているかは、コアの mutex ではなくパイプで確かめてください。
- `BLUECASTER_INSTANCE=<name>` を付けて起動した開発ビルドは、インストール済みのものと並べて動かせるよう、代わりに `\\.\pipe\bluecaster-<SID>-<name>` で待ち受けます。

### フレーム {#frames}

どちらの向きのメッセージも 1 つのフレームです。JSON の長さを **u32 リトルエンディアン** で置き、続けて JSON（UTF-8）を置きます。4 MiB を超えるフレームは拒否します。

### ハンドシェイク {#handshake}

1. クライアントは、接続してから 5 秒以内に `hello` を送ります。
   ```json
   {"type": "hello", "protocol": 1, "client": "mytool/1.2.0"}
   ```
   `client` は自由な名前とバージョンで、コアのログに出ます。
2. コアは `welcome` で答えるか、`reject` で答えてパイプを閉じます（プロトコルのバージョンが違う、または hello がない場合）。
   ```json
   {"type": "welcome", "protocol": 1, "server": "bluecaster-core/2.0.2-beta.12", "features": ["rfcomm"]}
   {"type": "reject", "reason": "protocol 2 not supported (core speaks 1)"}
   ```
3. コアは状態全体を入れた `snapshot` を送り、その後は `event` を送ります。

プロトコルのバージョンは完全に一致する必要があります。バージョン 1 のまま使える追加は、代わりに `features` で知らせます（[互換性](#compatibility)を参照）。

## 状態とイベント {#state-and-events}

```json
{"type": "snapshot", "rev": 41, "state": {"adapter": {...}, "devices": [...], "stream": {...}, "profiles": [...], "active_profile": "WH-1000XM5", "settings": {...}, ...}}
{"type": "event", "rev": 42, "event": {"kind": "stream", "value": {"state": "streaming", "profile": "WH-1000XM5", "info": {"codec": "ldac", "bitrate_kbps": 990, ...}, "volume": 80, ...}}}
```

クライアントは自分では状態を持ちません。最後のスナップショットを持っておき、イベントのたびにそれへ反映します。状態のイベントはそれぞれ、スナップショットの **ある部分を丸ごと置き換えます**。

| イベントの `kind` | 置き換えるもの |
|:--|:--|
| `adapter` | `adapter` |
| `devices` | `devices` |
| `scanning`、`le_scanning` | `scanning`、`le_scanning` |
| `stream` | `stream`（状態、メッセージ、プロファイル、コーデックの情報、音量、バッテリー） |
| `profiles` | `profiles` と `active_profile` |
| `settings` | `settings` |
| `update` | `update` |
| `capture` | `capture` |
| `v1_running`、`verbose`、`crashes` | 同じ名前のフィールド |

`stream.state` は `idle`、`connecting`、`streaming`、`reconnecting`、`error` のいずれかで、その理由は `stream.message` が [メッセージキー](#errors-and-messages) で示します。

**リビジョン。** 状態のイベントはどれも次のリビジョン（`rev` + 1）を持ちます。番号が飛んだらイベントを取りこぼしたということなので、`state.snapshot` を送って新しいスナップショットを取ってください。読むのが遅すぎるクライアントにも、コアは求められなくても新しい `snapshot` を送ります。状態ではないイベント（`link`、`log`、`receiver`、`rfcomm_data`、`rfcomm_closed`）は今のリビジョンを持ち、リビジョンを進めません。

コアは終了するとき、イベント `bye` を送ってパイプを閉じます。

## リクエスト {#requests}

```json
{"type": "request", "id": 7, "method": "stream.start", "params": {"profile": "WH-1000XM5"}}
{"type": "response", "id": 7, "result": null, "error": null}
```

- `id` はクライアントが選ぶ任意の数で、レスポンスがそれを返します。リクエストは並行して動くので、レスポンスはリクエストと違う順番で届くことがあります。
- パラメーターのないメソッドでは `params` を省きます。
- 状態を変えるリクエストは、それが終わってから答え、その変化はイベントでも届きます。クライアントは、レスポンスではなくイベントから状態を表示してください。

### エラーとメッセージ {#errors-and-messages}

```json
{"type": "response", "id": 8, "result": null, "error": {"code": "rfcomm.not_connected", "message": {"key": "ipc.error.rfcomm_not_connected", "args": []}}}
```

`code` はプログラム向けです（`adapter.driver_missing`、`app.streaming`、...）。`message` は BlueCaster の言語ファイル（[`app/lang`](https://github.com/SeiyaFunaokaJP/BlueCaster/tree/main/app/lang)）の翻訳キーと printf 形式の引数で、状態に含まれるステータスのテキストも同じ形です。ツールはそこでキーを引くことも、コードに対して自分のテキストを出すこともできます。

### メソッド {#methods}

ツール向けのメソッド:

| メソッド | パラメーター | 結果 |
|:--|:--|:--|
| `state.snapshot` | -- | `Snapshot` |
| `stream.start` | `profile` | `null`。動いているストリームがあれば先に止める |
| `stream.stop` | -- | `null` |
| `stream.set_volume` | `volume`（AVRCP の絶対音量 0-127） | `null` |
| `profile.set_active` | `name` | `null` |
| `device.scan` | `seconds`（3-600） | `Device[]`。`devices` イベントとしても届く |
| `device.scan_stop` | -- | `null`。動いている `device.scan` が答える |
| `audio.devices` | -- | `AudioDevice[]`（キャプチャのソース） |
| `diag.subscribe` | `logs`（レベルまたは `null`）、`link`（bool）、`receiver`（bool、省略可） | `null`。この接続への `log` / `link` / `receiver` イベントを始める、または止める |
| `diag.info` | `{"mask": bool}`（省略可） | `DiagInfo`（「診断情報をコピー」のテキスト） |
| `rfcomm.open`、`rfcomm.send`、`rfcomm.close` | [RFCOMM パススルー](#rfcomm-passthrough)を参照 | |

BlueCaster のウィンドウが使うメソッドで、ツールでは注意して使うべきもの: `profile.save` / `profile.delete`、`settings.set`（設定を **すべて** 置き換えます。最新の `settings` に変更を加えて送り、一部だけのオブジェクトは決して送らないでください）、`device.forget` / `device.rename`、`adapter.*`（`adapter.assign_winusb` と `adapter.restore_driver` は UAC の確認を出します。開発者モードのチップのツールである `adapter.probe` と `adapter.run_script` は Bluetooth スタックを止めます）、`app.quit`。それ以外（`stream.start_custom`、ほかの `device.*`（`device.import_windows_pairings` は UAC の確認を出します）、`audio.apps`、`audio.mixer` / `audio.mixer_set`、上の 2 つ以外の `diag.*`、`receiver.*`、`app.*`）はウィンドウ自身の画面のためのもので、予告なく変わることがあります。パラメーターと結果を含む全一覧は、`protocol.rs` の `Method` 列挙型です。

## RFCOMM パススルー {#rfcomm-passthrough}

`welcome.features` に `"rfcomm"` があるとき（2.0.2-beta.10 以降）に使えます。BlueCaster 経由でつないだヘッドホンは Windows からは見えないので、ツールは BlueCaster がすでに持っているリンクを通して、ヘッドホン自身の RFCOMM サービス（メーカーの操作アプリ: バッテリー、ノイズコントロール、EQ）を使えます。

- `rfcomm.open {address, uuid}` → `{channel_id, mtu}`。`address` は配信先のデバイスでなければなりません。コアはこのために接続やペアリングをしません（`rfcomm.not_connected`）。`uuid` は 128 ビットの完全な UUID か、Bluetooth のベース UUID 上の 16 ビットの `"xxxx"` です。答えはほかの open の後ろで待たされた場合も 20 秒以内に返るので、少なくとも 25 秒は待ってください。エラー: `rfcomm.not_connected`、`rfcomm.service_not_found`、`rfcomm.open_failed`、`rfcomm.busy`。
- `rfcomm.send {channel_id, data}` → `null`。`data` は base64（標準、パディングあり）で、1 リクエストあたり最大 64 KiB。リクエストの順に、最大 `mtu` のフレームで送ります。エラー: `rfcomm.no_channel`、`rfcomm.busy`（キューがいっぱいで、送っていない）、`rfcomm.timeout`（送ったかどうか分からない。繰り返さず、チャネルを開き直してください）。
- 受け取ったデータは、そのチャネルを開いた接続にだけ `rfcomm_data` イベント `{channel_id, data}` として届きます。
- `rfcomm.close {channel_id}` → `null`。その前に送ったデータを送ってから閉じ、`rfcomm_closed` は続きません。
- チャネルは、パイプの接続が終わったときにも閉じます。また `rfcomm_closed` イベント `{channel_id, reason}` で閉じることもあります: `remote`（デバイスが閉じた、またはリンクが切れた）、`disconnected`（A2DP の接続が終わった）、`stopped`（Bluetooth スタックが止まった）、`overflow`（クライアントが受け取ったデータを 1 MiB を超えて読まずに残した）。

## 互換性 {#compatibility}

- **機能。** 追加を使う前に `welcome.features` を確かめてください。2.0.2-beta.10 より前のコアは、このフィールドを省きます。
- **不明なリクエスト。** コアが読めないリクエスト（不明なメソッド、不正なパラメーター）には、エラー `ipc.bad_request` で答え、接続はそのまま開いています。2.0.2-beta.10 より前のコアは、代わりに接続を閉じます。それ以外の読めないフレームは接続を閉じます。
- **新しいフィールド。** コアは状態やイベントに少しずつフィールドを追加します。知らないフィールドは無視し、それを理由にメッセージを拒否しないでください。
- **3.0.0 での名前の変更**（[#193](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/193)）: パイプは `\\.\pipe\bluecaster-<SID>` になります。コアは 3.0.0 の後も少なくとも 6 か月は、同じプロトコルで `\\.\pipe\a2dpwb-<SID>` でも待ち受けます。その廃止は先にリリースノートで知らせます。まず `bluecaster-<SID>` を試し、なければ `a2dpwb-<SID>` を使ってください。コアの mutex は、このような移行期間なしに名前が変わります。

## 例 {#example}

ストリームの状態を表示し、その変化を追う最小限の Python のクライアント（標準ライブラリのみ）:

```python
import json, struct, subprocess

sid = subprocess.run(["whoami", "/user", "/fo", "csv", "/nh"],
                     capture_output=True, text=True).stdout.strip().split(",")[1].strip('"')
pipe = open(rf"\\.\pipe\bluecaster-{sid}", "r+b", buffering=0)

def send(msg):
    b = json.dumps(msg).encode()
    pipe.write(struct.pack("<I", len(b)) + b)

def read_exact(n):
    buf = b""
    while len(buf) < n:
        chunk = pipe.read(n - len(buf))
        if not chunk:
            raise EOFError
        buf += chunk
    return buf

def recv():
    (n,) = struct.unpack("<I", read_exact(4))
    return json.loads(read_exact(n))

send({"type": "hello", "protocol": 1, "client": "example/0.1"})
welcome = recv()
assert welcome["type"] == "welcome", welcome
snap = recv()
print("stream:", snap["state"]["stream"]["state"])
while True:
    m = recv()
    if m["type"] == "event" and m["event"]["kind"] == "stream":
        s = m["event"]["value"]
        print("stream:", s["state"], (s.get("info") or {}).get("codec"))
    elif m["type"] == "event" and m["event"]["kind"] == "bye":
        break
```

インストール済みの BlueCaster に触れずにツールを試すには、`BLUECASTER_INSTANCE` と `BLUECASTER_CONFIG_DIR` を設定して開発ビルドを動かし（[ビルド](building)を参照）、そのパイプにつないでください。
