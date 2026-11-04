// Time series for the Diagnostics charts: the link samples on a time axis
// (gaps where samples are missing), the nearest sample to a hovered time,
// and per-second rates from the cumulative counters.

import type { LinkSample, LinkTotals } from "../ipc/types";

/** Two samples further apart than this are not joined by a line (4 a second normally) */
export const GAP_MS = 1000;

/** The smallest round number (1, 1.2, 1.5, 2, 2.5, 3, 4, 5, 6, 8 × 10^n) at or
 *  above v: an automatic scale's top, close enough to keep the line tall */
export function niceCeil(v: number): number {
  if (!Number.isFinite(v) || v <= 0) return 1;
  const p = 10 ** Math.floor(Math.log10(v));
  for (const m of [1, 1.2, 1.5, 2, 2.5, 3, 4, 5, 6, 8, 10]) if (m * p >= v - 1e-9 * p) return m * p;
  return 10 * p;
}

/** The scale of a chart: fixed ends where given, else the data's (the top rounded up) */
export function scaleOf(series: readonly (readonly (number | null)[])[], min?: number, max?: number): [number, number] {
  const known: number[] = [];
  for (const s of series) for (const v of s) if (v !== null && Number.isFinite(v)) known.push(v);
  const lo = min ?? (known.length ? Math.min(...known) : 0);
  let hi = max ?? (known.length ? Math.max(...known) : 1);
  if (max === undefined) hi = lo >= 0 ? niceCeil(hi) : hi;
  if (hi <= lo) hi = lo + 1;
  return [lo, hi];
}

/** Polylines (SVG points) of the values over [start, start + span]: one per
 *  run of known values, broken where a value is unknown or samples are
 *  missing */
export function timeRuns(
  times: readonly number[],
  values: readonly (number | null)[],
  start: number,
  span: number,
  lo: number,
  hi: number,
  width: number,
  height: number,
): string[] {
  const runs: string[] = [];
  let cur: string[] = [];
  const range = hi - lo || 1;
  const flush = () => {
    if (cur.length) runs.push(cur.join(" "));
    cur = [];
  };
  values.forEach((v, i) => {
    const time = times[i];
    if (i > 0 && time - times[i - 1] > GAP_MS) flush();
    if (v === null || v === undefined || !Number.isFinite(v) || time < start) {
      flush();
      return;
    }
    const x = ((time - start) / (span || 1)) * width;
    const y = height - ((Math.min(Math.max(v, lo), hi) - lo) / range) * height;
    cur.push(`${x.toFixed(2)},${y.toFixed(2)}`);
  });
  flush();
  return runs;
}

/** The sample closest to `time`, when one is within GAP_MS of it */
export function nearestIndex(times: readonly number[], time: number): number | null {
  let best = -1;
  let dist = Infinity;
  for (let i = 0; i < times.length; i++) {
    const d = Math.abs(times[i] - time);
    if (d < dist) {
      dist = d;
      best = i;
    }
  }
  return best >= 0 && dist <= GAP_MS ? best : null;
}

/** The index of the last known value */
export function lastKnownIndex(values: readonly (number | null)[]): number | null {
  for (let i = values.length - 1; i >= 0; i--) {
    const v = values[i];
    if (v !== null && v !== undefined && Number.isFinite(v)) return i;
  }
  return null;
}

/** The last known value */
export function lastKnown(values: readonly (number | null)[]): number | null {
  const i = lastKnownIndex(values);
  return i === null ? null : values[i];
}

/** Captured audio Windows lost in its own gaps, per sample in ms: null
 *  where a gap came without a known loss (per-app capture never measures
 *  it) or the rate is unknown */
export function captureLostPerSample(samples: readonly LinkSample[], fallbackRate: number): (number | null)[] {
  return samples.map((x, i) => {
    const p = samples[i - 1];
    if (!p) return 0;
    const lost = Math.max(0, (x.capture_lost ?? 0) - (p.capture_lost ?? 0));
    const gaps = Math.max(0, (x.capture_discontinuities ?? 0) - (p.capture_discontinuities ?? 0));
    const rate = x.source_rate || fallbackRate;
    if (lost === 0) return gaps === 0 ? 0 : null;
    return rate > 0 ? (lost * 1000) / rate : null;
  });
}

/** A cumulative counter as a rate per second between samples (null for the
 *  first sample, without counters, or where the counter went back) */
export function ratePerSecond(samples: readonly LinkSample[], field: keyof LinkTotals): (number | null)[] {
  return samples.map((x, i) => {
    const p = samples[i - 1];
    if (!p || !x.counters || !p.counters) return null;
    const dt = x.time_ms - p.time_ms;
    const d = x.counters.total[field] - p.counters.total[field];
    if (dt <= 0 || dt > GAP_MS || d < 0) return null;
    return (d * 1000) / dt;
  });
}

/** Audio dropped at capture per sample, in ms (frames where the rate is unknown) */
export function captureDropsPerSample(samples: readonly LinkSample[], fallbackRate: number): number[] {
  return samples.map((x, i) => {
    const p = samples[i - 1];
    if (!p) return 0;
    const d =
      x.counters && p.counters
        ? x.counters.total.capture_dropped - p.counters.total.capture_dropped
        : x.capture_dropped - p.capture_dropped;
    const rate = x.source_rate || fallbackRate;
    return Math.max(0, rate > 0 ? (d * 1000) / rate : d);
  });
}
