// The streaming controls shared by Home and the flyout: the connection
// ring, the Connect / Disconnect button, volume, codec badge, link quality
// and the error panel with its fix.

import { createMemo, createSignal, For, onCleanup, Show, type JSX } from "solid-js";
import { errorText, useApp, type AppContext } from "../app/context";
import { t, tm } from "../i18n";
import { bitrateNoteKey, codecLabelKey } from "../lib/codecs";
import { liveBitpool } from "../lib/codecdetails";
import { copyText, khz, kbps, maskIdentifiers, percentToVolume, volumePercent } from "../lib/format";
import { bugReportUrl, firmwareUrl, type BugReport } from "../lib/links";
import { IpcError } from "../ipc/types";
import type { AdapterInfo, LinkSample, Msg, Profile, Snapshot, SourceInfo, StreamInfo, StreamState, StreamStatus } from "../ipc/types";
import { Sparkline } from "./charts";
import { Banner, Button, Dialog } from "./controls";
import { Icon } from "./Icon";
import type { IconName } from "./icons";

/** The profile the big button connects: the one streaming, else the active one */
export function currentProfile(snap: Snapshot): Profile | null {
  const name = snap.stream.state !== "idle" ? (snap.stream.profile ?? snap.active_profile) : snap.active_profile;
  return snap.profiles.find((p) => p.name === name) ?? snap.profiles[0] ?? null;
}

/** A state in words; streaming reads "Connected", as the core's own
 *  message on Home and in the tray (issue #86) */
export function stateLabel(s: StreamState): string {
  return t(s === "streaming" ? "status.connected" : `status.${s}`);
}

// A reconnect or a retry reads as plain connecting, and its end as connected
// (issue #86): no "1/3" counts; the attempts stay in the log
const PLAIN_STATUS: Record<string, string> = {
  "status.reconnect_attempt": "status.connecting",
  "status.connecting_retry": "status.connecting",
  "status.reconnected": "status.connected",
  // Why another codec is used goes on the codec's line (issue #86)
  "status.connected_fallback": "status.connected",
  "status.connected_fallback_in_use": "status.connected",
  "status.connected_fallback_experimental": "status.connected",
};

/** The device's status line: the core's message, else the state; an error's
 *  text is in its panel, with the fix */
export function statusText(s: Pick<StreamStatus, "state" | "message">): string {
  if (s.state === "error") return t("status.error");
  const plain = PLAIN_STATUS[s.message.key];
  return plain ? t(plain) : tm(s.message) || stateLabel(s.state);
}

/** The ring around the device: pulses while connecting, glows while streaming */
export function Ring(props: { state: StreamState; volume: number | null; size?: "big" | "small"; children: JSX.Element }) {
  const glow = () => (props.volume === null ? 0.6 : 0.35 + (0.65 * props.volume) / 127);
  return (
    <div
      class={`ring ring-${props.size ?? "big"} ring-${props.state}`}
      style={{ "--glow": String(glow()) }}
      aria-hidden="true"
    >
      <div class="ring-inner">{props.children}</div>
    </div>
  );
}

/** Connect / Disconnect for the current profile; reports a failed request through onError */
export function ConnectButton(props: { onError?: (e: unknown) => void; compact?: boolean }) {
  const { core } = useApp();
  const [busy, setBusy] = createSignal(false);
  const snap = () => core.state.snap!;
  const state = () => snap().stream.state;
  const profile = () => currentProfile(snap());
  const active = () => state() === "streaming" || state() === "connecting" || state() === "reconnecting";
  const label = () =>
    state() === "streaming"
      ? t("connection.disconnect")
      : state() === "connecting" || state() === "reconnecting"
        ? t("modal.cancel")
        : t("connection.connect");
  const press = async () => {
    setBusy(true);
    try {
      if (active()) await core.request("stream.stop");
      else if (profile()) await core.request("stream.start", { profile: profile()!.name });
    } catch (e) {
      props.onError?.(e);
    } finally {
      setBusy(false);
    }
  };
  return (
    <button
      type="button"
      class={`connect-btn ${props.compact ? "compact" : ""}`}
      classList={{ on: active() }}
      disabled={busy() || !profile()}
      aria-busy={busy() ? "true" : undefined}
      onClick={press}
    >
      <Icon name={active() ? "plug_disconnected" : "power"} />
      <span>{label()}</span>
    </button>
  );
}

