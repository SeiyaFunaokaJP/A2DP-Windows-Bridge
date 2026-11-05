---
title: 集成（IPC）
layout: default
parent: 首页
nav_order: 2.7
---

# 集成（IPC）
{: .no_toc }

第三方工具如何与 A2DPWB 核心通信：显示串流状态、启动和停止配置文件、设置音量，或通过 A2DPWB 的 Bluetooth 链路访问耳机自己的服务。实现了这些功能的工具列在 [配合使用的工具](works-with) 上。

{: .warning }
> 在 2.0.2 的 beta 期间，协议仍可能发生变化。以 [`app/src/crates/ipc/src/protocol.rs`](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/app/src/crates/ipc/src/protocol.rs) 以及由它生成、位于 [`app/src/ui/src/ipc/bindings`](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/tree/main/app/src/ui/src/ipc/bindings) 的 TypeScript 类型为准。本页说明如何使用它们，并不列出每一个字段。

## 目录
{: .no_toc .text-delta }

1. TOC
{:toc}

## 连接 {#connecting}

核心（`a2dpwb-core.exe`）在 A2DPWB 运行期间运行，无论窗口是否打开。它在其运行用户的命名管道上监听：

```
\\.\pipe\a2dpwb-<user SID>
```

- `<user SID>` 是已登录用户的 SID 字符串（`S-1-5-21-...`），例如可以通过 `whoami /user` 获得。每个用户都有自己的核心和管道；管道的 DACL 只允许该用户访问，远程客户端会被拒绝。
- 管道为字节模式。可以同时连接多个客户端（A2DPWB 窗口就是其中之一）；每个客户端都会收到相同的状态和事件。
- 如果管道不存在，说明核心没有运行。请用管道而不是核心的互斥体来检查这一点。
- 以 `A2DPWB_INSTANCE=<name>` 启动的开发版本改为在 `\\.\pipe\a2dpwb-<SID>-<name>` 上监听，以便与已安装的版本并行运行。

### 帧 {#frames}

两个方向上的每条消息都是一帧：先是 JSON 的长度，为 **u32 小端序**，然后是 JSON（UTF-8）。超过 4 MiB 的帧会被拒绝。

### 握手 {#handshake}

1. 客户端在连接后 5 秒内发送 `hello`：
   ```json
   {"type": "hello", "protocol": 1, "client": "mytool/1.2.0"}
   ```
   `client` 是自由填写的名称和版本；它会出现在核心的日志中。
2. 核心以 `welcome` 应答，或以 `reject` 应答并关闭管道（协议版本不符，或没有 hello）：
   ```json
   {"type": "welcome", "protocol": 1, "server": "a2dpwb-core/2.0.2-beta.12", "features": ["rfcomm"]}
   {"type": "reject", "reason": "protocol 2 not supported (core speaks 1)"}
   ```
3. 核心发送包含其全部状态的 `snapshot`，之后发送 `event`。

