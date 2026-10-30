// Picking the headphones: the saved devices and the ones a scan found
// (device.scan), or an address typed by hand; a saved device can be renamed
// or removed from the list (v1 OnDeviceEditName / OnDeviceDelete). The
// first-run setup connects one with a button on its row instead. With LE
// Audio (experimental) on, the LE Audio entries too: a dual-mode device is
// listed twice, its classic entry and its LE Audio one (address and kind
// name an entry). Once one is picked, the others are folded away until asked
// for or a new scan. Used by the profile editor and the first-run setup.

import { createEffect, createMemo, createSignal, createUniqueId, For, on, onCleanup, onMount, Show, type JSX } from "solid-js";
import { useApp, type AppContext } from "../app/context";
import { t } from "../i18n";
import type { Device } from "../ipc/types";
import { parseAddress } from "../lib/format";
import {
  defaultProfileName,
  deviceKey,
  deviceProfile,
  listedDevices,
  newProfile,
  normalizeProfile,
  validDeviceName,
} from "../lib/profile";
import { Button, Dialog, Field, IconButton } from "./controls";
import { Icon } from "./Icon";

/** How long a scan looks for devices (issue #86: 10 s found too few) */
export const SCAN_SECONDS = 30;
/** The first-run setup looks until a device is added (issue #86): scans of
 *  the core's longest, one after another */
export const SETUP_SCAN_SECONDS = 600;

export function sortDevices(devices: readonly Device[]): Device[] {
  // Saved first, then audio devices by signal; other kinds (keyboards...) last
  return [...devices].sort((a, b) => {
    if (a.audio !== b.audio) return a.audio ? -1 : 1;
    if (a.saved !== b.saved) return a.saved ? -1 : 1;
    return (b.rssi ?? -127) - (a.rssi ?? -127);
  });
}

/**
 * Saves a device found by a scan or typed (the list keeps its name; an
 * unnamed one is saved under its address and can be renamed). A device
 * without a profile gets one that picks the best codec ("<name> | Auto").
 * The profile of the device, or null when saving failed.
 */
export async function saveWithProfile(app: AppContext, address: string, name: string, leAudio: boolean): Promise<string | null> {
  const snap = () => app.core.state.snap!;
  // A saved device typed by its address keeps its name
  const known = snap().devices.find((d) => d.saved && d.le_audio === leAudio && d.address.toUpperCase() === address.toUpperCase());
  const saved = name.trim() || known?.name || address;
  const ok =
    (known && known.name === saved) ||
    (await app.run(() =>
      app.core.request("device.rename", { address, name: saved, ...(leAudio ? { le_audio: true } : {}) }),
    ));
  // LE Audio profiles are made on the Codecs screen (its LC3 card)
  if (!ok || leAudio) return null;
  // The device's own (classic) profile: one per device entry
  const own = deviceProfile(snap(), address, false);
  if (own) return own.name;
  const taken = snap().profiles.map((p) => p.name);
  const base = defaultProfileName(saved, address, false, t("codec.auto"));
  let profileName = base;
  for (let n = 2; taken.includes(profileName); n++) profileName = `${base} (${n})`;
  // Captured by System Loopback, as every new device (issue #86)
  const profile = normalizeProfile({ ...newProfile(), name: profileName, device_address: address, device_name: saved });
  const made = await app.run(() => app.core.request("profile.save", { profile, original_name: null }));
  return made ? profileName : null;
}

