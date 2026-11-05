// Choosing the device for the big button (the flyout), one entry per audio
// device (its saved settings, internally a Profile, since the Codecs screen,
// issue #86). While a stream runs, choosing another device streams to that
// one (stream.start stops the running one); otherwise it becomes the active one.

import { useApp } from "../app/context";
import { t } from "../i18n";
import { audioEntries } from "../lib/profile";
import { Select } from "./controls";
import { currentProfile } from "./stream";

export function DeviceSwitcher() {
  const app = useApp();
  const snap = () => app.core.state.snap!;
  const value = () => currentProfile(snap())?.name ?? "";
  const options = () =>
    audioEntries(snap()).flatMap((e) =>
      e.profile
        ? [{ value: e.profile.name, label: `${e.name || e.address}${e.le_audio ? ` (${t("device.tag_le_audio")})` : ""}` }]
        : [],
    );
  const choose = (name: string) => {
    const running = snap().stream.state === "streaming" || snap().stream.state === "connecting";
    void app.run(() =>
      running ? app.core.request("stream.start", { profile: name }) : app.core.request("profile.set_active", { name }),
    );
  };
  return (
    <div class="device-switcher">
      <Select label={t("ui.home.device")} value={value()} options={options()} onChange={choose} />
    </div>
  );
}
