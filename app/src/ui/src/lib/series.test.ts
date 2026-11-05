import { describe, expect, it } from "vitest";
import type { LinkSample, LinkTotals } from "../ipc/types";
import { chipPacketText } from "../screens/diagnostics/RadioPanel";
import { gapsText } from "../screens/diagnostics/CapturePanel";
import {
  captureDropsPerSample,
  captureLostPerSample,
  lastKnown,
  lastKnownIndex,
  nearestIndex,
  niceCeil,
  ratePerSecond,
  scaleOf,
  timeRuns,
} from "./series";

const totals = (over: Partial<LinkTotals> = {}): LinkTotals => ({
  packets: 0,
  bytes: 0,
  dropped: 0,
  capture_frames: 0,
  capture_dropped: 0,
  capture_discontinuities: 0,
  capture_lost: 0,
  ...over,
});
const sample = (time_ms: number, total: Partial<LinkTotals>, over: Partial<LinkSample> = {}): LinkSample => ({
  time_ms,
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
  counters: { total: totals(total), stream_start: totals(), stream_starts: 1 },
  ...over,
});

describe("chart series", () => {
  it("rounds an automatic scale's top up to a round number", () => {
    expect(niceCeil(0)).toBe(1);
    expect(niceCeil(7)).toBe(8);
    expect(niceCeil(1010)).toBe(1200);
    expect(niceCeil(18)).toBe(20);
    expect(niceCeil(21)).toBe(25);
    expect(niceCeil(330)).toBe(400);
    expect(niceCeil(1000)).toBe(1000);
    // Fixed ends stay; references widen an automatic scale
    expect(scaleOf([[1, 2, null]], 0)).toEqual([0, 2]);
    expect(scaleOf([[1, 2], [140]], 0)).toEqual([0, 150]);
    expect(scaleOf([[-30, -5]], -40, 10)).toEqual([-40, 10]);
    expect(scaleOf([[]], 0)).toEqual([0, 1]);
  });

  it("draws samples on the time axis and breaks the line at gaps", () => {
    const times = [0, 250, 500, 3000, 3250];
    const runs = timeRuns(times, [0, 1, null, 1, 0], 0, 4000, 0, 1, 400, 100);
    // Unknown value at 500, nothing between 500 and 3000
    expect(runs).toEqual(["0.00,100.00 25.00,0.00", "300.00,0.00 325.00,100.00"]);
    // Samples before the axis starts are left out
    expect(timeRuns([0, 250], [1, 1], 100, 1000, 0, 1, 100, 10)).toEqual(["15.00,0.00"]);
  });

  it("finds the sample under the cursor", () => {
    expect(nearestIndex([0, 250, 500], 300)).toBe(1);
    expect(nearestIndex([0, 250, 500], 5000)).toBeNull();
    expect(nearestIndex([], 0)).toBeNull();
    expect(lastKnown([1, null, 3, null])).toBe(3);
    expect(lastKnown([null])).toBeNull();
    expect(lastKnownIndex([1, null, 3, null])).toBe(2);
    expect(lastKnownIndex([])).toBeNull();
  });

  it("turns counters into rates and capture drops into audio time", () => {
    const s = [
      sample(0, { capture_frames: 0 }),
      sample(250, { capture_frames: 12000 }),
      sample(500, { capture_frames: 24000, capture_dropped: 480 }),
      // A new core: the counters went back
      sample(750, { capture_frames: 100 }),
    ];
    expect(ratePerSecond(s, "capture_frames")).toEqual([null, 48000, 48000, null]);
    expect(captureDropsPerSample(s, 0)).toEqual([0, 0, 10, 0]);
    // Without the stream's rate: the fallback, else frames
    const noRate = s.map((x) => ({ ...x, source_rate: 0 }));
    expect(captureDropsPerSample(noRate, 96000)[2]).toBe(5);
    expect(captureDropsPerSample(noRate, 0)[2]).toBe(480);
  });

  it("puts Windows' capture gaps in audio time, unknown where a gap lost nothing measured", () => {
    const s = [
      sample(0, {}),
      sample(250, {}, { capture_discontinuities: 1, capture_lost: 960 }),
      sample(500, {}, { capture_discontinuities: 1, capture_lost: 960 }),
      // Per-app capture: a gap without the frames it lost
      sample(750, {}, { capture_discontinuities: 2, capture_lost: 960 }),
    ];
    expect(captureLostPerSample(s, 0)).toEqual([0, 20, 0, null]);
    expect(captureLostPerSample(s.map((x) => ({ ...x, source_rate: 0 })), 0)[1]).toBeNull();
    // The row: the count, with the audio lost when it is known
    expect(gapsText(0, 0)).toBe("0");
    expect(gapsText(3, 0)).toBe("3");
    expect(gapsText(3, null)).toBe("3");
    expect(gapsText(2, 40)).toBe("2 (40 ms lost)");
  });

  it("names the remote chip's fastest packet type with its rate", () => {
    const features = { edr_2m: true, edr_3m: true, three_slot: true, five_slot: true, edr_three_slot: true, edr_five_slot: true };
    expect(chipPacketText({ features: { ...features, largest_packet: "3-DH5" } })).toBe("3-DH5 (3 Mbps)");
    expect(chipPacketText({ features: { ...features, edr_3m: false, largest_packet: "2-DH5" } })).toBe("2-DH5 (2 Mbps)");
    expect(chipPacketText({ features: { ...features, edr_2m: false, edr_3m: false, largest_packet: "DH5" } })).toBe("DH5 (1 Mbps)");
    expect(chipPacketText({ company: "Airoha" })).toBe("–");
  });
});
