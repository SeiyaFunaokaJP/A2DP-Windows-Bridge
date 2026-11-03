import { createRoot } from "solid-js";
import { describe, expect, it } from "vitest";
import { MockCore, sampleSnapshot } from "./mock";
import { createCore } from "./store";
import type { ConnStatus, Transport } from "./transport";
import type { Event, ServerMessage } from "./types";

/** A transport the test drives by hand */
function fakeTransport() {
  const calls: { method: string; params: unknown }[] = [];
  // The answer to state.snapshot, released by the test
  let answerSnapshot: (s: unknown) => void = () => {};
  let deliver: (m: ServerMessage) => void = () => {};
  let status: (s: ConnStatus) => void = () => {};
  // Answers of other methods, set by the test
  const answers: Record<string, unknown> = {};
  const transport: Transport = {
    window: "main",
    mica: false,
    version: "test",
    licensesDir: null,
    async start(onMessage, onStatus) {
      deliver = onMessage;
      status = onStatus;
    },
    async call(method, params) {
      calls.push({ method, params });
      if (method === "state.snapshot") return new Promise((r) => (answerSnapshot = r));
      return answers[method] ?? null;
    },
    async showMain() {},
    async hideWindow() {},
    async openUrl() {},
    async openFolder() {},
    async revealPath() {},
    async setTheme() {},
    onNavigate() {},
    onMinimized() {},
    async updateStatus() {
      return { state: "idle" };
    },
    async updateInstall() {},
    onUpdatePhase() {},
  };
  return {
    transport,
    calls,
    answers,
    send: (m: ServerMessage) => deliver(m),
    status: (s: ConnStatus) => status(s),
    event: (rev: number, event: Event) => deliver({ type: "event", rev, event }),
    answerSnapshot: (s: unknown) => answerSnapshot(s),
  };
}

const tick = () => new Promise((r) => setTimeout(r, 0));

