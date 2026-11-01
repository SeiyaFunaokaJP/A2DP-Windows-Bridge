// First-run setup, shown when there is no profile or no adapter on WinUSB.
// Four steps (issue #86): 1 choose the adapter, 2 driver and firmware (one
// button switches it to WinUSB and downloads its firmware), 3 find the audio
// devices: the scan starts by itself, and Connect on a device makes its
// "<name> | Auto" profile and connects with it, so one click is enough.
// 4 the general settings worth asking (Start with Windows and in the tray are
// on by default). The scan runs while nothing is connected, as the core
// refuses one during a stream: a connected device joins the saved ones above
// the list, and adding another one disconnects it. With VB-Cable installed
// new devices capture it; without it the step says why one is wanted. The
// setup has a window size of its own.

import { createEffect, createMemo, createSignal, For, Match, on, onCleanup, onMount, Show, Switch } from "solid-js";
import { errorText, useApp, type AppContext } from "../app/context";
import { saveSettings } from "../app/settings";
import { Banner, Button, Toggle } from "../components/controls";
import { DevicePicker, saveWithProfile } from "../components/DevicePicker";
import { Icon } from "../components/Icon";
import { openFirmwareFolder, statusText } from "../components/stream";
import { t } from "../i18n";
import { IpcError } from "../ipc/types";
import type { AdapterInfo, AudioDevice, Device, Snapshot } from "../ipc/types";
import { firmwareUrl, LINKS } from "../lib/links";
import { audioEntries, deviceKey, vbCable } from "../lib/profile";
import {
  adapterExperimental,
  adapterIds,
  adapterIdsHint,
  adapterName,
  adapterSupported,
  driverText,
  usableFirst,
} from "./Adapter";
import { entryState, V1Running } from "./Home";
import { BatteryLowChoice, ThemeChoice } from "./Settings";

export function needsSetup(snap: Snapshot): boolean {
  return snap.profiles.length === 0 || !snap.adapter.adapters.some((a) => a.driver === "winusb");
}

const STEP_KEYS = ["ui.setup.step_adapter", "ui.setup.step_driver", "ui.setup.step_device", "ui.setup.step_general"];
/** Each step's heading; the last two have headings of their own below it */
const HEADING_KEYS = ["ui.setup.adapter_intro", "ui.setup.selected_adapter", "ui.setup.step_device", "ui.setup.step_general"];
const DEVICE_STEP = 2;
const LAST = STEP_KEYS.length - 1;

/** The classic saved devices: what the setup finds and connects */
const classicEntries = (snap: Snapshot) => audioEntries(snap).filter((e) => !e.le_audio);

/** The adapter's name and what A2DPWB can do with it (issue #86): Device
 *  Manager's name with the chip as the title, its IDs and the driver below,
 *  and a green "Supported" or red "Not supported" tag */
function AdapterLine(props: { adapter: AdapterInfo }) {
  const ok = () => adapterSupported(props.adapter);
  return (
    <>
      <Icon name="usb_stick" />
      <span class="choice-text">
        <strong>{adapterName(props.adapter)}</strong>
        <span class="muted" title={adapterIdsHint(props.adapter)}>
          {adapterIds(props.adapter)} · {driverText(props.adapter.driver).text}
        </span>
      </span>
      <Show
        when={!ok() || !adapterExperimental(props.adapter)}
        fallback={
          <span class="tag st-warn" title={t("ui.adapter.experimental_hint")}>
            {t("ui.adapter.experimental")}
          </span>
        }
      >
        <span class={`tag st-${ok() ? "good" : "bad"}`}>{t(ok() ? "ui.setup.supported" : "ui.adapter.firmware_unsupported")}</span>
      </Show>
    </>
  );
}

