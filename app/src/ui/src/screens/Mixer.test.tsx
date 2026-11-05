import { cleanup, fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "../app/App";
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

async function open(settings: Partial<Settings> = { developer_mode: true, volume_mixer: true, mute_output: false }) {
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
    await open({ developer_mode: false, mute_output: true });
    expect(screen.queryByRole("link", { name: t("ui.nav.mixer") })).toBeNull();
    expect(screen.queryByRole("heading", { level: 1, name: t("ui.nav.mixer") })).toBeNull();
    expect(screen.queryByRole("slider", { name: "Music Player" })).toBeNull();
  });

  it("needs its Developer toggle as well (issue #86)", async () => {
    await open({ developer_mode: true, volume_mixer: false, mute_output: true });
    expect(screen.queryByRole("link", { name: t("ui.nav.mixer") })).toBeNull();
    expect(screen.queryByRole("slider", { name: "Music Player" })).toBeNull();
  });

  it("shows the apps with the output's mute off", async () => {
    await open();
    expect(await screen.findByRole("link", { name: t("ui.nav.mixer") })).toBeInTheDocument();
    expect(await screen.findByRole("slider", { name: "Music Player" })).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: (n) => n.startsWith(t("capture.mute_output")) })).not.toBeChecked();
  });

  it("keeps the slider in the hand through a move and a refresh", async () => {
    const { calls } = await open();
    const slider = await screen.findByRole<HTMLInputElement>("slider", { name: "Music Player" });
    fireEvent.input(slider, { target: { value: "30" } });
    fireEvent.input(slider, { target: { value: "31" } });
    expect(screen.getByRole("slider", { name: "Music Player" })).toBe(slider);

    const refreshes = calls.filter((c) => c.method === "audio.mixer").length;
    await waitFor(() => expect(calls.filter((c) => c.method === "audio.mixer").length).toBeGreaterThan(refreshes), {
      timeout: 3000,
    });
    expect(screen.getByRole("slider", { name: "Music Player" })).toBe(slider);
    expect(slider.value).toBe("31");
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
    expect(mute).not.toBeChecked();
    fireEvent.click(mute);
    await waitFor(() => expect(core.state.snap!.settings.mute_output).toBe(true));
    expect(screen.getByText(t("ui.mixer.note_exit"))).toBeInTheDocument();
  });
});
