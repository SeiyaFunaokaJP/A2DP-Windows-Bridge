import { cleanup, render } from "@solidjs/testing-library";
import { createSignal } from "solid-js";
import { afterEach, beforeAll, describe, expect, it } from "vitest";
import { setLanguage } from "../i18n";
import type { CodecDetails, StreamInfo } from "../ipc/types";
import { bitpoolBar, chips, frameMs, liveBitpool, packetFill } from "../lib/codecdetails";
import { CodecDetailsCard } from "./CodecDetails";

const SBC: CodecDetails = {
  mtu: 679,
  rtp: true,
  media_header: 1,
  frame_samples: 128,
  frame_bytes: 119,
  frames_per_packet: 5,
  sbc: {
    channel_mode: "joint_stereo",
    blocks: 16,
    subbands: 8,
    allocation: "loudness",
    bitpool: 51,
    config_bitpool: [2, 51],
    device: {
      sample_rates: [44100, 48000],
      channel_modes: ["mono", "joint_stereo"],
      blocks: [4, 8, 12, 16],
      subbands: [8],
      allocations: ["loudness"],
      min_bitpool: 2,
      max_bitpool: 51,
    },
  },
  aac: null,
  device: null,
};

const INFO: StreamInfo = {
  codec: "SBC",
  bitrate_kbps: 317,
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
  details: SBC,
};

beforeAll(() => setLanguage("en"));
afterEach(cleanup);

