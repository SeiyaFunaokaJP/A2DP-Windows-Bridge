// Audio capture tab: what Windows hands over. The capture rate against the
// source's rate, audio dropped at capture because the encoder fell behind
// (the PC busy, not the radio), where the audio comes from. Further capture
// counters (e.g. gaps Windows reports in the captured audio) belong here as
// a chart and a row.

import { createMemo, Show } from "solid-js";
import { useApp } from "../../app/context";
import { TimeChart } from "../../components/charts";
import { currentProfile, sourceText } from "../../components/stream";
import { t } from "../../i18n";
import { captureDropsPerSample, ratePerSecond } from "../../lib/series";
import { DASH, LinkCharts, LossCard, NotStreaming, Row, useLink } from "./shared";

const khz = (v: number) => `${(v / 1000).toFixed(1)} kHz`;
const ms = (v: number) => `${v < 10 && v > 0 ? v.toFixed(1) : Math.round(v)} ms`;

export function CapturePanel() {
  const { core } = useApp();
  const link = useLink();
  const { samples, times, stream, streaming, rate, last } = link;

  const captureRate = createMemo(() => ratePerSecond(samples(), "capture_frames"));
  const drops = createMemo(() => captureDropsPerSample(samples(), rate()));
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
          series={[{ values: drops() }]}
          min={0}
          format={ms}
          status={(v) => (v === 0 ? "good" : v < 20 ? "warn" : "bad")}
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
          fallback={<Row label={t("quality.capture_dropped")} value={last() ? String(last()!.capture_dropped) : DASH} />}
          rows={(l) => (
            <Row
              label={t("quality.capture_dropped")}
              class={l.captureDropped > 0 ? "st-bad" : ""}
              value={`${l.captureMs === null ? String(l.captureDropped) : `${l.captureMs.toFixed(0)} ms`} (${l.capturePct.toFixed(2)}%)`}
            />
          )}
        />
      </div>
    </div>
  );
}