export function DevicePicker(props: {
  address: string;
  /** The chosen entry is the LE Audio one */
  leAudio?: boolean;
  onPick: (address: string, name: string, leAudio: boolean) => void;
  error?: string | null;
  /** Classic entries only (the first-run setup makes an A2DP profile) */
  classicOnly?: boolean;
  /** The Audio Devices screen: no choosing; scan, add, rename and forget */
  manage?: boolean;
  /** The profile editor: the saved devices only, to choose from; they are
   *  managed on the Audio Devices screen */
  savedOnly?: boolean;
  /** With manage: a saved device's own settings, below its row */
  settings?: (d: Device) => JSX.Element;
  /** The first-run setup: no choosing; Connect on each row and Use on the
   *  typed address */
  onConnect?: (address: string, name: string) => void;
  /** With onConnect: the address being connected (its button spins, the
   *  others wait) */
  connecting?: string | null;
  /** With onConnect: no connection now (another BlueCaster holds the adapter) */
  connectDisabled?: boolean;
  /** With onConnect: the device already added, no longer listed (issue #86) */
  hidden?: string;
  /** The first-run setup: the scan starts by itself and goes on until a
   *  device is added, Stop is pressed or the step is left (issue #86) */
  continuous?: boolean;
}) {
  const app = useApp();
  const snap = () => app.core.state.snap!;
  // LE Audio entries with the setting on and an adapter known to stream it
  const leListed = () =>
    snap().settings.le_audio_experimental &&
    snap().adapter.le_audio === true &&
    !props.classicOnly;
  const devices = createMemo(() => {
    const hidden = props.hidden?.toUpperCase();
    const all = sortDevices(listedDevices(snap().devices, leListed())).filter(
      (d) => !hidden || d.le_audio || d.address.toUpperCase() !== hidden,
    );
    return props.savedOnly ? all.filter((d) => d.saved) : all;
  });
  const chosen = (d: Device) =>
    d.address === props.address && d.le_audio === (props.leAudio ?? false);
  // Rename and forget name the entry: the LE Audio one with le_audio
  const kind = (d: Device) => (d.le_audio ? { le_audio: true } : {});
  // Picked here: the list shows the chosen entry only
  const [folded, setFolded] = createSignal(false);
  const shown = () =>
    folded() && !props.manage ? devices().filter(chosen) : devices();
  const others = () => devices().length - shown().length;
  const pick = (address: string, name: string, leAudio: boolean) => {
    props.onPick(address, name, leAudio);
    setFolded(true);
  };
  const [manual, setManual] = createSignal("");
  const scanning = () => snap().scanning;
  // Stop pressed, or the picker gone: a continuous scan ends there
  let stopped = false;
  let gone = false;
  const scan = async () => {
    setFolded(false);
    stopped = false;
    for (;;) {
      const ok = await app.run(() =>
        app.core.request("device.scan", { seconds: props.continuous ? SETUP_SCAN_SECONDS : SCAN_SECONDS }),
      );
      // A refused scan (a device streams) is not tried again
      if (!ok || !props.continuous || stopped || gone || props.connecting) return ok;
    }
  };
  // The devices show up as they answer: the user stops once theirs is there
  const stopScan = () => {
    stopped = true;
    return app.run(() => app.core.request("device.scan_stop"));
  };
  onMount(() => {
    if (props.continuous && !scanning()) void scan();
  });
  // Continuous again (the setup's device disconnected to choose another): the scan resumes
  createEffect(
    on(
      () => props.continuous,
      (now, before) => {
        if (now && before === false && !scanning()) void scan();
      },
      { defer: true },
    ),
  );
  onCleanup(() => {
    gone = true;
    if (props.continuous && scanning()) void app.core.request("device.scan_stop").catch(() => {});
  });
  const name = "device-pick";
  const uid = createUniqueId();

  // Rename: the device and the name being typed
  const [renaming, setRenaming] = createSignal<Device | null>(null);
  const [newName, setNewName] = createSignal("");
  const [renameTried, setRenameTried] = createSignal(false);
  const nameError = () =>
    renameTried() && !validDeviceName(newName())
      ? t("ui.device.err_name")
      : null;
  const [forgetting, setForgetting] = createSignal<Device | null>(null);
  const [busy, setBusy] = createSignal(false);

  const rename = async () => {
    const d = renaming();
    const n = newName().trim();
    setRenameTried(true);
    if (!d || !validDeviceName(n) || busy()) return;
    setBusy(true);
    const ok = await app.run(() =>
      app.core.request("device.rename", {
        address: d.address,
        name: n,
        ...kind(d),
      }),
    );
    setBusy(false);
    if (!ok) return;
    setRenaming(null);
    // The profile shows the new name (v1 set the name field too)
    if (chosen(d)) props.onPick(d.address, n, d.le_audio);
  };
  // A device without a profile gets one and is connected with it right
  // away (pairing it, as other systems do); the user makes more. A running
  // stream and the first-run setup (it connects the device itself) are left
  // alone.
  const add = async (address: string, name: string, leAudio: boolean) => {
    if (busy()) return;
    setBusy(true);
    const had = snap().profiles.some((p) => p.device_address.toUpperCase() === address.toUpperCase());
    const profileName = await saveWithProfile(app, address, name, leAudio);
    const state = snap().stream.state;
    if (profileName && !had && !app.setupActive() && (state === "idle" || state === "error")) {
      await app.run(() => app.core.request("stream.start", { profile: profileName }));
    }
    setBusy(false);
  };
  const forget = async () => {
    const d = forgetting();
    if (!d || busy()) return;
    setBusy(true);
    await app.run(() =>
      app.core.request("device.forget", { address: d.address, ...kind(d) }),
    );
    setBusy(false);
    setForgetting(null);
  };

  // One device's row; on the Audio Devices screen a saved one has its
  // settings below it
  const row = (d: Device, i: () => number, prefix: string) => {
    const label = () => d.name || t("device.name_unknown");
    const nameId = () => `${uid}-${prefix}${i()}`;
    return (
      // The radio covers the row (app.css); the buttons sit above it
      <div
        class="device-row"
        classList={{ on: chosen(d), dim: !d.audio }}
        data-key={deviceKey(d)}
      >
        <Show when={!props.manage && !props.onConnect}>
          <input
            type="radio"
            name={name}
            value={deviceKey(d)}
            checked={chosen(d)}
            aria-labelledby={nameId()}
            onChange={() => pick(d.address, d.name, d.le_audio)}
          />
        </Show>
        <Icon name={d.audio ? "headphones" : "bluetooth"} />
        <span class="device-row-text" id={nameId()}>
          <span class="device-row-name">{label()}</span>{" "}
          <span class="mono muted">{d.address}</span>
        </span>
        <span class="tags">
          <Show when={d.saved}>
            <span class="tag">{t("device.tag_saved")}</span>
          </Show>
          <Show
            when={d.le_audio}
            fallback={
              <Show when={d.audio}>
                <span class="tag">{t("device.tag_audio")}</span>
              </Show>
            }
          >
            <span class="tag">
              {t(
                d.le_audio_set
                  ? "device.tag_le_audio_set"
                  : "device.tag_le_audio",
              )}
            </span>
          </Show>
          <Show when={d.rssi !== null}>
            <span class="tag num">{d.rssi} dBm</span>
          </Show>
        </span>
        <Show when={props.onConnect}>
          {(connect) => (
            <span class="inline device-row-actions">
              <Button
                kind={d.audio ? "strong" : undefined}
                icon="power"
                busy={props.connecting === d.address}
                disabled={!!props.connecting || props.connectDisabled}
                onClick={() => connect()(d.address, d.name)}
              >
                {t("connection.connect")}
              </Button>
            </span>
          )}
        </Show>
        <Show when={props.manage && !d.saved}>
          <span class="inline device-row-actions">
            <Button
              kind="subtle"
              icon="add"
              disabled={busy()}
              onClick={() =>
                void add(d.address, d.name, d.le_audio)
              }
            >
              {t("ui.devices.add")}
            </Button>
          </span>
        </Show>
        <Show when={d.saved && !props.savedOnly && !props.onConnect}>
          <span class="inline device-row-actions">
            <IconButton
              icon="edit"
              label={`${t("device.edit_name")}: ${label()}`}
              onClick={() => {
                setNewName(d.name);
                setRenameTried(false);
                setRenaming(d);
              }}
            />
            <IconButton
              icon="delete"
              label={`${t("ui.device.forget")}: ${label()}`}
              onClick={() => setForgetting(d)}
            />
          </span>
        </Show>
      </div>
    );
  };

  const head = (count: number) => (
    <div class="picker-head">
      <span class="muted" role="status" aria-live="polite">
        {scanning()
          ? t(
              snap().le_scanning
                ? "status.le_scanning"
                : "connection.scanning_hint",
            )
          : t("ui.devices.count", count)}
      </span>
      <Show
        when={!props.savedOnly}
        fallback={
          <Button
            icon="bluetooth"
            onClick={() => (location.hash = "#/devices")}
          >
            {t("ui.devices.manage")}
          </Button>
        }
      >
        <Button
          icon={scanning() ? "stop" : "search"}
          onClick={() => void (scanning() ? stopScan() : scan())}
        >
          {scanning() ? t("connection.scan_stop") : t("connection.scan")}
        </Button>
      </Show>
    </div>
  );

  const manualField = () => (
    <Field
      label={t("connection.manual")}
      hint={t("ui.devices.manual_hint")}
      error={
        manual() && !parseAddress(manual())
          ? t("error.invalid_address_format")
          : props.error
      }
    >
      {(id, described) => (
        <div class="inline">
          <input
            id={id}
            class="input mono"
            placeholder="AA:BB:CC:DD:EE:FF"
            value={manual()}
            aria-describedby={described}
            spellcheck={false}
            onInput={(e) => setManual(e.currentTarget.value)}
          />
          <Button
            disabled={!parseAddress(manual()) || !!props.connecting || (!!props.onConnect && props.connectDisabled)}
            busy={!!props.connecting && props.connecting === parseAddress(manual())}
            onClick={() => {
              const a = parseAddress(manual());
              if (!a) return;
              if (props.onConnect) {
                props.onConnect(a, "");
              } else if (props.manage) {
                void add(a, "", false).then(() => setManual(""));
              } else {
                // A typed address keeps the kind chosen (the profile's check box)
                pick(a, "", props.leAudio ?? false);
              }
            }}
          >
            {props.manage
              ? t("ui.devices.add")
              : t("ui.devices.use_address")}
          </Button>
        </div>
      )}
    </Field>
  );

  // The Audio Devices screen (issue #86): the saved devices, each with its
  // settings, then a card to add one (scan, or the address by hand)
  const saved = () => devices().filter((d) => d.saved);
  const found = () => devices().filter((d) => !d.saved);
  const managed = () => (
    <>
      <div class="device-list saved-devices">
        <Show
          when={saved().length > 0}
          fallback={<p class="muted empty-line">{t("ui.devices.none_saved")}</p>}
        >
          <For each={saved()}>
            {(d, i) => (
              <div class="device-entry">
                {row(d, i, "s")}
                {props.settings?.(d)}
              </div>
            )}
          </For>
        </Show>
      </div>
      <section class="card device-add" aria-label={t("ui.devices.add_title")}>
        <h2 class="mini-title">{t("ui.devices.add_title")}</h2>
        {head(found().length)}
        <Show when={found().length > 0 || scanning()}>
          <div class="device-list">
            <Show
              when={found().length > 0}
              fallback={<p class="muted empty-line">{t("connection.no_devices")}</p>}
            >
              <For each={found()}>{(d, i) => row(d, i, "f")}</For>
            </Show>
          </div>
        </Show>
        {manualField()}
      </section>
    </>
  );

  return (
    <div class="device-picker">
      <Show when={!props.manage} fallback={managed()}>
        {head(devices().length)}
        {/* Folded with a typed address: no row, only the button below */}
        <Show when={!folded() || shown().length > 0}>
          <div
            class="device-list"
            role="radiogroup"
            aria-label={t("connection.device_addr")}
          >
            <Show
              when={shown().length > 0}
              fallback={
                <p class="muted empty-line">
                  {t(
                    props.savedOnly
                      ? "ui.devices.saved_empty"
                      : "connection.no_devices",
                  )}
                </p>
              }
            >
              <For each={shown()}>{(d, i) => row(d, i, "")}</For>
            </Show>
          </div>
        </Show>
        <Show when={folded() && others() > 0 && !props.manage}>
          <div>
            <Button
              kind="subtle"
              icon="chevron_down"
              onClick={() => setFolded(false)}
            >
              {t("ui.devices.show_others", others())}
            </Button>
          </div>
        </Show>
        <Show when={props.savedOnly && props.error}>
          <p class="field-error">
            <Icon name="warning" />
            <span>{props.error}</span>
          </p>
        </Show>
        <Show when={!props.savedOnly}>{manualField()}</Show>
      </Show>
      <Dialog
        open={renaming() !== null}
        title={t("device.edit_name_title")}
        confirm={t("modal.save")}
        cancel={t("modal.cancel")}
        busy={busy()}
        onConfirm={() => void rename()}
        onCancel={() => setRenaming(null)}
      >
        <Field
          label={t("device.enter_name")}
          hint={
            renaming()
              ? `${renaming()!.address}${renaming()!.le_audio ? ` (${t("device.tag_le_audio")})` : ""}`
              : undefined
          }
          error={nameError()}
        >
          {(id, described) => (
            <input
              id={id}
              class="input"
              value={newName()}
              maxLength={64}
              aria-describedby={described}
              aria-invalid={nameError() ? "true" : undefined}
              data-autofocus
              onInput={(e) => setNewName(e.currentTarget.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  void rename();
                }
              }}
            />
          )}
        </Field>
      </Dialog>
      <Dialog
        open={forgetting() !== null}
        title={t("modal.confirm_title")}
        confirm={t("ui.device.forget")}
        cancel={t("modal.cancel")}
        danger
        busy={busy()}
        onConfirm={() => void forget()}
        onCancel={() => setForgetting(null)}
      >
        <p>{t("ui.device.forget_confirm")}</p>
        <p>
          <strong>{forgetting()?.name || t("device.name_unknown")}</strong>{" "}
          <span class="mono muted">{forgetting()?.address}</span>
          <Show when={forgetting()?.le_audio}>
            {" "}
            <span class="tag">{t("device.tag_le_audio")}</span>
          </Show>
        </p>
      </Dialog>
    </div>
  );
}
