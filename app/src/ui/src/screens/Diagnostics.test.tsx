import { cleanup, fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it, vi } from "vitest";
import { App } from "../app/App";
import { MockCore } from "../ipc/mock";
import { createCore } from "../ipc/store";

let mock: MockCore | null = null;

function start() {
  location.hash = "#/diagnostics";
  const calls: { method: string; params: unknown }[] = [];
  mock = new MockCore({ speed: 0, onCall: (method, params) => calls.push({ method, params }) });
  const core = createCore(mock);
  const view = render(() => <App core={core} />);
  return { core, calls, view, mock };
}

afterEach(() => {
  cleanup();
  mock?.dispose();
  mock = null;
});

describe("Diagnostics", () => {
  it("shows a capture started elsewhere, its size, and the saved file", async () => {
    const { core, mock, view } = start();
    await screen.findByRole("heading", { level: 1, name: "Diagnostics" });
    const reveal = vi.spyOn(mock, "revealPath");
    // Started from another window: the snapshot event shows it here
    const path = await core.request("diag.capture_start");
    const line = await waitFor(() => {
      const el = view.container.querySelector(".capture-line");
      expect(el).not.toBeNull();
      return el!;
    });
    expect(line).toHaveTextContent(path);
    expect(line.querySelector(".capture-size")!.textContent).toMatch(/\d+ (B|KB)/);

    fireEvent.click(screen.getByRole("button", { name: "Stop HCI Capture" }));
    await waitFor(() => expect(view.container.querySelector(".capture-saved")).not.toBeNull());
    expect(view.container.querySelector(".capture-saved")).toHaveTextContent(path);
    fireEvent.click(view.container.querySelector(".capture-saved .btn-subtle")!);
    expect(reveal).toHaveBeenCalledWith(path);
  });

  it("turns verbose logging on and off and saves a diagnostic bundle", async () => {
    const { calls, core, mock, view } = start();
    await screen.findByRole("heading", { level: 1, name: "Diagnostics" });
    const select = screen.getByRole("combobox", { name: "Verbose logging" }) as HTMLSelectElement;
    expect(select.value).toBe("off");
    select.selectedIndex = 1;
    fireEvent.change(select);
    await waitFor(() => expect(core.state.snap!.verbose?.level).toBe("debug"));
    expect(calls.find((c) => c.method === "diag.set_verbose")!.params).toEqual({ level: "debug" });
    expect(view.container.querySelector("#verbose-help")).toHaveTextContent(/^On until /);
    select.selectedIndex = 0;
    fireEvent.change(select);
    await waitFor(() => expect(core.state.snap!.verbose).toBeNull());

    const reveal = vi.spyOn(mock, "revealPath");
    const open = vi.spyOn(mock, "openUrl").mockResolvedValue();
    // The contents are chosen first: no bundle until Save in the dialog
    fireEvent.click(screen.getByRole("button", { name: "Save…" }));
    const dialog = await screen.findByRole("dialog", { name: "Save Diagnostic Bundle" });
    expect(calls.some((c) => c.method === "diag.bundle")).toBe(false);
    // HCI captures are off unless asked for: ticking the box opts in
    const hci = screen.getByRole("switch", { name: "Include HCI captures" });
    expect(hci).not.toBeChecked();
    fireEvent.click(hci);
    fireEvent.click(dialog.querySelector(".dialog-actions .btn-strong")!);
    const saved = await waitFor(() => {
      const el = view.container.querySelector(".bundle-saved");
      expect(el).not.toBeNull();
      return el!;
    });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(calls.find((c) => c.method === "diag.bundle")!.params).toEqual({ mask: true, hci: true });
    expect(saved).toHaveTextContent(/a2dpwb-diag-\d{8}_\d{6}\.zip · \d+ KB/);
    expect(saved).toHaveTextContent("Attach this file to your GitHub issue");

    // A prefilled issue, with the bundle shown in Explorer to drag in
    fireEvent.click(screen.getByRole("button", { name: "Report on GitHub" }));
    await waitFor(() => expect(open).toHaveBeenCalled());
    const url = new URL(open.mock.calls[0][0]);
    expect(url.pathname).toBe("/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/new");
    expect(url.searchParams.get("template")).toBe("bug_report.yml");
    expect(url.searchParams.get("version")).toBe(core.state.snap!.update.current_version);
    expect(url.searchParams.get("bundle")).toMatch(/^a2dpwb-diag-/);
    expect(calls.filter((c) => c.method === "diag.info").at(-1)!.params).toEqual({ mask: true });
    expect(reveal).toHaveBeenCalledWith(expect.stringMatching(/\\bundles\\a2dpwb-diag-/));
    fireEvent.click([...saved.querySelectorAll("button")].find((b) => b.textContent === "Close")!);
    await waitFor(() => expect(view.container.querySelector(".bundle-saved")).toBeNull());
    // Back to the default for the other tests (the choice lives with the window)
    fireEvent.click(screen.getByRole("button", { name: "Save…" }));
    fireEvent.click(await screen.findByRole("switch", { name: "Include HCI captures" }));
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  });

  it("masks addresses in the copied diagnostic info only while asked to", async () => {
    const { calls } = start();
    const copy = await screen.findByRole("button", { name: "Copy Diagnostics" });
    fireEvent.click(copy);
    await waitFor(() => expect(calls.some((c) => c.method === "diag.info")).toBe(true));
    expect(calls.find((c) => c.method === "diag.info")!.params).toEqual({ mask: true });
    fireEvent.click(screen.getByRole("switch", { name: "Mask Bluetooth addresses" }));
    fireEvent.click(copy);
    await waitFor(() => expect(calls.filter((c) => c.method === "diag.info").length).toBe(2));
    expect(calls.filter((c) => c.method === "diag.info")[1].params).toEqual({ mask: false });
    // Back to the default for the other tests (the setting lives with the window)
    fireEvent.click(screen.getByRole("switch", { name: "Mask Bluetooth addresses" }));
  });

  it("jumps from a flow step to its log line", async () => {
    const { core, view } = start();
    await screen.findByRole("heading", { level: 1, name: "Diagnostics" });
    await waitFor(() => expect(core.state.snap).not.toBeNull());
    await core.request("stream.start", { profile: core.state.snap!.profiles[0].name });
    await waitFor(() => expect(core.state.snap!.stream.state).toBe("streaming"));

    fireEvent.click(screen.getByRole("tab", { name: "Connection Flow" }));
    const jump = await waitFor(() => {
      const b = view.container.querySelector<HTMLButtonElement>(".flow-step .flow-jump button");
      expect(b).not.toBeNull();
      return b!;
    });
    // The step's time of day, from the core's clock (after its column name
    // for screen readers)
    expect(view.container.querySelector(".flow-step .flow-clock")!.textContent).toMatch(/^Time: \d\d:\d\d:\d\d\.\d{3}$/);
    expect(view.container.querySelector(".flow-head")).toHaveTextContent(/Step\s*Status\s*Time\s*Elapsed\s*Detail/);
    // The details the core computes: the audio rate
    await waitFor(() => expect(view.container.querySelector(".flow-step:nth-child(10) .flow-detail")).toHaveTextContent(/packets\/s/));
    fireEvent.click(jump);

    await waitFor(() => expect(screen.getByRole("tab", { name: "Log" })).toHaveAttribute("aria-selected", "true"));
    const line = await waitFor(() => {
      const el = view.container.querySelector(".log-line.jumped");
      expect(el).not.toBeNull();
      return el!;
    });
    expect(line).toHaveClass("selected");
    expect(line).toHaveTextContent("stream: device");
    expect(screen.getByRole("button", { name: "Copy Selected Lines" })).not.toBeDisabled();

    // This attempt only keeps the attempt's lines
    fireEvent.click(screen.getByRole("switch", { name: "Current attempt only" }));
    await waitFor(() => expect(view.container.querySelector(".log-line")).toHaveTextContent("stream: device"));
  });

  it("moves between the tabs with the arrow keys", async () => {
    start();
    const first = await screen.findByRole("tab", { name: "Stream" });
    expect(first).toHaveAttribute("aria-selected", "true");
    const names = ["Stream", "Radio", "Audio capture", "Connection Flow", "Log"];
    for (const name of [...names.slice(1), names[0]]) {
      fireEvent.keyDown(document.activeElement === document.body ? first : document.activeElement!, { key: "ArrowRight" });
      const tab = screen.getByRole("tab", { name });
      expect(tab).toHaveAttribute("aria-selected", "true");
      expect(tab).toHaveAttribute("tabindex", "0");
      expect(screen.getByRole("tabpanel")).toHaveAttribute("aria-labelledby", tab.id);
    }
    fireEvent.keyDown(first, { key: "End" });
    expect(screen.getByRole("tab", { name: "Log" })).toHaveAttribute("aria-selected", "true");
    fireEvent.keyDown(first, { key: "Home" });
    expect(first).toHaveAttribute("aria-selected", "true");
  });

  it("graphs the stream, the radio and the capture on one time axis with a shared cursor", async () => {
    const { core, view } = start();
    await waitFor(() => expect(core.state.snap).not.toBeNull());
    await core.request("stream.start", { profile: core.state.snap!.profiles[0].name });
    await waitFor(() => expect(core.state.snap!.stream.state).toBe("streaming"));
    await waitFor(() => expect(core.link().length).toBeGreaterThan(8));

    // Stream: bitrate, packets, queue depth and wait, drops
    const charts = () => [...view.container.querySelectorAll<HTMLElement>(".chart")];
    await waitFor(() => expect(charts().map((c) => c.querySelector(".chart-title")!.textContent)).toEqual([
      "Actual bitrate",
      "Packets",
      "Send queue",
      "Send queue wait",
      "Packets dropped before sending",
    ]));
    await waitFor(() => expect(charts()[0].querySelector(".chart-value")).toHaveTextContent(/^\d+ kbps$/));
    expect(charts()[3].querySelector(".chart-value")).toHaveTextContent(/^\d+ ms$/);
    // The encoder's bitrate as the target, the queue's limit
    expect(charts()[0].querySelector(".ref-target")).not.toBeNull();
    expect(charts()[3].querySelector(".ref-limit")).not.toBeNull();
    expect(charts()[0].querySelector(".chart-axis")).toHaveTextContent("−60 s0 s");

    // Hovering one chart puts the cursor on every chart of the tab
    const plot = charts()[1].querySelector("svg")!;
    plot.getBoundingClientRect = () => ({ left: 0, top: 0, width: 300, height: 100, right: 300, bottom: 100, x: 0, y: 0, toJSON: () => ({}) });
    fireEvent.pointerMove(plot, { clientX: 299, clientY: 50 });
    await waitFor(() => expect(view.container.querySelectorAll(".chart .cursor").length).toBe(5));
    for (const c of charts()) expect(c.querySelector(".chart-ago")).toHaveTextContent(/^−?\d+\.\d s$/);
    fireEvent.pointerLeave(plot);
    await waitFor(() => expect(view.container.querySelectorAll(".chart .cursor").length).toBe(0));

    // Radio: the quality and what lowers it, RSSI, AFH, radio time
    fireEvent.click(screen.getByRole("tab", { name: "Radio" }));
    await waitFor(() => expect(charts().map((c) => c.querySelector(".chart-title")!.textContent)).toEqual([
      "Signal vs. ideal",
      "Channel hopping (AFH)",
      "Bluetooth bandwidth (estimate)",
    ]));
    expect(view.container.querySelector(".lq-verdict-label")!.textContent).toMatch(/^(Good|Fair|Poor|PC busy)$/);
    const afh = charts()[1];
    expect(afh.querySelectorAll(".chart-legend li")).toHaveLength(2);
    await waitFor(() => expect(afh.querySelector(".chart-legend")).toHaveTextContent(/In use\s*58\s*Usable\s*79/));
    expect(screen.getByRole("heading", { name: "Bluetooth bandwidth (estimate)" })).toBeInTheDocument();
    expect(charts()[2].querySelector(".chart-legend")).toHaveTextContent(/Radio time used.*Share of theoretical max/);

    // Audio capture: the rate against the source's, drops as audio time
    fireEvent.click(screen.getByRole("tab", { name: "Audio capture" }));
    await waitFor(() => expect(charts().map((c) => c.querySelector(".chart-title")!.textContent)).toEqual([
      "Capture rate",
      "Audio dropped at capture",
    ]));
    expect(charts()[1].querySelector(".chart-value")).toHaveTextContent(/^0 ms$/);
    expect(screen.getByText("Output Source").closest(".lq-row")).toHaveTextContent("System Loopback");
    expect(view.container.querySelector(".lq-group .hint")).toHaveTextContent(/^Output source format: \d+ Hz/);
  });
});