// The volume a mute replaced (percent), for the click that unmutes it
let unmutedPercent: number | null = null;
/** Unmuting with no volume from before the mute (muted by the headphones) */
const UNMUTE_DEFAULT = 30;

/** The volume of the device in use; `off`: a device not connected, whose
 *  card keeps the slider disabled so the cards stay alike (issue #86).
 *  The speaker mutes and unmutes, as Windows' does (issue #86): volume 0 on
 *  the headphones, then the volume from before. */
export function VolumeControl(props: { off?: boolean }) {
  const { core } = useApp();
  const vol = () => (props.off ? null : (core.state.snap?.stream.volume ?? null));
  // The value under the pointer while dragging, so the core's echo of an
  // older value does not make the thumb jump back
  const [held, setHeld] = createSignal<number | null>(null);
  const shown = () => held() ?? (vol() === null ? 0 : volumePercent(vol()!));
  let lastSent = 0;
  let pending: ReturnType<typeof setTimeout> | undefined;
  let release: ReturnType<typeof setTimeout> | undefined;
  onCleanup(() => {
    clearTimeout(pending);
    clearTimeout(release);
  });
  const send = (pct: number) => {
    // At most ten requests a second while dragging, and the final value
    clearTimeout(pending);
    const go = () => {
      lastSent = Date.now();
      core.request("stream.set_volume", { volume: percentToVolume(pct) }).catch(() => {});
    };
    const wait = 100 - (Date.now() - lastSent);
    if (wait <= 0) go();
    else pending = setTimeout(go, wait);
  };
  const disabled = () => vol() === null;
  // A device not connected is not muted: its speaker is just disabled
  const muted = () => !disabled() && shown() === 0;
  const toggleMute = () => {
    if (muted()) {
      send(unmutedPercent ?? UNMUTE_DEFAULT);
      unmutedPercent = null;
    } else {
      unmutedPercent = shown();
      send(0);
    }
  };
  return (
    <div class="volume" classList={{ disabled: disabled() }}>
      <button
        type="button"
        class="volume-mute"
        disabled={disabled()}
        aria-label={muted() ? t("ui.volume.unmute") : t("ui.volume.mute")}
        aria-pressed={muted()}
        title={muted() ? t("ui.volume.unmute") : t("ui.volume.mute")}
        onClick={toggleMute}
      >
        <Icon name={muted() ? "speaker_mute" : "speaker_2"} />
      </button>
      <input
        type="range"
        min="0"
        max="100"
        step="1"
        value={shown()}
        disabled={disabled()}
        aria-label={t("volume.label")}
        aria-valuetext={disabled() ? t("ui.volume.unknown") : `${shown()}%`}
        style={{ "--fill": `${shown()}%` }}
        onInput={(e) => {
          const v = Number(e.currentTarget.value);
          setHeld(v);
          clearTimeout(release);
          send(v);
        }}
        onChange={(e) => {
          send(Number(e.currentTarget.value));
          clearTimeout(release);
          release = setTimeout(() => setHeld(null), 600);
        }}
      />
      <span class="num volume-value">{disabled() ? "–" : `${shown()}%`}</span>
    </div>
  );
}

/** A stream's codec by its translated name ("LDAC", "aptX HD") */
export function codecName(info: StreamInfo): string {
  const key = codecLabelKey(info.codec);
  return key ? t(key) : info.codec;
}

/** Whether the stream has a battery to show: a level, or "unknown" when the
 *  headphones refused to report it (another device holds their HFP) */
export const showsBattery = (s: StreamStatus) => s.battery !== null || s.battery_refused;

/** The battery level: a small battery that fills and changes colour with it,
 *  and the number, without the word beside the symbol (issue #86); null is
 *  "unknown", with the reason on hover */
