import { describe, expect, it } from "vitest";
import {
  chipTargetIndex,
  type AdapterFirmware,
  adapterFirmwareTag,
  buildChipOptions,
  CHIP_AUTO,
  CHIP_CUSTOM,
  CHIP_NOT_REALTEK,
  chipStatus,
  firmwareStem,
  REALTEK_CHIPS,
} from "./chips";
import { adapterWithFirmware, sampleSnapshot } from "../ipc/mock";
import { firmwareUrl, LINKS, vendorFirmwareUrl } from "./links";

const labels = { auto: "Auto (RTL8761BU)", nonRealtek: "Not Realtek" };
const setting = (pid: number, stem = "") => ({ bt_chip_pid: pid, bt_chip_fw_stem: stem });
const file = (stem: string, pid: number | null = null, has_config = true) => ({
  stem,
  display_name: stem.toUpperCase(),
  pid,
  has_config,
});

describe("chip choice", () => {
  it("lists Auto, Not Realtek, the table, then Custom, as v1", () => {
    const { options, selected } = buildChipOptions(setting(CHIP_AUTO), [], labels);
    expect(options.map((o) => o.label)).toEqual([
      "Auto (RTL8761BU)",
      "Not Realtek",
      ...REALTEK_CHIPS.map((c) => `${c.name} (${c.stem}_fw.bin)`),
      "Custom (custom_fw.bin, custom_config.bin)",
    ]);
    expect(selected).toBe(options[0].key);
    expect(new Set(options.map((o) => o.key)).size).toBe(options.length);
  });

  it("adds chips known only by their firmware files, before Custom", () => {
    const files = [file("rtl8761bu", 0x8771), file("rtl8723bu"), file("RTL8723DU"), file("custom"), file("rtl8703bu", null, false)];
    const { options } = buildChipOptions(setting(CHIP_AUTO), files, labels);
    const extra = options.slice(2 + REALTEK_CHIPS.length);
    // Table chips (by PID or by stem) and custom are not listed twice
    expect(extra.map((o) => [o.pid, o.stem, o.label])).toEqual([
      [CHIP_AUTO, "rtl8723bu", "RTL8723BU (rtl8723bu_fw.bin)"],
      [CHIP_AUTO, "rtl8703bu", "RTL8703BU (rtl8703bu_fw.bin)"],
      [CHIP_CUSTOM, "custom", "Custom (custom_fw.bin, custom_config.bin)"],
    ]);
  });

  it("selects the entry the setting stands for", () => {
    const files = [file("rtl8723bu")];
    const pick = (pid: number, stem = "") => {
      const r = buildChipOptions(setting(pid, stem), files, labels);
      return r.options.find((o) => o.key === r.selected)!;
    };
    expect(pick(CHIP_NOT_REALTEK).label).toBe("Not Realtek");
    // A table PID matches by itself, whatever the stem
    expect(pick(0x8771, "rtl8761bu").label).toBe("RTL8761BU (rtl8761bu_fw.bin)");
    expect(pick(0x8771, "").label).toBe("RTL8761BU (rtl8761bu_fw.bin)");
    // PID 0 with a stem: the file-only chip (file names ignore case)
    expect(pick(CHIP_AUTO, "RTL8723BU")).toMatchObject({ pid: CHIP_AUTO, stem: "rtl8723bu" });
    expect(pick(CHIP_CUSTOM, "custom").stem).toBe("custom");
  });

  it("keeps a setting no entry stands for choosable", () => {
    const gone = buildChipOptions(setting(CHIP_AUTO, "rtl8723bu"), [], labels);
    const last = gone.options[gone.options.length - 1];
    expect(gone.selected).toBe(last.key);
    expect(last).toMatchObject({ pid: CHIP_AUTO, stem: "rtl8723bu", label: "0x0000 rtl8723bu" });

    const dropped = buildChipOptions(setting(0x1234, "old"), [], labels);
    expect(dropped.options[dropped.options.length - 1]).toMatchObject({ pid: 0x1234, stem: "old", label: "0x1234 old" });
    expect(dropped.selected).toBe(dropped.options[dropped.options.length - 1].key);
  });
});

