import { describe, expect, it } from "vitest";
// New keys (ui.receiver.*) may not be in the language files yet: compared through t()
import { t } from "../i18n";
import type { LinkSample, ReceivedStream, ReceiverState, StreamInfo, StreamStatus } from "../ipc/types";
import {
  AUDIO_TIMEOUT_MS,
  canStart,
  compare,
  CONFIGURE_TIMEOUT_MS,
  newModel,
  noBaseline,
  radioText,
  startFailed,
  startMeasurement,
  statusText,
  stopMeasurement,
  takeBaseline,
  update,
  type TestModel,
} from "./receiver";

const INFO = {
  ip: "192.168.1.42",
  port: 51201,
  host: "sink-pi",
  bt_address: "DC:A6:32:01:02:03",
  bt_name: "bluecaster-sink",
  codecs: ["SBC", "LDAC"],
  mtu: 1005,
  buffer_ms: 200,
};

const SBC: StreamInfo = {
  codec: "SBC",
  bitrate_kbps: 328,
  sample_rate: 48000,
  channels: 2,
  bits: 16,
  source_sample_rate: 48000,
  source_channels: 2,
  source_bits: 32,
  requested_codec: null,
  requested_in_use: false,
  abr: false,
  bitrate_capped: false,
  source: null,
  le: null,
  details: null,
};

function recv(id: number, o: Partial<ReceivedStream> = {}): ReceivedStream {
  return {
    id,
    codec: "SBC",
    config: "SBC 48000 Hz, joint stereo",
    decoding: true,
    packets: 0,
    bytes: 0,
    frames: 0,
    lost: 0,
    late: 0,
    ts_errors: 0,
    frame_errors: 0,
    decode_errors: 0,
    underruns: 0,
    overflows: 0,
    pauses: 0,
    idle: false,
    audio_ms: 0,
    underrun_ms: 0,
    overflow_ms: 0,
    max_gap_ms: 0,
    jitter_ms: 1.5,
    buffer_ms: 190,
    interval_packets: 0,
    interval_bytes: 0,
    interval_max_gap_ms: 12,
    ...o,
  };
}

function state(o: Partial<ReceiverState> & { stream?: ReceivedStream | null; streaming?: boolean; mtu?: number; buffer?: number } = {}): ReceiverState {
  const { stream = null, streaming = false, mtu = 1005, buffer = 200, ...rest } = o;
  return {
    connection: "connected",
    detail: "",
    target: "auto",
    receiver: { ...INFO },
    stats: {
      connected: streaming,
      streaming,
      device: "",
      buffer_target_ms: buffer,
      mtu,
      codecs: INFO.codecs,
      rssi: streaming ? -3 : null,
      tx_power: null,
      rssi_note: streaming ? "" : "no connection",
      afh_channels: streaming ? 60 : null,
      stream,
    },
    stats_age_ms: 300,
    sent: null,
    stream_starts: 0,
    ...rest,
  };
}

const stream = (s: StreamStatus["state"], info: StreamInfo | null = null, key = "status.idle"): StreamStatus => ({
  state: s,
  message: { key, args: [] },
  profile: null,
  info,
  volume: null,
  battery: null,
  battery_refused: false,
});

const IDLE = stream("idle");
const STREAMING = stream("streaming", SBC, "status.streaming");

function run(m: TestModel, r: ReceiverState | null, s: StreamStatus, now: number) {
  return update(m, { receiver: r, stream: s, serviceText: s.message.key, now });
}