export function BatteryLevel(props: { percent: number | null; class?: string }) {
  const level = () => Math.max(0, Math.min(100, props.percent ?? 0));
  const known = () => props.percent !== null;
  const tone = () => (!known() ? "unknown" : level() <= 15 ? "bad" : level() <= 40 ? "warn" : "good");
  const text = () => (known() ? t("ui.home.battery", level()) : t("ui.home.battery_unknown"));
  const tip = () => (known() ? undefined : t("ui.home.battery_refused_tip"));
  return (
    <span
      class={`battery ${props.class ?? ""}`}
      classList={{ "battery-unknown": !known() }}
      role="img"
      aria-label={tip() ? `${text()}: ${tip()}` : text()}
      title={tip()}
    >
      <svg class={`battery-icon st-${tone()}`} width="22" height="12" viewBox="0 0 22 12" aria-hidden="true">
        <rect x="0.5" y="0.5" width="18" height="11" rx="2.5" fill="none" stroke="currentColor" opacity="0.7" />
        <rect x="19.5" y="3.5" width="2" height="5" rx="1" fill="currentColor" opacity="0.7" />
        <rect x="2" y="2" width={(15 * level()) / 100} height="8" rx="1.5" fill="currentColor" />
      </svg>
      <span class="num">{known() ? `${level()}%` : "?"}</span>
    </span>
  );
}

// The stream's codec is the core's display name ("AAC"), not a profile key
const isAac = (info: StreamInfo) => info.codec === "AAC";

/** AAC runs a variable bit rate, the others an adaptive one */
export const abrLabel = (info: StreamInfo) => (isAac(info) ? "VBR" : "ABR");

/** What the ABR / VBR mark explains: AAC's VBR, SBC's adaptive bitpool, the others' ABR */
export const abrTooltipKey = (info: StreamInfo) =>
  isAac(info) ? "tooltip.vbr" : liveBitpool(info) ? "tooltip.sbc_abr" : "tooltip.abr";

/** SBC under ABR: "bitpool 41/53" (now / the highest it goes back to) */
export function bitpoolText(info: StreamInfo): string | null {
  const bp = liveBitpool(info);
  return bp ? t("ui.codec.bitpool_live", bp.now, bp.max) : null;
}

/** What the codec runs with, in short: "990 kbps", "48 kHz", "24-bit",
 *  "ABR"; SBC under ABR "328 kbps", "bitpool 53/53", ... as it moves */
export function codecDetails(info: StreamInfo): string[] {
  const out = [`${info.bitrate_kbps} kbps`];
  const bp = bitpoolText(info);
  if (bp) out.push(bp);
  out.push(`${khz(info.sample_rate)} kHz`);
  if (info.bits > 0) out.push(t(`bitdepth.${info.bits}`));
  if (info.abr) out.push(abrLabel(info));
  return out;
}

/** Beside the codec's name when it is not the one chosen: "(Opus is
 *  unsupported)", in use elsewhere, or experimental and off (issue #86); the
 *  reason is the core's message, the status line just says "Connected" */
export function fallbackNoteKey(info: StreamInfo, messageKey: string): string | null {
  if (!info.requested_codec) return null;
  if (messageKey === "status.connected_fallback_experimental") return "ui.codec.fallback_experimental";
  return info.requested_in_use ? "ui.codec.fallback_in_use" : "ui.codec.fallback_unsupported";
}

/** How long the note beside a codec used instead of the one chosen stays
 *  (issue #86: then only its warning icon, with the reason as its tooltip) */
export const FALLBACK_NOTE_MS = 5000;
// The note shown and since when, for every badge: one that comes on screen
// later (another page) does not show it again
let noteShown: { key: string; at: number } | null = null;

/** The codec in two lines (issue #86): its name, then what it runs with in
 *  a badge; `aside` goes beside the badge (the flyout's link quality).
 *  `picker` takes the name's place: the codec chosen right there (Home,
 *  the flyout). */
