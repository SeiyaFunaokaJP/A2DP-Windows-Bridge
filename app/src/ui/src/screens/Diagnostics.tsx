// Diagnostics: live graphs of the stream, the radio and the audio capture
// (an LE Audio stream's ISO counters on the stream tab), the eleven-step
// connection flow, the log with level, text and attempt filters, HCI
// capture, "copy diagnostic info", verbose logging, the diagnostic bundle
// (one .zip of logs, captures and crash reports for a report). The receiver
// test is on the Developer screen. Each tab is its own file in diagnostics/.

import { createEffect, createSignal, For, on, onCleanup, Show } from "solid-js";
import { errorText, useApp } from "../app/context";
import { Banner, Button, Check, Dialog, IconButton, Section, Select } from "../components/controls";
import { Icon } from "../components/Icon";
import { bugReportFor } from "../components/stream";
import type { IconName } from "../components/icons";
import { t } from "../i18n";
import type { CaptureState, DiagBundle, LogLevel } from "../ipc/types";
import { copyText, knowAdapters } from "../lib/format";
import { baseName } from "../lib/links";
import { formatBytes } from "../lib/linkstats";
import { CapturePanel } from "./diagnostics/CapturePanel";
import { FlowPanel } from "./diagnostics/FlowPanel";
import { createLogView, JUMP_WAIT_MS, LogPanel } from "./diagnostics/LogPanel";
import { RadioPanel } from "./diagnostics/RadioPanel";
import { mask, observeCoreTime, setCoreClock, setEpoch, setMask } from "./diagnostics/shared";
import { StreamPanel } from "./diagnostics/StreamPanel";

type Tab = "stream" | "radio" | "capture" | "flow" | "log";

// ---- Window-wide view state (kept while the window lives) ------------------

/** The capture stopped last (file size, "show in Explorer") */
const [savedCapture, setSavedCapture] = createSignal<CaptureState | null>(null);
/** The diagnostic bundle saved last */
const [savedBundle, setSavedBundle] = createSignal<DiagBundle | null>(null);

/** The bundle's HCI captures: off unless asked for (they hold Bluetooth
 *  addresses and device names, and most reports do not need them) */
const [withHci, setWithHci] = createSignal(false);
/** GitHub takes issue attachments up to this size */
const GITHUB_ATTACHMENT_LIMIT = 25 * 1024 * 1024;

type VerboseChoice = "off" | "debug" | "trace";

/** When verbose logging ends, as the local date and time */
function untilText(ms: number): string {
  return new Date(ms).toLocaleString(undefined, { month: "numeric", day: "numeric", hour: "2-digit", minute: "2-digit" });
}

