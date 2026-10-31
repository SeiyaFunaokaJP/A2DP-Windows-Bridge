import { describe, expect, it } from "vitest";
import { setLanguage, t } from "../i18n";
import { sampleProfile } from "../ipc/mock";
import type { LogLine } from "../ipc/types";
import {
  abrCarries,
  AUTO_ORDER,
  autoOrder,
  autoPick,
  bitDepths,
  bitrateNoteKey,
  capsOf,
  CARD_ORDER,
  cardOrder,
  carryToCodec,
  codecLabelKey,
  CODECS,
  effectiveCodec,
  fitToCodec,
  isDefaultOrder,
  isExperimental,
  fixedBitDepth,
  moved,
  offeredBitDepths,
  offeredQualities,
  offeredSampleRates,
  qualityIgnored,
  qualityKbps,
  qualityTag,
  sampleRates,
} from "./codecs";
import { FlowTracker, stepDetail } from "./flow";
import { knowAdapters, maskIdentifiers, parseAddress } from "./format";
import { bugReportUrl, thirdPartyUrl } from "./links";
import { deviceProfile, followedName, validateProfile, withCodec } from "./profile";

const line = (time_ms: number, message: string, level: LogLine["level"] = "info"): LogLine => ({
  time_ms,
  level,
  target: "test",
  message,
});

describe("connection flow", () => {
  it("follows a connection from start to streaming", () => {
    const f = new FlowTracker();
    const lines = [
      "stream: device AC:80:0A:12:34:56 codec Ldac quality Hq",
      "state Connecting: Starting Bluetooth...",
      "controller: company 0x005D (Realtek), HCI version 10",
      "HCI ready after 412 ms",
      "connecting to AC:80:0A:12:34:56",
      "HCI ACL connection established to AC:80:0A:12:34:56",
      "signaling connection established (cid 0x0041, handle 0x000b)",
      "capability discovery complete (LDAC=true SBC=true)",
      "codec selected: LDAC",
      "capture: 48000 Hz, 2 channel(s), F32 -> 2 channel(s)",
      "HCI capture: started -> C:\\x.pklg",
      "codec configured (local 1, remote 3)",
      "stream established (local 1, remote 3, mtu 1005)",
      "encoder ready: LDAC 990 kbps, mtu 1005, 24-bit, ABR false",
      "streaming started",
      "state Streaming: Streaming",
    ];
    lines.forEach((m, i) => f.feed(line(1000 + i, m)));
    const s = f.session!;
    for (const step of Object.values(s.steps)) expect(step.status).toBe("done");
    expect(s.steps.btstack.detail).toContain("Realtek");
    expect(s.steps.codec.detail).toBe("LDAC");
    expect(s.steps.capture.detail).toContain("48000 Hz");
  });

  it("blames the first unfinished step on an error, and counts reconnects", () => {
    const f = new FlowTracker();
    f.feed(line(1, "stream: device AA:BB:CC:DD:EE:FF"));
    f.feed(line(2, "HCI ready after 300 ms"));
    f.feed(line(3, "connecting to AA:BB:CC:DD:EE:FF"));
    f.feed(line(4, "state Error: The device did not answer.", "error"));
    expect(f.session!.steps.connect.status).toBe("failed");
    expect(f.session!.steps.connect.detail).toBe("The device did not answer.");

    f.feed(line(10, "stream: device AA:BB:CC:DD:EE:FF"));
    expect(f.session!.id).toBe(2);
    f.feed(line(11, "state Streaming: Streaming"));
    f.feed(line(12, "connection lost, reconnecting", "warn"));
    expect(f.session!.steps.link.status).toBe("running");
    f.feed(line(13, "reconnected on attempt 1"));
    expect(f.session!.steps.link.status).toBe("warning");
    expect(f.session!.reconnects).toBe(1);
    f.feed(line(14, "streaming stopped, cleaning up"));
    expect(f.session!.stop_ms).toBe(14);
  });

  it("keeps the first connection's steps through a reconnect with a failed attempt", () => {
    const f = new FlowTracker();
    const at = (time_ms: number, target: string, message: string, level: LogLine["level"] = "info") =>
      f.feed({ time_ms, target, level, message });
    at(1, "bluecaster_core::stream", "stream: device 58:18:62:02:7A:D7 codec LDAC");
    at(2, "bt_host", "HCI ready after 69 ms");
    at(3, "bt_host", "connecting to 58:18:62:02:7A:D7");
    at(4, "bt_host", "signaling connection established (cid 0x0041, handle 0x0001)");
    at(5, "bluecaster_core::stream", "codec selected: LDAC");
    at(6, "audio_capture::loopback", 'audio capture: loopback of "CABLE Input": mix 96000 Hz 2 ch 32 bit');
    at(7, "bluecaster_core::stream", "capture: 96000 Hz, 2 channel(s), F32 -> 2 channel(s)");
    at(8, "bt_host::app", "stream established (local 1, remote 3, mtu 895)");
    at(9, "bluecaster_core::service", "state Streaming: Streaming");
    expect(f.session!.steps.capture.status).toBe("done");

    at(20, "bluecaster_core::stream", "connection lost, reconnecting", "warn");
    at(21, "bt_host", "reconnecting to 58:18:62:02:7A:D7");
    at(40, "bt_host::app", "signaling connection failed (0x67)", "warn");
    at(40, "bt_host::app", "stream establishment failed (0x67)", "warn");
    at(40, "bluecaster_core::stream", "reconnect attempt 1 failed", "warn");
    at(43, "bt_host", "reconnecting to 58:18:62:02:7A:D7");
    at(44, "bluecaster_core::stream", "reconnected on attempt 2");
    const s = f.session!;
    expect(s.steps.connect.status).toBe("done");
    expect(s.steps.config.status).toBe("done");
    expect(s.steps.link.status).toBe("warning");
    expect(s.reconnects).toBe(1);
  });

  it("shows a connection that needed a second attempt as a warning", () => {
    const f = new FlowTracker();
    const at = (time_ms: number, target: string, message: string) => f.feed({ time_ms, target, level: "info", message });
    at(1, "bluecaster_core::stream", "stream: device AA:BB:CC:DD:EE:FF");
    at(2, "bt_host", "connecting to AA:BB:CC:DD:EE:FF");
    at(3, "bt_host::app", "signaling connection failed (0x04)");
    at(4, "bt_host", "connection attempt 1/3 failed (PageTimeout), trying again");
    at(5, "bt_host::app", "signaling connection established (cid 0x0041, handle 0x000b)");
    expect(f.session!.steps.connect.status).toBe("warning");
    expect(f.session!.steps.connect.detail).toContain("after 1 failed attempt(s)");
  });

  it("shows the core's computed step details in the UI language", async () => {
    const live = { key: "dbgc.audio_live", args: ["120", "990"] };
    expect(stepDetail({ detail: "120 packets/s, 990 kbps", detail_msg: live })).toBe("120 packets/s, 990 kbps");
    expect(stepDetail({ detail: "reconnecting to AA:BB", detail_msg: null })).toBe("reconnecting to AA:BB");
    const reconnects = { key: "dbgc.link_reconnects", args: ["2"] };
    await setLanguage("ja");
    try {
      expect(stepDetail({ detail: "No disconnects", detail_msg: { key: "dbgc.link_stable", args: [] } })).toBe(t("dbgc.link_stable"));
      expect(t("dbgc.link_stable")).not.toBe("No disconnects");
      // What the core puts after its message stays
      expect(stepDetail({ detail: "2 reconnect(s) - reconnecting to AA:BB", detail_msg: reconnects })).toBe(
        `${t("dbgc.link_reconnects", "2")} - reconnecting to AA:BB`,
      );
    } finally {
      await setLanguage("en");
    }
  });
});

