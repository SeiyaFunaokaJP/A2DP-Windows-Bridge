import { cleanup, fireEvent, render, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, describe, expect, it, vi } from "vitest";
import { App } from "../app/App";
import { MockCore, type MockScenario, sampleSnapshot } from "../ipc/mock";
import { createCore } from "../ipc/store";
import { adapterExperimental, adapterIds, adapterIdsHint, adapterName, driverText } from "./Adapter";

let mock: MockCore | null = null;

function start(scenario: MockScenario = "normal") {
  location.hash = "#/adapter";
  const calls: { method: string; params: unknown }[] = [];
  mock = new MockCore({ speed: 0, scenario, onCall: (method, params) => calls.push({ method, params }) });
  const core = createCore(mock);
  render(() => <App core={core} />);
  return { calls, mock };
}

afterEach(() => {
  cleanup();
  mock?.dispose();
  mock = null;
  vi.restoreAllMocks();
});

describe("AFH", () => {
  it("has its mode and channels in the heading: Auto and Off read only, Manual typed (issue #86)", async () => {
    const { calls, mock } = start();
    const section = (await screen.findByRole("heading", { level: 2, name: "AFH" })).closest("section")!;
    const head = section.querySelector(".section-head") as HTMLElement;
    const box = within(head).getByRole("textbox", { name: "Wi-Fi ch" }) as HTMLInputElement;
    // One set of controls only
    expect(within(section).getAllByRole("radiogroup")).toHaveLength(1);
    expect(within(section).getAllByRole("textbox")).toHaveLength(1);
    // Auto: what it avoids now (the mock's stream avoids Wi-Fi channel 6)
    expect(box.readOnly).toBe(true);
    await mock.call("stream.start", { profile: "WH-1000XM5 (LDAC)" });
    await waitFor(() => expect(box.value).toBe("6"), { timeout: 3000 });
    // Off: nothing, read only
    fireEvent.click(within(head).getByRole("radio", { name: "Off" }));
    await waitFor(() => expect(box.value).toBe(""));
    expect(box.readOnly).toBe(true);
    // Manual: typed and saved
    fireEvent.click(within(head).getByRole("radio", { name: "Manual" }));
    await waitFor(() => expect(box.readOnly).toBe(false));
    fireEvent.input(box, { target: { value: "11, 6" } });
    fireEvent.blur(box);
    await waitFor(() =>
      expect(calls.filter((c) => c.method === "settings.set").at(-1)?.params).toMatchObject({ settings: { afh: "6,11" } }),
    );
  });
});

