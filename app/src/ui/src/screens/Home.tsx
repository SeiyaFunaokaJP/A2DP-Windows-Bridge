// Home: a card per saved audio device, side by side (issue #86), each with
// its connection ring, state and codec and its own Connect / Disconnect.
// One device streams at a time: Connect on another one switches to it (the
// core stops the running stream first). The volume is on the card of the
// device in use, as each device keeps its own, and disabled on the others
// (issue #86); its codec is chosen on its card (e.g. one for games and one
// for music), its link quality below. The capture source is chosen on
// Audio Devices.

import { createEffect, createSignal, For, on, onCleanup, Show } from "solid-js";
import { useApp, type AppContext } from "../app/context";
import { saveSettings } from "../app/settings";
import { Banner, Button } from "../components/controls";
import { Icon } from "../components/Icon";
import {
  BatteryLevel,
  CodecBadge,
  currentProfile,
  ErrorPanel,
  FirmwareDownload,
  LinkSparkline,
  openFirmwareFolder,
  QualityIndicator,
  Ring,
  showsBattery,
  sourceText,
  statusText,
  VolumeControl,
} from "../components/stream";
import { t } from "../i18n";
import { IpcError, type AdapterInfo, type Snapshot, type StreamState } from "../ipc/types";
import { CARD_ORDER, cardOrder, codecSpec } from "../lib/codecs";
import { audioEntries, entryBase, entryByKey, entryOf, knownCodecs, type AudioEntry } from "../lib/profile";
import { chooseCodec, codecShown, pickedCodec, settle, settling, streamingCodec } from "./Codecs";

/** Adapters whose firmware files are missing from the settings folder or
 *  invalid (v1's "Firmware not found!" / "Firmware file is invalid!" bar) */
export function adaptersMissingFirmware(snap: Snapshot): AdapterInfo[] {
  return snap.adapter.adapters.filter((a) => a.firmware_present === false);
}

/** The bar over Home while an adapter lacks its firmware files, or the
 *  running stack found its Realtek adapter on the ROM code (issue #81) */
function FirmwareMissing() {
  const app = useApp();
  const { core, navigate } = app;
  const missing = () => adaptersMissingFirmware(core.state.snap!);
  // One whose files the core can fetch (the first: one download at a time)
  const downloadable = () => missing().find((a) => a.firmware_download);
  const toFirmware = () => navigate({ name: "adapter", arg: "firmware" });
  return (
    <Show
      when={missing().length > 0}
      fallback={
        <Show when={core.state.snap!.adapter.rom_firmware}>
          <Banner
            kind="warn"
            title={t("ui.adapter.rom_firmware")}
            actions={
              <Button kind="strong" icon="usb_stick" onClick={toFirmware}>
                {t("ui.home.firmware_fix")}
              </Button>
            }
          >
            {t("firmware.rom_status")}
          </Banner>
        </Show>
      }
    >
      <Banner
        kind="warn"
        title={missing().some((a) => a.firmware_invalid) ? t("firmware.invalid_short") : t("firmware.missing_manual")}
        actions={
          // The Adapter screen's actions right here (issue #81): the
          // download, the folder for the files fetched by hand, the details
          <>
            <Show when={downloadable()}>{(a) => <FirmwareDownload files={a().firmware_files} kind="strong" />}</Show>
            <Button icon="folder_open" onClick={() => void openFirmwareFolder(app)}>
              {t("firmware.open_folder")}
            </Button>
            <Button kind={downloadable() ? undefined : "strong"} icon="usb_stick" onClick={toFirmware}>
              {t("ui.home.firmware_fix")}
            </Button>
          </>
        }
      >
        <For each={missing()}>
          {(a) => (
            <span>
              <strong>{a.display_name}</strong> ·{" "}
              {a.firmware_invalid ? (
                t("firmware.invalid", a.firmware_invalid)
              ) : (
                <>
                  {t("firmware.required_files")} <span class="mono">{a.firmware_files.join(", ")}</span>
                </>
              )}
            </span>
          )}
        </For>
        {/* Missing files and the ROM code at once: say it is not usable (issue #81) */}
        <Show when={core.state.snap!.adapter.rom_firmware}>
          <span>{t("firmware.rom_short")}</span>
        </Show>
      </Banner>
    </Show>
  );
}

/** v1 still runs (both cannot use the adapter): on Home and in the setup */
export function V1Running() {
  const { core } = useApp();
  return (
    <Show when={core.state.snap?.v1_running}>
      <Banner kind="warn" title={t("ui.v1_running.title")}>
        {t("ui.v1_running.text")}
      </Banner>
    </Show>
  );
}

