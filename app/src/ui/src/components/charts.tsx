// Small SVG charts for the link samples. No chart library: a polyline over
// a fixed number of samples is all these need, and SVG scales with the DPI.

import { createMemo, Show } from "solid-js";

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

/** A labelled live chart (Diagnostics) */
export function LineChart(props: {
  title: string;
  values: readonly (number | null)[];
  min?: number;
  max?: number;
  format: (v: number) => string;
  /** The latest value in the header, when it says more than the scale */
  formatValue?: (v: number) => string;
  /** The status colour of the latest value */
  status?: (v: number) => "good" | "warn" | "bad";
}) {
  const range = createMemo(() => bounds(props.values, props.min, props.max));
  const runs = createMemo(() => points(props.values, range()[0], range()[1], 300, 96));
  const last = createMemo(() => {
    for (let i = props.values.length - 1; i >= 0; i--) {
      const v = props.values[i];
      if (v !== null && Number.isFinite(v)) return v;
    }
    return null;
  });
  const lastText = () => (last() === null ? "–" : (props.formatValue ?? props.format)(last()!));
  return (
    <figure class="chart card">
      <figcaption class="chart-head">
        <span class="chart-title">{props.title}</span>
        <span class={`chart-value num ${last() !== null && props.status ? `st-${props.status(last()!)}` : ""}`}>
          {lastText()}
        </span>
      </figcaption>
      <svg
        class="chart-plot"
        viewBox="0 0 300 100"
        preserveAspectRatio="none"
        role="img"
        aria-label={`${props.title}: ${lastText()}`}
      >
        <line class="grid" x1="0" x2="300" y1="2" y2="2" vector-effect="non-scaling-stroke" />
        <line class="grid" x1="0" x2="300" y1="50" y2="50" vector-effect="non-scaling-stroke" />
        <line class="grid" x1="0" x2="300" y1="98" y2="98" vector-effect="non-scaling-stroke" />
        {runs().map((p) => (
          <polyline points={p} transform="translate(0 2)" vector-effect="non-scaling-stroke" />
        ))}
      </svg>
      <Show when={props.values.length > 0}>
        <div class="chart-scale num" aria-hidden="true">
          <span>{props.format(range()[1])}</span>
          <span>{props.format(range()[0])}</span>
        </div>
      </Show>
    </figure>
  );
}
