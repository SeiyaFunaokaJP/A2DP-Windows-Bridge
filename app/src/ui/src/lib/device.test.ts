// The profile editor's device, codec and capture helpers (lib/profile.ts,
// lib/codecs.ts, lib/format.ts)

import { describe, expect, it } from "vitest";
import type { AudioDevice, Device } from "../ipc/types";
import { codecLabelKey, codecSupport } from "./codecs";
import { asciiFromFullwidth, parseAddress } from "./format";
import {
  appCaptureSupportFrom,
  captureDevice,
  defaultProfileName,
  deviceKey,
  entryBase,
  formatMismatch,
  knownCodecs,
  listedDevices,
  newProfile,
  validateProfile,
  validDeviceName,
} from "./profile";

const dev = (address: string, codecs: string[]): Device => ({
  address,
  name: "",
  saved: true,
  audio: true,
  rssi: null,
  codecs,
  sbc: null,
  codec_caps: [],
  codecs_on_request: [],
  le_audio: false,
  le_audio_set: false,
});

const audio = (id: string, is_default: boolean, mix_rate: number | null, mix_bits: number | null): AudioDevice => ({
  id,
  name: id,
  is_default,
  is_virtual: !is_default,
  mix_rate,
  mix_channels: 2,
  mix_bits,
});

describe("addresses", () => {
  it("accepts full-width input from an IME, as v1", () => {
    expect(asciiFromFullwidth("ＡＣ：８０")).toBe("AC:80");
    expect(parseAddress("ＡＣ：８０：０Ａ：１２：３４：５６")).toBe("AC:80:0A:12:34:56");
    expect(parseAddress("ａｃ－８０－０ａ－１２－３４－５６")).toBe("AC:80:0A:12:34:56");
    // Ideographic space around, the long vowel mark a Japanese IME types for "-"
    expect(parseAddress("　acー80ー0a-12-34-56　")).toBe("AC:80:0A:12:34:56");
    expect(parseAddress("ＡＣ８００Ａ１２３４５６")).toBe("AC:80:0A:12:34:56");
  });

  it("accepts the separators v1 did and still rejects the rest", () => {
    expect(parseAddress("ac.80.0a.12.34.56")).toBe("AC:80:0A:12:34:56");
    expect(parseAddress("ＡＣ：８０：０Ａ：１２：３４")).toBeNull();
    expect(parseAddress("ＧＧ：８０：０Ａ：１２：３４：５６")).toBeNull();
    expect(parseAddress("AC:80:0A:12:34:56:78")).toBeNull();
  });
});

describe("codecs a device offers", () => {
  it("is unknown until the device connected once", () => {
    expect(codecSupport("ldac", [])).toBe("unknown");
    expect(codecSupport("auto", [])).toBe("unknown");
  });

  it("tells offered from not offered", () => {
    const known = ["sbc", "aac", "ldac"];
    expect(codecSupport("ldac", known)).toBe("offered");
    expect(codecSupport("aptxhd", known)).toBe("not_offered");
    expect(codecSupport("auto", known)).toBe("offered");
    expect(codecSupport("LDAC", ["ldac"])).toBe("offered");
  });

  it("does not call one it may offer once chosen not offered", () => {
    expect(codecSupport("ldac", ["sbc", "aac"], ["ldac"])).toBe("unknown");
    expect(codecSupport("aptx", ["sbc", "aac"], ["ldac"])).toBe("not_offered");
    expect(codecSupport("ldac", ["sbc", "ldac"], ["ldac"])).toBe("offered");
  });

  it("finds the profile's device by its address as typed", () => {
    const devices = [dev("AC:80:0A:12:34:56", ["sbc", "ldac"]), dev("00:1A:7D:DA:71:13", [])];
    expect(knownCodecs(devices, "ac-80-0a-12-34-56")).toEqual(["sbc", "ldac"]);
    expect(knownCodecs(devices, "00:1A:7D:DA:71:13")).toEqual([]);
    expect(knownCodecs(devices, "11:22:33:44:55:66")).toEqual([]);
    expect(knownCodecs(devices, "")).toEqual([]);
  });
});