export function CodecBadge(props: { compact?: boolean; aside?: JSX.Element; picker?: JSX.Element }) {
  const { core } = useApp();
  const info = () => core.state.snap?.stream.info ?? null;
  const name = () => {
    const i = info();
    return i ? codecName(i) : "";
  };
  // "400 kbps — ABR on": why the bit rate is below the profile's
  const bitrateNote = (i: StreamInfo) => {
    const key = bitrateNoteKey(i);
    return key ? `${i.bitrate_kbps} kbps${t(key)}` : undefined;
  };
  const fallbackHelp = () => {
    const i = info();
    if (!i?.requested_codec) return "";
    const req = codecLabelKey(i.requested_codec);
    return t("tooltip.codec_fallback_help", req ? t(req) : i.requested_codec, name());
  };
  const fallbackNote = () => {
    const i = info();
    const key = i && fallbackNoteKey(i, core.state.snap?.stream.message.key ?? "");
    if (!i || !key) return "";
    const req = codecLabelKey(i.requested_codec!);
    return t(key, req ? t(req) : i.requested_codec!);
  };
  // The note for its first few seconds only (FALLBACK_NOTE_MS)
  const [now, setNow] = createSignal(Date.now());
  const noteFresh = createMemo(() => {
    const note = fallbackNote();
    if (!note) {
      noteShown = null;
      return false;
    }
    const key = `${core.state.snap?.stream.profile ?? ""}|${info()!.codec}|${note}`;
    if (noteShown?.key !== key) noteShown = { key, at: Date.now() };
    const left = noteShown.at + FALLBACK_NOTE_MS - now();
    if (left > 0) {
      const timer = setTimeout(() => setNow(Date.now()), left);
      onCleanup(() => clearTimeout(timer));
    }
    return left > 0;
  });
  return (
    <Show when={info()}>
      {(i) => (
        <div class="codec-info">
          <span class="codec-name" title={fallbackHelp() || undefined}>
            <Show when={i().requested_codec}>
              <Icon name="warning" class="st-warn" title={fallbackHelp()} />
            </Show>
            {props.picker ?? <strong class="ellipsis">{name()}</strong>}
            <Show when={noteFresh()}>
              <span class="codec-note ellipsis">{fallbackNote()}</span>
            </Show>
          </span>
          <div class="codec-line">
            <span class="codec-badge">
              <span class="num" title={bitrateNote(i())}>
                {i().bitrate_kbps} kbps
              </span>
              <Show when={liveBitpool(i())}>
                {(bp) => (
                  <span class="num live-bitpool" title={t("tooltip.sbc_bitpool_live", bp().min, bp().max)}>
                    {bitpoolText(i())}
                  </span>
                )}
              </Show>
              <Show when={!props.compact}>
                <span class="num">{khz(i().sample_rate)} kHz</span>
                <Show when={i().bits > 0}>
                  <span class="num">{t(`bitdepth.${i().bits}`)}</span>
                </Show>
              </Show>
              {/* Last, as in the sidebar's line (issue #86) */}
              <Show when={i().abr}>
                <span class="muted" title={t(abrTooltipKey(i()))}>
                  {abrLabel(i())}
                </span>
              </Show>
            </span>
            {props.aside}
          </div>
        </div>
      )}
    </Show>
  );
}

/** What a running stream captures (StreamInfo.source), in words: the
 *  profile's application name when the executable is the profile's */
export function sourceText(src: SourceInfo, profile: Profile | null): { icon: IconName; text: string; waiting: boolean } {
  switch (src.kind) {
    case "tone":
      return { icon: "music_note_2", text: t("receiver.source_tone"), waiting: false };
    case "virtual":
      return { icon: "desktop_speaker", text: src.name || t("capture.virtual"), waiting: false };
    case "app": {
      const known = profile && profile.app_exe && profile.app_exe.toLowerCase() === src.name.toLowerCase() ? profile.app_name : "";
      const name = known || src.name || t("capture.app");
      return { icon: "window_apps", text: src.waiting ? t("ui.source.waiting", name) : name, waiting: src.waiting };
    }
    default:
      return { icon: "speaker_2", text: t("capture.loopback"), waiting: false };
  }
}

export type QualityLevel = 0 | 1 | 2 | 3 | 4;

/** Samples (at 4 a second) the quality looks back over for drops: 10 s */
const QUALITY_DROP_SAMPLES = 40;
/** ... and for a send queue backing up: 4 s */
const QUALITY_QUEUE_SAMPLES = 16;
/** Packets in the send queue (of 64) that mean the radio fell behind */
const QUALITY_QUEUE_BACKLOG = 32;

/** Link quality from the last samples: what reaches the headphones (drops,
 *  the send queue backing up, air time). Not RSSI: controllers report it
 *  differently (vs. the golden range or not) and the headphones' power
 *  control moves it by 10-20 dB on a link that delivers everything. */