/** The stream state a device's card shows: the stream's for the device of
 *  the profile streaming (or that last failed), idle for the others */
export function entryState(snap: Snapshot, entry: AudioEntry): StreamState {
  const s = snap.stream;
  if (s.state === "idle" || !s.profile) return "idle";
  return entryOf([entry], snap.profiles.find((p) => p.name === s.profile) ?? null) ? s.state : "idle";
}

/** Connect for a device: its profile (Auto made for it if it has none);
 *  another device streaming gives way (stream.start stops it first) */
export async function connectEntry(app: AppContext, entry: AudioEntry): Promise<void> {
  const name = entry.profile?.name ?? (await chooseCodec(app, entry, "auto"));
  if (name) await app.core.request("stream.start", { profile: name });
}

export function Home() {
  const { core, navigate } = useApp();
  const snap = () => core.state.snap!;
  const stream = () => snap().stream;
  const entries = () => audioEntries(snap());
  // The device in use (or last used)
  const inUse = () => entryOf(entries(), currentProfile(snap()));
  // A failed request (Connect) until the state moves on
  const [reqError, setReqError] = createSignal<IpcError | null>(null);
  createEffect(on(() => stream().state, () => setReqError(null), { defer: true }));

  const error = () => {
    const e = reqError();
    if (e) return { code: e.code, message: e.msg };
    if (stream().state === "error") return { code: null, message: stream().message };
    const a = snap().adapter;
    if (a.status === "error" && a.error) return { code: null, message: a.error };
    return null;
  };

  return (
    <div class="page home">
      <header class="page-head">
        <h1>{t("ui.nav.home")}</h1>
      </header>

      <V1Running />
      <Show when={error()}>{(e) => <ErrorPanel code={e().code} message={e().message} />}</Show>
      <FirmwareMissing />

      <Show
        when={entries().length > 0}
        fallback={
          <section class="card empty">
            <Icon name="headphones" class="empty-icon" />
            <p>{t("ui.home.no_device")}</p>
            <Button kind="strong" icon="add" onClick={() => navigate({ name: "devices" })}>
              {t("ui.home.add_device")}
            </Button>
          </section>
        }
      >
        <div class="home-devices">
          <For each={entries().map((e) => e.key)}>
            {(key) => {
              const entry = entryByKey(entries, key);
              return <DeviceCard entry={entry()} onError={(err) => setReqError(IpcError.from(err))} />;
            }}
          </For>
        </div>

        <Show when={inUse()}>
          <div class="home-grid">
            <section class="card mini" aria-labelledby="home-link">
              <h2 id="home-link" class="mini-title">
                {t("quality.title")}
              </h2>
              <div class="mini-body">
                <QualityIndicator />
              </div>
              <LinkSparkline />
            </section>
          </div>
        </Show>
      </Show>
    </div>
  );
}

/** One device: ring, name, battery and state, codec, and its own button */
function DeviceCard(props: { entry: AudioEntry; onError: (e: unknown) => void }) {
  const app = useApp();
  const snap = () => app.core.state.snap!;
  const stream = () => snap().stream;
  const state = () => entryState(snap(), props.entry);
  const live = () => state() !== "idle";
  const running = () => state() === "streaming" || state() === "connecting" || state() === "reconnecting";
  const waiting = () => {
    const src = stream().info?.source;
    return src?.waiting ? sourceText(src, props.entry.profile).text : null;
  };
  const [busy, setBusy] = createSignal(false);
  const press = async () => {
    setBusy(true);
    try {
      if (running()) await app.core.request("stream.stop");
      else await connectEntry(app, props.entry);
    } catch (e) {
      props.onError(e);
    } finally {
      setBusy(false);
    }
  };
  const label = () =>
    state() === "streaming"
      ? t("connection.disconnect")
      : state() === "connecting" || state() === "reconnecting"
        ? t("modal.cancel")
        : t("connection.connect");
  const name = () => props.entry.name || props.entry.address;
  return (
    // The group names the device for its button ("Connect" on each card)
    <div class="home-device" classList={{ live: live() }} role="group" aria-label={name()}>
      <section class="card device-card">
        <Ring state={state()} volume={live() ? stream().volume : null}>
          <Icon name="headphones" />
        </Ring>
        <div class="device-info">
          {/* The device, battery and state, the codec; the sidebar shows the same */}
          <div class="device-name ellipsis" title={name()}>
            {name()}
          </div>
          <div class="device-state">
            <Show when={live() && showsBattery(stream())}>
              <BatteryLevel percent={stream().battery} />
              <span class="muted state-sep" aria-hidden="true">
                |
              </span>
            </Show>
            <div class={`status-line st-${state()}`} role="status" aria-live="polite">
              <span class="dot" aria-hidden="true" />
              <span>
                {/* An error's text is in the panel above, with its fix */}
                {live() ? statusText({ state: state(), message: stream().message }) : t("status.ready")}
              </span>
            </div>
          </div>
          <Show when={live()} fallback={<span class="codec-badge idle">{t("ui.codec.none")}</span>}>
            <CodecBadge picker={<CodecPicker entry={props.entry} />} />
          </Show>
          {/* The application it captures is not playing yet (the source is
              chosen on Audio Devices) */}
          <Show when={live() && waiting()}>
            {(w) => (
              <span class="capture-source st-warn" title={`${t("capture.mode")}: ${w()}`}>
                <Icon name="warning" />
                <span class="ellipsis">{w()}</span>
              </span>
            )}
          </Show>
        </div>
      </section>
      {/* Its own volume while it is the one in use, disabled on the others */}
      <VolumeControl off={!live()} />
      <button
        type="button"
        class="connect-btn"
        classList={{ on: running() }}
        disabled={busy()}
        aria-busy={busy() ? "true" : undefined}
        onClick={press}
      >
        <Icon name={running() ? "plug_disconnected" : "power"} />
        <span>{label()}</span>
      </button>
    </div>
  );
}

