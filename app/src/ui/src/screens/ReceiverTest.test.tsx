import { cleanup, fireEvent, render, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "../app/App";
import { t } from "../i18n";
import { MockCore, type MockOptions } from "../ipc/mock";
import { createCore } from "../ipc/store";

let mock: MockCore | null = null;

async function open(opts: MockOptions = {}) {
  location.hash = "#/developer";
  const calls: { method: string; params: unknown }[] = [];
  mock = new MockCore({ speed: 0, ...opts, onCall: (method, params) => calls.push({ method, params }) });
  const core = createCore(mock);
  render(() => <App core={core} />);
  await screen.findByRole("navigation");
  await core.request("settings.set", { settings: { ...core.state.snap!.settings, developer_mode: true } });
  await screen.findByRole("heading", { level: 1, name: "Developer" });
  fireEvent.click(await screen.findByRole("tab", { name: "Peer Receiver Test" }));
  return { core, calls };
}

const steps = () => within(screen.getByRole("list", { name: t("ui.receiver.checks") })).getAllByRole("listitem");
const row = (name: string) => screen.getByRole("rowheader", { name }).closest("tr")!;

afterEach(() => {
  cleanup();
  mock?.dispose();
  mock = null;
});

describe("receiver test", () => {
  it("finds the receiver, measures and compares", async () => {
    const { calls } = await open();
    // Looks for the receiver at once (the saved address is empty: the local network)
    await screen.findByText("Ready. Start sends to the receiver's Bluetooth adapter.");
    expect(calls.find((c) => c.method === "receiver.connect")?.params).toEqual({ target: "auto" });
    expect(screen.getByText("sink-pi (192.168.1.42), Bluetooth DC:A6:32:01:02:03, L2CAP MTU 895")).toBeInTheDocument();
    await waitFor(() => expect(steps()[1]).toHaveTextContent("OK"));
    expect(steps()[0]).toHaveTextContent("Find receiver");

    // Choices: SBC, MTU 679, buffer 300 ms, system audio
    fireEvent.change(screen.getByLabelText("Codec"), { target: { selectedIndex: 10 } });
    fireEvent.change(screen.getByLabelText("L2CAP MTU"), { target: { selectedIndex: 2 } });
    fireEvent.change(screen.getByLabelText("Playout buffer"), { target: { selectedIndex: 3 } });
    fireEvent.click(screen.getByRole("radio", { name: "System audio" }));

    fireEvent.click(screen.getByRole("button", { name: "Start" }));
    await waitFor(() => expect(calls.some((c) => c.method === "stream.start_custom")).toBe(true));
    expect(calls.find((c) => c.method === "receiver.configure")?.params).toEqual({ mtu: 679, buffer_ms: 300 });
    expect(calls.find((c) => c.method === "stream.start_custom")?.params).toMatchObject({
      device: "DC:A6:32:01:02:03",
      codec: "sbc",
      quality: "hq",
      source: "loopback",
    });
    // The choices are locked while measuring
    expect(screen.getByLabelText("Codec")).toBeDisabled();

    // All six steps OK
    await waitFor(() => expect(steps().map((s) => s.querySelector(".flow-status")!.textContent)).toEqual(["OK", "OK", "OK", "OK", "OK", "OK"]), {
      timeout: 3000,
    });
    expect(steps()[2]).toHaveTextContent("MTU 679, 300 ms");
    await screen.findByText("Measuring.");

    // The table: sent beside received
    await waitFor(() => expect(Number(within(row("Packets")).getAllByRole("cell")[1].textContent)).toBeGreaterThan(0));
    const codec = within(row("Codec")).getAllByRole("cell");
    expect(codec[0]).toHaveTextContent(/^SBC, 48 kHz$/);
    expect(codec[1]).toHaveTextContent("SBC 48 kHz");
    expect(codec[2]).toHaveTextContent(t("ui.receiver.codec_same"));

    // The log
    const log = screen.getByRole("log", { name: "Log" });
    expect(log).toHaveTextContent("Start: send SBC, High, Auto, Auto, System audio to DC:A6:32:01:02:03; receiver MTU 679, buffer 300 ms");
    expect(log).toHaveTextContent("OK Audio arrives");

    fireEvent.click(screen.getByRole("button", { name: "Stop" }));
    await waitFor(() => expect(calls.some((c) => c.method === "stream.stop")).toBe(true));
    await screen.findByRole("button", { name: "Start" });
    expect(log).toHaveTextContent("Stopped");
    await waitFor(() => expect(within(row("Codec")).getAllByRole("cell")[0]).toHaveTextContent("SBC, 48 kHz (at stop)"));

    // Clear the log
    fireEvent.click(screen.getByRole("button", { name: "Clear" }));
    expect(screen.getByRole("log", { name: "Log" })).toHaveTextContent("No log lines yet");
  });

  it("disconnects, searches and connects to a receiver found", async () => {
    const { calls } = await open();
    await screen.findByText("Ready. Start sends to the receiver's Bluetooth adapter.");
    fireEvent.click(screen.getByRole("button", { name: "Disconnect" }));
    await screen.findByText("Not connected to a receiver.");
    expect(screen.getByRole("button", { name: "Start" })).toBeDisabled();
    expect(screen.getByRole("textbox", { name: "Receiver" })).not.toBeDisabled();

    fireEvent.click(screen.getByRole("button", { name: t("ui.receiver.search") }));
    const found = await screen.findByRole("button", { name: "sink-pi (192.168.1.42), Bluetooth DC:A6:32:01:02:03, L2CAP MTU 895" });
    fireEvent.click(found);
    await waitFor(() => expect(calls.filter((c) => c.method === "receiver.connect").pop()?.params).toEqual({ target: "192.168.1.42" }));
    await screen.findByText("Ready. Start sends to the receiver's Bluetooth adapter.");
    expect(screen.getByRole("textbox", { name: "Receiver" })).toHaveValue("192.168.1.42");
  });

  it("links to the AFH settings on the Adapter screen", async () => {
    await open();
    fireEvent.click(await screen.findByRole("button", { name: t("ui.receiver.afh_open") }));
    await screen.findByRole("heading", { level: 1, name: t("ui.nav.adapter") });
    expect(location.hash).toBe("#/adapter/afh");
    expect(document.getElementById("adapter-afh")).toHaveTextContent(t("receiver.afh_tooltip"));
  });

  it("shows why there is no receiver", async () => {
    await open({ scenario: "offline" });
    await screen.findByText("No receiver: no receiver found on the local network. Retrying.");
    await waitFor(() => expect(steps()[0]).toHaveTextContent("NG"));
    expect(screen.getByRole("button", { name: "Start" })).toBeDisabled();
  });
});
