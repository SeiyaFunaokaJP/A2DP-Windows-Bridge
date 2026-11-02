// The apps on the default output device, each with its own volume (issue
// #223): on the Volume Mixer screen and in the flyout, in developer mode.

import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { createStore, reconcile } from "solid-js/store";
import { useApp } from "../app/context";
import { t } from "../i18n";
import type { MixerApp } from "../ipc/types";
import { Icon } from "./Icon";

/** Sessions come and go without an event */
const REFRESH_MS = 2000;

const appName = (a: MixerApp) => (a.key === "*system" ? t("ui.mixer.system") : a.name);

export function MixerList() {
  const { core } = useApp();
  // A store keeps each row's element through a change: a new element would
  // take the thumb out of the pointer's hand
  const [apps, setApps] = createStore<MixerApp[]>([]);
  const [loaded, setLoaded] = createSignal(false);

  const refresh = async () => {
    if (document.hidden) return;
    const list = await core.request("audio.mixer").catch(() => null);
    if (!list) return;
    setApps(reconcile(list, { key: "key" }));
    setLoaded(true);
  };
  const patch = (key: string, p: Partial<MixerApp>) => setApps((a) => a.key === key, p);

  onMount(() => {
    void refresh();
    const timer = setInterval(() => void refresh(), REFRESH_MS);
    onCleanup(() => clearInterval(timer));
  });

  return (
    <div class="mixer">
      <Show when={!loaded() || apps.length > 0} fallback={<p class="muted empty-line">{t("ui.mixer.empty")}</p>}>
        <For each={apps}>
          {(a) => {
            const label = () => (a.muted ? t("ui.volume.unmute") : t("ui.volume.mute"));
            // The value under the pointer, which a refresh does not move
            const [held, setHeld] = createSignal<number | null>(null);
            const shown = () => held() ?? a.volume;
            let lastSent = 0;
            let pending: ReturnType<typeof setTimeout> | undefined;
            let release: ReturnType<typeof setTimeout> | undefined;
            onCleanup(() => {
              clearTimeout(pending);
              clearTimeout(release);
            });
            const setVolume = (volume: number) => {
              // At most ten requests a second while dragging, and the final value
              clearTimeout(pending);
              const go = () => {
                lastSent = Date.now();
                void core.request("audio.mixer_set", { app: a.key, volume, muted: null }).catch(() => {});
              };
              const wait = 100 - (Date.now() - lastSent);
              if (wait <= 0) go();
              else pending = setTimeout(go, wait);
            };
            const letGo = () => {
              clearTimeout(release);
              release = setTimeout(() => {
                const volume = held();
                if (volume !== null) patch(a.key, { volume });
                setHeld(null);
              }, 600);
            };
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
                    onClick={() => {
                      const muted = !a.muted;
                      patch(a.key, { muted });
                      void core.request("audio.mixer_set", { app: a.key, volume: null, muted }).catch(() => {});
                    }}
                  >
                    <Icon name={a.muted ? "speaker_mute" : "speaker_2"} />
                  </button>
                  <input
                    type="range"
                    min="0"
                    max="100"
                    step="1"
                    value={shown()}
                    aria-label={appName(a)}
                    aria-valuetext={`${shown()}%`}
                    style={{ "--fill": `${shown()}%` }}
                    onInput={(e) => {
                      const v = Number(e.currentTarget.value);
                      setHeld(v);
                      clearTimeout(release);
                      setVolume(v);
                    }}
                    onChange={letGo}
                    onPointerUp={letGo}
                  />
                  <span class="num volume-value">{shown()}%</span>
                </div>
              </div>
            );
          }}
        </For>
      </Show>
    </div>
  );
}