/** The codec of the device in use, chosen on its card (Home, the flyout;
 *  issue #86): one of the codecs it supports, as on Codecs, by name only.
 *  It shows the codec streaming (the warning beside it says why another
 *  one than chosen), else Auto's pick or the one chosen; choosing a codec turns
 *  Auto-select off, which is turned back on on Codecs. A choice applies at
 *  once: the device in use connects again with it (settings.set,
 *  profile.save in the core). An LE Audio device has LC3 only. */
export function CodecPicker(props: { entry: AudioEntry }) {
  const app = useApp();
  const snap = () => app.core.state.snap!;
  const e = () => props.entry;
  const autoAll = () => snap().settings.auto_codec;
  const codec = () => (e().profile ?? entryBase(e(), snap().profiles)).codec;
  const order = () => (autoAll() ? [...CARD_ORDER] : cardOrder(e().profile?.codec_order));
  const picked = () => pickedCodec(snap(), e(), autoAll(), order());
  // What it streams with; else Auto's pick, or the one used instead of a
  // codec it does not offer
  const value = () => streamingCodec(snap(), e()) ?? picked() ?? (codec() === "auto" ? null : codec());
  // A quick choice: what the device supports, never the codecs Codecs hides
  const choices = () => {
    const ids: string[] = order().filter((id) => codecShown(id, picked(), knownCodecs(snap().devices, e().address), true));
    const v = value();
    return v && !ids.includes(v) ? [v, ...ids] : ids;
  };
  const [busy, setBusy] = createSignal(false);
  let select: HTMLSelectElement | undefined;
  // A save that ends after the card is gone starts no wait
  let mounted = true;
  onCleanup(() => (mounted = false));
  const choose = async (id: string) => {
    if (id === value()) return;
    // Chosen already, another one streams instead (in use elsewhere)
    if (!autoAll() && id === codec()) {
      if (select) select.value = value() ?? "";
      return;
    }
    // The stream starts again with the codec: the next choice waits for it,
    // as on Codecs (issue #86)
    const running = entryState(snap(), e()) !== "idle" && entryState(snap(), e()) !== "error";
    setBusy(true);
    // Auto-select is one setting for every device (as on Codecs): turned
    // off before the codec is chosen, so the device connects again with it
    const ok =
      (!autoAll() || (await saveSettings(app, { auto_codec: false }))) && (await chooseCodec(app, e(), id)) !== null;
    setBusy(false);
    if (ok && running && mounted) settle(app);
    // Not saved: back to what the device has
    if (!ok && select) select.value = value() ?? "";
  };
  return (
    <div class="select codec-picker">
      <Show
        when={!e().le_audio}
        fallback={
          <select aria-label={t("quality.codec")} disabled>
            <option>{t("ui.codec.lc3")}</option>
          </select>
        }
      >
        <select
          ref={select}
          aria-label={t("quality.codec")}
          disabled={busy() || settling()}
          aria-busy={busy() || settling() ? "true" : undefined}
          onChange={(ev) => void choose(ev.currentTarget.value)}
        >
          <Show when={value() === null}>
            <option value="" selected disabled>
              {t("ui.codec.none")}
            </option>
          </Show>
          <For each={choices()}>
            {(id) => (
              <option value={id} selected={value() === id}>
                {t(codecSpec(id).label)}
              </option>
            )}
          </For>
        </select>
      </Show>
      <Icon name="chevron_down" class="select-chevron" />
    </div>
  );
}
