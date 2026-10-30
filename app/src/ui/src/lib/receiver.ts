// The peer receiver test's logic, as v1's dialog (wx_receiver_dialog.cpp
// at b819893): the six OK / NG steps of a measurement and the comparison of
// what this PC sent with what the receiver (tools/linux_sink) counted. The
// screen (screens/ReceiverTest.tsx) feeds it the core's ReceiverState, the
// stream status and the last link sample; everything here is plain data so
// the tests can drive it without a core.

import { t } from "../i18n";
import type { LinkSample, LinkTotals, ReceivedStream, ReceiverState, ReceiverStats, StreamInfo, StreamStatus } from "../ipc/types";
import { kbps, khz } from "./format";

/** The receiver reports every second; older data means it stopped */
export const STATS_STALE_MS = 3000;
/** A step not confirmed within this long after it started is NG */
export const CONFIGURE_TIMEOUT_MS = 5000;
export const AUDIO_TIMEOUT_MS = 8000;
/** Log lines kept (v1 kept 64 KB) */
export const LOG_KEEP = 1000;

/** Receiver L2CAP MTU: 0 = as the receiver was started; 672 = BlueZ default,
 *  679 = smallest LDAC accepts, 895 / 1005 = common in headphones */
export const MTUS = [0, 672, 679, 895, 1005, 2048];
export const BUFFERS_MS = [0, 100, 200, 300, 500];

// ---------------------------------------------------------------------------
// Steps
// ---------------------------------------------------------------------------

export type StepId = "discover" | "stats" | "configure" | "link" | "codec" | "audio";
export const STEP_IDS: StepId[] = ["discover", "stats", "configure", "link", "codec", "audio"];
export const STEP_KEYS: Record<StepId, string> = {
  discover: "receiver.step_discover",
  stats: "receiver.step_stats",
  configure: "receiver.step_configure",
  link: "receiver.step_link",
  codec: "receiver.step_codec",
  audio: "receiver.step_audio",
};

export type StepState = "pending" | "running" | "ok" | "failed";

export interface Step {
  state: StepState;
  /** What was found, or why it failed */
  detail: string;
  /** ms after Start when it became OK / NG (measurement steps only) */
  at: number | null;
}

export interface TestModel {
  steps: Record<StepId, Step>;
  /** When Start was pressed (ms); null before the first measurement */
  startedAt: number | null;
  /** Receiver settings asked for at Start (0: as it was started) */
  mtu: number;
  bufferMs: number;
  /** The receiver's stream when Start was pressed: a new one is ours */
  streamIdAtStart: number;
  hadStatsAtStart: boolean;
  /** When the Bluetooth connection came up (for the audio timeout) */
  linkOkAt: number | null;
  /** This screen started the stream and it has not ended */
  measuring: boolean;
  /** The stream left Idle since Start (Idle before that is not the end) */
  leftIdle: boolean;
  /** Why the last stream this screen started ended */
  failure: string;
  /** The core's status text last logged */
  serviceText: string;
  /** The last stream's info: its counts stay shown after it stopped */
  lastInfo: StreamInfo | null;
}

const pending = (): Step => ({ state: "pending", detail: "", at: null });

export function newModel(): TestModel {
  return {
    steps: {
      discover: pending(),
      stats: pending(),
      configure: pending(),
      link: pending(),
      codec: pending(),
      audio: pending(),
    },
    startedAt: null,
    mtu: 0,
    bufferMs: 0,
    streamIdAtStart: 0,
    hadStatsAtStart: false,
    linkOkAt: null,
    measuring: false,
    leftIdle: false,
    failure: "",
    serviceText: "",
    lastInfo: null,
  };
}

/** The receiver's latest report, if it is recent */
export function freshStats(r: ReceiverState | null | undefined): ReceiverStats | null {
  if (!r || !r.stats || r.stats_age_ms === null || r.stats_age_ms >= STATS_STALE_MS) return null;
  return r.stats;
}

