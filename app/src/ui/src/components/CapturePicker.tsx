// The capture source choices of a device (its settings on Audio Devices):
// System Loopback, a virtual device or a playing app. The source is the
// device's: the choice is saved into all the device's profiles (issue #86);
// a stream in use takes it without reconnecting (profile.save in the core).
// Home chose it too until issue #86 put the codec and the link latency there.

import { t } from "../i18n";
import type { AudioApp, AudioDevice, Profile } from "../ipc/types";
import type { IconName } from "./icons";

/** The profile's capture source, as the profile list and an idle Home show it */
export function captureText(p: Profile): { icon: IconName; text: string } {
  switch (p.capture_mode) {
    case "virtual":
      return { icon: "desktop_speaker", text: p.audio_device_name || t("capture.virtual") };
    case "app":
      return { icon: "window_apps", text: p.app_name || p.app_exe || t("capture.app") };
    default:
      return { icon: "speaker_2", text: t("capture.loopback") };
  }
}

interface Choice {
  value: string;
  label: string;
}

/** The value of the profile's capture source among the choices */
export function captureValue(p: Profile): string {
  if (p.capture_mode === "virtual") return `virtual:${p.audio_device_id}`;
  if (p.capture_mode === "app") return `app:${p.app_exe.toLowerCase()}`;
  return "loopback";
}

/** The choices: virtual devices and playing apps, the profile's own also when
 *  it is not there now. No apps for LE Audio (error.le_audio_capture) or on a
 *  Windows that cannot capture one app. */
export function captureChoices(
  p: Profile,
  devices: readonly AudioDevice[],
  apps: readonly AudioApp[],
  appsOk: boolean,
): { virtual: Choice[]; apps: Choice[] } {
  const virtual = devices.filter((d) => d.is_virtual).map((d) => ({ value: `virtual:${d.id}`, label: d.name }));
  if (p.capture_mode === "virtual" && p.audio_device_id && !virtual.some((c) => c.value === captureValue(p))) {
    virtual.unshift({
      value: captureValue(p),
      label: t("capture.volume_sync_missing", p.audio_device_name || p.audio_device_id),
    });
  }
  const list: Choice[] = [];
  if (!p.le_audio && appsOk) {
    for (const a of apps) {
      const value = `app:${a.exe.toLowerCase()}`;
      if (!list.some((c) => c.value === value)) list.push({ value, label: a.name });
    }
  }
  if (p.capture_mode === "app" && p.app_exe && !list.some((c) => c.value === captureValue(p))) {
    list.unshift({ value: captureValue(p), label: t("capture.app_not_playing", p.app_name || p.app_exe) });
  }
  return { virtual, apps: list };
}

/** The profile with the chosen capture source */
export function withCapture(
  p: Profile,
  value: string,
  devices: readonly AudioDevice[],
  apps: readonly AudioApp[],
): Profile {
  if (value.startsWith("virtual:")) {
    const id = value.slice("virtual:".length);
    const d = devices.find((x) => x.id === id);
    return { ...p, capture_mode: "virtual", audio_device_id: id, audio_device_name: d?.name ?? p.audio_device_name };
  }
  if (value.startsWith("app:")) {
    const exe = value.slice("app:".length);
    if (p.app_exe.toLowerCase() === exe) return { ...p, capture_mode: "app" };
    const a = apps.find((x) => x.exe.toLowerCase() === exe);
    return { ...p, capture_mode: "app", app_exe: a?.exe ?? exe, app_name: a?.name ?? exe };
  }
  return { ...p, capture_mode: "loopback" };
}
