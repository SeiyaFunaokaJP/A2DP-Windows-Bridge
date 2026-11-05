---
title: 연동 (IPC)
layout: default
parent: 홈
nav_order: 2.7
---

# 연동 (IPC)
{: .no_toc }

타사 도구가 A2DPWB 코어와 통신하는 방법입니다. 스트림 상태를 표시하고, 프로필을 시작·중지하고, 볼륨을 설정하거나, A2DPWB의 Bluetooth 링크를 통해 헤드폰 자체의 서비스에 접근할 수 있습니다. 이런 일을 하는 도구는 [함께 쓰는 도구](works-with)에 있습니다.

{: .warning }
> 프로토콜은 2.0.2 베타 동안 아직 바뀔 수 있습니다. 기준이 되는 것은 [`app/src/crates/ipc/src/protocol.rs`](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/app/src/crates/ipc/src/protocol.rs)와 이로부터 생성된 [`app/src/ui/src/ipc/bindings`](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/tree/main/app/src/ui/src/ipc/bindings)의 TypeScript 타입입니다. 이 페이지는 그 사용법을 설명하며, 모든 필드를 나열하지는 않습니다.

## 목차
{: .no_toc .text-delta }

1. TOC
{:toc}

## 연결 {#connecting}

코어(`a2dpwb-core.exe`)는 A2DPWB가 실행되는 동안, 창이 있든 없든 실행됩니다. 코어는 자신을 실행한 사용자의 named pipe에서 대기합니다.

```
\\.\pipe\a2dpwb-<user SID>
```

- `<user SID>`는 로그온한 사용자의 SID 문자열(`S-1-5-21-...`)로, 예를 들어 `whoami /user`로 알 수 있습니다. 사용자마다 각자의 코어와 파이프가 있으며, 파이프의 DACL은 그 사용자만 허용하고 원격 클라이언트는 거부됩니다.
- 파이프는 바이트 모드입니다. 여러 클라이언트가 동시에 연결할 수 있으며(A2DPWB 창도 그중 하나), 각 클라이언트는 같은 상태와 이벤트를 받습니다.
- 파이프가 없으면 코어가 실행되지 않은 것입니다. 이를 확인할 때는 코어의 뮤텍스가 아니라 파이프를 사용하십시오.
- `A2DPWB_INSTANCE=<name>`으로 시작한 개발 빌드는 대신 `\\.\pipe\a2dpwb-<SID>-<name>`에서 대기하므로, 설치된 버전과 나란히 실행할 수 있습니다.

### 프레임 {#frames}

양방향의 모든 메시지는 프레임 하나입니다. JSON의 길이를 **u32 little-endian**으로 쓰고, 그 뒤에 JSON(UTF-8)이 옵니다. 4 MiB를 넘는 프레임은 거부됩니다.

### 핸드셰이크 {#handshake}

1. 클라이언트는 연결 후 5초 안에 `hello`를 보냅니다.
   ```json
   {"type": "hello", "protocol": 1, "client": "mytool/1.2.0"}
   ```
   `client`는 자유로운 이름과 버전이며, 코어의 로그에 나타납니다.
2. 코어는 `welcome`으로 응답하거나, `reject`로 응답하고 파이프를 닫습니다(프로토콜 버전이 다르거나 hello가 없을 때).
   ```json
   {"type": "welcome", "protocol": 1, "server": "a2dpwb-core/2.0.2-beta.12", "features": ["rfcomm"]}
   {"type": "reject", "reason": "protocol 2 not supported (core speaks 1)"}
   ```
3. 코어는 전체 상태를 담은 `snapshot`을 보내고, 그다음 `event`를 보냅니다.

