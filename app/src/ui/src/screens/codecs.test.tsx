// The Codecs screen (issue #86, it replaced the profile editor: v1's
// profile dialog) and the Audio Devices screen: saved devices renamed and
// removed, the codecs each device offers and their settings, the capture
// device's format, the packet size

import { cleanup, fireEvent, render, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "../app/App";
import { setLanguage, t } from "../i18n";
import { MockCore, type MockScenario } from "../ipc/mock";
import { createCore } from "../ipc/store";
import { CARD_ORDER } from "../lib/codecs";
import { endSettle } from "./Codecs";

let mock: MockCore | null = null;

function start(hash: string, scenario: MockScenario = "normal") {
  location.hash = hash;
  const calls: { method: string; params: unknown }[] = [];
  mock = new MockCore({ scenario, speed: 0, onCall: (method, params) => calls.push({ method, params }) });
  const core = createCore(mock);
  render(() => <App core={core} />);
  return { calls, core };
}

/** A device's section on the Codecs screen */
const section = (name: string) => screen.findByRole("region", { name });
/** The cards can be chosen again (the last choice saved) */
const settled = (where: HTMLElement) =>
  waitFor(() => expect(within(where).getAllByRole("radio").every((r) => !(r as HTMLInputElement).disabled)).toBe(true));
/** The screen itself (the sidebar names the devices too) */
const main = () => within(screen.getByRole("main"));
/** "Auto-select the codec with the highest quality", for every device */
const autoName = (n: string) => n.startsWith(t("ui.codecs.auto_select"));
const autoSwitch = () => main().getByRole("switch", { name: autoName });
/** "Show only supported codecs": a switch, on by default (issue #86) */
const hideSwitch = () => main().getByRole("switch", { name: (n) => n.startsWith(t("ui.profile.codecs_hide")) });
/** The codec cards' names in a device's section, in order */
const cardNames = (where: HTMLElement) => [...where.querySelectorAll(".codec-card-name")].map((n) => n.textContent);
/** "Disable Auto-select?" (issue #86 round 8) */
const autoDialog = () => screen.findByRole("dialog", { name: t("ui.codecs.auto_dialog_title") });
/** The profiles saved, in order */
const saves = (calls: { method: string; params: unknown }[]) =>
  calls
    .filter((c) => c.method === "profile.save")
    .map((c) => c.params as { profile: Record<string, unknown>; original_name: string | null });

afterEach(async () => {
  cleanup();
  endSettle();
  mock?.dispose();
  mock = null;
  localStorage.clear();
  await setLanguage("en");
});

describe("the Codecs screen", () => {
  it("lists each saved device with its codec cards, the codec in use chosen", async () => {
    start("#/codecs");
    expect(await screen.findByRole("heading", { level: 1, name: t("ui.nav.codecs") })).toBeInTheDocument();
    const wh = await section("WH-1000XM5");
    // The name only: the address is on Audio Devices (issue #86)
    expect(wh).not.toHaveTextContent("AC:80:0A:12:34:56");
    expect(wh).toHaveTextContent(t("device.tag_saved"));
    expect(within(wh).getByRole("radio", { name: /LDAC/ })).toBeChecked();
    // No Auto card: Auto is one switch for every device (issue #86), off
    // here, not the device's own
    expect(within(wh).queryByRole("radio", { name: /Auto/ })).toBeNull();
    expect(autoSwitch()).not.toBeChecked();
    expect(within(wh).queryByRole("switch", { name: autoName })).toBeNull();
    const xb = await section("SRS-XB13");
    expect(within(xb).getByRole("radio", { name: /AAC/ })).toBeChecked();
    expect(screen.queryByRole("radio", { name: t("capture.virtual") })).toBeNull();
    // Just the switches (issue #86), Auto-select's with what turning it off
    // allows (round 8); no experimental codecs switch, no "Auto's order"
    // button, nor a way back to the default order while it is the default
    expect(main().queryByRole("button", { name: t("ui.codecs.order_reset") })).toBeNull();
    expect(main().queryAllByRole("checkbox")).toHaveLength(0);
    expect(hideSwitch()).toBeChecked();
    // A line between the device and its codecs
    expect(wh.querySelector(".device-codecs-head")).not.toBeNull();
  });

  it("saves the device's profile when a card is chosen, its name following the codec", async () => {
    const { calls } = start("#/codecs");
    // A device without a profile yet: Auto until one is chosen; with
    // Auto-select off its cards can be chosen
    await mock!.call("device.rename", { address: "54:2F:04:4B:29:DC", name: "moto buds+" });
    const moto = await section("moto buds+");
    expect(within(moto).getByRole("radio", { name: /AAC/ })).not.toBeDisabled();
    // One click on a card chooses it
    fireEvent.click(within(moto).getByRole("radio", { name: /AAC/ }));
    await waitFor(() => expect(saves(calls)).toHaveLength(1));
    expect(saves(calls)[0]).toMatchObject({
      profile: { name: "moto buds+ | AAC", device_address: "54:2F:04:4B:29:DC", codec: "aac" },
      original_name: null,
    });
    await settled(moto);
    fireEvent.click(within(moto).getByRole("radio", { name: /LDAC/ }));
    await waitFor(() => expect(saves(calls)).toHaveLength(2));
    expect(saves(calls)[1]).toMatchObject({
      profile: { name: "moto buds+ | LDAC", codec: "ldac" },
      original_name: "moto buds+ | AAC",
    });
    // A name of the user's own stays
    await settled(moto);
    const wh = await section("WH-1000XM5");
    fireEvent.click(within(wh).getByRole("radio", { name: /AAC/ }));
    await waitFor(() => expect(saves(calls)).toHaveLength(3));
    expect(saves(calls)[2]).toMatchObject({ profile: { name: "WH-1000XM5 (LDAC)", codec: "aac" } });
  });

  it("keeps each device's codec to that device (issue #86: LHDC asked of the EarFun after the moto buds+)", async () => {
    const { calls } = start("#/codecs");
    await mock!.call("device.rename", { address: "54:2F:04:4B:29:DC", name: "moto buds+" });
    const moto = await section("moto buds+");
    // Its codecs are not known yet: LHDC V5 is offered
    fireEvent.click(within(moto).getByRole("radio", { name: /LHDC V5/ }));
    await waitFor(() => expect(saves(calls)).toHaveLength(1));
    expect(saves(calls)[0].profile).toMatchObject({ device_address: "54:2F:04:4B:29:DC", codec: "lhdcv5" });
    await settled(moto);
    // The other devices keep theirs, on screen and in what Connect streams
    const wh = await section("WH-1000XM5");
    expect(within(wh).getByRole("radio", { name: /LDAC/ })).toBeChecked();
    expect(within(await section("SRS-XB13")).getByRole("radio", { name: /AAC/ })).toBeChecked();
    const snap = (await mock!.call("state.snapshot")) as { profiles: { device_address: string; codec: string }[] };
    expect(snap.profiles.map((p) => [p.device_address, p.codec])).toEqual([
      ["AC:80:0A:12:34:56", "ldac"],
      ["00:1A:7D:DA:71:13", "aac"],
      ["54:2F:04:4B:29:DC", "lhdcv5"],
    ]);
    // A new device starts from Auto, not from the codec chosen last
    await mock!.call("device.rename", { address: "70:5A:6F:6D:ED:1F", name: "EarFun Air Pro 4+" });
    const earfun = await section("EarFun Air Pro 4+");
    expect(within(earfun).getByRole("radio", { name: /LHDC V5/ })).not.toBeChecked();
    location.hash = "#/home";
    const card = await screen.findByRole("group", { name: "EarFun Air Pro 4+" });
    fireEvent.click(within(card).getByRole("button", { name: "Connect" }));
    await waitFor(() => expect(saves(calls)).toHaveLength(2));
    expect(saves(calls)[1].profile).toMatchObject({ device_address: "70:5A:6F:6D:ED:1F", codec: "auto" });
  });

  it("brings a codec's settings back when it is chosen again (issue #86: AAC's VBR came back off)", async () => {
    const { calls } = start("#/codecs");
    const wh = await section("WH-1000XM5");
    const dialog = await codecSettings(wh, "codec.aac");
    fireEvent.click(within(dialog).getByRole("switch", { name: t("connection.vbr") }));
    fireEvent.click(within(dialog).getByRole("button", { name: t("modal.ok") }));
    await waitFor(() => expect(saves(calls)).toHaveLength(1));
    expect(saves(calls)[0].profile).toMatchObject({ codec: "aac", abr: true, codec_settings: { ldac: { quality: "hq" } } });
    await settled(wh);
    fireEvent.click(within(wh).getByRole("radio", { name: /LDAC/ }));
    await waitFor(() => expect(saves(calls)).toHaveLength(2));
    expect(saves(calls)[1].profile).toMatchObject({ codec: "ldac", abr: false, codec_settings: { aac: { abr: true } } });
    await settled(wh);
    fireEvent.click(within(wh).getByRole("radio", { name: /AAC/ }));
    await waitFor(() => expect(saves(calls)).toHaveLength(3));
    expect(saves(calls)[2].profile).toMatchObject({ codec: "aac", abr: true });
    expect(saves(calls)[2].profile.codec_settings).not.toHaveProperty("aac");
  });

  it("turns Auto-select on and off for every device, each keeping the codec chosen for it (issue #86)", async () => {
    const { calls } = start("#/codecs");
    // No page-wide fallback option any more: a codec always falls back
    expect(screen.queryByRole("switch", { name: /another supported codec/ })).toBeNull();
    const wh = await section("WH-1000XM5");
    fireEvent.click(within(wh).getByRole("radio", { name: /AAC/ }));
    await waitFor(() => expect(saves(calls)).toHaveLength(1));
    await settled(wh);
    fireEvent.click(autoSwitch());
    await waitFor(() => expect(autoSwitch()).toBeChecked());
    expect(calls.filter((c) => c.method === "settings.set").at(-1)?.params).toMatchObject({
      settings: { auto_codec: true },
    });
    // The WH-1000XM5 supported SBC, AAC and LDAC: Auto picks LDAC, shown
    // on the read-only cards; every device's cards are read-only
    await waitFor(() => expect(within(wh).getByRole("radio", { name: /SBC/ })).toBeDisabled());
    const ldac = within(wh).getByRole("radio", { name: /LDAC/ });
    expect(ldac).toBeDisabled();
    expect(ldac).toBeChecked();
    // The card chosen is enough: no "In use" line (issue #86)
    expect(wh).not.toHaveTextContent(/In use/);
    expect(within(await section("SRS-XB13")).getByRole("radio", { name: /AAC/ })).toBeDisabled();
    // No profile changed: off again, the codec chosen is back
    fireEvent.click(autoSwitch());
    await waitFor(() => expect(within(wh).getByRole("radio", { name: /AAC/ })).toBeChecked());
    expect(within(wh).getByRole("radio", { name: /AAC/ })).not.toBeDisabled();
    expect(saves(calls)).toHaveLength(1);
  });

  it("offers to turn Auto-select off when a card is clicked while it is on (issue #86 round 8)", async () => {
    const { calls } = start("#/codecs");
    const wh = await section("WH-1000XM5");
    fireEvent.click(autoSwitch());
    await waitFor(() => expect(autoSwitch()).toBeChecked());
    const aac = within(wh).getByRole("radio", { name: /AAC/ });
    await waitFor(() => expect(aac).toBeDisabled());
    expect(aac.closest("label")).toHaveClass("ro");
    // Cancel: nothing changes
    fireEvent.click(aac.closest("label")!);
    let dialog = await autoDialog();
    expect(dialog).toHaveTextContent(t("ui.codecs.auto_dialog_text"));
    fireEvent.click(within(dialog).getByRole("button", { name: t("modal.cancel") }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(autoSwitch()).toBeChecked();
    // Its "..." the same
    fireEvent.click(within(wh).getByRole("button", { name: t("ui.profile.codec_settings", t("codec.aac")) }));
    dialog = await autoDialog();
    const before = calls.filter((c) => c.method === "settings.set").length;
    fireEvent.click(within(dialog).getByRole("button", { name: t("ui.codecs.auto_dialog_disable") }));
    await waitFor(() => expect(autoSwitch()).not.toBeChecked());
    const sets = calls.filter((c) => c.method === "settings.set");
    expect(sets).toHaveLength(before + 1);
    expect(sets.at(-1)?.params).toMatchObject({ settings: { auto_codec: false } });
    // Its settings open; no profile changed yet, LDAC still chosen
    expect(await screen.findByRole("dialog", { name: t("ui.profile.codec_settings", t("codec.aac")) })).toBeTruthy();
    expect(saves(calls)).toHaveLength(0);
    expect(within(wh).getByRole("radio", { name: /LDAC/ })).toBeChecked();
    expect(within(wh).getByRole("radio", { name: /AAC/ })).not.toBeDisabled();
  });

  it("chooses the card clicked once Auto-select is turned off (issue #86)", async () => {
    const { calls } = start("#/codecs");
    const wh = await section("WH-1000XM5");
    fireEvent.click(autoSwitch());
    await waitFor(() => expect(autoSwitch()).toBeChecked());
    const aac = within(wh).getByRole("radio", { name: /AAC/ });
    await waitFor(() => expect(aac).toBeDisabled());
    fireEvent.click(aac.closest("label")!);
    fireEvent.click(within(await autoDialog()).getByRole("button", { name: t("ui.codecs.auto_dialog_disable") }));
    await waitFor(() => expect(autoSwitch()).not.toBeChecked());
    await waitFor(() => expect(saves(calls)).toHaveLength(1));
    expect(saves(calls)[0]?.profile).toMatchObject({ codec: "aac" });
    await waitFor(() => expect(within(wh).getByRole("radio", { name: /AAC/ })).toBeChecked());
  });

  it("chooses the best supported codec for an unsupported card, not the next one below it (issue #86)", async () => {
    const { calls } = start("#/codecs");
    const wh = await section("WH-1000XM5");
    fireEvent.click(hideSwitch());
    // AAC first, then aptX HD: not offered by the WH-1000XM5 (SBC, AAC,
    // LDAC): LDAC, the best it offers, not AAC below aptX HD, and no note
    fireEvent.click(within(wh).getByRole("radio", { name: /^AAC/ }));
    await waitFor(() => expect(saves(calls)).toHaveLength(1));
    await waitFor(() => expect(within(wh).getByRole("radio", { name: /^AAC/ })).toBeChecked());
    fireEvent.click(await within(wh).findByRole("radio", { name: /aptX HD/ }));
    await waitFor(() => expect(saves(calls)).toHaveLength(2));
    expect(saves(calls).at(-1)?.profile).toMatchObject({ codec: "ldac" });
    await waitFor(() => expect(within(wh).getByRole("radio", { name: /LDAC/ })).toBeChecked());
    expect(within(wh).getByRole("radio", { name: /aptX HD/ })).not.toBeChecked();
    expect(within(wh).queryByText(t("error.codec_not_supported", "aptX HD"))).toBeNull();
  });

  it("keeps LDAC choosable on earbuds that list it only once asked (issue #30)", async () => {
    const { calls } = start("#/codecs", "huawei");
    const buds = await section("HUAWEI FreeBuds Pro 4");
    // Not "Unsupported", and hidden with no other codec they did not list
    const ldac = within(buds).getByRole("radio", { name: /LDAC/ });
    expect(ldac.closest("label")).toHaveTextContent(t("ui.profile.codec_support_unknown"));
    expect(within(buds).queryByRole("radio", { name: /aptX HD/ })).toBeNull();
    fireEvent.click(ldac);
    await waitFor(() => expect(saves(calls)).toHaveLength(1));
    // LDAC itself, not the best codec they listed
    expect(saves(calls)[0]?.profile).toMatchObject({ codec: "ldac" });
    await waitFor(() => expect(within(buds).getByRole("radio", { name: /LDAC/ })).toBeChecked());
  });

  it("offers every codec, with no Experimental tag (issue #86: the README marks them)", async () => {
    const { calls, core } = start("#/codecs");
    const wh = await section("WH-1000XM5");
    fireEvent.click(hideSwitch());
    // Even with the old setting off
    await core.request("settings.set", { settings: { ...core.state.snap!.settings, experimental_codecs: false } });
    const hd = await within(wh).findByRole("radio", { name: /aptX HD/ });
    for (const name of [/^aptX LL/, /^OpusUnsupported/, /^Opus \(PipeWire\)Unsupported/, /FastStream/]) {
      expect(within(wh).getByRole("radio", { name })).toBeInTheDocument();
    }
    expect(within(wh).queryByRole("radio", { name: /Opus 05/ })).toBeNull();
    expect(wh).not.toHaveTextContent(/Experimental/);
    // Chosen: the device does not support it, LDAC stays (nothing to save)
    fireEvent.click(hd);
    await new Promise((r) => setTimeout(r, 50));
    expect(saves(calls)).toHaveLength(0);
    expect(within(wh).getByRole("radio", { name: /LDAC/ })).toBeChecked();
  });
});

describe("saved devices on the Audio Devices screen", () => {
  it("renames a saved device", async () => {
    const { calls } = start("#/devices");
    fireEvent.click(await screen.findByRole("button", { name: `${t("device.edit_name")}: SRS-XB13` }));
    const dialog = await screen.findByRole("dialog");
    const input = within(dialog).getByLabelText(t("device.enter_name"));
    expect(input).toHaveValue("SRS-XB13");

    // Not empty, no "|": the core refuses those
    fireEvent.input(input, { target: { value: "a|b" } });
    fireEvent.click(within(dialog).getByRole("button", { name: t("modal.save") }));
    await within(dialog).findByText(t("ui.device.err_name"));
    expect(calls.some((c) => c.method === "device.rename")).toBe(false);

    fireEvent.input(input, { target: { value: "Desk speaker" } });
    fireEvent.click(within(dialog).getByRole("button", { name: t("modal.save") }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(calls.find((c) => c.method === "device.rename")?.params).toEqual({
      address: "00:1A:7D:DA:71:13",
      name: "Desk speaker",
    });
    expect(await main().findByText(/Desk speaker/)).toBeInTheDocument();
  });

  it("removes a saved device after asking", async () => {
    const { calls } = start("#/devices");
    fireEvent.click(await screen.findByRole("button", { name: `${t("ui.device.forget")}: SRS-XB13` }));
    const dialog = await screen.findByRole("dialog");
    expect(dialog).toHaveTextContent("00:1A:7D:DA:71:13");
    fireEvent.click(within(dialog).getByRole("button", { name: t("ui.device.forget") }));
    await waitFor(() => expect(main().queryByText(/SRS-XB13/)).toBeNull());
    expect(calls.find((c) => c.method === "device.forget")?.params).toEqual({ address: "00:1A:7D:DA:71:13" });
  });

  it("offers no rename or removal for devices that are not saved, but adding them", async () => {
    const { calls } = start("#/devices");
    fireEvent.click(await screen.findByRole("button", { name: t("connection.scan") }));
    const row = (await screen.findByText(/Galaxy Buds2 Pro/)).closest(".device-row") as HTMLElement;
    expect(within(row).queryByRole("button", { name: new RegExp(t("device.edit_name")) })).toBeNull();
    fireEvent.click(within(row).getByRole("button", { name: t("ui.devices.add") }));
    await waitFor(() => expect(calls.find((c) => c.method === "device.rename")?.params).toMatchObject({ name: "Galaxy Buds2 Pro" }));
    // Saved: it moves up to the saved devices, with its settings (issue #86)
    await waitFor(() =>
      expect(main().getByText(/Galaxy Buds2 Pro/).closest(".saved-devices")).not.toBeNull(),
    );
    const entry = main().getByText(/Galaxy Buds2 Pro/).closest(".device-entry") as HTMLElement;
    expect(within(entry).queryByRole("button", { name: t("ui.devices.add") })).toBeNull();
  });

  it("makes a profile that picks the best codec for a device added without one", async () => {
    const { calls } = start("#/devices");
    fireEvent.click(await screen.findByRole("button", { name: t("connection.scan") }));
    const row = (await screen.findByText(/Galaxy Buds2 Pro/)).closest(".device-row") as HTMLElement;
    fireEvent.click(within(row).getByRole("button", { name: t("ui.devices.add") }));
    await waitFor(() => expect(calls.some((c) => c.method === "profile.save")).toBe(true));
    expect(calls.find((c) => c.method === "profile.save")!.params).toMatchObject({
      profile: { name: "Galaxy Buds2 Pro | Auto", codec: "auto", device_name: "Galaxy Buds2 Pro" },
      original_name: null,
    });
    // ... and connects with it, as pairing does on other systems
    await waitFor(() =>
      expect(calls.find((c) => c.method === "stream.start")?.params).toEqual({ profile: "Galaxy Buds2 Pro | Auto" }),
    );
  });

  it("leaves a running stream alone when a device is added", async () => {
    const { calls } = start("#/devices");
    fireEvent.click(await screen.findByRole("button", { name: t("connection.scan") }));
    const row = (await screen.findByText(/Galaxy Buds2 Pro/)).closest(".device-row") as HTMLElement;
    await mock!.call("stream.start", { profile: "WH-1000XM5 (LDAC)" });
    await waitFor(() => expect(screen.getAllByText(/^Connected$/).length).toBeGreaterThan(0));
    calls.length = 0;
    fireEvent.click(within(row).getByRole("button", { name: t("ui.devices.add") }));
    await waitFor(() => expect(calls.some((c) => c.method === "profile.save")).toBe(true));
    expect(calls.some((c) => c.method === "stream.start")).toBe(false);
  });
});

describe("codec cards", () => {
  it("show what the device offered when it last connected", async () => {
    const { calls } = start("#/codecs");
    const wh = await section("WH-1000XM5");
    const ldac = within(wh).getByRole("radio", { name: /LDAC/ });
    expect(within(wh).queryByText(t("ui.profile.codecs_unknown"))).toBeNull();
    // The ones it did not support hidden (on by default, issue #86): SBC,
    // AAC and LDAC are supported, and need no "Supported" tag then
    expect(within(wh).queryByRole("radio", { name: /aptX HD/ })).toBeNull();
    expect(within(wh).getAllByRole("radio")).toHaveLength(3);
    expect(within(wh).queryByText(t("ui.profile.codec_supported"))).toBeNull();
    const hide = hideSwitch();
    expect(hide).toBeChecked();
    // Shown: with their "Unsupported" tag, the others "Supported"
    fireEvent.click(hide);
    const hd = await within(wh).findByRole("radio", { name: /aptX HD/ });
    expect(ldac.closest("label")).toHaveTextContent(t("ui.profile.codec_supported"));
    // Its mark in green, not the whole tag (issue #86)
    const good = within(ldac.closest("label")!).getByText(t("ui.profile.codec_supported")).closest(".tag")!;
    expect(good).not.toHaveClass("st-good");
    expect(good.querySelector(".icon")).toHaveClass("st-good");
    expect(hd.closest("label")).toHaveTextContent("Unsupported");
    // Its mark in red
    const bad = within(hd.closest("label")!).getByText(t("ui.profile.codec_not_supported")).closest(".tag")!;
    expect(bad).not.toHaveClass("st-bad");
    expect(bad.querySelector(".icon")).toHaveClass("st-bad");
    expect(hide).not.toBeChecked();

    // A codec it does not offer: LDAC, the best it offers, stays chosen
    fireEvent.click(hd);
    await new Promise((r) => setTimeout(r, 50));
    expect(calls.some((c) => c.method === "profile.save")).toBe(false);
    expect(ldac).toBeChecked();
  });

  it("say it is not known before the first connection", async () => {
    start("#/codecs");
    const xb = await section("SRS-XB13");
    expect(within(xb).getByText(t("ui.profile.codecs_unknown"))).toBeInTheDocument();
    expect(within(xb).queryByText(t("ui.profile.codec_supported"))).toBeNull();
    expect(within(xb).queryByText(t("ui.profile.codec_not_supported"))).toBeNull();
    // Each card says so, in neither green nor red
    const aac = within(xb).getByRole("radio", { name: /AAC/ }).closest("label")!;
    const tag = within(aac).getByText(t("ui.profile.codec_support_unknown")).closest(".tag")!;
    expect(tag).not.toHaveClass("st-good");
    expect(tag).not.toHaveClass("st-bad");
  });

  it("are put in the device's own order with Alt+arrow keys, kept in its profile (issue #86 round 8)", async () => {
    const { calls } = start("#/codecs");
    const wh = await section("WH-1000XM5");
    // Highest sound quality first
    expect(cardNames(wh)).toEqual([t("codec.ldac"), t("codec.aac"), t("codec.sbc")]);
    const aac = within(wh).getByRole("radio", { name: /AAC/ });
    expect(aac).toHaveAccessibleDescription(t("ui.codecs.order_keys"));
    // AAC before LDAC
    aac.focus();
    fireEvent.keyDown(aac, { key: "ArrowLeft", altKey: true });
    await waitFor(() => expect(saves(calls)).toHaveLength(1));
    const order = saves(calls)[0].profile.codec_order as string[];
    expect(order.indexOf("aac")).toBeLessThan(order.indexOf("ldac"));
    // Every card once, Opus 05 and FastStream too
    expect([...order].sort()).toEqual([...CARD_ORDER].sort());
    // Only the order changed: the codec chosen stays
    expect(saves(calls)[0].profile).toMatchObject({ codec: "ldac", name: "WH-1000XM5 (LDAC)" });
    await waitFor(() => expect(cardNames(wh)).toEqual([t("codec.aac"), t("codec.ldac"), t("codec.sbc")]));
    expect(wh).toHaveTextContent(t("ui.codecs.moved", t("codec.aac"), 1, 3));
    // It keeps the focus, and goes no further than the first place
    await waitFor(() => expect(document.activeElement).toBe(within(wh).getByRole("radio", { name: /AAC/ })));
    fireEvent.keyDown(within(wh).getByRole("radio", { name: /AAC/ }), { key: "ArrowUp", altKey: true });
    // An arrow key alone is the radio group's
    fireEvent.keyDown(within(wh).getByRole("radio", { name: /SBC/ }), { key: "ArrowLeft" });
    expect(saves(calls)).toHaveLength(1);
    // The other devices keep the default order
    expect(cardNames(await section("SRS-XB13"))[0]).toBe(t("codec.lhdc_v5"));

    // An unsupported card chooses the first card the device supports (AAC
    // for aptX HD)
    fireEvent.click(hideSwitch());
    fireEvent.click(await within(wh).findByRole("radio", { name: /aptX HD/ }));
    await waitFor(() => expect(saves(calls)).toHaveLength(2));
    expect(saves(calls)[1].profile).toMatchObject({ codec: "aac", codec_order: order });
    await waitFor(() => expect(within(wh).getByRole("radio", { name: /AAC/ })).toBeChecked());

    // Back to the default order: saved as none
    await settled(wh);
    fireEvent.click(main().getByRole("button", { name: t("ui.codecs.order_reset") }));
    await waitFor(() => expect(saves(calls)).toHaveLength(3));
    expect(saves(calls)[2].profile.codec_order).toEqual([]);
    await waitFor(() => expect(main().queryByRole("button", { name: t("ui.codecs.order_reset") })).toBeNull());
  });

  it("are dragged into the device's order with the pointer (issue #86 round 8)", async () => {
    const { calls } = start("#/codecs");
    const wh = await section("WH-1000XM5");
    const sbc = within(wh).getByRole("radio", { name: /SBC/ });
    const ldac = within(wh).getByRole("radio", { name: /LDAC/ });
    const grid = wh.querySelector(".codec-grid")!;
    const at = document.elementFromPoint;
    // jsdom has no layout: the card under the pointer is LDAC's
    document.elementFromPoint = () => ldac;
    try {
      fireEvent.pointerDown(sbc, { button: 0, pointerId: 1, clientX: 300, clientY: 10 });
      fireEvent.pointerMove(grid, { pointerId: 1, clientX: 20, clientY: 10 });
      // Shown as it goes
      await waitFor(() => expect(cardNames(wh)[0]).toBe(t("codec.sbc")));
      expect(sbc.closest(".codec-cell")).toHaveClass("dragging");
      expect(saves(calls)).toHaveLength(0);
      // Let go over itself, now where LDAC was
      document.elementFromPoint = () => sbc;
      fireEvent.pointerUp(grid, { pointerId: 1, clientX: 20, clientY: 10 });
      // The click that ends it does not choose SBC
      fireEvent.click(sbc);
    } finally {
      document.elementFromPoint = at;
    }
    await waitFor(() => expect(saves(calls)).toHaveLength(1));
    expect(saves(calls)[0].profile).toMatchObject({ codec: "ldac" });
    const order = saves(calls)[0].profile.codec_order as string[];
    expect(order.indexOf("sbc")).toBeLessThan(order.indexOf("ldac"));
    expect(sbc.closest(".codec-cell")).not.toHaveClass("dragging");
    // A click without moving still chooses
    await settled(wh);
    fireEvent.pointerDown(sbc, { button: 0, pointerId: 2, clientX: 5, clientY: 5 });
    fireEvent.pointerUp(sbc, { pointerId: 2, clientX: 5, clientY: 5 });
    fireEvent.click(sbc);
    await waitFor(() => expect(saves(calls)).toHaveLength(2));
    expect(saves(calls)[1].profile).toMatchObject({ codec: "sbc" });
  });

  it("offer no Reset when the cards shown are back in the default order (issue #86)", async () => {
    const { calls } = start("#/codecs");
    const wh = await section("WH-1000XM5");
    const reset = () => main().queryByRole("button", { name: t("ui.codecs.order_reset") });
    // AAC before LDAC, then back after it: the default order, saved as none
    fireEvent.keyDown(within(wh).getByRole("radio", { name: /AAC/ }), { key: "ArrowLeft", altKey: true });
    await waitFor(() => expect(saves(calls)).toHaveLength(1));
    await waitFor(() => expect(reset()).not.toBeNull());
    fireEvent.keyDown(within(wh).getByRole("radio", { name: /AAC/ }), { key: "ArrowRight", altKey: true });
    await waitFor(() => expect(saves(calls)).toHaveLength(2));
    expect(saves(calls)[1].profile.codec_order).toEqual([]);
    await waitFor(() => expect(reset()).toBeNull());
    expect(cardNames(wh)).toEqual([t("codec.ldac"), t("codec.aac"), t("codec.sbc")]);
  });

  it("are in the default order and cannot be moved under Auto-select, which ignores the device's order", async () => {
    const { calls, core } = start("#/codecs");
    const wh = await section("WH-1000XM5");
    const p = core.state.snap!.profiles.find((x) => x.device_address === "AC:80:0A:12:34:56")!;
    await core.request("profile.save", {
      profile: { ...JSON.parse(JSON.stringify(p)), codec: "auto", codec_order: ["sbc", "aac"] },
      original_name: p.name,
    });
    // Auto-select off, no codec chosen: Auto picks the first card offered
    await waitFor(() => expect(cardNames(wh)).toEqual([t("codec.sbc"), t("codec.aac"), t("codec.ldac")]));
    expect(within(wh).getByRole("radio", { name: /SBC/ })).toBeChecked();
    calls.length = 0;
    fireEvent.click(autoSwitch());
    // On: highest sound quality first, LDAC picked
    await waitFor(() => expect(cardNames(wh)).toEqual([t("codec.ldac"), t("codec.aac"), t("codec.sbc")]));
    expect(within(wh).getByRole("radio", { name: /LDAC/ })).toBeChecked();
    expect(main().queryByRole("button", { name: t("ui.codecs.order_reset") })).toBeNull();
    const aac = within(wh).getByRole("radio", { name: /AAC/ });
    expect(aac).not.toHaveAccessibleDescription();
    fireEvent.keyDown(aac, { key: "ArrowLeft", altKey: true });
    fireEvent.pointerDown(aac, { button: 0, pointerId: 1, clientX: 300, clientY: 10 });
    fireEvent.pointerMove(wh.querySelector(".codec-grid")!, { pointerId: 1, clientX: 20, clientY: 10 });
    fireEvent.pointerUp(wh.querySelector(".codec-grid")!, { pointerId: 1 });
    expect(saves(calls)).toHaveLength(0);
    expect(cardNames(wh)[0]).toBe(t("codec.ldac"));
  });
});

describe("the device's activity (issue #86 round 8)", () => {
  it("shows a dot beside each device's name, green for the one streaming", async () => {
    start("#/codecs");
    const wh = await section("WH-1000XM5");
    const xb = await section("SRS-XB13");
    const dot = (where: HTMLElement) => where.querySelector(".device-codecs-head > .dot")!;
    expect(dot(wh)).toHaveClass("st-idle");
    expect(within(wh).getByRole("img", { name: t("status.idle") })).toBe(dot(wh));
    await mock!.call("stream.start", { profile: "WH-1000XM5 (LDAC)" });
    await waitFor(() => expect(dot(wh)).toHaveClass("st-streaming"));
    expect(within(wh).getByRole("img", { name: t("status.connected") })).toBe(dot(wh));
    expect(dot(xb)).toHaveClass("st-idle");
  });

  it("waits for the stream started again after the codec of the device streaming changed (issue #86)", async () => {
    const { calls, core } = start("#/codecs");
    const wh = await section("WH-1000XM5");
    await mock!.call("stream.start", { profile: "WH-1000XM5 (LDAC)" });
    await waitFor(() => expect(wh.querySelector(".device-codecs-head > .dot")).toHaveClass("st-streaming"));
    fireEvent.click(within(wh).getByRole("radio", { name: /AAC/ }));
    await waitFor(() => expect(saves(calls)).toHaveLength(1));
    // Every card waits, with the reason said
    expect(await within(wh).findByText(t("ui.codecs.settling"))).toBeInTheDocument();
    expect(within(wh).getAllByRole("radio").every((r) => (r as HTMLInputElement).disabled)).toBe(true);
    expect(within(wh).getByRole("button", { name: t("ui.profile.codec_settings", t("codec.sbc")) })).toBeDisabled();
    fireEvent.click(within(wh).getByRole("radio", { name: /SBC/ }));
    expect(saves(calls)).toHaveLength(1);
    // Then, once it streams again, they can be chosen again
    await waitFor(() => expect(within(wh).queryByText(t("ui.codecs.settling"))).toBeNull(), { timeout: 5000 });
    expect(core.state.snap!.stream.state).toBe("streaming");
    await settled(wh);
  }, 10000);

  it("does not wait after a codec chosen for a device not streaming", async () => {
    const { calls } = start("#/codecs");
    const wh = await section("WH-1000XM5");
    fireEvent.click(within(wh).getByRole("radio", { name: /AAC/ }));
    await waitFor(() => expect(saves(calls)).toHaveLength(1));
    await settled(wh);
    expect(within(wh).queryByText(t("ui.codecs.settling"))).toBeNull();
  });
});

/** Opens a codec's settings ("...") in a device's section and returns its dialog */
async function codecSettings(where: HTMLElement, label: string) {
  fireEvent.click(await within(where).findByRole("button", { name: t("ui.profile.codec_settings", t(label)) }));
  return screen.findByRole("dialog");
}

describe("codec settings", () => {
  it("LHDC V5 offers 192 kHz and greys out the quality while ABR is on", async () => {
    start("#/codecs");
    // A device whose codecs are not known yet: all of them offered
    const xb = await section("SRS-XB13");
    const dialog = await codecSettings(xb, "codec.lhdc_v5");
    const rate = within(dialog).getByLabelText(t("connection.sample_rate"));
    expect(within(rate).getByRole("option", { name: /192/ })).toBeInTheDocument();
    const quality = within(dialog).getByLabelText(t("connection.quality"));
    expect(quality).not.toBeDisabled();
    fireEvent.click(within(dialog).getByRole("switch", { name: t("connection.abr") }));
    await waitFor(() => expect(quality).toBeDisabled());
    fireEvent.click(within(dialog).getByRole("switch", { name: t("connection.abr") }));
    await waitFor(() => expect(quality).not.toBeDisabled());
    fireEvent.click(within(dialog).getByRole("button", { name: t("modal.cancel") }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());

    // LDAC's ABR keeps the quality, and has no 192 kHz
    const ldac = await codecSettings(xb, "codec.ldac");
    fireEvent.click(within(ldac).getByRole("switch", { name: t("connection.abr") }));
    expect(within(ldac).getByLabelText(t("connection.quality"))).not.toBeDisabled();
    expect(within(within(ldac).getByLabelText(t("connection.sample_rate"))).queryByRole("option", { name: /192/ })).toBeNull();
  });

  it("OK chooses the codec with its settings, Cancel changes nothing", async () => {
    const { calls } = start("#/codecs");
    const wh = await section("WH-1000XM5");
    let dialog = await codecSettings(wh, "codec.aac");
    expect(within(dialog).getByText(t("ui.profile.codec_settings_apply", t("codec.aac")))).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole("switch", { name: t("connection.vbr") }));
    fireEvent.click(within(dialog).getByRole("button", { name: t("modal.cancel") }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(within(wh).getByRole("radio", { name: /LDAC/ })).toBeChecked();
    expect(calls.some((c) => c.method === "profile.save")).toBe(false);

    dialog = await codecSettings(wh, "codec.aac");
    fireEvent.click(within(dialog).getByRole("switch", { name: t("connection.vbr") }));
    fireEvent.click(within(dialog).getByRole("button", { name: t("modal.ok") }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await waitFor(() => expect(saves(calls)).toHaveLength(1));
    expect(saves(calls)[0].profile).toMatchObject({ codec: "aac", quality: "hq", abr: true });
    await waitFor(() => expect(within(wh).getByRole("radio", { name: /AAC/ })).toBeChecked());
  });
});

describe("codec settings within what the device lists (issue #86: moto buds+)", () => {
  it("LHDC V5 offers only the rates the device lists", async () => {
    start("#/codecs", "capped");
    const dialog = await codecSettings(await section("moto buds+"), "codec.lhdc_v5");
    const rate = within(dialog).getByLabelText(t("connection.sample_rate"));
    expect(within(rate).getAllByRole("option").map((o) => o.textContent)).toEqual([t("codec.auto"), "44100 Hz", "48000 Hz"]);
    // High, Medium-High (issue #203), Medium, Low (issue #86)
    const quality = within(dialog).getByLabelText(t("connection.quality"));
    expect(within(quality).getAllByRole("option").map((o) => o.textContent)).toEqual([
      `${t("quality.high")} (1000 kbps)`,
      `${t("quality.medium_high")} (900 kbps)`,
      `${t("quality.medium")} (500 kbps)`,
      `${t("quality.low")} (400 kbps)`,
    ]);
  });

  it("AAC shows its one quality at the device's largest bit rate", async () => {
    start("#/codecs", "capped");
    const dialog = await codecSettings(await section("moto buds+"), "codec.aac");
    const quality = within(dialog).getByLabelText(t("connection.quality"));
    // Not a choice: High is the highest the device takes (0x020000 bps)
    expect(quality.tagName).toBe("OUTPUT");
    expect(quality).toHaveTextContent(`${t("quality.high")} (131 kbps)`);
  });

  it("SBC's bitpool stops at the device's largest, and says so", async () => {
    const { calls } = start("#/codecs", "capped");
    const dialog = await codecSettings(await section("moto buds+"), "codec.sbc");
    const bitpool = within(dialog).getByRole("slider", { name: t("ui.profile.sbc_bitpool_max") });
    expect(bitpool).toHaveAttribute("max", "39");
    expect(bitpool).toHaveAccessibleDescription(new RegExp(t("ui.profile.sbc_bitpool_device", 2, 39)));
    // Auto: where the quality puts it, the device's largest here
    expect(bitpool).toHaveValue("39");
    expect(bitpool).toHaveAttribute("aria-valuetext", t("ui.profile.sbc_bitpool_auto", 39));
    // Never past the device's largest
    fireEvent.input(bitpool, { target: { value: "53" } });
    expect(bitpool).toHaveValue("39");
    // Bitpool 39, 16 blocks, 8 subbands, joint stereo at 48 kHz
    await waitFor(() => expect(within(dialog).getByRole("status")).toHaveTextContent(t("ui.profile.sbc_bitrate", 273, "48")));
    fireEvent.click(within(dialog).getByRole("button", { name: t("modal.ok") }));
    await waitFor(() => expect(saves(calls)).toHaveLength(1));
    expect(saves(calls)[0].profile).toMatchObject({ codec: "sbc", sbc_bitpool: 39, sbc_min_bitpool: 0 });
  });

  it("aptX Adaptive offers 96 kHz only to a device that lists it", async () => {
    start("#/codecs", "capped");
    const rates = async (device: string) => {
      const dialog = await codecSettings(await section(device), "codec.aptx_adaptive");
      const list = within(within(dialog).getByLabelText(t("connection.sample_rate")))
        .getAllByRole("option")
        .map((o) => o.textContent);
      fireEvent.click(within(dialog).getByRole("button", { name: t("modal.cancel") }));
      await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
      return list;
    };
    // A Bose QC Ultra lists 44.1 / 48 kHz only (issue #86); an EarFun Air
    // Pro 4+ 96 kHz too; a device not connected yet is not offered 96 kHz
    expect(await rates("QC Ultra Earbuds")).toEqual([t("codec.auto"), "48000 Hz"]);
    expect(await rates("EarFun Air Pro 4+")).toEqual([t("codec.auto"), "48000 Hz", "96000 Hz"]);
    expect(await rates("SRS-XB13")).toEqual([t("codec.auto"), "48000 Hz"]);
  });
});

describe("SBC settings", () => {
  it("offer what the device listed and show the bit rate as it changes", async () => {
    const { calls } = start("#/codecs");
    const dialog = await codecSettings(await section("WH-1000XM5"), "codec.sbc");
    // Auto: 16 blocks, 8 subbands, joint stereo, bitpool 53, at 48 kHz
    // only; read-only where the quality list was (issue #86)
    const rate = within(dialog).getByRole("status");
    expect(rate).toBe(within(dialog).getByLabelText(t("connection.quality")));
    expect(rate).toHaveTextContent(new RegExp(`^${t("ui.profile.sbc_bitrate", 357, "48")}$`));

    // The WH-1000XM5 lists 8 subbands and loudness only (the mock's XM5_SBC)
    const subbands = within(dialog).getByLabelText(t("ui.diag.cd_subbands"));
    expect(within(subbands).getAllByRole("option").map((o) => o.textContent)).toEqual([t("codec.auto"), "8"]);
    const alloc = within(dialog).getByLabelText(t("ui.diag.cd_allocation"));
    expect(within(alloc).queryByRole("option", { name: t("ui.diag.cd_alloc.snr") })).toBeNull();
    const blocks = within(dialog).getByLabelText(t("ui.diag.cd_blocks"));
    expect(within(blocks).getAllByRole("option")).toHaveLength(5);

    // 8 blocks: shorter frames, a higher rate
    fireEvent.change(blocks, { target: { value: "8" } });
    await waitFor(() => expect(rate).toHaveTextContent(t("ui.profile.sbc_bitrate", 396, "48")));
    // Stereo at 44.1 kHz with bitpool 40
    fireEvent.change(within(dialog).getByLabelText(t("ui.diag.cd_channel_mode")), { target: { value: "stereo" } });
    fireEvent.change(within(dialog).getByLabelText(t("connection.sample_rate")), { target: { value: "44100" } });
    fireEvent.input(within(dialog).getByRole("slider", { name: t("ui.profile.sbc_bitpool_max") }), { target: { value: "40" } });
    // 4 + 8 + 8 * 40 / 8 = 52 bytes per 64 samples
    await waitFor(() => expect(rate).toHaveTextContent(new RegExp(`^${t("ui.profile.sbc_bitrate", 286, "44.1")}$`)));

    fireEvent.click(within(dialog).getByRole("button", { name: t("modal.ok") }));
    await waitFor(() => expect(saves(calls)).toHaveLength(1));
    expect(saves(calls)[0].profile).toMatchObject({
      codec: "sbc",
      sbc_blocks: 8,
      sbc_subbands: 0,
      sbc_allocation: "",
      sbc_channel_mode: "stereo",
      sbc_bitpool: 40,
      sample_rate: 44100,
    });
  });

  it("one slider with two handles sets the lowest and the highest bitpool (issue #86)", async () => {
    const { calls } = start("#/codecs");
    const dialog = await codecSettings(await section("WH-1000XM5"), "codec.sbc");
    const group = within(dialog).getByRole("group", { name: t("connection.sbc_bitpool") });
    const low = within(group).getByRole("slider", { name: t("ui.profile.sbc_bitpool_min") });
    const high = within(group).getByRole("slider", { name: t("ui.profile.sbc_bitpool_max") });
    const auto = within(dialog).getByRole("button", { name: t("codec.auto") });
    // Auto: the lowest at the start, the highest where High puts it (53)
    expect([low, high].map((s) => [s.getAttribute("min"), s.getAttribute("max")])).toEqual([
      ["2", "53"],
      ["2", "53"],
    ]);
    expect(low).toHaveValue("2");
    expect(low).toHaveAttribute("aria-valuetext", t("codec.auto"));
    expect(high).toHaveValue("53");
    expect(high).toHaveAttribute("aria-valuetext", t("ui.profile.sbc_bitpool_auto", 53));
    expect(auto).toBeDisabled();

    // The highest down to 30: the bit rate follows it (73-byte frames at 48 kHz)
    fireEvent.input(high, { target: { value: "30" } });
    expect(high).toHaveAttribute("aria-valuetext", "30");
    const rate = within(dialog).getByRole("status");
    await waitFor(() => expect(rate).toHaveTextContent(t("ui.profile.sbc_bitrate", 219, "48")));
    expect(auto).toBeEnabled();
    // The lowest never passes the highest, nor the highest the lowest
    fireEvent.input(low, { target: { value: "40" } });
    expect(low).toHaveValue("30");
    fireEvent.input(low, { target: { value: "20" } });
    expect(low).toHaveValue("20");
    expect(low).toHaveAttribute("aria-valuetext", "20");
    fireEvent.input(high, { target: { value: "10" } });
    expect(high).toHaveValue("20");
    fireEvent.input(high, { target: { value: "30" } });

    fireEvent.click(within(dialog).getByRole("button", { name: t("modal.ok") }));
    await waitFor(() => expect(saves(calls)).toHaveLength(1));
    expect(saves(calls)[0].profile).toMatchObject({ codec: "sbc", sbc_min_bitpool: 20, sbc_bitpool: 30, sbcxq_min_bitpool: 0 });

    // Back in (once saved): the handles where they were; Auto puts both back
    await settled(await section("WH-1000XM5"));
    const again = await codecSettings(await section("WH-1000XM5"), "codec.sbc");
    const [low2, high2] = within(again).getAllByRole("slider");
    expect([low2, high2].map((s) => (s as HTMLInputElement).value)).toEqual(["20", "30"]);
    fireEvent.click(within(again).getByRole("button", { name: t("codec.auto") }));
    expect(low2).toHaveAttribute("aria-valuetext", t("codec.auto"));
    expect(high2).toHaveAttribute("aria-valuetext", t("ui.profile.sbc_bitpool_auto", 53));
    fireEvent.click(within(again).getByRole("button", { name: t("modal.ok") }));
    await waitFor(() => expect(saves(calls)).toHaveLength(2));
    expect(saves(calls)[1].profile).toMatchObject({ sbc_min_bitpool: 0, sbc_bitpool: 0 });
  });

  it("SBC XQ keeps its own minimum, within its own range", async () => {
    const { calls } = start("#/codecs");
    fireEvent.click(await screen.findByRole("switch", { name: (n) => n.startsWith(t("ui.profile.codecs_hide")) }));
    const dialog = await codecSettings(await section("WH-1000XM5"), "codec.sbc_xq");
    const low = within(dialog).getByRole("slider", { name: t("ui.profile.sbc_bitpool_min") });
    const high = within(dialog).getByRole("slider", { name: t("ui.profile.sbc_bitpool_max") });
    // Auto at 48 kHz: 43 per channel
    expect(high).toHaveAttribute("aria-valuetext", t("ui.profile.sbc_bitpool_auto", 43));
    // With the highest on Auto, the lowest stops where Auto's highest is
    fireEvent.input(low, { target: { value: "47" } });
    expect(low).toHaveValue("43");
    fireEvent.input(low, { target: { value: "35" } });
    fireEvent.click(within(dialog).getByRole("button", { name: t("modal.ok") }));
    await waitFor(() => expect(saves(calls)).toHaveLength(1));
    expect(saves(calls)[0].profile).toMatchObject({ codec: "sbcxq", sbcxq_min_bitpool: 35, sbcxq_bitpool: 0, sbc_min_bitpool: 0 });
  });

  it("SBC XQ has no channel mode of its own", async () => {
    start("#/codecs");
    // Not among what the mock's WH-1000XM5 supported: shown when not hidden
    fireEvent.click(await screen.findByRole("switch", { name: (n) => n.startsWith(t("ui.profile.codecs_hide")) }));
    const dialog = await codecSettings(await section("WH-1000XM5"), "codec.sbc_xq");
    expect(within(dialog).queryByLabelText(t("ui.diag.cd_channel_mode"))).toBeNull();
    // Bitpool 43 per channel at 48 kHz
    expect(within(dialog).getByRole("status")).toHaveTextContent(t("ui.profile.sbc_bitrate", 552, "48"));
  });

  it("plain SBC offers no dual channel: that is SBC XQ (issue #86)", async () => {
    start("#/codecs");
    const dialog = await codecSettings(await section("WH-1000XM5"), "codec.sbc");
    const mode = within(dialog).getByLabelText(t("ui.diag.cd_channel_mode"));
    expect(within(mode).getAllByRole("option").map((o) => o.textContent)).toEqual(
      [t("codec.auto"), ...["joint_stereo", "stereo", "mono"].map((m) => t(`ui.diag.cd_mode.${m}`))],
    );
  });

  it("SBC's ABR and frame setup do not carry to SBC XQ, nor back (issue #86)", async () => {
    const { calls } = start("#/codecs");
    fireEvent.click(await screen.findByRole("switch", { name: (n) => n.startsWith(t("ui.profile.codecs_hide")) }));
    const wh = await section("WH-1000XM5");
    let dialog = await codecSettings(wh, "codec.sbc");
    fireEvent.click(within(dialog).getByRole("switch", { name: t("connection.abr") }));
    fireEvent.change(within(dialog).getByLabelText(t("ui.diag.cd_blocks")), { target: { value: "8" } });
    fireEvent.click(within(dialog).getByRole("button", { name: t("modal.ok") }));
    await waitFor(() => expect(saves(calls)).toHaveLength(1));
    expect(saves(calls)[0].profile).toMatchObject({ codec: "sbc", abr: true, sbc_blocks: 8 });
    await settled(wh);

    dialog = await codecSettings(wh, "codec.sbc_xq");
    expect(within(dialog).getByRole("switch", { name: t("connection.abr") })).not.toBeChecked();
    expect(within(dialog).getByLabelText(t("ui.diag.cd_blocks"))).toHaveValue("0");
    fireEvent.click(within(dialog).getByRole("button", { name: t("modal.ok") }));
    await waitFor(() => expect(saves(calls)).toHaveLength(2));
    expect(saves(calls)[1].profile).toMatchObject({ codec: "sbcxq", abr: false, sbc_blocks: 0 });
    expect(saves(calls)[1].profile.codec_settings).toMatchObject({ sbc: { abr: true, sbc_blocks: 8 } });
    await settled(wh);

    // SBC's own come back as they were
    dialog = await codecSettings(wh, "codec.sbc");
    expect(within(dialog).getByRole("switch", { name: t("connection.abr") })).toBeChecked();
    expect(within(dialog).getByLabelText(t("ui.diag.cd_blocks"))).toHaveValue("8");
  });
});

/** A saved device's entry on the Audio Devices screen (its row and settings) */
const deviceEntry = async (name: RegExp) =>
  (await within(await screen.findByRole("main")).findAllByText(name))
    .map((e) => e.closest(".device-entry"))
    .find((e) => e) as HTMLElement;

describe("a device's capture source (Audio Devices, issue #86)", () => {
  it("shows the capture device's format and warns when a profile differs", async () => {
    start("#/devices");
    const entry = await deviceEntry(/WH-1000XM5/);
    // Loopback: the default device (the mock's speakers mix 48 kHz, 32-bit),
    // in a box beside the capture source (issue #86)
    const box = await within(entry).findByRole("note", { name: t("capture.device_format", 48000, 32) });
    expect(box.closest(".capture-source")).not.toBeNull();
    expect([...box.querySelectorAll(".num")].map((e) => e.textContent)).toEqual(["48000 Hz", "32-bit"]);
    expect(box).not.toHaveClass("st-warn");

    const snap = (await mock!.call("state.snapshot")) as { profiles: { name: string }[] };
    const p = snap.profiles.find((x) => x.name === "WH-1000XM5 (LDAC)")!;
    await mock!.call("profile.save", { profile: { ...p, sample_rate: 96000 }, original_name: p.name });
    // A warning in amber, not an error (issue #86)
    await waitFor(() => expect(entry.querySelector(".field-warn")).not.toBeNull());
    expect(entry.querySelector(".field-error")).toBeNull();
    const warning = entry.querySelector(".field-warn")!;
    expect(box).toHaveClass("st-warn");
    expect(warning).toHaveTextContent(t("ui.profile.format_differs", t("connection.sample_rate")));
    // The device, not its profile by name (issue #86)
    expect(warning).not.toHaveTextContent("WH-1000XM5 (LDAC)");
  });

  it("lists System Loopback, the virtual devices and the playing apps in one list", async () => {
    start("#/devices");
    const entry = await deviceEntry(/WH-1000XM5/);
    const select = within(entry).getByLabelText(t("capture.mode"));
    const virtual = await within(select).findByRole("group", { name: t("capture.virtual") });
    expect(within(virtual).getByRole("option", { name: "CABLE Input (VB-Audio Virtual Cable)" })).toBeInTheDocument();
    const apps = await within(select).findByRole("group", { name: t("capture.app") });
    expect(within(apps).getByRole("option", { name: "Spotify" })).toBeInTheDocument();
    expect(within(select).getByRole("option", { name: t("capture.loopback") })).toBeInTheDocument();
    // What the source does, below it, with no info button (issue #86)
    expect(within(entry).queryByRole("button", { name: t("ui.more_info", t("capture.mode")) })).toBeNull();
    expect(within(entry).getByText(t("capture.hint_loopback"))).toBeInTheDocument();
  });

  it("is saved into all the device's profiles when chosen", async () => {
    const { calls } = start("#/devices");
    const snap = (await mock!.call("state.snapshot")) as { profiles: { name: string }[] };
    const p = snap.profiles.find((x) => x.name === "WH-1000XM5 (LDAC)")!;
    await mock!.call("profile.save", { profile: { ...p, name: "XM5 AAC", codec: "aac" }, original_name: null });
    const entry = await deviceEntry(/WH-1000XM5/);
    calls.length = 0;
    expect(within(entry).queryByRole("switch", { name: (n) => n.startsWith(t("capture.auto_switch")) })).toBeNull();
    const select = within(entry).getByLabelText(t("capture.mode"));
    await within(select).findByRole("option", { name: "CABLE Input (VB-Audio Virtual Cable)" });
    fireEvent.change(select, { target: { value: "virtual:{0.0.0.00000000}.{cable}" } });
    await waitFor(() => expect(calls.filter((c) => c.method === "profile.save")).toHaveLength(2));
    // A virtual device can become the Windows default output
    expect(await within(entry).findByRole("switch", { name: (n) => n.startsWith(t("capture.auto_switch")) })).toBeInTheDocument();
    for (const c of calls.filter((x) => x.method === "profile.save")) {
      expect((c.params as { profile: Record<string, unknown> }).profile).toMatchObject({
        capture_mode: "virtual",
        audio_device_id: "{0.0.0.00000000}.{cable}",
      });
    }
  });

  it("gives a device without a profile one with its first setting", async () => {
    const { calls } = start("#/devices");
    await mock!.call("device.rename", { address: "54:2F:04:4B:29:DC", name: "moto buds+" });
    const entry = await deviceEntry(/moto buds\+/);
    const select = within(entry).getByLabelText(t("capture.mode"));
    await within(select).findByRole("option", { name: "Spotify" });
    // None chosen yet: System Loopback, though a VB-Audio cable is
    // installed (issue #86); the source it has already: nothing to save
    await waitFor(() => expect(select).toHaveValue("loopback"));
    fireEvent.change(select, { target: { value: "loopback" } });
    expect(calls.some((c) => c.method === "profile.save")).toBe(false);
    fireEvent.change(select, { target: { value: "app:spotify.exe" } });
    await waitFor(() => expect(saves(calls)).toHaveLength(1));
    expect(saves(calls)[0]).toMatchObject({
      profile: {
        name: "moto buds+ | Auto",
        codec: "auto",
        capture_mode: "app",
        app_exe: "Spotify.exe",
        device_address: "54:2F:04:4B:29:DC",
      },
      original_name: null,
    });
  });

  it("has one Volume Sync, buttons and remembered volume for every device, above the list", async () => {
    const { calls } = start("#/devices");
    const entry = await deviceEntry(/WH-1000XM5/);
    const all = await screen.findByRole("region", { name: t("ui.devices.all_title") });
    const named = (key: string) => ({ name: (n: string) => n.startsWith(t(key)) });
    // Not in a device's own settings
    for (const key of ["capture.volume_sync", "settings.media_keys", "settings.remember_volume"]) {
      expect(within(entry).queryByRole("switch", named(key))).toBeNull();
    }
    expect(within(entry).queryByLabelText(t("capture.volume_sync"))).toBeNull();
    // Switches, on by default
    const sync = within(all).getByRole("switch", named("capture.volume_sync"));
    const keys = within(all).getByRole("switch", named("settings.media_keys"));
    expect(sync).toBeChecked();
    expect(keys).toBeChecked();
    expect(within(all).getByRole("switch", named("settings.remember_volume"))).toBeChecked();
    expect(within(all).getByRole("switch", named("settings.auto_reconnect"))).toBeChecked();
    fireEvent.click(sync);
    await waitFor(() => expect(calls.some((c) => c.method === "settings.set")).toBe(true));
    const set = calls.find((c) => c.method === "settings.set")!.params as { settings: Record<string, unknown> };
    expect(set.settings.volume_sync).toBe(false);
    expect(set.settings).not.toHaveProperty("device_options");
    await waitFor(() => expect(sync).not.toBeChecked());
    // Settings keeps only the general ones
    location.hash = "#/settings";
    await screen.findByRole("heading", { name: t("settings.header") });
    expect(screen.queryByRole("switch", named("settings.media_keys"))).toBeNull();
  });

  it("has no Volume Sync or packet size of its own, developer mode or not (issue #86)", async () => {
    const { core } = start("#/devices");
    const entry = await deviceEntry(/WH-1000XM5/);
    await core.request("settings.set", { settings: { ...core.state.snap!.settings, developer_mode: true } });
    await within(entry).findByLabelText(t("capture.mode"));
    expect(within(entry).queryByLabelText(t("capture.volume_sync"))).toBeNull();
    expect(within(entry).queryByLabelText(t("advanced.max_packet"))).toBeNull();
  });

  it("says what application capture needs when the Windows build is not known", async () => {
    start("#/devices");
    const entry = await deviceEntry(/SRS-XB13/);
    expect(await within(entry).findByText(t("ui.profile.app_capture_requirement"))).toBeInTheDocument();
    expect(within(entry).queryByText(t("ui.profile.app_capture_unsupported"))).toBeNull();
  });

  it("says plainly when this Windows cannot capture one application", async () => {
    // WebView2's client hints of Windows 10 1909 (build 18363)
    Object.defineProperty(navigator, "userAgentData", {
      configurable: true,
      value: { getHighEntropyValues: () => Promise.resolve({ platform: "Windows", platformVersion: "8.0.0" }) },
    });
    try {
      start("#/devices");
      const entry = await deviceEntry(/SRS-XB13/);
      expect(await within(entry).findByText(t("ui.profile.app_capture_unsupported"))).toBeInTheDocument();
      // Only the app it has: no playing app to choose
      const apps = within(within(entry).getByLabelText(t("capture.mode"))).getByRole("group", { name: t("capture.app") });
      expect(within(apps).getAllByRole("option")).toHaveLength(1);
    } finally {
      delete (navigator as { userAgentData?: unknown }).userAgentData;
    }
  });
});

describe("the max media packet size (each device's, on the Developer page since issue #86)", () => {
  it("is listed per saved device, checked, then saved into the device's profile", async () => {
    const { calls, core } = start("#/devices");
    await deviceEntry(/WH-1000XM5/);
    await core.request("settings.set", { settings: { ...core.state.snap!.settings, developer_mode: true } });
    location.hash = "#/developer";
    const section = await screen.findByRole("region", { name: t("advanced.max_packet") });
    // By the device's name, not its profile's
    const packet = within(section).getByLabelText("WH-1000XM5");
    expect(within(section).queryByLabelText("WH-1000XM5 (LDAC)")).toBeNull();
    expect(packet).toHaveValue(1023);
    fireEvent.input(packet, { target: { value: "2000" } });
    fireEvent.change(packet, { target: { value: "2000" } });
    expect(await within(section).findByText(t("ui.profile.err_packet", 679, 1679))).toBeInTheDocument();
    expect(calls.some((c) => c.method === "profile.save")).toBe(false);
    fireEvent.input(packet, { target: { value: "800" } });
    fireEvent.change(packet, { target: { value: "800" } });
    await waitFor(() => expect(saves(calls)).toHaveLength(1));
    expect(saves(calls)[0].profile).toMatchObject({ name: "WH-1000XM5 (LDAC)", max_media_payload: 800 });
    await waitFor(() => expect(within(section).getByLabelText("WH-1000XM5")).toHaveValue(800));
    // Not on the Codecs screen
    location.hash = "#/codecs";
    await screen.findByRole("heading", { level: 1, name: t("ui.nav.codecs") });
    expect(screen.queryByLabelText(t("advanced.max_packet"))).toBeNull();
  });
});

describe("SBC ABR", () => {
  it("is in the SBC dialog, and LDAC's ABR does not carry over to it", async () => {
    const { calls } = start("#/codecs");
    const wh = await section("WH-1000XM5");
    // LDAC with ABR on
    let dialog = await codecSettings(wh, "codec.ldac");
    fireEvent.click(within(dialog).getByRole("switch", { name: t("connection.abr") }));
    fireEvent.click(within(dialog).getByRole("button", { name: t("modal.ok") }));
    await waitFor(() => expect(saves(calls)).toHaveLength(1));
    // SBC's dialog starts with its adaptive bitpool off
    dialog = await codecSettings(wh, "codec.sbc");
    const abr = within(dialog).getByRole("switch", { name: t("connection.abr") });
    expect(abr).not.toBeChecked();
    fireEvent.click(abr);
    fireEvent.click(within(dialog).getByRole("button", { name: t("modal.ok") }));
    await waitFor(() => expect(saves(calls)).toHaveLength(2));
    expect(saves(calls)[1].profile).toMatchObject({ codec: "sbc", abr: true });
  });

  it("SBC XQ offers it too", async () => {
    start("#/codecs");
    fireEvent.click(await screen.findByRole("switch", { name: (n) => n.startsWith(t("ui.profile.codecs_hide")) }));
    const dialog = await codecSettings(await section("WH-1000XM5"), "codec.sbc_xq");
    expect(within(dialog).getByRole("switch", { name: t("connection.abr") })).not.toBeChecked();
  });
});
