// What the main window shows above or beside every screen: a newer
// release (the update check's result, not only on Settings), a crash of
// A2DPWB itself and a running HCI capture (v1 put " - [HCI capture
// running]" in the title bar).

import { createEffect, createSignal, For, Show } from "solid-js";
import { Button, Dialog, IconButton } from "../components/controls";
import { Icon } from "../components/Icon";
import { copyReport, openReport } from "../components/stream";
import { t } from "../i18n";
import type { UpdatePhase } from "../ipc/transport";
import type { UpdateInfo } from "../ipc/bindings/UpdateInfo";
import type { CrashReport } from "../ipc/types";
import { useApp } from "./context";

/** "HCI capture running" from v1's title suffix (" - [HCI capture running]") */
export function captureRunningText(): string {
  return t("debug.capture_title_suffix")
    .replace(/^\s*-\s*/, "")
    .replace(/^\[(.*)\]$/, "$1")
    .trim();
}

/** A small red sign while an HCI capture runs; it leads to Diagnostics */
export function CaptureSign(props: { onOpen?: () => void }) {
  const { core, navigate } = useApp();
  const capture = () => core.state.snap?.capture ?? null;
  return (
    <Show when={capture()}>
      {(c) => (
        <button
          type="button"
          class="capture-sign"
          title={`${captureRunningText()}\n${c().path}`}
          aria-label={captureRunningText()}
          onClick={() => (props.onOpen ? props.onOpen() : navigate({ name: "diagnostics" }))}
        >
          <span class="dot rec" aria-hidden="true" />
          <span class="capture-sign-text ellipsis">{captureRunningText()}</span>
        </button>
      )}
    </Show>
  );
}

/** The head of a crash's report: what stopped, when, why */
export function crashHead(c: CrashReport): string {
  return `${t("ui.crash.title")}: ${c.process}, ${c.time}\n${c.reason}`;
}

/** A crash report of A2DPWB's own processes (a panic, a crash of the core
 *  or the window), until it is dismissed: its cause and backtrace go into
 *  the report with the diagnostic info */
export function CrashNotice() {
  const app = useApp();
  const crash = () => app.core.state.snap?.crashes?.[0] ?? null;
  const more = () => Math.max(0, (app.core.state.snap?.crashes?.length ?? 0) - 1);
  const dismiss = () => void app.core.request("diag.crashes_seen").catch(() => undefined);
  return (
    <Show when={crash()}>
      {(c) => (
        <div class="banner banner-error app-crash" role="alert">
          <Icon name="error_circle" class="banner-icon" />
          <div class="banner-body">
            <strong class="banner-title">{t("ui.crash.title")}</strong>
            <div class="banner-text">
              {t("ui.crash.when", c().process, c().time)}
              <Show when={more() > 0}> {t("ui.crash.more", more())}</Show>
            </div>
            <Show when={c().reason}>
              <div class="banner-text crash-reason ellipsis" title={c().reason}>
                {c().reason}
              </div>
            </Show>
            <div class="banner-actions">
              <Button kind="strong" icon="open" onClick={() => void openReport(app, crashHead(c()), c().text)}>
                {t("ui.diag.report_github")}
              </Button>
              <Button icon="copy" onClick={() => void copyReport(app, crashHead(c()), c().text)}>
                {t("ui.error.copy")}
              </Button>
            </div>
          </div>
          <IconButton icon="dismiss" label={t("modal.close")} onClick={dismiss} />
        </div>
      )}
    </Show>
  );
}

/** Kept per viewer: the version whose update dialog was answered "Later" */
const UPDATE_LATER_KEY = "a2dpwb.update_later";

/** A release's in-app notes as lines to show: list marks become bullets,
 *  Markdown emphasis and links plain text */
export function noteLines(notes: string): { text: string; bullet: boolean }[] {
  return notes
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l !== "")
    .map((l) => {
      const bullet = /^[-*•]\s+/.test(l);
      const text = l
        .replace(/^[-*•]\s+/, "")
        .replace(/^#+\s*/, "")
        .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1")
        .replace(/\*\*([^*]*)\*\*/g, "$1")
        .replace(/`([^`]*)`/g, "$1");
      return { text, bullet };
    });
}

/** A newer release (issue #86): a dialog with its short notes and Update
 *  now (Open download page for a portable copy), once per version; Later
 *  leaves it to Settings, which has the details */
export function UpdateNotice() {
  const app = useApp();
  const { core, route, navigate } = app;
  const read = () => {
    try {
      return localStorage.getItem(UPDATE_LATER_KEY);
    } catch {
      return null;
    }
  };
  const [later, setLater] = createSignal<string | null>(read());
  const [phase, setPhase] = createSignal<UpdatePhase | null>(null);
  const available = () => {
    const u = core.state.snap?.update.available ?? null;
    if (!u || u.version === later() || route().name === "settings" || app.setupActive()) return null;
    return u;
  };
  // Whether this copy updates itself, asked when a release shows up
  createEffect(() => {
    if (available()) void core.transport.updateStatus().then(setPhase, () => setPhase(null));
  });
  const canInstall = () => phase()?.state === "idle" || phase()?.state === "failed";
  const close = (version: string) => {
    setLater(version);
    try {
      localStorage.setItem(UPDATE_LATER_KEY, version);
    } catch {
      // Not kept: it shows again at the next start
    }
  };
  const confirm = (u: UpdateInfo) => {
    close(u.version);
    if (canInstall()) {
      // Settings shows the download and the install
      navigate({ name: "settings" });
      void app.run(() => core.transport.updateInstall(u.manifest_url));
    } else {
      void core.transport.openUrl(u.url);
    }
  };
  return (
    <Show when={available()}>
      {(u) => (
        <Dialog
          open
          title={t("update.available_title")}
          confirm={t(canInstall() ? "ui.update.install" : "ui.settings.open_download")}
          cancel={t("ui.update.later")}
          onConfirm={() => confirm(u())}
          onCancel={() => close(u().version)}
        >
          <p>{t("ui.settings.update_available", u().version, core.state.snap!.update.current_version)}</p>
          <Show when={u().notes.trim()}>
            <ul class="update-notes">
              <For each={noteLines(u().notes)}>
                {(l) => <li classList={{ plain: !l.bullet }}>{l.text}</li>}
              </For>
            </ul>
          </Show>
          <p>
            <a
              href={u().url}
              onClick={(e) => {
                e.preventDefault();
                void core.transport.openUrl(u().url);
              }}
            >
              {t("ui.update.release_notes")}
            </a>
          </p>
        </Dialog>
      )}
    </Show>
  );
}
