// Radio tab: the BR/EDR link. The link quality and what lowered it, RSSI,
// channel hopping, the share of the radio's time the stream takes. More
// controller figures (its backlog, failed contacts, its own link quality)
// belong in the charts and the link card; what the device and the link are
// (role, sniff, the remote chip, key size, supervision timeout) in a device
// card next to it.

import { createMemo, Show } from "solid-js";
import { Gauge, TimeChart } from "../../components/charts";
import { linkQuality, QUALITY_AIRTIME_MAX, qualityLabel, type QualityCause } from "../../components/stream";
import { t } from "../../i18n";
import { EDR3_MAX_KBPS, gaugeValue } from "../../lib/linkstats";
import { DASH, LinkCharts, NotStreaming, Row, useLink } from "./shared";

/** The highest the golden receive range's lower edge may sit (Core spec):
 *  a reading below the range is at most this plus the reading */
const GOLDEN_LOW_MAX_DBM = -56;

/** BR/EDR hops over 79 channels */
const BR_CHANNELS = 79;

function rssiDb(v: number): string {
  return `${v > 0 ? "+" : ""}${Math.round(v)} dB`;
}

/** The reading, and below the range the most the actual level can be */
function rssiValue(v: number): string {
  return v < 0 ? t("ui.diag.rssi_value", rssiDb(v), Math.round(v) + GOLDEN_LOW_MAX_DBM) : rssiDb(v);
}

const pct = (v: number) => `${Math.round(v)} %`;

const CAUSE_KEY: Record<QualityCause, string> = {
  drops: "quality.dropped",
  queue: "ui.diag.cause_queue",
  airtime: "ui.diag.cause_airtime",
};

export function RadioPanel() {
  const link = useLink();
  const { samples, times, streaming, live, sending, le } = link;

  const quality = createMemo(() => (streaming() ? linkQuality(samples()) : null));
  const causes = () => {
    const q = quality();
    if (!q) return "";
    const list = q.causes.map((c) => t(CAUSE_KEY[c]));
    if (q.pcBusy) list.push(t("quality.capture_dropped"));
    return list.join(t("radio.separator"));
  };

  const rssiText = () => {
    const s = live();
    return !s || s.rssi === null ? DASH : rssiValue(s.rssi);
  };
  const afhText = () => {
    const s = live();
    if (!s) return DASH;
    const parts: string[] = [];
    if (s.afh_in_use !== null) parts.push(t("radio.afh", s.afh_in_use));
    if (s.afh_usable !== null) parts.push(t("ui.diag.afh_usable", s.afh_usable));
    if (parts.length === 0) return DASH;
    return parts.join(t("radio.separator")) + (s.afh_usable !== null && s.afh_avoided ? t("radio.afh_avoid", s.afh_avoided) : "");
  };
  const theoryPct = () => {
    const s = sending();
    return s && s.bandwidth_share != null ? s.bandwidth_share * 100 : null;
  };
  const airPct = () => {
    const s = sending();
    return s ? s.airtime * 100 : null;
  };
  const airHigh = () => (airPct() ?? 0) >= QUALITY_AIRTIME_MAX * 100;
  const airText = () => {
    const s = sending();
    const p = airPct();
    if (!s || p === null) return DASH;
    return s.acl_packet ? t("quality.airtime_value", p, s.acl_packet.name, s.acl_packet.slots) : `${p.toFixed(1)} %`;
  };
  // An LE Audio stream has none of these figures; its counters are on the Stream tab
  const brEdr = () => !le();

  return (
    <div>
      <NotStreaming link={link} />
      <Show when={brEdr()}>
        <LinkCharts link={link}>
          <TimeChart
            title={t("ui.diag.rssi")}
            times={times()}
            series={[{ values: samples().map((s) => s.rssi) }]}
            min={-40}
            max={10}
            refs={[{ value: 0, kind: "target" }]}
            format={rssiDb}
            formatValue={rssiValue}
          />
          <TimeChart
            title={t("ui.diag.afh")}
            times={times()}
            series={[
              { label: t("ui.diag.afh_in_use"), values: samples().map((s) => s.afh_in_use) },
              { label: t("ui.diag.afh_usable_short"), values: samples().map((s) => s.afh_usable) },
            ]}
            min={0}
            max={BR_CHANNELS}
            format={(v) => `${Math.round(v)}`}
          />
          <TimeChart
            title={t("quality.section_band")}
            times={times()}
            series={[
              { label: t("quality.airtime"), values: samples().map((s) => (s.packets_per_second > 0 ? s.airtime * 100 : null)) },
              {
                label: t("quality.theory"),
                values: samples().map((s) => (s.packets_per_second > 0 && s.bandwidth_share != null ? s.bandwidth_share * 100 : null)),
              },
            ]}
            min={0}
            max={100}
            refs={[{ value: QUALITY_AIRTIME_MAX * 100, kind: "limit" }]}
            format={pct}
            formatValue={(v) => `${v.toFixed(1)} %`}
          />
        </LinkCharts>
      </Show>

      <div class="lq-groups">
        <section class="card lq-group" aria-labelledby="lq-link">
          <h2 id="lq-link">{t("quality.title")}</h2>
          <p class={`lq-verdict ${quality() ? `st-${quality()!.status}` : ""}`}>
            <span class="lq-verdict-label">{qualityLabel(quality())}</span>
            <Show when={causes()}>
              <span class="lq-verdict-causes">{causes()}</span>
            </Show>
          </p>
          <Show when={brEdr()}>
            <dl class="lq-rows" aria-labelledby="lq-link">
              <Row label={t("ui.diag.rssi")} value={rssiText()} />
              <Row label={t("ui.diag.afh")} value={afhText()} />
            </dl>
          </Show>
        </section>

        {/* The BR/EDR link's band: not for LE Audio */}
        <Show when={brEdr()}>
          <section class="card lq-group" aria-labelledby="lq-band">
            <h2 id="lq-band">{t("quality.section_band")}</h2>
            <dl class="lq-rows">
              <Row label={t("quality.theory")} value={theoryPct() === null ? DASH : t("quality.theory_value", EDR3_MAX_KBPS, theoryPct()!)}>
                <Gauge value={theoryPct() === null ? null : gaugeValue(theoryPct())} label={t("quality.theory")} />
              </Row>
              <Row label={t("quality.airtime")} class={airHigh() ? "st-bad" : ""} value={airText()}>
                <Gauge
                  value={airPct() === null ? null : gaugeValue(airPct())}
                  label={t("quality.airtime")}
                  status={airHigh() ? "bad" : undefined}
                />
              </Row>
            </dl>
            <p class="hint">{t("quality.band_help")}</p>
          </section>
        </Show>
      </div>
    </div>
  );
}
