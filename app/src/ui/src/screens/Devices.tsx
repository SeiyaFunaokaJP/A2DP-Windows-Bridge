// Audio Devices: what holds for every audio device (Windows Volume Sync, the
// buttons, the remembered volume: one switch each, issue #86), then the
// headphones and speakers A2DPWB knows, each with its own settings (capture
// source, default output switching), and a card to add one: scan for new
// ones or type an address. Profiles then pick from this list.

import { createSignal, onCleanup, onMount, Show } from "solid-js";
import { useApp } from "../app/context";
import { saveSettings } from "../app/settings";
import { Toggle } from "../components/controls";
import { DevicePicker } from "../components/DevicePicker";
import { ImportPairings } from "../components/ImportPairings";
import { DeviceSettings, VbCableNote } from "../components/DeviceSettings";
import { t } from "../i18n";
import { vbCable } from "../lib/profile";

export function Devices() {
  const { core } = useApp();
  // Not installed, once Windows' list was read (installed meanwhile: gone
  // when the window comes back)
  const [noCable, setNoCable] = createSignal(false);
  const load = () =>
    core
      .request("audio.devices")
      .then((list) => setNoCable(vbCable(list) === null))
      .catch(() => {});
  onMount(() => {
    void load();
    window.addEventListener("focus", load);
  });
  onCleanup(() => window.removeEventListener("focus", load));
  return (
    <div class="page">
      <header class="page-head">
        <h1>{t("ui.nav.devices")}</h1>
      </header>
      <Show when={noCable()}>
        <section class="card device-note">
          <VbCableNote />
        </section>
      </Show>
      <AllDevices />
      <ImportPairings />
      <DevicePicker manage address="" onPick={() => {}} settings={(d) => <DeviceSettings device={d} />} />
    </div>
  );
}

/** The settings shared by every audio device (Settings, saved at once; a
 *  Volume Sync or output mute change starts the profile in use again in the core) */
function AllDevices() {
  const app = useApp();
  const s = () => app.core.state.snap!.settings;
  return (
    <section class="card device-global" aria-labelledby="devices-all">
      <h2 id="devices-all" class="mini-title">
        {t("ui.devices.all_title")}
      </h2>
      <Toggle
        label={t("capture.volume_sync")}
        hint={t("capture.volume_sync_hint")}
        checked={s().volume_sync}
        onChange={(v) => void saveSettings(app, { volume_sync: v })}
      />
      <Toggle
        label={t("settings.media_keys")}
        checked={s().media_keys}
        onChange={(v) => void saveSettings(app, { media_keys: v })}
      />
      <Toggle
        label={t("settings.auto_reconnect")}
        checked={s().auto_reconnect}
        onChange={(v) => void saveSettings(app, { auto_reconnect: v })}
      />
    </section>
  );
}
