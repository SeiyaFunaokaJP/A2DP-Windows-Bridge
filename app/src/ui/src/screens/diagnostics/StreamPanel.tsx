// Stream tab: what goes out and how long it waits. Bitrate, packets, the
// send queue (depth and the oldest packet's wait), packets dropped before
// the radio; the codec and its details, an LE Audio stream's ISO counters,
// the losses, and the backlog the Bluetooth adapter holds after the send
// queue, and the playout delay the device reports. In developer mode the
// encoder's load: its time per time of audio, and per frame.

import { createMemo, Show } from "solid-js";
import { useApp } from "../../app/context";
import { TimeChart } from "../../components/charts";
import { CodecDetailsCard } from "../../components/CodecDetails";
import { QUALITY_QUEUE_BACKLOG_MS } from "../../components/stream";
import { t } from "../../i18n";
import { bitrateNoteKey, codecLabelKey } from "../../lib/codecs";
import { kbps } from "../../lib/format";
import type { LinkSample } from "../../ipc/types";
import { dropsPerSample } from "../../lib/linkstats";
import { DASH, LinkCharts, LossCard, NotStreaming, Row, useLink } from "./shared";

const ms = (v: number) => `${Math.round(v)} ms`;
const count = (v: number) => `${Math.round(v)}`;
const pct = (v: number) => `${v < 10 ? v.toFixed(1) : Math.round(v)} %`;
const us = (v: number) => `${v < 10 ? v.toFixed(1) : Math.round(v)} µs`;