describe("Adapter firmware", () => {
  it("shows the chip's firmware and saves another chip", async () => {
    const { calls } = start();
    await screen.findByRole("heading", { level: 1, name: "Adapters" });
    await waitFor(() => expect(calls.some((c) => c.method === "adapter.firmware_files")).toBe(true));
    const chip = screen.getByLabelText("Bluetooth Chip") as HTMLSelectElement;
    expect(chip.options[chip.selectedIndex].text).toBe("Auto (RTL8761BU)");
    // In the firmware box of the adapter it applies to, not a section of its own
    expect(chip.closest(".adapter-card")).not.toBeNull();
    const box = screen.getByRole("group", { name: "Firmware: Firmware OK" });
    expect(box).toContainElement(chip);
    // The tag says the files are there; the box can fetch them again
    expect(screen.queryByText("rtl8761bu_config.bin")).toBeNull();
    expect(within(box).getByRole("button", { name: "Download Firmware" })).toBeInTheDocument();

    fireEvent.change(chip, { target: { selectedIndex: 1 } });
    await waitFor(() => expect(calls.some((c) => c.method === "settings.set")).toBe(true));
    const saved = calls.find((c) => c.method === "settings.set")!.params as {
      settings: { bt_chip_pid: number; bt_chip_fw_stem: string };
    };
    expect(saved.settings).toMatchObject({ bt_chip_pid: 0xfffe, bt_chip_fw_stem: "" });
    expect(await screen.findByText(/Realtek initialization is skipped/)).toBeInTheDocument();
    // The core checks the firmware for the new chip: none needed now
    expect(await screen.findByText("No Realtek files needed")).toBeInTheDocument();
    // The folder is scanned again after the change
    await waitFor(() => expect(calls.filter((c) => c.method === "adapter.firmware_files").length).toBeGreaterThan(1));
  });

  it("names the adapter as Device Manager does, with its driver and firmware side by side (issue #86)", async () => {
    // The first run's adapters (that scenario opens the setup instead)
    const { mock } = start();
    await screen.findByRole("heading", { level: 1, name: "Adapters" });
    const m = mock as any;
    const [mediatek, realtek] = sampleSnapshot("fresh").adapter.adapters;
    // An Intel adapter on WinUSB that reports no product name
    const intel = {
      ...realtek,
      instance_id: "USB\\VID_8087&PID_0033\\5&1",
      vid: 0x8087,
      pid: 0x0033,
      display_name: "Intel (0x8087:0x0033) [experimental]",
      vendor: "Intel",
      driver: "winusb",
      realtek_chip: null,
      firmware_present: null,
      vendor_note: { key: "firmware.vendor_intel", args: [] },
      windows_name: null,
      // Neither Windows' Bluetooth nor a stack of A2DPWB ran it yet
      address: null,
      bt_version: null,
    };
    m.snap.adapter = { ...m.snap.adapter, adapters: [mediatek, realtek, intel] };
    m.emit({ kind: "adapter", value: m.snap.adapter });
    const rtl = await screen.findByRole("region", { name: "TP-Link Bluetooth USB Adapter (RTL8761BU)" });
    expect(rtl).toHaveTextContent("00:1A:7D:DA:71:13 | 2357:0604 | v5.1");
    // The driver WinUSB replaces is called the original one (issue #86)
    const rtlDriver = within(rtl).getByRole("group", { name: "Driver: Original driver" });
    const assign = within(rtl).getByRole("button", { name: "Replace with WinUSB driver" });
    expect(rtlDriver).toContainElement(assign);
    expect(assign).toBeEnabled();
    expect(rtlDriver).toHaveTextContent("Replaces the original driver");
    // Windows' Bluetooth runs it: its name, address and version from there
    const mtk = screen.getByRole("region", { name: "MediaTek Bluetooth Adapter" });
    expect(mtk).toHaveTextContent("C0:E3:50:2E:00:09 | 0E8D:7925 | v5.4");
    expect(mtk).not.toHaveTextContent("[unsupported]");
    const fw = within(mtk).getByRole("group", { name: "Firmware: Unsupported" });
    expect(within(fw).getByText("Unsupported")).toHaveClass("st-bad");
    expect(within(fw).getByRole("button", { name: "Download not available" })).toBeDisabled();
    // A vendor's driver is the original one too, its service on hover; A2DPWB
    // cannot use the adapter, so it keeps that driver
    const mtkDriver = within(mtk).getByRole("group", { name: "Driver: Original driver" });
    expect(within(mtkDriver).getByText("Original driver")).toHaveAttribute("title", "mtkbtfilterx");
    expect(within(mtkDriver).getByRole("button", { name: "Replace with WinUSB driver" })).toBeDisabled();
    expect(mtkDriver).toHaveTextContent("A2DP Windows Bridge cannot use this adapter, so its driver stays as it is");
    // The chip choice goes with the Realtek adapter only
    expect(within(mtk).queryByLabelText("Bluetooth Chip")).toBeNull();
    expect(within(rtl).getByLabelText("Bluetooth Chip")).toBeInTheDocument();
    // "[experimental]" is a tag beside the name, not part of it
    const intelCard = screen.getByRole("region", { name: "Intel" });
    expect(intelCard).not.toHaveTextContent("[experimental]");
    expect(within(intelCard).getByText("Experimental")).toHaveClass("st-warn");
    // WinUSB in one name too (issue #86)
    expect(within(intelCard).getByRole("group", { name: "Driver: WinUSB driver" })).toBeInTheDocument();
    // What is known; why the rest is missing on hover
    expect(within(intelCard).getByText("8087:0033")).toHaveAttribute(
      "title",
      expect.stringContaining("once Windows' Bluetooth or A2DP Windows Bridge has used this adapter"),
    );
    expect(within(rtl).getByText("00:1A:7D:DA:71:13 | 2357:0604 | v5.1")).not.toHaveAttribute("title");
  });

  it("lists the adapters by itself, without a Refresh button (issue #86)", async () => {
    const { calls } = start();
    await screen.findByRole("heading", { level: 1, name: "Adapters" });
    expect(screen.queryByRole("button", { name: "Refresh" })).toBeNull();
    expect(screen.getByRole("button", { name: "Restart Bluetooth" })).toBeInTheDocument();
    // On opening the screen, and when the window gets the focus back
    await waitFor(() => expect(calls.filter((c) => c.method === "adapter.list").length).toBe(1));
    await waitFor(() => expect(calls.some((c) => c.method === "adapter.firmware_files")).toBe(true));
    // Once that request has its answer (one at a time)
    await new Promise((r) => setTimeout(r, 10));
    fireEvent.focus(window);
    await waitFor(() => expect(calls.filter((c) => c.method === "adapter.list").length).toBe(2));
  });

  it("adapter names", () => {
    const a = { display_name: "RTL8761BU (0x0BDA:0xA729)", vendor: "Realtek", windows_name: null, realtek_chip: "RTL8761BU" };
    // Windows' Bluetooth never ran it: vendor and chip
    expect(adapterName(a)).toBe("Realtek RTL8761BU");
    expect(adapterName({ ...a, display_name: "MediaTek (0x0E8D:0x7925) [unsupported]", vendor: "MediaTek", realtek_chip: null })).toBe(
      "MediaTek",
    );
    expect(adapterName({ ...a, windows_name: "Realtek Bluetooth 5.3 Adapter" })).toBe("Realtek Bluetooth 5.3 Adapter (RTL8761BU)");
    // The chip once only
    expect(adapterName({ ...a, windows_name: "Realtek RTL8761BU Bluetooth" })).toBe("Realtek RTL8761BU Bluetooth");
    expect(adapterIds({ address: null, vid: 0x0bda, pid: 0xa729, bt_version: null })).toBe("0BDA:A729");
    // A Realtek adapter's USB serial gives the address before any stack ran it
    expect(adapterIds({ address: "00:E0:4C:23:99:87", vid: 0x0bda, pid: 0xa729, bt_version: null })).toBe(
      "00:E0:4C:23:99:87 | 0BDA:A729",
    );
    expect(adapterIdsHint({ address: "00:E0:4C:23:99:87", bt_version: null })).toMatch(/Bluetooth version show once/);
    expect(adapterIdsHint({ address: "00:E0:4C:23:99:87", bt_version: "5.1" })).toBeUndefined();
    // The driver in two names only, whatever the manufacturer
    expect(driverText("bluetooth").text).toBe("Original driver");
    expect(driverText("other").text).toBe("Original driver");
    expect(driverText("winusb").text).toBe("WinUSB driver");
    expect(driverText("none").text).toBe("No driver");
    // An Intel adapter on WinUSB without a product string: the mark is a tag
    const intel = { ...a, display_name: "Intel (0x8087:0x0033) [experimental]", vendor: "Intel", realtek_chip: null };
    expect(adapterName(intel)).toBe("Intel");
    expect(adapterExperimental(intel)).toBe(true);
    expect(adapterExperimental(a)).toBe(false);
  });

  it("opens the firmware folder", async () => {
    const { mock } = start();
    const open = vi.spyOn(mock, "openFolder");
    fireEvent.click(await screen.findByRole("button", { name: "Open Firmware Folder" }));
    await waitFor(() => expect(open).toHaveBeenCalledWith("C:\\Users\\you\\AppData\\Roaming\\A2DPWB\\firmware"));
  });

  it("downloads the missing firmware after the user has read the terms", async () => {
    const { calls } = start();
    // A chip whose files are not in the sample folder
    const chip = (await screen.findByLabelText("Bluetooth Chip")) as HTMLSelectElement;
    fireEvent.change(chip, { target: { value: [...chip.options].find((o) => o.text.startsWith("RTL8852BU"))!.value } });
    expect(await screen.findByText("Firmware not found")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Download Firmware" }));
    const dialog = await screen.findByRole("dialog", { name: "Download the Realtek firmware" });
    expect(dialog).toHaveTextContent("LICENCE.rtlwifi_firmware.txt");
    expect(calls.some((c) => c.method === "adapter.download_firmware")).toBe(false);
    fireEvent.click([...dialog.querySelectorAll("button")].find((b) => b.textContent === "Download Firmware")!);
    await waitFor(() =>
      expect(calls.find((c) => c.method === "adapter.download_firmware")?.params).toEqual({ file: "rtl8852bu_fw.bin" }),
    );
    expect(await screen.findByText("Firmware downloaded. Bluetooth loads it at the next connection.")).toBeInTheDocument();
    await waitFor(() => expect(screen.queryByText("Firmware not found")).not.toBeInTheDocument());
  });

  /** Opens the download dialog for a chip with no files and starts the
   *  download, which the test answers itself */
  async function startDownload() {
    const { mock } = start();
    let answer!: { resolve: (v: unknown) => void; reject: (e: unknown) => void };
    vi.spyOn(mock as any, "downloadFirmware").mockImplementation(
      () => new Promise((resolve, reject) => (answer = { resolve, reject })),
    );
    const chip = (await screen.findByLabelText("Bluetooth Chip")) as HTMLSelectElement;
    fireEvent.change(chip, { target: { value: [...chip.options].find((o) => o.text.startsWith("RTL8852BU"))!.value } });
    expect(await screen.findByText("Firmware not found")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Download Firmware" }));
    const dialog = await screen.findByRole("dialog", { name: "Download the Realtek firmware" });
    fireEvent.click([...dialog.querySelectorAll("button")].find((b) => b.textContent === "Download Firmware")!);
    await waitFor(() => expect(answer).toBeDefined());
    return { dialog, answer: () => answer };
  }

  it("shows a spinner and stays open while downloading", async () => {
    const { dialog, answer } = await startDownload();
    expect(await within(dialog).findByRole("status")).toHaveTextContent("Downloading the firmware from kernel.org");
    expect(dialog.querySelector(".spinner.big")).not.toBeNull();
    // Neither Cancel, Escape nor the backdrop closes it meanwhile
    const cancel = within(dialog).getByRole("button", { name: "Cancel" });
    expect(cancel).toBeDisabled();
    fireEvent.keyDown(dialog, { key: "Escape" });
    fireEvent.click(dialog.parentElement!);
    expect(screen.getByRole("dialog", { name: "Download the Realtek firmware" })).toBeInTheDocument();
    answer().resolve([]);
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("does not call the UI's own timeout a failed download", async () => {
    const { answer } = await startDownload();
    answer().reject({ code: "ipc.timeout", message: { key: "ipc.error.timeout", args: ["adapter.download_firmware"] } });
    expect(await screen.findByText(/The download is taking a while/)).toBeInTheDocument();
    expect(screen.queryByText(/not responding/i)).not.toBeInTheDocument();
  });
});
