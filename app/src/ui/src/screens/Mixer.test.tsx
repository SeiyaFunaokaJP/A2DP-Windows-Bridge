import { cleanup, fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "../app/App";
import { mixerAvailable } from "../components/Mixer";
import { t } from "../i18n";
import { MockCore } from "../ipc/mock";
import { createCore } from "../ipc/store";

let mock: MockCore | null = null;

afterEach(() => {
  cleanup();
  mock?.dispose();
  mock = null;
});

async function open() {
  location.hash = "#/mixer";
  const calls: { method: string; params: unknown }[] = [];
  mock = new MockCore({ speed: 0, onCall: (method, params) => calls.push({ method, params }) });
  const core = createCore(mock);
  render(() => <App core={core} />);
  await screen.findByRole("navigation");
  return { core, calls };
}

describe("volume mixer", () => {
  it("is in the menu for System Loopback, and for any source in developer mode", async () => {
    const { core } = await open();
    expect(await screen.findByRole("link", { name: t("ui.nav.mixer") })).toBeInTheDocument();
    expect(await screen.findByRole("heading", { level: 1, name: t("ui.nav.mixer") })).toBeInTheDocument();

    const snap = core.state.snap!;
    const app = snap.profiles.map((p) => ({ ...p, capture_mode: "app" }));
    expect(mixerAvailable({ ...snap, profiles: app })).toBe(false);
    expect(mixerAvailable({ ...snap, profiles: app, settings: { ...snap.settings, developer_mode: true } })).toBe(true);
    // No profile in use: any that captures System Loopback
    expect(mixerAvailable({ ...snap, active_profile: null })).toBe(true);
    expect(mixerAvailable({ ...snap, active_profile: null, profiles: app })).toBe(false);
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
