// Diagnostics' codec details card: what an A2DP stream's codec runs with,
// next to what the device offered (the SBC bitpool range, channel modes,
// AAC object types ...; for the other codecs the rates, bit depths and peak
// bit rate their endpoint lists, issue #86), and how the codec frames fill
// a packet.

import { For, type JSX, Show } from "solid-js";
import { t } from "../i18n";
import type { CodecDeviceCaps } from "../ipc/bindings/CodecDeviceCaps";
import type { CodecDetails, StreamInfo } from "../ipc/types";
import {
  AAC_CHANNELS,
  type AacDetails,
  bitpoolBar,
  type Chip,
  chips,
  frameMs,
  khz,
  liveBitpool,
  packetFill,
  SBC_ALLOCATIONS,
  SBC_BLOCKS,
  SBC_MODES,
  SBC_RATES,
  SBC_SUBBANDS,
  type SbcDetails,
} from "../lib/codecdetails";
import { Gauge } from "./charts";

function Row(props: { label: string; value: string; children?: JSX.Element }) {
  return (
    <div class="lq-row">
      <dt>{props.label}</dt>
      <dd class="num">
        <span>{props.value}</span>
        {props.children}
      </dd>
    </div>
  );
}

/** A setting as chips: in use, offered by the device, not offered */
function ChipRow<T>(props: { label: string; items: Chip<T>[]; text: (v: T) => string }) {
  const note = (c: Chip<T>) =>
    c.state === "used" ? ` (${t("ui.diag.cd_used")})` : c.state === "off" ? ` (${t("ui.diag.cd_not_offered")})` : "";
  return (
    <div class="lq-row">
      <dt>{props.label}</dt>
      <dd>
        <ul class="cd-chips">
          <For each={props.items}>
            {(c) => (
              <li class={`cd-chip cd-${c.state}`}>
                {props.text(c.value)}
                <span class="sr-only">{note(c)}</span>
              </li>
            )}
          </For>
        </ul>
      </dd>
    </div>
  );
}

/** The bitpool in use within the configured and the device's ranges */
function BitpoolBarView(props: { sbc: SbcDetails }) {
  const bar = () => bitpoolBar(props.sbc);
  const parts = () => {
    const s = props.sbc;
    const p: { cls: string; text: string }[] = [];
    if (s.device) p.push({ cls: "device", text: t("ui.diag.cd_bitpool_device", s.device.min_bitpool, s.device.max_bitpool) });
    if (s.config_bitpool) p.push({ cls: "config", text: t("ui.diag.cd_bitpool_config", s.config_bitpool[0], s.config_bitpool[1]) });
    if (s.abr_bitpool) p.push({ cls: "abr", text: t("ui.diag.cd_bitpool_abr_range", s.abr_bitpool[0], s.abr_bitpool[1]) });
    p.push({ cls: "used", text: t("ui.diag.cd_bitpool_used", s.bitpool) });
    return p;
  };
  return (
    <div class="bitpool" classList={{ live: !!props.sbc.abr_bitpool }}>
      <div class="bitpool-track" role="img" aria-label={parts().map((p) => p.text).join(", ")}>
        <Show when={bar().device}>
          {(r) => <span class="bitpool-range device" style={{ left: `${r().from}%`, width: `${r().width}%` }} />}
        </Show>
        <Show when={bar().config}>
          {(r) => <span class="bitpool-range config" style={{ left: `${r().from}%`, width: `${r().width}%` }} />}
        </Show>
        <Show when={bar().abr}>
          {(r) => <span class="bitpool-range abr" style={{ left: `${r().from}%`, width: `${r().width}%` }} />}
        </Show>
        <span class="bitpool-used" style={{ left: `${bar().used}%` }} />
      </div>
      <div class="bitpool-scale num" aria-hidden="true">
        <span>0</span>
        <span>{bar().max}</span>
      </div>
      <ul class="bitpool-legend" aria-hidden="true">
        <For each={parts()}>
          {(p) => (
            <li>
              <span class={`bitpool-swatch ${p.cls}`} />
              {p.text}
            </li>
          )}
        </For>
      </ul>
    </div>
  );
}

