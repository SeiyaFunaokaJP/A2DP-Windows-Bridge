// One audio device's own settings on the Audio Devices screen (issue #86):
// what is captured for it (one list, as on Home) and whether a virtual
// device becomes the Windows default output (kept in its profile: the core
// reads them from the profile it streams; a device without one gets it with
// the first change). Windows Volume Sync, the buttons and the remembered
// volume are one setting each for every device, above the list
// (Devices.tsx); the max media packet size is on the Developer page. The
// capture source's format is a box beside it, and what the source does is
// said below it (issue #86).

import { createEffect, createMemo, createResource, createSignal, createUniqueId, on, onCleanup, onMount, Show } from "solid-js";
import { createStore, unwrap } from "solid-js/store";
import { useApp } from "../app/context";
import { t } from "../i18n";
import { codecSpec } from "../lib/codecs";
import type { AudioApp, AudioDevice, Device, Profile } from "../ipc/types";
import { LINKS } from "../lib/links";
import {
  appCaptureSupport,
  captureDevice,
  deviceFields,
  deviceFieldsComplete,
  entryBase,
  followedName,
  formatMismatch,
  normalizeProfile,
  profilesOf,
  type DeviceFields,
} from "../lib/profile";
import { captureChoices, captureText, captureValue, withCapture } from "./CapturePicker";
import { Banner, Button, Field, IconButton, Select, Toggle, type Option } from "./controls";
import { Icon } from "./Icon";

export function DeviceSettings(props: { device: Device }) {
  const app = useApp();
  const { core } = app;
  const snap = () => core.state.snap!;
  const profiles = createMemo(() => profilesOf(snap().profiles, props.device.address));
  // An LE Audio entry: its own profile
  const leEntry = () => props.device.le_audio;
  // What is shown: the first profile's, until changed here
  const [f, setF] = createStore<DeviceFields>(deviceFields(newestBase(profiles())));
  createEffect(on(profiles, (list) => list[0] && setF(deviceFields(list[0])), { defer: true }));
  const [saving, setSaving] = createSignal(false);
  // What the capture source does, below it
  const aboutId = createUniqueId();

  // ---- Audio devices and apps (asked from the core when needed) ----------
  const [devices, setDevices] = createSignal<AudioDevice[] | null>(null);
  // Windows' list was read (not only failed to be): the VB-Cable suggestion
  // relies on it
  const [listed, setListed] = createSignal(false);
  const [apps, setApps] = createSignal<AudioApp[] | null>(null);
  const loadDevices = () =>
    core
      .request("audio.devices")
      .then((list) => {
        setDevices(list);
        setListed(true);
      })
      .catch(() => {
        setDevices([]);
        setListed(false);
      });
  const loadApps = () =>
    core
      .request("audio.apps")
      .then(setApps)
      .catch(() => setApps([]));
  // Both are in the capture source's list; asked again whenever the window
  // comes back (apps start and stop playing)
  const load = () => {
    void loadDevices();
    void loadApps();
  };
  onMount(() => {
    load();
    window.addEventListener("focus", load);
  });
  onCleanup(() => window.removeEventListener("focus", load));
  const virtualDevices = createMemo(() => (devices() ?? []).filter((d) => d.is_virtual));
  // VB-Cable is suggested only when Windows has no capture source of that
  // kind at all: no virtual device (issue #86)
  const noVirtual = () => listed() && virtualDevices().length === 0;
  const [appSupport] = createResource(appCaptureSupport);
  const leOnly = () => leEntry() || (profiles().length > 0 && profiles().every((p) => p.le_audio));
  /** The shown settings as a profile (what Home's capture source list takes) */
  const shown = (): Profile => ({ ...newestBase(profiles()), ...f, le_audio: leOnly() });

  /** Changes the device's settings: into each of its profiles once complete
   *  (a virtual device or an app chosen); a profile in use starts again with
   *  them (profile.save in the core). A device without a profile gets one
   *  (Auto, as the Codecs screen would make it). */
  const apply = async (patch: Partial<DeviceFields>) => {
    setF(patch);
    const next = { ...unwrap(f) };
    if (!deviceFieldsComplete(next)) return;
    setSaving(true);
    if (profiles().length === 0) {
      const base: Profile = { ...entryBase(props.device, snap().profiles), ...next };
      base.name = followedName(base, props.device.le_audio ? "" : t(codecSpec("auto").label), snap().profiles.map((p) => p.name));
      await app.run(() => core.request("profile.save", { profile: normalizeProfile(base), original_name: null }));
    }
    for (const p of profiles()) {
      // An LE Audio profile cannot capture one application: it keeps its own
      if (p.le_audio && next.capture_mode === "app") continue;
      const updated: Profile = normalizeProfile({ ...structuredClone(unwrap(p)), ...next });
      if (JSON.stringify(deviceFields(updated)) === JSON.stringify(deviceFields(p))) continue;
      const ok = await app.run(() => core.request("profile.save", { profile: updated, original_name: p.name }));
      if (!ok) break;
    }
    setSaving(false);
  };

  /** System Loopback, the virtual devices and the playing apps in one list */
  const sources = createMemo((): Option<string>[] => {
    const p = shown();
    const c = captureChoices(p, devices() ?? [], apps() ?? [], appSupport() !== "no");
    const virtual: Option<string>[] = c.virtual.map((o) => ({ ...o, group: t("capture.virtual") }));
    if (virtual.length === 0 && noVirtual()) {
      virtual.push({ value: "virtual:", label: t("capture.no_virtual_device"), group: t("capture.virtual"), disabled: true });
    }
    const list: Option<string>[] = c.apps.map((o) => ({ ...o, group: t("capture.app") }));
    if (list.length === 0 && apps() !== null && !p.le_audio && appSupport() !== "no") {
      list.push({ value: "app:", label: t("capture.no_app_playing"), group: t("capture.app"), disabled: true });
    }
    return [{ value: "loopback", label: t("capture.loopback") }, ...virtual, ...list];
  });
  const chooseSource = (value: string) => {
    const p = shown();
    if (value === captureValue(p)) return;
    const next = withCapture(p, value, devices() ?? [], apps() ?? []);
    void apply({
      capture_mode: next.capture_mode,
      audio_device_id: next.audio_device_id,
      audio_device_name: next.audio_device_name,
      app_exe: next.app_exe,
      app_name: next.app_name,
    });
  };

  return (
    <div class="device-settings" aria-busy={saving() ? "true" : undefined}>
      <Field label={t("capture.mode")}>
        {(id) => (
          <div class="inline capture-source">
            <Icon name={captureText(shown()).icon} />
            <Select id={id} describedBy={aboutId} value={captureValue(shown())} options={sources()} onChange={chooseSource} />
            <CaptureFormat fields={f} profiles={profiles()} devices={devices()} />
            <IconButton icon="arrow_sync" label={t("capture.app_refresh")} onClick={load} />
          </div>
        )}
      </Field>
      <div class="capture-about" id={aboutId}>
        <p class="hint">
          {f.capture_mode === "virtual"
            ? t("capture.hint_virtual")
            : f.capture_mode === "app"
              ? `${t("capture.hint_app")}\n${t("capture.app_hint")}`
              : t("capture.hint_loopback")}
        </p>
        <Show when={f.capture_mode === "app" && appSupport() !== "yes" && appSupport() !== "no"}>
          <p class="hint">{t("ui.profile.app_capture_requirement")}</p>
        </Show>
        <Show when={noVirtual()}>
          <div class="inline">
            <span class="hint">{t("capture.virtual_supported")}</span>
            <Button icon="open" onClick={() => core.transport.openUrl(LINKS.vbCable)}>
              {t("vbcable.download")}
            </Button>
          </div>
        </Show>
      </div>
      <FormatWarning fields={f} profiles={profiles()} devices={devices()} />
      {/* Whether this Windows can capture one app: only the core knows the
          build for sure (it refuses the stream on an older Windows); the UI
          asks the WebView's client hints */}
      <Show when={f.capture_mode === "app" && appSupport() === "no"}>
        <Banner kind="warn">{t("ui.profile.app_capture_unsupported")}</Banner>
      </Show>
      <Show when={f.capture_mode === "virtual"}>
        <Toggle
          label={t("capture.auto_switch")}
          hint={t("tooltip.auto_switch")}
          checked={f.auto_switch_device}
          onChange={(v) => void apply({ auto_switch_device: v })}
        />
      </Show>
    </div>
  );
}

