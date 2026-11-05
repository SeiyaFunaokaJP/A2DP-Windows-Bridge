// Radio tab: the BR/EDR link. The link quality and what lowered it, RSSI,
// channel hopping, the share of the radio's time the stream takes. In
// developer mode the controller's own figures too (its link quality, failed
// contacts) in the charts and the link card. The device card: the device's
// Bluetooth chip for everyone; the link's role, mode, key size and
// supervision timeout in developer mode. (The adapter's backlog is on the
// Stream tab, next to the send queue it follows.) Below them the channel
// map (ChannelMap).

import { createMemo, Show } from "solid-js";
import { useApp } from "../../app/context";
import { Gauge, TimeChart } from "../../components/charts";
import { linkQuality, QUALITY_AIRTIME_MAX, qualityLabel, type QualityCause } from "../../components/stream";
import { t } from "../../i18n";
import { EDR3_MAX_KBPS, gaugeValue } from "../../lib/linkstats";
import type { LinkSample } from "../../ipc/types";
import { ChannelMap } from "./ChannelMap";
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

type RemoteChip = NonNullable<LinkSample["remote_chip"]>;
type LinkDetails = NonNullable<LinkSample["link_details"]>;

/** The chip's fastest packet type and its rate ("3-DH5 (3 Mbps)") */
export function chipPacketText(c: RemoteChip | null | undefined): string {
  const f = c?.features;
  if (!f) return DASH;
  return t("ui.diag.chip_packet_value", f.largest_packet, f.edr_3m ? 3 : f.edr_2m ? 2 : 1);
}

const seconds = (ms: number) => `${Number((ms / 1000).toFixed(2))} s`;

/** Role, with the changes since the stream started */
function roleText(d: LinkDetails): string {
  if (!d.role) return DASH;
  const role = t(d.role === "central" ? "ui.diag.role_central" : "ui.diag.role_peripheral");
  return d.role_changes > 0 ? t("ui.diag.changes_value", role, d.role_changes) : role;
}

const MODE_KEY = {
  active: "ui.diag.mode_active",
  hold: "ui.diag.mode_hold",
  sniff: "ui.diag.mode_sniff",
  park: "ui.diag.mode_park",
} as const;

/** Mode (sniff with its interval), with the sniff entries since the stream started */
function modeText(d: LinkDetails): string {
  if (!d.mode) return DASH;
  const mode =
    d.mode === "sniff" && d.sniff_interval_ms != null
      ? t("ui.diag.mode_sniff_value", Math.round(d.sniff_interval_ms))
      : t(MODE_KEY[d.mode]);
  return d.sniff_entries > 0 ? t("ui.diag.sniff_entries_value", mode, d.sniff_entries) : mode;
}

/** Failed contacts since the stream started, and their rate */
function failedText(d: LinkDetails): string {
  const n = d.failed_contacts_stream;
  if (n == null) return DASH;
  const rate = d.failed_contacts_per_second;
  return n > 0 && rate != null && rate > 0 ? `${n} (${rate.toFixed(1)} /s)` : String(n);
}

const CAUSE_KEY: Record<QualityCause, string> = {
  drops: "quality.dropped",
  queue: "ui.diag.cause_queue",
  airtime: "ui.diag.cause_airtime",
};

export function RadioPanel() {
  const { core } = useApp();
  const link = useLink();
  const { samples, times, streaming, live, sending, le } = link;
  const developer = () => core.state.snap!.settings.developer_mode;
  const chip = () => live()?.remote_chip ?? null;
  const details = () => live()?.link_details ?? null;
  const detail = (f: (d: LinkDetails) => string) => {
    const d = details();
    return d ? f(d) : DASH;
  };

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
          <Show when={developer()}>
            <TimeChart
              title={t("ui.diag.hci_link_quality")}
              times={times()}
              series={[{ values: samples().map((s) => s.link_details?.link_quality ?? null) }]}
              min={0}
              max={255}
              format={(v) => `${Math.round(v)}`}
            />
          </Show>
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
              <Show when={developer()}>
                <Row
                  label={t("ui.diag.hci_link_quality")}
                  value={detail((d) => (d.link_quality == null ? DASH : t("ui.diag.of_255", d.link_quality)))}
                />
                <Row
                  label={t("ui.diag.failed_contacts")}
                  class={(details()?.failed_contacts_stream ?? 0) > 0 ? "st-warn" : ""}
                  value={detail(failedText)}
                />
              </Show>
            </dl>
          </Show>
        </section>

        {/* The device's chip (everyone), the link's state (developer mode) */}
        <Show when={brEdr()}>
          <section class="card lq-group" aria-labelledby="lq-device">
            <h2 id="lq-device">{t("ui.home.device")}</h2>
            <dl class="lq-rows" aria-labelledby="lq-device">
              <Row label={t("ui.diag.chip_maker")} value={chip()?.company ?? DASH} />
              <Row label={t("ui.diag.chip_version")} value={chip()?.version ?? DASH} />
              <Row label={t("ui.diag.chip_packet")} value={chipPacketText(chip())} />
              <Show when={developer()}>
                <Row label={t("ui.diag.role")} value={detail(roleText)} />
                <Row label={t("ui.diag.mode")} value={detail(modeText)} />
                <Row
                  label={t("ui.diag.key_size")}
                  value={detail((d) => (d.encryption_key_size == null ? DASH : t("ui.diag.bytes_value", d.encryption_key_size)))}
                />
                <Row
                  label={t("ui.diag.supervision_timeout")}
                  value={detail((d) => (d.supervision_timeout_ms == null ? DASH : seconds(d.supervision_timeout_ms)))}
                />
              </Show>
            </dl>
          </section>
        </Show>

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
      <ChannelMap />
    </div>
  );
}
