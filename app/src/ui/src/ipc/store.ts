// The core's state as the UI sees it: the last Snapshot with each Event
// applied (each event replaces one part), plus the streams the UI
// subscribes to (link samples, log lines, the receiver test's state). The UI keeps no state of its
// own beyond this and view state.
//
// Revisions: every state event carries the next revision; link samples
// and log lines are not state and carry none of their own. A gap means a
// lost event, so the snapshot is fetched again (state.snapshot). Its answer
// travels apart from the events and says nothing about which events it
// already contains, so the state events that arrive while it is on its way
// are kept and applied again on top of it, in order: each replaces a whole
// part, so the newest one of each part ends up in place.

import { batch, createSignal } from "solid-js";
import { createStore, reconcile } from "solid-js/store";
import type { ConnStatus, Transport } from "./transport";
import {
  IpcError,
  type ArgsOf,
  type Event,
  type FlowView,
  type LinkSample,
  type LogLevel,
  type LogLine,
  type MethodName,
  type ReceiverState,
  type ResultOf,
  type ServerMessage,
  type Snapshot,
} from "./types";

/** Link samples kept: one minute at 4 per second */
export const LINK_KEEP = 240;
/** Log lines kept in the window (the core keeps its own files) */
export const LOG_KEEP = 5000;

const LEVELS: LogLevel[] = ["error", "warn", "info", "debug", "trace"];

export interface CoreState {
  conn: ConnStatus;
  /** Revision of the snapshot plus the events applied; null before the first */
  rev: number | null;
  snap: Snapshot | null;
}

export type Core = ReturnType<typeof createCore>;