export function Setup() {
  const app = useApp();
  const { core } = app;
  const snap = () => core.state.snap!;
  const [step, setStep] = createSignal(0);
  const [chosen, setChosen] = createSignal<string | null>(null);
  const adapter = createMemo<AdapterInfo | null>(() => {
    const list = supportedFirst(snap().adapter.adapters);
    return (
      list.find((a) => a.instance_id === chosen()) ??
      list.find((a) => a.driver === "winusb" && adapterSupported(a)) ??
      list.find(adapterSupported) ??
      list[0] ??
      null
    );
  });
  // A first setup: no profile yet when it reached the devices
  let first = false;

  onMount(() => void core.transport.setupWindow?.(true).catch(() => {}));
  onCleanup(() => void core.transport.setupWindow?.(false).catch(() => {}));

  const finish = () => {
    app.setSetupActive(false);
    app.navigate({ name: "home" });
  };

  const canNext = () => {
    // Another A2DPWB (v1, or another v2) holds the adapter: the banner asks
    // to exit it first, the setup waits (Skip stays)
    if (snap().v1_running) return false;
    switch (step()) {
      case 0:
        return adapter() !== null;
      case 1:
        return adapter()?.driver === "winusb";
      default:
        return classicEntries(snap()).some((e) => e.profile);
    }
  };

  let heading: HTMLHeadingElement | undefined;
  // "Start with Windows" and "Start in the tray" are on by default (issue
  // #86): turned on once when a first setup reaches them; switched off
  // there, they stay off
  let startupOn = false;
  const go = (n: number) => {
    if (n === DEVICE_STEP && snap().profiles.length === 0) first = true;
    if (n === LAST && !startupOn) {
      startupOn = true;
      const s = snap().settings;
      if (first && !(s.start_with_windows && s.start_minimized)) {
        void saveSettings(app, { start_with_windows: true, start_minimized: true });
      }
    }
    setStep(n);
    // Screen readers and keyboard users land on the new step's title
    queueMicrotask(() => heading?.focus());
  };

  return (
    <div class="setup">
      <header class="setup-head">
        <img src="/app-icon.png" alt="" width="32" height="32" />
        <div>
          <h1>{t("ui.setup.wizard_title")}</h1>
          <p class="muted">{t("ui.setup.intro")}</p>
        </div>
      </header>
      <V1Running />
      <ol class="stepper">
        <For each={STEP_KEYS}>
          {(k, i) => (
            <li classList={{ on: i() === step(), done: i() < step() }} aria-current={i() === step() ? "step" : undefined}>
              <span class="stepper-num num">{i() < step() ? <Icon name="checkmark" /> : i() + 1}</span>
              <span>{t(k)}</span>
            </li>
          )}
        </For>
      </ol>
      <section class="card setup-body" aria-labelledby="setup-step-title">
        <h2 id="setup-step-title" tabindex="-1" ref={heading} classList={{ "sr-only": step() >= DEVICE_STEP }}>
          {t(HEADING_KEYS[step()])}
        </h2>
        <Switch>
          <Match when={step() === 0}>
            <StepAdapter chosen={adapter()} onChoose={setChosen} />
          </Match>
          <Match when={step() === 1}>
            <StepDriver adapter={adapter()} />
          </Match>
          <Match when={step() === DEVICE_STEP}>
            <StepDevice />
          </Match>
          <Match when={step() === LAST}>
            <StepFinish />
          </Match>
        </Switch>
      </section>
      <footer class="setup-foot">
        <Show when={step() < LAST}>
          <Button kind="subtle" onClick={finish}>
            {t("ui.setup.skip")}
          </Button>
        </Show>
        <span class="spacer" />
        <Show when={step() > 0}>
          <Button icon="arrow_left" onClick={() => go(step() - 1)}>
            {t("ui.setup.back")}
          </Button>
        </Show>
        <Show
          when={step() < LAST}
          fallback={
            <Button kind="strong" icon="checkmark" onClick={finish}>
              {t("ui.setup.finish")}
            </Button>
          }
        >
          <Button kind="strong" icon="arrow_right" disabled={!canNext()} onClick={() => go(step() + 1)}>
            {t("ui.setup.next")}
          </Button>
        </Show>
      </footer>
    </div>
  );
}

/** Usable first (the Adapters screen's order), the unsupported ones last;
 *  with the same driver a Realtek chip goes before the others, so that a
 *  PC's built-in adapter (often Intel) is not the one chosen by default */
export function supportedFirst(list: readonly AdapterInfo[]): AdapterInfo[] {
  const usable = usableFirst(list);
  // Stable sort: within usableFirst's driver rank, Realtek first, else as listed
  const rank = (a: AdapterInfo) => (a.driver === "winusb" ? 0 : a.driver === "none" ? 2 : 1);
  const supported = usable
    .filter(adapterSupported)
    .sort((a, b) => rank(a) - rank(b) || Number(!a.realtek_chip) - Number(!b.realtek_chip));
  return [...supported, ...usable.filter((a) => !adapterSupported(a))];
}

