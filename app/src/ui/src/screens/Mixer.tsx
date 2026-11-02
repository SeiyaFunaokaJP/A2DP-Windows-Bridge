// Volume Mixer (in developer mode, issue #223): the output's mute, and each
// app's volume, which also works under that mute.

import { useApp } from "../app/context";
import { saveSettings } from "../app/settings";
import { Section, Toggle } from "../components/controls";
import { MixerList } from "../components/Mixer";
import { t } from "../i18n";

export function Mixer() {
  const app = useApp();
  return (
    <div class="page">
      <header class="page-head">
        <h1>{t("ui.nav.mixer")}</h1>
      </header>
      <Section title={t("ui.mixer.output")}>
        <Toggle
          label={t("capture.mute_output")}
          hint={t("capture.mute_output_hint")}
          checked={app.core.state.snap!.settings.mute_output}
          onChange={(v) => void saveSettings(app, { mute_output: v })}
        />
        <ul class="mixer-notes">
          <li>{t("ui.mixer.note_windows")}</li>
          <li>{t("ui.mixer.note_exit")}</li>
          <li>{t("ui.mixer.note_virtual")}</li>
        </ul>
      </Section>
      <Section title={t("ui.mixer.apps")}>
        <MixerList />
      </Section>
    </div>
  );
}