프로토콜 버전은 정확히 일치해야 합니다. 버전 1을 깨뜨리지 않는 추가 사항은 대신 `features`로 알립니다([호환성](#compatibility) 참조).

## 상태와 이벤트 {#state-and-events}

```json
{"type": "snapshot", "rev": 41, "state": {"adapter": {...}, "devices": [...], "stream": {...}, "profiles": [...], "active_profile": "WH-1000XM5", "settings": {...}, ...}}
{"type": "event", "rev": 42, "event": {"kind": "stream", "value": {"state": "streaming", "profile": "WH-1000XM5", "info": {"codec": "ldac", "bitrate_kbps": 990, ...}, "volume": 80, ...}}}
```

클라이언트는 자체 상태를 갖지 않습니다. 마지막 스냅샷을 보관하고 각 이벤트를 거기에 적용합니다. 각 상태 이벤트는 스냅샷의 **한 부분 전체를 대체**합니다.

| 이벤트 `kind` | 대체하는 부분 |
|:--|:--|
| `adapter` | `adapter` |
| `devices` | `devices` |
| `scanning`, `le_scanning` | `scanning`, `le_scanning` |
| `stream` | `stream` (상태, 메시지, 프로필, 코덱 정보, 볼륨, 배터리) |
| `profiles` | `profiles`와 `active_profile` |
| `settings` | `settings` |
| `update` | `update` |
| `capture` | `capture` |
| `v1_running`, `verbose`, `crashes` | 같은 이름의 필드 |

`stream.state`는 `idle`, `connecting`, `streaming`, `reconnecting`, `error` 중 하나이며, `stream.message`가 그 이유를 [메시지 키](#errors-and-messages)로 알려 줍니다.

**리비전.** 모든 상태 이벤트는 다음 리비전(`rev` + 1)을 담고 있습니다. 번호가 건너뛰면 이벤트가 유실된 것이므로 `state.snapshot`을 보내 새 스냅샷을 받으십시오. 읽는 속도가 너무 느린 클라이언트도 요청하지 않아도 코어로부터 새 `snapshot`을 받습니다. 상태가 아닌 이벤트(`link`, `log`, `receiver`, `rfcomm_data`, `rfcomm_closed`)는 현재 리비전을 담으며 리비전을 올리지 않습니다.

코어가 종료될 때는 이벤트 `bye`를 보내고 파이프를 닫습니다.

## 요청 {#requests}

```json
{"type": "request", "id": 7, "method": "stream.start", "params": {"profile": "WH-1000XM5"}}
{"type": "response", "id": 7, "result": null, "error": null}
```

- `id`는 클라이언트가 고르는 임의의 숫자이며, 응답에 그대로 담겨 돌아옵니다. 요청은 동시에 실행되므로 응답이 요청과 다른 순서로 올 수 있습니다.
- 매개변수가 없는 메서드에서는 `params`를 생략합니다.
- 상태를 바꾸는 요청은 처리가 끝나면 응답하며, 그 변경은 이벤트로도 도착합니다. 클라이언트는 응답이 아니라 이벤트를 바탕으로 상태를 표시해야 합니다.

### 오류와 메시지 {#errors-and-messages}

```json
{"type": "response", "id": 8, "result": null, "error": {"code": "rfcomm.not_connected", "message": {"key": "ipc.error.rfcomm_not_connected", "args": []}}}
```

`code`는 프로그램용입니다(`adapter.driver_missing`, `app.streaming`, ...). `message`는 상태 안의 상태 텍스트와 마찬가지로 A2DPWB 언어 파일([`app/lang`](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/tree/main/app/lang))의 번역 키와 printf 형식의 인수입니다. 도구는 거기에서 키를 찾아보거나, 코드에 맞는 자체 텍스트를 표시할 수 있습니다.

### 메서드 {#methods}

도구용 메서드:

| 메서드 | 매개변수 | 결과 |
|:--|:--|:--|
| `state.snapshot` | -- | `Snapshot` |
| `stream.start` | `profile` | `null`. 실행 중인 스트림이 있으면 먼저 중지 |
| `stream.stop` | -- | `null` |
| `stream.set_volume` | `volume` (AVRCP 절대 볼륨 0-127) | `null` |
| `profile.set_active` | `name` | `null` |
| `device.scan` | `seconds` (3-600) | `Device[]`, `devices` 이벤트로도 전달 |
| `device.scan_stop` | -- | `null`. 실행 중인 `device.scan`이 응답함 |
| `audio.devices` | -- | `AudioDevice[]` (캡처 소스) |
| `diag.subscribe` | `logs` (수준 또는 `null`), `link` (bool), `receiver` (bool, 선택) | `null`. 이 연결에 대해 `log` / `link` / `receiver` 이벤트를 시작하거나 중지 |
| `diag.info` | `{"mask": bool}` (선택) | `DiagInfo` ("진단 정보 복사"의 텍스트) |
| `rfcomm.open`, `rfcomm.send`, `rfcomm.close` | [RFCOMM 패스스루](#rfcomm-passthrough) 참조 | |

A2DPWB 창이 사용하며 도구는 주의해서만 써야 하는 메서드: `profile.save` / `profile.delete`, `settings.set`(설정을 **모두** 대체합니다. 일부만 담은 객체는 절대 보내지 말고, 최신 `settings`에 변경 사항을 더해 보내십시오), `device.forget` / `device.rename`, `adapter.*`(`adapter.assign_winusb`와 `adapter.restore_driver`는 UAC 프롬프트를 표시하고, 개발자 모드의 칩 도구인 `adapter.probe`와 `adapter.run_script`는 Bluetooth 스택을 중지합니다), `app.quit`. 그 밖의 메서드(`stream.start_custom`, 나머지 `device.*`(`device.import_windows_pairings`는 UAC 프롬프트를 표시), `audio.apps`, `audio.mixer` / `audio.mixer_set`, 위 두 가지 외의 `diag.*`, `receiver.*`, `app.*`)는 창 자체의 화면을 위한 것이며 예고 없이 바뀔 수 있습니다. 매개변수와 결과를 포함한 전체 목록은 `protocol.rs`의 `Method` enum입니다.

## RFCOMM 패스스루 {#rfcomm-passthrough}

`welcome.features`에 `"rfcomm"`이 있을 때 사용할 수 있습니다(2.0.2-beta.10 이상). A2DPWB를 통해 연결된 헤드폰은 Windows에 보이지 않으므로, 도구가 A2DPWB가 이미 가진 링크를 통해 헤드폰 자체의 RFCOMM 서비스(제조사 제어 앱: 배터리, 노이즈 컨트롤, EQ)에 접근할 수 있게 합니다.

- `rfcomm.open {address, uuid}` → `{channel_id, mtu}`. `address`는 스트리밍 중인 장치여야 하며, 코어는 이를 위해 연결하거나 페어링하지 않습니다(`rfcomm.not_connected`). `uuid`는 128비트 전체 UUID이거나, Bluetooth 기본 UUID 위의 16비트 `"xxxx"`입니다. 다른 open 뒤에 대기 중일 때도 응답은 20초 안에 오므로, 최소 25초는 기다리십시오. 오류: `rfcomm.not_connected`, `rfcomm.service_not_found`, `rfcomm.open_failed`, `rfcomm.busy`.
- `rfcomm.send {channel_id, data}` → `null`. `data`는 base64(표준, 패딩 있음)이며 요청당 최대 64 KiB이고, 요청 순서대로 최대 `mtu` 크기의 프레임으로 전송됩니다. 오류: `rfcomm.no_channel`, `rfcomm.busy`(대기열이 가득 차서 전송되지 않음), `rfcomm.timeout`(전송되었는지 알 수 없음: 반복하지 말고 채널을 다시 여십시오).
- 수신한 데이터는 채널을 연 연결에만 `rfcomm_data` 이벤트 `{channel_id, data}`로 도착합니다.
- `rfcomm.close {channel_id}` → `null`. 그 전에 보낸 데이터가 전송된 뒤에 닫히며, 그 뒤에 `rfcomm_closed`는 오지 않습니다.
- 채널은 파이프 연결이 끝날 때도 닫히고, `rfcomm_closed` 이벤트 `{channel_id, reason}`과 함께 닫히기도 합니다: `remote`(장치가 닫았거나 링크가 끊김), `disconnected`(A2DP 연결이 끝남), `stopped`(Bluetooth 스택이 중지됨), `overflow`(클라이언트가 읽지 않은 수신 데이터가 1 MiB를 넘음).

## 호환성 {#compatibility}

- **기능.** 추가 사항을 사용하기 전에 `welcome.features`를 확인하십시오. 2.0.2-beta.10 이전의 코어에는 이 필드가 없습니다.
- **알 수 없는 요청.** 코어가 읽을 수 없는 요청(알 수 없는 메서드, 잘못된 매개변수)에는 오류 `ipc.bad_request`로 응답하며, 연결은 유지됩니다. 2.0.2-beta.10 이전의 코어는 대신 연결을 닫습니다. 그 밖의 읽을 수 없는 프레임은 연결을 닫습니다.
- **새 필드.** 코어는 시간이 지나면서 상태와 이벤트에 필드를 추가합니다. 모르는 필드는 무시하고, 그 때문에 메시지를 거부하지 마십시오.
- **3.0.0에서의 이름 변경**([#193](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/193)): 파이프는 `\\.\pipe\bcfw-<SID>`가 됩니다. 코어는 3.0.0 이후 최소 6개월 동안 같은 프로토콜로 `\\.\pipe\a2dpwb-<SID>`에서도 계속 대기하며, 이를 없앨 때는 먼저 릴리스 노트에서 알립니다. 먼저 `bcfw-<SID>`를 시도하고, 안 되면 `a2dpwb-<SID>`로 돌아가십시오. 코어의 뮤텍스는 이런 전환 기간 없이 이름이 바뀝니다.

## 예제 {#example}

스트림 상태를 출력하고 그 변화를 따라가는 최소한의 Python 클라이언트(표준 라이브러리만 사용):

```python
import json, struct, subprocess

sid = subprocess.run(["whoami", "/user", "/fo", "csv", "/nh"],
                     capture_output=True, text=True).stdout.strip().split(",")[1].strip('"')
pipe = open(rf"\\.\pipe\a2dpwb-{sid}", "r+b", buffering=0)

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

설치된 A2DPWB를 건드리지 않고 도구를 시험하려면, `A2DPWB_INSTANCE`와 `A2DPWB_CONFIG_DIR`을 설정한 개발 빌드를 실행하고([빌드](building) 참조) 그 파이프에 연결하십시오.