describe("a device's profile (Codecs screen, issue #86)", () => {
  it("keeps the settings of the codec it leaves and brings back those of the one chosen", () => {
    const aac = sampleProfile({ codec: "aac", abr: true, quality: "hq" });
    const ldac = withCodec(aac, "ldac");
    expect(ldac).toMatchObject({ codec: "ldac", abr: false });
    expect(ldac.codec_settings.aac).toMatchObject({ abr: true, quality: "hq" });
    const back = withCodec(ldac, "aac");
    expect(back).toMatchObject({ codec: "aac", abr: true });
    expect(back.codec_settings).not.toHaveProperty("aac");
    expect(back.codec_settings.ldac).toBeDefined();
    // The "..." dialog's values win over what was kept; a quality the codec
    // no longer offers is High (issue #86: AAC has High only)
    const sq = withCodec(ldac, "aac", { ...ldac.codec_settings.aac, quality: "sq", abr: false });
    expect(sq).toMatchObject({ codec: "aac", quality: "hq", abr: false });
    expect(withCodec(aac, "ldac", { ...ldac.codec_settings.aac, quality: "sq" })).toMatchObject({ codec: "ldac", quality: "sq" });
    // Fitted to the codec: SBC's bitpool does not reach AAC
    expect(withCodec(sampleProfile({ codec: "sbc", sbc_bitpool: 40 }), "aac").sbc_bitpool).toBe(0);
  });

  it("names it after the device and codec unless the user named it", () => {
    const p = sampleProfile({ name: "Buds | Auto", device_name: "Buds", codec: "aac" });
    expect(followedName(p, "AAC", [])).toBe("Buds | AAC");
    expect(followedName({ ...p, name: "" }, "AAC", ["Buds | AAC"])).toBe("Buds | AAC (2)");
    expect(followedName({ ...p, name: "Gaming" }, "AAC", [])).toBe("Gaming");
    expect(followedName({ ...p, le_audio: true, name: "" }, "", [])).toBe("Buds LE Audio");
  });

  it("is the one streaming, else the active one, else the first", () => {
    const a = sampleProfile({ name: "A" });
    const b = sampleProfile({ name: "B" });
    const le = sampleProfile({ name: "LE", le_audio: true });
    const idle = { state: "idle", profile: null } as never;
    expect(deviceProfile({ profiles: [a, b, le], active_profile: null, stream: idle }, a.device_address, false)?.name).toBe("A");
    expect(deviceProfile({ profiles: [a, b, le], active_profile: "B", stream: idle }, a.device_address, false)?.name).toBe("B");
    const streaming = { state: "streaming", profile: "A" } as never;
    expect(deviceProfile({ profiles: [a, b, le], active_profile: "B", stream: streaming }, a.device_address, false)?.name).toBe("A");
    expect(deviceProfile({ profiles: [a, b, le], active_profile: "B", stream: idle }, a.device_address, true)?.name).toBe("LE");
  });
});

