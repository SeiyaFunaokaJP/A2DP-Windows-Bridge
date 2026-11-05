// Diagnostics > Radio, below the link figures: the channels the link hops
// over (the controller's AFH map) on the 2.4 GHz band, the Wi-Fi channels
// drawn under it, a few tips, and in developer mode a Wi-Fi scan drawn over
// the same band.

import { createSignal, For, onCleanup, Show } from "solid-js";
import { errorText, useApp } from "../../app/context";
import { Banner, Button } from "../../components/controls";
import { lang, t } from "../../i18n";
import type { WifiAccessPoint } from "../../ipc/types";
import {
  apLevel,
  BT_CHANNELS,
  BT_FIRST_MHZ,
  busiestWifiChannel,
  channelsInUse,
  parseWifiChannels,
  WIFI_CLEAR_CHANNELS,
  WIFI_HALF_WIDTH_MHZ,
  wifiCenterMhz,
  wifiCovers,
} from "../../lib/radio";

const DOCS = "https://seiyafunaokajp.github.io/A2DP-Windows-Bridge/";

/** The last scan, kept while the window lives */
const [scan, setScan] = createSignal<WifiAccessPoint[] | null>(null);
const [scanError, setScanError] = createSignal<string | null>(null);

// The drawing: 10 units per MHz, 2400-2490 MHz
const X0 = 2400;
const W = 900;
const AP_TOP = 4;
const AP_H = 56;
const STRIP_Y = AP_TOP + AP_H + 4;
const STRIP_H = 28;
const AXIS_Y = STRIP_Y + STRIP_H + 16;
const H = AXIS_Y + 6;
const x = (mhz: number) => (mhz - X0) * 10;

