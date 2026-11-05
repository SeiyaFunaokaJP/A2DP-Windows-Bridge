// The pairing dialog's helpers (lib/pairing.ts)

import { describe, expect, it } from "vitest";
import { bluezInfo, bluezLinkKey, serviceLabel, serviceName, shortUuid } from "./pairing";

describe("pairing", () => {
  it("names the SIG services and leaves vendors' alone", () => {
    expect(serviceName("0000110B-0000-1000-8000-00805F9B34FB")).toBe("Audio Sink");
    expect(serviceName("0000111e-0000-1000-8000-00805f9b34fb")).toBe("Handsfree");
    expect(serviceName("0000eb04-d102-11e1-9b23-00025b00a5a5")).toBeNull();
    expect(serviceName("0000fff0-0000-1000-8000-00805f9b34fb")).toBeNull();
    expect(shortUuid("df21fe2c-2515-4fdb-8886-f12c4d67927c")).toBeNull();
    expect(shortUuid("0000180f-0000-1000-8000-00805f9b34fb")).toBe(0x180f);
  });

  it("writes the key as BlueZ keeps it", () => {
    expect(bluezLinkKey("00112233445566778899aabbccddeeff", 4)).toBe(
      "[LinkKey]\nKey=00112233445566778899AABBCCDDEEFF\nType=4\nPINLength=0\n",
    );
  });

  it("labels services as Windows does (issue #86)", () => {
    const sink = "0000110b-0000-1000-8000-00805f9b34fb";
    const spp = "00001101-0000-1000-8000-00805f9b34fb";
    expect(serviceLabel({ uuids: [sink], name: "Audio Sink" })).toBe("Audio Sink");
    expect(serviceLabel({ uuids: [spp], name: "GAIA" })).toBe("Serial Port 'GAIA'");
    expect(serviceLabel({ uuids: ["df21fe2c-2515-4fdb-8886-f12c4d67927c"], name: "Vendor" })).toBe("Vendor");
    expect(serviceLabel({ uuids: ["df21fe2c-2515-4fdb-8886-f12c4d67927c"], name: null })).toBeNull();
  });

  it("writes BlueZ' whole info file, services when known", () => {
    expect(bluezInfo("EarFun", ["0000110B-0000-1000-8000-00805F9B34FB"], "00112233445566778899aabbccddeeff", 4)).toBe(
      "[General]\nName=EarFun\nSupportedTechnologies=BR/EDR;\nTrusted=true\nBlocked=false\n" +
        "Services=0000110b-0000-1000-8000-00805f9b34fb;\n\n" +
        "[LinkKey]\nKey=00112233445566778899AABBCCDDEEFF\nType=4\nPINLength=0\n",
    );
    expect(bluezInfo("EarFun", [], "00", 4)).not.toContain("Services=");
  });
});