describe("profiles", () => {
  it("accepts the address forms v1 accepted", () => {
    expect(parseAddress("aa:bb:cc:dd:ee:ff")).toBe("AA:BB:CC:DD:EE:FF");
    expect(parseAddress("AA-BB-CC-DD-EE-FF")).toBe("AA:BB:CC:DD:EE:FF");
    expect(parseAddress("AABBCCDDEEFF")).toBe("AA:BB:CC:DD:EE:FF");
    expect(parseAddress("AA BB CC DD EE FF")).toBe("AA:BB:CC:DD:EE:FF");
    expect(parseAddress("AA:BB:CC:DD:EE")).toBeNull();
    expect(parseAddress("GG:BB:CC:DD:EE:FF")).toBeNull();
  });

  it("validates before saving", () => {
    const ok = sampleProfile();
    expect(validateProfile(ok, [])).toEqual({});
    expect(validateProfile({ ...ok, name: "  " }, []).name?.key).toBe("ui.profile.err_name_empty");
    expect(validateProfile(ok, [ok.name]).name?.key).toBe("ui.profile.err_name_taken");
    expect(validateProfile({ ...ok, device_address: "" }, []).device?.key).toBe("ui.profile.err_device");
    expect(validateProfile({ ...ok, device_address: "12:34" }, []).device?.key).toBe("error.invalid_address_format");
    expect(validateProfile({ ...ok, max_media_payload: 600 }, []).packet?.args).toEqual([679, 1679]);
    expect(validateProfile({ ...ok, max_media_payload: 1680 }, []).packet).toBeDefined();
    expect(validateProfile({ ...ok, capture_mode: "virtual" }, []).audio_device?.key).toBe("error.no_virtual_device_selected");
    expect(validateProfile({ ...ok, capture_mode: "app", app_exe: "" }, []).app?.key).toBe("error.no_app_selected");
  });

  it("drops what the new codec cannot do", () => {
    const p = { ...sampleProfile(), codec: "lhdcv5", quality: "mhq", abr: true, sample_rate: 96000, bit_depth: 24 };
    expect(fitToCodec({ ...p, codec: "ldac" })).toMatchObject({ quality: "hq", abr: true, sample_rate: 96000, bit_depth: 24 });
    // AAC takes the flag as VBR (the editor clears it when the codec changes to or from AAC)
    expect(fitToCodec({ ...p, codec: "aac" })).toMatchObject({ abr: true, sample_rate: 0, bit_depth: 0 });
    // SBC takes it as its adaptive bitpool (the editor clears it when the
    // codec changes to or from SBC / SBC XQ: abrCarries)
    expect(fitToCodec({ ...p, codec: "sbc", sbc_bitpool: 40 })).toMatchObject({ abr: true, sbc_bitpool: 40 });
    expect(fitToCodec({ ...p, codec: "faststream" })).toMatchObject({ abr: false });
    expect(abrCarries("ldac", "lhdcv5")).toBe(true);
    expect(abrCarries("auto", "ldac")).toBe(true);
    // SBC's adaptive bitpool is not SBC XQ's (issue #86)
    expect(abrCarries("sbc", "sbcxq")).toBe(false);
    expect(abrCarries("sbcxq", "sbc")).toBe(false);
    expect(abrCarries("sbc", "sbc")).toBe(true);
    const sbcAbr = { ...p, codec: "sbc", abr: true, sbc_bitpool: 40, sbcxq_bitpool: 0, sbc_blocks: 8, sbc_subbands: 4, sbc_allocation: "snr", sbc_channel_mode: "stereo" };
    expect(carryToCodec(sbcAbr, "sbcxq")).toMatchObject({ codec: "sbcxq", abr: false, sbc_bitpool: 0, sbc_blocks: 0, sbc_subbands: 0, sbc_allocation: "", sbc_channel_mode: "" });
    expect(carryToCodec(sbcAbr, "sbc")).toMatchObject({ abr: true, sbc_bitpool: 40, sbc_blocks: 8, sbc_subbands: 4, sbc_allocation: "snr", sbc_channel_mode: "stereo" });
    expect(carryToCodec({ ...p, codec: "ldac", abr: true }, "lhdcv5")).toMatchObject({ abr: true });
    expect(abrCarries("ldac", "sbc")).toBe(false);
    expect(abrCarries("sbcxq", "auto")).toBe(false);
    expect(abrCarries("aac", "sbc")).toBe(false);
    expect(abrCarries("aac", "ldac")).toBe(false);
    expect(fitToCodec({ ...p, codec: "ldac", sbc_bitpool: 40 })).toMatchObject({ sbc_bitpool: 0 });
    // SBC and SBC XQ keep their own bitpool
    expect(fitToCodec({ ...p, codec: "sbcxq", sbc_bitpool: 53, sbcxq_bitpool: 40 })).toMatchObject({ sbc_bitpool: 0, sbcxq_bitpool: 40 });
    // ... and their own minimum (issue #86)
    expect(fitToCodec({ ...p, codec: "sbcxq", sbc_min_bitpool: 20, sbcxq_min_bitpool: 30 })).toMatchObject({ sbc_min_bitpool: 0, sbcxq_min_bitpool: 30 });
    expect(fitToCodec({ ...p, codec: "aac", sbc_min_bitpool: 20 })).toMatchObject({ sbc_min_bitpool: 0 });
    // The frame setup is both's; the channel mode SBC's only
    const frame = { sbc_blocks: 8, sbc_subbands: 4, sbc_allocation: "snr", sbc_channel_mode: "stereo" };
    expect(fitToCodec({ ...p, ...frame, codec: "sbc" })).toMatchObject(frame);
    expect(fitToCodec({ ...p, ...frame, codec: "sbcxq" })).toMatchObject({ ...frame, sbc_channel_mode: "" });
    expect(fitToCodec({ ...p, ...frame, codec: "aac" })).toMatchObject({
      sbc_blocks: 0,
      sbc_subbands: 0,
      sbc_allocation: "",
      sbc_channel_mode: "",
    });
    expect(fitToCodec({ ...p, codec: "auto" })).toMatchObject({ abr: true, sample_rate: 0, bit_depth: 24 });
    expect(fitToCodec({ ...p, codec: "aptx" })).toMatchObject({ quality: "mhq", abr: false });
    expect(fitToCodec({ ...p, codec: "opus" })).toMatchObject({ quality: "hq", abr: false, sample_rate: 0, bit_depth: 0 });
    expect(fitToCodec({ ...p, codec: "opus", sample_rate: 48000 }).sample_rate).toBe(48000);
    expect(fitToCodec({ ...p, codec: "opus", sample_rate: 44100 }).sample_rate).toBe(0);
    expect(sampleRates("opus")).toEqual([0, 48000]);
    expect(fitToCodec({ ...p, codec: "opus05", sample_rate: 44100 })).toMatchObject({ quality: "hq", abr: false, sample_rate: 0, bit_depth: 0 });
    expect(sampleRates("opus05")).toEqual([0, 48000]);
    // aptX Adaptive: 48 / 96 kHz, ABR, and its low latency mode as a quality
    expect(fitToCodec({ ...p, codec: "aptxad", sample_rate: 44100 })).toMatchObject({ quality: "hq", abr: true, sample_rate: 0, bit_depth: 0 });
    expect(fitToCodec({ ...p, codec: "aptxad", quality: "ll" }).quality).toBe("ll");
    expect(fitToCodec({ ...p, codec: "ldac", quality: "ll" }).quality).toBe("hq");
    expect(sampleRates("aptxad")).toEqual([0, 48000, 96000]);
    expect(fitToCodec({ ...p, codec: "aptxad", sample_rate: 96000 }).sample_rate).toBe(96000);
    // Always 24-bit, no choice (aptX HD too); the others without a choice 16
    expect(bitDepths("aptxad")).toEqual([0]);
    expect([fixedBitDepth("aptxad"), fixedBitDepth("aptxhd"), fixedBitDepth("aptx"), fixedBitDepth("opus")]).toEqual([24, 24, 16, 16]);
  });

  it("offers 192 kHz for LHDC V5 only, and no quality under its ABR", () => {
    expect(sampleRates("lhdcv5")).toEqual([0, 44100, 48000, 88200, 96000, 192000]);
    expect(sampleRates("ldac")).not.toContain(192000);
    const p = { ...sampleProfile(), codec: "lhdcv5", sample_rate: 192000 };
    expect(fitToCodec(p).sample_rate).toBe(192000);
    expect(fitToCodec({ ...p, codec: "ldac" }).sample_rate).toBe(0);
    // LHDC V5 ABR stays at 400 kbps or below whatever the quality (as on Android)
    expect(qualityIgnored("lhdcv5", true)).toBe(true);
    expect(qualityIgnored("lhdcv5", false)).toBe(false);
    expect(qualityIgnored("ldac", true)).toBe(false);
  });

  it("offers High / Medium / Low for LDAC and LHDC V5 (and its Medium-High), High / LL for aptX Adaptive, High elsewhere (issue #86, #203)", () => {
    const ids = (codec: string) => CODECS.find((c) => c.id === codec)!.qualities.map((q) => q.id);
    expect(ids("ldac")).toEqual(["hq", "sq", "mq"]);
    expect(ids("lhdcv5")).toEqual(["hq", "mhq", "sq", "mq"]);
    expect(ids("aptxad")).toEqual(["hq", "ll"]);
    for (const c of ["auto", "aac", "opus", "opus05", "sbc", "sbcxq"]) expect(ids(c)).toEqual(["hq"]);
    for (const c of ["aptx", "aptxhd", "aptxll", "faststream"]) expect(ids(c)).toEqual([]);
  });

  it("offers only what the device lists (issue #86: moto buds+)", () => {
    const caps = [
      { codec: "lhdcv5", sample_rates: [44100, 48000], bit_depths: [16], max_bitrate: 0 },
      { codec: "aac", sample_rates: [44100, 48000], bit_depths: [], max_bitrate: 0x020000 },
    ];
    const lhdc = capsOf(caps, "LHDCV5");
    expect(offeredSampleRates("lhdcv5", lhdc)).toEqual([0, 44100, 48000]);
    expect(offeredBitDepths("lhdcv5", lhdc)).toEqual([0, 16]);
    // Not known: everything; 88.2 kHz runs as 96 kHz on LHDC V5
    expect(offeredSampleRates("lhdcv5", null)).toEqual(sampleRates("lhdcv5"));
    expect(offeredSampleRates("lhdcv5", { ...lhdc!, sample_rates: [48000, 96000] })).toEqual([0, 48000, 88200, 96000]);
    // AAC's High at the device's largest bit rate
    expect(qualityKbps("aac", "hq", 0, capsOf(caps, "aac")!.max_bitrate)).toBe("131");
    expect(qualityKbps("aac", "hq", 0, 320000)).toBe("256");
    // LHDC V5 at most 500 kbps: High is 500, Medium the same (left out)
    expect(offeredQualities("lhdcv5", 48000).map((q) => [q.id, q.kbps])).toEqual([
      ["hq", "1000"],
      ["mhq", "900"],
      ["sq", "500"],
      ["mq", "400"],
    ]);
    expect(offeredQualities("lhdcv5", 48000, 500000).map((q) => [q.id, q.kbps])).toEqual([
      ["hq", "500"],
      ["mq", "400"],
    ]);
    expect(offeredQualities("lhdcv5", 48000, 500000, "sq").map((q) => q.id)).toEqual(["hq", "sq", "mq"]);
    // aptX Adaptive's 96 kHz only when the device lists it (issue #86: a
    // Bose QC Ultra, 0x51, does not; an EarFun Air Pro 4+ does), also
    // while not known
    const aptxad = (sample_rates: number[]) => ({ codec: "aptxad", sample_rates, bit_depths: [], max_bitrate: 0 });
    expect(offeredSampleRates("aptxad", aptxad([48000]))).toEqual([0, 48000]);
    expect(offeredSampleRates("aptxad", aptxad([48000, 96000]))).toEqual([0, 48000, 96000]);
    expect(offeredSampleRates("aptxad", null)).toEqual([0, 48000]);
    expect(offeredSampleRates("aptxad", aptxad([]))).toEqual([0, 48000]);
  });

  it("says why the bit rate is below the profile's", () => {
    expect(bitrateNoteKey({ abr: true, bitrate_capped: false })).toBe("quality.codec_abr");
    expect(bitrateNoteKey({ abr: false, bitrate_capped: true })).toBe("quality.codec_capped");
    expect(bitrateNoteKey({ abr: true, bitrate_capped: true })).toBe("quality.codec_abr");
    expect(bitrateNoteKey({ abr: false, bitrate_capped: false })).toBeNull();
  });

  it("names codecs as the core reports them", () => {
    expect(codecLabelKey("aptX HD")).toBe("codec.aptx_hd");
    expect(codecLabelKey("LHDC V5")).toBe("codec.lhdc_v5");
    expect(codecLabelKey("Opus")).toBe("codec.opus");
    expect(codecLabelKey("Opus 05")).toBe("codec.opus05");
    expect(codecLabelKey("aptX Adaptive")).toBe("codec.aptx_adaptive");
    expect(codecLabelKey("SBC XQ")).toBe("codec.sbc_xq");
    expect(codecLabelKey("FastStream")).toBe("codec.faststream");
    expect(codecLabelKey("Ldac")).toBe("codec.ldac");
  });

  /** The number an address got (the map lives as long as the module) */
  const addrOf = (addr: string) => /<addr-(\d+)>/.exec(maskIdentifiers(addr))?.[1];

  it("masks addresses as <addr-N>, the same address the same N in every form", () => {
    const m = maskIdentifiers;
    const n = addrOf("AC:80:0A:12:34:56");
    expect(n).toBeDefined();
    expect(m("connecting to AC:80:0A:12:34:56 now")).toBe(`connecting to <addr-${n}> now`);
    expect(m("ac-80-0a-12-34-56 and AC800A123456")).toBe(`<addr-${n}> and <addr-${n}>`);
    expect(addrOf("00:1A:7D:DA:71:13")).not.toBe(n);
  });

  // The same cases as the core's mask_identifiers tests
  it("masks addresses in other forms", () => {
    const m = maskIdentifiers;
    const a = addrOf("00-1A-7D-DA-71-13");
    const b = addrOf("46:62:99:4A:5B:6C");
    const c = addrOf("58:18:62:3a:1b:2c");
    expect(m("00-1A-7D-DA-71-13")).toBe(`<addr-${a}>`);
    // A colon after it ends the address (the LE Audio pairing lines of #81)
    expect(m("LE Audio: 46:62:99:4A:5B:6C: pairing (Just Works)")).toBe(`LE Audio: <addr-${b}>: pairing (Just Works)`);
    expect(m("to 46:62:99:4A:5B:6C:")).toBe(`to <addr-${b}>:`);
    expect(m("46-62-99-4A-5B-6C-x")).toBe(`<addr-${b}>-x`);
    expect(m("BluetoothLE#BluetoothLE00:1a:7d:da:71:13-58:18:62:3a:1b:2c")).toBe(`BluetoothLE#BluetoothLE<addr-${a}>-<addr-${c}>`);
    expect(m("fe80::00:11:22:33:44:55")).toBe("fe80::00:11:22:33:44:55");
    expect(m("00:11:22:33:44:55:66")).toBe("00:11:22:33:44:55:66");
    expect(m("00:11:22:33:44:556")).toBe("00:11:22:33:44:556");
    expect(m("ヘッドホン5818623A1B2C接続")).toBe(`ヘッドホン<addr-${c}>接続`);
    expect(m(String.raw`BTHENUM\{0000110b-0000-1000-8000-00805f9b34fb}_LOCALMFG&005d\7&1a2b3c4d&0&5818623A1B2C_C00000000`)).toBe(
      String.raw`BTHENUM\{0000110b-0000-1000-8000-00805f9b34fb}_LOCALMFG&005d\7&1a2b3c4d&0&<addr-${c}>_C00000000`,
    );
    expect(m("5818623A1B2C5818623A1B2C, 0x5818623A1B2C, 5818623A1B2G")).toBe(
      "5818623A1B2C5818623A1B2C, 0x5818623A1B2C, 5818623A1B2G",
    );
  });

  it("shows the adapters' own addresses as <adapter>", () => {
    knowAdapters(["00:e0:4c:00:00:01", null, "nope"]);
    expect(maskIdentifiers(String.raw`local 00:E0:4C:00:00:01, USB\VID_0BDA&PID_4853\00E04C000001`)).toBe(
      String.raw`local <adapter>, USB\VID_0BDA&PID_4853\<adapter>`,
    );
  });

  it("masks USB serial numbers", () => {
    const m = maskIdentifiers;
    const n = addrOf("00E04C000002");
    expect(m(String.raw`opened 0BDA:4853 USB\VID_0BDA&PID_4853\00E04C000002 (event 0x81)`)).toBe(
      String.raw`opened 0BDA:4853 USB\VID_0BDA&PID_4853\<addr-${n}> (event 0x81)`,
    );
    expect(m(String.raw`USB\VID_18D1&PID_4EE7\HT7A1B234567`)).toBe(String.raw`USB\VID_18D1&PID_4EE7\xxxxxxxxxxxx`);
    expect(m(String.raw`\\?\usb#vid_0a12&pid_0001#serial42#{a5dcbf10-6530-11d2-901f-00c04fb951ed}`)).toBe(
      String.raw`\\?\usb#vid_0a12&pid_0001#xxxxxxxx#{a5dcbf10-6530-11d2-901f-00c04fb951ed}`,
    );
    expect(m(String.raw`USB\VID_8087&PID_0026&MI_00\SN123`)).toBe(String.raw`USB\VID_8087&PID_0026&MI_00\xxxxx`);
    expect(m(String.raw`USB\VID_8087&PID_0026\5&2C7B4E3&0&14.`)).toBe(String.raw`USB\VID_8087&PID_0026\5&2C7B4E3&0&14.`);
    expect(m("USB_VID_0BDA_PID_8771_SN123.json")).toBe("USB_VID_0BDA_PID_8771_xxxxx.json");
    expect(m("RTL8852BU (0BDA:4853 -> 0xB85B), VID_0BDA&PID_4853.")).toBe("RTL8852BU (0BDA:4853 -> 0xB85B), VID_0BDA&PID_4853.");
  });

  it("masks a path above the app's own folder, or all but its last part", () => {
    const m = maskIdentifiers;
    expect(m(String.raw`settings folder: C:\Users\Taro Yamada\AppData\Roaming\BlueCaster`)).toBe(String.raw`settings folder: <path>\BlueCaster`);
    expect(m(String.raw`dump: C:\Users\admin\AppData\Roaming\BlueCaster\logs\crash\bluecaster-core-20261022-235220-831.dmp`)).toBe(
      String.raw`dump: <path>\BlueCaster\logs\crash\bluecaster-core-20261022-235220-831.dmp`,
    );
    expect(m(String.raw`C:\Program Files\BlueCaster\bluecaster-ui.exe`)).toBe(String.raw`<path>\BlueCaster\bluecaster-ui.exe`);
    expect(m(String.raw`starting: pid 1, I:\A2DP-Windows-Bridge\target\release\bluecaster-core.exe`)).toBe(
      String.raw`starting: pid 1, <path>\bluecaster-core.exe`,
    );
    expect(m(String.raw`C:\Users\山田太郎\Music\foobar2000\foobar2000.exe`)).toBe(String.raw`<path>\foobar2000.exe`);
    // The last part takes the text after it, but not another path; a trailing separator keeps the folder
    expect(m(String.raw`HCI capture running: C:\x\BlueCaster\hci.pklg (3.0 MB)`)).toBe(String.raw`HCI capture running: <path>\BlueCaster\hci.pklg (3.0 MB)`);
    expect(m(String.raw`moved C:\a\b\x.bin to C:\a\c\x.bin`)).toBe(String.raw`moved <path>\x.bin to <path>\x.bin`);
    expect(m("folder C:\\a\\b\\\n")).toBe("folder <path>\\b\\\n");
    expect(m(String.raw`C:\ only`)).toBe(String.raw`<path>\ only`);
    // JSON's doubled '\', '/', a device path, a user's folder at the end (its name as x's)
    expect(m(String.raw`{"path":"C:\\Users\\taro\\x.txt"}`)).toBe(String.raw`{"path":"<path>\\x.txt"}`);
    expect(m("file:///C:/users/taro/x.txt")).toBe("file:///<path>/x.txt");
    expect(m(String.raw`\\?\C:\USERS\TARO\x`)).toBe(String.raw`\\?\<path>\x`);
    expect(m("dump: C:\\Users\\taro\n")).toBe("dump: <path>\\xxxx\n");
    // Not paths
    expect(m("at 23:52:25 LE Audio: 46 kHz, x:y, C: drive, https://a.b/c")).toBe("at 23:52:25 LE Audio: 46 kHz, x:y, C: drive, https://a.b/c");
    expect(m(String.raw`\\.\pipe\bluecaster-x`)).toBe(String.raw`\\.\pipe\bluecaster-x`);
  });

  it("masks the Windows user name in a path without a drive", () => {
    const m = maskIdentifiers;
    expect(m(String.raw`\Users\Taro Yamada\AppData\Roaming\BlueCaster`)).toBe(String.raw`\Users\xxxxxxxxxxx\AppData\Roaming\BlueCaster`);
    expect(m(String.raw`\Users\山田太郎\AppData`)).toBe(String.raw`\Users\xxxx\AppData`);
    expect(m("/users/taro/x.txt")).toBe("/users/xxxx/x.txt");
    expect(m(String.raw`\Users\Public\Desktop and \Users\All Users\y`)).toBe(String.raw`\Users\Public\Desktop and \Users\All Users\y`);
    expect(m("3 Users, \\Users\\\n")).toBe("3 Users, \\Users\\\n");
    expect(m("in Users\\taro")).toBe("in Users\\taro");
  });

  it("masks account SIDs", () => {
    const m = maskIdentifiers;
    expect(m(String.raw`IPC: listening on \\.\pipe\bluecaster-S-1-5-21-2123068572-4202720387-3275098450-1001`)).toBe(
      String.raw`IPC: listening on \\.\pipe\bluecaster-<sid>`,
    );
    expect(m("user S-1-5-21-1-2-3-500, then")).toBe("user <sid>, then");
    // Well-known SIDs, the start of one, a longer word
    expect(m("S-1-5-18 S-1-5-32-544 S-1-5-21- S-1-")).toBe("S-1-5-18 S-1-5-32-544 S-1-5-21- S-1-");
    expect(m("xS-1-5-21-1-2-3-4 S-1-5-21-1-2-3-4x bluecaster-S-1-5-21-1-2-3-4-dev")).toBe("xS-1-5-21-1-2-3-4 S-1-5-21-1-2-3-4x bluecaster-<sid>-dev");
  });

  it("masks quoted device names, the same name the same number", () => {
    const m = maskIdentifiers;
    const first = m("stream: device AA:BB:CC:DD:EE:FF source virtual device “Taro's AirPods”");
    const n = /<device-(\d+)>$/.exec(first)?.[1];
    expect(n).toBeDefined();
    const a = addrOf("AA:BB:CC:DD:EE:FF");
    expect(first).toBe(`stream: device <addr-${a}> source virtual device <device-${n}>`);
    expect(m("remote name of AA:BB:CC:DD:EE:FF: name=“Taro's AirPods”")).toBe(`remote name of <addr-${a}>: name=<device-${n}>`);
    expect(m("name=“Hanako's iPhone”")).not.toBe(`name=<device-${n}>`);
    expect(m("name=“Hanako's iPhone”")).toMatch(/^name=<device-\d+>$/);
    // Empty, unclosed and multi-line quotes stay
    expect(m("name=“” and “open")).toBe("name=“” and “open");
    expect(m("“a\nb”")).toBe("“a\nb”");
  });
});

