// The apps on the default output device, each with its own volume (issue
// #223): on the Volume Mixer screen and in the flyout, in developer mode.

import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { useApp } from "../app/context";
import { t } from "../i18n";
import type { MixerApp, Profile, Snapshot } from "../ipc/types";
import { Icon } from "./Icon";

/** Sessions come and go without an event */
const REFRESH_MS = 2000;

/** System Loopback plays on the default output too; developer mode opens it to every source */
export function mixerAvailable(snap: Snapshot): boolean {
  const loopback = (p: Profile) => p.capture_mode === "loopback";
  const active = snap.profiles.find((p) => p.name === snap.active_profile);
  return snap.settings.developer_mode || (active ? loopback(active) : snap.profiles.some(loopback));
}

const appName = (a: MixerApp) => (a.key === "*system" ? t("ui.mixer.system") : a.name);

export function MixerList() {
  const { core } = useApp();
  const [apps, setApps] = createSignal<MixerApp[]>([]);
  const [loaded, setLoaded] = createSignal(false);
  // The slider in the hand keeps its value through a refresh
  let dragging: string | null = null;

  const refresh = async () => {
    if (document.hidden) return;
    const list = await core.request("audio.mixer").catch(() => null);
    if (!list) return;
    setApps(list.map((a) => (a.key === dragging ? (apps().find((x) => x.key === a.key) ?? a) : a)));
    setLoaded(true);
  };
  const patch = (key: string, p: Partial<MixerApp>) =>
    setApps(apps().map((a) => (a.key === key ? { ...a, ...p } : a)));
  const send = (a: MixerApp, volume: number | null, muted: boolean | null) => {
    patch(a.key, { ...(volume === null ? {} : { volume }), ...(muted === null ? {} : { muted }) });
    void core.request("audio.mixer_set", { app: a.key, volume, muted }).catch(() => {});
  };

  onMount(() => {
    void refresh();
    const timer = setInterval(() => void refresh(), REFRESH_MS);
    onCleanup(() => clearInterval(timer));
  });

  return (
    <div class="mixer">
      <Show when={!loaded() || apps().length > 0} fallback={<p class="muted empty-line">{t("ui.mixer.empty")}</p>}>
        <For each={apps()}>
          {(a) => {
            const label = () => (a.muted ? t("ui.volume.unmute") : t("ui.volume.mute"));
            return (
              <div class="mixer-app">
                <span class="ellipsis">{appName(a)}</span>
                <div class="volume">
                  <button
                    type="button"
                    class="volume-mute"
                    aria-label={`${appName(a)}: ${label()}`}
                    aria-pressed={a.muted}
                    title={label()}
                    onClick={() => send(a, null, !a.muted)}
                  >
                    <Icon name={a.muted ? "speaker_mute" : "speaker_2"} />
                  </button>
                  <input
                    type="range"
                    min="0"
                    max="100"
                    step="1"
                    value={a.volume}
                    aria-label={appName(a)}
                    aria-valuetext={`${a.volume}%`}
                    style={{ "--fill": `${a.volume}%` }}
                    onInput={(e) => {
                      dragging = a.key;
                      send(a, Number(e.currentTarget.value), null);
                    }}
                    onChange={() => {
                      dragging = null;
                    }}
                  />
                  <span class="num volume-value">{a.volume}%</span>
                </div>
              </div>
            );
          }}
        </For>
      </Show>
    </div>
  );
}
