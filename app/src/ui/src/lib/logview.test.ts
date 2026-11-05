import { describe, expect, it } from "vitest";
import type { LogLine } from "../ipc/types";
import {
  coreTimeOfDay,
  drawnRange,
  filterLog,
  formatLogLine,
  inAttempt,
  keysBetween,
  levelShown,
  logKey,
  nextEpoch,
  timeOfDay,
} from "./logview";

const line = (seq: number | null, session: number | null, message = `line ${seq}`, level: LogLine["level"] = "info"): LogLine => ({
  time_ms: (seq ?? 0) * 10,
  level,
  target: "a2dpwb_core::stream",
  message,
  seq,
  session,
});

describe("log filters", () => {
  it("levels", () => {
    expect(levelShown("debug", "all")).toBe(true);
    expect(levelShown("debug", "info")).toBe(false);
    expect(levelShown("info", "info")).toBe(true);
    expect(levelShown("warn", "info")).toBe(true);
    expect(levelShown("warn", "error")).toBe(false);
    expect(levelShown("error", "warn")).toBe(true);
  });

  it("this attempt only: from its first line, or tagged with it", () => {
    const attempt = { session: 2, first_seq: 10 };
    expect(inAttempt(line(9, 1), attempt)).toBe(false);
    expect(inAttempt(line(10, 2), attempt)).toBe(true);
    expect(inAttempt(line(12, 2), attempt)).toBe(true);
    // Only one of the two known
    expect(inAttempt(line(11, null), attempt)).toBe(true);
    expect(inAttempt(line(5, null), attempt)).toBe(false);
    expect(inAttempt(line(null, 2), attempt)).toBe(true);
    expect(inAttempt(line(null, 1), attempt)).toBe(false);
    expect(inAttempt(line(5, 2), { session: 2, first_seq: null })).toBe(true);
    // Neither: an older core, kept
    expect(inAttempt(line(null, null), attempt)).toBe(true);
    // No attempt yet: the lines before the first
    expect(inAttempt(line(3, 0), { session: 0, first_seq: null })).toBe(true);
    expect(inAttempt(line(3, 1), { session: 0, first_seq: null })).toBe(false);
  });

  it("combines level, text and attempt", () => {
    const lines = [
      line(1, 1, "connecting to AC:80:0A:12:34:56"),
      line(2, 1, "HCI ready", "debug"),
      line(3, 2, "connecting to AC:80:0A:12:34:56"),
      line(4, 2, "codec selected: LDAC", "warn"),
    ];
    const seqs = (f: Parameters<typeof filterLog>[1]) => filterLog(lines, f).map((l) => l.seq);
    expect(seqs({ level: "all", query: "", attempt: null })).toEqual([1, 2, 3, 4]);
    expect(seqs({ level: "info", query: "", attempt: null })).toEqual([1, 3, 4]);
    expect(seqs({ level: "info", query: "CONNECTING", attempt: null })).toEqual([1, 3]);
    expect(seqs({ level: "info", query: "stream", attempt: null })).toEqual([1, 3, 4]); // the target matches
    expect(seqs({ level: "all", query: "", attempt: { session: 2, first_seq: 3 } })).toEqual([3, 4]);
    expect(seqs({ level: "warn", query: " ", attempt: { session: 2, first_seq: 3 } })).toEqual([4]);
  });

  it("keys and copied text", () => {
    expect(logKey(line(7, 1))).toBe("#7");
    const old = line(null, null, "x");
    expect(logKey(old)).toBe(logKey({ ...old }));
    expect(formatLogLine(line(1, 1, "to AC:80:0A:12:34:56"), "12:00:00.000", true)).toMatch(
      /^12:00:00\.000 INFO  a2dpwb_core::stream: to <addr-\d+>$/,
    );
    expect(formatLogLine(line(1, 1, "to AC:80:0A:12:34:56", "error"), "t", false)).toBe(
      "t ERROR a2dpwb_core::stream: to AC:80:0A:12:34:56",
    );
  });

  it("selects a range in either direction", () => {
    const lines = [line(1, 1), line(2, 1), line(3, 1), line(4, 1)];
    expect(keysBetween(lines, "#2", "#4")).toEqual(["#2", "#3", "#4"]);
    expect(keysBetween(lines, "#4", "#2")).toEqual(["#2", "#3", "#4"]);
    expect(keysBetween(lines, "#9", "#2")).toEqual(["#2"]);
    expect(keysBetween(lines, "#2", "#9")).toEqual([]);
  });

  it("draws the newest lines, or a window around a line jumped to", () => {
    expect(drawnRange(100, 1500, null)).toEqual([0, 100]);
    expect(drawnRange(5000, 1500, null)).toEqual([3500, 5000]);
    expect(drawnRange(5000, 1500, 4000)).toEqual([3500, 5000]);
    expect(drawnRange(5000, 1500, 1000)).toEqual([625, 2125]);
    expect(drawnRange(5000, 1500, 100)).toEqual([0, 1500]);
    expect(drawnRange(5000, 1500, -1)).toEqual([3500, 5000]);
  });
});

describe("time of day", () => {
  it("keeps the smallest epoch estimate", () => {
    let e: number | null = null;
    e = nextEpoch(e, 1000, 1_000_000_050); // 50 ms late
    expect(e).toBe(999_999_050);
    e = nextEpoch(e, 2000, 1_000_001_010); // 10 ms late
    expect(e).toBe(999_999_010);
    e = nextEpoch(e, 3000, 1_000_002_100); // later again: kept
    expect(e).toBe(999_999_010);
  });

  it("formats the local time", () => {
    const epoch = new Date(2026, 9, 17, 13, 4, 5, 6).getTime();
    expect(timeOfDay(0, epoch)).toBe("13:04:05.006");
    expect(timeOfDay(3_600_000 + 994, epoch)).toBe("14:04:06.000");
  });

  it("takes the time of day from the core's clock, past midnight too", () => {
    const at = ((23 * 60 + 59) * 60 + 58) * 1000 + 500; // 23:59:58.500 at time 0
    expect(coreTimeOfDay(0, at)).toBe("23:59:58.500");
    expect(coreTimeOfDay(1_499, at)).toBe("23:59:59.999");
    expect(coreTimeOfDay(1_500, at)).toBe("00:00:00.000");
    expect(coreTimeOfDay(86_400_000 + 3_723_004, 0)).toBe("01:02:03.004");
  });
});