describe("about", () => {
  it("links the third-party licences of the release tag", () => {
    expect(thirdPartyUrl("2.0.0-alpha.1")).toBe(
      "https://github.com/SeiyaFunaokaJP/BlueCaster/blob/v2.0.0-alpha.1/THIRD_PARTY_LICENSES.md",
    );
  });
});

describe("bug report", () => {
  const info = "```text\nBlueCaster diagnostics - 2026-10-22\nVersion      : 2.0.3\nOS           : Windows build 26100 (x86_64)\n```\n";
  const base = { version: "2.0.3", info, bundle: "bluecaster-diag-20261022_120000.zip" };

  it("fills the issue form's fields the app knows", () => {
    const url = new URL(
      bugReportUrl({ ...base, adapter: "RTL8761BU (2357:0604)", headphones: "WH-1000XM5", codec: "ldac", captureMode: "app" }),
    );
    const q = url.searchParams;
    expect(url.origin + url.pathname).toBe("https://github.com/SeiyaFunaokaJP/BlueCaster/issues/new");
    expect(q.get("template")).toBe("bug_report.yml");
    expect(q.get("version")).toBe("2.0.3");
    expect(q.get("windows")).toBe("Windows build 26100 (x86_64)");
    expect(q.get("adapter")).toBe("RTL8761BU (2357:0604)");
    expect(q.get("headphones")).toBe("WH-1000XM5");
    // The dropdowns take their option's text
    expect(q.get("codec")).toBe("LDAC");
    expect(q.get("capture_mode")).toBe("Application");
    expect(q.get("interface")).toBe("Window / tray");
    expect(q.get("bundle")).toMatch(/^bluecaster-diag-20261022_120000\.zip\n/);
    // The log field is a code block already: no fence of its own
    expect(q.get("logs")).toBe("BlueCaster diagnostics - 2026-10-22\nVersion      : 2.0.3\nOS           : Windows build 26100 (x86_64)\n");
  });

  it("leaves out what it does not know and cuts a long log", () => {
    const long = info + "  T+1.000 INFO bt_host: connecting to AA:BB:CC:xx:xx:xx\n".repeat(400);
    const url = bugReportUrl({ ...base, info: long, codec: "opus05" });
    const q = new URL(url).searchParams;
    expect(q.has("codec")).toBe(false);
    expect(q.has("adapter")).toBe(false);
    expect(url.length).toBeLessThan(8000);
    expect(q.get("logs")).toMatch(/\n\.\.\. \(cut here: the full text is diagnostics\.txt in the bundle\)$/);
    expect(q.get("logs")).toMatch(/^BlueCaster diagnostics/);
  });
});