export function linkQuality(samples: readonly LinkSample[]): { level: QualityLevel; status: "good" | "warn" | "bad" } | null {
  if (samples.length === 0) return null;
  const last = samples[samples.length - 1];
  if (last.bytes_per_second === 0 && last.rssi === null) return null;
  const caps: QualityLevel[] = [4];
  const old = samples[Math.max(0, samples.length - QUALITY_DROP_SAMPLES)];
  const dropped = last.dropped - old.dropped + (last.capture_dropped - old.capture_dropped);
  if (dropped > 5) caps.push(1);
  else if (dropped > 0) caps.push(2);
  const backlog = Math.max(...samples.slice(-QUALITY_QUEUE_SAMPLES).map((s) => s.queue_depth));
  if (backlog >= QUALITY_QUEUE_BACKLOG) caps.push(2);
  else if (backlog >= QUALITY_QUEUE_BACKLOG / 2) caps.push(3);
  if (last.airtime > 0.8) caps.push(2);
  const level = Math.min(...caps) as QualityLevel;
  return { level, status: level >= 3 ? "good" : level === 2 ? "warn" : "bad" };
}

export function QualityIndicator() {
  const { core } = useApp();
  const q = createMemo(() => linkQuality(core.link()));
  const label = () => {
    const v = q();
    if (!v) return t("ui.quality.unknown");
    return v.status === "good" ? t("ui.quality.good") : v.status === "warn" ? t("ui.quality.fair") : t("ui.quality.poor");
  };
  return (
    <span class={`quality ${q() ? `st-${q()!.status}` : ""}`} title={`${t("quality.title")}: ${label()}`}>
      <span class="bars" aria-hidden="true">
        {[1, 2, 3, 4].map((n) => (
          <span class="bar" classList={{ lit: (q()?.level ?? 0) >= n }} style={{ height: `${n * 25}%` }} />
        ))}
      </span>
      <span class="quality-label">{label()}</span>
    </span>
  );
}