export function createCore(transport: Transport) {
  const [state, setState] = createStore<CoreState>({ conn: { state: "connecting" }, rev: null, snap: null });

  // The streams change several times a second: plain arrays and a signal
  // that says "changed" are cheaper than store arrays
  const linkBuf: LinkSample[] = [];
  const [linkTick, setLinkTick] = createSignal(0);
  const logBuf: LogLine[] = [];
  const [logTick, setLogTick] = createSignal(0);
  /** Lines up to this time were cleared: the kept lines do not bring them back */
  let logsClearedAt = -1;
  const [flow, setFlow] = createSignal<FlowView | null>(null);
  const [receiver, setReceiver] = createSignal<ReceiverState | null>(null);

  /** A state.snapshot on its way: the revision when asked, and the state events since */
  let pending: { from: number | null; events: { rev: number; ev: Event }[] } | null = null;

  async function request<M extends MethodName>(method: M, ...args: ArgsOf<M>): Promise<ResultOf<M>> {
    try {
      return (await transport.call(method, args[0])) as ResultOf<M>;
    } catch (e) {
      throw IpcError.from(e);
    }
  }

  function setSnapshot(snap: Snapshot, rev: number | null) {
    batch(() => {
      setState("snap", state.snap ? reconcile(snap) : snap);
      setState("rev", rev);
    });
  }

  function resync() {
    if (pending) return;
    // A window that opens late has no revision yet: the next event after
    // the answer then sets it, without a gap check
    const p = { from: state.rev, events: [] as { rev: number; ev: Event }[] };
    pending = p;
    request("state.snapshot")
      .then((snap) => {
        // A snapshot message of the core came first and replaced this
        if (pending !== p) return;
        pending = null;
        batch(() => {
          setSnapshot(snap, p.from);
          for (const e of p.events) {
            if (p.from === null || e.rev > p.from) {
              applyState(e.ev);
              if (e.rev > (state.rev ?? 0)) setState("rev", e.rev);
            }
          }
        });
      })
      .catch(() => {
        // Not connected: the next connection brings a snapshot anyway
        if (pending === p) pending = null;
      });
  }

  function applyData(ev: Event) {
    switch (ev.kind) {
      case "link":
        linkBuf.push(ev.value);
        if (linkBuf.length > LINK_KEEP) linkBuf.splice(0, linkBuf.length - LINK_KEEP);
        setLinkTick((n) => n + 1);
        return;
      case "log":
        logBuf.push(ev.value);
        if (logBuf.length > LOG_KEEP) logBuf.splice(0, logBuf.length - LOG_KEEP);
        setLogTick((n) => n + 1);
        return;
      case "receiver":
        setReceiver(ev.value);
        return;
    }
  }

  function applyState(ev: Event) {
    // Before the first snapshot there is nothing to patch: it comes next
    if (!state.snap) return;
    switch (ev.kind) {
      case "adapter":
        setState("snap", "adapter", reconcile(ev.value));
        break;
      case "devices":
        setState("snap", "devices", reconcile(ev.value));
        break;
      case "scanning":
        setState("snap", "scanning", ev.value);
        break;
      case "le_scanning":
        setState("snap", "le_scanning", ev.value);
        break;
      case "stream":
        setState("snap", "stream", reconcile(ev.value));
        break;
      case "profiles":
        batch(() => {
          setState("snap", "profiles", reconcile(ev.value.profiles));
          setState("snap", "active_profile", ev.value.active_profile);
        });
        break;
      case "settings":
        setState("snap", "settings", reconcile(ev.value));
        break;
      case "update":
        setState("snap", "update", reconcile(ev.value));
        break;
      case "capture":
        setState("snap", "capture", ev.value === null ? null : reconcile(ev.value));
        break;
      case "v1_running":
        setState("snap", "v1_running", ev.value);
        break;
      case "verbose":
        setState("snap", "verbose", ev.value === null ? null : reconcile(ev.value));
        break;
      case "crashes":
        setState("snap", "crashes", reconcile(ev.value));
        break;
      default:
        // An event kind of a newer core: the next snapshot has it
        break;
    }
  }

  function applyEvent(rev: number, ev: Event) {
    if (ev.kind === "link" || ev.kind === "log" || ev.kind === "receiver") {
      applyData(ev);
      return;
    }
    if (ev.kind === "bye") {
      setState("snap", null);
      return;
    }
    const cur = state.rev;
    if (cur !== null) {
      if (rev <= cur) return; // already in the snapshot
      if (rev > cur + 1) resync(); // lost one: the snapshot fixes it
    }
    if (pending) pending.events.push({ rev, ev });
    setState("rev", rev);
    applyState(ev);
  }

  function onMessage(m: ServerMessage) {
    switch (m.type) {
      case "snapshot":
        // The core's own snapshot (after Hello, or when this client fell
        // behind) has its true revision: a requested one is not needed
        pending = null;
        setSnapshot(m.state, m.rev);
        return;
      case "event":
        applyEvent(m.rev, m.event);
        return;
      default:
        // welcome / reject / response: the transport handles them
        return;
    }
  }

  function onStatus(s: ConnStatus) {
    const was = state.conn.state;
    setState("conn", reconcile(s));
    if (s.state === "connected" && was !== "connected") {
      // A window opened after Hello missed the snapshot; on a fresh
      // connection the core sends one too (harmless)
      lastSent = undefined;
      lastLogs = null;
      linkOn = false;
      resync();
      pushSubscription();
    }
  }

  // ---- Subscriptions (diag.subscribe) -----------------------------------
  // Views ask for what they show while they are mounted; the window asks
  // for nothing while it is hidden. The backend merges both windows.

  const linkWants = new Set<symbol>();
  const receiverWants = new Set<symbol>();
  const logWants = new Map<symbol, LogLevel>();
  let lastSent: string | undefined;
  /** The log level of the last subscription sent */
  let lastLogs: LogLevel | null = null;
  /** Whether the last subscription sent asked for link samples */
  let linkOn = false;
  let pushQueued = false;

  const lineKey = (l: LogLine) => `${l.time_ms}\u0000${l.target}\u0000${l.message}`;

  /** Puts the core's kept lines before the live ones (the live lines that
   *  came in meanwhile may be among them) */
  function mergeRecent(recent: LogLine[]) {
    const kept = recent.filter((l) => l.time_ms > logsClearedAt);
    const last = kept.length > 0 ? kept[kept.length - 1].time_ms : -1;
    const seen = new Set(kept.filter((l) => l.time_ms === last).map(lineKey));
    const live = logBuf.filter((l) => l.time_ms > last || (l.time_ms === last && !seen.has(lineKey(l))));
    logBuf.length = 0;
    logBuf.push(...kept, ...live);
    if (logBuf.length > LOG_KEEP) logBuf.splice(0, logBuf.length - LOG_KEEP);
    setLogTick((n) => n + 1);
  }

  function windowHidden() {
    return typeof document !== "undefined" && document.visibilityState === "hidden";
  }

  function pushSubscription() {
    if (pushQueued) return;
    pushQueued = true;
    // Mounting one view and unmounting another in the same tick sends once
    queueMicrotask(() => {
      pushQueued = false;
      const hidden = windowHidden();
      const link = !hidden && linkWants.size > 0;
      const receiver = !hidden && receiverWants.size > 0;
      let logs: LogLevel | null = null;
      if (!hidden) {
        for (const l of logWants.values()) {
          if (logs === null || LEVELS.indexOf(l) > LEVELS.indexOf(logs)) logs = l;
        }
      }
      const key = JSON.stringify({ logs, link, receiver });
      if (key === lastSent || state.conn.state !== "connected") return;
      const before = lastLogs;
      lastSent = key;
      lastLogs = logs;
      // Samples from an earlier subscription would join the new ones across
      // the gap (a step in the charts, the drops meanwhile in one sample)
      if (link && !linkOn) {
        linkBuf.length = 0;
        setLinkTick((n) => n + 1);
      }
      linkOn = link;
      request("diag.subscribe", { logs, link, receiver })
        .then(() => {
          // Lines logged while nobody listened (the window was closed, or
          // opened later): the core keeps the recent ones
          if (logs !== null && (before === null || LEVELS.indexOf(logs) > LEVELS.indexOf(before))) {
            return request("diag.recent_logs", { level: logs }).then((r) => mergeRecent(r ?? []));
          }
        })
        .catch(() => {
          lastSent = undefined;
          lastLogs = null;
          linkOn = false;
        });
    });
  }

  if (typeof document !== "undefined") {
    document.addEventListener("visibilitychange", pushSubscription);
  }

  /** Link samples while the caller lives; returns the release */
  function wantLink(): () => void {
    const k = Symbol("link");
    linkWants.add(k);
    pushSubscription();
    return () => {
      linkWants.delete(k);
      pushSubscription();
    };
  }

  /** The receiver test's state (receiver()) while the caller lives; also
   *  asks for it at once */
  function wantReceiver(): () => void {
    const k = Symbol("receiver");
    receiverWants.add(k);
    pushSubscription();
    if (state.conn.state === "connected") {
      request("receiver.status")
        .then(setReceiver)
        .catch(() => {});
    }
    return () => {
      receiverWants.delete(k);
      pushSubscription();
    };
  }

  /** Log lines at `level` and above while the caller lives */
  function wantLogs(level: LogLevel): () => void {
    const k = Symbol("logs");
    logWants.set(k, level);
    pushSubscription();
    return () => {
      logWants.delete(k);
      pushSubscription();
    };
  }

  // ---- The connection flow (diag.flow) -----------------------------------
  // The core follows every log line; a view asks for its flow while mounted

  const FLOW_REFRESH_MS = 1000;
  let flowWants = 0;
  let flowTimer: ReturnType<typeof setInterval> | null = null;

  function refreshFlow() {
    if (state.conn.state !== "connected" || windowHidden()) return;
    request("diag.flow")
      .then(setFlow)
      .catch(() => {});
  }

  /** Keeps flow() current while the caller lives; returns the release */
  function wantFlow(): () => void {
    flowWants++;
    if (flowWants === 1) {
      refreshFlow();
      flowTimer = setInterval(refreshFlow, FLOW_REFRESH_MS);
    }
    return () => {
      flowWants--;
      if (flowWants === 0 && flowTimer !== null) {
        clearInterval(flowTimer);
        flowTimer = null;
      }
    };
  }

  void transport.start(onMessage, onStatus);

  return {
    state,
    transport,
    request,
    /** The last LINK_KEEP samples (reactive) */
    link: (): readonly LinkSample[] => {
      linkTick();
      return linkBuf;
    },
    /** The last LOG_KEEP lines (reactive) */
    logs: (): readonly LogLine[] => {
      logTick();
      return logBuf;
    },
    clearLogs() {
      if (logBuf.length > 0) logsClearedAt = logBuf[logBuf.length - 1].time_ms;
      logBuf.length = 0;
      setLogTick((n) => n + 1);
    },
    /** The core's connection flow, while someone wants it (reactive) */
    flow,
    /** The receiver test's last state, while someone wants it (reactive) */
    receiver,
    wantFlow,
    wantLink,
    wantLogs,
    wantReceiver,
  };
}
