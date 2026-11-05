import { describe, expect, it } from "vitest";
import { apLevel, busiestWifiChannel, channelsInUse, parseWifiChannels, wifiCenterMhz, wifiCovers } from "./radio";

const ALL = [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f];

describe("radio", () => {
  it("reads the channel map bit by bit", () => {
    expect(channelsInUse(null)).toBeNull();
    expect(channelsInUse([1, 2])).toBeNull();
    const all = channelsInUse(ALL)!;
    expect(all).toHaveLength(79);
    expect(all.every(Boolean)).toBe(true);
    const m = channelsInUse([0xfe, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x3f])!;
    expect(m[0]).toBe(false);
    expect(m[1]).toBe(true);
    expect(m[78]).toBe(false);
  });

  it("places Wi-Fi channels as bt-host's afh module does", () => {
    expect(wifiCenterMhz(1)).toBe(2412);
    expect(wifiCenterMhz(14)).toBe(2484);
    // Channel 6: 2426-2448 MHz = Bluetooth channels 24-46
    const six = wifiCovers(6);
    expect(six[0]).toBe(24);
    expect(six[six.length - 1]).toBe(46);
    expect(parseWifiChannels("6, 11")).toEqual([6, 11]);
    expect(parseWifiChannels("0;15;3")).toEqual([3]);
  });

  it("names the Wi-Fi channel the link leaves out most", () => {
    expect(busiestWifiChannel(null)).toBeNull();
    expect(busiestWifiChannel(channelsInUse(ALL))).toBeNull();
    const inUse = channelsInUse(ALL)!;
    for (const n of wifiCovers(11)) inUse[n] = false;
    expect(busiestWifiChannel(inUse)).toBe(11);
    // Under half of a channel's width left out is not named
    const few = channelsInUse(ALL)!;
    for (const n of wifiCovers(1).slice(0, 5)) few[n] = false;
    expect(busiestWifiChannel(few)).toBeNull();
  });

  it("scales signal to the drawing", () => {
    expect(apLevel(-95)).toBe(0);
    expect(apLevel(-60)).toBeCloseTo(0.5);
    expect(apLevel(-20)).toBe(1);
  });
});