describe("firmware state", () => {
  const rtl = (present: boolean | null): AdapterFirmware => ({
    vendor: "Realtek",
    realtek_chip: "RTL8761BU",
    firmware_present: present,
    firmware_files: present === null ? [] : ["rtl8761bu_fw.bin", "rtl8761bu_config.bin"],
    vendor_note: null,
  });
  const other = (vendor: string, key: string | null): AdapterFirmware => ({
    vendor,
    realtek_chip: null,
    firmware_present: null,
    firmware_files: [],
    vendor_note: key ? { key, args: [] } : null,
  });

  it("names the files of a chip setting", () => {
    expect(firmwareStem(0x8771, "")).toBe("rtl8761bu");
    expect(firmwareStem(CHIP_CUSTOM, "custom")).toBe("custom");
    expect(firmwareStem(CHIP_AUTO, "rtl8723bu")).toBe("rtl8723bu");
    expect(firmwareStem(CHIP_AUTO, "")).toBeNull();
    expect(firmwareStem(CHIP_NOT_REALTEK, "")).toBeNull();
    expect(firmwareStem(0x1234, "")).toBeNull();
  });

  it("takes present / missing from the adapters the core checked", () => {
    expect(chipStatus(setting(CHIP_AUTO), [rtl(true)], [])).toEqual({
      kind: "present",
      files: ["rtl8761bu_fw.bin", "rtl8761bu_config.bin"],
    });
    expect(chipStatus(setting(0x8771, "rtl8761bu"), [other("Intel", "firmware.vendor_intel"), rtl(false)], []).kind).toBe(
      "missing",
    );
    // The core can fetch the files: the Download button
    expect(chipStatus(setting(CHIP_AUTO), [{ ...rtl(false), firmware_download: true }], [])).toEqual({
      kind: "missing",
      files: ["rtl8761bu_fw.bin", "rtl8761bu_config.bin"],
      download: true,
    });
  });

  it("tells about another vendor where v1 did, instead of 'no files needed'", () => {
    const intel = other("Intel", "firmware.vendor_intel");
    expect(chipStatus(setting(CHIP_AUTO), [intel], [])).toEqual({
      kind: "vendor",
      note: { key: "firmware.vendor_intel", args: [] },
      vendor: "Intel",
    });
    expect(chipStatus(setting(CHIP_NOT_REALTEK), [other("Broadcom", "firmware.vendor_broadcom")], []).kind).toBe("vendor");
    // With a Realtek adapter there, Not Realtek is just that
    expect(chipStatus(setting(CHIP_NOT_REALTEK), [rtl(null), intel], []).kind).toBe("non_realtek");
    expect(chipStatus(setting(CHIP_NOT_REALTEK), [], []).kind).toBe("non_realtek");
  });

  it("asks for the chip when Auto finds none", () => {
    expect(chipStatus(setting(CHIP_AUTO), [], []).kind).toBe("auto_no_adapter");
    expect(chipStatus(setting(CHIP_AUTO), [other("Unknown", null)], []).kind).toBe("auto_unrecognized");
    expect(chipStatus(setting(0x1234, ""), [other("Unknown", null)], []).kind).toBe("select_chip");
  });

  it("checks the folder scan when no adapter is there", () => {
    expect(chipStatus(setting(CHIP_AUTO, "rtl8723bu"), [], [file("RTL8723BU")])).toEqual({
      kind: "unchecked",
      files: ["rtl8723bu_fw.bin", "rtl8723bu_config.bin"],
      present: true,
    });
    expect(chipStatus(setting(0xb82c), [], [file("rtl8822bu", 0xb82c, false)])).toMatchObject({ present: false });
    expect(chipStatus(setting(0xc821), [], [])).toMatchObject({ present: false });
    // The scan leaves custom out
    expect(chipStatus(setting(CHIP_CUSTOM, "custom"), [], [])).toMatchObject({ present: null });
  });

  it("gives each adapter card a short state", () => {
    const s = setting(CHIP_AUTO);
    expect(adapterFirmwareTag(rtl(true), s)).toEqual({ key: "firmware.present", status: "good" });
    expect(adapterFirmwareTag(rtl(false), s)).toEqual({ key: "firmware.missing_short", status: "bad" });
    expect(adapterFirmwareTag(other("Intel", "firmware.vendor_intel"), s).key).toBe("ui.adapter.firmware_not_realtek");
    expect(adapterFirmwareTag(other("Qualcomm", "firmware.vendor_unsupported"), s)).toEqual({
      key: "ui.adapter.firmware_unsupported",
      status: "bad",
    });
    expect(adapterFirmwareTag(rtl(null), setting(CHIP_NOT_REALTEK)).key).toBe("ui.adapter.firmware_not_realtek");
    expect(adapterFirmwareTag(other("Unknown", null), s)).toEqual({ key: "ui.adapter.firmware_choose_chip", status: "warn" });
  });

  it("tells a file that is no firmware apart from a missing one", () => {
    // Issue #66: kernel.org's preview page saved as rtl8761bu_fw.bin
    const page: AdapterFirmware = { ...rtl(false), firmware_invalid: "rtl8761bu_fw.bin" };
    expect(chipStatus(setting(CHIP_AUTO), [page], [])).toEqual({
      kind: "invalid",
      files: ["rtl8761bu_fw.bin", "rtl8761bu_config.bin"],
      file: "rtl8761bu_fw.bin",
      download: false,
    });
    expect(adapterFirmwareTag(page, setting(CHIP_AUTO))).toEqual({ key: "firmware.invalid_short", status: "bad" });
    expect(chipStatus(setting(CHIP_AUTO), [{ ...rtl(false), firmware_invalid: null }], []).kind).toBe("missing");
    // Present wins over a stale name
    expect(chipStatus(setting(CHIP_AUTO), [{ ...rtl(true), firmware_invalid: "x" }], []).kind).toBe("present");
  });

  it("opens the vendor's firmware page", () => {
    expect(vendorFirmwareUrl("Intel")).toBe(LINKS.firmwareIntel);
    expect(vendorFirmwareUrl("Broadcom")).toBe(LINKS.firmwareBroadcom);
    expect(vendorFirmwareUrl("CSR")).toBeNull();
    expect(firmwareUrl("Realtek")).toBe(LINKS.firmwareRealtek);
  });
});

