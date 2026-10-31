// The app shell's v1 leftovers: update check result, update notice, tray
// hide, firmware bar, error copy, capture source, HCI capture sign, the
// setup wizard's adapter and device steps.

import { cleanup, fireEvent, render, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, describe, expect, it, vi } from "vitest";
import { App, hideToTray, isDiagnosticsShortcut } from "./App";
import { captureRunningText } from "./indicators";
import { errorCopyText, isAppFault, sourceText, stateLabel, statusText } from "../components/stream";
import { setLanguage, t } from "../i18n";
import { MockCore, sampleProfile, type MockOptions } from "../ipc/mock";
import type { UpdatePhase } from "../ipc/transport";
import { createCore } from "../ipc/store";
import { LINKS } from "../lib/links";
import { toneParams } from "../lib/profile";

let mock: MockCore | null = null;

function start(hash: string, opts: MockOptions = {}) {
  location.hash = hash;
  const calls: { method: string; params: unknown }[] = [];
  mock = new MockCore({ speed: 0, ...opts, onCall: (method, params) => calls.push({ method, params }) });
  const core = createCore(mock);
  const view = render(() => <App core={core} />);
  return { core, calls, view, mock };
}

afterEach(async () => {
  cleanup();
  mock?.dispose();
  mock = null;
  vi.restoreAllMocks();
  await setLanguage("en");
});

describe("keyboard", () => {
  it("Ctrl+Shift+D opens the diagnostics", async () => {
    start("#/home");
    await screen.findAllByRole("button", { name: "Connect" });
    fireEvent.keyDown(window, { key: "D", code: "KeyD", ctrlKey: true, shiftKey: true });
    expect(await screen.findByRole("heading", { level: 1, name: "Diagnostics" })).toBeInTheDocument();
    expect(isDiagnosticsShortcut({ key: "d", code: "KeyD", ctrlKey: true, shiftKey: false, altKey: false, metaKey: false })).toBe(false);
    expect(isDiagnosticsShortcut({ key: "D", code: "KeyD", ctrlKey: true, shiftKey: true, altKey: true, metaKey: false })).toBe(false);
  });
});