export function Diagnostics() {
  const app = useApp();
  const { core } = app;
  const [tab, setTab] = createSignal<Tab>("stream");
  // The adapters' own addresses: <adapter> in masked copies
  createEffect(() => knowAdapters(core.state.snap?.adapter.adapters.map((a) => a.address) ?? []));
  const capture = () => core.state.snap!.capture;
  const [capBusy, setCapBusy] = createSignal(false);
  const [bundleBusy, setBundleBusy] = createSignal(false);
  /** The bundle's options are open */
  const [asking, setAsking] = createSignal(false);
  const [reporting, setReporting] = createSignal(false);
  const verbose = () => core.state.snap!.verbose ?? null;
  const logView = createLogView();
  // A new core process sends a new snapshot, with its own clock
  createEffect(() => setCoreClock(core.state.snap?.clock_ms ?? null));

  // Live samples while the screen is open: the clock for the time of day,
  // and the link history; info lines so a step's line is there to jump to
  onCleanup(core.wantLink());
  onCleanup(core.wantLogs("info"));
  createEffect(
    on(
      core.link,
      (s) => {
        const last = s[s.length - 1];
        if (last) observeCoreTime(last.time_ms);
      },
      { defer: true },
    ),
  );
  createEffect(
    on(
      () => core.state.conn.state,
      (s) => {
        if (s !== "connected") setEpoch(null);
      },
      { defer: true },
    ),
  );

  const toggleCapture = async () => {
    setCapBusy(true);
    try {
      const running = capture();
      if (running) {
        const done = (await core.request("diag.capture_stop")) ?? running;
        setSavedCapture(done);
        app.notify(t("ui.diag.capture_saved", `${done.path} (${formatBytes(done.bytes)})`));
      } else {
        setSavedCapture(null);
        await core.request("diag.capture_start");
      }
    } catch (e) {
      app.notify(t("debug.capture_failed", errorText(e)), "error");
    } finally {
      setCapBusy(false);
    }
  };

  const copyInfo = async () => {
    try {
      const info = await core.request("diag.info", { mask: mask() });
      app.notify((await copyText(info.text)) ? t("ui.diag.copied") : t("dbgc.copy_failed"), "info");
    } catch (e) {
      app.notify(errorText(e), "error");
    }
  };

  const changeVerbose = async (v: VerboseChoice) => {
    try {
      await core.request("diag.set_verbose", { level: v === "off" ? null : (v as LogLevel) });
    } catch (e) {
      app.notify(errorText(e), "error");
    }
  };

  const saveBundle = async () => {
    setBundleBusy(true);
    try {
      const b = await core.request("diag.bundle", { mask: mask(), hci: withHci() });
      setSavedBundle(b);
      setAsking(false);
    } catch (e) {
      app.notify(t("ui.diag.bundle_failed", errorText(e)), "error");
    } finally {
      setBundleBusy(false);
    }
  };

  /** A prefilled GitHub issue in the browser, the bundle shown in Explorer
   *  to drag into it, the diagnostic info on the clipboard (a report is
   *  public: addresses masked) */
  const report = async (b: DiagBundle) => {
    setReporting(true);
    try {
      const info = (await core.request("diag.info", { mask: true })).text;
      const url = bugReportFor(core.state.snap!, info, { bundle: baseName(b.path) });
      await core.transport.revealPath(b.path);
      await core.transport.openUrl(url);
      // Only a convenience: the issue has the text already (when it fit)
      await copyText(info).catch(() => false);
      app.notify(t("ui.diag.report_opened"));
    } catch (e) {
      app.notify(errorText(e), "error");
    } finally {
      setReporting(false);
    }
  };

  const jumpTo = (seq: number) => {
    const j = { seq };
    logView.setJump(j);
    setTab("log");
    setTimeout(() => {
      if (logView.jump() !== j) return;
      logView.setJump(null);
      app.notify(t("ui.diag.line_gone"), "error");
    }, JUMP_WAIT_MS);
  };

  const tabs = (): { id: Tab; label: string; icon: IconName }[] => [
    { id: "stream", label: t("quality.section_stream"), icon: "pulse" },
    { id: "radio", label: t("ui.diag.tab_radio"), icon: "bluetooth" },
    { id: "capture", label: t("ui.diag.tab_capture"), icon: "speaker_2" },
    { id: "flow", label: t("dbgc.section_flow"), icon: "checkmark_circle" },
    { id: "log", label: t("dbgc.section_log"), icon: "people_list" },
  ];

  const onTabKey = (e: KeyboardEvent) => {
    // Arrow keys move between tabs (WAI-ARIA tabs pattern)
    const list = tabs();
    const i = list.findIndex((x) => x.id === tab());
    let n = -1;
    if (e.key === "ArrowRight") n = (i + 1) % list.length;
    else if (e.key === "ArrowLeft") n = (i - 1 + list.length) % list.length;
    else if (e.key === "Home") n = 0;
    else if (e.key === "End") n = list.length - 1;
    if (n < 0) return;
    e.preventDefault();
    setTab(list[n].id);
    document.getElementById(`tab-${list[n].id}`)?.focus();
  };

  return (
    <div class="page diagnostics">
      <header class="page-head">
        <h1>{t("ui.nav.diagnostics")}</h1>
        <div class="head-actions">
          <Check label={t("dbgc.mask")} title={t("dbgc.mask_tip")} checked={mask()} onChange={setMask} />
          <Button icon={capture() ? "stop" : "record"} busy={capBusy()} onClick={toggleCapture} classList={{ recording: !!capture() }}>
            {capture() ? t("debug.capture_stop") : t("debug.capture_start")}
          </Button>
          <Button icon="copy" onClick={copyInfo}>
            {t("dbgc.copy_diag")}
          </Button>
        </div>
      </header>
      <Section title={t("ui.diag.report")} class="report-card">
        <div class="row">
          <span class="row-text">
            <span class="row-label">{t("ui.diag.verbose")}</span>
            <span id="verbose-help" class="hint">
              {verbose() ? t("ui.diag.verbose_until", untilText(verbose()!.until_ms)) : t("ui.diag.verbose_help")}
            </span>
          </span>
          <Select<VerboseChoice>
            value={verbose()?.level === "trace" ? "trace" : verbose() ? "debug" : "off"}
            label={t("ui.diag.verbose")}
            describedBy="verbose-help"
            options={[
              { value: "off", label: t("ui.diag.verbose_off") },
              { value: "debug", label: t("ui.diag.verbose_debug") },
              { value: "trace", label: t("ui.diag.verbose_trace") },
            ]}
            onChange={(v) => void changeVerbose(v)}
          />
        </div>
        <div class="row">
          <span class="row-text">
            <span class="row-label">{t("ui.diag.bundle_title")}</span>
            <span class="hint">{t("ui.diag.bundle_tip")}</span>
          </span>
          <Button icon="arrow_download" onClick={() => setAsking(true)}>
            {t("ui.diag.bundle_button")}
          </Button>
        </div>
        <Show when={savedBundle()}>
          {(b) => (
            <div class="bundle-saved">
              <Banner
                kind={b().bytes > GITHUB_ATTACHMENT_LIMIT ? "warn" : "good"}
                title={t("ui.diag.bundle_saved_title")}
                actions={
                  <>
                    <Button icon="open" kind="strong" busy={reporting()} onClick={() => void report(b())}>
                      {t("ui.diag.report_github")}
                    </Button>
                    <Button icon="folder_open" kind="subtle" onClick={() => void core.transport.revealPath(b().path)}>
                      {t("ui.diag.show_in_explorer")}
                    </Button>
                    <Button icon="dismiss" kind="subtle" onClick={() => setSavedBundle(null)}>
                      {t("modal.close")}
                    </Button>
                  </>
                }
              >
                <span>
                  <span class="mono bundle-name">{baseName(b().path)}</span>
                  <span class="num bundle-size"> · {formatBytes(b().bytes)}</span>
                </span>
                <span>
                  {b().bytes > GITHUB_ATTACHMENT_LIMIT ? t("ui.diag.bundle_too_big") : t("ui.diag.bundle_attach")}
                </span>
              </Banner>
            </div>
          )}
        </Show>
      </Section>
      <Dialog
        open={asking()}
        title={t("ui.diag.bundle")}
        confirm={t("ui.diag.bundle_save")}
        cancel={t("modal.cancel")}
        busy={bundleBusy()}
        onConfirm={() => void saveBundle()}
        onCancel={() => setAsking(false)}
      >
        <p>{t("ui.diag.bundle_dialog_text")}</p>
        <div class="bundle-options">
          <Check label={t("ui.diag.bundle_hci")} checked={withHci()} onChange={setWithHci} />
          <span class="hint">{t("ui.diag.bundle_hci_hint")}</span>
          <Check label={t("dbgc.mask")} checked={mask()} onChange={setMask} />
          <span class="hint">{t("ui.diag.bundle_mask_hint")}</span>
        </div>
      </Dialog>
      <Show when={capture()}>
        {(c) => (
          <p class="capture-line" role="status">
            <span class="dot rec" aria-hidden="true" />
            {t("ui.diag.capturing")} <span class="mono capture-path">{c().path}</span>
            <span class="num capture-size">{formatBytes(c().bytes)}</span>
          </p>
        )}
      </Show>
      <Show when={!capture() && savedCapture()}>
        {(c) => (
          <div class="capture-line capture-saved" role="status">
            <Icon name="checkmark_circle" />
            <span class="capture-saved-text">
              {t("ui.diag.capture_saved", "")}
              <span class="mono capture-path">{c().path}</span> <span class="num capture-size">({formatBytes(c().bytes)})</span>
            </span>
            <Button icon="folder_open" kind="subtle" onClick={() => void core.transport.revealPath(c().path)}>
              {t("ui.diag.show_in_explorer")}
            </Button>
            <IconButton icon="dismiss" label={t("modal.close")} onClick={() => setSavedCapture(null)} />
          </div>
        )}
      </Show>
      <div class="tabs" role="tablist" aria-label={t("ui.nav.diagnostics")} onKeyDown={onTabKey}>
        <For each={tabs()}>
          {(x) => (
            <button
              type="button"
              role="tab"
              id={`tab-${x.id}`}
              class="tab"
              aria-selected={tab() === x.id}
              aria-controls={`panel-${x.id}`}
              tabindex={tab() === x.id ? 0 : -1}
              onClick={() => setTab(x.id)}
            >
              <Icon name={x.icon} />
              {x.label}
            </button>
          )}
        </For>
      </div>
      <div class="tab-panel" role="tabpanel" id={`panel-${tab()}`} aria-labelledby={`tab-${tab()}`}>
        <Show when={tab() === "stream"}>
          <StreamPanel />
        </Show>
        <Show when={tab() === "radio"}>
          <RadioPanel />
        </Show>
        <Show when={tab() === "capture"}>
          <CapturePanel />
        </Show>
        <Show when={tab() === "flow"}>
          <FlowPanel onJump={jumpTo} />
        </Show>
        <Show when={tab() === "log"}>
          <LogPanel view={logView} />
        </Show>
      </div>
    </div>
  );
}