/** Sets a step; a change to OK / NG goes to `log` ("OK Codec: SBC ...") */
export function setStep(m: TestModel, id: StepId, state: StepState, detail: string, now: number, log: string[]) {
  const s = m.steps[id];
  const changed = s.state !== state;
  s.state = state;
  s.detail = detail;
  if (!changed) return;
  const measurement = id !== "discover" && id !== "stats";
  s.at = (state === "ok" || state === "failed") && measurement && m.startedAt !== null ? now - m.startedAt : null;
  if (state === "ok" || state === "failed") {
    log.push(`${state === "ok" ? "OK" : "NG"} ${t(STEP_KEYS[id])}${detail ? `: ${detail}` : ""}`);
  }
}

/** Start: the measurement steps from the beginning; configure and link run */
export function startMeasurement(
  m: TestModel,
  o: { receiver: ReceiverState | null; mtu: number; bufferMs: number; now: number },
): string[] {
  const log: string[] = [];
  for (const id of ["configure", "link", "codec", "audio"] as StepId[]) m.steps[id] = pending();
  const s = freshStats(o.receiver);
  m.startedAt = o.now;
  m.mtu = o.mtu;
  m.bufferMs = o.bufferMs;
  m.hadStatsAtStart = s !== null;
  m.streamIdAtStart = s?.stream?.id ?? 0;
  m.linkOkAt = null;
  m.measuring = true;
  m.leftIdle = false;
  m.failure = "";
  m.serviceText = "";
  m.lastInfo = null;
  setStep(m, "configure", "running", "", o.now, log);
  setStep(m, "link", "running", "", o.now, log);
  return log;
}

/** Stop pressed (the stream is stopped by the caller) */
export function stopMeasurement(m: TestModel) {
  m.measuring = false;
}

/** The stream could not be started at all (the request failed) */
export function startFailed(m: TestModel, text: string, now: number): string[] {
  const log: string[] = [];
  m.measuring = false;
  m.failure = text;
  if (m.steps.link.state === "running") setStep(m, "link", "failed", text, now, log);
  return log;
}

/** The receiver did not take the settings (receiver.configure failed) */
export function configureFailed(m: TestModel, text: string, now: number): string[] {
  const log: string[] = [];
  if (m.steps.configure.state === "running") setStep(m, "configure", "failed", text, now, log);
  return log;
}

export interface Inputs {
  receiver: ReceiverState | null;
  stream: StreamStatus;
  /** The stream's status text, translated */
  serviceText: string;
  now: number;
}

