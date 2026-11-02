import { cleanup, fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "../app/App";
import { mixerAvailable } from "../components/Mixer";
import { t } from "../i18n";
import { MockCore } from "../ipc/mock";
import { createCore } from "../ipc/store";
import type { Settings } from "../ipc/types";

let mock: MockCore | null = null;

afterEach(() => {
  cleanup();
  mock?.dispose();
  mock = null;
});

async function open(settings: Partial<Settings> = { developer_mode: true, mute_output: true }) {
  location.hash = "#/mixer";
  const calls: { method: string; params: unknown }[] = [];
  mock = new MockCore({ speed: 0, onCall: (method, params) => calls.push({ method, params }) });
  const core = createCore(mock);
  render(() => <App core={core} />);
  await screen.findByRole("navigation");
  await core.request("settings.set", { settings: { ...core.state.snap!.settings, ...settings } });
  return { core, calls };
}

describe("volume mixer", () => {
  it("is only in the menu in developer mode, even with the mute left on", async () => {
    const { core } = await open({ developer_mode: false, mute_output: true });
    expect(screen.queryByRole("link", { name: t("ui.nav.mixer") })).toBeNull();
    expect(screen.queryByRole("heading", { level: 1, name: t("ui.nav.mixer") })).toBeNull();
    expect(mixerAvailable(core.state.snap!)).toBe(false);
  });

  it("shows the apps, here and in the flyout, once the mute is on", async () => {
    const { core } = await open({ developer_mode: true, mute_output: false });
    expect(await screen.findByRole("link", { name: t("ui.nav.mixer") })).toBeInTheDocument();
    await screen.findByRole("heading", { level: 1, name: t("ui.nav.mixer") });
    expect(screen.queryByRole("slider", { name: "Music Player" })).toBeNull();
    expect(mixerAvailable(core.state.snap!)).toBe(false);

    fireEvent.click(screen.getByRole("switch", { name: (n) => n.startsWith(t("capture.mute_output")) }));
    expect(await screen.findByRole("slider", { name: "Music Player" })).toBeInTheDocument();
    expect(mixerAvailable(core.state.snap!)).toBe(true);
  });

  it("sets an app's volume and mute, and the output's mute", async () => {
    const { core, calls } = await open();
    const slider = await screen.findByRole("slider", { name: "Music Player" });
    fireEvent.input(slider, { target: { value: "30" } });
    await waitFor(() =>
      expect(calls.at(-1)).toEqual({ method: "audio.mixer_set", params: { app: "player.exe", volume: 30, muted: null } }),
    );
    fireEvent.click(screen.getByRole("button", { name: `Music Player: ${t("ui.volume.mute")}` }));
    await waitFor(() =>
      expect(calls.at(-1)).toEqual({ method: "audio.mixer_set", params: { app: "player.exe", volume: null, muted: true } }),
    );

    const mute = screen.getByRole("switch", { name: (n) => n.startsWith(t("capture.mute_output")) });
    expect(mute).toBeChecked();
    fireEvent.click(mute);
    await waitFor(() => expect(core.state.snap!.settings.mute_output).toBe(false));
    expect(screen.getByText(t("ui.mixer.note_exit"))).toBeInTheDocument();
  });
});
