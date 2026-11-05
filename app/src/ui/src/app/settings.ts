import { unwrap } from "solid-js/store";
import type { Settings, SettingsApplied } from "../ipc/types";
import type { AppContext } from "./context";

/** The Volume Mixer (screen, flyout list, output mute): Developer > Volume
 *  Mixer on, in developer mode (issue #86) */
export function mixerShown(s: Pick<Settings, "developer_mode" | "volume_mixer"> | undefined): boolean {
  return !!s?.developer_mode && s.volume_mixer;
}

/** Changes some settings: the core takes the whole object and sends it back as an event */
export function saveSettings(app: AppContext, patch: Partial<Settings>): Promise<boolean> {
  return applySettings(app, patch).then((r) => r !== null);
}

/** As saveSettings, with what the core did besides saving (null: it failed, already told) */
export async function applySettings(app: AppContext, patch: Partial<Settings>): Promise<SettingsApplied | null> {
  const current = unwrap(app.core.state.snap!.settings);
  let applied: SettingsApplied | null = null;
  const ok = await app.run(async () => {
    // An older core answers null
    applied = (await app.core.request("settings.set", { settings: { ...current, ...patch } })) ?? {
      le_audio_deferred: false,
    };
  });
  return ok ? applied : null;
}
