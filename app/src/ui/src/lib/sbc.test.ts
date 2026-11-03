// The SBC bit rate as the core works it out (crates/codec/src/sbc/mod.rs
// tests have the same numbers)

import { describe, expect, it } from "vitest";
import { sampleDevice, sampleProfile, XM5_SBC } from "../ipc/mock";
import { knownSbcCaps, sbcFrameLength, sbcKbps, sbcKbpsText, sbcOffered, sbcRate, sbcSetup } from "./sbc";

const sbc = (over: Partial<ReturnType<typeof sampleProfile>> = {}) => sampleProfile({ codec: "sbc", ...over });

describe("SBC frame length and bit rate", () => {
  it("follows the A2DP specification's formula", () => {
    // (rate, blocks, subbands, mode, bitpool, frame length, kbps)
    const cases: [number, number, number, string, number, number, number][] = [
      [44100, 16, 8, "joint_stereo", 53, 119, 327],
      [44100, 16, 8, "stereo", 53, 118, 325],
      [44100, 16, 8, "dual_channel", 32, 140, 385],
      [44100, 16, 8, "mono", 31, 70, 192],
      [44100, 8, 4, "joint_stereo", 35, 44, 485],
      [48000, 8, 4, "stereo", 35, 43, 516],
      [44100, 4, 8, "joint_stereo", 53, 40, 441],
      [48000, 12, 8, "joint_stereo", 53, 93, 372],
      [48000, 12, 4, "dual_channel", 20, 68, 544],
      [44100, 4, 4, "mono", 30, 21, 463],
    ];
    for (const [rate, blocks, subbands, mode, bitpool, len, kbps] of cases) {
      expect(sbcFrameLength(mode, blocks, subbands, bitpool)).toBe(len);
      expect(sbcKbps(len, rate, blocks, subbands)).toBe(kbps);
    }
  });

  it("works out a profile's setup as the core", () => {
    // Auto: joint stereo, 16 blocks, 8 subbands, the quality's bitpool
    expect(sbcSetup(sbc(), 44100, null)).toMatchObject({ blocks: 16, subbands: 8, mode: "joint_stereo", bitpool: 53, kbps: 327 });
    expect(sbcSetup(sbc({ quality: "sq" }), 48000, null)).toMatchObject({ bitpool: 35, kbps: 249 });
    // Plain SBC has no dual channel (SBC XQ is that, issue #86): joint stereo; a bitpool asked for wins
    expect(sbcSetup(sbc({ sbc_channel_mode: "dual_channel" }), 44100, null)).toMatchObject({ mode: "joint_stereo", bitpool: 53 });
    expect(sbcSetup(sbc({ sbc_bitpool: 40, sbc_blocks: 8, sbc_subbands: 4 }), 44100, null)).toMatchObject({ frameBytes: 49, kbps: 540 });
    // SBC XQ: dual channel whatever was asked, its own bitpools
    const xq = sampleProfile({ codec: "sbcxq", sbc_channel_mode: "stereo" });
    expect(sbcSetup(xq, 44100, null)).toMatchObject({ mode: "dual_channel", bitpool: 47, frameBytes: 200, kbps: 551 });
    expect(sbcSetup({ ...xq, sbcxq_bitpool: 53 }, 48000, null)).toMatchObject({ bitpool: 43 });
    // The minimum (issue #86): the bitpool never below it, nor above the cap
    expect(sbcSetup(sbc({ quality: "mq", sbc_min_bitpool: 30 }), 44100, null)).toMatchObject({ bitpool: 30 });
    expect(sbcSetup(sbc({ sbc_bitpool: 25, sbc_min_bitpool: 30 }), 44100, null)).toMatchObject({ bitpool: 30 });
    expect(sbcSetup({ ...xq, sbcxq_min_bitpool: 53 }, 48000, null)).toMatchObject({ bitpool: 43 });
    // ... and the device's range still wins
    expect(sbcSetup(sbc({ sbc_min_bitpool: 45 }), 44100, { ...XM5_SBC, max_bitpool: 35 })).toMatchObject({ bitpool: 35 });
  });

  it("takes what the device does not list as the default, and its bitpool range", () => {
    const caps = { ...XM5_SBC, max_bitpool: 35 };
    const s = sbcSetup(sbc({ sbc_subbands: 4, sbc_allocation: "snr", sbc_blocks: 8 }), 44100, caps);
    expect(s).toMatchObject({ blocks: 8, subbands: 8, allocation: "loudness", bitpool: 35 });
    expect(sbcOffered(caps, "subbands", 4)).toBe(false);
    expect(sbcOffered(caps, "blocks", 4)).toBe(true);
    expect(sbcOffered(null, "subbands", 4)).toBe(true);
    expect(sbcRate({ sample_rate: 0 }, { ...caps, sample_rates: [48000] })).toBe(48000);
    expect(sbcRate({ sample_rate: 0 }, { ...caps, sample_rates: [44100] })).toBe(44100);
    expect(sbcRate({ sample_rate: 44100 }, null)).toBe(44100);
    // Auto: 48 kHz, 44.1 kHz only on a device without it
    expect(sbcRate({ sample_rate: 0 }, null)).toBe(48000);
    expect(sbcKbpsText(sbc({}), null)).toBe(`≈ ${sbcSetup(sbc({}), 48000, null).kbps} kbps`);
  });

  it("finds the caps of the profile's device", () => {
    const list = [sampleDevice({ address: "AC:80:0A:12:34:56", sbc: XM5_SBC }), sampleDevice({ address: "00:1A:7D:DA:71:13" })];
    expect(knownSbcCaps(list, "ac-80-0a-12-34-56")).toEqual(XM5_SBC);
    expect(knownSbcCaps(list, "00:1A:7D:DA:71:13")).toBeNull();
    expect(knownSbcCaps(list, "nope")).toBeNull();
  });
});