function StepAdapter(props: { chosen: AdapterInfo | null; onChoose: (id: string) => void }) {
  const app = useApp();
  const adapters = () => app.core.state.snap!.adapter.adapters;
  const [busy, setBusy] = createSignal(false);
  const refresh = async () => {
    setBusy(true);
    await app.run(() => app.core.request("adapter.list"));
    setBusy(false);
  };
  return (
    <div class="stack">
      <Show
        when={adapters().length > 0}
        fallback={
          <Banner kind="warn" title={t("firmware.no_adapters")}>
            {t("ui.adapter.none_hint")}
          </Banner>
        }
      >
        <div class="choice-list" role="radiogroup" aria-label={t("firmware.detected_adapters")}>
          <For each={supportedFirst(adapters())}>
            {(a) => (
              // One A2DPWB cannot use is listed, not chosen (issue #86)
              <label
                class="choice"
                classList={{ on: props.chosen?.instance_id === a.instance_id, ro: !adapterSupported(a) }}
              >
                <input
                  type="radio"
                  name="setup-adapter"
                  checked={props.chosen?.instance_id === a.instance_id}
                  disabled={!adapterSupported(a)}
                  onChange={() => props.onChoose(a.instance_id)}
                />
                <AdapterLine adapter={a} />
              </label>
            )}
          </For>
        </div>
      </Show>
      <div>
        <Button icon="arrow_sync" busy={busy()} onClick={refresh}>
          {t("ui.adapter.refresh")}
        </Button>
      </div>
    </div>
  );
}

/**
 * One explanation and one button (issue #86: the driver and firmware
 * choices were intimidating): it switches the adapter to WinUSB and
 * downloads its firmware when it needs some. The separate steps stay on the
 * Adapters screen. Firmware A2DPWB cannot fetch still has the manual way.
 */
