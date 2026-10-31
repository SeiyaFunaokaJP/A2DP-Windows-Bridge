import { cleanup, fireEvent, render, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "../app/App";
import { FALLBACK_NOTE_MS } from "../components/stream";
import { setLanguage, t } from "../i18n";
import { MockCore, type MockOptions } from "../ipc/mock";
import { createCore } from "../ipc/store";
import { endSettle } from "./Codecs";

let mock: MockCore | null = null;

function start(hash: string, opts: MockOptions = {}) {
  location.hash = hash;
  const calls: { method: string; params: unknown }[] = [];
  mock = new MockCore({ speed: 0, ...opts, onCall: (method, params) => calls.push({ method, params }) });
  const core = createCore(mock);
  const view = render(() => <App core={core} />);
  return { core, calls, view };
}

afterEach(async () => {
  cleanup();
  endSettle();
  mock?.dispose();
  mock = null;
  await setLanguage("en");
});

/** A device's card on Home (its button is "Connect" on every card) */
const homeCard = (name: string) => screen.findByRole("group", { name });

describe("Home", () => {
  it("connects and disconnects with the device's own button", async () => {
    const { calls } = start("#/home");
    const wh = await homeCard("WH-1000XM5");
    const xb = await homeCard("SRS-XB13");
    fireEvent.click(within(wh).getByRole("button", { name: "Connect" }));
    // The mock goes connecting -> streaming
    const disconnect = await within(wh).findByRole("button", { name: "Disconnect" });
    expect(calls.find((c) => c.method === "stream.start")?.params).toEqual({ profile: "WH-1000XM5 (LDAC)" });
    await waitFor(() => expect(wh).toHaveTextContent("990 kbps"));
    // The badge on Home has the bit depth too, on the line below the codec (issue #86)
    expect(wh.querySelector(".codec-badge")).toHaveTextContent("24-bit");
    // The codec is chosen right on the card, by name only (issue #86)
    expect(within(wh).getByRole("combobox", { name: "Codec" })).toHaveDisplayValue("LDAC");
    expect(within(wh).getByRole("status")).toHaveTextContent(/^Connected$/);
    // The other device: ready, not connected, its own Connect
    expect(xb).toHaveTextContent("Disconnected");
    expect(xb).not.toHaveTextContent("—");
    expect(within(xb).getByRole("button", { name: "Connect" })).toBeInTheDocument();
    // The volume is the device's own; the other card keeps its slider,
    // disabled, so the cards stay alike (issue #86)
    expect(within(wh).getByRole("slider", { name: "Volume" })).not.toBeDisabled();
    expect(within(xb).getByRole("slider", { name: "Volume" })).toBeDisabled();
    expect(within(xb).queryByRole("combobox", { name: "Codec" })).toBeNull();
    fireEvent.click(disconnect);
    await within(wh).findByRole("button", { name: "Connect" });
    expect(calls.some((c) => c.method === "stream.stop")).toBe(true);
  });

  it("mutes and unmutes with the speaker, as Windows does (issue #86)", async () => {
    const { calls } = start("#/home");
    const wh = await homeCard("WH-1000XM5");
    fireEvent.click(within(wh).getByRole("button", { name: "Connect" }));
    await within(wh).findByRole("button", { name: "Disconnect" });
    const slider = within(wh).getByRole("slider", { name: "Volume" }) as HTMLInputElement;
    await waitFor(() => expect(Number(slider.value)).toBeGreaterThan(0));
    const before = Number(slider.value);
    const volumes = () => calls.filter((c) => c.method === "stream.set_volume").map((c) => (c.params as { volume: number }).volume);
    fireEvent.click(within(wh).getByRole("button", { name: "Mute" }));
    await waitFor(() => expect(volumes().at(-1)).toBe(0));
    const unmute = await within(wh).findByRole("button", { name: "Unmute" });
    expect(unmute).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(unmute);
    await waitFor(() => expect(volumes().at(-1)).toBeGreaterThan(0));
    await waitFor(() => expect(Number(slider.value)).toBe(before));
    // Not on the card of a device not connected
    const xb = await homeCard("SRS-XB13");
    expect(within(xb).getByRole("button", { name: "Mute" })).toBeDisabled();
  });

  it("shows the battery as unknown, with the reason on hover, when the headphones refuse to report it", async () => {
    start("#/home", { scenario: "battery_refused" });
    const wh = await homeCard("WH-1000XM5");
    fireEvent.click(within(wh).getByRole("button", { name: "Connect" }));
    await within(wh).findByRole("button", { name: "Disconnect" });
    const battery = await waitFor(() => {
      const b = wh.querySelector(".battery");
      expect(b).not.toBeNull();
      return b!;
    });
    expect(battery.getAttribute("aria-label")).toMatch(/^Battery unknown/);
    expect(battery.getAttribute("title")).toMatch(/Disconnect them from other devices/);
    expect(battery).not.toHaveTextContent("%");
  });

  it("shows the link quality, not a codec card, the link latency nor the capture source (issue #86)", async () => {
    start("#/home");
    await homeCard("WH-1000XM5");
    const main = within(screen.getByRole("main"));
    expect(await main.findByRole("region", { name: t("quality.title") })).toBeInTheDocument();
    // The codec is on the device's card
    expect(main.queryByRole("region", { name: t("quality.codec") })).toBeNull();
    expect(main.queryByText("Link latency")).toBeNull();
    expect(main.queryByLabelText(t("capture.mode"))).toBeNull();
  });

  it("switches to another device with its Connect (one device streams at a time)", async () => {
    const { calls } = start("#/home");
    const wh = await homeCard("WH-1000XM5");
    const xb = await homeCard("SRS-XB13");
    fireEvent.click(within(wh).getByRole("button", { name: "Connect" }));
    await within(wh).findByRole("button", { name: "Disconnect" });
    fireEvent.click(within(xb).getByRole("button", { name: "Connect" }));
    await within(xb).findByRole("button", { name: "Disconnect" });
    expect(calls.filter((c) => c.method === "stream.start").map((c) => c.params)).toEqual([
      { profile: "WH-1000XM5 (LDAC)" },
      { profile: "Desk speaker (AAC)" },
    ]);
    // stream.start stops the running stream itself
    expect(calls.some((c) => c.method === "stream.stop")).toBe(false);
    expect(within(wh).getByRole("button", { name: "Connect" })).toBeInTheDocument();
  });

  it("gives a device without a profile one (Auto) when connected", async () => {
    const { calls } = start("#/home");
    await homeCard("WH-1000XM5");
    await mock!.call("device.rename", { address: "54:2F:04:4B:29:DC", name: "moto buds+" });
    const moto = await homeCard("moto buds+");
    fireEvent.click(within(moto).getByRole("button", { name: "Connect" }));
    await waitFor(() => expect(calls.some((c) => c.method === "stream.start")).toBe(true));
    expect(calls.find((c) => c.method === "profile.save")?.params).toMatchObject({
      profile: { name: "moto buds+ | Auto", codec: "auto", device_address: "54:2F:04:4B:29:DC" },
      original_name: null,
    });
    expect(calls.find((c) => c.method === "stream.start")?.params).toEqual({ profile: "moto buds+ | Auto" });
  });

  it("shows each device, and the codec and its details of the one streaming, in the sidebar on every screen (issue #86)", async () => {
    const { core } = start("#/settings");
    const pills = () => [...document.querySelectorAll(".status-pill")] as HTMLElement[];
    await waitFor(() => expect(pills()).toHaveLength(2));
    const [wh, xb] = pills();
    expect(wh).toHaveTextContent("WH-1000XM5");
    expect(wh).not.toHaveTextContent("AC:80:0A:12:34:56");
    expect(wh).toHaveTextContent("Disconnected");
    expect(xb).toHaveTextContent("SRS-XB13");
    await core.request("stream.start", { profile: "WH-1000XM5 (LDAC)" });
    await waitFor(() => expect(wh.querySelector(".codec-name")).toHaveTextContent(/^LDAC$/));
    // Streaming reads "Connected", as on Home and in the tray (issue #86)
    expect(wh.querySelector(".status-pill-state")).toHaveTextContent(/^Connected$/);
    expect(wh).not.toHaveTextContent("Streaming");
    // Two lines: "LDAC", then "990 kbps 96 kHz 24-bit" in the badge below it
    const badge = wh.querySelector(".codec-line .codec-badge")!;
    expect(badge).not.toHaveTextContent("LDAC");
    expect([...badge.children].map((c) => c.textContent)).toEqual(["990 kbps", expect.stringMatching(/^[\d.]+ kHz$/), "24-bit"]);
    expect(wh).toHaveAccessibleName(/^WH-1000XM5, Connected, LDAC 990 kbps/);
    // The other one: idle, no codec
    expect(xb).toHaveTextContent("Disconnected");
    expect(xb.querySelector(".codec-info")).toBeNull();
  });

  it("says why another codec is used on the codec's line, not the state line (issue #86)", async () => {
    // "long": LDAC chosen, in use elsewhere, aptX HD used
    const { core } = start("#/home", { scenario: "long" });
    await waitFor(() => expect(core.state.snap).toBeTruthy());
    await core.request("stream.start", { profile: core.state.snap!.active_profile! });
    const pill = await waitFor(() => {
      const p = document.querySelector(".status-pill.st-streaming");
      expect(p).not.toBeNull();
      return p as HTMLElement;
    });
    const card = document.querySelector(".home-device.live") as HTMLElement;
    const note = t("ui.codec.fallback_in_use", "LDAC");
    await waitFor(() => expect(pill.querySelector(".codec-name")).toHaveTextContent(`aptX HD${note}`));
    // On Home the codec is chosen on the card: the note beside the choice
    expect(await within(card).findByRole("combobox")).toHaveDisplayValue("aptX HD");
    for (const where of [card, pill]) {
      const name = where.querySelector(".codec-name")!;
      expect(name.querySelector(".icon")).not.toBeNull();
      expect(name.querySelector(".codec-note")).toHaveTextContent(note);
    }
    expect(within(card).getByRole("status")).toHaveTextContent(new RegExp(`^${t("status.connected")}$`));
    expect(pill.querySelector(".status-pill-state")).toHaveTextContent(new RegExp(`^${t("status.connected")}$`));
    // For a few seconds only, then the icon alone, its tooltip saying why
    await waitFor(() => expect(card.querySelector(".codec-note")).toBeNull(), { timeout: FALLBACK_NOTE_MS + 2000 });
    for (const where of [card, pill]) {
      expect(where.querySelector(".codec-note")).toBeNull();
      expect(where.querySelector(".codec-name .icon")).not.toBeNull();
    }
  }, 15000);

  it("chooses the codec on the card of the device in use, by name only (issue #86)", async () => {
    const { calls, core } = start("#/home");
    const wh = await homeCard("WH-1000XM5");
    expect(screen.queryByRole("heading", { name: "Output Source" })).toBeNull();
    // Not on a device that is not connected
    expect(screen.queryByRole("combobox", { name: "Codec" })).toBeNull();
    fireEvent.click(within(wh).getByRole("button", { name: "Connect" }));
    const select = (await within(wh).findByRole("combobox", { name: "Codec" })) as HTMLSelectElement;
    // The codecs the device supports in its order, no Auto-select entry
    expect([...select.options].map((o) => o.value)).toEqual(["ldac", "aac", "sbc"]);
    expect(select.value).toBe("ldac");
    // Saved at once, as a card chosen on Codecs
    fireEvent.change(select, { target: { value: "aac" } });
    await waitFor(() =>
      expect(calls.find((c) => c.method === "profile.save")?.params).toMatchObject({
        profile: { codec: "aac" },
        original_name: "WH-1000XM5 (LDAC)",
      }),
    );
    await waitFor(() => expect(select.value).toBe("aac"));
    // Under Auto-select (turned on on Codecs) it shows the codec streaming,
    // by name, no "Auto-select" (the mock keeps streaming AAC)
    await core.request("settings.set", { settings: { ...core.state.snap!.settings, auto_codec: true } });
    await waitFor(() => expect(core.state.snap!.settings.auto_codec).toBe(true));
    expect(select.selectedOptions[0]).toHaveTextContent(/^AAC$/);
    // A codec chosen under Auto-select turns it off first
    const before = calls.length;
    fireEvent.change(select, { target: { value: "sbc" } });
    const after = () => calls.slice(before).filter((c) => c.method === "settings.set" || c.method === "profile.save");
    await waitFor(() => expect(after().map((c) => c.method)).toEqual(["settings.set", "profile.save"]));
    await waitFor(() => expect(select.value).toBe("sbc"));
    expect(select.selectedOptions[0]).toHaveTextContent(/^SBC$/);
    expect(after()[0].params).toMatchObject({ settings: { auto_codec: false } });
    expect(after()[1].params).toMatchObject({ profile: { codec: "sbc" } });
  });

  it("asks to exit the other BlueCaster (v1, or another v2) while it runs", async () => {
    start("#/home");
    await homeCard("WH-1000XM5");
    expect(screen.queryByText("Another BlueCaster is running")).toBeNull();
    mock!.setV1Running(true);
    expect(await screen.findByText("Another BlueCaster is running")).toBeInTheDocument();
    mock!.setV1Running(false);
    await waitFor(() => expect(screen.queryByText("Another BlueCaster is running")).toBeNull());
  });

  it("shows the cause of an error and a fix", async () => {
    start("#/home", { scenario: "error" });
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("The device did not answer");
    expect(within(alert).getByRole("button", { name: "Try again" })).toBeInTheDocument();
  });

  it("connects a device again after an error instead of offering Disconnect", async () => {
    const { calls } = start("#/home", { scenario: "error" });
    const wh = await homeCard("WH-1000XM5");
    expect(wh).toHaveTextContent("Error");
    expect(within(wh).queryByRole("button", { name: "Disconnect" })).toBeNull();
    fireEvent.click(within(wh).getByRole("button", { name: "Connect" }));
    await waitFor(() => expect(calls.some((c) => c.method === "stream.start")).toBe(true));
    expect(calls.some((c) => c.method === "stream.stop")).toBe(false);
  });
});

describe("every screen renders", () => {
  const routes: [string, string][] = [
    ["#/home", "Home"],
    ["#/codecs", "Codecs"],
    // The old Profiles links lead to Codecs
    ["#/profiles", "Codecs"],
    ["#/profiles/new", "Codecs"],
    ["#/devices", "Audio Devices"],
    ["#/diagnostics", "Diagnostics"],
    ["#/adapter", "Adapters"],
    ["#/about", "About"],
    ["#/settings", "Settings"],
  ];
  for (const [hash, heading] of routes) {
    it(hash, async () => {
      start(hash);
      expect(await screen.findByRole("heading", { level: 1, name: heading })).toBeInTheDocument();
      expect(screen.getByRole("navigation")).toBeInTheDocument();
    });
  }

  it("diagnostics tabs", async () => {
    start("#/diagnostics");
    for (const name of ["Connection Flow", "Log", "Link Quality"]) {
      fireEvent.click(await screen.findByRole("tab", { name }));
      expect(screen.getByRole("tab", { name })).toHaveAttribute("aria-selected", "true");
    }
    expect(screen.queryByRole("tab", { name: "Peer Receiver Test" })).toBeNull();
  });

  it("the first-run setup when no adapter is on WinUSB", async () => {
    const { calls } = start("#/home", { scenario: "fresh" });
    expect(await screen.findByRole("heading", { level: 1, name: "Set up BlueCaster" })).toBeInTheDocument();
    mock!.setV1Running(true);
    expect(await screen.findByText("Another BlueCaster is running")).toBeInTheDocument();
    // The setup waits while the other one holds the adapter
    expect(screen.getByRole("button", { name: "Next" })).toBeDisabled();
    mock!.setV1Running(false);
    await waitFor(() => expect(screen.getByRole("button", { name: "Next" })).toBeEnabled());
    fireEvent.click(screen.getByRole("button", { name: "Next" }));
    // Step 2: one explanation and one button, which switches the driver and
    // downloads the firmware (issue #86)
    const setUp = await screen.findByRole("button", { name: "Set up this adapter" });
    expect(screen.getByText(/Windows stops using this adapter/)).toBeInTheDocument();
    expect(screen.getByText("rtl8761bu_fw.bin, rtl8761bu_config.bin")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Replace with WinUSB driver" })).toBeNull();
    // Another BlueCaster started meanwhile: no driver change until it exits
    mock!.setV1Running(true);
    await waitFor(() => expect(setUp).toBeDisabled());
    mock!.setV1Running(false);
    await waitFor(() => expect(setUp).toBeEnabled());
    fireEvent.click(setUp);
    await screen.findByText("The adapter is ready for BlueCaster", {}, { timeout: 5000 });
    const order = calls.map((c) => c.method).filter((m) => m === "adapter.assign_winusb" || m === "adapter.download_firmware");
    expect(order).toEqual(["adapter.assign_winusb", "adapter.download_firmware"]);
    expect(screen.getByRole("button", { name: "Next" })).not.toBeDisabled();
    // Step 3: no connection while another BlueCaster holds the adapter
    fireEvent.click(screen.getByRole("button", { name: "Next" }));
    const connect = (await screen.findAllByRole("button", { name: "Connect" }, { timeout: 5000 }))[0];
    mock!.setV1Running(true);
    await waitFor(() => expect(connect).toBeDisabled());
    mock!.setV1Running(false);
    await waitFor(() => expect(connect).toBeEnabled());
  });

  it("the flyout", async () => {
    start("", { window: "flyout" });
    expect(await screen.findByRole("button", { name: "Connect" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open app" })).toBeInTheDocument();
  });

  it("the flyout looks like Home's card: the device chooser as its title, the codec in two lines (issue #86)", async () => {
    start("", { window: "flyout" });
    const connect = await screen.findByRole("button", { name: "Connect" });
    // The device once: the chooser in the title's place, no "Device:" row
    const chooser = screen.getByRole("combobox", { name: "Device" });
    expect(chooser.closest(".fly-title")).not.toBeNull();
    expect(screen.queryByText("Device:")).toBeNull();
    expect(document.querySelector(".fly-title .device-name")).toBeNull();
    // Hidden by Escape and on losing the focus: no close button
    expect(screen.queryByRole("button", { name: "Close" })).toBeNull();
    expect(screen.queryByText("—")).toBeNull();
    fireEvent.click(connect);
    await screen.findByRole("button", { name: "Disconnect" });
    // The codec is chosen right there, as on Home (issue #86)
    const codec = await screen.findByRole("combobox", { name: "Codec" });
    expect(codec.closest(".fly-title .codec-name")).not.toBeNull();
    expect(codec).toHaveDisplayValue("LDAC");
    expect(screen.getByRole("status")).toHaveTextContent(/^Connected$/);
    // The link quality beside the codec's details
    const line = document.querySelector(".fly-title .codec-line")!;
    expect(line.querySelector(".codec-badge")).toHaveTextContent("990 kbps");
    expect(line.querySelector(".quality")).not.toBeNull();
    // Then the volume, then Disconnect
    const slider = screen.getByRole("slider", { name: "Volume" });
    const disconnect = screen.getByRole("button", { name: "Disconnect" });
    expect(slider.compareDocumentPosition(disconnect) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("in Japanese", async () => {
    const { core } = start("#/settings");
    await screen.findByRole("heading", { level: 1, name: "Settings" });
    await core.request("settings.set", { settings: { ...core.state.snap!.settings, language: "ja" } });
    expect(await screen.findByRole("link", { name: "ホーム" })).toBeInTheDocument();
  });
});

describe("update", () => {
  it("installs at once when nothing streams", async () => {
    start("#/settings", { scenario: "update" });
    fireEvent.click(await screen.findByRole("button", { name: "Update now" }));
    await screen.findByText(/Installing version 2\.0\.0-beta\.1/);
  });

  it("waits for the stream to stop", async () => {
    start("#/settings", { scenario: "update" });
    await mock!.call("stream.start", { profile: "WH-1000XM5 (LDAC)" });
    await waitFor(() => expect(mock!["snap"].stream.state).toBe("streaming"));
    fireEvent.click(await screen.findByRole("button", { name: "Update now" }));
    await screen.findByText(/It is installed when the audio device disconnects/);
    await mock!.call("stream.stop");
    await screen.findByText(/Installing version 2\.0\.0-beta\.1/, {}, { timeout: 3000 });
  });
});
