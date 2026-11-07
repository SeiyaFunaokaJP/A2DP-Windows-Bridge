// Developer mode: a saved device's pairing (Audio Devices) and taking over
// Windows' pairings (Adapter), issue #86

import { cleanup, fireEvent, render, screen, within } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "../app/App";
import { t } from "../i18n";
import { MockCore } from "../ipc/mock";
import { createCore } from "../ipc/store";

let mock: MockCore | null = null;

afterEach(() => {
  cleanup();
  mock?.dispose();
  mock = null;
});

async function open(hash: string, developer: boolean) {
  location.hash = hash;
  const calls: { method: string; params: unknown }[] = [];
  mock = new MockCore({ speed: 0, onCall: (method, params) => calls.push({ method, params }) });
  const core = createCore(mock);
  render(() => <App core={core} />);
  await screen.findByRole("navigation");
  if (developer) {
    await core.request("settings.set", { settings: { ...core.state.snap!.settings, developer_mode: true } });
  }
  return { core, calls };
}

describe("device pairing", () => {
  it("has no Services tag without developer mode", async () => {
    await open("#/devices", false);
    await screen.findAllByText(t("device.tag_saved"));
    expect(screen.queryByRole("button", { name: t("ui.pairing.services") })).toBeNull();
    // The tag is a tag, not a button (issue #86)
    expect(screen.queryByRole("button", { name: t("device.tag_saved") })).toBeNull();
  });

  it("lists the services as a table, the key below them (issue #86)", async () => {
    const { calls } = await open("#/devices", true);
    const [tag] = await screen.findAllByRole("button", { name: t("ui.pairing.services") });
    expect(tag).toHaveClass("tag");
    fireEvent.click(tag);
    const dialog = await screen.findByRole("dialog", { name: t("ui.pairing.services") });
    const table = await within(dialog).findByRole("table");
    expect(within(table).getByRole("columnheader", { name: t("ui.pairing.col_name") })).toBeInTheDocument();
    expect(within(table).getByRole("columnheader", { name: t("ui.pairing.col_guid") })).toBeInTheDocument();
    expect(within(table).getByText("Audio Sink")).toBeInTheDocument();
    expect(within(table).getByText("A/V Remote Control")).toBeInTheDocument();
    // One name: the SIG's, not the device's beside it; one known to neither
    expect(within(table).getByText("Serial Port")).toBeInTheDocument();
    expect(within(table).queryByText(/GAIA/)).toBeNull();
    expect(within(table).getByText(t("ui.pairing.unknown_service"))).toBeInTheDocument();
    // The key in the same dialog, with no Linux file
    expect(await within(dialog).findByText("00112233445566778899AABBCCDDEEFF")).toBeInTheDocument();
    expect(within(dialog).getByText(t("ui.pairing.key_type", 4))).toBeInTheDocument();
    expect(within(dialog).getByText(t("ui.pairing.no_le_keys"))).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: t("ui.pairing.copy_key") })).toBeInTheDocument();
    expect(within(dialog).getAllByRole("button").map((b) => b.textContent)).not.toContain("Copy for Linux");
    expect(calls.filter((c) => c.method === "device.services")).toHaveLength(1);
  });

  it("imports Windows' pairings on Audio Devices, without developer mode (issue #86)", async () => {
    const { calls } = await open("#/devices", false);
    // Only while Windows lists a device BlueCaster has not saved
    mock!.windowsPaired = ["11:22:33:44:55:66"];
    await mock!.call("adapter.list");
    const button = await screen.findByRole("button", { name: t("ui.adapter.import_windows") });
    expect(screen.getByText(t("ui.devices.import_hint"))).toBeInTheDocument();
    fireEvent.click(button);
    expect(await screen.findByText(t("ui.adapter.import_done", 1))).toBeInTheDocument();
    const sent = calls.find((c) => c.method === "device.import_windows_pairings")!.params as { adapter: string };
    expect(sent.adapter).toMatch(/^[0-9A-F]{2}(:[0-9A-F]{2}){5}$/);
  });

  it("offers no import when Windows has nothing new, nor on the Adapter screen", async () => {
    await open("#/devices", true);
    await screen.findAllByText(t("device.tag_saved"));
    expect(screen.queryByRole("button", { name: t("ui.adapter.import_windows") })).toBeNull();
    location.hash = "#/adapter";
    await screen.findByRole("heading", { level: 1, name: t("ui.nav.adapter") });
    expect(screen.queryByRole("button", { name: t("ui.adapter.import_windows") })).toBeNull();
  });
});