export function ChannelMap() {
  const { core } = useApp();
  onCleanup(core.wantLink());
  /** The last reading while audio is sent */
  const sample = () => (core.state.snap?.stream.state === "streaming" ? (core.link().at(-1) ?? null) : null);
  const [busy, setBusy] = createSignal(false);
  const developer = () => !!core.state.snap?.settings.developer_mode;

  const inUse = () => channelsInUse(sample()?.afh_map);
  const used = () => inUse()?.filter(Boolean).length ?? 0;
  /** Bluetooth channels A2DPWB asked the adapter to avoid (Wi-Fi channels
   *  of the AFH setting) */
  const avoided = () => {
    const s = sample();
    if (!s || s.afh_usable === null || !s.afh_avoided) return new Set<number>();
    return new Set(parseWifiChannels(s.afh_avoided).flatMap(wifiCovers));
  };
  const busiest = () => busiestWifiChannel(inUse());
  /** The strip: no reading draws every channel empty */
  const cells = (): (boolean | null)[] => inUse() ?? Array<null>(BT_CHANNELS).fill(null);

  const runScan = async () => {
    setBusy(true);
    setScanError(null);
    try {
      setScan(await core.request("diag.wifi_scan"));
    } catch (e) {
      setScan(null);
      setScanError(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  /** Room above the strip only for access points to draw */
  const viewBox = () => {
    const top = (scan()?.length ?? 0) > 0 ? 0 : STRIP_Y - 4;
    return `0 ${top} ${W} ${H - top}`;
  };
  const docsUrl = () => `${DOCS}${lang() === "en" ? "" : `${lang()}/`}wireless#checklist`;
  const label = () =>
    inUse() ? t("ui.radio.in_use", used()) + (busiest() ? ` ${t("ui.radio.busy_near", busiest()!)}` : "") : "";

  return (
    <div class="radio-panel">
      <section class="card lq-group radio-map" aria-labelledby="radio-map">
        <div class="lq-group-head">
          <h2 id="radio-map">{t("ui.radio.channels")}</h2>
          <Show when={inUse()}>
            <span class="num muted">{t("ui.radio.in_use", used())}</span>
          </Show>
        </div>
        <Show when={!inUse()}>
          <p class="muted">{t("quality.not_streaming")}</p>
        </Show>
        <Show when={inUse() || (scan()?.length ?? 0) > 0}>
          {(_) => (
            <>
              <svg class="band" viewBox={viewBox()} role="img" aria-label={label()}>
                {/* Access points of the last scan, louder ones higher */}
                <For each={scan() ?? []}>
                  {(ap) => {
                    const c = wifiCenterMhz(ap.channel);
                    const top = AP_TOP + AP_H * (1 - apLevel(ap.rssi));
                    const l = x(c - WIFI_HALF_WIDTH_MHZ);
                    const r = x(c + WIFI_HALF_WIDTH_MHZ);
                    return (
                      <path
                        class="band-ap"
                        d={`M${l} ${STRIP_Y - 2} L${l + 30} ${top} L${r - 30} ${top} L${r} ${STRIP_Y - 2} Z`}
                      >
                        <title>{`${ap.ssid || t("ui.radio.hidden")} · ${ap.channel} · ${ap.rssi} dBm`}</title>
                      </path>
                    );
                  }}
                </For>
                {/* The 79 channels */}
                <For each={cells()}>
                  {(on, n) => (
                    <rect
                      class={on === null ? "band-none" : on ? "band-on" : "band-off"}
                      classList={{ "band-asked": avoided().has(n()) }}
                      x={x(BT_FIRST_MHZ + n()) + 1}
                      y={STRIP_Y}
                      width={8}
                      height={STRIP_H}
                    />
                  )}
                </For>
                {/* Wi-Fi channels 1-13 under the strip */}
                <For each={Array.from({ length: 13 }, (_, i) => i + 1)}>
                  {(ch) => (
                    <text
                      class="band-axis"
                      classList={{ strong: WIFI_CLEAR_CHANNELS.includes(ch) }}
                      x={x(wifiCenterMhz(ch)) + 5}
                      y={AXIS_Y}
                      text-anchor="middle"
                    >
                      {ch}
                    </text>
                  )}
                </For>
              </svg>
              <div class="band-legend">
                <Show when={inUse()}>
                  <span>
                    <i class="swatch band-on" />
                    {t("ui.radio.legend_on")}
                  </span>
                  <span>
                    <i class="swatch band-off" />
                    {t("ui.radio.legend_off")}
                  </span>
                </Show>
                <Show when={(scan()?.length ?? 0) > 0}>
                  <span>
                    <i class="swatch band-ap" />
                    {t("ui.radio.scan_title")}
                  </span>
                </Show>
                <span class="muted">{t("ui.radio.axis")}</span>
              </div>
              <Show when={busiest()}>
                <p class="hint">{t("ui.radio.busy_near", busiest()!)}</p>
              </Show>
            </>
          )}
        </Show>
      </section>

      <section class="card lq-group" aria-labelledby="radio-tips">
        <h2 id="radio-tips">{t("ui.radio.tips")}</h2>
        <ul class="radio-tips">
          <li>{t("ui.radio.tip_usb")}</li>
          <li>{t("ui.radio.tip_dongles")}</li>
          <li>{t("ui.radio.tip_wifi")}</li>
          <li>{t("ui.radio.tip_one_link")}</li>
        </ul>
        <div>
          <Button icon="open" kind="subtle" onClick={() => void core.transport.openUrl(docsUrl())}>
            {t("ui.radio.more")}
          </Button>
        </div>
      </section>

      <Show when={developer()}>
        <section class="card lq-group" aria-labelledby="radio-scan">
          <div class="lq-group-head">
            <h2 id="radio-scan">{t("ui.radio.scan_title")}</h2>
            <Button icon="search" busy={busy()} onClick={() => void runScan()}>
              {t("ui.radio.scan")}
            </Button>
          </div>
          <Show when={scanError()}>{(e) => <Banner kind="error">{e()}</Banner>}</Show>
          <Show when={scan()}>
            {(list) => (
              <Show when={list().length > 0} fallback={<p class="muted">{t("ui.radio.scan_empty")}</p>}>
                <table class="table">
                  <thead>
                    <tr>
                      <th scope="col">{t("ui.radio.network")}</th>
                      <th scope="col">{t("ui.radio.channel")}</th>
                      <th scope="col">{t("ui.radio.signal")}</th>
                    </tr>
                  </thead>
                  <tbody>
                    <For each={list()}>
                      {(ap) => (
                        <tr>
                          <td classList={{ muted: !ap.ssid }}>{ap.ssid || t("ui.radio.hidden")}</td>
                          <td class="num">{ap.channel}</td>
                          <td class="num">{ap.rssi} dBm</td>
                        </tr>
                      )}
                    </For>
                  </tbody>
                </table>
              </Show>
            )}
          </Show>
        </section>
      </Show>
    </div>
  );
}