function StepDriver(props: { adapter: AdapterInfo | null }) {
  const app = useApp();
  const { core } = app;
  const [phase, setPhase] = createSignal<"idle" | "driver" | "firmware">("idle");
  const [failed, setFailed] = createSignal<string | null>(null);
  // The download outlived the UI's wait; the core finishes it and publishes the adapter
  const [slow, setSlow] = createSignal(false);

  const setUp = async (a: AdapterInfo) => {
    setFailed(null);
    setSlow(false);
    try {
      let now = a;
      if (a.driver !== "winusb") {
        setPhase("driver");
        const list = await core.request("adapter.assign_winusb", { instance_id: a.instance_id });
        now = list.find((x) => x.instance_id === a.instance_id) ?? a;
      }
      if (now.firmware_present === false && now.firmware_download) {
        setPhase("firmware");
        try {
          await core.request("adapter.download_firmware", { file: now.firmware_files[0] });
        } catch (e) {
          if (IpcError.from(e).code !== "ipc.timeout") throw e;
          setSlow(true);
        }
      }
    } catch (e) {
      setFailed(errorText(e));
    } finally {
      setPhase("idle");
    }
  };

  return (
    <Show when={props.adapter} fallback={<p class="muted">{t("firmware.no_adapters")}</p>}>
      {(a) => {
        const needsDriver = () => a().driver !== "winusb";
        const needsFirmware = () => a().firmware_present === false;
        // Firmware the user has to fetch: the button does the driver only
        const manualFirmware = () => needsFirmware() && !a().firmware_download;
        const todo = () => needsDriver() || (needsFirmware() && a().firmware_download);
        return (
          <div class="stack">
            <div class="selected-device">
              <AdapterLine adapter={a()} />
            </div>
            <Show when={!adapterSupported(a())}>
              <p class="field-error">
                <Icon name="warning" />
                <span>{t("ui.setup.unsupported_note")}</span>
              </p>
            </Show>
            <Switch>
              <Match when={phase() !== "idle"}>
                <div class="working" role="status">
                  <span class="spinner big" aria-hidden="true" />
                  <p>{phase() === "driver" ? t("ui.setup.auto_driver") : t("firmware.downloading")}</p>
                </div>
              </Match>
              <Match when={todo()}>
                <p>{t(needsDriver() ? "ui.setup.auto_explain_admin" : "ui.setup.auto_explain")}</p>
                <ul class="files">
                  <Show when={needsDriver()}>
                    <li>{t("ui.setup.auto_will_driver")}</li>
                  </Show>
                  <Show when={needsFirmware() && a().firmware_download}>
                    <li>
                      {t("ui.setup.auto_will_firmware")} <span class="mono">{a().firmware_files.join(", ")}</span>
                      <p class="hint">{t("ui.setup.license_note")}</p>
                    </li>
                  </Show>
                </ul>
                <div>
                  <Button
                    kind="strong"
                    icon="checkmark"
                    disabled={!!core.state.snap?.v1_running || !adapterSupported(a())}
                    onClick={() => setUp(a())}
                  >
                    {t("ui.setup.auto_button")}
                  </Button>
                </div>
                <p class="hint">{t("ui.setup.auto_advanced")}</p>
              </Match>
              <Match when={!manualFirmware()}>
                <Banner kind="good" title={t("ui.setup.driver_ok")} />
              </Match>
            </Switch>
            <Show when={failed()}>
              {(why) => (
                <Banner kind="error" title={t("ui.setup.auto_failed")}>
                  <p>{why()}</p>
                  <p class="hint">{t("ui.setup.auto_advanced")}</p>
                </Banner>
              )}
            </Show>
            <Show when={slow() && needsFirmware()}>
              <Banner kind="info" title={t("firmware.download_slow")} />
            </Show>
            <Show when={!needsDriver() && a().restart_needed}>
              <Banner kind="warn" title={t("ui.adapter.assigned_restart", a().display_name)} />
            </Show>
            <Show when={phase() === "idle" && manualFirmware()}>
              <Banner
                kind="warn"
                title={a().firmware_invalid ? t("firmware.invalid_short") : t("firmware.missing_manual")}
                actions={
                  <>
                    <Button icon="open" onClick={() => core.transport.openUrl(firmwareUrl(a().vendor))}>
                      {t("firmware.open_download_page")}
                    </Button>
                    <Button icon="folder_open" onClick={() => openFirmwareFolder(app)}>
                      {t("firmware.open_folder")}
                    </Button>
                    <Button icon="open" kind="subtle" onClick={() => core.transport.openUrl(LINKS.setupGuide)}>
                      {t("ui.setup.guide")}
                    </Button>
                  </>
                }
              >
                <Show when={a().firmware_invalid}>{(file) => <p>{t("firmware.invalid", file())}</p>}</Show>
                <p>
                  {t("firmware.required_files")} <span class="mono">{a().firmware_files.join(", ")}</span>
                </p>
                <p class="hint">{t("firmware.manual_instructions")}</p>
              </Banner>
            </Show>
          </div>
        );
      }}
    </Show>
  );
}

/** Resolves once no scan runs: a connection waits for the scan to end */
function scanEnded(app: AppContext): Promise<void> {
  return new Promise((resolve) => {
    const check = () => (app.core.state.snap?.scanning ? setTimeout(check, 200) : resolve());
    check();
  });
}