describe("LE Audio entries", () => {
  const le = (address: string, set = false): Device => ({ ...dev(address, []), le_audio: true, le_audio_set: set });

  it("name an entry by address and kind: a dual-mode device has two", () => {
    const list = [dev("F4:4E:FD:00:11:22", ["sbc", "aac"]), le("F4:4E:FD:00:11:22", true), le("D0:3E:7D:4C:2A:10")];
    expect(list.map(deviceKey)).toEqual(["F4:4E:FD:00:11:22", "F4:4E:FD:00:11:22/le", "D0:3E:7D:4C:2A:10/le"]);
    // The codecs belong to the classic entry
    expect(knownCodecs([le("F4:4E:FD:00:11:22"), ...list], "F4:4E:FD:00:11:22")).toEqual(["sbc", "aac"]);
  });

  it("are listed only with LE Audio (experimental) on, as v1", () => {
    const list = [dev("AC:80:0A:12:34:56", []), le("F4:4E:FD:00:11:22")];
    expect(listedDevices(list, false).map(deviceKey)).toEqual(["AC:80:0A:12:34:56"]);
    expect(listedDevices(list, true)).toHaveLength(2);
  });

  it("name a new profile \"<device> LE Audio\" (v1)", () => {
    expect(defaultProfileName("Galaxy Buds2 Pro", "F4:4E:FD:00:11:22", true)).toBe("Galaxy Buds2 Pro LE Audio");
    expect(defaultProfileName("", "F4:4E:FD:00:11:22", true)).toBe("F4:4E:FD:00:11:22 LE Audio");
    expect(defaultProfileName("Galaxy Buds2 Pro", "F4:4E:FD:00:11:22", false)).toBe("Galaxy Buds2 Pro");
    expect(defaultProfileName("EarFun Air Pro 4+", "F4:4E:FD:00:11:22", false, "Auto")).toBe("EarFun Air Pro 4+ | Auto");
    expect(defaultProfileName("", "", false, "Auto")).toBe("");
    expect(defaultProfileName("", "", true)).toBe("");
  });

  it("refuse application capture", () => {
    const p = { ...newProfile(), name: "P", device_address: "F4:4E:FD:00:11:22", capture_mode: "app", app_exe: "x.exe" };
    expect(validateProfile(p, []).app).toBeUndefined();
    expect(validateProfile({ ...p, le_audio: true }, []).app).toEqual({ key: "error.le_audio_capture" });
    expect(validateProfile({ ...p, le_audio: true, capture_mode: "loopback" }, [])).toEqual({});
  });

  it("stream LC3", () => {
    expect(codecLabelKey("LC3")).toBe("ui.codec.lc3");
  });
});

describe("capture device format", () => {
  const list = [audio("speakers", true, 48000, 24), audio("cable", false, 44100, 16)];

  it("takes the default device for loopback, the chosen one for a virtual device", () => {
    expect(captureDevice({ capture_mode: "loopback", audio_device_id: "cable" }, list)?.id).toBe("speakers");
    expect(captureDevice({ capture_mode: "virtual", audio_device_id: "cable" }, list)?.id).toBe("cable");
    expect(captureDevice({ capture_mode: "virtual", audio_device_id: "" }, list)).toBeNull();
    // Application capture: Windows converts to the rate asked for
    expect(captureDevice({ capture_mode: "app", audio_device_id: "" }, list)).toBeNull();
  });

  it("warns when the profile's rate or bit depth differs (Auto never does)", () => {
    const d = list[0];
    expect(formatMismatch({ sample_rate: 0, bit_depth: 0 }, d)).toEqual({ rate: false, bits: false });
    expect(formatMismatch({ sample_rate: 48000, bit_depth: 24 }, d)).toEqual({ rate: false, bits: false });
    expect(formatMismatch({ sample_rate: 96000, bit_depth: 16 }, d)).toEqual({ rate: true, bits: true });
    expect(formatMismatch({ sample_rate: 44100, bit_depth: 0 }, d)).toEqual({ rate: true, bits: false });
    // Not known: nothing to compare
    expect(formatMismatch({ sample_rate: 96000, bit_depth: 24 }, { mix_rate: null, mix_bits: null })).toEqual({
      rate: false,
      bits: false,
    });
  });
});

describe("profile defaults and names", () => {
  it("follows the virtual device by default, as v1 and the core", () => {
    expect(newProfile().auto_switch_device).toBe(true);
  });

  it("captures a new device by System Loopback, a virtual device installed or not (issue #86)", () => {
    expect(newProfile()).toMatchObject({ capture_mode: "loopback", audio_device_id: "", audio_device_name: "" });
    // The other entry's capture source counts first
    const other = { ...newProfile(), device_address: "AA:BB:CC:DD:EE:FF", capture_mode: "app", app_exe: "x.exe" };
    expect(entryBase({ address: "aa:bb:cc:dd:ee:ff", name: "X", le_audio: true }, [other])).toMatchObject({
      capture_mode: "app",
    });
    expect(entryBase({ address: "11:22:33:44:55:66", name: "Y", le_audio: false }, [other])).toMatchObject({
      capture_mode: "loopback",
      audio_device_id: "",
    });
  });

  it("checks a device name as the core does", () => {
    expect(validDeviceName("WH-1000XM5")).toBe(true);
    expect(validDeviceName("  ")).toBe(false);
    expect(validDeviceName("a|b")).toBe(false);
    expect(validDeviceName("a\nb")).toBe(false);
  });
});

describe("application capture support", () => {
  it("reads the Windows version from the client hints", () => {
    // Windows 10 1909, 2004 and later, Windows 11
    expect(appCaptureSupportFrom("Windows", "8.0.0")).toBe("no");
    expect(appCaptureSupportFrom("Windows", "10.0.0")).toBe("yes");
    expect(appCaptureSupportFrom("Windows", "15.0.0")).toBe("yes");
  });

  it("does not guess without them", () => {
    expect(appCaptureSupportFrom(undefined, undefined)).toBe("unknown");
    expect(appCaptureSupportFrom("macOS", "14.0.0")).toBe("unknown");
    expect(appCaptureSupportFrom("Windows", "")).toBe("unknown");
  });
});
