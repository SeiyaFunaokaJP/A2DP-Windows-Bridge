import { describe, expect, it } from "vitest";
import type { AdapterInfo } from "../ipc/types";
import { supportedFirst } from "./Setup";

const adapter = (id: string, over: Partial<AdapterInfo>): AdapterInfo =>
  ({ instance_id: id, driver: "bluetooth", realtek_chip: null, vendor_note: null, ...over }) as AdapterInfo;

const INTEL = adapter("intel", { vendor: "Intel" });
const RTL = adapter("rtl", { vendor: "Realtek", realtek_chip: "RTL8761BU" });
const MEDIATEK = adapter("mtk", { vendor_note: { key: "firmware.vendor_unsupported", args: [] } });

const ids = (l: AdapterInfo[]) => l.map((a) => a.instance_id);

describe("setup adapter order", () => {
  it("puts a Realtek dongle before the built-in Intel adapter on the same driver", () => {
    expect(ids(supportedFirst([INTEL, MEDIATEK, RTL]))).toEqual(["rtl", "intel", "mtk"]);
  });

  it("keeps an adapter already on WinUSB first", () => {
    expect(ids(supportedFirst([RTL, { ...INTEL, driver: "winusb" }]))).toEqual(["intel", "rtl"]);
    expect(ids(supportedFirst([{ ...INTEL, driver: "winusb" }, { ...RTL, driver: "winusb" }]))).toEqual(["rtl", "intel"]);
  });
});