describe("receiver steps", () => {
  it("finding the receiver and its statistics", () => {
    const m = newModel();
    run(m, { ...state(), connection: "connecting", receiver: null, stats: null, stats_age_ms: null }, IDLE, 0);
    expect(m.steps.discover.state).toBe("running");
    expect(m.steps.stats.state).toBe("running");

    // An address given: nothing to find
    const m2 = newModel();
    run(m2, { ...state(), target: "sink-pi", connection: "connecting", receiver: null, stats: null }, IDLE, 0);
    expect(m2.steps.discover.state).toBe("pending");

    const log = run(m, state(), IDLE, 100);
    expect(m.steps.discover).toMatchObject({ state: "ok", detail: "sink-pi (192.168.1.42), Bluetooth DC:A6:32:01:02:03" });
    expect(m.steps.stats.state).toBe("ok");
    expect(log).toEqual(["OK Find receiver: sink-pi (192.168.1.42), Bluetooth DC:A6:32:01:02:03", "OK Statistics"]);
    // No change, no new lines
    expect(run(m, state(), IDLE, 200)).toEqual([]);
  });

  it("not found on the network, or not reachable", () => {
    const m = newModel();
    const log = run(m, { ...state(), connection: "failed", detail: "no receiver found", receiver: null, stats: null }, IDLE, 0);
    expect(m.steps.discover).toMatchObject({ state: "failed", detail: "no receiver found" });
    expect(log).toEqual(["NG Find receiver: no receiver found"]);

    const m2 = newModel();
    run(m2, { ...state(), target: "10.0.0.9", connection: "failed", detail: "connection refused", receiver: null, stats: null }, IDLE, 0);
    expect(m2.steps.discover.state).toBe("pending");
    expect(m2.steps.stats).toMatchObject({ state: "failed", detail: "connection refused" });
  });

  it("stale statistics are not counted", () => {
    const m = newModel();
    run(m, { ...state(), stats_age_ms: 5000 }, IDLE, 0);
    expect(m.steps.discover.state).toBe("ok");
    expect(m.steps.stats.state).toBe("pending");
  });

  it("a measurement from Start to audio", () => {
    const m = newModel();
    const before = state({ stream: recv(4, { packets: 900 }) });
    run(m, before, IDLE, 0);
    const startLog = startMeasurement(m, { receiver: before, mtu: 679, bufferMs: 300, now: 1000 });
    expect(startLog).toEqual([]);
    expect(m.steps.configure.state).toBe("running");
    expect(m.steps.link.state).toBe("running");
    expect(m.measuring).toBe(true);

    // The receiver has not taken the settings yet
    let log = run(m, before, stream("connecting", null, "status.connecting_device"), 1200);
    expect(m.steps.configure.state).toBe("running");
    // The core's status messages go to the log once each
    expect(log).toEqual(["BlueCaster: status.connecting_device"]);
    const configured = state({ stream: recv(4, { packets: 900 }), mtu: 679, buffer: 300 });
    log = run(m, configured, stream("connecting", null, "status.connecting_device"), 1500);
    expect(m.steps.configure).toMatchObject({ state: "ok", detail: "MTU 679, 300 ms", at: 500 });
    expect(log).toEqual(["OK Receiver settings: MTU 679, 300 ms"]);

    log = run(m, configured, STREAMING, 2000);
    expect(m.steps.link.state).toBe("ok");
    expect(m.steps.codec.state).toBe("running");
    expect(m.steps.audio.state).toBe("running");
    expect(log).toContain("OK Bluetooth connection");

    // Still the old stream at the receiver: not ours
    run(m, configured, STREAMING, 2500);
    expect(m.steps.codec.state).toBe("running");

    const arriving = state({ streaming: true, mtu: 679, buffer: 300, stream: recv(5, { packets: 120, interval_packets: 120, interval_bytes: 41000 }) });
    log = run(m, arriving, STREAMING, 3000);
    expect(m.steps.codec).toMatchObject({ state: "ok", detail: "SBC 48000 Hz, joint stereo" });
    expect(m.steps.audio).toMatchObject({ state: "ok", detail: "328 kbps, 120 packets/s" });
    expect(log).toEqual(["OK Codec: SBC 48000 Hz, joint stereo", "OK Audio arrives: 328 kbps, 120 packets/s"]);
    expect(statusText(m, arriving, STREAMING)).toEqual({
      lines: ["sink-pi (192.168.1.42), Bluetooth DC:A6:32:01:02:03, L2CAP MTU 1005", "Measuring."],
      bad: false,
    });

    // Stop
    stopMeasurement(m);
    run(m, arriving, IDLE, 4000);
    expect(m.measuring).toBe(false);
    expect(statusText(m, state(), IDLE).lines[1]).toBe("Ready. Start sends to the receiver's Bluetooth adapter.");
  });

  it("the receiver does not confirm the settings", () => {
    const m = newModel();
    const r = state();
    run(m, r, IDLE, 0);
    startMeasurement(m, { receiver: r, mtu: 679, bufferMs: 0, now: 0 });
    run(m, r, IDLE, CONFIGURE_TIMEOUT_MS - 1);
    expect(m.steps.configure.state).toBe("running");
    const log = run(m, r, IDLE, CONFIGURE_TIMEOUT_MS + 1);
    expect(m.steps.configure.state).toBe("failed");
    expect(log[0]).toMatch(/^NG Receiver settings: the receiver did not confirm/);
  });

  it("default settings are confirmed by any report", () => {
    const m = newModel();
    const r = state({ mtu: 895, buffer: 150 });
    startMeasurement(m, { receiver: r, mtu: 0, bufferMs: 0, now: 0 });
    run(m, r, IDLE, 10);
    expect(m.steps.configure).toMatchObject({ state: "ok", detail: "MTU 895, 150 ms" });
  });

  it("nothing arrives at the receiver", () => {
    const m = newModel();
    const r = state();
    startMeasurement(m, { receiver: r, mtu: 0, bufferMs: 0, now: 0 });
    run(m, r, stream("connecting"), 100);
    run(m, r, STREAMING, 1000);
    run(m, r, STREAMING, 1000 + AUDIO_TIMEOUT_MS);
    expect(m.steps.audio.state).toBe("running");
    const log = run(m, r, STREAMING, 1001 + AUDIO_TIMEOUT_MS);
    expect(m.steps.audio).toMatchObject({ state: "failed", detail: "nothing arrives at the receiver" });
    expect(m.steps.codec.state).toBe("failed");
    expect(log).toHaveLength(2);
  });

  it("the stream fails, or the connection is lost", () => {
    const m = newModel();
    const r = state();
    startMeasurement(m, { receiver: r, mtu: 0, bufferMs: 0, now: 0 });
    run(m, r, stream("connecting"), 100);
    const err = stream("error", null, "error.connect_failed");
    const log = run(m, r, err, 200);
    expect(m.steps.link).toMatchObject({ state: "failed", detail: "error.connect_failed" });
    expect(m.measuring).toBe(false);
    expect(m.failure).toBe("error.connect_failed");
    expect(log).toContain("NG Bluetooth connection: error.connect_failed");
    expect(statusText(m, r, err)).toMatchObject({ bad: true });
    expect(statusText(m, r, err).lines[1]).toMatch(/^Sending failed: error.connect_failed/);

    // Lost while streaming: running again until it is back
    const m2 = newModel();
    startMeasurement(m2, { receiver: r, mtu: 0, bufferMs: 0, now: 0 });
    run(m2, r, STREAMING, 100);
    const lost = run(m2, r, stream("reconnecting", null, "status.connection_lost"), 200);
    expect(m2.steps.link.state).toBe("running");
    expect(lost).toContain("Bluetooth connection lost, BlueCaster reconnects");
    run(m2, r, STREAMING, 300);
    expect(m2.steps.link.state).toBe("ok");
  });

  it("the stream request is refused", () => {
    const m = newModel();
    startMeasurement(m, { receiver: state(), mtu: 0, bufferMs: 0, now: 0 });
    const log = startFailed(m, "No adapter", 50);
    expect(m.steps.link).toMatchObject({ state: "failed", detail: "No adapter", at: 50 });
    expect(log).toEqual(["NG Bluetooth connection: No adapter"]);
    expect(m.measuring).toBe(false);
  });

  it("Start needs a receiver with a Bluetooth address", () => {
    expect(canStart(null)).toBe(false);
    expect(canStart({ ...state(), connection: "off" })).toBe(false);
    expect(canStart({ ...state(), receiver: null })).toBe(false);
    expect(canStart(state())).toBe(true);
  });
});

