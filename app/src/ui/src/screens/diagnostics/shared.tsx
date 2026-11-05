// What the Diagnostics tabs share: the window-wide view state (masking, the
// losses' Reset baseline, the core's clock), the link samples of the radio,
// stream and capture tabs, their charts' common time axis, and the losses
// card.

import { createMemo, createSignal, onCleanup, Show, type JSX } from "solid-js";
import { useApp } from "../../app/context";
import { ChartGroup } from "../../components/charts";
import { Button } from "../../components/controls";
import { t } from "../../i18n";
import { LINK_KEEP } from "../../ipc/store";
import { clock } from "../../lib/format";
import { lossStats, takeBaseline, type LinkBaseline, type LossStats } from "../../lib/linkstats";
import { coreTimeOfDay, nextEpoch, timeOfDay } from "../../lib/logview";

// ---- Window-wide view state (kept while the window lives, so it is still
// there when the user comes back to the screen) ----------------------------

/** Mask Bluetooth addresses in the log and in every copy */
export const [mask, setMask] = createSignal(true);
/** The link counters when Reset was pressed */
export const [resetBase, setResetBase] = createSignal<LinkBaseline | null>(null);

/** The core's local time of day at its time 0 (Snapshot.clock_ms) */
export const [coreClock, setCoreClock] = createSignal<number | null>(null);
/** Without it: the wall clock (ms since 1970) at the core's time 0, estimated from live samples */
export const [epoch, setEpoch] = createSignal<number | null>(null);
let lastCoreTime = -1;

export function observeCoreTime(timeMs: number) {
  // The core's clock went back: a new core process
  if (timeMs + 1000 < lastCoreTime) setEpoch(null);
  lastCoreTime = timeMs;
  setEpoch((e) => nextEpoch(e, timeMs, Date.now()));
}

/** A core time as the time of day (v1): from the core's clock, else from
 *  the estimate, else as the core's uptime until the clock is known */
export function timeText(ms: number): string {
  const c = coreClock();
  if (c !== null) return coreTimeOfDay(ms, c);
  const e = epoch();
  return e === null ? clock(ms) : timeOfDay(ms, e);
}

// ---- Link samples ----------------------------------------------------------

export const DASH = "–";

/** The time the charts show: every sample the store keeps (4 a second) */
export const CHART_SPAN_MS = LINK_KEEP * 250;

/** The link samples and the stream, while the calling tab is open */
export function useLink() {
  const { core } = useApp();
  onCleanup(core.wantLink());
  const samples = () => core.link();
  const times = createMemo(() => samples().map((s) => s.time_ms));
  const stream = () => core.state.snap!.stream;
  const streaming = () => stream().state === "streaming";
  const last = () => samples()[samples().length - 1];
  /** The last sample while streaming */
  const live = () => (streaming() ? (last() ?? null) : null);
  /** The last sample while packets go out (the "last second" rows) */
  const sending = () => {
    const s = last();
    return streaming() && s && s.packets_per_second > 0 ? s : null;
  };
  /** LE Audio: the stream that was set up */
  const le = () => (streaming() ? (stream().info?.le ?? null) : null);
  // Capture drops in ms after the stream stopped: the last known rate
  const rate = createMemo(() => {
    const s = samples();
    for (let i = s.length - 1; i >= 0; i--) if (s[i].source_rate) return s[i].source_rate!;
    return stream().info?.source_sample_rate ?? 0;
  });
  const loss = createMemo(() => {
    const s = last();
    return s ? lossStats(s, resetBase(), rate()) : null;
  });
  return { samples, times, stream, streaming, last, live, sending, le, rate, loss };
}
export type Link = ReturnType<typeof useLink>;

/** The charts of a tab: one time axis, one hover cursor */
export function LinkCharts(props: { link: Link; children: JSX.Element }) {
  return (
    <div class="chart-grid">
      <ChartGroup end={props.link.last()?.time_ms ?? null} span={CHART_SPAN_MS}>
        {props.children}
      </ChartGroup>
    </div>
  );
}

export function NotStreaming(props: { link: Link }) {
  return (
    <Show when={!props.link.streaming()}>
      <p class="muted">{t("quality.not_streaming")}</p>
    </Show>
  );
}

export function Row(props: { label: string; value: string; class?: string; children?: JSX.Element }) {
  return (
    <div class="lq-row">
      <dt>{props.label}</dt>
      <dd class={`num ${props.class ?? ""}`}>
        <span>{props.value}</span>
        {props.children}
      </dd>
    </div>
  );
}

/** Losses since the stream started or since Reset (one baseline for every tab) */
export function LossCard(props: {
  link: Link;
  id: string;
  rows: (l: LossStats) => JSX.Element;
  /** Without the core's counters */
  fallback: JSX.Element;
  hint?: string;
}) {
  const l = props.link;
  return (
    <section class="card lq-group" aria-labelledby={props.id}>
      <div class="lq-group-head">
        <h2 id={props.id}>
          {l.loss()?.sinceReset ? t("ui.diag.loss_since_reset", timeText(resetBase()!.time_ms)) : t("ui.diag.loss_since_stream")}
        </h2>
        <Button icon="arrow_reset" kind="subtle" disabled={!l.last()?.counters} onClick={() => setResetBase(takeBaseline(l.last()))}>
          {t("quality.reset")}
        </Button>
      </div>
      <Show when={l.loss()} fallback={<dl class="lq-rows">{props.fallback}</dl>}>
        {(x) => <dl class="lq-rows">{props.rows(x())}</dl>}
      </Show>
      <Show when={props.hint}>
        <p class="hint">{props.hint}</p>
      </Show>
    </section>
  );
}