协议版本必须完全一致。保持版本 1 可用的新增内容改为在 `features` 中公布（请参阅[兼容性](#compatibility)）。

## 状态与事件 {#state-and-events}

```json
{"type": "snapshot", "rev": 41, "state": {"adapter": {...}, "devices": [...], "stream": {...}, "profiles": [...], "active_profile": "WH-1000XM5", "settings": {...}, ...}}
{"type": "event", "rev": 42, "event": {"kind": "stream", "value": {"state": "streaming", "profile": "WH-1000XM5", "info": {"codec": "ldac", "bitrate_kbps": 990, ...}, "volume": 80, ...}}}
```

客户端不保存自己的状态：它持有最后一个快照，并把每个事件应用到它上面。每个状态事件都会**替换快照中完整的一个部分**：

| 事件 `kind` | 替换 |
|:--|:--|
| `adapter` | `adapter` |
| `devices` | `devices` |
| `scanning`、`le_scanning` | `scanning`、`le_scanning` |
| `stream` | `stream`（状态、消息、配置文件、编解码器信息、音量、电量） |
| `profiles` | `profiles` 和 `active_profile` |
| `settings` | `settings` |
| `update` | `update` |
| `capture` | `capture` |
| `v1_running`、`verbose`、`crashes` | 同名的字段 |

`stream.state` 是 `idle`、`connecting`、`streaming`、`reconnecting`、`error` 之一；`stream.message` 以[消息键](#errors-and-messages)说明原因。

**修订号。** 每个状态事件都带有下一个修订号（`rev` + 1）。出现间隔说明丢失了事件：请发送 `state.snapshot` 获取新的快照。读取过慢的客户端也会在未请求的情况下从核心收到新的 `snapshot`。非状态事件（`link`、`log`、`receiver`、`rfcomm_data`、`rfcomm_closed`）带有当前修订号，不会使其递增。

核心关闭时会发送事件 `bye` 并关闭管道。

## 请求 {#requests}

```json
{"type": "request", "id": 7, "method": "stream.start", "params": {"profile": "WH-1000XM5"}}
{"type": "response", "id": 7, "result": null, "error": null}
```

- `id` 是客户端任选的数字；响应会将其带回。请求并发执行，因此响应的顺序可能与请求不同。
- 没有参数的方法省略 `params`。
- 更改状态的请求在完成后应答，该变更也会以事件的形式到达。客户端应根据事件而不是响应来显示状态。

### 错误与消息 {#errors-and-messages}

```json
{"type": "response", "id": 8, "result": null, "error": {"code": "rfcomm.not_connected", "message": {"key": "ipc.error.rfcomm_not_connected", "args": []}}}
```

`code` 供程序使用（`adapter.driver_missing`、`app.streaming` 等）。`message` 是 A2DPWB 语言文件（[`app/lang`](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/tree/main/app/lang)）中的翻译键，附带 printf 风格的参数，状态中的状态文本也是如此；工具可以在那里查找该键，也可以针对该代码显示自己的文本。

### 方法 {#methods}

供工具使用的方法：

| 方法 | 参数 | 结果 |
|:--|:--|:--|
| `state.snapshot` | -- | `Snapshot` |
| `stream.start` | `profile` | `null`；会先停止正在运行的串流 |
| `stream.stop` | -- | `null` |
| `stream.set_volume` | `volume`（AVRCP 绝对音量 0-127） | `null` |
| `profile.set_active` | `name` | `null` |
| `device.scan` | `seconds`（3-600） | `Device[]`，也作为 `devices` 事件发送 |
| `device.scan_stop` | -- | `null`；正在运行的 `device.scan` 会应答 |
| `audio.devices` | -- | `AudioDevice[]`（捕获源） |
| `diag.subscribe` | `logs`（级别或 `null`）、`link`（bool）、`receiver`（bool，可选） | `null`；为此连接启动或停止 `log` / `link` / `receiver` 事件 |
| `diag.info` | `{"mask": bool}`（可选） | `DiagInfo`（“复制诊断信息”的文本） |
| `rfcomm.open`、`rfcomm.send`、`rfcomm.close` | 请参阅 [RFCOMM 直通](#rfcomm-passthrough) | |

A2DPWB 窗口使用、工具应谨慎使用的方法：`profile.save` / `profile.delete`、`settings.set`（替换**全部**设置：请发送带有你的更改的最新 `settings`，切勿发送部分对象）、`device.forget` / `device.rename`、`adapter.*`（`adapter.assign_winusb` 和 `adapter.restore_driver` 会显示 UAC 提示；`adapter.probe` 和 `adapter.run_script` 是开发者模式的芯片工具，会停止 Bluetooth 堆栈）、`app.quit`。其他方法（`stream.start_custom`、其余的 `device.*`（`device.import_windows_pairings` 会显示 UAC 提示）、`audio.apps`、`audio.mixer` / `audio.mixer_set`、上述两个之外的 `diag.*`、`receiver.*`、`app.*`）服务于窗口自己的页面，可能在不另行通知的情况下更改。包含参数和结果的完整列表是 `protocol.rs` 中的 `Method` 枚举。

## RFCOMM 直通 {#rfcomm-passthrough}

当 `welcome.features` 包含 `"rfcomm"` 时可用（2.0.2-beta.10 及更高版本）。由于 Windows 看不到通过 A2DPWB 连接的耳机，它让工具可以通过 A2DPWB 已有的链路访问耳机自己的 RFCOMM 服务（厂商控制应用：电量、降噪、EQ）。

- `rfcomm.open {address, uuid}` → `{channel_id, mtu}`。`address` 必须是正在串流的设备；核心绝不会为此连接或配对（`rfcomm.not_connected`）。`uuid` 是完整的 128 位 UUID，或基于 Bluetooth 基础 UUID 的 16 位 `"xxxx"`。应答会在 20 秒内到达，排在其他打开请求之后时也是如此：请至少等待 25 秒。错误：`rfcomm.not_connected`、`rfcomm.service_not_found`、`rfcomm.open_failed`、`rfcomm.busy`。
- `rfcomm.send {channel_id, data}` → `null`。`data` 为 base64（标准，带填充），每个请求最多 64 KiB，按请求顺序以最多 `mtu` 大小的帧发送。错误：`rfcomm.no_channel`、`rfcomm.busy`（队列已满，未发送）、`rfcomm.timeout`（不确定是否已发送：请勿重复发送，请重新打开信道）。
- 收到的数据以 `rfcomm_data` 事件 `{channel_id, data}` 到达，且只发给打开该信道的连接。
- `rfcomm.close {channel_id}` → `null`，在此之前发送的数据发送完毕后执行；之后不会再有 `rfcomm_closed`。
- 管道连接结束时信道也会关闭，或者随 `rfcomm_closed` 事件 `{channel_id, reason}` 关闭：`remote`（设备关闭了它或链路断开）、`disconnected`（A2DP 连接结束）、`stopped`（Bluetooth 堆栈停止）、`overflow`（客户端未读取的接收数据超过 1 MiB）。

## 兼容性 {#compatibility}

- **功能。** 使用新增内容之前，请检查 `welcome.features`。2.0.2-beta.10 之前的核心不包含该字段。
- **未知请求。** 核心无法读取的请求（未知方法、错误的参数）会以错误 `ipc.bad_request` 应答；连接保持打开。2.0.2-beta.10 之前的核心则会关闭连接。其他任何无法读取的帧都会关闭连接。
- **新字段。** 核心会随时间向状态和事件中添加字段。请忽略不认识的字段；不要因此拒绝消息。
- **3.0.0 中的改名**（[#193](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/193)）：管道将改为 `\\.\pipe\bcfw-<SID>`。在 3.0.0 之后至少 6 个月内，核心仍会以相同的协议同时在 `\\.\pipe\a2dpwb-<SID>` 上监听；移除前会先在发布说明中公布。请先尝试 `bcfw-<SID>`，失败时再回退到 `a2dpwb-<SID>`。核心的互斥体会改名，但没有这样的过渡期。

## 示例 {#example}

一个最小的 Python 客户端（仅使用标准库），打印串流状态并跟踪其变化：

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

要在不影响已安装的 A2DPWB 的情况下试用工具，请设置 `A2DPWB_INSTANCE` 和 `A2DPWB_CONFIG_DIR` 运行开发版本（请参阅[构建](building)），并连接到其管道。