describe("core store", () => {
  it("takes the snapshot and applies events in order", async () => {
    const f = fakeTransport();
    const core = createRoot(() => createCore(f.transport));
    await tick();
    expect(core.state.snap).toBeNull();
    f.send({ type: "snapshot", rev: 10, state: sampleSnapshot() });
    expect(core.state.rev).toBe(10);
    expect(core.state.snap?.scanning).toBe(false);

    f.event(11, { kind: "scanning", value: true });
    expect(core.state.snap?.scanning).toBe(true);
    expect(core.state.rev).toBe(11);

    f.event(12, {
      kind: "stream",
      value: { state: "connecting", message: { key: "status.connecting", args: [] }, profile: "X", info: null, volume: null, battery: null, battery_refused: false },
    });
    expect(core.state.snap?.stream.state).toBe("connecting");

    // The LE Audio part of a scan (state, with a revision)
    f.event(13, { kind: "le_scanning", value: true });
    expect(core.state.snap?.le_scanning).toBe(true);
    expect(core.state.rev).toBe(13);
    f.event(14, { kind: "le_scanning", value: false });
    expect(core.state.snap?.le_scanning).toBe(false);

    f.event(15, { kind: "profiles", value: { profiles: [], active_profile: null } });
    expect(core.state.snap?.profiles).toHaveLength(0);
    expect(core.state.snap?.active_profile).toBeNull();
    // No resync was needed
    expect(f.calls.filter((c) => c.method === "state.snapshot")).toHaveLength(0);
  });

  it("asks for a new snapshot when a revision is missing", async () => {
    const f = fakeTransport();
    const core = createRoot(() => createCore(f.transport));
    await tick();
    f.send({ type: "snapshot", rev: 1, state: sampleSnapshot() });
    f.event(2, { kind: "scanning", value: true });
    expect(f.calls).toHaveLength(0);
    // Revision 3 was lost
    f.event(4, { kind: "scanning", value: false });
    expect(f.calls.map((c) => c.method)).toEqual(["state.snapshot"]);
    // The newer event is applied meanwhile
    expect(core.state.snap?.scanning).toBe(false);
    // Only one request while it is on its way
    f.event(6, { kind: "scanning", value: true });
    expect(f.calls.filter((c) => c.method === "state.snapshot")).toHaveLength(1);
    // The answer was taken before events 4 and 6 (it says scanning: false
    // and has the lost event 3's devices): those events go on top again
    const s = sampleSnapshot();
    s.scanning = false;
    s.devices = [];
    f.answerSnapshot(s);
    await tick();
    expect(core.state.snap?.scanning).toBe(true);
    expect(core.state.snap?.devices).toEqual([]);
    expect(core.state.rev).toBe(6);
    // Older events after it are ignored, the next one applies
    f.event(5, { kind: "scanning", value: false });
    expect(core.state.snap?.scanning).toBe(true);
    f.event(7, { kind: "scanning", value: false });
    expect(core.state.snap?.scanning).toBe(false);
    expect(f.calls.filter((c) => c.method === "state.snapshot")).toHaveLength(1);
  });

  it("prefers the core's own snapshot to a requested one", async () => {
    const f = fakeTransport();
    const core = createRoot(() => createCore(f.transport));
    await tick();
    f.status({ state: "connected", server: "core" });
    await tick();
    expect(f.calls[0].method).toBe("state.snapshot");
    const own = sampleSnapshot();
    own.scanning = true;
    f.send({ type: "snapshot", rev: 10, state: own });
    f.answerSnapshot(sampleSnapshot());
    await tick();
    expect(core.state.snap?.scanning).toBe(true);
    expect(core.state.rev).toBe(10);
  });

  it("keeps link samples and log lines without advancing past state events", async () => {
    const f = fakeTransport();
    const core = createRoot(() => createCore(f.transport));
    await tick();
    f.send({ type: "snapshot", rev: 5, state: sampleSnapshot() });
    // Subscription data may carry the current revision...
    f.event(5, { kind: "log", value: { time_ms: 1, level: "info", target: "t", message: "stream: device AA codec Ldac" } });
    f.event(5, {
      kind: "link",
      value: {
        time_ms: 2,
        rssi: -3,
        afh_in_use: 79,
        afh_usable: 79,
        afh_avoided: "",
        bytes_per_second: 1000,
        packets_per_second: 10,
        queue_depth: 1,
        dropped: 0,
        capture_dropped: 0,
        airtime: 0.3,
      },
    });
    expect(core.logs()).toHaveLength(1);
    expect(core.link()).toHaveLength(1);
    // ...and the next state event is still in order
    f.event(6, { kind: "scanning", value: true });
    expect(f.calls.filter((c) => c.method === "state.snapshot")).toHaveLength(0);
  });

  it("re-requests the snapshot when the connection comes up, and merges subscriptions", async () => {
    const f = fakeTransport();
    const core = createRoot(() => createCore(f.transport));
    await tick();
    const releaseLink = core.wantLink();
    const releaseLogs = core.wantLogs("info");
    const releaseDebug = core.wantLogs("debug");
    await tick();
    // Not connected yet: nothing sent
    expect(f.calls).toHaveLength(0);
    f.status({ state: "connected", server: "core" });
    await tick();
    const subs = () => f.calls.filter((c) => c.method === "diag.subscribe").map((c) => c.params);
    expect(f.calls.map((c) => c.method)).toEqual(["state.snapshot", "diag.subscribe", "diag.recent_logs"]);
    expect(subs()).toEqual([{ logs: "debug", link: true, receiver: false }]);
    releaseDebug();
    releaseLink();
    await tick();
    expect(subs()[1]).toEqual({ logs: "info", link: false, receiver: false });
    releaseLogs();
    await tick();
    expect(subs()[2]).toEqual({ logs: null, link: false, receiver: false });
  });

  it("starts the link samples afresh when they are asked for again", async () => {
    const f = fakeTransport();
    const core = createRoot(() => createCore(f.transport));
    await tick();
    f.status({ state: "connected", server: "core" });
    f.send({ type: "snapshot", rev: 1, state: sampleSnapshot() });
    const sample = (time_ms: number) => ({
      kind: "link" as const,
      value: {
        time_ms,
        rssi: -3,
        afh_in_use: 79,
        afh_usable: 79,
        afh_avoided: "",
        bytes_per_second: 1000,
        packets_per_second: 10,
        queue_depth: 1,
        dropped: 0,
        capture_dropped: 0,
        airtime: 0.3,
      },
    });
    let release = core.wantLink();
    await tick();
    f.event(1, sample(1));
    f.event(1, sample(2));
    // Another view taking over in the same tick keeps them
    release();
    release = core.wantLink();
    await tick();
    expect(core.link()).toHaveLength(2);
    release();
    await tick();
    expect(core.link()).toHaveLength(2);
    // Asked for again later: the old ones would join across the gap
    release = core.wantLink();
    await tick();
    expect(core.link()).toHaveLength(0);
    f.event(1, sample(9));
    expect(core.link().map((s) => s.time_ms)).toEqual([9]);
    release();
  });

  it("follows the receiver test while a view wants it", async () => {
    const f = fakeTransport();
    const core = createRoot(() => createCore(f.transport));
    await tick();
    f.send({ type: "snapshot", rev: 1, state: sampleSnapshot() });
    const first = {
      connection: "connecting" as const,
      detail: "",
      target: "auto",
      receiver: null,
      stats: null,
      stats_age_ms: null,
      sent: null,
      stream_starts: 0,
    };
    f.answers["receiver.status"] = first;
    f.status({ state: "connected", server: "core" });
    await tick();
    const release = core.wantReceiver();
    await tick();
    await tick();
    expect(f.calls.filter((c) => c.method === "diag.subscribe").map((c) => c.params)).toContainEqual({
      logs: null,
      link: false,
      receiver: true,
    });
    expect(core.receiver()).toEqual(first);
    // Its events are data: they take no revision
    f.event(1, { kind: "receiver", value: { ...first, connection: "connected" } });
    expect(core.receiver()?.connection).toBe("connected");
    f.event(2, { kind: "scanning", value: true });
    expect(f.calls.filter((c) => c.method === "state.snapshot")).toHaveLength(1);
    release();
    await tick();
    const subs = f.calls.filter((c) => c.method === "diag.subscribe");
    expect(subs[subs.length - 1].params).toEqual({ logs: null, link: false, receiver: false });
  });

  it("keeps the HCI capture state", async () => {
    const f = fakeTransport();
    const core = createRoot(() => createCore(f.transport));
    await tick();
    f.send({ type: "snapshot", rev: 1, state: sampleSnapshot() });
    expect(core.state.snap?.capture).toBeNull();
    const c = { path: "C:\\x\\hci_20261017_101500.pklg", bytes: 64, started_ms: 1_760_000_000_000 };
    f.event(2, { kind: "capture", value: c });
    expect(core.state.snap?.capture).toEqual(c);
    f.event(3, { kind: "capture", value: { ...c, bytes: 4096 } });
    expect(core.state.snap?.capture?.bytes).toBe(4096);
    f.event(4, { kind: "capture", value: null });
    expect(core.state.snap?.capture).toBeNull();
    expect(core.state.rev).toBe(4);
  });

  it("puts the core's kept lines before the live ones, once each", async () => {
    const f = fakeTransport();
    const core = createRoot(() => createCore(f.transport));
    await tick();
    const line = (time_ms: number, message: string) => ({ time_ms, level: "info" as const, target: "t", message });
    f.answers["diag.recent_logs"] = [line(1, "a"), line(2, "b"), line(3, "c")];
    f.status({ state: "connected", server: "core" });
    core.wantLogs("info");
    // A live line that is also among the kept ones, and a newer one
    f.event(0, { kind: "log", value: line(3, "c") });
    f.event(0, { kind: "log", value: line(4, "d") });
    await tick();
    await tick();
    expect(core.logs().map((l) => l.message)).toEqual(["a", "b", "c", "d"]);
  });

  it("asks the core for the connection flow while a view wants it", async () => {
    const f = fakeTransport();
    const core = createRoot(() => createCore(f.transport));
    await tick();
    f.answers["diag.flow"] = { session: 1, start_ms: 0, stop_ms: null, reconnects: 0, steps: [] };
    f.status({ state: "connected", server: "core" });
    const release = core.wantFlow();
    await tick();
    expect(core.flow()?.session).toBe(1);
    release();
    const asked = f.calls.filter((c) => c.method === "diag.flow").length;
    await new Promise((r) => setTimeout(r, 1100));
    expect(f.calls.filter((c) => c.method === "diag.flow").length).toBe(asked);
  });

  it("turns request failures into IpcError", async () => {
    const f = fakeTransport();
    f.transport.call = async () => {
      throw { code: "adapter.driver_missing", message: { key: "error.btstack_init", args: [] } };
    };
    const core = createRoot(() => createCore(f.transport));
    await expect(core.request("stream.start", { profile: "x" })).rejects.toMatchObject({
      code: "adapter.driver_missing",
      msg: { key: "error.btstack_init" },
    });
  });
});

