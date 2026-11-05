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
  it("sets each vendor's protocol and options by vendor id (issue #237)", async () => {
    const { core, calls } = await open();
    await core.request("settings.set", { settings: { ...core.state.snap!.settings, developer_mode: true } });
    fireEvent.click(await screen.findByRole("tab", { name: t("ui.dev.vendors") }));
    // One section a vendor the core lists, OPPO with its own option
    expect(await screen.findByRole("heading", { name: "Huawei" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "OPPO" })).toBeInTheDocument();
    expect(screen.getAllByLabelText(t("ui.dev.vendor_mode"))).toHaveLength(2);
    const sent = () =>
      (calls.filter((c) => c.method === "settings.set").pop()!.params as { settings: { dev_vendors: unknown } }).settings
        .dev_vendors;

    fireEvent.change(screen.getAllByLabelText(t("ui.dev.vendor_mode"))[1], { target: { value: "always" } });
    await screen.findByText(t("ui.dev.active"));
    expect(sent()).toEqual({ oppo: { mode: "always", options: [] } });
    fireEvent.click(screen.getByRole("switch", { name: t("ui.dev.vendor_option.oppo.every_connection") }));
    await waitFor(() => expect(sent()).toEqual({ oppo: { mode: "always", options: ["every_connection"] } }));

    fireEvent.click(screen.getByRole("button", { name: t("ui.dev.reset") }));
    await screen.findByText(t("ui.dev.vendors_intro"));
    expect(sent()).toEqual({});
  });

  it("is hidden until developer mode is on", async () => {
    const { core } = await open();
    expect(screen.queryByRole("link", { name: "Developer" })).toBeNull();
    await core.request("settings.set", { settings: { ...core.state.snap!.settings, developer_mode: true } });
    expect(await screen.findByRole("link", { name: "Developer" })).toBeInTheDocument();
    expect(await screen.findByRole("heading", { level: 1, name: "Developer" })).toBeInTheDocument();
  });

  it("saves the overrides and resets them from the app settings", async () => {
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

    // The send queue limit is clamped the same way; 0 means the default
    const queueWait = screen.getByLabelText(t("ui.dev.queue_wait")) as HTMLInputElement;
    fireEvent.input(queueWait, { target: { value: "5" } });
    fireEvent.change(queueWait, { target: { value: "5" } });
    await screen.findByDisplayValue("50");
    expect(core.state.snap!.settings.dev_overrides.queue_max_wait_ms).toBe(50);
    fireEvent.change(queueWait, { target: { value: "" } });
    await waitFor(() => expect(core.state.snap!.settings.dev_overrides.queue_max_wait_ms).toBe(0));

    // One reset for the whole screen, in the app settings
    expect(screen.queryByRole("button", { name: t("ui.dev.reset") })).toBeNull();
    fireEvent.click(screen.getByRole("tab", { name: t("ui.dev.app_settings") }));
    const reset = await screen.findByRole("button", { name: t("ui.dev.reset_all") });
    expect(reset).toBeEnabled();
    fireEvent.click(reset);
    await waitFor(() => expect(reset).toBeDisabled());
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

describe("chip tools", () => {
  it("runs a read-only script, and asks before lines that may change the chip", async () => {
    const { core, calls } = await open();
    await core.request("settings.set", { settings: { ...core.state.snap!.settings, developer_mode: true } });
    fireEvent.click(await screen.findByRole("tab", { name: "Chip" }));
    const box = (await screen.findByRole("textbox", { name: "Chip script" })) as HTMLTextAreaElement;
    expect(box.value).toContain("hci 1001");
    const run = screen.getByRole("button", { name: "Run" });
    // The example only reads: it runs at once
    await waitFor(() => expect(run).toBeEnabled());
    fireEvent.click(run);
    expect(await screen.findByText("2 of 2 lines done")).toBeInTheDocument();
    const sent = calls.find((c) => c.method === "adapter.run_script")!.params as { allow_writes: boolean };
    expect(sent.allow_writes).toBe(false);
    expect(screen.getByLabelText("Result")).toHaveTextContent("line 2: hci 1001");

    // A vendor command: listed, and run only when allowed
    fireEvent.input(box, { target: { value: "hci fc05 ff\n" } });
    expect(await screen.findByText("These lines may change the chip")).toBeInTheDocument();
    expect(screen.getByText("line 1: hci fc05 ff")).toBeInTheDocument();
    expect(run).toBeDisabled();
    fireEvent.click(screen.getByRole("switch", { name: "Run them" }));
    expect(run).toBeEnabled();
    fireEvent.click(run);
    await waitFor(() =>
      expect(calls.filter((c) => c.method === "adapter.run_script").pop()!.params).toMatchObject({ allow_writes: true }),
    );

    // Errors and denied lines never run
    fireEvent.input(box, { target: { value: "fly away\n" } });
    expect(await screen.findByText("The script has errors")).toBeInTheDocument();
    expect(run).toBeDisabled();
    fireEvent.input(box, { target: { value: "hci fc00 01\n" } });
    expect(await screen.findByText("These lines could change the chip for good and never run")).toBeInTheDocument();
    expect(run).toBeDisabled();
  });
});

describe("Auracast tab", () => {
  it("starts and stops a broadcast", async () => {
    const { core, calls } = await open();
    await core.request("settings.set", { settings: { ...core.state.snap!.settings, developer_mode: true } });
    fireEvent.click(await screen.findByRole("tab", { name: t("ui.dev.auracast") }));
    const start = await screen.findByRole("button", { name: t("receiver.start") });

    // Name and password are checked in bytes
    const name = screen.getByLabelText(t("ui.dev.auracast_name"));
    fireEvent.input(name, { target: { value: "abc" } });
    expect(await screen.findByText(t("ui.dev.auracast_err_length", 4, 32))).toBeInTheDocument();
    expect(start).toBeDisabled();
    fireEvent.input(name, { target: { value: "Living room" } });
    const code = screen.getByLabelText(t("ui.dev.auracast_code"));
    fireEvent.input(code, { target: { value: "ああああああ" } });
    expect(await screen.findByText(t("ui.dev.auracast_err_length", 4, 16))).toBeInTheDocument();
    fireEvent.input(code, { target: { value: "secret" } });
    await waitFor(() => expect(start).toBeEnabled());
    fireEvent.click(screen.getByRole("radio", { name: t("ui.dev.auracast_sq") }));
    fireEvent.click(screen.getByRole("radio", { name: t("ui.dev.auracast_mono") }));

    fireEvent.click(start);
    expect(await screen.findByText(t("status.broadcasting"))).toBeInTheDocument();
    expect(calls.find((c) => c.method === "stream.start_broadcast")!.params).toEqual({
      name: "Living room",
      code: "secret",
      quality: "sq",
      mono: true,
      source: "tone",
    });
    expect(screen.getByText(/1 ch over 1 BIS, 24_2_2/)).toBeInTheDocument();
    expect(name).toBeDisabled();

    fireEvent.click(screen.getByRole("button", { name: t("receiver.stop") }));
    expect(await screen.findByRole("button", { name: t("receiver.start") })).toBeEnabled();
    expect(calls.some((c) => c.method === "stream.stop")).toBe(true);
    expect(screen.queryByText(t("status.broadcasting"))).toBeNull();
  });

  it("shows why a broadcast did not start", async () => {
    const { core } = await open();
    // The core refuses a broadcast while developer mode is off
    await expect(core.request("stream.start_broadcast", { name: "A2DPWB", quality: "hq", source: "tone" })).rejects.toMatchObject({
      code: "developer.off",
    });
    await core.request("settings.set", { settings: { ...core.state.snap!.settings, developer_mode: true } });
    fireEvent.click(await screen.findByRole("tab", { name: t("ui.dev.auracast") }));
    mock!.failNextConnect("error.btstack_init");
    fireEvent.click(await screen.findByRole("button", { name: t("receiver.start") }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent(t("error.btstack_init")));
    expect(screen.getByRole("button", { name: t("receiver.start") })).toBeEnabled();
  });
});
