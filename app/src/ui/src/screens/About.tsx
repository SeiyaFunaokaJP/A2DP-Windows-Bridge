// About: the program, its version, licences and where to find the author.

import { useApp } from "../app/context";
import { Button, Section } from "../components/controls";
import { t } from "../i18n";
import { LINKS, thirdPartyUrl } from "../lib/links";

export function About() {
  const app = useApp();
  const { core } = app;
  const update = () => core.state.snap!.update;
  // The licence texts shipped next to the programs; a development build
  // has none, so the release's copy online
  const openLicenses = () => {
    const dir = core.transport.licensesDir;
    void (dir ? core.transport.openFolder(dir) : core.transport.openUrl(thirdPartyUrl(core.transport.version)));
  };
  return (
    <div class="page">
      <header class="page-head">
        <h1>{t("ui.nav.about")}</h1>
      </header>
      <Section title={t("help.about_title")}>
        <div class="about">
          <img src="/app-icon.png" alt="" width="48" height="48" />
          <div>
            <strong>{t("help.full_name")}</strong>
            <p class="muted num">
              {t("help.version")} {core.transport.version} · {t("ui.settings.core_version", update().current_version)}
            </p>
            <p class="muted">{t("help.author")}</p>
          </div>
        </div>
        <p class="hint">{t("help.license")}</p>
        <p class="hint">{t("help.binary_license")}</p>
        <p class="hint">{t("ui.settings.icons_license")}</p>
        <p class="hint">{t("help.third_party")}</p>
        <div class="inline wrap">
          <Button icon="open" kind="subtle" onClick={() => core.transport.openUrl(LINKS.repo)}>
            {t("help.github")}
          </Button>
          <Button icon="open" kind="subtle" onClick={() => openLicenses()}>
            {t("help.third_party_link")}
          </Button>
          <Button icon="open" kind="subtle" onClick={() => core.transport.openUrl(LINKS.issues)}>
            {t("menu.help.report_issue")}
          </Button>
          <Button icon="open" kind="subtle" onClick={() => core.transport.openUrl(LINKS.sponsor)}>
            {t("help.sponsor")}
          </Button>
        </div>
      </Section>
    </div>
  );
}