/** One refresh (v1's refresh + refresh_steps): returns the new log lines */
export function update(m: TestModel, i: Inputs): string[] {
  const log: string[] = [];
  const state = i.stream.state;
  if (state === "streaming" && i.stream.info) m.lastInfo = { ...i.stream.info };

  // Whether the stream this screen started has ended
  if (m.measuring && state !== "idle") m.leftIdle = true;
  if (m.measuring && (state === "error" || (state === "idle" && m.leftIdle))) {
    m.measuring = false;
    if (state === "error") m.failure = i.serviceText;
  }

  // 1-2: finding the receiver and its statistics
  const r = i.receiver;
  const conn = r?.connection ?? "off";
  const info = r?.receiver ?? null;
  const stats = freshStats(r);
  const automatic = r?.target === "auto";
  switch (conn) {
    case "off":
      setStep(m, "discover", "pending", "", i.now, log);
      setStep(m, "stats", "pending", "", i.now, log);
      break;
    case "connecting":
      if (!info) setStep(m, "discover", automatic ? "running" : "pending", "", i.now, log);
      setStep(m, "stats", "running", "", i.now, log);
      break;
    case "failed":
      if (!info && automatic) setStep(m, "discover", "failed", r?.detail ?? "", i.now, log);
      else setStep(m, "stats", "failed", r?.detail ?? "", i.now, log);
      break;
    case "connected":
      if (info) setStep(m, "discover", "ok", `${info.host} (${info.ip}), Bluetooth ${info.bt_address}`, i.now, log);
      if (stats) setStep(m, "stats", "ok", "", i.now, log);
      break;
  }

  if (m.startedAt === null) return log;
  const elapsed = i.now - m.startedAt;

  // 3: the receiver took the settings (it reports them in its statistics)
  if (m.steps.configure.state === "running") {
    const mtuOk = m.mtu === 0 || (stats !== null && stats.mtu === m.mtu);
    const bufferOk = m.bufferMs === 0 || (stats !== null && stats.buffer_target_ms === m.bufferMs);
    if (stats && mtuOk && bufferOk) {
      setStep(m, "configure", "ok", `MTU ${stats.mtu}, ${stats.buffer_target_ms} ms`, i.now, log);
    } else if (elapsed > CONFIGURE_TIMEOUT_MS) {
      setStep(m, "configure", "failed", t("receiver.log_configure_failed"), i.now, log);
    }
  }

  // 4: this PC's own connection, with its status messages in the log
  if (m.measuring && m.leftIdle && i.serviceText !== m.serviceText) {
    m.serviceText = i.serviceText;
    if (i.serviceText) log.push(`BlueCaster: ${i.serviceText}`);
  }
  const link = m.steps.link.state;
  if (link === "running") {
    if (state === "streaming") {
      setStep(m, "link", "ok", "", i.now, log);
      m.linkOkAt = i.now;
      setStep(m, "codec", "running", "", i.now, log);
      setStep(m, "audio", "running", "", i.now, log);
    } else if (state === "error" || (!m.measuring && m.leftIdle && state === "idle")) {
      setStep(m, "link", "failed", i.serviceText, i.now, log);
    }
  } else if (link === "ok" && state === "reconnecting") {
    setStep(m, "link", "running", "", i.now, log);
    log.push(t("receiver.log_link_lost"));
  }

  // 5-6: what the receiver sees of it
  const s = stats?.stream ?? null;
  const newStream = s !== null && (s.id !== m.streamIdAtStart || !m.hadStatsAtStart);
  const timedOut = m.linkOkAt !== null && i.now - m.linkOkAt > AUDIO_TIMEOUT_MS;
  if (m.steps.codec.state === "running" && newStream) setStep(m, "codec", "ok", s!.config, i.now, log);
  if (m.steps.audio.state === "running") {
    if (newStream && stats!.streaming && s!.interval_packets > 0) {
      setStep(m, "audio", "ok", t("receiver.bitrate_value", (s!.interval_bytes * 8) / 1000, s!.interval_packets), i.now, log);
    } else if (timedOut) {
      setStep(m, "audio", "failed", t("receiver.log_no_audio"), i.now, log);
    }
  }
  if (m.steps.codec.state === "running" && timedOut) setStep(m, "codec", "failed", t("receiver.log_no_audio"), i.now, log);
  return log;
}

// ---------------------------------------------------------------------------
// Status line
// ---------------------------------------------------------------------------

/** The text under the address (v1's refresh_status): the receiver, then what happens */
export function statusText(m: TestModel, r: ReceiverState | null, stream: StreamStatus): { lines: string[]; bad: boolean } {
  const conn = r?.connection ?? "off";
  const info = conn !== "off" ? (r?.receiver ?? null) : null;
  const lines: string[] = [];
  if (info) lines.push(t("receiver.found", info.host, info.ip, info.bt_address, info.mtu));
  let bad = false;
  switch (conn) {
    case "off":
      lines.push(t("receiver.off"));
      break;
    case "connecting":
      lines.push(t(r?.target === "auto" ? "receiver.searching" : "receiver.connecting"));
      break;
    case "failed":
      lines.push(t("receiver.failed", r?.detail ?? ""));
      bad = true;
      break;
    case "connected": {
      const stats = freshStats(r);
      if (stats?.streaming) lines.push(t("receiver.state_receiving"));
      else if (m.measuring && (stream.state === "connecting" || stream.state === "reconnecting")) lines.push(t("receiver.state_connecting"));
      else if (m.measuring) lines.push(t("receiver.state_waiting"));
      else if (m.failure) {
        lines.push(t("receiver.state_failed", m.failure));
        bad = true;
      } else lines.push(t("receiver.state_ready"));
      break;
    }
  }
  return { lines, bad };
}

/** Start is possible: a receiver with a Bluetooth address is known */
export function canStart(r: ReceiverState | null): boolean {
  return !!r && r.connection !== "off" && !!r.receiver?.bt_address;
}

// ---------------------------------------------------------------------------
// Comparison
// ---------------------------------------------------------------------------

