import { beforeAll, describe, expect, it } from "vitest";
import { setLanguage } from "../i18n";
import type { CodecDetails, LinkSample, StreamInfo } from "../ipc/types";
import { abrTooltipKey, codecDetails, fallbackNoteKey, linkQuality } from "./stream";

const sample = (i: number, over: Partial<LinkSample> = {}): LinkSample => ({
  time_ms: i * 250,
  rssi: -18,
  afh_in_use: 79,
  afh_usable: 79,
  afh_avoided: "",
  bytes_per_second: 125_000,
  packets_per_second: 188,
  queue_depth: 0,
  dropped: 0,
  capture_dropped: 0,
  airtime: 0.4,
  ...over,
});
const run = (n: number, f: (i: number) => Partial<LinkSample> = () => ({})) =>
  Array.from({ length: n }, (_, i) => sample(i, f(i)));

describe("link quality", () => {
  it("is good on a link that delivers everything, whatever the RSSI", () => {
    const q = linkQuality(run(60, (i) => ({ rssi: i % 2 ? -27 : -6 })));
    expect(q).toEqual({ level: 4, status: "good", pcBusy: false });
  });

  it("does not blame the radio for audio the PC lost at capture (issue #252)", () => {
    const q = linkQuality(run(60, (i) => ({ capture_dropped: i < 50 ? 0 : 480 })));
    expect(q).toEqual({ level: 4, status: "warn", pcBusy: true });
    const later = run(60, (i) => ({ capture_dropped: i < 10 ? 0 : 480 }));
    expect(linkQuality(later)?.pcBusy).toBe(false);
  });

  it("says what the radio does when both go wrong", () => {
    const q = linkQuality(run(60, (i) => ({ dropped: i < 50 ? 0 : 128, capture_dropped: i < 50 ? 0 : 480 })));
    expect(q).toEqual({ level: 1, status: "bad", pcBusy: false });
  });

  it("is poor while packets drop, and recovers 10 s later", () => {
    const drops = run(60, (i) => ({ dropped: i < 50 ? 0 : 128 }));
    expect(linkQuality(drops)?.status).toBe("bad");
    const later = [...drops, ...run(40, () => ({ dropped: 128 }))];
    expect(linkQuality(later)?.status).toBe("good");
  });

  it("is fair while the send queue backs up", () => {
    const q = linkQuality(run(30, (i) => ({ queue_ms: i === 25 ? 200 : 0 })));
    expect(q?.status).toBe("warn");
    expect(q?.pcBusy).toBe(false);
  });

  it("has no data when nothing streams", () => {
    expect(linkQuality([])).toBeNull();
    expect(linkQuality([sample(0, { bytes_per_second: 0, rssi: null })])).toBeNull();
  });
});

const SBC_ABR: CodecDetails = {
  mtu: 1011,
  rtp: true,
  media_header: 1,
  frame_samples: 128,
  frame_bytes: 95,
  frames_per_packet: 10,
  sbc: {
    channel_mode: "joint_stereo",
    blocks: 16,
    subbands: 8,
    allocation: "loudness",
    bitpool: 41,
    config_bitpool: [2, 53],
    device: null,
    abr_bitpool: [19, 53],
  },
  aac: null,
  device: null,
};

const SBC_INFO: StreamInfo = {
  codec: "SBC",
  bitrate_kbps: 285,
  sample_rate: 48000,
  channels: 2,
  bits: 16,
  source_sample_rate: 48000,
  source_channels: 2,
  source_bits: 32,
  requested_codec: null,
  requested_in_use: false,
  abr: true,
  bitrate_capped: false,
  source: null,
  le: null,
  details: SBC_ABR,
};

describe("codec summary", () => {
  beforeAll(() => setLanguage("en"));

  it("shows SBC's bitpool now and its ceiling under ABR", () => {
    expect(codecDetails(SBC_INFO)).toEqual(["285 kbps", "48 kHz", "16-bit", "ABR", "bitpool 41/53"]);
    expect(codecDetails(SBC_INFO).join(" | ")).toBe("285 kbps | 48 kHz | 16-bit | ABR | bitpool 41/53");
    expect(abrTooltipKey(SBC_INFO)).toBe("tooltip.abr");
  });

  it("shows no bitpool without ABR, nor for other codecs", () => {
    const fixed: StreamInfo = { ...SBC_INFO, abr: false, details: { ...SBC_ABR, sbc: { ...SBC_ABR.sbc!, abr_bitpool: null } } };
    expect(codecDetails(fixed)).toEqual(["285 kbps", "48 kHz", "16-bit"]);
    const ldac: StreamInfo = { ...SBC_INFO, codec: "LDAC", bitrate_kbps: 660, bits: 24, details: null };
    expect(codecDetails(ldac)).toEqual(["660 kbps", "48 kHz", "24-bit", "ABR"]);
    expect(abrTooltipKey(ldac)).toBe("tooltip.abr");
  });

  it("calls AAC's flag VBR (the core names the codec \"AAC\")", () => {
    const aac: StreamInfo = { ...SBC_INFO, codec: "AAC", bitrate_kbps: 256, sample_rate: 44100, details: null };
    expect(codecDetails(aac)).toEqual(["256 kbps", "44.1 kHz", "16-bit", "VBR"]);
    expect(abrTooltipKey(aac)).toBe("tooltip.vbr");
  });

  it("says beside the codec why the chosen one is not used (issue #86)", () => {
    expect(fallbackNoteKey(SBC_INFO, "status.connected")).toBeNull();
    const opus: StreamInfo = { ...SBC_INFO, requested_codec: "Opus" };
    expect(fallbackNoteKey(opus, "status.connected_fallback")).toBe("ui.codec.fallback_unsupported");
    expect(fallbackNoteKey({ ...opus, requested_in_use: true }, "status.connected_fallback_in_use")).toBe("ui.codec.fallback_in_use");
    expect(fallbackNoteKey(opus, "status.connected_fallback_experimental")).toBe("ui.codec.fallback_experimental");
    // A later message (a reconnect) keeps the note
    expect(fallbackNoteKey(opus, "status.reconnected")).toBe("ui.codec.fallback_unsupported");
  });
});