/** Bandwidth over the last minute, while the caller is on screen */
export function LinkSparkline() {
  const { core } = useApp();
  const release = core.wantLink();
  onCleanup(release);
  const values = createMemo(() => core.link().map((s) => kbps(s.bytes_per_second)));
  const latest = () => {
    const v = values();
    return v.length ? Math.round(v[v.length - 1]) : null;
  };
  return (
    <div class="link-mini">
      <Sparkline values={values()} min={0} label={`${t("quality.bitrate")}: ${latest() ?? "–"} kbps`} />
      <span class="num link-mini-value">{latest() === null ? "–" : `${latest()} kbps`}</span>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Errors and their fixes
// ---------------------------------------------------------------------------

export interface Fix {
  label: string;
  icon: IconName;
  run: () => void | Promise<unknown>;
}

const RETRY_KEYS = [
  "error.connect_no_answer",
  "error.connect_link_lost",
  "error.connect_refused",
  "error.connect_auth_failed",
  "error.reconnect_failed",
  "error.pairing_hint",
  "error.stream_start",
];
const PROFILE_KEYS = [
  "error.codec_not_supported",
  "error.codec_adaptive_only",
  "error.codec_sep_in_use",
  "error.no_compatible_codec",
  "error.no_virtual_device_selected",
  "error.virtual_capture_init",
  "error.no_app_selected",
  "error.app_capture_unsupported",
  "error.app_capture_init",
  "error.aac_unavailable",
];

/** The adapter A2DPWB would use but cannot: not on WinUSB */
export function adapterNeedingDriver(snap: Snapshot): AdapterInfo | null {
  return snap.adapter.adapters.find((a) => a.driver !== "winusb") ?? null;
}

/** The next step for an error: a code from a request, and/or the message key */
export function useFixes() {
  const app = useApp();
  const { core, navigate } = app;
  const [winusbFor, setWinusbFor] = createSignal<AdapterInfo | null>(null);

  const fixFor = (code: string | null, key: string): Fix | null => {
    const snap = core.state.snap;
    if (!snap) return null;
    if (code === "adapter.driver_missing" || key === "error.btstack_init" || key === "firmware.auto_no_adapter") {
      const a = adapterNeedingDriver(snap);
      if (a) return { label: t("ui.adapter.assign_winusb"), icon: "shield", run: () => void setWinusbFor(a) };
      return { label: t("ui.nav.adapter"), icon: "usb_stick", run: () => navigate({ name: "adapter" }) };
    }
    if (key === "firmware.rom_running") {
      // The Adapter screen names the files and opens the download page
      return { label: t("ui.nav.adapter"), icon: "usb_stick", run: () => navigate({ name: "adapter", arg: "firmware" }) };
    }
    if (code === "adapter.firmware_missing" || key.startsWith("firmware.")) {
      // The Adapter screen downloads the files where the core can fetch them
      if (snap.adapter.adapters.some((a) => a.firmware_present === false && a.firmware_download)) {
        return { label: t("ui.home.firmware_fix"), icon: "usb_stick", run: () => navigate({ name: "adapter", arg: "firmware" }) };
      }
      const vendor = snap.adapter.adapters[0]?.vendor ?? "";
      return { label: t("firmware.open_download_page"), icon: "open", run: () => core.transport.openUrl(firmwareUrl(vendor)) };
    }
    if (key === "error.btstack_no_answer" || key.startsWith("error.btstack_crash")) {
      return { label: t("ui.adapter.restart"), icon: "arrow_reset", run: () => core.request("adapter.restart") };
    }
    if (RETRY_KEYS.includes(key)) {
      const p = currentProfile(snap);
      if (p) return { label: t("ui.fix.retry"), icon: "arrow_sync", run: () => core.request("stream.start", { profile: p.name }) };
    }
    // The codec is chosen on Codecs, the capture source on Audio Devices
    if (PROFILE_KEYS.includes(key)) {
      const codec = key.startsWith("error.codec") || key === "error.no_compatible_codec" || key === "error.aac_unavailable";
      return codec
        ? { label: t("ui.nav.codecs"), icon: "headphones", run: () => navigate({ name: "codecs" }) }
        : { label: t("ui.nav.devices"), icon: "bluetooth", run: () => navigate({ name: "devices" }) };
    }
    return { label: t("ui.nav.diagnostics"), icon: "pulse", run: () => navigate({ name: "diagnostics" }) };
  };

  const dialog = () => <WinusbDialog adapter={winusbFor()} mode="assign" onClose={() => setWinusbFor(null)} />;
  return { fixFor, dialog };
}

/** The text "copy the error" puts on the clipboard: the message, and the
 *  code of a failed request (what an issue report needs) */
export function errorCopyText(message: Msg, code?: string | null): string {
  const text = tm(message) || message.key;
  return code ? `${text}\n[${code}]` : text;
}

/** Errors with a cause on the user's side (the device off or out of range,
 *  pairing, the adapter's setup, a choice the profile lacks): their fix is
 *  the answer, a bug report would only say "it does not connect" */
const USER_SIDE_ERRORS = new Set([
  "error.connect_no_answer",
  "error.connect_link_lost",
  "error.connect_refused",
  "error.connect_auth_failed",
  "error.pairing_hint",
  "error.reconnect_failed",
  "error.btstack_init",
  "error.btstack_no_answer",
  "error.codec_not_supported",
  "error.codec_adaptive_only",
  "error.codec_sep_in_use",
  "error.no_compatible_codec",
  "error.le_audio_disabled",
  "error.le_audio_unavailable",
  "error.le_audio_connect",
  "error.le_audio_capture",
  "error.no_virtual_device_selected",
  "error.no_app_selected",
  "error.app_capture_unsupported",
  "error.uac_cancelled",
  "error.driver_assign_failed",
  "error.invalid_address",
  "error.invalid_address_format",
  "error.sbc_bitpool_range",
  "error.profile_not_found",
]);
/** Request errors with such a cause: an input or choice to correct, a
 *  prompt declined */
const USER_SIDE_CODES = new Set([
  "adapter.driver_missing",
  "adapter.uac_cancelled",
  "app.streaming",
  "device.invalid_address",
  "device.invalid_name",
  "profile.invalid",
  "profile.invalid_address",
  "profile.not_found",
  "stream.bad_codec",
  "stream.bad_source",
]);

/** Whether an error points at A2DPWB itself (worth a bug report); anything
 *  not known to be on the user's side is */
export function isAppFault(key: string, code?: string | null): boolean {
  return !USER_SIDE_ERRORS.has(key) && !(code && USER_SIDE_CODES.has(code));
}

/** A bug report's URL with what the state tells (the issue form's fields)
 *  around the diagnostic info. Not the device's name: it is often the
 *  owner's ("Taro's AirPods") and the issue is public, so the headphones
 *  field stays for the user to fill in with the model. */
export function bugReportFor(snap: Snapshot, info: string, extra: Partial<BugReport> = {}): string {
  const profile = currentProfile(snap);
  const adapter = snap.adapter.adapters.find((a) => a.driver === "winusb") ?? snap.adapter.adapters[0];
  const hex = (n: number) => n.toString(16).toUpperCase().padStart(4, "0");
  return bugReportUrl({
    version: snap.update.current_version,
    info,
    adapter: adapter && `${adapter.display_name} (${hex(adapter.vid)}:${hex(adapter.pid)})`,
    codec: profile?.codec,
    captureMode: profile?.capture_mode,
    ...extra,
  });
}

/** What a report holds: `head` (the error, the crash), `details` (a crash
 *  report's text) and the diagnostic info with its warnings and recent log
 *  (identifiers masked: it goes into a public issue; the core masks its
 *  info, the UI the head and the details, which name the user's folder in
 *  a crash report's dump path). `log` is the details and the info for the
 *  issue's log field; `text` all of it, for the clipboard. Without the
 *  core's info: what the UI has. */
export async function reportText(app: AppContext, head: string, details = ""): Promise<{ text: string; log: string }> {
  const info = await app.core
    .request("diag.info", { mask: true })
    .then((d) => d.text.replace(/^```text\n/, "").replace(/\n?```\s*$/, "\n"))
    .catch(() => "");
  head = maskIdentifiers(head);
  const log = [maskIdentifiers(details.trimEnd()), info].filter(Boolean).join("\n\n");
  return { text: log ? `${head}\n\n\`\`\`text\n${log.trimEnd()}\n\`\`\`\n` : head, log };
}

/** Copies a report, ready to paste */
export async function copyReport(app: AppContext, head: string, details = "") {
  const { text } = await reportText(app, head, details);
  const ok = await copyText(text);
  app.notify(ok ? t("ui.error.copied") : t("dbgc.copy_failed"), ok ? "info" : "error");
}

/** Opens a GitHub issue prefilled with a report; the whole text also goes
 *  to the clipboard (the URL holds only part of a long log) */
export async function openReport(app: AppContext, head: string, details = "") {
  try {
    const { text, log } = await reportText(app, head, details);
    const what = `${head}\n\n`;
    const snap = app.core.state.snap;
    const url = snap
      ? bugReportFor(snap, log, { whatHappened: what })
      : bugReportUrl({ version: "", info: log, whatHappened: what });
    await app.core.transport.openUrl(url);
    await copyText(text).catch(() => false);
    app.notify(t("ui.error.report_opened"));
  } catch (e) {
    app.notify(errorText(e), "error");
  }
}

/** Copies an error with the log around it */
export function copyError(app: AppContext, message: Msg, code?: string | null) {
  return copyReport(app, errorCopyText(message, code));
}

/** A GitHub issue about an error */
export function reportError(app: AppContext, message: Msg, code?: string | null) {
  return openReport(app, errorCopyText(message, code));
}

/** An error with its cause and the button that fixes it */
export function ErrorPanel(props: { code?: string | null; message: Msg; title?: string }) {
  const app = useApp();
  const { fixFor, dialog } = useFixes();
  const fix = () => fixFor(props.code ?? null, props.message.key);
  const [busy, setBusy] = createSignal(false);
  const run = async () => {
    const f = fix();
    if (!f) return;
    setBusy(true);
    await app.run(async () => f.run());
    setBusy(false);
  };
  return (
    <>
      <Banner
        kind="error"
        title={props.title ?? t("status.error")}
        actions={
          <>
            <Show when={fix()}>
              {(f) => (
                <Button kind="strong" icon={f().icon} busy={busy()} onClick={run}>
                  {f().label}
                </Button>
              )}
            </Show>
            <Button
              kind="subtle"
              icon="copy"
              title={t("ui.error.copy_hint")}
              onClick={() => void copyError(app, props.message, props.code)}
            >
              {t("ui.error.copy")}
            </Button>
            <Show when={isAppFault(props.message.key, props.code)}>
              <Button
                kind="subtle"
                icon="open"
                title={t("ui.error.report_hint")}
                onClick={() => void reportError(app, props.message, props.code)}
              >
                {t("ui.diag.report_github")}
              </Button>
            </Show>
          </>
        }
      >
        {tm(props.message)}
      </Banner>
      {dialog()}
    </>
  );
}

/**
 * Assign WinUSB / restore the driver, after saying what it does: while
 * A2DPWB has the adapter, Windows' Bluetooth does not see it.
 */
export function WinusbDialog(props: { adapter: AdapterInfo | null; mode: "assign" | "restore"; onClose: (done: boolean) => void }) {
  const app = useApp();
  const [busy, setBusy] = createSignal(false);
  const go = async () => {
    const a = props.adapter;
    if (!a) return;
    setBusy(true);
    try {
      if (props.mode === "assign") {
        const list = await app.core.request("adapter.assign_winusb", { instance_id: a.instance_id });
        // Windows loads WinUSB only after a replug or a restart (issue #81)
        if (list.some((x) => x.instance_id === a.instance_id && x.restart_needed)) {
          app.notify(t("ui.adapter.assigned_restart", a.display_name), "error");
        } else {
          app.notify(t("ui.adapter.assigned", a.display_name));
        }
      } else {
        await app.core.request("adapter.restore_driver", { instance_id: a.instance_id });
        app.notify(t("ui.adapter.restored", a.display_name));
      }
      props.onClose(true);
    } catch (e) {
      app.notify(errorText(e), "error");
      props.onClose(false);
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog
      open={props.adapter !== null}
      title={props.mode === "assign" ? t("ui.adapter.assign_title") : t("ui.adapter.restore_title")}
      confirm={props.mode === "assign" ? t("ui.adapter.assign_winusb") : t("ui.adapter.restore_driver")}
      cancel={t("modal.cancel")}
      busy={busy()}
      danger={props.mode === "restore"}
      onConfirm={go}
      onCancel={() => props.onClose(false)}
    >
      <p>
        <strong>{props.adapter?.display_name}</strong>
        <span class="muted"> · {props.adapter?.device_name}</span>
      </p>
      <p>{props.mode === "assign" ? t("ui.adapter.assign_explain") : t("ui.adapter.restore_explain")}</p>
      <p class="muted">{t("ui.adapter.uac_note")}</p>
    </Dialog>
  );
}

/** Opens the firmware folder (the shell creates it if it is not there yet) */
export function openFirmwareFolder(app: AppContext) {
  return app.run(() => app.core.transport.openFolder(`${app.core.state.snap!.config_dir}\\firmware`));
}

/** Fetches an adapter's Realtek files from linux-firmware once the user
 *  has read where they come from and under which terms (issue #81) */
export function FirmwareDownload(props: { files: string[]; kind?: "strong" }) {
  const app = useApp();
  const [open, setOpen] = createSignal(false);
  const [busy, setBusy] = createSignal(false);
  const go = async () => {
    setBusy(true);
    try {
      await app.core.request("adapter.download_firmware", { file: props.files[0] });
      app.notify(t("firmware.downloaded"));
    } catch (e) {
      // The UI stopped waiting, the core did not: it publishes the adapter
      // anew when the files are saved, so this is no failure yet
      if (IpcError.from(e).code === "ipc.timeout") app.notify(t("firmware.download_slow"));
      else app.notify(errorText(e), "error");
    } finally {
      setBusy(false);
      setOpen(false);
    }
  };
  return (
    <>
      <Button kind={props.kind} icon="arrow_download" busy={busy()} onClick={() => setOpen(true)}>
        {t("firmware.download")}
      </Button>
      <Dialog
        open={open()}
        title={t("firmware.download_title")}
        confirm={t("firmware.download")}
        cancel={t("modal.cancel")}
        busy={busy()}
        onConfirm={go}
        onCancel={() => setOpen(false)}
      >
        <Show
          when={busy()}
          fallback={
            <>
              <p>{t("firmware.download_explain")}</p>
              <ul class="mono files">
                <For each={props.files}>{(f) => <li>{f}</li>}</For>
              </ul>
              <p class="muted">{t("firmware.download_license")}</p>
            </>
          }
        >
          <div class="working" role="status">
            <span class="spinner big" aria-hidden="true" />
            <p>{t("firmware.downloading")}</p>
          </div>
        </Show>
      </Dialog>
    </>
  );
}
