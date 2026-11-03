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
  it("is a plain tag without developer mode", async () => {
    await open("#/devices", false);
    await screen.findAllByText(t("device.tag_saved"));
    expect(screen.queryByRole("button", { name: new RegExp(`^${t("ui.pairing.title")}:`) })).toBeNull();
  });

  it("shows the key, its Linux form and the services", async () => {
    const { calls } = await open("#/devices", true);
    const [tag] = await screen.findAllByRole("button", { name: new RegExp(`^${t("ui.pairing.title")}:`) });
    fireEvent.click(tag);
    const dialog = await screen.findByRole("dialog");
    expect(await within(dialog).findByText("00112233445566778899AABBCCDDEEFF")).toBeInTheDocument();
    expect(within(dialog).getByText(t("ui.pairing.key_type", 4))).toBeInTheDocument();
    expect(await within(dialog).findByText("Audio Sink")).toBeInTheDocument();
    // No SIG name: the device's own
    expect(within(dialog).getByText("Vendor")).toBeInTheDocument();
    expect(within(dialog).getByText("A/V Remote Control")).toBeInTheDocument();
    expect(calls.filter((c) => c.method === "device.services")).toHaveLength(1);
  });

  it("imports Windows' pairings of an adapter on WinUSB", async () => {
    const { calls } = await open("#/adapter", true);
    const buttons = await screen.findAllByRole("button", { name: t("ui.adapter.import_windows") });
    fireEvent.click(buttons[0]);
    expect(await screen.findByText(t("ui.adapter.import_done", 1))).toBeInTheDocument();
    const sent = calls.find((c) => c.method === "device.import_windows_pairings")!.params as { adapter: string };
    expect(sent.adapter).toMatch(/^[0-9A-F]{2}(:[0-9A-F]{2}){5}$/);
  });
});