export function StreamPanel() {
  const { core } = useApp();
  const developer = () => !!core.state.snap?.settings.developer_mode;
  const link = useLink();
  const { samples, times, stream, streaming, live, sending, le, last } = link;
  const drops = createMemo(() => dropsPerSample(samples()));

  const codec = () => {
    const i = stream().info;
    if (!streaming() || !i) return DASH;
    const key = codecLabelKey(i.codec);
    const note = bitrateNoteKey(i);
    return t("quality.codec_value", key ? t(key) : i.codec, i.sample_rate / 1000, i.channels, i.bitrate_kbps) + (note ? t(note) : "");
  };
  const queueClass = () => {
    const s = live();
    if (!s) return "";
    const f = s.queue_ms != null ? s.queue_ms / (2 * QUALITY_QUEUE_BACKLOG_MS) : s.queue_depth / (s.queue_capacity || 64);
    return f < 0.25 ? "st-good" : f < 0.5 ? "st-warn" : "st-bad";
  };
  const queueText = () => {
    const s = live();
    if (!s) return DASH;
    return s.queue_capacity ? t("quality.queue_value", s.queue_depth, s.queue_capacity) : String(s.queue_depth);
  };
  const waitText = () => {
    const s = live();
    return s && s.queue_ms != null ? ms(s.queue_ms) : DASH;
  };
  /** The adapter's backlog: ms of audio, and the packets it held */
  const backlogText = (s: LinkSample | null | undefined) => {
    if (!s || s.controller_backlog == null) return DASH;
    return s.controller_backlog_ms != null
      ? t("ui.diag.backlog_value", ms(s.controller_backlog_ms), s.controller_backlog)
      : t("ui.diag.backlog_packets", s.controller_backlog);
  };
  /** The playout delay the device reported (not every device does) */
  const sinkDelayText = () => {
    const s = live();
    return !s || s.sink_delay_ms == null ? DASH : t("quality.sink_delay_value", s.sink_delay_ms);
  };
  /** The encoder's time per time of audio, and per frame */
  const encodeText = () => {
    const e = live()?.encode;
    return e ? t("ui.diag.encode_load_value", pct(e.load * 100), us(e.avg_us), us(e.max_us)) : DASH;
  };
  const packetsText = () => {
    const s = sending();
    if (!s) return DASH;
    return s.avg_packet_bytes != null
      ? t("quality.packets_value", s.packets_per_second, s.avg_packet_bytes)
      : `${s.packets_per_second.toFixed(1)} /s`;
  };
  /** The encoder's bitrate: the line the actual bitrate should follow */
  const target = () => {
    const i = stream().info;
    return streaming() && i && i.bitrate_kbps > 0 ? [{ value: i.bitrate_kbps, kind: "target" as const }] : [];
  };
  const iso = () => last()?.iso ?? null;
  const isoCount = (pick: (i: NonNullable<ReturnType<typeof iso>>) => number) => {
    const i = iso();
    return i ? String(pick(i)) : DASH;
  };

  return (
    <div>
      <NotStreaming link={link} />
      <LinkCharts link={link}>
        <TimeChart
          title={t("quality.bitrate")}
          times={times()}
          series={[{ values: samples().map((s) => kbps(s.bytes_per_second)) }]}
          min={0}
          refs={target()}
          format={(v) => `${Math.round(v)} kbps`}
        />
        <TimeChart
          title={t("quality.packets")}
          times={times()}
          series={[{ values: samples().map((s) => s.packets_per_second) }]}
          min={0}
          format={(v) => `${Math.round(v)} /s`}
        />
        <TimeChart
          title={t("quality.queue")}
          times={times()}
          series={[{ values: samples().map((s) => s.queue_depth) }]}
          min={0}
          format={count}
          status={(v) => (v < 8 ? "good" : v < 16 ? "warn" : "bad")}
        />
        <TimeChart
          title={t("ui.diag.queue_wait")}
          times={times()}
          series={[{ values: samples().map((s) => s.queue_ms ?? null) }]}
          min={0}
          refs={[{ value: QUALITY_QUEUE_BACKLOG_MS, kind: "limit" }]}
          format={ms}
          status={(v) => (v < QUALITY_QUEUE_BACKLOG_MS / 2 ? "good" : v < QUALITY_QUEUE_BACKLOG_MS ? "warn" : "bad")}
        />
        <TimeChart
          title={t("ui.diag.backlog")}
          times={times()}
          series={[{ values: samples().map((s) => s.controller_backlog_ms ?? null) }]}
          min={0}
          format={ms}
          formatValue={(_, i) => backlogText(samples()[i])}
        />
        <TimeChart
          title={t("quality.dropped")}
          times={times()}
          series={[{ values: drops() }]}
          min={0}
          format={count}
          status={(v) => (v === 0 ? "good" : v < 3 ? "warn" : "bad")}
        />
        <Show when={developer()}>
          <TimeChart
            title={t("ui.diag.encode_load")}
            times={times()}
            series={[{ values: samples().map((s) => (s.encode ? s.encode.load * 100 : null)) }]}
            min={0}
            format={pct}
          />
        </Show>
      </LinkCharts>

      <div class="lq-groups">
        <section class="card lq-group" aria-labelledby="lq-send">
          <dl class="lq-rows" aria-label={t("quality.section_stream")}>
            <Row label={t("quality.codec")} value={codec()} />
          </dl>
          <h2 id="lq-send">{t("quality.section_send")}</h2>
          <dl class="lq-rows" aria-labelledby="lq-send">
            <Row label={t("quality.bitrate")} value={sending() ? t("quality.bitrate_value", kbps(sending()!.bytes_per_second)) : DASH} />
            <Row label={t("quality.packets")} value={packetsText()} />
            <Row label={t("quality.queue")} class={queueClass()} value={queueText()} />
            <Row label={t("ui.diag.queue_wait")} class={queueClass()} value={waitText()} />
            <Row label={t("ui.diag.backlog")} value={backlogText(live())} />
            <Row label={t("quality.sink_delay")} value={sinkDelayText()} />
            <Show when={developer()}>
              <Row label={t("ui.diag.encode_load")} value={encodeText()} />
            </Show>
          </dl>
        </section>

        {/* What the A2DP codec runs with and what the device offered */}
        <Show when={streaming() && !le() && stream().info}>
          {(i) => <Show when={i().details}>{(d) => <CodecDetailsCard info={i()} details={d()} />}</Show>}
        </Show>

        <Show when={le() || iso()}>
          <section class="card lq-group" aria-labelledby="lq-iso">
            <h2 id="lq-iso">{t("ui.diag.iso_title")}</h2>
            <dl class="lq-rows">
              <Row label={t("ui.diag.iso_config")} value={le()?.summary ?? DASH} />
              <Row label={t("ui.diag.iso_cis")} value={isoCount((i) => i.cis)} />
              <Row label={t("ui.diag.iso_sdu_sets")} value={isoCount((i) => i.sdu_sets)} />
              <Row label={t("ui.diag.iso_silence")} class={(iso()?.silence ?? 0) > 0 ? "st-warn" : ""} value={isoCount((i) => i.silence)} />
              <Row
                label={t("ui.diag.iso_underruns")}
                class={(iso()?.underruns ?? 0) > 0 ? "st-warn" : ""}
                value={isoCount((i) => i.underruns)}
              />
              <Row label={t("ui.diag.iso_dropped")} class={(iso()?.dropped ?? 0) > 0 ? "st-bad" : ""} value={isoCount((i) => i.dropped)} />
              <Row label={t("ui.diag.iso_prefill")} value={iso() ? t("ui.diag.iso_prefill_value", iso()!.prefill) : DASH} />
            </dl>
            <p class="hint">{t("ui.diag.iso_help")}</p>
          </section>
        </Show>

        <LossCard
          link={link}
          id="lq-loss"
          hint={t("quality.loss_help")}
          fallback={<Row label={t("quality.dropped")} value={last() ? String(last()!.dropped) : DASH} />}
          rows={(l) => (
            <>
              <Row label={t("quality.sent_total")} value={String(l.sent)} />
              <Row label={t("quality.dropped")} class={l.dropped > 0 ? "st-bad" : ""} value={`${l.dropped} (${l.dropPct.toFixed(2)}%)`} />
            </>
          )}
        />
      </div>
    </div>
  );
}
