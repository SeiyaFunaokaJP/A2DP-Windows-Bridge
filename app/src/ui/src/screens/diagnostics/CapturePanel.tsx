// Audio capture tab: what Windows hands over. The capture rate against the
// source's rate, audio lost at capture: dropped because the encoder fell
// behind (the PC busy, not the radio) and lost in gaps Windows reports in the
// captured audio (its own glitches); where the audio comes from. Per-app
// capture counts its gaps without the audio lost in them: the count alone.

import { createMemo, Show } from "solid-js";
import { useApp } from "../../app/context";
import { TimeChart } from "../../components/charts";
import { currentProfile, sourceText } from "../../components/stream";
import { t } from "../../i18n";
import type { LossStats } from "../../lib/linkstats";
import { captureDropsPerSample, captureLostPerSample, ratePerSecond } from "../../lib/series";
import { DASH, LinkCharts, LossCard, NotStreaming, Row, useLink } from "./shared";

const khz = (v: number) => `${(v / 1000).toFixed(1)} kHz`;
const ms = (v: number) => `${v < 10 && v > 0 ? v.toFixed(1) : Math.round(v)} ms`;

/** Windows' gaps: the count, and the audio lost in them when known */
export function gapsText(gaps: number, lostMs: number | null): string {
  return lostMs !== null && lostMs > 0 ? t("ui.diag.capture_gaps_value", gaps, ms(lostMs)) : String(gaps);
}

export function CapturePanel() {
  const { core } = useApp();
  const link = useLink();
  const { samples, times, stream, streaming, rate, last } = link;

  const captureRate = createMemo(() => ratePerSecond(samples(), "capture_frames"));
  const drops = createMemo(() => captureDropsPerSample(samples(), rate()));
  const lost = createMemo(() => captureLostPerSample(samples(), rate()));
  /** Without the core's counters: the sample's own, since the stream started */
  const gapsFallback = () => {
    const s = last();
    if (!s) return DASH;
    const r = s.source_rate || rate();
    return gapsText(s.capture_discontinuities ?? 0, r > 0 ? ((s.capture_lost ?? 0) * 1000) / r : null);
  };
  const gapsClass = (l: LossStats) => (l.captureGaps > 0 ? "st-warn" : "");
  /** The rate the capture should run at (a scale of ±20 % around it) */
  const nominal = () => (streaming() ? rate() : 0);

  const source = () => {
    const i = stream().info;
    const src = streaming() ? i?.source : null;
    return src ? sourceText(src, currentProfile(core.state.snap!)).text : DASH;
  };
  const format = () => {
    const i = stream().info;
    return streaming() && i && i.source_sample_rate > 0 ? t("capture.device_format", i.source_sample_rate, i.source_bits) : "";
  };

  return (
    <div>
      <NotStreaming link={link} />
      <LinkCharts link={link}>
        <TimeChart
          title={t("ui.diag.capture_rate")}
          times={times()}
          series={[{ values: captureRate() }]}
          min={nominal() > 0 ? nominal() * 0.8 : 0}
          max={nominal() > 0 ? nominal() * 1.2 : undefined}
          refs={nominal() > 0 ? [{ value: nominal(), kind: "target" }] : []}
          format={khz}
          formatValue={(v) => `${(v / 1000).toFixed(2)} kHz`}
        />
        <TimeChart
          title={t("quality.capture_dropped")}
          times={times()}
          series={[
            { label: t("ui.quality.pc_busy"), values: drops() },
            { label: t("ui.diag.capture_gaps"), values: lost() },
          ]}
          min={0}
          format={ms}
        />
      </LinkCharts>

      <div class="lq-groups">
        <section class="card lq-group" aria-label={t("capture.mode")}>
          <dl class="lq-rows">
            <Row label={t("capture.mode")} value={source()} />
          </dl>
          <Show when={format()}>
            <p class="hint">{format()}</p>
          </Show>
        </section>

        <LossCard
          link={link}
          id="lq-capture-loss"
          fallback={
            <>
              <Row label={t("quality.capture_dropped")} value={last() ? String(last()!.capture_dropped) : DASH} />
              <Row label={t("ui.diag.capture_gaps")} value={gapsFallback()} />
            </>
          }
          rows={(l) => (
            <>
              <Row
                label={t("quality.capture_dropped")}
                class={l.captureDropped > 0 ? "st-bad" : ""}
                value={`${l.captureMs === null ? String(l.captureDropped) : `${l.captureMs.toFixed(0)} ms`} (${l.capturePct.toFixed(2)}%)`}
              />
              <Row label={t("ui.diag.capture_gaps")} class={gapsClass(l)} value={gapsText(l.captureGaps, l.captureLostMs)} />
            </>
          )}
        />
      </div>
    </div>
  );
}
