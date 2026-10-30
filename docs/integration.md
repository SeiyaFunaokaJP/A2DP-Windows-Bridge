---
title: Integration (IPC)
layout: default
nav_order: 3.7
english_only: true
---

# Integration (IPC)
{: .no_toc }

How a third-party tool talks to the BlueCaster core: show the stream state, start and stop profiles, set the volume, or reach the headphones' own services over BlueCaster's Bluetooth link. Tools that do this are listed on [Works With](works-with).

{: .warning }
> The protocol may still change during the 2.0.2 betas. The source of truth is [`app/src/crates/ipc/src/protocol.rs`](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/app/src/crates/ipc/src/protocol.rs) and the TypeScript types generated from it in [`app/src/ui/src/ipc/bindings`](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/tree/main/app/src/ui/src/ipc/bindings). This page explains how to use them; it does not list every field.

## Table of contents
{: .no_toc .text-delta }

1. TOC
{:toc}

## Connecting

The core (`bluecaster-core.exe`) runs while BlueCaster runs, with or without its window. It listens on a named pipe of the user it runs as:

```
\\.\pipe\bluecaster-<user SID>
```

- `<user SID>` is the logged-on user's SID string (`S-1-5-21-...`), for example from `whoami /user`. Each user has their own core and pipe; the pipe's DACL admits only that user, and remote clients are refused.
- The pipe is in byte mode. Several clients can be connected at once (the BlueCaster window is one of them); each gets the same state and events.
- If the pipe does not exist, the core is not running. Use the pipe, not the core's mutex, to check for it.
- Development builds started with `BLUECASTER_INSTANCE=<name>` listen on `\\.\pipe\bluecaster-<SID>-<name>` instead, so that they run beside the installed one.

### Frames

Every message in both directions is one frame: the length of the JSON as a **u32 little-endian**, then the JSON (UTF-8). Frames over 4 MiB are refused.

### Handshake

1. The client sends `hello` within 5 seconds of connecting:
   ```json
   {"type": "hello", "protocol": 1, "client": "mytool/1.2.0"}
   ```
   `client` is a free name and version; it appears in the core's log.
2. The core answers with `welcome`, or with `reject` and closes the pipe (wrong protocol version, or no hello):
   ```json
   {"type": "welcome", "protocol": 1, "server": "bluecaster-core/2.0.2-beta.12", "features": ["rfcomm"]}
   {"type": "reject", "reason": "protocol 2 not supported (core speaks 1)"}
   ```
3. The core sends a `snapshot` with its whole state, then `event`s.