describe("profile list", () => {
  it("shows the quality and its bit rate as v1 did", () => {
    const tag = (over: Parameters<typeof sampleProfile>[0]) => qualityTag(sampleProfile(over), t);
    expect(tag({ codec: "ldac", quality: "hq" })).toBe("High (990 kbps)");
    expect(tag({ codec: "ldac", quality: "sq", sample_rate: 44100 })).toBe("Medium (606 kbps)");
    expect(tag({ codec: "ldac", quality: "mq", sample_rate: 88200 })).toBe("Low (303 kbps)");
    expect(tag({ codec: "lhdcv5", quality: "sq", sample_rate: 44100 })).toBe("Medium (480 kbps)");
    expect(tag({ codec: "lhdcv5", quality: "sq", sample_rate: 88200 })).toBe("Medium (500 kbps)");
    // LHDC V5's Medium-High is back (issue #203); a quality a codec does
    // not offer is High
    expect(tag({ codec: "lhdcv5", quality: "mhq" })).toBe("Medium-High (900 kbps)");
    expect(tag({ codec: "ldac", quality: "mhq" })).toBe("High (990 kbps)");
    expect(tag({ codec: "aptxad", quality: "ll" })).toBe("Low Latency (265 kbps)");
    expect(tag({ codec: "aptxad", quality: "hq", sample_rate: 96000 })).toBe("High (646 kbps)");
    expect(tag({ codec: "aptxad", quality: "mq", sample_rate: 96000 })).toBe("High (646 kbps)");
    expect(tag({ codec: "aptxad", quality: "ll", sample_rate: 96000 })).toBe("Low Latency (265 kbps)");
    expect(tag({ codec: "lhdcv5", quality: "hq", abr: true })).toBe("ABR (≤400 kbps)");
    expect(tag({ codec: "sbc", quality: "sq" })).toBe("High (~345 kbps)");
    expect(tag({ codec: "aac", quality: "mq" })).toBe("High (256 kbps)");
    expect(tag({ codec: "auto", quality: "hq" })).toBe("High");
    // No quality choice
    expect(tag({ codec: "aptxhd" })).toBeNull();
    expect(tag({ codec: "faststream" })).toBeNull();
    expect(tag({ codec: "auto", le_audio: true })).toBeNull();
  });
});