function SbcRows(props: { sbc: SbcDetails; info: StreamInfo }) {
  const dev = () => props.sbc.device;
  // Under ABR: "41 (ABR 19–53)", updating as it moves
  const bitpool = () => {
    const bp = liveBitpool(props.info);
    return bp ? t("ui.diag.cd_bitpool_abr", bp.now, bp.min, bp.max) : String(props.sbc.bitpool);
  };
  return (
    <>
      <Row label={t("ui.diag.cd_bitpool")} value={bitpool()}>
        <BitpoolBarView sbc={props.sbc} />
      </Row>
      <ChipRow label={t("ui.diag.cd_rate")} items={chips(SBC_RATES, dev()?.sample_rates, props.info.sample_rate)} text={khz} />
      <ChipRow
        label={t("ui.diag.cd_channel_mode")}
        items={chips(SBC_MODES, dev()?.channel_modes, props.sbc.channel_mode)}
        text={(m) => t(`ui.diag.cd_mode.${m}`)}
      />
      <ChipRow label={t("ui.diag.cd_blocks")} items={chips(SBC_BLOCKS, dev()?.blocks, props.sbc.blocks)} text={String} />
      <ChipRow label={t("ui.diag.cd_subbands")} items={chips(SBC_SUBBANDS, dev()?.subbands, props.sbc.subbands)} text={String} />
      <ChipRow
        label={t("ui.diag.cd_allocation")}
        items={chips(SBC_ALLOCATIONS, dev()?.allocations, props.sbc.allocation)}
        text={(a) => t(`ui.diag.cd_alloc.${a}`)}
      />
    </>
  );
}

function AacRows(props: { aac: AacDetails; info: StreamInfo }) {
  const dev = () => props.aac.device;
  // The device lists only some of the many AAC values: show what it offers
  const offered = <T,>(list: readonly T[] | undefined, used: T) => chips(list ?? [], list, used);
  return (
    <>
      <Row label={t("ui.diag.cd_sent")} value={t("ui.diag.cd_sent_cbr", Math.round(props.aac.bit_rate / 1000))} />
      <ChipRow
        label={t("ui.diag.cd_object_type")}
        items={offered(dev()?.object_types, props.aac.object_type)}
        text={String}
      />
      <ChipRow label={t("ui.diag.cd_rate")} items={offered(dev()?.sample_rates, props.info.sample_rate)} text={khz} />
      <ChipRow label={t("ui.diag.cd_channels")} items={chips(AAC_CHANNELS, dev()?.channels, props.info.channels)} text={String} />
      <Show when={dev()}>
        {(d) => (
          <>
            <Row
              label={t("ui.diag.cd_peak")}
              value={d().max_bitrate > 0 ? t("ui.diag.cd_peak_value", Math.round(d().max_bitrate / 1000)) : t("ui.diag.cd_peak_none")}
            />
            <Row label={t("ui.diag.cd_vbr")} value={d().vbr ? t("ui.diag.cd_vbr_yes") : t("ui.diag.cd_vbr_no")} />
          </>
        )}
      </Show>
    </>
  );
}

/** Any other codec: what its endpoint lists, as SBC and AAC show theirs */
function DeviceRows(props: { device: CodecDeviceCaps; info: StreamInfo }) {
  // The device lists only some of the values: show what it offers
  const offered = <T,>(list: readonly T[], used: T) => chips(list, list.length ? list : null, used);
  return (
    <>
      <ChipRow label={t("ui.diag.cd_rate")} items={offered(props.device.sample_rates, props.info.sample_rate)} text={khz} />
      <Show when={props.info.bits > 0}>
        <ChipRow
          label={t("connection.bit_depth")}
          items={offered(props.device.bit_depths, props.info.bits)}
          text={(b) => `${b}-bit`}
        />
      </Show>
      <Row
        label={t("ui.diag.cd_peak")}
        value={
          props.device.max_bitrate > 0
            ? t("ui.diag.cd_peak_value", Math.round(props.device.max_bitrate / 1000))
            : t("ui.diag.cd_peak_none")
        }
      />
    </>
  );
}