export type RowId = "codec" | "bitrate" | "total" | "loss" | "timing" | "buffer" | "audio" | "errors" | "signal";
export const ROW_IDS: RowId[] = ["codec", "bitrate", "total", "loss", "timing", "buffer", "audio", "errors", "signal"];
export const ROW_KEYS: Record<RowId, string> = {
  codec: "receiver.row_codec",
  bitrate: "receiver.row_bitrate",
  total: "receiver.row_total",
  loss: "receiver.row_loss",
  timing: "receiver.row_timing",
  buffer: "receiver.row_buffer",
  audio: "receiver.row_audio",
  errors: "receiver.row_errors",
  signal: "receiver.row_signal",
};

export interface Cell {
  text: string;
  bad?: boolean;
}
export type Row = { sent: Cell; received: Cell; diff: Cell };

const DASH: Cell = { text: "-" };

/** Counts start with each stream and at Reset: what Reset took */
export interface Baseline {
  /** ReceiverState.stream_starts and `sent` at Reset; another stream starts from zero */
  sentStarts: number;
  sent: LinkTotals | null;
  /** The receiver's stream and its counters at Reset */
  received: ReceivedStream | null;
}

export const noBaseline = (): Baseline => ({ sentStarts: -1, sent: null, received: null });

/** Reset: the counts from now on */
export function takeBaseline(r: ReceiverState | null): Baseline {
  const s = freshStats(r)?.stream ?? null;
  return { sentStarts: r?.stream_starts ?? -1, sent: r?.sent ? { ...r.sent } : null, received: s ? { ...s } : null };
}

export function pct(part: number, whole: number): number {
  return whole > 0 ? (100 * part) / whole : 0;
}

/** "RSSI +0 dB vs. golden range, AFH 45/79 ch (avoiding Wi-Fi 6, 11)" */
export function radioText(rssi: number | null | undefined, afh: number | null | undefined, avoided: string): string {
  let text = rssi != null ? t("radio.rssi", rssi) : "";
  if (afh != null) {
    if (text) text += t("radio.separator");
    text += t("radio.afh", afh);
  }
  if (avoided && text) text += t("radio.afh_avoid", avoided);
  return text || "-";
}

const sub = (a: number, b: number | undefined) => Math.max(0, a - (b ?? 0));

/** What this PC sent since the stream started or Reset */
export function sentSince(r: ReceiverState | null, base: Baseline): LinkTotals | null {
  const now = r?.sent;
  if (!now) return null;
  const b = base.sent && base.sentStarts === r!.stream_starts ? base.sent : null;
  return {
    packets: sub(now.packets, b?.packets),
    bytes: sub(now.bytes, b?.bytes),
    dropped: sub(now.dropped, b?.dropped),
    capture_frames: sub(now.capture_frames, b?.capture_frames),
    capture_dropped: sub(now.capture_dropped, b?.capture_dropped),
  };
}

const normCodec = (name: string) => name.toLowerCase().replace(/[\s_-]/g, "");

export interface CompareInputs {
  receiver: ReceiverState | null;
  stream: StreamStatus;
  /** The last link sample (rate, queue, radio of this PC) */
  link: LinkSample | null | undefined;
  base: Baseline;
  /** The last stream's info (kept after it stopped) */
  lastInfo: StreamInfo | null;
}

