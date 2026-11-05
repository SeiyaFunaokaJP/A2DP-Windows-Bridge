// The log of the Diagnostics screen: level, text and "this attempt only"
// filters, the line text copies use, and the window of lines drawn, as
// v1's debug console (wx_debug_console_dialog.cpp on the develop branch) had them.

import type { FlowView, LogLevel, LogLine } from "../ipc/types";
import { maskIdentifiers } from "./format";

/** Most severe first */
export const LEVEL_ORDER: LogLevel[] = ["error", "warn", "info", "debug", "trace"];

export type LevelFilter = LogLevel | "all";

export interface LogFilter {
  level: LevelFilter;
  /** Text in the message or the target (any case) */
  query: string;
  /** Only the lines of this attempt (null: every line) */
  attempt: Pick<FlowView, "session" | "first_seq"> | null;
}

/** The line is at the filter's level or more severe */
export function levelShown(level: LogLevel, filter: LevelFilter): boolean {
  if (filter === "all") return true;
  return LEVEL_ORDER.indexOf(level) <= LEVEL_ORDER.indexOf(filter);
}

/**
 * The line belongs to the attempt: it came at or after the attempt's first
 * line, or the core tagged it with the attempt. A line that carries neither
 * (an older core) is kept.
 */
export function inAttempt(line: LogLine, attempt: Pick<FlowView, "session" | "first_seq">): boolean {
  const bySeq = line.seq != null && attempt.first_seq != null;
  const bySession = line.session != null;
  if (!bySeq && !bySession) return true;
  return (bySeq && line.seq! >= attempt.first_seq!) || (bySession && line.session === attempt.session);
}

export function filterLog(lines: readonly LogLine[], f: LogFilter): LogLine[] {
  const q = f.query.trim().toLowerCase();
  return lines.filter(
    (l) =>
      levelShown(l.level, f.level) &&
      (!f.attempt || inAttempt(l, f.attempt)) &&
      (!q || l.message.toLowerCase().includes(q) || l.target.toLowerCase().includes(q)),
  );
}

/** A key that stays with a line while the buffer moves (seq when the core numbers its lines) */
export function logKey(l: LogLine): string {
  return l.seq != null ? `#${l.seq}` : `${l.time_ms}\u0000${l.target}\u0000${l.message}`;
}

/** One line as copied: time, level, target, message */
export function formatLogLine(l: LogLine, time: string, mask: boolean): string {
  const msg = mask ? maskIdentifiers(l.message) : l.message;
  return `${time} ${l.level.toUpperCase().padEnd(5)} ${l.target}: ${msg}`;
}

/**
 * Which of `count` filtered lines to draw, at most `limit`: the newest ones,
 * or a window around `focus` (the index of a line jumped to) when that is
 * older. Returns [start, end).
 */
export function drawnRange(count: number, limit: number, focus: number | null): [number, number] {
  const start = Math.max(0, count - limit);
  if (focus === null || focus < 0 || focus >= start) return [start, count];
  const from = Math.max(0, focus - Math.floor(limit / 4));
  return [from, Math.min(count, from + limit)];
}

/** The keys of the lines from `a` to `b` (either order), for a shift-click */
export function keysBetween(lines: readonly LogLine[], a: string, b: string): string[] {
  const i = lines.findIndex((l) => logKey(l) === a);
  const j = lines.findIndex((l) => logKey(l) === b);
  if (i < 0 || j < 0) return j >= 0 ? [b] : [];
  const [lo, hi] = i <= j ? [i, j] : [j, i];
  return lines.slice(lo, hi + 1).map(logKey);
}

// ---- Time of day ------------------------------------------------------------
// The core stamps lines with milliseconds since it started; v1 showed the
// time of day. The core tells its local time of day at time 0
// (Snapshot.clock_ms), which gives the exact time. Without it (an older
// core) the wall clock at the core's time 0 is estimated from live samples:
// now - time_ms is the epoch plus the delivery delay, so the smallest value
// seen is the best estimate.

/** The better of two epoch estimates (null: none yet) */
export function nextEpoch(prev: number | null, timeMs: number, nowMs: number): number {
  const e = nowMs - timeMs;
  return prev === null ? e : Math.min(prev, e);
}

const pad = (n: number, w = 2) => String(n).padStart(w, "0");

/** "hh:mm:ss.mmm" of the local time of day at core time `timeMs` */
export function timeOfDay(timeMs: number, epoch: number): string {
  const d = new Date(epoch + timeMs);
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}.${pad(d.getMilliseconds(), 3)}`;
}

const DAY_MS = 86_400_000;

/** "hh:mm:ss.mmm" of core time `timeMs` from the core's own clock: `clockMs`
 *  is its local time of day at time 0 (Snapshot.clock_ms); past midnight
 *  the time of day starts again from 00:00 */
export function coreTimeOfDay(timeMs: number, clockMs: number): string {
  const t = (((clockMs + timeMs) % DAY_MS) + DAY_MS) % DAY_MS;
  const h = Math.floor(t / 3_600_000);
  const m = Math.floor(t / 60_000) % 60;
  const s = Math.floor(t / 1000) % 60;
  return `${pad(h)}:${pad(m)}:${pad(s)}.${pad(Math.floor(t % 1000), 3)}`;
}
