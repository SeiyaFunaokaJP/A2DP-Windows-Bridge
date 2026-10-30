---
title: LE Audio (developer note)
layout: default
nav_exclude: true
---

# LE Audio (experimental) - developer note

BlueCaster can stream to LE Audio devices as a **BAP unicast client** (audio
source): LC3 over Connected Isochronous Streams (CIS), on the same BTstack
session and WinUSB adapter as A2DP. It is **experimental**, used only for a
device's LE Audio entry (or `--le-audio` in CLI mode). `le_audio_experimental`
in `settings.json` (on by default, no switch in the window) turns the LE side
off altogether. With the setting off none of the LE code runs and HCI init is that of a build
without LE (see "LE Audio off"). It has only been verified against the Bumble
emulator (`tools/emu`), not yet with real headphones or a real 5.2
controller.

The code was ported from v1 (C++, branch `feat/le-audio`, commit `b819893`)
on 2026-10-18 with v1's behaviour, constants and log lines; the emulator's
checks parse those lines, so keep them verbatim. The v1 sources stay in git
for reference (`git show b819893:app/src/le_audio/<file>`).

## Where the code is

| Path | What |
|------|------|
| `app/src/crates/le-audio/` | Pure logic, no BTstack, `#![forbid(unsafe_code)]`: `defs` (UUIDs, LTV types, contexts, locations, ASE states), `ltv`, `pacs_ascs` (PAC records, ASE values, Control Point PDUs), `presets` (BAP / GMAP configurations), `plan` (`plan_stream`: preset, ASEs, CIS, channels), `csip_crypto` (AES-128, AES-CMAC, s1 / k1 / sef / sdf / sih, RSI), `vcp_scale`, `ccp_model` (call model, simulation), `mcp_model` (media state, `SimulatedPlayer`), `db_hash` (Database Hash), `sdu_queue` (jitter buffer, send guard, counters and their log lines) |
| `app/src/crates/lc3-sys/` | liblc3 (`extern/btstack/3rd-party/lc3-google`, Apache-2.0) built with `cc`; `Lc3Encoder` |
| `app/src/crates/codec/src/lc3.rs` | LC3 behind the `codec` crate's `lc3` feature (not a `CodecKind`) |
| `app/src/crates/btstack-sys/` | `btstack_config.h` LE block (`ENABLE_BLE`, LE central / peripheral, LE Secure Connections with micro-ecc, extended advertising, `ENABLE_LE_ISOCHRONOUS_STREAMS`, `MAX_NR_CIS 4`, `HCI_ISO_PAYLOAD_SIZE 400`, `NVN_NUM_GATT_SERVER_CCC 256`); `build.rs` adds `src/ble`, micro-ecc and `btstack_tlv_windows.c`; `src/le.rs` (Stack's LE API: scan, connect, SM, LE device DB, TLV, GATT client, att_db_util, att_server, CIG / CIS, ISO send), `src/att.rs` (ATT read / write callbacks, notification queue), `src/events.rs` (event readers), `src/transport.rs` (LE hidden, below), `src/timer_resolution.rs` (`HighTimerResolution`) |
| `app/src/crates/hci-transport/src/usb.rs` | HCI ISO data on the ACL bulk endpoints (OUT: the bulk ACL slot; IN: passed up as ACL, `hci.c` reclassifies by CIS handle), as v1 and Linux btusb |
| `app/src/crates/bt-host/src/le/` | The LE core: `mod.rs` (public types), `api.rs` (`BtHost::le_*`, worker side), `core.rs` (`LeCore`, `LeCtx`, scan / connect / SM / discovery, module dispatch), `gatt_client.rs` (per-connection queue), `gatt_server.rs` (the PC's GATT server), `bap.rs` (PACS / ASCS client, `Unicast` state machine), `iso_send.rs` (`IsoSender`), `module.rs` (hook traits) |
| `app/src/crates/bt-host/src/le/modules/` | The profiles: `csip.rs`, `vcp.rs`, `mcp.rs`, `ccp.rs`, `tmap_gmap.rs`; `mod.rs` registers them (v1's order: CSIP, VCP, MCP, CCP, TMAP / GMAP) |
| `app/src/crates/pipeline/src/le.rs` | The LE encode thread: capture -> stereo downmix -> linear resampling to the LC3 rate -> LC3 per channel -> SDU set per frame into the `LeSduSink` |
| `app/src/crates/core/src/le_stream.rs` | The session of an LE Audio profile (v1 `a2dp_service_le_audio.cpp`): connect, start, VolumeSync, re-setup after a loss (10 times), the `error.le_audio_*` messages |
| `app/src/crates/core/src/le_media.rs` | MCP's Windows backend (System Media Transport Controls, media keys) |
| `app/src/crates/core/src/service.rs` | `HostConfig.le_audio` from the setting or the CLI, the deferred toggle, `scan_devices` (classic + LE entries), `le_stream()` |
| `app/src/crates/core/src/devices.rs` | `le_audio_devices.txt` (`load_le` / `save_le` / `remove_le`) |
| `app/src/core/src/cli.rs` | `--le-audio`, `--le-device`, `--le-preset`, `--le-low-latency`, `--le-scan`, `--le-sim-volume`, `--mcp-dry-run`, `--ccp-sim`, `--ccp-console` (v1 `le_audio_cli.cpp`'s flow and lines) |
| `app/src/core/src/state.rs`, `resident.rs` | The UI's state: LE settings, LE devices, `le_scanning`, `StreamInfo.le`, `LinkSample.iso`, `settings.set`'s `le_audio_deferred`; diagnostics section `[LE Audio (experimental)]` (`crates/core/src/diag.rs`) |
| `app/src/ui/src/` | Settings toggles, ProfileEditor's "LE Audio device" check box, DevicePicker's LE tags, Diagnostics' ISO counters; `?mock=le` mocks LE |
| `tools/a2dp_decode/le_audio_decode.cpp` | `bluecaster_decode --le-audio` |
| `tools/emu/emu_le_audio_sink.py`, `tools/emu/le_audio/` | Bumble LE Audio sink(s); `le_audio/<profile>.py` = the sink side of each profile |
| `tools/emu/le_audio_tests/` | `run_test.py --le-audio` cases and checks, one file per profile (`core.py`, `privacy.py`, `csip.py`, `vcp.py`, `mcp.py`, `ccp.py`, `tmap_gmap.py`) |

Files in the config folder (the same as v1): `le_audio_bonds.tlv` (LE bonds
and the GATT server's persistent CCC values), `le_audio_csip.txt`
(coordinated sets), `le_audio_devices.txt` (LE Audio entries of the device
list).

## Threads - the one rule that matters

BTstack is single-threaded. Everything that touches BTstack, an `LeDevice`,
the GATT client or the GATT server runs on the **BTstack thread**, through an
`LeCtx`:

- `LeModule` hooks (all of them), GATT client callbacks (`GattValue`,
  `GattDone`, `GattNotify`, `GattDiscovered`), timers (`LeCtx::run_after`)
  and posted closures get `&mut LeCtx`. They must not block. A callback
  reaches its module with `ctx.module_mut::<XModule>()` (None for the module
  whose own hook is running: the core takes a module out of its registry
  while its hook runs).
- The worker side (`BtHost::le_*`, `LeGroupHooks::prepare_group` /
  `adjust_stream_request`) blocks with timeouts and reaches the BTstack
  thread with `BtHost::le_run(timeout, f)` (run and wait for the result) or
  `BtHost::le_call(timeout, f(ctx, done))` (`f` calls `done(ok)` later, e.g.
  from a GATT callback). A user stop ends these waits (the host's cancel),
  except the teardown ones (`le_stop_stream`, `le_disconnect(_all)`).
- Any thread: `LePoster::post(f)` (the module gets its poster in
  `LeModuleSetup`); `LeGroupHooks::remote_volume` / `request_volume` keep
  their state in atomics or a mutex and post the GATT work.
- Never call `le_run` / `le_call` or the blocking `BtHost::le_*` from the
  BTstack thread: they wait for it and would time out.
- Closures for `le_run` capture owned data only: after a timeout they may
  still run later.
- `LeCore::run` "settles" after every entry: it starts queued GATT
  operations, runs finished callbacks (never inside the call that queued
  them), hands GATT server notifications to `att_server_send` and arms the
  LE timer.

## Lifecycle

1. `HostConfig.le_audio = Some(LeConfig { bond_dir, media, ccp_sim,
   ccp_console })` (the service sets it from the setting, or from `--le-audio`;
   None = LE hidden). LE is unhidden before `hci_init`.
2. Stack init (BTstack thread, after the classic endpoints): LE bond TLV,
   `le_device_db_tlv`, SM (Secure Connections, bonding, Just Works), GATT
   client, HCI / SM / ISO handlers, the modules (`modules::create_all`, once
   per BtHost), **`on_stack_init`**, the GATT server (GAP, GATT with Service
   Changed and Database Hash, **`add_gatt_services`**), `att_server_init`.
3. HCI working: `le_ready()`; ISO buffers (LE Read Buffer Size v2) in
   `le_iso_buffers()`. If either is 0 the adapter has no ISO support and
   `le_start_stream` refuses ("adapter does not support LE Audio (no ISO
   buffers)"): BTstack's ISO send would loop for ever.
4. `le_connect(addr, name, timeout)`: `gap_connect` -> **`on_connected`** ->
   SM pairing / re-encryption (a bond the device lost is deleted and pairing
   repeated once) -> **`on_encrypted`** -> GATT discovery into
   `LeDevice::services` -> PACS / ASCS reads and notifications ->
   **`on_gatt_ready`** -> the device id. On timeout / cancel the connect is
   cancelled; a link that still comes up afterwards is disconnected.
5. `le_start_stream(devices, request, silence)`: **`prepare_group`** (worker)
   -> **`adjust_stream_request`** (worker) -> `plan_stream` -> `Unicast`:
   Config Codec -> LE Set CIG Parameters -> Config QoS -> Enable (contexts +
   CCID list; waits for an available context) -> LE Create CIS (all CIS at
   once) -> sink ASEs go Streaming -> `IsoSender` starts ->
   **`on_stream_state(status, true)`**. Returns the `LeSduSink` the encode
   thread pushes to.
6. `le_stop_stream()`: Disable -> HCI_Disconnect of the CIS -> Release ->
   remove the CIG -> **`on_stream_state(status, false)`**. Devices stay
   connected (`le_disconnect_all()` drops them).
7. A link or CIS lost while streaming: `le_stream_lost()` turns true once; the
   session stops and sets the stream up again (CLI: 3 tries, GUI: 10).
   **`on_disconnected(dev, reason)`** for every LE link that goes down.
8. Stack deinit (run loop stopped): **`on_stack_deinit`**, no BTstack calls.
   Device ids start from 0 again with the next BtHost.

## Module hooks (`le/module.rs`)

A module file has `pub fn create(setup: &mut LeModuleSetup) -> LeModuleEntry`
returning its `LeModule` and, if it has worker / volume hooks, an
`Arc<dyn LeGroupHooks>` sharing state with it.

| Hook | Thread | When / what for |
|------|--------|-----------------|
| `name()` | - | log name |
| `on_stack_init(ctx)` | BTstack | session start, before HCI power on |
| `add_gatt_services(builder)` | BTstack | the PC-side services of the profile (only here) |
| `on_stack_deinit()` | BTstack (loop stopped) | drop all state |
| `on_scan_result(ctx, result, ad)` | BTstack | every advertising report while a scan / find runs; may set `le_audio`, `set_id`, `note` |
| `on_connected` / `on_encrypted` / `on_gatt_ready(ctx, id)` | BTstack | link up / encrypted / services and PACS / ASCS read (queue your reads and subscriptions here) |
| `on_disconnected(ctx, id, reason)` | BTstack | link down; the device still has its last data |
| `on_stream_state(ctx, status, streaming)` | BTstack | audio flows (true) / stopped or lost (false) |
| `add_ccids(contexts, ccids)` | BTstack | CCIDs for the Enable metadata (MCP: `CCID_GMCS` for every context but pure Conversational; CCP: `CCID_GTBS` for Conversational) |
| `on_att_read(ctx, read)` / `on_att_write(ctx, write)` | BTstack | reads / writes of this module's characteristics (whole value; the server serves offsets and assembles prepared writes) |
| `prepare_group(host, ids, cancelled)` | **worker** | CSIP: append the other set members (may scan / connect) |
| `adjust_stream_request(host, ids, request)` | **worker** | GMAP: low-latency presets first, the Game context |
| `remote_volume()` | any | headset volume 0-127, None unknown; the first `Some` wins |
| `request_volume(level)` | any | set the headset volume; true if handled |

`LeDevice` (`le/core.rs`): `id`, `addr` (the identity once known),
`peer_addr` (the address of the current link, maybe an RPA), `name`, `acl`,
`encrypted`, `ready`, `services` (`find_service`, `find_char`,
`find_chars`; secondary / included services have `primary == false` and
`included_by`), PACS (`sink_pacs`, `sink_locations`, available / supported
contexts), ASCS (`ases`, `ase_cp_handle`). In callbacks check that the
device is still the same link (`LeCtx::device_on(id, con)`), as the BAP
client does.

### Addresses and privacy

Devices are keyed by identity: `addr` becomes the identity on SM's identity
events (pairing, or a reconnect with a resolvable private address); a device
keeps its id across reconnects with new RPAs. While scanning every RPA is
looked up (`sm_address_resolution_lookup`); `LeScanResult::identity` holds
the result. `le_find(addr)` matches the address, an advertiser whose RPA
resolves to it, or one resolving to the same identity as a saved RPA;
`le_connect` uses the RPA the device was last seen with (within 5 minutes).
Software resolution only (no controller resolving list), so it works with any
5.2 controller. Not covered: RPAs without a bond, RSIs that change with the
RPA. Emulator case `privacy-rpa`.

## GATT client: always through the queue

BTstack's `gatt_client` allows one query per connection. Never call it
directly: use `ctx.gatt()` (`le/gatt_client.rs`):

```rust
fn on_gatt_ready(&mut self, ctx: &mut LeCtx<'_>, id: LeDeviceId) {
    let Some(dev) = ctx.device(id) else { return };
    let Some(state) = dev.find_char(UUID_SERVICE_VCS, UUID_VOLUME_STATE).cloned() else { return };
    let con = dev.acl;
    ctx.gatt().listen(con, state.value_handle, Box::new(move |ctx, value| { /* notifications */ }));
    ctx.gatt().read(con, state.value_handle, Box::new(move |ctx, status, value| {
        if let Some(m) = ctx.module_mut::<VcpModule>() { /* status 0: value */ }
    }));
    ctx.gatt().subscribe(con, &state, false, None);
}
```

- `read` (long reads included), `write` (with response; a long write when
  the value does not fit the MTU), `subscribe` (writes the CCC),
  `discover_all`, `listen`.
- Operations run in order per connection; callbacks get status 0 or an ATT
  error / BTstack status; when the link goes down everything pending
  completes with `ATT_ERROR_HCI_DISCONNECT_RECEIVED` (0x1F) and listeners are
  dropped. A busy `gatt_client` (BTstack's own MTU exchange) is retried.

## GATT server: services of the PC

`add_gatt_services(builder)` only (the database is fixed once att_server
runs): `begin_service(AttUuid, primary)`, `add_characteristic(uuid,
properties, read_security, write_security)` -> value handle (CCC at + 1 with
NOTIFY / INDICATE), `end_service()`. Reads and writes come to the module's
`on_att_read` / `on_att_write` synchronously (btstack-sys's ATT callbacks
reach the app through a thread-local handler; a request that arrives while
the app is busy is answered "pending" and repeated by BTstack).

- `ctx.gatt_server().notify(con, handle, value)` / `indicate(...)` send only
  to clients that enabled it (`client_config(con, handle)`), queue until ATT
  can send, and are cut to ATT_MTU - 3 by btstack-sys (modules that must cut
  at a UTF-8 boundary or per spec do it themselves: MCP, CCP).
- CCC values of **bonded** clients persist in `le_audio_bonds.tlv` (BTstack's
  `BTC` tags) and are replayed when the link is encrypted again (emulator
  case `mcp-resume`).
- The GATT service has Service Changed and a **Database Hash** (0x2B2A,
  AES-CMAC over the DB, `le_audio::db_hash`; `mcp-read` checks it). When the
  stored hash (`BTDB`) differs, the stored CCC values are dropped and bonded
  clients with Service Changed indications get one (0x0001-0xFFFF) on their
  next encrypted link (`AWS` tag until then). No Robust Caching.
- Prepared writes are executed for all handles of the connection (BTstack
  passes handle 0 to EXECUTE; v1 dropped them).

## Stream configuration

`PresetTable::new()` (`le_audio::presets`) holds BAP Table 5.2 by name
(`16_1_1` ... `48_6_2`) and GMAP's `_gr` presets. `StreamRequest::preset_order`
(default `48_6_2, 48_4_2, 48_2_2, 32_2_2, 24_2_2, 16_2_2`) is tried in order;
the first preset every device's sink PAC records allow and whose SDU fits
one HCI ISO data packet (SDU + 4 <= ISO_Data_Packet_Length) wins.

Channels: a device with Front Left + Right gets one 2-channel ASE when its
PAC allows 2 channels and the SDU fits, else two 1-channel ASEs, else one
mono ASE; a device with one location gets one ASE with it; no location =
mono. Left / right audio go to FL / FR only when the group has both;
otherwise every channel carries the downmix. One CIG (id 1, another id when
BTstack still holds it), CIS in plan order, at most `MAX_CIS` (4). The
presentation delay is clamped into every ASE's range, the max transport
latency capped by the servers' preference; framed ISO when an ASE cannot do
unframed.

Streaming Audio Contexts (Enable): the requested contexts available on
every device, else Unspecified if available; evaluated again right before
Enable, which waits for an Available Audio Contexts notification when
nothing is available ("no audio context available for Enable"). The CCID
list comes from the modules' `add_ccids` (log: "Enable metadata: contexts
0x0004, CCID list 1").

## Audio path

`le_stream.rs` asks the capture for the plan's LC3 rate; `pipeline::le`
downmixes any PCM to stereo, resamples linearly if needed, encodes LC3 per
channel and pushes one SDU set per frame (10 or 7.5 ms) into the jitter
buffer (`le_audio::sdu_queue::SduQueue`: prefill queue + 3 sets, one more per
underrun up to queue + 7, 16 at most, the oldest dropped beyond). No
allocation per frame: sets are recycled through the queue's pool.

The BTstack thread sends on `HCI_EVENT_CIS_CAN_SEND_NOW` (`le/iso_send.rs`):
complete SDUs, sequence numbers per CIS, **no time stamp** (the controller
places SDUs by sequence number), up to 2 SDUs queued per CIS
(`hci_set_num_iso_packets_to_queue`; queue x CIS within the controller's ISO
buffers), and a send guard that keeps the sent sets at most queue + 2
intervals ahead of real time. A round starts once every CIS still up got its
SDU, so one CIS going down does not stall the others. Underrun -> encoded
silence ("LE Audio: no audio for SDU set N ..."). While sending, the system
timer resolution is 1 ms (`btstack_sys::HighTimerResolution`, as v1's
`timeBeginPeriod(1)`): BTstack's Windows run loop and its time otherwise
move in 15.6 ms steps. At the end: "LE Audio: stopped sending: N SDU sets
(...), S silence, U underruns, ..." and, if the guard waited, how late its
timer fired.

The UI's ISO counters (`LinkSample.iso`) come from `BtHost::le_stream_stats()`
(sets, silence, underruns, dropped, prefill).

## Emulator (`tools/emu`)

`emu_le_audio_sink.py --port P --capture sink.pklg --layout L` runs BlueCaster's
controller (H4 over TCP) and one or two sinks on a Bumble `LocalLink`:

| Layout | Sinks |
|--------|-------|
| `stereo` | 1 sink `00:A2:D0:00:00:12`, FL+FR, 2 sink ASEs, 1 or 2 channels per ASE |
| `two-ase` | 1 sink, FL+FR, 2 sink ASEs of 1 channel |
| `earbuds` | 2 sinks (`...:12` FL, `...:13` FR), 1 ASE each |
| `mono` | 1 sink, no location, 1 ASE |

It prints `READY 127.0.0.1:P sinks=<addr>[,<addr>]`. Options:
`--ascs-release-waits-for-cis`, `--drop-link-after S`, `--privacy`,
`--no-iso-buffers`, and the profiles' own (`--csip-set`, `--vcp`, `--mcp-client`,
`--ccp`, `--tmap-role`, `--gmap-role`, ...; see each `le_audio/<profile>.py`).

Emulator facts to know:
- Legacy advertising, 31 bytes: the name gets what flags + ASCS + CAS
  service data (and a CSIP RSI) leave.
- `LeEmuController` (`le_audio/controller.py`) carries ISO data between the
  controllers, one SDU per interval per CIS. An SDU that arrives more than
  2 ms after its interval began is logged as `late SDU` and fails the case (a
  real controller's queue of 2 SDUs per CIS would have run empty); the
  emulator's own event-loop stalls are not counted. It runs asyncio on the
  performance counter with 1 ms timers.
- Every HCI command gets an answer (unknown ones: Unknown HCI Command).
- Bumble's ASCS goes to Streaming right after the CIS is up; pairing is Just
  Works with an in-memory key store; the controller does not check LTKs, so
  the "device lost its bond" path is not covered.
- When an LE link drops, the CISes are reported down first, as real
  controllers do.
- The sink writes UTF-8 (`run_test.py` sets `PYTHONIOENCODING` /
  `PYTHONUTF8` for it: a Japanese Windows would otherwise write cp932).

## Running the LE cases

Once: `python tools/emu/setup_env.py` (Bumble venv in `tools/emu/.venv`).
Then:

```
pwsh ./build.ps1 -Decoder                       # bluecaster_decode -> build/a2dp_decode/Release
cargo build --release -p bluecaster-core-exe        # target/release/bluecaster-core.exe
tools\emu\.venv\Scripts\python.exe tools\emu\run_test.py --le-audio
tools\emu\.venv\Scripts\python.exe tools\emu\run_test.py --le-audio stereo earbuds --base-port 10200 --out $env:TEMP\le
tools\emu\.venv\Scripts\python.exe tools\emu\run_test.py          # the A2DP codecs: must keep passing
```

`--le-audio` without names runs all 41 cases; CI runs them after the A2DP set
(`.github/workflows/ci.yml`, emulator job). Each case gets its own config
folder (`BLUECASTER_CONFIG_DIR=<out>/le-<case>/config`), so nothing touches
`%APPDATA%\BlueCaster`. Parallel runs need distinct `--base-port`s (each case
uses its own port from there). Per case `<out>/le-<case>/` has
`bluecaster.log`, `sink.log`, the captures and `sent_report.txt` /
`received_report.txt` from `bluecaster_decode --le-audio`.

| File | Cases |
|------|-------|
| `core.py` | `stereo`, `two-ase`, `earbuds`, `mono-16k`, `link-loss`, `release-cis`, `contexts-later`, `afh`, `no-iso-buffers` |
| `privacy.py` | `privacy-rpa` |
| `csip.py` | `csip-set`, `csip-encrypted-sirk`, `csip-missing-member`, `csip-both-named`, `csip-link-loss`, `csip-lock-denied`, `csip-no-lock` |
| `vcp.py` | `vcp-set`, `vcp-remote`, `vcp-change-counter`, `vcp-earbuds`, `vcp-press-during-write`, `vcp-press-before-write` |
| `mcp.py` | `mcp-read`, `mcp-control`, `mcp-unsupported`, `mcp-ccid`, `mcp-long-read`, `mcp-resume` |
| `ccp.py` | `ccp-read`, `ccp-incoming-accept`, `ccp-terminate`, `ccp-invalid` |
| `tmap_gmap.py` | `tmap-role`, `gmap-low-latency`, `gmap-earbuds`, `gmap-off`, `gmap-not-ugt`, `gmap-media-context` |

Each case checks: BlueCaster exits 0; one stream of the expected configuration
on both sides; no decoder problems (consecutive SDU sequence numbers,
lengths, LC3 decoding); the sinks received every SDU BlueCaster sent; decoded
audio identical on both sides and the test tone (1 kHz left, 1.5 kHz right;
mono: both) in every 100 ms of every stream but a short gap at its start and
end; no `late SDU`; no underrun in BlueCaster's "stopped sending" counters (one
as the tone stopped is allowed) and most of the duration sent as audio; each
CIS's channel allocation within the PACS locations of its sink; the
teardown steps in the log; then the checks of every
`le_audio_tests/<profile>.py`. `Case` fields: `le_audio_tests/case.py`.

A case runs `bluecaster-core.exe --cli --hci-tcp 127.0.0.1:<port> --le-audio -d
<first sink> --le-device <other sinks> --test-tone --duration N
--hci-capture ...` plus its `bluecaster_args` (`--hci-tcp` implies
`--mcp-dry-run`: MCP never controls the PC's real players).

Unit tests: `cargo test -p le-audio -p lc3-sys -p bt-host -p pipeline -p
bluecaster-core` (BAP phases, GATT queue and server, ISO rounds, each module
against a scripted peer, the CLI's parsing); btstack-sys's
`tests/le_emulator.rs` is ignored by default and runs against a sink given in
`BLUECASTER_EMU_LE=host:port`.

## The profiles

**CSIP (Set Coordinator, `csip.rs`).** Reads SIRK (plaintext, or encrypted:
decrypted with sdf and the bond's LTK), size, rank and lock of a CSIS;
devices with equal SIRKs form a set. Resolves RSIs in scan results (cache of
256). `prepare_group` waits up to 6 s for the reads, adds connected members,
scans up to 10 s for the missing ones, connects them one at a time and
orders the set by rank ("CSIP: streaming to coordinated set N, k of n
members: ..."; "only k of n" otherwise). Locks the members in rank order
before the stream (Lock Denied: back-off, 5 attempts, then without the lock)
and releases them in reverse order when the stream runs or stops (50 s at
the latest). Sets of bonded devices persist in `le_audio_csip.txt`. The
crypto self-check (CSIS appendix A) runs at stack init.

**VCP (Volume Controller, `vcp.rs`).** Volume State read and notifications,
Set Absolute Volume / Unmute with the Change Counter (one retry after 0x80),
one group volume for all connected VCS devices (later members are aligned to
it), button presses during BlueCaster's own write applied afterwards, 0-127 <->
0-255 with `vcp_scale`. `remote_volume` / `request_volume` feed VolumeSync,
so the profile's volume sync applies as for A2DP. `--le-sim-volume` runs
VolumeSync against a simulated Windows volume in the CLI.

**MCP (GMCS server, `mcp.rs`, Windows side in `core/src/le_media.rs`).**
GMCS with v1's 11 characteristics, CCID 1. The control point goes to the
current SMTC session, else media keys; the "media keys" setting off answers
Command Cannot Be Completed. `--mcp-dry-run` (always with `--hci-tcp`): a
simulated player, commands only logged ("MCP: dry run: PLAY ..."). Long reads
of changing text: 0x80; notifications cut to ATT_MTU - 3 at a UTF-8
boundary.

**CCP (GTBS server, `ccp.rs`).** A spec-correct GTBS (16 characteristics,
CCID 2) around `ccp_model::CallModel` with the simulation backend: BlueCaster has
no telephony. `--ccp-sim "<t>:<cmd>;..."` runs commands after the first
client subscribed to Call State, `--ccp-console` reads them from stdin.
Notification order as Zephyr's `tbs.c`. Call indices restart at 1 with each
BTstack session (v1 kept counting).

**TMAP / GMAP (`tmap_gmap.rs`).** The PC serves TMAS (UMS) and GMAS (UGG,
features 0x03) and reads the headset's roles ("LE Audio: <addr>: TMAP: UMR,
GMAP: UGT (UGT features 0x3C: ...)"; also in the diagnostics). With "Low
latency for gaming headsets (GMAP)" (`le_audio_low_latency`, or
`--le-low-latency`) and a group of UGTs with the Sink feature,
`adjust_stream_request` puts `48_3_gr, 48_4_gr, 48_1_gr, 48_2_gr, 32_1_gr,
32_2_gr` first with the Game context (Media when Game is not available).

## LE Audio off

Users with the setting off get the behaviour of a build without LE:

- **HCI init.** BTstack is always built with LE and would set it up on any
  dual-mode controller. With LE hidden (the default of every run;
  `Stack::set_le_hidden(false)` only when `HostConfig.le_audio` is Some),
  `btstack-sys/src/transport.rs` clears LE Supported (Controller) in Read
  Local Supported Features and the LE command bits BTstack looks up in Read
  Local Supported Commands, so BTstack skips all LE setup; LE commands it
  still sends (LE Set Scan Parameters, whose flag cannot be cleared from
  outside) are answered by the transport with Unknown HCI Command and never
  reach the controller. The emulator's A2DP set sends the same HCI commands
  as a build without LE, except that LE Set Scan Parameters is answered
  locally.
- **Toggling the setting** takes effect when BTstack starts: an idle stack
  is stopped at once; while streaming or scanning the change waits for the
  next scan or connection (`Service::le_audio_deferred()`; the UI shows v1's
  note from `settings.set`'s `le_audio_deferred`).
- **Devices and profiles.** A dual-mode device has two entries (classic and
  LE Audio, same address); an LE scan never marks a classic entry. LE Audio
  entries are saved in `le_audio_devices.txt` so that `devices.txt` stays
  readable by older versions; `addr|name|le` lines of earlier prototypes are
  still read and moved. With the setting off the UI does not list LE Audio
  entries, and an LE Audio profile fails with `error.le_audio_disabled`.
  `profiles.json` has `le_audio` only for LE Audio profiles.

## Pitfalls

- BTstack thread only, never block it; worker waits only from the worker.
- All CIS of a CIG are created at once and a CIG cannot get new CIS later:
  to change the CIS set, stop the stream and start again. BTstack never
  removes a CIG whose CIS were never established (`gap_cig_remove`
  "succeeds" without sending LE Remove CIG); btstack-sys has 4 CIG slots,
  freed when LE Remove CIG is sent, after a failed CIG_CREATED or at power
  off.
- Only one LE connection can be set up at a time: connect set members one
  after the other.
- LE ACL buffers can be small (27 bytes in the emulator): GATT writes are
  fragmented; BTstack negotiates the ATT MTU before the first query.
- Connect with the address type the advertisement showed.
- The device / CIS handles in captures differ between BlueCaster's and the
  sink's controller; `bluecaster_decode --le-audio` maps them by CIG / CIS id.

## Known limitations

- Sink ASEs only (no microphone / source ASE, no conversational streams);
  ISO data from the controller is ignored.
- No broadcast (BIS), no BASS, no CAP procedures beyond what BAP needs.
- SDUs without time stamps; no LE Read ISO TX Sync; a simple jitter buffer
  (the prefill only grows).
- Application capture is not supported for LE Audio (system loopback,
  virtual device and the test tone are).
- Not verified with real LE Audio headphones or a real controller's CIS
  scheduling; extended scanning only through BTstack's fallback to legacy
  scanning in the emulator. The aarch64 build of the LE code has not been
  checked.

## TODO (before LE Audio can leave "experimental")

Collected from the v1 profile branches (2026-10-16) and the v2 port
(2026-10-18). Everything so far is verified only against the Bumble
emulator.

Verification
- [ ] Real hardware: a Bluetooth 5.2+ adapter with ISO channels in WinUSB
  mode (the HCI ISO path over the bulk endpoints has never run on real
  hardware), an LE Audio headset and a pair of LE Audio earbuds.
- [ ] The UI with real devices: the LE Audio settings, LE devices and set
  members in the scan list, streaming an LE Audio profile; classic A2DP
  while the LE setting is on; toggling the setting while streaming; a
  dual-mode headset's two entries.
- [ ] MCP commands to real Windows players (SMTC and the media-key fallback);
  only reading SMTC was tried.
- [ ] Values written from memory because the specs could not be fetched:
  TBS (Bearer Technology 0x04, Signal Strength 255, ATT error 0x80 for a long
  read that changed, ATT 0x0D for a wrong length; the notification order
  follows Zephyr's `tbs.c`), MCS (ATT error 0x80 for a long read that
  changed), CSIS error codes (0x80-0x84) and GMAP (Table 3.16 preset values,
  UGG / UGT feature bits).
- [ ] Service Changed after a database change and the Database Hash were
  only exercised for the hash itself (`mcp-read`); a client bonded with an
  older database has no emulator case.
- [ ] Untested paths: a device that lost its bond, extended advertising
  scanning, rotating RSIs, CSIS included by CAS, RPAs on real devices
  (emulated: `privacy-rpa`).
- [ ] Emulator: late SDUs are measured against each SDU interval with the
  emulator's own stalls taken out; watch whether `late SDU` still fails now
  and then on a loaded machine (with the 1 ms timer resolution it was not
  seen on a quiet one).
- [ ] The aarch64 build (`cargo check --target aarch64-pc-windows-msvc`).

Missing features
- [ ] Source ASEs (microphone): needed for TMAP Call Gateway, real CCP calls
  and the GMAP `_gs` configurations.
- [ ] Broadcast (Auracast source, TMAP BMS, GMAP BGS).
- [ ] CSIP: SIRK / size / rank change notifications, OOB-only SIRK; after a
  link loss the CLI reconnects only the address it was given; the 10 s
  member search is fixed.
- [ ] VCP: "remember volume" per device, Mute and relative opcodes, mute
  mirrored to Windows, Volume Flags.
- [ ] MCP: fast forward / rewind and seeking state, artist in the track
  title.
- [ ] CCP: a real call source (softphone integration via
  `ccp_model::Backend`), Join, TBS instances besides GTBS, signal strength
  reports.
- [ ] TMAP / GMAP roles in the UI's device list (the diagnostics show them);
  TMAP low-latency 48_x_1 presets are only used with `--le-preset`.
- [ ] Core: SDU time stamps / LE Read ISO TX Sync, a better jitter buffer,
  application capture mode.
