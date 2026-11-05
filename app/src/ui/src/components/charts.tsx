// Small SVG charts for the link samples. No chart library: polylines over
// the last minute of samples are all these need, and SVG scales with the DPI.
// The Diagnostics charts of one tab share a time axis and a hover cursor
// (ChartGroup).

import { createContext, createMemo, createSignal, For, Show, useContext, type JSX } from "solid-js";
import { lastKnownIndex, nearestIndex, scaleOf, timeRuns } from "../lib/series";

function points(values: readonly (number | null)[], min: number, max: number, width: number, height: number): string[] {
  // One polyline per run of known values (gaps where the value is unknown)
  const runs: string[] = [];
  let cur: string[] = [];
  const span = max - min || 1;
  const n = Math.max(values.length - 1, 1);
  values.forEach((v, i) => {
    if (v === null || !Number.isFinite(v)) {
      if (cur.length) runs.push(cur.join(" "));
      cur = [];
      return;
    }
    const x = (i / n) * width;
    const y = height - ((Math.min(Math.max(v, min), max) - min) / span) * height;
    cur.push(`${x.toFixed(2)},${y.toFixed(2)}`);
  });
  if (cur.length) runs.push(cur.join(" "));
  return runs;
}

function bounds(values: readonly (number | null)[], min?: number, max?: number): [number, number] {
  const known = values.filter((v): v is number => v !== null && Number.isFinite(v));
  const lo = min ?? (known.length ? Math.min(...known) : 0);
  let hi = max ?? (known.length ? Math.max(...known) : 1);
  if (hi <= lo) hi = lo + 1;
  return [lo, hi];
}

/** A tiny trend line (Home, flyout) */
export function Sparkline(props: { values: readonly (number | null)[]; min?: number; max?: number; label: string }) {
  const runs = createMemo(() => {
    const [lo, hi] = bounds(props.values, props.min, props.max);
    return points(props.values, lo, hi, 100, 28);
  });
  return (
    <svg class="sparkline" viewBox="0 0 100 30" preserveAspectRatio="none" role="img" aria-label={props.label}>
      {runs().map((p) => (
        <polyline points={p} transform="translate(0 1)" vector-effect="non-scaling-stroke" />
      ))}
    </svg>
  );
}

/** A horizontal bar for a share of 0..100 % (Diagnostics' bandwidth rows) */
export function Gauge(props: { value: number | null; label: string; status?: "good" | "warn" | "bad" }) {
  const v = () => (props.value === null || !Number.isFinite(props.value) ? 0 : Math.min(100, Math.max(0, props.value)));
  return (
    <div
      class={`gauge ${props.status ? `st-${props.status}` : ""}`}
      role="meter"
      aria-label={props.label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(v())}
      aria-valuetext={props.value === null ? "–" : `${v().toFixed(1)} %`}
    >
      <span class="gauge-fill" style={{ width: `${v()}%` }} />
    </div>
  );
}

// ---- Time charts (Diagnostics) ---------------------------------------------

interface Group {
  /** The axis: [start, end] in core time (ms) */
  range: () => [number, number];
  /** The hovered core time, in every chart of the group */
  hover: () => number | null;
  setHover: (t: number | null) => void;
}
const GroupContext = createContext<Group>();

/** Charts with one time axis ending at `end` (the latest sample) and one hover cursor */
export function ChartGroup(props: { end: number | null; span: number; children: JSX.Element }) {
  const [hover, setHover] = createSignal<number | null>(null);
  const range = (): [number, number] => {
    const end = props.end ?? props.span;
    return [end - props.span, end];
  };
  return <GroupContext.Provider value={{ range, hover, setHover }}>{props.children}</GroupContext.Provider>;
}

export interface ChartSeries {
  values: readonly (number | null)[];
  /** The legend's name (charts with more than one series) */
  label?: string;
}

/** A horizontal line: a limit (warning colour) or a target the line should follow */
export interface ChartRef {
  value: number;
  kind: "limit" | "target";
}

const W = 300;
const H = 96;