describe("codec details", () => {
  it("marks each value as in use, offered or not offered", () => {
    expect(chips([4, 8], [8], 8)).toEqual([
      { value: 4, state: "off" },
      { value: 8, state: "used" },
    ]);
    expect(chips(["a", "b", "c"], ["a", "c"], "c").map((c) => c.state)).toEqual(["offered", "off", "used"]);
    // The device did not say: only what is used
    expect(chips([4, 8], null, 8)).toEqual([{ value: 8, state: "used" }]);
    // A value outside the list still shows
    expect(chips([1, 2], [1], 3).map((c) => c.value)).toEqual([1, 2, 3]);
  });

  it("scales the bitpool bar to 64 or beyond the largest value", () => {
    const bar = bitpoolBar(SBC.sbc!);
    expect(bar.max).toBe(64);
    expect(bar.used).toBeCloseTo((51 / 64) * 100);
    expect(bar.device!.from).toBeCloseTo((2 / 64) * 100);
    expect(bar.config!.width).toBeCloseTo((49 / 64) * 100);
    const wide = bitpoolBar({ ...SBC.sbc!, device: { ...SBC.sbc!.device!, max_bitpool: 94 }, config_bitpool: null });
    expect(wide.config).toBeNull();
    expect(wide.max).toBe(96);
  });

  it("works out frame durations and how full a packet is", () => {
    expect(frameMs(128, 48000)).toBeCloseTo(2.667, 3);
    expect(frameMs(1024, 0)).toBe(0);
    expect(packetFill(SBC)).toEqual({ used: 1 + 5 * 119, of: 679 });
    expect(packetFill({ ...SBC, frames_per_packet: null })).toBeNull();
  });

  it("shows the SBC setup against what the device offered", () => {
    const view = render(() => <CodecDetailsCard info={INFO} details={SBC} />);
    const c = view.container;
    expect(c.querySelector("h2")).toHaveTextContent("Codec details SBC");
    expect(c).toHaveTextContent("Up to 679 bytes");
    expect(c).toHaveTextContent("RTP 12 B + media header 1 B");
    expect(c).toHaveTextContent("119 bytes, 128 samples (2.67 ms)");
    expect(c).toHaveTextContent("5 (596 of 679 bytes used)");
    expect(c.querySelector(".bitpool-track")).toHaveAttribute("aria-label", "Device 2–51, Configured 2–51, In use 51");
    const used = [...c.querySelectorAll(".cd-chip.cd-used")].map((e) => e.firstChild!.textContent);
    expect(used).toEqual(["48 kHz", "Joint stereo", "16", "8", "Loudness"]);
    const off = [...c.querySelectorAll(".cd-chip.cd-off")].map((e) => e.textContent);
    expect(off).toContain("16 kHz (not offered)");
    expect(off).toContain("Dual channel (not offered)");
    expect(off).toContain("SNR (not offered)");
    expect(c).toHaveTextContent("Filled: in use.");
  });

  it("shows only what is used when the device did not report its capabilities", () => {
    const details: CodecDetails = { ...SBC, sbc: { ...SBC.sbc!, device: null, config_bitpool: null } };
    const view = render(() => <CodecDetailsCard info={INFO} details={details} />);
    const c = view.container;
    expect(c.querySelectorAll(".cd-chip.cd-offered, .cd-chip.cd-off")).toHaveLength(0);
    expect(c.querySelector(".bitpool-track")).toHaveAttribute("aria-label", "In use 51");
    expect(c).toHaveTextContent("The device did not report these capabilities");
    expect(c).not.toHaveTextContent("Filled: in use.");
  });

  it("shows what any other codec's endpoint lists, as for SBC and AAC (issue #86)", () => {
    const ldac: CodecDetails = {
      ...SBC,
      frame_bytes: null,
      frames_per_packet: null,
      sbc: null,
      device: { codec: "ldac", sample_rates: [44100, 48000, 96000], bit_depths: [16, 24], max_bitrate: 0 },
    };
    const info = { ...INFO, codec: "LDAC", sample_rate: 96000, bits: 24, details: ldac };
    const c = render(() => <CodecDetailsCard info={info} details={ldac} />).container;
    expect(c.querySelector("h3")).toHaveTextContent("LDAC");
    expect([...c.querySelectorAll(".cd-chip")].map((x) => x.textContent)).toEqual([
      "44.1 kHz",
      "48 kHz",
      "96 kHz (in use)",
      "16-bit",
      "24-bit (in use)",
    ]);
    expect(c).toHaveTextContent("Device's peak bit rateNot given");
    expect(c).toHaveTextContent("Filled: in use.");
  });

  it("shows AAC's object type, peak bit rate and VBR", () => {
    const aac: CodecDetails = {
      mtu: 894,
      rtp: true,
      media_header: 0,
      frame_samples: 1024,
      frame_bytes: null,
      frames_per_packet: null,
      sbc: null,
      aac: {
        object_type: "MPEG-2 AAC-LC",
        bit_rate: 256000,
        device: {
          object_types: ["MPEG-2 AAC-LC", "MPEG-4 HE-AAC"],
          sample_rates: [44100, 48000, 96000],
          channels: [1, 2],
          max_bitrate: 0,
          vbr: true,
        },
      },
      device: null,
    };
    const view = render(() => <CodecDetailsCard info={{ ...INFO, codec: "AAC", sample_rate: 44100, details: aac }} details={aac} />);
    const c = view.container;
    expect(c).toHaveTextContent("HeadersRTP 12 B");
    expect(c).toHaveTextContent("1024 samples (23.22 ms)");
    expect(c).toHaveTextContent("CBR 256 kbps");
    expect(c).toHaveTextContent("Device's peak bit rateNot given");
    expect(c).toHaveTextContent("Device takes VBRYes");
    expect(c.querySelector(".gauge")).toBeNull();
    const used = [...c.querySelectorAll(".cd-chip.cd-used")].map((e) => e.firstChild!.textContent);
    expect(used).toEqual(["MPEG-2 AAC-LC", "44.1 kHz", "2"]);
    expect(c.querySelectorAll(".cd-chip.cd-off")).toHaveLength(0);
  });

  it("shows SBC ABR's range and the bitpool as it moves", () => {
    const abr = (bitpool: number): CodecDetails => ({
      ...SBC,
      sbc: { ...SBC.sbc!, bitpool, config_bitpool: [2, 53], device: { ...SBC.sbc!.device!, max_bitpool: 53 }, abr_bitpool: [19, 53] },
    });
    const info = (bitpool: number): StreamInfo => ({ ...INFO, abr: true, details: abr(bitpool) });
    expect(liveBitpool(info(41))).toEqual({ now: 41, min: 19, max: 53 });
    // Not under ABR, or no ABR range (an older core, another codec): none
    expect(liveBitpool({ ...info(41), abr: false })).toBeNull();
    expect(liveBitpool(INFO)).toBeNull();
    expect(liveBitpool({ ...INFO, abr: true, details: null })).toBeNull();
    const bar = bitpoolBar(abr(41).sbc!);
    expect(bar.abr!.from).toBeCloseTo((19 / 64) * 100);
    expect(bar.abr!.width).toBeCloseTo((34 / 64) * 100);
    expect(bitpoolBar(SBC.sbc!).abr).toBeNull();

    const [bp, setBp] = createSignal(53);
    const view = render(() => <CodecDetailsCard info={info(bp())} details={abr(bp())} />);
    const c = view.container;
    expect(c).toHaveTextContent("Bitpool53 (ABR 19–53)");
    expect(c.querySelector(".bitpool-track")).toHaveAttribute("aria-label", "Device 2–53, Configured 2–53, ABR 19–53, In use 53");
    expect(c.querySelector(".bitpool.live")).not.toBeNull();
    expect(c.querySelector(".bitpool-range.abr")).not.toBeNull();
    expect(c).toHaveTextContent("ABR lowers it when the send queue backs up");
    // A new stream info moves it
    setBp(41);
    expect(c).toHaveTextContent("Bitpool41 (ABR 19–53)");
    const used = c.querySelector<HTMLElement>(".bitpool-used")!;
    expect(parseFloat(used.style.left)).toBeCloseTo((41 / 64) * 100);
  });

  it("shows a fixed bitpool without ABR", () => {
    const view = render(() => <CodecDetailsCard info={INFO} details={SBC} />);
    const c = view.container;
    expect(c).toHaveTextContent("Bitpool51");
    expect(c).not.toHaveTextContent("ABR");
    expect(c.querySelector(".bitpool.live")).toBeNull();
    expect(c.querySelector(".bitpool-range.abr")).toBeNull();
  });
});
