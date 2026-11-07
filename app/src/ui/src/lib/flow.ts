// The connection flow of the Diagnostics screen: eleven steps from "start"
// to "connection upkeep", each with a status, derived from the log lines as
// v1's debug console did (debug_log_model.cpp on the develop branch). The real core
// follows every line itself (crates/core/src/diag.rs, method diag.flow);
// this tracker is the mock core's, with rules for the messages of the v2
// core (crates/core/src/stream.rs and service.rs, crates/bt-host/src).

import { englishText, tm } from "../i18n";
import type { FlowView, LogLine, Msg } from "../ipc/types";

export type StepStatus = "pending" | "running" | "done" | "warning" | "failed";

export const STEPS = [
  "start",
  "btstack",
  "connect",
  "caps",
  "codec",
  "capture",
  "config",
  "encoder",
  "stream",
  "audio",
  "link",
] as const;
export type Step = (typeof STEPS)[number];

export interface FlowStep {
  status: StepStatus;
  /** Core time of the line that set the status */
  time_ms: number | null;
  detail: string;
}

export interface FlowSession {
  id: number;
  start_ms: number;
  stop_ms: number | null;
  steps: Record<Step, FlowStep>;
  reconnects: number;
  /** Failed connection attempts before the signaling connection came up */
  connect_retries: number;
}

type Action = "done" | "running" | "warning" | "failed" | "detail" | "retry";

interface Rule {
  step: Step;
  /** Only lines of this log target (or its submodules) */
  target?: string;
  /** Substring of the message, or a pattern where a substring would also
   *  match other lines ("capture: " is in "HCI capture: started") */
  needle: string | RegExp;
  action: Action;
  /** Detail: the text after the needle, the whole message, or none */
  detail: "after" | "whole" | "none";
}

const RULES: Rule[] = [
  // Before "connecting to ", which it contains
  { step: "link", needle: "reconnecting to ", action: "detail", detail: "whole" },
  { step: "btstack", needle: "state Connecting: Initializing BTstack", action: "running", detail: "none" },
  { step: "btstack", needle: "controller: ", action: "detail", detail: "after" },
  { step: "btstack", needle: "Realtek firmware likely NOT loaded", action: "warning", detail: "whole" },
  { step: "btstack", needle: "HCI initialization failed", action: "failed", detail: "whole" },
  { step: "btstack", needle: "HCI initialization timed out", action: "failed", detail: "whole" },
  { step: "btstack", needle: "could not open the Bluetooth adapter", action: "failed", detail: "whole" },
  { step: "btstack", needle: "HCI ready", action: "done", detail: "none" },
  { step: "connect", needle: "connecting to ", action: "running", detail: "after" },
  { step: "connect", needle: "HCI ACL connection established", action: "detail", detail: "whole" },
  { step: "connect", needle: "signaling connection established", action: "done", detail: "whole" },
  // A failed attempt is retried; the state Error line blames the step if
  // none succeeds
  { step: "connect", needle: "signaling connection failed", action: "detail", detail: "whole" },
  { step: "connect", target: "bt_host", needle: /^connection attempt \d+\/\d+ failed/, action: "retry", detail: "whole" },
  { step: "caps", needle: "capability discovery complete", action: "done", detail: "after" },
  { step: "codec", needle: "codec fallback: ", action: "warning", detail: "after" },
  { step: "codec", needle: "codec selected: ", action: "done", detail: "after" },
  // audio_capture's own "audio capture: loopback of ..." describes the
  // source; the stream's "audio capture: ..." is the failure
  { step: "capture", target: "audio_capture", needle: /^audio capture: /, action: "detail", detail: "after" },
  { step: "capture", target: "bluecaster_core::stream", needle: /^audio capture: /, action: "failed", detail: "after" },
  { step: "capture", needle: "capturing app ", action: "detail", detail: "after" },
  { step: "capture", needle: /^capture: /, action: "done", detail: "after" },
  { step: "config", needle: "codec configured", action: "running", detail: "after" },
  { step: "config", needle: "stream establishment failed", action: "failed", detail: "whole" },
  { step: "config", needle: "stream established", action: "done", detail: "after" },
  { step: "encoder", needle: "encoder ready: ", action: "done", detail: "after" },
  { step: "encoder", needle: /^encoder: /, action: "failed", detail: "after" },
  { step: "stream", needle: "streaming started", action: "done", detail: "none" },
  { step: "audio", needle: "audio capture start: ", action: "failed", detail: "after" },
  { step: "audio", needle: "state Streaming", action: "done", detail: "none" },
  { step: "link", needle: "connection lost, reconnecting", action: "running", detail: "whole" },
  { step: "link", needle: ", reconnecting", action: "running", detail: "whole" },
  { step: "link", needle: "reconnect attempt ", action: "detail", detail: "whole" },
  { step: "link", needle: "reconnected on attempt ", action: "warning", detail: "whole" },
];

const START = "stream: device ";
const STOP = "streaming stopped";
const ERROR_STATE = "state Error: ";

function targetMatches(target: string, prefix: string): boolean {
  return target === prefix || target.startsWith(prefix + "::");
}