describe("update check", () => {
  it("a failed check shows as failed with the releases page", async () => {
    const { mock } = start("#/settings", { scenario: "offline" });
    const open = vi.spyOn(mock, "openUrl").mockResolvedValue();
    fireEvent.click(await screen.findByRole("button", { name: "Check for Updates" }));
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("Could not check for updates.");
    fireEvent.click(within(alert).getByRole("button", { name: t("ui.update.open_releases") }));
    expect(open).toHaveBeenCalledWith(LINKS.releases);
    // Not also as a transient notice
    expect(document.querySelector(".notice-error")).toBeNull();
  });

  it("a newer release opens a dialog with its short notes; Update now installs it on Settings (issue #86)", async () => {
    const { mock } = start("#/home", { scenario: "update" });
    const install = vi.spyOn(mock, "updateInstall");
    const dialog = await screen.findByRole("dialog", { name: "Update Available" });
    expect(dialog).toHaveTextContent("Version 2.0.0-beta.1 is available (current: 2.0.0-alpha.1).");
    // Markdown as plain lines
    const notes = within(dialog).getAllByRole("listitem").map((li) => li.textContent);
    expect(notes).toEqual(["Faster scan: devices show up as they answer", "A notification when a device connects (#86)"]);
    fireEvent.click(await within(dialog).findByRole("button", { name: "Update now" }));
    expect(await screen.findByRole("heading", { level: 1, name: "Settings" })).toBeInTheDocument();
    expect(install).toHaveBeenCalledWith(expect.stringContaining("v2.0.0-beta.1/latest.json"));
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("Later closes the update dialog for that version; a portable copy opens the download page", async () => {
    localStorage.removeItem("bluecaster.update_later");
    const { mock } = start("#/codecs", { scenario: "update" });
    (mock as unknown as { updatePhase: UpdatePhase }).updatePhase = { state: "unavailable", reason: "portable" };
    const dialog = await screen.findByRole("dialog", { name: "Update Available" });
    expect(within(dialog).getByRole("button", { name: "Open download page" })).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole("button", { name: "Later" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(localStorage.getItem("bluecaster.update_later")).toBe("2.0.0-beta.1");
    localStorage.removeItem("bluecaster.update_later");
  });

  it("a crash of BlueCaster shows on every screen, ready to report, until closed", async () => {
    const { mock } = start("#/profiles", { scenario: "crash" });
    const open = vi.spyOn(mock, "openUrl").mockResolvedValue();
    const write = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText: write }, configurable: true });
    const banner = (await screen.findByText(t("ui.crash.title"))).closest(".banner") as HTMLElement;
    expect(banner).toHaveTextContent("bluecaster-core, 2026-10-22 01:23:38");
    expect(banner).toHaveTextContent("panic in thread encode");

    fireEvent.click(within(banner).getByRole("button", { name: t("ui.diag.report_github") }));
    await waitFor(() => expect(open).toHaveBeenCalled());
    const url = new URL(open.mock.calls[0][0]);
    expect(url.searchParams.get("what_happened")).toContain("panic in thread encode");
    // The backtrace first, then the diagnostic info
    const logs = url.searchParams.get("logs")!;
    expect(logs).toContain("bluecaster_core::encode::run");
    expect(logs.indexOf("bluecaster_core::encode::run")).toBeLessThan(logs.indexOf("BlueCaster"));
    expect(write.mock.calls[0][0]).toContain("```text\n");

    const seen = vi.spyOn(mock, "call");
    fireEvent.click(within(banner).getByRole("button", { name: "Close" }));
    await waitFor(() => expect(screen.queryByText(t("ui.crash.title"))).toBeNull());
    expect(seen.mock.calls.some(([m]) => m === "diag.crashes_seen")).toBe(true);
  });
});

describe("tray", () => {
  it("hiding to the tray tells the core", async () => {
    const { core, calls, mock } = start("#/home");
    await screen.findByRole("heading", { level: 1, name: "Home" });
    const hide = vi.spyOn(mock, "hideWindow");
    await hideToTray(core);
    expect(hide).toHaveBeenCalled();
    expect(calls.some((c) => c.method === "app.hidden_to_tray")).toBe(true);
  });

  it("start in the tray applies only when starting with Windows", async () => {
    const { core } = start("#/settings");
    const toggle = await screen.findByRole("switch", { name: /Start in the tray/ });
    expect(toggle).not.toBeDisabled();
    await core.request("settings.set", { settings: { ...core.state.snap!.settings, start_with_windows: false } });
    await waitFor(() => expect(toggle).toBeDisabled());
  });

  it("a click on the tray icon can open the main window instead of the flyout (issue #86)", async () => {
    const { core, calls } = start("#/settings");
    const toggle = await screen.findByRole("switch", { name: new RegExp(t("ui.settings.tray_click_opens_window")) });
    expect(toggle).not.toBeChecked();
    fireEvent.click(toggle);
    await waitFor(() => expect(core.state.snap!.settings.tray_click_opens_window).toBe(true));
    const set = calls.filter((c) => c.method === "settings.set").at(-1)!.params as { settings: { tray_click_opens_window: boolean } };
    expect(set.settings.tray_click_opens_window).toBe(true);
  });

  it("turns the notification of a connection off and on (issue #86)", async () => {
    const { core } = start("#/settings");
    const toggle = await screen.findByRole("switch", { name: new RegExp(t("ui.settings.notify_connected")) });
    expect(toggle).toBeChecked();
    fireEvent.click(toggle);
    await waitFor(() => expect(core.state.snap!.settings.notify_connected).toBe(false));
    expect(toggle).not.toBeChecked();
  });

  it("chooses when the low battery is notified, or never (issue #86)", async () => {
    const { core } = start("#/settings");
    const select = (await screen.findByRole("combobox", { name: t("ui.settings.battery_low") })) as HTMLSelectElement;
    expect([...select.options].map((o) => o.text)).toEqual(["Below 20%", "Below 10%", "Below 5%", "Never"]);
    expect(select).toHaveDisplayValue("Below 20%");
    fireEvent.change(select, { target: { value: "5" } });
    await waitFor(() => expect(core.state.snap!.settings.battery_low_percent).toBe(5));
    fireEvent.change(select, { target: { value: "0" } });
    await waitFor(() => expect(core.state.snap!.settings.battery_low_percent).toBe(0));
    expect(select).toHaveDisplayValue("Never");
  });
});

describe("Home", () => {
  it("shows the firmware bar and leads to the Adapter screen", async () => {
    const { mock } = start("#/home");
    await screen.findByRole("heading", { level: 1, name: "Home" });
    const m = mock as unknown as { snap: { adapter: { adapters: { firmware_present: boolean | null }[] } }; emit: (e: unknown) => void };
    const adapter = { ...m.snap.adapter, adapters: [{ ...m.snap.adapter.adapters[0], firmware_present: false }] };
    m.emit({ kind: "adapter", value: adapter });
    const title = await screen.findByText("Firmware not found");
    const bar = title.closest(".banner") as HTMLElement;
    expect(bar).toHaveTextContent("rtl8761bu_fw.bin, rtl8761bu_config.bin");
    // The Adapter screen's firmware actions are in the bar as well (issue #81)
    expect(within(bar).getByRole("button", { name: t("firmware.download") })).toBeInTheDocument();
    expect(within(bar).getByRole("button", { name: t("firmware.open_folder") })).toBeInTheDocument();
    fireEvent.click(within(bar).getByRole("button", { name: t("ui.home.firmware_fix") }));
    expect(await screen.findByRole("heading", { level: 1, name: "Adapters" })).toBeInTheDocument();
    expect(location.hash).toBe("#/adapter/firmware");
    expect(document.getElementById("adapter-firmware")).toBeInTheDocument();
  });

  it("copies the error with the diagnostic log", async () => {
    start("#/home", { scenario: "error" });
    const write = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText: write }, configurable: true });
    const alert = await screen.findByRole("alert");
    fireEvent.click(within(alert).getByRole("button", { name: t("ui.error.copy") }));
    await waitFor(() => expect(write).toHaveBeenCalled());
    const text = write.mock.calls[0][0] as string;
    expect(text.startsWith(`${t("error.connect_no_answer")}\n\n`)).toBe(true);
    expect(text).toContain("BlueCaster");
    expect(await screen.findByText(t("ui.error.copied"))).toBeInTheDocument();
  });

  it("offers no bug report for a cause on the user's side", async () => {
    start("#/home", { scenario: "error" });
    const alert = await screen.findByRole("alert");
    // The device did not answer: off or out of range, the fix is the answer
    expect(within(alert).getByRole("button", { name: t("ui.error.copy") })).toBeInTheDocument();
    expect(within(alert).queryByRole("button", { name: t("ui.diag.report_github") })).toBeNull();
    expect(isAppFault("error.connect_no_answer")).toBe(false);
    expect(isAppFault("error.encoder_init", "stream.bad_codec")).toBe(false);
    expect(isAppFault("error.encoder_init")).toBe(true);
    // Not known to be the user's: BlueCaster's own
    expect(isAppFault("error.something_new")).toBe(true);
  });

  it("reports an error of BlueCaster itself on GitHub with the form filled in", async () => {
    const { mock } = start("#/home", { scenario: "error" });
    const open = vi.spyOn(mock, "openUrl").mockResolvedValue();
    const write = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText: write }, configurable: true });
    await screen.findByRole("alert");
    const m = mock as unknown as { snap: { stream: object }; emit: (e: unknown) => void };
    m.emit({ kind: "stream", value: { ...m.snap.stream, message: { key: "error.encoder_init", args: [] } } });
    const report = await screen.findByRole("button", { name: t("ui.diag.report_github") });
    fireEvent.click(report);
    await waitFor(() => expect(open).toHaveBeenCalled());
    const url = new URL(open.mock.calls[0][0]);
    expect(url.searchParams.get("template")).toBe("bug_report.yml");
    expect(url.searchParams.get("what_happened")).toContain(t("error.encoder_init"));
    expect(url.searchParams.get("logs")).toContain("BlueCaster");
    // No bundle was saved: its field stays empty
    expect(url.searchParams.has("bundle")).toBe(false);
    expect(write).toHaveBeenCalled();
    expect(await screen.findByText(t("ui.error.report_opened"))).toBeInTheDocument();
  });

  it("shows the capture source, waiting for the application", async () => {
    const { mock } = start("#/home");
    await screen.findByRole("heading", { level: 1, name: "Home" });
    await mock.call("stream.start", { profile: "Desk speaker (AAC)" });
    await mock.call("profile.set_active", { name: "Desk speaker (AAC)" });
    // On the card of the device streaming (issue #86: no capture source card on Home)
    const card = await screen.findByRole("group", { name: "SRS-XB13" });
    expect(await within(card).findByText(t("ui.source.waiting", "Spotify"))).toBeInTheDocument();
  });

  it("shows a running HCI capture app-wide", async () => {
    const { mock } = start("#/home");
    await screen.findByRole("heading", { level: 1, name: "Home" });
    await mock.call("diag.capture_start");
    const sign = await screen.findByRole("button", { name: "HCI capture running" });
    fireEvent.click(sign);
    expect(await screen.findByRole("heading", { level: 1, name: "Diagnostics" })).toBeInTheDocument();
    await mock.call("diag.capture_stop");
    await waitFor(() => expect(screen.queryByRole("button", { name: "HCI capture running" })).toBeNull());
  });
});