The protocol version must match exactly. Additions that keep version 1 working are announced in `features` instead (see [Compatibility](#compatibility)).

## State and events

```json
{"type": "snapshot", "rev": 41, "state": {"adapter": {...}, "devices": [...], "stream": {...}, "profiles": [...], "active_profile": "WH-1000XM5", "settings": {...}, ...}}
{"type": "event", "rev": 42, "event": {"kind": "stream", "value": {"state": "streaming", "profile": "WH-1000XM5", "info": {"codec": "ldac", "bitrate_kbps": 990, ...}, "volume": 80, ...}}}
```

The client keeps no state of its own: it holds the last snapshot and applies each event to it. Each state event **replaces one whole part** of the snapshot:

| Event `kind` | Replaces |
|:--|:--|
| `adapter` | `adapter` |
| `devices` | `devices` |
| `scanning`, `le_scanning` | `scanning`, `le_scanning` |
| `stream` | `stream` (state, message, profile, codec info, volume, battery) |
| `profiles` | `profiles` and `active_profile` |
| `settings` | `settings` |
| `update` | `update` |
| `capture` | `capture` |
| `v1_running`, `verbose`, `crashes` | the field of that name |

`stream.state` is one of `idle`, `connecting`, `streaming`, `reconnecting`, `error`; `stream.message` says why, as a [message key](#errors-and-messages).

**Revisions.** Every state event carries the next revision (`rev` + 1). A gap means an event was lost: send `state.snapshot` for a new one. A client that reads too slowly also gets a new `snapshot` from the core without asking. Events that are not state (`link`, `log`, `receiver`, `rfcomm_data`, `rfcomm_closed`) carry the current revision and do not advance it.

When the core shuts down it sends the event `bye` and closes the pipe.

## Requests

```json
{"type": "request", "id": 7, "method": "stream.start", "params": {"profile": "WH-1000XM5"}}
{"type": "response", "id": 7, "result": null, "error": null}
```

- `id` is any number the client chooses; the response carries it back. Requests run concurrently, so responses can come in another order than the requests.
- `params` is left out for methods without parameters.
- A request that changes state answers once it is done, and the change also arrives as an event. Clients should show the state from the events, not from the response.

### Errors and messages

```json
{"type": "response", "id": 8, "result": null, "error": {"code": "rfcomm.not_connected", "message": {"key": "ipc.error.rfcomm_not_connected", "args": []}}}
```

`code` is for the program (`adapter.driver_missing`, `app.streaming`, ...). `message` is a translation key of BlueCaster's language files ([`app/lang`](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/tree/main/app/lang)) with printf-style arguments, as are the status texts in the state; a tool can look the key up there or show its own text for the code.

### Methods

Methods for tools:

| Method | Params | Result |
|:--|:--|:--|
| `state.snapshot` | -- | `Snapshot` |
| `stream.start` | `profile` | `null`; stops a stream that runs first |
| `stream.stop` | -- | `null` |
| `stream.set_volume` | `volume` (AVRCP absolute volume 0-127) | `null` |
| `profile.set_active` | `name` | `null` |
| `device.scan` | `seconds` (3-600) | `Device[]`, also as the `devices` event |
| `device.scan_stop` | -- | `null`; the running `device.scan` answers |
| `audio.devices` | -- | `AudioDevice[]` (capture sources) |
| `diag.subscribe` | `logs` (level or `null`), `link` (bool), `receiver` (bool, optional) | `null`; starts or stops `log` / `link` / `receiver` events for this connection |
| `diag.info` | `{"mask": bool}` (optional) | `DiagInfo` (the "copy diagnostic info" text) |
| `rfcomm.open`, `rfcomm.send`, `rfcomm.close` | see [RFCOMM passthrough](#rfcomm-passthrough) | |

Methods the BlueCaster window uses that a tool should use only with care: `profile.save` / `profile.delete`, `settings.set` (replaces **all** settings: send the latest `settings` with your change, never a partial object), `device.forget` / `device.rename`, `adapter.*` (`adapter.assign_winusb` and `adapter.restore_driver` show a UAC prompt), `app.quit`. The others (`stream.start_custom`, `audio.apps`, `diag.*` beyond the two above, `receiver.*`, `app.*`) serve the window's own screens and may change without notice. The full list with parameters and results is the `Method` enum in `protocol.rs`.

## RFCOMM passthrough

Available when `welcome.features` contains `"rfcomm"` (2.0.2-beta.10 and later). It lets a tool reach the headphones' own RFCOMM services (vendor control apps: battery, noise control, EQ) over the link BlueCaster already has, since Windows does not see headphones connected through BlueCaster.

- `rfcomm.open {address, uuid}` → `{channel_id, mtu}`. `address` must be the device being streamed to; the core never connects or pairs for this (`rfcomm.not_connected`). `uuid` is a full 128-bit UUID or a 16-bit `"xxxx"` on the Bluetooth base UUID. The answer comes within 20 s, also when queued behind other opens: wait at least 25 s. Errors: `rfcomm.not_connected`, `rfcomm.service_not_found`, `rfcomm.open_failed`, `rfcomm.busy`.
- `rfcomm.send {channel_id, data}` → `null`. `data` is base64 (standard, padded), at most 64 KiB per request, sent in frames of at most `mtu` in request order. Errors: `rfcomm.no_channel`, `rfcomm.busy` (queue full, not sent), `rfcomm.timeout` (unknown whether sent: do not repeat, reopen the channel).
- Received data arrives as `rfcomm_data` events `{channel_id, data}` to the connection that opened the channel only.
- `rfcomm.close {channel_id}` → `null`, after the data sent before it; no `rfcomm_closed` follows.
- A channel also closes when the pipe connection ends, or with an `rfcomm_closed` event `{channel_id, reason}`: `remote` (the device closed it or the link went down), `disconnected` (the A2DP connection ended), `stopped` (the Bluetooth stack stopped), `overflow` (the client left more than 1 MiB of received data unread).

## Compatibility

- **Features.** Check `welcome.features` before using an addition. Cores before 2.0.2-beta.10 leave the field out.
- **Unknown requests.** A request the core cannot read (an unknown method, bad parameters) is answered with the error `ipc.bad_request`; the connection stays open. Cores before 2.0.2-beta.10 close the connection instead. Any other unreadable frame closes it.
- **New fields.** The core adds fields to the state and to events over time. Ignore fields you do not know; do not reject a message for them.
- **Rename in 3.0.0** ([#193](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/193)): the pipe becomes `\\.\pipe\bcfw-<SID>`. The core keeps listening on `\\.\pipe\bluecaster-<SID>` as well, with the same protocol, for at least 6 months after 3.0.0; its removal is announced in the release notes first. Try `bcfw-<SID>` first and fall back to `bluecaster-<SID>`. The core's mutex changes its name without such a transition.

## Example

A minimal Python client (standard library only) that prints the stream state and follows its changes:

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

To try a tool without touching the installed BlueCaster, run a development build with `BLUECASTER_INSTANCE` and `BLUECASTER_CONFIG_DIR` set (see [Building](building)) and connect to its pipe.