describe("mock core firmware state", () => {
  const folder = [file("rtl8761bu", 0x8771), file("rtl8723du", 0xd723, false), file("rtl8723bu")];
  const rtl = sampleSnapshot().adapter.adapters[0];

  it("follows the chip setting as the core's adapter_infos", () => {
    const pair = (stem: string) => [`${stem}_fw.bin`, `${stem}_config.bin`];
    expect(adapterWithFirmware(rtl, setting(CHIP_AUTO), folder)).toMatchObject({ firmware_present: true, firmware_files: pair("rtl8761bu") });
    expect(adapterWithFirmware(rtl, setting(CHIP_NOT_REALTEK), folder)).toMatchObject({ firmware_present: null, firmware_files: [] });
    // The config file is missing
    expect(adapterWithFirmware(rtl, setting(0xd723, "rtl8723du"), folder)).toMatchObject({ firmware_present: false });
    // A chip known only by its files
    expect(adapterWithFirmware(rtl, setting(CHIP_AUTO, "rtl8723bu"), folder)).toMatchObject({ firmware_present: true, firmware_files: pair("rtl8723bu") });
    expect(adapterWithFirmware(rtl, setting(CHIP_CUSTOM, "custom"), folder)).toMatchObject({ firmware_present: false, firmware_files: pair("custom") });
    // Auto without a detected Realtek chip: nothing to load
    expect(adapterWithFirmware({ ...rtl, realtek_chip: null }, setting(CHIP_AUTO), folder)).toMatchObject({ firmware_present: null });
  });
});

describe("chipTargetIndex", () => {
  const base = { vendor: "x", realtek_chip: null, firmware_present: null, firmware_files: [], vendor_note: null };
  it("picks the Realtek adapter, then a checked one, then a vendor note, then the first", () => {
    expect(chipTargetIndex([])).toBe(-1);
    expect(chipTargetIndex([base, base])).toBe(0);
    const note = { ...base, vendor_note: { key: "firmware.vendor_intel", args: [] } };
    expect(chipTargetIndex([base, note])).toBe(1);
    expect(chipTargetIndex([note, { ...base, firmware_present: false }])).toBe(1);
    expect(chipTargetIndex([note, base, { ...base, realtek_chip: "RTL8761BU" }])).toBe(2);
  });
});