function afterNeedle(text: string, needle: string | RegExp): string {
  let rest = "";
  if (typeof needle === "string") {
    const i = text.indexOf(needle);
    rest = i < 0 ? "" : text.slice(i + needle.length);
  } else {
    const m = needle.exec(text);
    rest = m ? text.slice(m.index + m[0].length) : "";
  }
  let s = rest.trim();
  if (s.endsWith("...")) s = s.slice(0, -3);
  if (s.startsWith("(") && s.endsWith(")")) s = s.slice(1, -1);
  return s;
}

const finished = (s: StepStatus) => s === "done" || s === "warning";

function emptySteps(): Record<Step, FlowStep> {
  return Object.fromEntries(STEPS.map((s) => [s, { status: "pending", time_ms: null, detail: "" }])) as Record<
    Step,
    FlowStep
  >;
}

export class FlowTracker {
  session: FlowSession | null = null;
  private nextId = 1;

  /** Takes one log line; true if the flow changed */
  feed(line: LogLine): boolean {
    const t = line.message;
    if (t.includes(START)) {
      this.session = {
        id: this.nextId++,
        start_ms: line.time_ms,
        stop_ms: null,
        steps: emptySteps(),
        reconnects: 0,
        connect_retries: 0,
      };
      this.set("start", "done", line, afterNeedle(t, START));
      return true;
    }
    const s = this.session;
    if (!s || s.stop_ms !== null) return false;
    if (t.includes(STOP)) {
      s.stop_ms = line.time_ms;
      return true;
    }
    if (t.includes(ERROR_STATE)) {
      // Blame the first step that did not complete
      const detail = afterNeedle(t, ERROR_STATE);
      const step = STEPS.find((st) => !finished(s.steps[st].status) || st === "link");
      if (step) this.set(step, "failed", line, detail);
      return true;
    }
    for (const r of RULES) {
      if (r.target && !targetMatches(line.target, r.target)) continue;
      if (typeof r.needle === "string" ? !t.includes(r.needle) : !r.needle.test(t)) continue;
      const detail = r.detail === "after" ? afterNeedle(t, r.needle) : r.detail === "whole" ? t : "";
      // While reconnecting, the steps keep the first connection's results:
      // what goes wrong in an attempt belongs to the upkeep step
      if (s.steps.link.status === "running" && r.step !== "link") {
        if (r.action === "failed" || r.action === "warning") s.steps.link.detail = t;
        return true;
      }
      if (r.action === "detail") {
        if (detail) s.steps[r.step].detail = detail;
      } else if (r.action === "retry") {
        s.connect_retries++;
        if (detail) s.steps[r.step].detail = detail;
      } else if (r.step === "connect" && r.action === "done" && s.connect_retries > 0) {
        this.set(r.step, "warning", line, `${detail} (after ${s.connect_retries} failed attempt(s))`);
      } else {
        this.set(r.step, r.action, line, detail);
      }
      if (r.step === "audio" && r.action === "done" && s.steps.link.status === "pending") {
        this.set("link", "done", line, "");
      }
      if (r.step === "link" && r.action === "running") s.reconnects++;
      return true;
    }
    return false;
  }

  private set(step: Step, status: StepStatus, line: LogLine, detail: string) {
    const st = this.session!.steps[step];
    switch (status) {
      case "failed":
        if (st.status === "failed") return; // keep the first cause
        break;
      case "warning":
        if (st.status === "failed") return;
        break;
      case "done":
        if (st.status === "failed") return;
        if (st.status === "warning") {
          // Completed, but the warning stays visible
          if (detail) st.detail = detail;
          return;
        }
        break;
      case "running":
        // Only the upkeep step goes back from finished to running
        if (st.status === "failed") return;
        if (finished(st.status) && step !== "link") return;
        break;
      case "pending":
        break;
    }
    st.status = status;
    st.time_ms = line.time_ms;
    if (detail) st.detail = detail;
  }
}

/**
 * A step's detail in the UI language: the message the core computed for it
 * (FlowStepView.detail_msg: the audio rate, the link's reconnects)
 * translated, with what the core put after that message in `detail`
 * ("2 reconnect(s) - <the line's detail>"); else the log line's own detail
 */
export function stepDetail(x: { detail: string; detail_msg?: Msg | null }): string {
  if (!x.detail_msg) return x.detail;
  const text = tm(x.detail_msg);
  const en = englishText(x.detail_msg);
  return x.detail.startsWith(`${en} - `) ? text + x.detail.slice(en.length) : text;
}

/** A session as the core's diag.flow answers it */
export function flowView(s: FlowSession | null): FlowView {
  if (!s) return { session: 0, start_ms: 0, stop_ms: null, reconnects: 0, steps: [] };
  return {
    session: s.id,
    start_ms: s.start_ms,
    stop_ms: s.stop_ms,
    reconnects: s.reconnects,
    steps: STEPS.map((step) => {
      const st = s.steps[step];
      return {
        step,
        status: st.status,
        elapsed_ms: st.time_ms === null ? null : st.time_ms - s.start_ms,
        detail: st.detail,
        detail_msg: null,
      };
    }),
  };
}
