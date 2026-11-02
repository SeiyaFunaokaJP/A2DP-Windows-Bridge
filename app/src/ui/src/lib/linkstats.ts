// The arithmetic of the Diagnostics screen's link tab, as v1's link quality
// dialog did it (wx_link_quality_dialog.cpp on the develop branch): losses since the
// stream started or since the Reset button, drop shares, capture drops in
// milliseconds. The core keeps cumulative counters only; the baselines are
// the view's.

import type { LinkCounters, LinkSample, LinkTotals } from "../ipc/types";

/** BR/EDR slot length */
export const SLOT_SECONDS = 625e-6;
/** 3-DH5 back to back on an EDR 3 Mbps link: 1021 bytes every 6 slots (2178 kbps) */
export const EDR3_MAX_KBPS = (1021 * 8) / (6 * SLOT_SECONDS) / 1000;

export const ZERO_TOTALS: LinkTotals = { packets: 0, bytes: 0, dropped: 0, capture_frames: 0, capture_dropped: 0 };

const FIELDS = ["packets", "bytes", "dropped", "capture_frames", "capture_dropped"] as const;

/** `now - base` per counter (never negative) */
export function diffTotals(now: LinkTotals, base: LinkTotals): LinkTotals {
  const r = { ...ZERO_TOTALS };
  for (const f of FIELDS) r[f] = Math.max(0, now[f] - base[f]);
  return r;
}

/** The counters when Reset was pressed */
export interface LinkBaseline {
  totals: LinkTotals;
  /** LinkCounters.stream_starts then */
  stream_starts: number;
  /** LinkSample.time_ms then */
  time_ms: number;
}

/** The baseline Reset takes from a sample (null: the core sends no counters) */
export function takeBaseline(s: LinkSample | undefined): LinkBaseline | null {
  const c = s?.counters;
  if (!s || !c) return null;
  return { totals: { ...c.total }, stream_starts: c.stream_starts, time_ms: s.time_ms };
}

/**
 * What the losses count from: the reset baseline, or the stream start when
 * there is none, when a stream started after the reset (the later of the
 * two), or when the counters went below it (the core restarted).
 */
export function activeBase(c: LinkCounters, reset: LinkBaseline | null): { base: LinkTotals; sinceReset: boolean } {
  if (reset && reset.stream_starts === c.stream_starts && FIELDS.every((f) => c.total[f] >= reset.totals[f])) {
    return { base: reset.totals, sinceReset: true };
  }
  return { base: c.stream_start, sinceReset: false };
}

/** part / whole in percent; 0 when there is no whole */
export function percent(part: number, whole: number): number {
  return whole > 0 ? (100 * part) / whole : 0;
}

/** A percentage for a gauge: 0..100 */
export function gaugeValue(pct: number | null | undefined): number {
  if (pct == null || !Number.isFinite(pct)) return 0;
  return Math.min(100, Math.max(0, pct));
}

export interface LossStats {
  /** Media packets handed to L2CAP */
  sent: number;
  /** Lost before the radio */
  dropped: number;
  /** dropped / (sent + dropped), percent */
  dropPct: number;
  captureFrames: number;
  captureDropped: number;
  /** Capture drops as audio time; null while the capture rate is unknown */
  captureMs: number | null;
  /** captureDropped / captureFrames, percent */
  capturePct: number;
  /** Counted since the Reset button (else since the stream started) */
  sinceReset: boolean;
}

/**
 * The losses of a sample since the active baseline. `rate` is the capture
 * rate to convert dropped frames with when the sample has none (0 while not
 * streaming, e.g. the last stream's StreamInfo.source_sample_rate).
 * Null when the core sends no counters.
 */
export function lossStats(s: LinkSample, reset: LinkBaseline | null, rate?: number | null): LossStats | null {
  const c = s.counters;
  if (!c) return null;
  const { base, sinceReset } = activeBase(c, reset);
  const d = diffTotals(c.total, base);
  const r = s.source_rate || rate || 0;
  return {
    sent: d.packets,
    dropped: d.dropped,
    dropPct: percent(d.dropped, d.packets + d.dropped),
    captureFrames: d.capture_frames,
    captureDropped: d.capture_dropped,
    captureMs: r > 0 ? (d.capture_dropped * 1000) / r : d.capture_dropped === 0 ? 0 : null,
    capturePct: percent(d.capture_dropped, d.capture_frames),
    sinceReset,
  };
}

/** Packets lost per sample (the first sample: 0), for the drop graph */
export function dropsPerSample(samples: readonly LinkSample[]): number[] {
  return samples.map((x, i) => {
    if (i === 0) return 0;
    const p = samples[i - 1];
    if (x.counters && p.counters) {
      const a = x.counters.total;
      const b = p.counters.total;
      return Math.max(0, a.dropped - b.dropped);
    }
    return Math.max(0, x.dropped - p.dropped);
  });
}

/** 12 345 678 -> "12.3 MB" (bytes as Explorer shows them, 1024-based) */
export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  const units = ["KB", "MB", "GB"];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v >= 100 ? v.toFixed(0) : v.toFixed(1)} ${units[i]}`;
}
