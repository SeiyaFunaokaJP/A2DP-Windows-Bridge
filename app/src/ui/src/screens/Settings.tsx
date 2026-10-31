// Settings: the general ones only: language, theme, startup, notifications,
// updates, developer mode, the settings folder. (A device's buttons and
// remembered volume are on Audio Devices; LE Audio and its GMAP low latency
// are chosen per profile.)

import { createSignal, Match, onMount, Show, Switch } from "solid-js";
import { errorText, useApp } from "../app/context";
import { saveSettings } from "../app/settings";
import { Banner, Button, Dialog, Field, Section, Segmented, Select, Toggle } from "../components/controls";
import { LANGUAGES, t, tm } from "../i18n";
import type { UpdatePhase } from "../ipc/transport";
import { IpcError, type Msg } from "../ipc/types";
import { LINKS } from "../lib/links";

export function Settings() {
  const app = useApp();
  const { core } = app;
  const snap = () => core.state.snap!;
  const s = () => snap().settings;
  const set = (patch: Parameters<typeof saveSettings>[1]) => void saveSettings(app, patch);
  const [quitting, setQuitting] = createSignal(false);
  const update = () => snap().update;
  // The in-app update (installed copies); otherwise the release page
  const [phase, setPhase] = createSignal<UpdatePhase>({ state: "unavailable", reason: "portable" });
  onMount(() => {
    core.transport.onUpdatePhase(setPhase);
    void core.transport.updateStatus().then(setPhase, () => {});
  });
  const canInstall = () => phase().state === "idle" || phase().state === "failed";
  const installUpdate = (manifestUrl: string) => app.run(() => core.transport.updateInstall(manifestUrl));
  const mb = (bytes: number) => (bytes / 1048576).toFixed(1);

  const checkNow = async () => {
    try {
      const u = await core.request("app.check_update");
      if (!u.available) app.notify(t("update.uptodate", u.current_version));
    } catch (e) {
      // A failed check stays in the section (UpdateState.error) with the
      // link to the releases page; other failures (no core) are a notice
      if (IpcError.from(e).code !== "update.failed") app.notify(errorText(e), "error");
    }
  };

  return (
    <div class="page settings">
      <header class="page-head">
        <h1>{t("settings.header")}</h1>
      </header>

      <Section title={t("ui.settings.appearance")}>
        <Field label={t("settings.language")}>
          {(id) => (
            <Select
              id={id}
              value={LANGUAGES.some((l) => l.code === s().language) ? s().language : "en"}
              options={LANGUAGES.map((l) => ({ value: l.code, label: l.name }))}
              onChange={(v) => set({ language: v })}
            />
          )}
        </Field>
        <div class="field">
          <span class="field-label" id="theme-label">
            {t("settings.theme")}
          </span>
          <Segmented
            label={t("settings.theme")}
            value={s().theme}
            options={[
              { value: "system", label: t("settings.theme_system") },
              { value: "light", label: t("settings.theme_light") },
              { value: "dark", label: t("settings.theme_dark") },
            ]}
            onChange={(v) => set({ theme: v })}
          />
        </div>
      </Section>

      <Section title={t("ui.settings.startup")}>
        <Toggle label={t("settings.start_with_windows")} checked={s().start_with_windows} onChange={(v) => set({ start_with_windows: v })} />
        {/* At logon: on starts A2DPWB with --background (only the tray
            icon), off opens the window; it applies only with Windows */}
        <Toggle
          label={t("ui.settings.start_minimized")}
          hint={t("ui.settings.start_minimized_logon")}
          checked={s().start_minimized}
          disabled={!s().start_with_windows}
          onChange={(v) => set({ start_minimized: v })}
        />
        <Toggle label={t("settings.minimize_to_tray")} checked={s().minimize_to_tray} onChange={(v) => set({ minimize_to_tray: v })} />
        <Toggle
          label={t("ui.settings.tray_click_opens_window")}
          hint={t("ui.settings.tray_click_opens_window_hint")}
          checked={s().tray_click_opens_window}
          onChange={(v) => set({ tray_click_opens_window: v })}
        />
      </Section>

      <Section title={t("ui.settings.notifications")}>
        <Toggle
          label={t("ui.settings.notify_connected")}
          checked={s().notify_connected}
          onChange={(v) => set({ notify_connected: v })}
        />
        <div class="row">
          <span class="row-text">
            <span class="row-label">{t("ui.settings.battery_low")}</span>
            <span id="battery-low-help" class="hint">
              {t("ui.settings.battery_low_hint")}
            </span>
          </span>
          <Select<number>
            value={s().battery_low_percent}
            label={t("ui.settings.battery_low")}
            describedBy="battery-low-help"
            options={[
              ...[20, 10, 5].map((p) => ({ value: p, label: t("ui.settings.battery_low_below", p) })),
              { value: 0, label: t("ui.settings.battery_low_never") },
            ]}
            onChange={(v) => set({ battery_low_percent: v })}
          />
        </div>
      </Section>

      <Section
        title={t("ui.settings.updates")}
        actions={
          <Button icon="arrow_sync" busy={update().checking} onClick={checkNow}>
            {t("update.check")}
          </Button>
        }
      >
        <Show when={!update().checking && update().error}>
          {(err) => <UpdateCheckFailed message={err()} />}
        </Show>
        <Show when={update().available}>
          {(u) => (
            <Banner
              kind="info"
              title={t("update.available_title")}
              actions={
                <>
                  <Show when={canInstall()}>
                    <Button kind="strong" icon="arrow_sync" onClick={() => installUpdate(u().manifest_url)}>
                      {t("ui.update.install")}
                    </Button>
                  </Show>
                  <Button
                    kind={phase().state === "unavailable" ? "strong" : undefined}
                    icon="open"
                    onClick={() => core.transport.openUrl(u().url)}
                  >
                    {t("ui.settings.open_download")}
                  </Button>
                </>
              }
            >
              {t("ui.settings.update_available", u().version, update().current_version)}
              <Switch>
                <Match when={phase().state === "unavailable" && phase()}>
                  {(p) => {
                    const reason = (p() as Extract<UpdatePhase, { state: "unavailable" }>).reason;
                    return <div class="hint">{t(reason === "unsigned" ? "ui.update.unsigned" : "ui.update.portable")}</div>;
                  }}
                </Match>
                <Match when={phase().state === "downloading" && phase()}>
                  {(p) => {
                    const d = p() as Extract<UpdatePhase, { state: "downloading" }>;
                    const progress = d.total ? `${mb(d.downloaded)} / ${mb(d.total)} MB` : `${mb(d.downloaded)} MB`;
                    return <div class="hint">{t("ui.update.downloading", d.version || u().version, progress)}</div>;
                  }}
                </Match>
                <Match when={phase().state === "waiting" && phase()}>
                  {(p) => <div class="hint">{t("ui.update.waiting", (p() as { version: string }).version)}</div>}
                </Match>
                <Match when={phase().state === "installing" && phase()}>
                  {(p) => <div class="hint">{t("ui.update.installing", (p() as { version: string }).version)}</div>}
                </Match>
                <Match when={phase().state === "failed" && phase()}>
                  {(p) => <div class="hint">{t("ui.update.failed", (p() as { code: string }).code)}</div>}
                </Match>
              </Switch>
            </Banner>
          )}
        </Show>
        <Toggle
          label={t("settings.check_updates_on_startup")}
          checked={s().check_updates_on_startup}
          onChange={(v) => set({ check_updates_on_startup: v })}
        />
        <Toggle
          label={t("settings.include_prereleases")}
          checked={s().include_prereleases}
          onChange={(v) => set({ include_prereleases: v })}
        />
      </Section>

      <Section title={t("ui.settings.advanced")}>
        <Toggle
          label={t("ui.settings.developer_mode")}
          hint={t("ui.settings.developer_mode_hint")}
          checked={s().developer_mode}
          onChange={(v) => set({ developer_mode: v })}
        />
        <div class="row">
          <span class="row-text">
            <span class="row-label">{t("ui.settings.config_folder")}</span>
            <span class="hint mono">{snap().config_dir}</span>
          </span>
          <Button icon="folder_open" onClick={() => core.transport.openFolder(snap().config_dir)}>
            {t("ui.settings.open_config_folder")}
          </Button>
        </div>
        <div class="row">
          <span class="row-text">
            <span class="row-label">{t("ui.setup.title")}</span>
          </span>
          <Button icon="play" onClick={() => app.setSetupActive(true)}>
            {t("ui.settings.run_setup")}
          </Button>
        </div>
      </Section>

      <div class="danger-zone">
        <Button kind="danger" icon="power" onClick={() => setQuitting(true)}>
          {t("ui.settings.quit")}
        </Button>
      </div>
      <Dialog
        open={quitting()}
        title={t("ui.settings.quit")}
        confirm={t("ui.settings.quit")}
        cancel={t("modal.cancel")}
        danger
        onConfirm={() => {
          setQuitting(false);
          void app.run(() => core.request("app.quit"));
        }}
        onCancel={() => setQuitting(false)}
      >
        <p>{t("ui.settings.quit_confirm")}</p>
      </Dialog>
    </div>
  );
}

/** A failed update check (update.error) and the way out: the releases page */
export function UpdateCheckFailed(props: { message: Msg }) {
  const { core } = useApp();
  return (
    <Banner
      kind="error"
      title={t("update.check")}
      actions={
        <Button icon="open" onClick={() => void core.transport.openUrl(LINKS.releases)}>
          {t("ui.update.open_releases")}
        </Button>
      }
    >
      {tm(props.message) || t("update.error")}
    </Banner>
  );
}