describe("mock core", () => {
  const wait = (ms = 20) => new Promise((r) => setTimeout(r, ms));

  it("streams without a profile, learns the device's codecs and runs the receiver test", async () => {
    const mock = new MockCore({ speed: 0 });
    const core = createRoot(() => createCore(mock));
    await wait();
    const release = core.wantReceiver();
    const st = await core.request("receiver.connect", { target: "" });
    expect(st.target).toBe("auto");
    await wait();
    expect(core.receiver()?.connection).toBe("connected");
    expect(core.state.snap?.settings.remote_sink).toBe("auto");
    const [found] = await core.request("receiver.discover");
    await core.request("receiver.configure", { mtu: 1005 });
    await core.request("stream.start_custom", { device: found.bt_address, codec: "sbc", quality: "hq", source: "tone" });
    await wait(50);
    const stream = core.state.snap!.stream;
    expect(stream.state).toBe("streaming");
    expect(stream.profile).toBeNull();
    expect(stream.info?.source).toEqual({ kind: "tone", name: "", waiting: false });
    expect(core.state.snap!.devices.find((d) => d.address === found.bt_address)?.codecs).toContain("ldac");
    await expect(
      core.request("stream.start_custom", { device: "nope", codec: "sbc", quality: "hq", source: "tone" }),
    ).rejects.toMatchObject({ code: "profile.invalid_address" });
    release();
    mock.dispose();
  });

  it("reports the capture, the flow's lines and failed update checks", async () => {
    const mock = new MockCore({ speed: 0, scenario: "offline" });
    const core = createRoot(() => createCore(mock));
    await wait();
    const path = await core.request("diag.capture_start");
    await wait();
    expect(core.state.snap?.capture?.path).toBe(path);
    expect(path).toMatch(/hci_\d{8}_\d{6}\.pklg$/);
    const done = await core.request("diag.capture_stop");
    expect(done?.path).toBe(path);
    await wait();
    expect(core.state.snap?.capture).toBeNull();

    await core.request("stream.start", { profile: "WH-1000XM5 (LDAC)" });
    await wait(50);
    const flow = await core.request("diag.flow");
    expect(flow.first_seq).toBeGreaterThan(0);
    const connect = flow.steps.find((s) => s.step === "connect")!;
    const lines = await core.request("diag.recent_logs", { level: "trace" });
    const line = lines.find((l) => l.seq === connect.seq);
    expect(line?.time_ms).toBe(connect.time_ms);
    expect(line?.session).toBe(flow.session);

    const info = await core.request("diag.info");
    expect(info.text).toContain("<addr-1>");
    expect(info.text).not.toContain("AC:80:0A");
    expect((await core.request("diag.info", { mask: false })).text).toContain("AC:80:0A:12:34:56");

    await expect(core.request("app.check_update")).rejects.toMatchObject({ code: "update.failed" });
    expect(core.state.snap?.update.error?.key).toBe("update.error");
    mock.dispose();
  });
});