export function CodecDetailsCard(props: { info: StreamInfo; details: CodecDetails }) {
  const d = () => props.details;
  const headers = () => {
    const x = d();
    if (x.rtp) return x.media_header > 0 ? t("ui.diag.cd_headers_rtp", x.media_header) : t("ui.diag.cd_headers_rtp_only");
    return x.media_header > 0 ? t("ui.diag.cd_headers_plain", x.media_header) : t("ui.diag.cd_headers_none");
  };
  const frame = () => {
    const x = d();
    const ms = frameMs(x.frame_samples, props.info.sample_rate);
    return x.frame_bytes != null
      ? t("ui.diag.cd_frame_bytes", x.frame_bytes, x.frame_samples, ms)
      : t("ui.diag.cd_frame_samples", x.frame_samples, ms);
  };
  const fill = () => packetFill(d());
  const fillPct = () => {
    const f = fill();
    return f ? (f.used / f.of) * 100 : null;
  };
  // Without the device's capabilities the chips show only what is used
  const noDevice = () => (d().sbc && !d().sbc!.device) || (d().aac && !d().aac!.device);
  const chipsShown = () => !!(d().sbc?.device || d().aac?.device || d().device);
  return (
    <section class="card lq-group cd-card" aria-labelledby="lq-codec">
      <h2 id="lq-codec">
        {t("ui.diag.cd_title")} <span class="muted cd-codec">{props.info.codec}</span>
      </h2>
      <dl class="lq-rows">
        <Row label={t("ui.diag.cd_payload")} value={t("ui.diag.cd_payload_value", d().mtu)} />
        <Row label={t("ui.diag.cd_headers")} value={headers()} />
        <Row label={t("ui.diag.cd_frame")} value={frame()} />
        <Show when={fill()}>
          {(f) => (
            <Row label={t("ui.diag.cd_per_packet")} value={t("ui.diag.cd_per_packet_value", d().frames_per_packet!, f().used, f().of)}>
              <Gauge value={fillPct()} label={t("ui.diag.cd_per_packet")} />
            </Row>
          )}
        </Show>
      </dl>
      <Show when={d().sbc}>
        {(s) => (
          <>
            <h3 class="cd-sub">SBC</h3>
            <dl class="lq-rows">
              <SbcRows sbc={s()} info={props.info} />
            </dl>
            <p class="hint">{t("ui.diag.cd_bitpool_help")}</p>
            <Show when={s().abr_bitpool}>
              <p class="hint">{t("ui.diag.cd_bitpool_abr_help")}</p>
            </Show>
          </>
        )}
      </Show>
      <Show when={d().aac}>
        {(a) => (
          <>
            <h3 class="cd-sub">AAC</h3>
            <dl class="lq-rows">
              <AacRows aac={a()} info={props.info} />
            </dl>
          </>
        )}
      </Show>
      <Show when={d().device}>
        {(dev) => (
          <>
            <h3 class="cd-sub">{props.info.codec}</h3>
            <dl class="lq-rows">
              <DeviceRows device={dev()} info={props.info} />
            </dl>
          </>
        )}
      </Show>
      <Show when={chipsShown()}>
        <p class="hint">{t("ui.diag.cd_legend")}</p>
      </Show>
      <Show when={noDevice()}>
        <p class="hint">{t("ui.diag.cd_no_device")}</p>
      </Show>
      <p class="hint">{t("ui.diag.cd_help")}</p>
    </section>
  );
}