/** A labelled live chart over time: the latest value (or the hovered one) in the header */
export function TimeChart(props: {
  title: string;
  /** LinkSample.time_ms of each value */
  times: readonly number[];
  series: readonly ChartSeries[];
  min?: number;
  max?: number;
  /** The scale's labels */
  format: (v: number) => string;
  /** The value in the header and legend, when it says more than the scale
   *  (`i`: the sample shown, hovered or the latest known) */
  formatValue?: (v: number, i: number) => string;
  /** The status colour of the first series' value */
  status?: (v: number) => "good" | "warn" | "bad";
  refs?: readonly ChartRef[];
}) {
  const group = useContext(GroupContext);
  const [ownHover, setOwnHover] = createSignal<number | null>(null);
  const hover = () => (group ? group.hover() : ownHover());
  const setHover = (t: number | null) => (group ? group.setHover(t) : setOwnHover(t));
  const range = (): [number, number] => {
    if (group) return group.range();
    const ts = props.times;
    return ts.length ? [ts[0], Math.max(ts[ts.length - 1], ts[0] + 1)] : [0, 1];
  };
  const span = () => {
    const [a, b] = range();
    return b - a || 1;
  };
  const scale = createMemo(() =>
    scaleOf([...props.series.map((s) => s.values), (props.refs ?? []).map((r) => r.value)], props.min, props.max),
  );
  const runs = createMemo(() => {
    const [lo, hi] = scale();
    const [start] = range();
    return props.series.map((s) => timeRuns(props.times, s.values, start, span(), lo, hi, W, H));
  });
  const yOf = (v: number) => {
    const [lo, hi] = scale();
    return H - ((Math.min(Math.max(v, lo), hi) - lo) / (hi - lo)) * H;
  };
  /** The sample under the cursor */
  const at = createMemo(() => {
    const h = hover();
    return h === null ? null : nearestIndex(props.times, h);
  });
  /** The sample of a series shown: the hovered one, else its latest known */
  const shown = (s: ChartSeries, hovered = true): number | null => {
    const i = hovered ? at() : null;
    if (i === null) return lastKnownIndex(s.values);
    const v = s.values[i];
    return v !== null && v !== undefined && Number.isFinite(v) ? i : null;
  };
  const text = (s: ChartSeries | undefined, hovered = true) => {
    const i = s ? shown(s, hovered) : null;
    if (s === undefined || i === null) return "–";
    const v = s.values[i]!;
    return props.formatValue ? props.formatValue(v, i) : props.format(v);
  };
  const first = () => {
    const s = props.series[0];
    const i = s ? shown(s) : null;
    return s && i !== null ? s.values[i] : null;
  };
  const multi = () => props.series.length > 1;
  const label = () =>
    multi() ? props.series.map((s) => `${s.label}: ${text(s, false)}`).join(", ") : text(props.series[0], false);
  const cursorX = () => {
    const i = at();
    return i === null ? null : ((props.times[i] - range()[0]) / span()) * W;
  };
  const onMove = (e: PointerEvent) => {
    const r = (e.currentTarget as Element).getBoundingClientRect();
    if (r.width <= 0) return;
    const f = Math.min(1, Math.max(0, (e.clientX - r.left) / r.width));
    setHover(range()[0] + f * span());
  };
  return (
    <figure class="chart card" classList={{ multi: multi() }}>
      <figcaption class="chart-head">
        <span class="chart-title">{props.title}</span>
        <span class="chart-readout">
          <Show when={!multi()}>
            <span class={`chart-value num ${first() !== null && props.status ? `st-${props.status(first()!)}` : ""}`}>
              {text(props.series[0])}
            </span>
          </Show>
        </span>
      </figcaption>
      <Show when={multi()}>
        <ul class="chart-legend">
          <For each={props.series}>
            {(s, i) => (
              <li class={`s${i()}`}>
                <span class="chart-swatch" aria-hidden="true" />
                <span>{s.label}</span>
                <span class="chart-value num">{text(s)}</span>
              </li>
            )}
          </For>
        </ul>
      </Show>
      <svg
        class="chart-plot"
        viewBox={`0 0 ${W} ${H + 4}`}
        preserveAspectRatio="none"
        role="img"
        aria-label={`${props.title}: ${label()}`}
        onPointerMove={onMove}
        onPointerDown={onMove}
        onPointerLeave={() => setHover(null)}
      >
        <g transform="translate(0 2)">
          <line class="grid" x1="0" x2={W} y1="0" y2="0" vector-effect="non-scaling-stroke" />
          <line class="grid" x1="0" x2={W} y1={H / 2} y2={H / 2} vector-effect="non-scaling-stroke" />
          <line class="grid" x1="0" x2={W} y1={H} y2={H} vector-effect="non-scaling-stroke" />
          <For each={props.refs ?? []}>
            {(r) => (
              <line
                class={`ref ref-${r.kind}`}
                x1="0"
                x2={W}
                y1={yOf(r.value)}
                y2={yOf(r.value)}
                vector-effect="non-scaling-stroke"
              />
            )}
          </For>
          <For each={runs()}>
            {(rs, i) => <For each={rs}>{(p) => <polyline class={`s${i()}`} points={p} vector-effect="non-scaling-stroke" />}</For>}
          </For>
          <Show when={cursorX() !== null}>
            <line class="cursor" x1={cursorX()!} x2={cursorX()!} y1="0" y2={H} vector-effect="non-scaling-stroke" />
          </Show>
        </g>
      </svg>
      <div class="chart-scale num" aria-hidden="true">
        <span>{props.format(scale()[1])}</span>
        <span>{props.format(scale()[0])}</span>
      </div>
      <div class="chart-axis num" aria-hidden="true">
        <span>−{Math.round(span() / 1000)} s</span>
        <span>0 s</span>
      </div>
    </figure>
  );
}