describe("Auto's order and the fallback (issue #86)", () => {
  it("goes by sound quality, as the core", () => {
    expect(AUTO_ORDER).toEqual(["lhdcv5", "ldac", "aptxad", "opus", "aptxhd", "aptx", "aptxll", "aac", "sbcxq", "sbc"]);
    expect(autoPick(["sbc", "aac", "aptx", "aptxad"])).toBe("aptxad");
    expect(autoPick(["sbc", "aac", "ldac", "lhdcv5"])).toBe("lhdcv5");
    expect(autoPick(["sbc", "faststream", "opus05"])).toBe("sbc");
    expect(autoPick([])).toBeNull();
  });

  it("falls back to the best codec the device offers (issue #86 round 8)", () => {
    const earfun = ["sbc", "aac", "aptx", "aptxad"];
    // The tester's EarFun: Opus chosen gives aptX Adaptive, not aptX below Opus
    expect(effectiveCodec("opus", earfun)).toBe("aptxad");
    expect(effectiveCodec("lhdcv5", earfun)).toBe("aptxad");
    expect(effectiveCodec("aptxhd", earfun)).toBe("aptxad");
    expect(effectiveCodec("aptxhd", ["sbc", "aac", "ldac"])).toBe("ldac");
    expect(effectiveCodec("sbcxq", ["sbc", "aac"])).toBe("aac");
    // Offered: itself; Auto: its pick
    expect(effectiveCodec("aac", earfun)).toBe("aac");
    expect(effectiveCodec("auto", earfun)).toBe("aptxad");
    // Opus 05 and FastStream are never a fallback
    expect(effectiveCodec("opus05", ["sbc", "opus"])).toBe("opus");
    expect(effectiveCodec("faststream", ["sbc", "aac"])).toBe("aac");
    expect(effectiveCodec("ldac", ["sbc", "faststream", "opus05"])).toBe("sbc");
    // Not known yet
    expect(effectiveCodec("ldac", [])).toBeNull();
  });

  it("leaves the experimental codecs out while they are off (issue #86)", () => {
    expect(CODECS.filter((c) => c.experimental).map((c) => c.id).sort()).toEqual(
      ["aptxhd", "aptxll", "faststream", "opus", "opus05"],
    );
    expect(isExperimental("aptxad")).toBe(false);
    const pw = ["sbc", "aac", "opus", "aptxhd"];
    expect(autoPick(pw)).toBe("opus");
    expect(autoPick(pw, false)).toBe("aac");
    // Chosen while off: the best one that is not experimental
    expect(effectiveCodec("opus", pw, false)).toBe("aac");
    expect(effectiveCodec("aptxhd", pw, false)).toBe("aac");
    // Nor a fallback for another codec
    expect(effectiveCodec("ldac", pw)).toBe("opus");
    expect(effectiveCodec("ldac", pw, false)).toBe("aac");
    expect(effectiveCodec("auto", ["sbc", "faststream"], false)).toBe("sbc");
  });
});