function StepDevice() {
  const app = useApp();
  const snap = () => app.core.state.snap!;
  const stream = () => snap().stream;
  const entries = createMemo(() => classicEntries(snap()));
  // VB-Cable, when Windows' list was read: new devices capture it, without
  // it the step says why one is wanted (issue #86)
  const [cable, setCable] = createSignal<AudioDevice | null | undefined>(undefined);
  const load = () =>
    app.core
      .request("audio.devices")
      .then((list) => setCable(vbCable(list)))
      .catch(() => {});
  onMount(() => {
    void load();
    window.addEventListener("focus", load);
  });
  onCleanup(() => window.removeEventListener("focus", load));
  // The device being connected: its button spins
  const [connecting, setConnecting] = createSignal<string | null>(null);
  // No scan from a Connect until the stream it starts has ended
  const [paused, setPaused] = createSignal(false);
  const running = () => ["connecting", "reconnecting", "streaming"].includes(stream().state);
  createEffect(on(running, (now) => void (now || setPaused(false)), { defer: true }));

  // One click: the device is saved, gets its "<name> | Auto" profile (the
  // best codec it has) and is connected with it
  const connect = async (address: string, name: string) => {
    if (connecting() || snap().v1_running) return;
    setConnecting(address);
    setPaused(true);
    try {
      const profile = await saveWithProfile(app, address, name, false, cable());
      const ok =
        profile &&
        (await app.run(async () => {
          await app.core.request("profile.set_active", { name: profile });
          // The stream waits for the scan to end
          if (snap().scanning) await app.core.request("device.scan_stop");
          await scanEnded(app);
          if (running()) await app.core.request("stream.stop");
          await app.core.request("stream.start", { profile });
        }));
      if (!ok) setPaused(false);
    } finally {
      setConnecting(null);
    }
  };
  // Disconnect (or Cancel while it connects), or making room for another
  // device: the scan resumes
  const [stopping, setStopping] = createSignal(false);
  const disconnect = async () => {
    setStopping(true);
    try {
      if (running()) await app.run(() => app.core.request("stream.stop"));
    } finally {
      setStopping(false);
    }
  };

  // A saved device's state and its Connect or Disconnect
  const paired = (d: Device) => {
    const key = deviceKey(d);
    const state = () => {
      const entry = entries().find((e) => e.key === key);
      return entry ? entryState(snap(), entry) : "idle";
    };
    const live = () => state() !== "idle" && state() !== "error";
    return {
      state: (
        <span class={`status-line st-${state()}`} role="status" aria-live="polite">
          {state() === "idle" ? t("status.ready") : statusText({ state: state(), message: stream().message })}
        </span>
      ),
      action: (
        <Show
          when={live()}
          fallback={
            <Button
              icon="power"
              busy={connecting() === d.address}
              disabled={!!connecting() || snap().v1_running}
              onClick={() => void connect(d.address, d.name)}
            >
              {t("connection.connect")}
            </Button>
          }
        >
          <Button icon="plug_disconnected" busy={stopping()} onClick={disconnect}>
            {state() === "streaming" ? t("connection.disconnect") : t("modal.cancel")}
          </Button>
        </Show>
      ),
    };
  };

  return (
    <div class="stack">
      <DevicePicker
        address=""
        classicOnly
        onPick={() => {}}
        onConnect={(a, n) => void connect(a, n)}
        connecting={connecting()}
        connectDisabled={snap().v1_running}
        paired={paired}
        scanText={t("ui.setup.scanning")}
        noScan={running()}
        continuous={!paused()}
      >
        <Show when={cable() === null}>
          <div class="setup-note">
            <p>{t("ui.setup.vbcable_note")}</p>
            <Button icon="open" onClick={() => app.core.transport.openUrl(LINKS.vbCable)}>
              {t("vbcable.download")}
            </Button>
          </div>
        </Show>
      </DevicePicker>
      <Show when={running()}>
        <div>
          <Button icon="add" busy={stopping()} onClick={disconnect}>
            {t("ui.home.add_device")}
          </Button>
        </div>
      </Show>
    </div>
  );
}

/** The general settings worth asking at the start (issue #86): the same
 *  controls as on Settings, saved at once */
function StepFinish() {
  const app = useApp();
  const s = () => app.core.state.snap!.settings;
  const set = (patch: Parameters<typeof saveSettings>[1]) => void saveSettings(app, patch);
  return (
    <div class="stack setup-general">
      <ThemeChoice />
      <section class="stack-tight" aria-labelledby="setup-startup">
        <h3 id="setup-startup">{t("ui.settings.startup")}</h3>
        <Toggle
          label={t("settings.start_with_windows")}
          checked={s().start_with_windows}
          onChange={(v) => set({ start_with_windows: v })}
        />
        <Toggle
          label={t("ui.settings.start_minimized")}
          hint={t("ui.settings.start_minimized_logon")}
          checked={s().start_minimized}
          disabled={!s().start_with_windows}
          onChange={(v) => set({ start_minimized: v })}
        />
      </section>
      <section class="stack-tight" aria-labelledby="setup-notifications">
        <h3 id="setup-notifications">{t("ui.settings.notifications")}</h3>
        <Toggle
          label={t("ui.settings.notify_connected")}
          checked={s().notify_connected}
          onChange={(v) => set({ notify_connected: v })}
        />
        <BatteryLowChoice />
      </section>
      <Toggle
        label={t("settings.check_updates_on_startup")}
        checked={s().check_updates_on_startup}
        onChange={(v) => set({ check_updates_on_startup: v })}
      />
    </div>
  );
}