describe("receiver comparison", () => {
  const link: LinkSample = {
    time_ms: 0,
    rssi: -2,
    afh_in_use: 58,
    afh_usable: 79,
    afh_avoided: "6",
    bytes_per_second: 41000,
    packets_per_second: 120,
    queue_depth: 3,
    dropped: 0,
    capture_dropped: 0,
    airtime: 0.3,
    queue_capacity: 64,
  };

  it("sets sent beside received, with the differences", () => {
    const r = state({
      streaming: true,
      stream: recv(5, { packets: 990, lost: 10, late: 1, interval_packets: 120, interval_bytes: 40000, underruns: 2, underrun_ms: 40 }),
      sent: { packets: 1000, bytes: 345000, dropped: 2, capture_frames: 480000, capture_dropped: 480 },
      stream_starts: 3,
    });
    const rows = compare({ receiver: r, stream: STREAMING, link, base: noBaseline(), lastInfo: null });
    expect(rows.codec).toMatchObject({ sent: { text: "SBC, 48 kHz" }, received: { text: "SBC 48000 Hz, joint stereo" }, diff: { text: t("ui.receiver.codec_same") } });
    expect(rows.bitrate.sent.text).toBe("328 kbps, 120 packets/s");
    expect(rows.bitrate.received.text).toBe("320 kbps, 120 packets/s");
    expect(rows.bitrate.diff.text).toBe(t("ui.receiver.diff_kbps", -8));
    expect(rows.total).toMatchObject({ sent: { text: "1000" }, received: { text: "990" } });
    // In flight while streaming: not marked
    expect(rows.total.diff).toEqual({ text: t("ui.receiver.diff_missing", 10, 1), bad: false });
    expect(rows.loss.sent).toEqual({ text: "2 dropped before sending (0.20%)", bad: true });
    expect(rows.loss.received).toEqual({ text: "10 (1.00%), late 1", bad: true });
    expect(rows.timing.received.text).toBe("jitter 1.5 ms, gap 12 ms");
    expect(rows.buffer.sent.text).toBe("send queue 3 / 64");
    expect(rows.buffer.received.text).toBe("190 / 200 ms (model)");
    expect(rows.audio.sent).toEqual({ text: "capture gaps 10 ms", bad: true });
    expect(rows.audio.received).toEqual({ text: "dropouts 2 (40 ms), skips 0", bad: true });
    expect(rows.errors.received).toEqual({ text: "0", bad: false });
    expect(rows.signal.sent.text).toBe("RSSI -2 dB vs. golden range, AFH 58/79 ch (avoiding Wi-Fi 6)");
    expect(rows.signal.received.text).toBe("RSSI -3 dB vs. golden range, AFH 60/79 ch");
  });

  it("keeps the final counts after the stream stopped", () => {
    const r = state({
      stream: recv(5, { packets: 995, idle: true, pauses: 1, decoding: false, frame_errors: 2 }),
      sent: { packets: 1000, bytes: 0, dropped: 0, capture_frames: 0, capture_dropped: 0 },
      stream_starts: 3,
    });
    const rows = compare({ receiver: r, stream: IDLE, link, base: noBaseline(), lastInfo: SBC });
    expect(rows.codec.sent.text).toBe("SBC, 48 kHz (at stop)");
    expect(rows.codec.received.text).toBe("SBC 48000 Hz, joint stereo (at stop)");
    expect(rows.bitrate.sent.text).toBe("-");
    expect(rows.total.diff).toEqual({ text: t("ui.receiver.diff_missing", 5, 0.5), bad: true });
    expect(rows.audio.received.text).toBe("source idle - dropouts 0 (0 ms), skips 0, source paused 1 time(s)");
    expect(rows.errors.received).toEqual({ text: "2 (headers only)", bad: true });
    expect(rows.signal.received.text).toBe("no connection");
  });

  it("Reset counts from then on, until another stream starts", () => {
    const r = state({
      streaming: true,
      stream: recv(5, { packets: 600, lost: 3 }),
      sent: { packets: 610, bytes: 0, dropped: 1, capture_frames: 0, capture_dropped: 0 },
      stream_starts: 3,
    });
    const base = takeBaseline(r);
    const later = state({
      streaming: true,
      stream: recv(5, { packets: 700, lost: 4 }),
      sent: { packets: 712, bytes: 0, dropped: 1, capture_frames: 0, capture_dropped: 0 },
      stream_starts: 3,
    });
    let rows = compare({ receiver: later, stream: STREAMING, link, base, lastInfo: null });
    expect(rows.total.sent.text).toBe("102");
    expect(rows.total.received.text).toBe("100");
    expect(rows.loss.sent).toMatchObject({ bad: false });
    expect(rows.loss.received.text).toBe("1 (0.99%), late 0");

    // A new stream on both sides: from zero again
    const next = state({
      streaming: true,
      stream: recv(6, { packets: 50 }),
      sent: { packets: 52, bytes: 0, dropped: 0, capture_frames: 0, capture_dropped: 0 },
      stream_starts: 4,
    });
    rows = compare({ receiver: next, stream: STREAMING, link, base, lastInfo: null });
    expect(rows.total.sent.text).toBe("52");
    expect(rows.total.received.text).toBe("50");
  });

  it("a different codec at the receiver is marked", () => {
    const r = state({ streaming: true, stream: recv(1, { codec: "AAC", config: "AAC 44.1 kHz" }) });
    const rows = compare({ receiver: r, stream: STREAMING, link, base: noBaseline(), lastInfo: null });
    expect(rows.codec.diff).toEqual({ text: t("ui.receiver.codec_differs"), bad: true });
  });

  it("nothing known: dashes", () => {
    const rows = compare({ receiver: null, stream: IDLE, link: null, base: noBaseline(), lastInfo: null });
    for (const row of Object.values(rows)) {
      expect([row.sent.text, row.received.text, row.diff.text]).toEqual(["-", "-", "-"]);
    }
  });

  it("radio text", () => {
    expect(radioText(null, null, "")).toBe("-");
    expect(radioText(0, null, "")).toBe("RSSI +0 dB vs. golden range");
    expect(radioText(null, 45, "6, 11")).toBe("AFH 45/79 ch (avoiding Wi-Fi 6, 11)");
  });
});