describe("A device's codec order (issue #86 round 8)", () => {
  it("keeps every codec of AUTO_ORDER once, the ones given first", () => {
    expect(autoOrder([])).toEqual([...AUTO_ORDER]);
    const o = autoOrder(["AAC", "nope", "ldac", "aac", "faststream"]);
    expect(o.slice(0, 3)).toEqual(["aac", "ldac", "lhdcv5"]);
    expect(o).toHaveLength(AUTO_ORDER.length);
  });

  it("puts every card once, the ones given first", () => {
    expect(cardOrder([])).toEqual([...CARD_ORDER]);
    expect(CARD_ORDER.filter((c) => AUTO_ORDER.includes(c))).toEqual([...AUTO_ORDER]);
    const o = cardOrder(["SBCXQ", "nope", "sbc", "sbcxq", "faststream"]);
    expect(o.slice(0, 4)).toEqual(["sbcxq", "sbc", "faststream", "lhdcv5"]);
    expect([...o].sort()).toEqual([...CARD_ORDER].sort());
    expect(isDefaultOrder([])).toBe(true);
    expect(isDefaultOrder([...CARD_ORDER])).toBe(true);
    expect(isDefaultOrder(o)).toBe(false);
    expect(moved(["a", "b", "c"], 2, 0)).toEqual(["c", "a", "b"]);
    expect(moved(["a", "b", "c"], 0, 1)).toEqual(["b", "a", "c"]);
  });

  it("is what Auto and a fallback follow", () => {
    const known = ["sbc", "aac", "ldac"];
    const aacFirst = cardOrder(["aac"]);
    expect(effectiveCodec("auto", known)).toBe("ldac");
    expect(effectiveCodec("auto", known, true, aacFirst)).toBe("aac");
    // aptX HD not offered: the first card the device offers
    expect(effectiveCodec("aptxhd", known, true, cardOrder(["aptxhd", "sbc", "ldac"]))).toBe("sbc");
    // The tester's order, SBC XQ first: SBC XQ for an Opus it does not offer
    const earfun = ["sbc", "sbcxq", "aac", "aptx", "aptxad"];
    expect(effectiveCodec("opus", earfun, true, cardOrder(["sbcxq", "sbc", "aptxad", "aac", "aptx"]))).toBe("sbcxq");
    // FastStream first: never Auto's pick nor a fallback
    expect(effectiveCodec("ldac", ["sbc", "faststream"], true, cardOrder(["faststream"]))).toBe("sbc");
  });
});