/** The comparison table: sent | received | difference, per row */
export function compare(i: CompareInputs): Record<RowId, Row> {
  const rows = Object.fromEntries(ROW_IDS.map((id) => [id, { sent: DASH, received: DASH, diff: DASH }])) as Record<RowId, Row>;
  const streaming = i.stream.state === "streaming" && i.stream.info !== null;
  const sent = sentSince(i.receiver, i.base);

  // ---- Sent (this PC) ----
  let sentKbps: number | null = null;
  const sentInfo = streaming ? i.stream.info : i.lastInfo;
  if (sentInfo) {
    const codec = `${sentInfo.codec}, ${khz(sentInfo.sample_rate)} kHz`;
    rows.codec.sent = { text: streaming ? codec : t("receiver.final_value", codec) };
    if (sent) {
      rows.total.sent = { text: String(sent.packets) };
      rows.loss.sent = {
        text: t("receiver.dropped_value", sent.dropped, pct(sent.dropped, sent.packets + sent.dropped)),
        bad: sent.dropped > 0,
      };
    }
  }
  if (streaming) {
    const l = i.link;
    if (l && l.packets_per_second > 0) {
      sentKbps = kbps(l.bytes_per_second);
      rows.bitrate.sent = { text: t("receiver.bitrate_value", sentKbps, l.packets_per_second) };
    }
    if (l) {
      rows.buffer.sent = { text: t("receiver.queue_value", l.queue_depth, l.queue_capacity ?? 0) };
      rows.signal.sent = { text: radioText(l.rssi, l.afh_in_use, l.afh_avoided) };
    }
    if (sent) {
      const rate = i.stream.info!.source_sample_rate || l?.source_rate || 0;
      const gaps = rate > 0 ? (1000 * sent.capture_dropped) / rate : 0;
      rows.audio.sent = { text: t("receiver.capture_value", gaps), bad: sent.capture_dropped > 0 };
    }
  }

  // ---- Received (the receiver) ----
  const stats = freshStats(i.receiver);
  const s = stats?.stream ?? null;
  let recvKbps: number | null = null;
  if (!stats || !s) {
    if (stats?.connected && stats.rssi != null) rows.signal.received = { text: t("receiver.rssi_value", stats.rssi) };
  } else {
    const b = i.base.received && i.base.received.id === s.id ? i.base.received : null;
    const packets = sub(s.packets, b?.packets);
    const lost = sub(s.lost, b?.lost);
    const late = sub(s.late, b?.late);
    const underruns = sub(s.underruns, b?.underruns);
    const overflows = sub(s.overflows, b?.overflows);
    const pauses = sub(s.pauses, b?.pauses);
    const errors = sub(s.frame_errors, b?.frame_errors) + sub(s.decode_errors, b?.decode_errors) + sub(s.ts_errors, b?.ts_errors);
    rows.codec.received = { text: stats.streaming ? s.config : t("receiver.final_value", s.config) };
    if (stats.streaming && s.interval_packets > 0) {
      recvKbps = (s.interval_bytes * 8) / 1000;
      rows.bitrate.received = { text: t("receiver.bitrate_value", recvKbps, s.interval_packets) };
      rows.timing.received = { text: t("receiver.timing_value", s.jitter_ms, s.interval_max_gap_ms) };
    }
    rows.total.received = { text: String(packets) };
    rows.loss.received = { text: t("receiver.lost_value", lost, pct(lost, packets + lost), late), bad: lost > 0 || late > 0 };
    rows.buffer.received =
      s.buffer_ms == null ? DASH : { text: t("receiver.buffer_value", s.buffer_ms, stats.buffer_target_ms) };
    let audio = t("receiver.dropouts_value", underruns, Math.max(0, s.underrun_ms - (b?.underrun_ms ?? 0)), overflows);
    if (pauses) audio += t("receiver.pauses_value", pauses);
    if (s.idle) audio = t("receiver.idle_value") + audio;
    rows.audio.received = { text: audio, bad: underruns > 0 || overflows > 0 };
    rows.errors.received = { text: t(s.decoding ? "receiver.errors_value" : "receiver.errors_value_headers", errors), bad: errors > 0 };
    rows.signal.received =
      stats.rssi != null || stats.afh_channels != null
        ? { text: radioText(stats.rssi, stats.afh_channels, "") }
        : { text: stats.rssi_note || "-" };

    // ---- Differences ----
    if (sentInfo) {
      const same = normCodec(sentInfo.codec) === normCodec(s.codec);
      rows.codec.diff = same ? { text: t("ui.receiver.codec_same") } : { text: t("ui.receiver.codec_differs"), bad: true };
    }
    if (sentKbps !== null && recvKbps !== null) rows.bitrate.diff = { text: t("ui.receiver.diff_kbps", recvKbps - sentKbps) };
    if (sent) {
      // While streaming, the packets on their way (up to a second's worth)
      // are among the missing ones; at the end it is the loss
      const missing = sent.packets - packets;
      rows.total.diff =
        missing < 0
          ? DASH
          : { text: t("ui.receiver.diff_missing", missing, pct(missing, sent.packets)), bad: !streaming && missing > 0 };
    }
  }
  return rows;
}