describe("flyout", () => {
  it("shows the capture sign (the capture source is chosen in the main window)", async () => {
    const { mock } = start("", { window: "flyout" });
    await screen.findByRole("button", { name: "Connect" });
    expect(screen.queryByRole("button", { name: "HCI capture running" })).toBeNull();
    await mock.call("diag.capture_start");
    expect(await screen.findByRole("button", { name: "HCI capture running" })).toBeInTheDocument();
    expect(screen.queryByRole("combobox", { name: "Output Source" })).toBeNull();
  });
});

describe("setup device step", () => {
  async function toDeviceStep(opts: MockOptions = {}) {
    const r = start("#/setup", opts);
    await screen.findByRole("heading", { level: 1, name: "Set up BlueCaster" });
    for (let i = 0; i < 2; i++) {
      const next = screen.getByRole("button", { name: "Next" });
      await waitFor(() => expect(next).not.toBeDisabled());
      fireEvent.click(next);
    }
    await screen.findByRole("heading", { level: 2, name: "Find your audio devices" });
    return r;
  }

  it("three steps, the last one ends with Finish", async () => {
    await toDeviceStep();
    expect(screen.getAllByRole("listitem").map((li) => li.textContent?.replace(/^\d/, ""))).toEqual([
      "Choose the adapter",
      "Driver and firmware",
      "Find your audio devices",
    ]);
    expect(screen.queryByRole("button", { name: "Next" })).toBeNull();
    expect(screen.getByRole("button", { name: "Finish" })).toBeInTheDocument();
    // No profile name to type: Connect makes it
    expect(screen.queryByRole("textbox", { name: "Profile Name" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Create profile" })).toBeNull();
  });

  it("scans by itself; Connect makes the Auto profile and connects with it", async () => {
    const { calls, core } = await toDeviceStep({ scenario: "normal" });
    await waitFor(() => expect(calls.some((c) => c.method === "device.scan")).toBe(true));
    const name = await screen.findByText("Galaxy Buds2 Pro");
    const row = name.closest(".device-row") as HTMLElement;
    fireEvent.click(within(row).getByRole("button", { name: "Connect" }));
    await waitFor(() => expect(calls.some((c) => c.method === "stream.start")).toBe(true));
    expect(calls.find((c) => c.method === "stream.start")!.params).toEqual({ profile: "Galaxy Buds2 Pro | Auto" });
    const saved = core.state.snap!.profiles.find((p) => p.name === "Galaxy Buds2 Pro | Auto")!;
    expect(saved).toMatchObject({ device_address: "F4:4E:FD:00:11:22", codec: "auto" });
    expect(core.state.snap!.active_profile).toBe("Galaxy Buds2 Pro | Auto");
    expect(core.state.snap!.devices.find((d) => d.address === "F4:4E:FD:00:11:22")?.saved).toBe(true);
    expect(screen.getByRole("button", { name: "Finish" })).not.toBeDisabled();
  });

  it("keeps the device connected out of the list after Back and Next (issue #86)", async () => {
    const { calls } = await toDeviceStep({ scenario: "normal" });
    const row = (await screen.findByText("Galaxy Buds2 Pro")).closest(".device-row") as HTMLElement;
    fireEvent.click(within(row).getByRole("button", { name: "Connect" }));
    await waitFor(() => expect(calls.some((c) => c.method === "stream.start")).toBe(true));
    await waitFor(() => expect(screen.queryByText("Galaxy Buds2 Pro", { selector: ".device-row *" })).toBeNull());
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    const next = await screen.findByRole("button", { name: "Next" });
    await waitFor(() => expect(next).not.toBeDisabled());
    fireEvent.click(next);
    await screen.findByRole("heading", { level: 2, name: "Find your audio devices" });
    // Shown above the list as the one added, not in the list again
    await new Promise((r) => setTimeout(r, 300));
    expect(screen.queryByText("Galaxy Buds2 Pro", { selector: ".device-row *" })).toBeNull();
  });

  it("a wrong device is disconnected right there: back in the list, the scan resumes", async () => {
    const { calls } = await toDeviceStep({ scenario: "normal" });
    const row = (await screen.findByText("Galaxy Buds2 Pro")).closest(".device-row") as HTMLElement;
    fireEvent.click(within(row).getByRole("button", { name: "Connect" }));
    await waitFor(() => expect(calls.some((c) => c.method === "stream.start")).toBe(true));
    await waitFor(() => expect(screen.queryByText("Galaxy Buds2 Pro", { selector: ".device-row *" })).toBeNull());
    const scans = calls.filter((c) => c.method === "device.scan").length;
    fireEvent.click(await screen.findByRole("button", { name: "Disconnect" }));
    await waitFor(() => expect(calls.some((c) => c.method === "stream.stop")).toBe(true));
    expect(await screen.findByText("Galaxy Buds2 Pro", { selector: ".device-row *" })).toBeInTheDocument();
    await waitFor(() => expect(calls.filter((c) => c.method === "device.scan").length).toBeGreaterThan(scans));
    expect(screen.queryByRole("button", { name: "Disconnect" })).toBeNull();
  });

  it("a device with a profile connects with that one", async () => {
    const { calls, core } = await toDeviceStep();
    const before = core.state.snap!.profiles.length;
    const row = (await screen.findAllByText("WH-1000XM5")).map((e) => e.closest(".device-row")).find((r) => r) as HTMLElement;
    fireEvent.click(within(row).getByRole("button", { name: "Connect" }));
    await waitFor(() => expect(calls.some((c) => c.method === "stream.start")).toBe(true));
    expect(calls.find((c) => c.method === "stream.start")!.params).toEqual({ profile: "WH-1000XM5 (LDAC)" });
    expect(core.state.snap!.profiles.length).toBe(before);
  });

  it("an address typed by hand connects with Use", async () => {
    const { calls } = await toDeviceStep();
    fireEvent.input(screen.getByRole("textbox", { name: "Manual:" }), { target: { value: "11:22:33:44:55:66" } });
    fireEvent.click(screen.getByRole("button", { name: "Use" }));
    await waitFor(() => expect(calls.some((c) => c.method === "stream.start")).toBe(true));
    expect(calls.find((c) => c.method === "stream.start")!.params).toEqual({ profile: "11:22:33:44:55:66 | Auto" });
  });

  it("asks whether BlueCaster starts with Windows, and then in the tray (issue #86)", async () => {
    const { core } = await toDeviceStep();
    await core.request("settings.set", {
      settings: { ...core.state.snap!.settings, start_with_windows: false, start_minimized: false },
    });
    const withWindows = await screen.findByRole("switch", { name: t("settings.start_with_windows") });
    const inTray = screen.getByRole("switch", { name: new RegExp(t("ui.settings.start_minimized")) });
    await waitFor(() => expect(withWindows).not.toBeChecked());
    expect(inTray).toBeDisabled();
    fireEvent.click(withWindows);
    await waitFor(() => expect(core.state.snap!.settings.start_with_windows).toBe(true));
    await waitFor(() => expect(inTray).not.toBeDisabled());
    fireEvent.click(inTray);
    await waitFor(() => expect(core.state.snap!.settings.start_minimized).toBe(true));
  });

  it("starts with Windows, in the tray, by default on a first setup (issue #86)", async () => {
    const { core, mock: m } = start("#/setup");
    await screen.findByRole("heading", { level: 1, name: "Set up BlueCaster" });
    // A first run: no profile yet, both off
    for (const p of [...core.state.snap!.profiles]) await m.call("profile.delete", { name: p.name });
    await core.request("settings.set", {
      settings: { ...core.state.snap!.settings, start_with_windows: false, start_minimized: false },
    });
    await waitFor(() => expect(core.state.snap!.profiles).toHaveLength(0));
    for (let i = 0; i < 2; i++) {
      const next = screen.getByRole("button", { name: "Next" });
      await waitFor(() => expect(next).not.toBeDisabled());
      fireEvent.click(next);
    }
    const withWindows = await screen.findByRole("switch", { name: t("settings.start_with_windows") });
    const inTray = screen.getByRole("switch", { name: new RegExp(t("ui.settings.start_minimized")) });
    await waitFor(() => expect(withWindows).toBeChecked());
    expect(inTray).toBeChecked();
    expect(core.state.snap!.settings).toMatchObject({ start_with_windows: true, start_minimized: true });
    // Turned off, then back and forth: it stays off
    fireEvent.click(withWindows);
    await waitFor(() => expect(core.state.snap!.settings.start_with_windows).toBe(false));
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    fireEvent.click(await screen.findByRole("button", { name: "Next" }));
    expect(await screen.findByRole("switch", { name: t("settings.start_with_windows") })).not.toBeChecked();
  });

  it("also turns on the tray when only Start with Windows was on (issue #86)", async () => {
    const { core, mock: m } = start("#/setup");
    await screen.findByRole("heading", { level: 1, name: "Set up BlueCaster" });
    for (const p of [...core.state.snap!.profiles]) await m.call("profile.delete", { name: p.name });
    await core.request("settings.set", {
      settings: { ...core.state.snap!.settings, start_with_windows: true, start_minimized: false },
    });
    await waitFor(() => expect(core.state.snap!.profiles).toHaveLength(0));
    for (let i = 0; i < 2; i++) {
      const next = screen.getByRole("button", { name: "Next" });
      await waitFor(() => expect(next).not.toBeDisabled());
      fireEvent.click(next);
    }
    const inTray = await screen.findByRole("switch", { name: new RegExp(t("ui.settings.start_minimized")) });
    await waitFor(() => expect(inTray).toBeChecked());
    expect(core.state.snap!.settings).toMatchObject({ start_with_windows: true, start_minimized: true });
  });

  it("scans until a device is added, then lists it no more; System Loopback captures it (issue #86)", async () => {
    const { calls, core } = await toDeviceStep({ scenario: "normal" });
    await waitFor(() => expect(calls.some((c) => c.method === "device.scan")).toBe(true));
    expect(calls.find((c) => c.method === "device.scan")!.params).toEqual({ seconds: 600 });
    const row = (await screen.findByText("Galaxy Buds2 Pro")).closest(".device-row") as HTMLElement;
    fireEvent.click(within(row).getByRole("button", { name: "Connect" }));
    await waitFor(() => expect(core.state.snap!.profiles.some((p) => p.name === "Galaxy Buds2 Pro | Auto")).toBe(true));
    // The added device is shown above the list, with its state, not in it
    await waitFor(() => expect(document.querySelector(".device-picker")!.textContent).not.toContain("Galaxy Buds2 Pro"));
    // and the scan is over: no new one starts
    await waitFor(() => expect(core.state.snap!.scanning).toBe(false));
    const scans = calls.filter((c) => c.method === "device.scan").length;
    await new Promise((r) => setTimeout(r, 50));
    expect(calls.filter((c) => c.method === "device.scan").length).toBe(scans);
    expect(core.state.snap!.profiles.find((p) => p.name === "Galaxy Buds2 Pro | Auto")).toMatchObject({
      capture_mode: "loopback",
      audio_device_id: "",
      audio_device_name: "",
    });
  });
});

describe("setup adapter step", () => {
  it("marks the adapters BlueCaster supports, and lists one it does not without letting it be chosen (issue #86)", async () => {
    start("#/setup", { scenario: "fresh" });
    await screen.findByRole("heading", { level: 1, name: "Set up BlueCaster" });
    const radios = screen.getAllByRole("radio");
    // The supported one first, and chosen
    const rtl = radios[0].closest(".choice") as HTMLElement;
    const mtk = radios[1].closest(".choice") as HTMLElement;
    expect(rtl).toHaveTextContent("TP-Link Bluetooth USB Adapter (RTL8761BU)");
    expect(within(rtl).getByText("Supported")).toHaveClass("st-good");
    expect(radios[0]).toBeChecked();
    expect(mtk).toHaveTextContent("MediaTek Bluetooth Adapter");
    expect(mtk).not.toHaveTextContent("[unsupported]");
    expect(within(mtk).getByText("Unsupported")).toHaveClass("st-bad");
    // Read-only: the supported one stays chosen
    expect(radios[1]).toBeDisabled();
    expect(mtk).toHaveClass("ro");
    expect(radios[0]).not.toBeDisabled();
    expect(radios[0]).toBeChecked();
    fireEvent.click(screen.getByRole("button", { name: "Next" }));
    expect(await screen.findByText("TP-Link Bluetooth USB Adapter (RTL8761BU)")).toBeInTheDocument();
    expect(screen.queryByText(t("ui.setup.unsupported_note"))).toBeNull();
  });
});

describe("helpers", () => {
  it("source text", () => {
    const p = sampleProfile({ app_exe: "Spotify.exe", app_name: "Spotify" });
    expect(sourceText({ kind: "tone", name: "", waiting: false }, null).text).toBe("Test tone");
    expect(sourceText({ kind: "loopback", name: "", waiting: false }, null).text).toBe("System Loopback");
    expect(sourceText({ kind: "virtual", name: "CABLE Input", waiting: false }, null).text).toBe("CABLE Input");
    expect(sourceText({ kind: "app", name: "spotify.exe", waiting: false }, p).text).toBe("Spotify");
    const waiting = sourceText({ kind: "app", name: "foo.exe", waiting: true }, p);
    expect(waiting).toMatchObject({ waiting: true, text: t("ui.source.waiting", "foo.exe") });
  });

  it("a reconnect reads as connecting, and its end as connected (issue #86)", () => {
    const msg = (key: string, ...args: string[]) => ({ key, args });
    expect(statusText({ state: "reconnecting", message: msg("status.reconnect_attempt", "1", "3") })).toBe("Connecting...");
    expect(statusText({ state: "connecting", message: msg("status.connecting_retry", "2", "3") })).toBe("Connecting...");
    expect(statusText({ state: "streaming", message: msg("status.reconnected") })).toBe("Connected");
    // Why another codec is used is on the codec's line (issue #86)
    expect(statusText({ state: "streaming", message: msg("status.connected_fallback", "Opus", "aptX") })).toBe("Connected");
    expect(statusText({ state: "streaming", message: msg("status.connected_fallback_in_use", "LDAC", "AAC") })).toBe("Connected");
    expect(statusText({ state: "connecting", message: msg("status.initializing_btstack") })).toBe("Starting Bluetooth...");
    expect(statusText({ state: "error", message: msg("error.connect_no_answer") })).toBe("Error");
    expect(statusText({ state: "idle", message: msg("") })).toBe("Disconnected");
    // Streaming reads "Connected" everywhere (issue #86); the other states keep their words
    expect(statusText({ state: "streaming", message: msg("") })).toBe("Connected");
    expect(stateLabel("streaming")).toBe("Connected");
    expect(stateLabel("connecting")).toBe("Connecting...");
    expect(stateLabel("reconnecting")).toBe("Reconnecting...");
    expect(stateLabel("idle")).toBe("Disconnected");
  });

  it("error copy text", () => {
    expect(errorCopyText({ key: "error.connect_no_answer", args: [] })).toBe(t("error.connect_no_answer"));
    expect(errorCopyText({ key: "error.connect_no_answer", args: [] }, "stream.failed")).toMatch(/\n\[stream\.failed\]$/);
  });

  it("capture sign text from v1's title suffix", async () => {
    expect(captureRunningText()).toBe("HCI capture running");
    await setLanguage("ja");
    expect(captureRunningText()).toBe("HCI キャプチャ中");
  });

  it("tone parameters from the profile", () => {
    const p = sampleProfile({ sample_rate: 0, bit_depth: 24, codec: "aac", max_media_payload: 0 });
    expect(toneParams(p)).toEqual({
      device: "AC:80:0A:12:34:56",
      device_name: "WH-1000XM5",
      codec: "aac",
      quality: "hq",
      sample_rate: null,
      bit_depth: 24,
      source: "tone",
      max_media_payload: null,
      abr: false,
      codec_fallback: true,
    });
  });
});
