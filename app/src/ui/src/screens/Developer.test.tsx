import { cleanup, fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
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

async function open() {
  location.hash = "#/developer";
  const calls: { method: string; params: unknown }[] = [];
  mock = new MockCore({ speed: 0, onCall: (method, params) => calls.push({ method, params }) });
  const core = createCore(mock);
  render(() => <App core={core} />);
  await screen.findByRole("navigation");
  return { core, calls };
}

describe("developer screen", () => {
  it("is hidden until developer mode is on", async () => {
    const { core } = await open();
    expect(screen.queryByRole("link", { name: "Developer" })).toBeNull();
    await core.request("settings.set", { settings: { ...core.state.snap!.settings, developer_mode: true } });
    expect(await screen.findByRole("link", { name: "Developer" })).toBeInTheDocument();
    expect(await screen.findByRole("heading", { level: 1, name: "Developer" })).toBeInTheDocument();
  });

  it("saves the overrides and turns them all off", async () => {
    const { core, calls } = await open();
    await core.request("settings.set", { settings: { ...core.state.snap!.settings, developer_mode: true } });
    fireEvent.click(await screen.findByRole("tab", { name: t("ui.dev.codec_settings") }));
    fireEvent.click(await screen.findByRole("switch", { name: new RegExp(t("ui.dev.rtp_marker")) }));
    await screen.findByText(t("ui.dev.active"));
    const sent = calls.filter((c) => c.method === "settings.set").pop()!.params as { settings: { dev_overrides: unknown } };
    expect(sent.settings.dev_overrides).toMatchObject({ rtp_marker: true, aac_bitrate_kbps: 0 });

    const bitrate = screen.getByLabelText(t("ui.dev.aac_bitrate")) as HTMLInputElement;
    fireEvent.input(bitrate, { target: { value: "9999" } });
    fireEvent.change(bitrate, { target: { value: "9999" } });
    await screen.findByDisplayValue("512");

    fireEvent.click(screen.getByRole("button", { name: t("ui.dev.reset") }));
    await screen.findByText(t("ui.dev.intro"));
    expect(core.state.snap!.settings.dev_overrides.rtp_marker).toBe(false);
  });

  it("saves the forced connection with the overrides", async () => {
    const { core } = await open();
    await core.request("settings.set", { settings: { ...core.state.snap!.settings, developer_mode: true } });
    fireEvent.click(await screen.findByRole("tab", { name: t("ui.dev.codec_settings") }));
    fireEvent.click(await screen.findByRole("switch", { name: new RegExp(t("ui.dev.force_codec")) }));
    await screen.findByText(t("ui.dev.active"));
    expect(core.state.snap!.settings.dev_overrides.force_codec).toBe(true);
  });

  it("keeps the Volume Mixer switch in the app settings (issue #86)", async () => {
    const { core } = await open();
    await core.request("settings.set", { settings: { ...core.state.snap!.settings, developer_mode: true } });
    expect(await screen.findByRole("tab", { name: t("ui.dev.app_settings") })).toHaveAttribute("aria-selected", "true");
    fireEvent.click(await screen.findByRole("switch", { name: new RegExp(t("ui.dev.volume_mixer")) }));
    await waitFor(() => expect(core.state.snap!.settings.volume_mixer).toBe(true));
    expect(await screen.findByRole("link", { name: t("ui.nav.mixer") })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("tab", { name: t("ui.dev.codec_settings") }));
    expect(screen.queryByRole("switch", { name: new RegExp(t("ui.dev.volume_mixer")) })).toBeNull();
  });

  it("saves the logging settings in range and puts markers in the log", async () => {
    const { core, calls } = await open();
    await core.request("settings.set", { settings: { ...core.state.snap!.settings, developer_mode: true } });
    fireEvent.click(await screen.findByRole("tab", { name: t("ui.dev.logging") }));

    const keep = (await screen.findByLabelText(t("ui.dev.log_keep_days"))) as HTMLInputElement;
    fireEvent.change(keep, { target: { value: "1000" } });
    await screen.findByDisplayValue("365");
    await waitFor(() => expect(core.state.snap!.settings.dev_log.keep_days).toBe(365));

    fireEvent.change(screen.getByLabelText(t("ui.dev.log_always")), { target: { value: "trace" } });
    await screen.findByText(t("ui.dev.log_trace_warn"));
    fireEvent.click(screen.getByRole("switch", { name: new RegExp(t("ui.dev.log_health")) }));
    await screen.findByRole("switch", { name: new RegExp(t("ui.dev.log_health")), checked: true });
    const sent = calls.filter((c) => c.method === "settings.set").pop()!.params as { settings: { dev_log: unknown } };
    expect(sent.settings.dev_log).toMatchObject({ keep_days: 365, always_verbose: "trace", health_every_interval: true });

    fireEvent.input(screen.getByLabelText(t("ui.dev.mark_note")), { target: { value: "stutter" } });
    fireEvent.click(screen.getByRole("button", { name: t("ui.dev.mark_add") }));
    await screen.findByText(t("ui.dev.mark_done", 1));
    expect(calls.some((c) => c.method === "diag.mark" && (c.params as { note: string }).note === "stutter")).toBe(true);

    fireEvent.click(screen.getByRole("button", { name: t("ui.dev.log_reset") }));
    await screen.findByDisplayValue("7");
    await waitFor(() => expect(core.state.snap!.settings.dev_log.always_verbose).toBeNull());
  });
});
