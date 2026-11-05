import { describe, expect, it } from "vitest";
import { lang, LANGUAGES, setLanguage, t, tm } from "./index";
import en from "../../../../../app/lang/en.json";
import { printf } from "./printf";

describe("printf", () => {
  it("fills the placeholders the language files use", () => {
    expect(printf("try %d/%d, %zu found, %.1f kHz, 100%%", ["2", "3", "4", "48.0"])).toBe(
      "try 2/3, 4 found, 48.0 kHz, 100%",
    );
    expect(printf("%s (%s), Bluetooth %s, L2CAP MTU %u", ["sink", "host", "5.0", "672"])).toBe(
      "sink (host), Bluetooth 5.0, L2CAP MTU 672",
    );
    expect(printf("%llu dropped (%.2f%%)", ["12", "0.50"])).toBe("12 dropped (0.50%)");
  });

  it("formats numbers from the UI by the conversion", () => {
    expect(printf("%.1f kHz", [44.1])).toBe("44.1 kHz");
    expect(printf("%.0f kbps", [989.6])).toBe("990 kbps");
    expect(printf("%d", [3.7])).toBe("3");
    expect(printf("RSSI %+d dB", [4])).toBe("RSSI +4 dB");
    expect(printf("RSSI %+d dB", [-4])).toBe("RSSI -4 dB");
  });

  it("leaves missing arguments empty", () => {
    expect(printf("a %s b %s", ["x"])).toBe("a x b ");
  });
});

describe("translations", () => {
  it("formats keys and core messages in English", async () => {
    await setLanguage("en");
    expect(t("status.connecting_retry", 2, 3)).toBe("Connection failed, trying again (2/3)...");
    expect(tm({ key: "status.scan_complete", args: ["4"] })).toBe("Scan complete: 4 device(s) found");
    // An unknown key shows itself, so it is noticed
    expect(t("no.such.key")).toBe("no.such.key");
  });

  it("loads a language and falls back to English for an unknown one", async () => {
    await setLanguage("ja");
    expect(lang()).toBe("ja");
    expect(t("ui.nav.home")).toBe("ホーム");
    await setLanguage("de");
    expect(t("ui.nav.home")).toBe("Start");
    expect(t("connection.connect")).not.toBe("");
    await setLanguage("xx");
    expect(lang()).toBe("en");
    expect(t("ui.nav.home")).toBe("Home");
  });

  it("lists every language file", async () => {
    const files = Object.keys(import.meta.glob("../../../../../app/lang/*.json")).map((p) =>
      p.replace(/^.*\/(.+)\.json$/, "$1"),
    );
    expect(LANGUAGES.map((l) => l.code).sort()).toEqual(files.sort());
    // The names match the files' own _language_name
    const names = import.meta.glob<string>("../../../../../app/lang/*.json", { import: "_language_name", eager: true });
    for (const l of LANGUAGES) {
      const key = Object.keys(names).find((p) => p.endsWith(`/${l.code}.json`))!;
      expect(names[key]).toBe(l.name);
    }
  });

  it("has every key the screens ask for by name", () => {
    // A key removed from the language files while still used shows as the
    // key itself (ui.home.profile_line on the setup, issue #86)
    const sources = import.meta.glob<string>(["../**/*.{ts,tsx}", "!../**/*.test.{ts,tsx}"], {
      query: "?raw",
      import: "default",
      eager: true,
    });
    expect(Object.keys(sources)).toContain("../screens/Setup.tsx");
    const missing: string[] = [];
    for (const [file, text] of Object.entries(sources)) {
      for (const m of text.matchAll(/\bt\(\s*"([a-z0-9_]+(?:\.[a-z0-9_]+)+)"/g)) {
        if (!(m[1] in en)) missing.push(`${file}: ${m[1]}`);
      }
    }
    expect(missing).toEqual([]);
  });
});
