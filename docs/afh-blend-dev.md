---
title: AFH Blend (developer note)
layout: default
nav_exclude: true
---

# AFH Blend (experimental) - developer note

**Blend** is an AFH policy next to Auto, Off and Manual, offered only in
developer mode (`blend-mild`, `blend-medium`, `blend-strong`; without
developer mode the core runs a saved `blend-*` as `auto`). It is not the
default and must not become one before the measurements below show it
helps. What users see is in [Link Quality and AFH](usage#afh-blend).

## Why

Auto avoids only the PC's own connected 2.4 GHz Wi-Fi channel. On
2026-10-17, avoiding every access point at -75 dBm or louder (v1 and early
v2) took a loud but idle neighbour's channel 11, left 56 of 79 channels, and
raised gaps over 40 ms from about 6 to 30-95 a minute. A loud access point
alone is not evidence of interference. The controller's own AFH map is: it
shows which channels the radio found bad. Blend combines both, and leaves
far more channels than the specification's 20.

## How it decides

Code: `app/src/crates/bt-host/src/afh.rs` (`blend`, `ControllerHistory`),
tests in `app/src/crates/bt-host/tests/afh.rs`; the scans run in
`app/src/crates/core/src/adapter.rs` (`WifiAfh`).

Per Bluetooth channel n (2402 + n MHz):

| Signal | Value 0..1 |
|:--|:--|
| Wi-Fi | access points heard within +-11 MHz, power summed; -90 dBm = 0, -50 dBm = 1, linear in dB between |
| Controller | share of the last 24 Read AFH Channel Map readings (one every 5 s, about 2 minutes) that left n out |

- Score = mean of the signals present. One signal alone counts in full; no
  signal scores 0.
- Readings of channels the host classification marked bad are not counted
  (the controller leaves them out because it was told to), nor the 2
  readings after a new classification. Observations older than 120
  readings (10 minutes) expire, so a channel kept out long enough loses its
  controller signal and is judged by Wi-Fi alone.
- The PC's own 2.4 GHz channel is avoided first, exactly as Auto does.
- Then channels at or above the threshold, highest score first, until the
  floor is reached. A channel blend marked bad last time needs 0.1 less
  (hysteresis).

| Strictness | Threshold | Channels kept at least |
|:--|:--|:--|
| Mild | 0.75 | 56 (one Wi-Fi channel's worth) |
| Medium (chosen first) | 0.55 | 48 |
| Strong | 0.35 | 40 |

Consequences worth knowing when reading results:

- Mild with the PC on 2.4 GHz Wi-Fi is Auto (Auto already leaves 56).
- A loud (-55 dBm) access point the controller keeps using scores 0.44:
  only Strong avoids it. At -75 dBm (the 2026-10-17 trap) nothing does.
- Without location access there is no scan, and the controller's map
  alone decides: then Blend only reinforces what the controller already
  does, which may still matter (the controller re-tests left-out channels).
- The classification is recomputed every 30 s, as for Auto.

The scan is `win_sys::wlan::scan_24ghz` (WlanScan, then the BSS list), on
its own thread every 120 s while Blend runs (stops 3 minutes after the last
classification asked for it); a scan older than 10 minutes is ignored. A
scan takes the PC's Wi-Fi off channel for a moment: that may itself cause
gaps, which is why the plan below has a scan-only control.

The debug log has one line per classification:
`AFH blend medium: 52 of 79 usable, left out 24-46, 60-63; scan 7 access points, controller readings 84`
and one per scan: `AFH blend: scan heard ch 6 -48 dBm, ...`.

## Measurement plan

Goal: for each strictness, gaps over 40 ms per minute and dropouts per
minute against Auto, with real adapters, in environments where Auto is
known to be good and where it is not.

### Setup

- **Sender**: the Windows PC with a build of this branch, the adapter
  under test (start with the UB500 / RTL8761BU of the 2026-10-17 test, then
  every other adapter at hand: Intel, Broadcom, CSR). A copy of the
  settings folder via `A2DPWB_CONFIG_DIR`, never the real one.
- **Receiver**: `tools/linux_sink` on a Linux PC, as in
  [Peer Receiver Test](usage#receiver). A2DPWB is the central, so the host
  classification applies as it does for headphones. Fixed positions, 3 m
  apart, same for every run; write down the receiver adapter.
- **Codec**: LDAC 990 kbps (the most demanding) for all runs; one round
  with SBC afterwards to see that nothing differs in kind.
- **Location access** on for the CLI (`Diagnostics > Radio > Scan` lists
  networks). One extra round with it off checks the controller-only path.

### Metrics

From the receiver's CSV (`--csv`, one row per second, counters cumulative
for the stream):

- **Gaps over 40 ms per minute**: `long_gaps`, last row minus the row at
  minute 2, divided by the minutes between them
- **Dropouts per minute**: `underruns` the same way (200 ms playout buffer)

Also note per run: `lost` / `late`, the sender's packets dropped before
sending (CLI end summary), the channels in use (sender log `radio: AFH
mode ... channels in use`), and the blend lines above. As a cross-check
with the 2026-10-17 method, one run per policy with `--hci-capture` (gaps
between Number Of Completed Packets events).

### Runs

Each run 10 minutes; the first 2 minutes are reported separately (Blend
has no controller history yet). Policies are interleaved, so that a change
in the room hits all of them:

```
auto, blend-medium, auto, blend-mild, auto, blend-strong   (x3)
```

plus one `off` run per scenario. Commands:

```bash
sudo sh run.sh --csv run-<scenario>-<policy>-<n>.csv          # Linux
```

```powershell
$env:A2DPWB_LOG = "info,bt_host::afh=debug,a2dpwb_core::adapter=debug"
a2dpwb-core.exe --cli --remote-sink auto -c ldac --test-tone --duration 600 --afh blend-medium
```

Scenarios (each with all policies):

| | PC Wi-Fi | Other 2.4 GHz traffic | What it tests |
|:--|:--|:--|:--|
| A | 2.4 GHz, busy (a download or video on the PC) | none | Blend adds nothing harmful to Auto |
| B | 5 GHz | another device streaming video on a 2.4 GHz access point 1-3 m away | where Auto does nothing and Blend should help |
| C | 5 GHz or off | quiet 2.4 GHz | Blend leaves a clean band alone |
| D | 5 GHz | a loud but idle 2.4 GHz access point (the 2026-10-17 case) | Blend does not repeat that mistake |

Scan-only control (scenario B): Auto, with **Diagnostics > Radio > Scan**
pressed every 2 minutes by hand. If gaps cluster at the scan times there as
well, the scan interval is the cost, not the classification.

### Reading the results

Per scenario and policy: mean and range of gaps over 40 ms per minute and of
dropouts per minute over the 3 runs, minutes 2-10.

- A strictness is a candidate to keep when it is no worse than Auto (within
  the run-to-run range) in A, C and D and clearly better (say a third fewer
  gaps) in B, with no more dropouts anywhere.
- None is: tune before measuring again. The knobs, all in `afh.rs`: the
  thresholds and floors (`Strictness`), `WIFI_QUIET_DBM` / `WIFI_LOUD_DBM`,
  the history length (`HISTORY_OBSERVATIONS`), and the even weighting of
  the two signals in `channel_scores`; the scan interval in `adapter.rs`.
- Even a good result keeps Blend in developer mode until more than one
  adapter and more than one place agree.