/** The profile the shown settings start from (a device without one: the defaults) */
function newestBase(list: Profile[]): Profile {
  return (
    list[0] ?? {
      ...({} as Profile),
      capture_mode: "loopback",
      audio_device_id: "",
      audio_device_name: "",
      auto_switch_device: true,
      app_exe: "",
      app_name: "",
    }
  );
}

type CaptureFormatProps = { fields: DeviceFields; profiles: Profile[]; devices: AudioDevice[] | null };

/** The format Windows mixes the captured device in (rate, bits; null while
 *  unknown), and what a profile of the device asks for otherwise */
function captureFormat(props: CaptureFormatProps) {
  const d = captureDevice(props.fields, props.devices ?? []);
  if (!d || d.mix_rate === null || d.mix_bits === null) return null;
  // The first profile that asks for another format
  const off = props.profiles.map((p) => formatMismatch(p, d)).find((m) => m.rate || m.bits) ?? null;
  return { rate: d.mix_rate, bits: d.mix_bits, off };
}

/**
 * The format Windows mixes the captured device in, a read-only box beside
 * the capture source (issue #86), amber when a profile of the device asks
 * for another rate or bit depth (v1 capture.device_format).
 */
function CaptureFormat(props: CaptureFormatProps) {
  return (
    <Show when={captureFormat(props)}>
      {(c) => (
        <span
          class="format-box"
          classList={{ "st-warn": !!c().off }}
          role="note"
          aria-label={t("capture.device_format", c().rate, c().bits)}
          title={t("capture.device_format", c().rate, c().bits)}
        >
          <Show when={c().off}>
            <Icon name="warning" />
          </Show>
          <span class="num">{c().rate} Hz</span>
          <span class="num">{c().bits === 16 || c().bits === 24 ? t(`bitdepth.${c().bits}`) : `${c().bits}-bit`}</span>
        </span>
      )}
    </Show>
  );
}

/** Why the format box is amber: it still streams, converted (issue #86).
 *  It speaks of the audio device, never of the profile by its name. */
function FormatWarning(props: CaptureFormatProps) {
  const which = (o: { rate: boolean; bits: boolean }) =>
    [o.rate ? t("connection.sample_rate") : "", o.bits ? t("connection.bit_depth") : ""].filter((x) => x).join(", ");
  return (
    <Show when={captureFormat(props)?.off}>
      {(o) => (
        <p class="field-warn">
          <Icon name="warning" />
          <span>{t("ui.profile.format_differs", which(o()))}</span>
        </p>
      )}
    </Show>
  );
}
