import { describe, expect, it } from "vitest";
import type { LinkSample, LinkTotals } from "../ipc/types";
import {
  activeBase,
  diffTotals,
  dropsPerSample,
  EDR3_MAX_KBPS,
  formatBytes,
  gaugeValue,
  lossStats,
  percent,
  takeBaseline,
  ZERO_TOTALS,
} from "./linkstats";

const totals = (t: Partial<LinkTotals>): LinkTotals => ({ ...ZERO_TOTALS, ...t });

function sample(total: Partial<LinkTotals>, start: Partial<LinkTotals> = {}, extra: Partial<LinkSample> = {}): LinkSample {
  return {
    time_ms: 1000,
    rssi: null,
    afh_in_use: null,
    afh_usable: null,
    afh_avoided: "",
    bytes_per_second: 0,
    packets_per_second: 0,
    queue_depth: 0,
    dropped: 0,
    capture_dropped: 0,
    airtime: 0,
    source_rate: 48000,
    counters: { total: totals(total), stream_start: totals(start), stream_starts: 1 },
    ...extra,
  };
}

describe("link statistics", () => {
  it("knows the EDR 3 Mbps ceiling", () => {
    expect(EDR3_MAX_KBPS).toBeCloseTo(2178.13, 1);
  });

  it("subtracts counters without going negative", () => {
    expect(diffTotals(totals({ packets: 10, bytes: 100 }), totals({ packets: 4, bytes: 200 }))).toEqual(
      totals({ packets: 6, bytes: 0 }),
    );
  });

  it("counts since the stream started without a reset", () => {
    const s = sample({ packets: 1100, dropped: 12, capture_frames: 96000, capture_dropped: 480 }, { packets: 100, dropped: 2 });
    const l = lossStats(s, null)!;
    expect(l.sinceReset).toBe(false);
    expect(l.sent).toBe(1000);
    expect(l.dropped).toBe(10);
    // 10 of 1010 packets
    expect(l.dropPct).toBeCloseTo(0.990099, 5);
    // 480 frames at 48 kHz = 10 ms, of 96000 frames = 0.5 %
    expect(l.captureMs).toBe(10);
    expect(l.capturePct).toBeCloseTo(0.5, 6);
  });

  it("counts since Reset, until a newer stream starts or the core restarts", () => {
    const at = sample({ packets: 500, dropped: 5 }, { packets: 100 });
    const reset = takeBaseline(at)!;
    expect(reset.totals.packets).toBe(500);
    const later = sample({ packets: 800, dropped: 6 }, { packets: 100 });
    expect(lossStats(later, reset)).toMatchObject({ sinceReset: true, sent: 300, dropped: 1 });

    // A new stream after the reset: from its start
    const next = sample({ packets: 900, dropped: 6 }, { packets: 850, dropped: 6 });
    next.counters!.stream_starts = 2;
    expect(activeBase(next.counters!, reset)).toEqual({ base: next.counters!.stream_start, sinceReset: false });
    expect(lossStats(next, reset)).toMatchObject({ sinceReset: false, sent: 50, dropped: 0 });

    // Counters below the baseline: a new core
    const restarted = sample({ packets: 20 }, {});
    expect(lossStats(restarted, reset)).toMatchObject({ sinceReset: false, sent: 20 });
  });

  it("converts capture drops with the last known rate when not streaming", () => {
    const idle = sample({ capture_frames: 44100, capture_dropped: 441 }, {}, { source_rate: 0 });
    expect(lossStats(idle, null, 44100)!.captureMs).toBe(10);
    expect(lossStats(idle, null)!.captureMs).toBeNull();
    const clean = sample({ capture_frames: 44100 }, {}, { source_rate: 0 });
    expect(lossStats(clean, null)!.captureMs).toBe(0);
  });

  it("has no losses without counters (an older core)", () => {
    expect(lossStats(sample({}, {}, { counters: null }), null)).toBeNull();
    expect(takeBaseline(sample({}, {}, { counters: null }))).toBeNull();
    expect(takeBaseline(undefined)).toBeNull();
  });

  it("drops per sample from the counters or the per-stream fields", () => {
    const a = sample({ dropped: 3, capture_dropped: 1 });
    const b = sample({ dropped: 5, capture_dropped: 4 });
    const c = sample({ dropped: 5, capture_dropped: 4 });
    expect(dropsPerSample([a, b, c])).toEqual([0, 2, 0]);
    const old = (dropped: number) => sample({}, {}, { counters: null, dropped });
    expect(dropsPerSample([old(1), old(4), old(0)])).toEqual([0, 3, 0]);
  });

  it("percentages and gauges", () => {
    expect(percent(1, 0)).toBe(0);
    expect(percent(1, 4)).toBe(25);
    expect(gaugeValue(-3)).toBe(0);
    expect(gaugeValue(140)).toBe(100);
    expect(gaugeValue(null)).toBe(0);
    expect(gaugeValue(NaN)).toBe(0);
    expect(gaugeValue(42.5)).toBe(42.5);
  });

  it("formats file sizes", () => {
    expect(formatBytes(64)).toBe("64 B");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(12 * 1024 * 1024 + 300 * 1024)).toBe("12.3 MB");
    expect(formatBytes(512 * 1024 * 1024)).toBe("512 MB");
  });
});
