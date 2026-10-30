// Diagnostics: live link graphs and v1's link quality figures (an LE
// Audio stream's ISO counters), the eleven-step connection flow, the log
// with level, text and attempt filters, HCI capture, "copy diagnostic
// info", verbose logging, the diagnostic bundle (one .zip of logs,
// captures and crash reports for a report). The receiver test is on the
// Developer screen.

import { createEffect, createMemo, createSignal, For, on, onCleanup, Show, untrack, type JSX } from "solid-js";
import { errorText, useApp } from "../app/context";
import { Gauge, LineChart } from "../components/charts";
import { CodecDetailsCard } from "../components/CodecDetails";
import { Banner, Button, Check, Dialog, IconButton, Section, Select } from "../components/controls";
import { Icon } from "../components/Icon";
import { bugReportFor } from "../components/stream";
import type { IconName } from "../components/icons";
import { t } from "../i18n";
import type { CaptureState, DiagBundle, FlowStepView, LogLevel, LogLine } from "../ipc/types";
import { bitrateNoteKey, codecLabelKey } from "../lib/codecs";
import { STEPS, type Step, type StepStatus, stepDetail } from "../lib/flow";
import { clock, copyText, kbps, knowAdapters, maskIdentifiers } from "../lib/format";
import {
  dropsPerSample,
  EDR3_MAX_KBPS,
  formatBytes,
  gaugeValue,
  lossStats,
  takeBaseline,
  type LinkBaseline,
} from "../lib/linkstats";
import {
  coreTimeOfDay,
  drawnRange,
  filterLog,
  formatLogLine,
  keysBetween,
  logKey,
  nextEpoch,
  timeOfDay,
  type LevelFilter,
} from "../lib/logview";

type Tab = "link" | "flow" | "log";

// ---- Window-wide view state (kept while the window lives, so it is still
// there when the user comes back to the screen) ----------------------------

/** Mask Bluetooth addresses in the log and in every copy */
const [mask, setMask] = createSignal(true);
/** The link counters when Reset was pressed */
const [resetBase, setResetBase] = createSignal<LinkBaseline | null>(null);
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

/** "a.zip" of "C:\x\a.zip" */
function baseName(path: string): string {
  return path.slice(Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/")) + 1);
}

/** When verbose logging ends, as the local date and time */
function untilText(ms: number): string {
  return new Date(ms).toLocaleString(undefined, { month: "numeric", day: "numeric", hour: "2-digit", minute: "2-digit" });
}
/** The core's local time of day at its time 0 (Snapshot.clock_ms) */
const [coreClock, setCoreClock] = createSignal<number | null>(null);
/** Without it: the wall clock (ms since 1970) at the core's time 0, estimated from live samples */
const [epoch, setEpoch] = createSignal<number | null>(null);
let lastCoreTime = -1;

function observeCoreTime(timeMs: number) {
  // The core's clock went back: a new core process
  if (timeMs + 1000 < lastCoreTime) setEpoch(null);
  lastCoreTime = timeMs;
  setEpoch((e) => nextEpoch(e, timeMs, Date.now()));
}

/** A core time as the time of day (v1): from the core's clock, else from
 *  the estimate, else as the core's uptime until the clock is known */
function timeText(ms: number): string {
  const c = coreClock();
  if (c !== null) return coreTimeOfDay(ms, c);
  const e = epoch();
  return e === null ? clock(ms) : timeOfDay(ms, e);
}

/** The log view's filters and selection (they outlive the log tab) */
function createLogView() {
  const [level, setLevel] = createSignal<LevelFilter>("info");
  const [query, setQuery] = createSignal("");
  const [attemptOnly, setAttemptOnly] = createSignal(false);
  const [follow, setFollow] = createSignal(true);
  const [selected, setSelected] = createSignal<ReadonlySet<string>>(new Set());
  const [anchor, setAnchor] = createSignal<string | null>(null);
  /** The line jumped to from the flow (highlighted) */
  const [focusKey, setFocusKey] = createSignal<string | null>(null);
  /** A jump from the flow waiting for its line (LogLine.seq) */
  const [jump, setJump] = createSignal<{ seq: number } | null>(null);
  return {
    level,
    setLevel,
    query,
    setQuery,
    attemptOnly,
    setAttemptOnly,
    follow,
    setFollow,
    selected,
    setSelected,
    anchor,
    setAnchor,
    focusKey,
    setFocusKey,
    jump,
    setJump,
  };
}
type LogView = ReturnType<typeof createLogView>;

/** How long a jump waits for its line (the debug lines come after a resubscription) */
const JUMP_WAIT_MS = 3000;

export function Diagnostics() {
  const app = useApp();
  const { core } = app;
  const [tab, setTab] = createSignal<Tab>("link");
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
    { id: "link", label: t("quality.title"), icon: "pulse" },
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
        <Show when={tab() === "link"}>
          <LinkPanel />
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

// ---- Link ------------------------------------------------------------------

function Row(props: { label: string; value: string; class?: string; children?: JSX.Element }) {
  return (
    <div class="lq-row">
      <dt>{props.label}</dt>
      <dd class={`num ${props.class ?? ""}`}>
        <span>{props.value}</span>
        {props.children}
      </dd>
    </div>
  );
}

const DASH = "–";

function LinkPanel() {
  const { core } = useApp();
  onCleanup(core.wantLink());
  const samples = () => core.link();
  const stream = () => core.state.snap!.stream;
  const streaming = () => stream().state === "streaming";
  const last = () => samples()[samples().length - 1];
  const drops = createMemo(() => dropsPerSample(samples()));
  // Capture drops in ms after the stream stopped: the last known rate
  const rate = createMemo(() => {
    const s = samples();
    for (let i = s.length - 1; i >= 0; i--) if (s[i].source_rate) return s[i].source_rate!;
    return stream().info?.source_sample_rate ?? 0;
  });
  const loss = createMemo(() => {
    const s = last();
    return s ? lossStats(s, resetBase(), rate()) : null;
  });

  const codec = () => {
    const i = stream().info;
    if (!streaming() || !i) return DASH;
    const key = codecLabelKey(i.codec);
    const note = bitrateNoteKey(i);
    return t("quality.codec_value", key ? t(key) : i.codec, i.sample_rate / 1000, i.channels, i.bitrate_kbps) + (note ? t(note) : "");
  };
  /** The last sample while packets go out (the "last second" rows) */
  const sending = () => {
    const s = last();
    return streaming() && s && s.packets_per_second > 0 ? s : null;
  };
  const live = () => (streaming() ? (last() ?? null) : null);
  const queueClass = () => {
    const s = live();
    if (!s) return "";
    const f = s.queue_depth / (s.queue_capacity || 64);
    return f < 0.25 ? "st-good" : f < 0.5 ? "st-warn" : "st-bad";
  };
  const queueText = () => {
    const s = live();
    if (!s) return DASH;
    return s.queue_capacity ? t("quality.queue_value", s.queue_depth, s.queue_capacity) : String(s.queue_depth);
  };
  const packetsText = () => {
    const s = sending();
    if (!s) return DASH;
    return s.avg_packet_bytes != null
      ? t("quality.packets_value", s.packets_per_second, s.avg_packet_bytes)
      : `${s.packets_per_second.toFixed(1)} /s`;
  };
  const rssiText = () => {
    const s = live();
    return !s || s.rssi === null ? DASH : `${s.rssi > 0 ? "+" : ""}${s.rssi} dB`;
  };
  const afhText = () => {
    const s = live();
    if (!s) return DASH;
    const parts: string[] = [];
    if (s.afh_in_use !== null) parts.push(t("radio.afh", s.afh_in_use));
    if (s.afh_usable !== null) parts.push(t("ui.diag.afh_usable", s.afh_usable));
    if (parts.length === 0) return DASH;
    return parts.join(t("radio.separator")) + (s.afh_usable !== null && s.afh_avoided ? t("radio.afh_avoid", s.afh_avoided) : "");
  };
  const theoryPct = () => {
    const s = sending();
    return s && s.bandwidth_share != null ? s.bandwidth_share * 100 : null;
  };
  const airPct = () => {
    const s = sending();
    return s ? s.airtime * 100 : null;
  };
  const airHigh = () => (airPct() ?? 0) >= 80;
  // LE Audio: the stream that was set up, and its isochronous channels
  const le = () => (streaming() ? (stream().info?.le ?? null) : null);
  const iso = () => last()?.iso ?? null;
  const isoCount = (pick: (i: NonNullable<ReturnType<typeof iso>>) => number) => {
    const i = iso();
    return i ? String(pick(i)) : DASH;
  };
  const airText = () => {
    const s = sending();
    const p = airPct();
    if (!s || p === null) return DASH;
    return s.acl_packet ? t("quality.airtime_value", p, s.acl_packet.name, s.acl_packet.slots) : `${p.toFixed(1)} %`;
  };

  return (
    <div>
      <Show when={!streaming()}>
        <p class="muted">{t("quality.not_streaming")}</p>
      </Show>
      <div class="chart-grid">
        <LineChart
          title={t("ui.diag.rssi")}
          values={samples().map((s) => s.rssi)}
          min={-40}
          max={10}
          format={(v) => `${v > 0 ? "+" : ""}${Math.round(v)} dB`}
        />
        <LineChart
          title={t("quality.bitrate")}
          values={samples().map((s) => kbps(s.bytes_per_second))}
          min={0}
          format={(v) => `${Math.round(v)} kbps`}
        />
        <LineChart
          title={t("quality.queue")}
          values={samples().map((s) => s.queue_depth)}
          min={0}
          format={(v) => `${Math.round(v)}`}
          status={(v) => (v < 8 ? "good" : v < 16 ? "warn" : "bad")}
        />
        <LineChart
          title={t("quality.dropped")}
          values={drops()}
          min={0}
          format={(v) => `${Math.round(v)}`}
          status={(v) => (v === 0 ? "good" : v < 3 ? "warn" : "bad")}
        />
      </div>

      <div class="lq-groups">
        <section class="card lq-group" aria-labelledby="lq-send">
          <h2 id="lq-stream">{t("quality.section_stream")}</h2>
          <dl class="lq-rows" aria-labelledby="lq-stream">
            <Row label={t("quality.codec")} value={codec()} />
          </dl>
          <h2 id="lq-send">{t("quality.section_send")}</h2>
          <dl class="lq-rows" aria-labelledby="lq-send">
            <Row label={t("quality.bitrate")} value={sending() ? t("quality.bitrate_value", kbps(sending()!.bytes_per_second)) : DASH} />
            <Row label={t("quality.packets")} value={packetsText()} />
            <Row label={t("quality.queue")} class={queueClass()} value={queueText()} />
            <Row label={t("ui.diag.rssi")} value={rssiText()} />
            <Row label={t("ui.diag.afh")} value={afhText()} />
          </dl>
        </section>

        {/* What the A2DP codec runs with and what the device offered */}
        <Show when={streaming() && !le() && stream().info}>
          {(i) => <Show when={i().details}>{(d) => <CodecDetailsCard info={i()} details={d()} />}</Show>}
        </Show>

        <Show when={le() || iso()}>
          <section class="card lq-group" aria-labelledby="lq-iso">
            <h2 id="lq-iso">{t("ui.diag.iso_title")}</h2>
            <dl class="lq-rows">
              <Row label={t("ui.diag.iso_config")} value={le()?.summary ?? DASH} />
              <Row label={t("ui.diag.iso_cis")} value={isoCount((i) => i.cis)} />
              <Row label={t("ui.diag.iso_sdu_sets")} value={isoCount((i) => i.sdu_sets)} />
              <Row label={t("ui.diag.iso_silence")} class={(iso()?.silence ?? 0) > 0 ? "st-warn" : ""} value={isoCount((i) => i.silence)} />
              <Row
                label={t("ui.diag.iso_underruns")}
                class={(iso()?.underruns ?? 0) > 0 ? "st-warn" : ""}
                value={isoCount((i) => i.underruns)}
              />
              <Row label={t("ui.diag.iso_dropped")} class={(iso()?.dropped ?? 0) > 0 ? "st-bad" : ""} value={isoCount((i) => i.dropped)} />
              <Row label={t("ui.diag.iso_prefill")} value={iso() ? t("ui.diag.iso_prefill_value", iso()!.prefill) : DASH} />
            </dl>
            <p class="hint">{t("ui.diag.iso_help")}</p>
          </section>
        </Show>

        {/* The BR/EDR link's band: not for LE Audio */}
        <Show when={!le()}>
          <section class="card lq-group" aria-labelledby="lq-band">
            <h2 id="lq-band">{t("quality.section_band")}</h2>
            <dl class="lq-rows">
              <Row label={t("quality.theory")} value={theoryPct() === null ? DASH : t("quality.theory_value", EDR3_MAX_KBPS, theoryPct()!)}>
                <Gauge value={theoryPct() === null ? null : gaugeValue(theoryPct())} label={t("quality.theory")} />
              </Row>
              <Row label={t("quality.airtime")} class={airHigh() ? "st-bad" : ""} value={airText()}>
                <Gauge
                  value={airPct() === null ? null : gaugeValue(airPct())}
                  label={t("quality.airtime")}
                  status={airHigh() ? "bad" : undefined}
                />
              </Row>
            </dl>
            <p class="hint">{t("quality.band_help")}</p>
          </section>
        </Show>

        <section class="card lq-group" aria-labelledby="lq-loss">
          <div class="lq-group-head">
            <h2 id="lq-loss">
              {loss()?.sinceReset ? t("ui.diag.loss_since_reset", timeText(resetBase()!.time_ms)) : t("ui.diag.loss_since_stream")}
            </h2>
            <Button icon="arrow_reset" kind="subtle" disabled={!last()?.counters} onClick={() => setResetBase(takeBaseline(last()))}>
              {t("quality.reset")}
            </Button>
          </div>
          <Show
            when={loss()}
            fallback={
              <dl class="lq-rows">
                <Row label={t("quality.dropped")} value={last() ? String(last()!.dropped) : DASH} />
                <Row label={t("quality.capture_dropped")} value={last() ? String(last()!.capture_dropped) : DASH} />
              </dl>
            }
          >
            {(l) => (
              <dl class="lq-rows">
                <Row label={t("quality.sent_total")} value={String(l().sent)} />
                <Row
                  label={t("quality.dropped")}
                  class={l().dropped > 0 ? "st-bad" : ""}
                  value={`${l().dropped} (${l().dropPct.toFixed(2)}%)`}
                />
                <Row
                  label={t("quality.capture_dropped")}
                  class={l().captureDropped > 0 ? "st-bad" : ""}
                  value={`${l().captureMs === null ? String(l().captureDropped) : `${l().captureMs!.toFixed(0)} ms`} (${l().capturePct.toFixed(2)}%)`}
                />
              </dl>
            )}
          </Show>
          <p class="hint">{t("quality.loss_help")}</p>
        </section>
      </div>
    </div>
  );
}

// ---- Flow ------------------------------------------------------------------

const STATUS_ICON: Record<StepStatus, IconName | null> = {
  pending: null,
  running: "arrow_sync",
  done: "checkmark_circle",
  warning: "warning",
  failed: "error_circle",
};

function FlowPanel(props: { onJump: (seq: number) => void }) {
  const app = useApp();
  onCleanup(app.core.wantFlow());
  // The core's flow; no session yet: none
  const session = () => {
    const f = app.core.flow();
    return f && f.session > 0 ? f : null;
  };
  const stepOf = (st: Step): FlowStepView | undefined => session()?.steps.find((x) => x.step === st);
  const statusText = (s: StepStatus) => t(`dbgc.status.${s}`);
  const elapsed = (ms: number | null | undefined) => (ms == null ? "" : `+${(ms / 1000).toFixed(3)} s`);
  /** Time of day of the line that set the step */
  const stepTime = (x: FlowStepView | undefined) => {
    if (!x) return "";
    const s = session();
    const ms = x.time_ms ?? (x.elapsed_ms != null && s ? s.start_ms + x.elapsed_ms : null);
    return ms == null ? "" : timeText(ms);
  };
  const detail = (st: Step) => {
    const x = stepOf(st);
    const d = x ? stepDetail(x) : "";
    return mask() ? maskIdentifiers(d) : d;
  };
  const header = () => {
    const s = session();
    if (!s) return t("dbgc.no_session");
    return (
      t("dbgc.session", s.session, timeText(s.start_ms)) + (s.stop_ms != null ? t("dbgc.session_stopped", timeText(s.stop_ms)) : "")
    );
  };
  const copyFlow = async () => {
    if (!session()) return;
    const lines = STEPS.map((st) => {
      const x = stepOf(st);
      const status = statusText((x?.status ?? "pending") as StepStatus);
      return [t(`dbgc.step.${st}`), status, stepTime(x), elapsed(x?.elapsed_ms), detail(st)].join("\t");
    });
    const text = [header(), ...lines].join("\n");
    const ok = await copyText(mask() ? maskIdentifiers(text) : text);
    app.notify(ok ? t("dbgc.copied", lines.length + 1) : t("dbgc.copy_failed"), ok ? "info" : "error");
  };
  const jump = (st: Step) => {
    const seq = stepOf(st)?.seq;
    if (seq != null) props.onJump(seq);
  };
  return (
    <div>
      <div class="panel-head">
        <span class="muted">{header()}</span>
        <Button icon="copy" kind="subtle" disabled={!session()} onClick={copyFlow}>
          {t("dbgc.copy_flow")}
        </Button>
      </div>
      <div class="flow">
        {/* Column headers (v1's list columns); screen readers get each cell's
            name with the cell instead */}
        <div class="flow-head" aria-hidden="true">
          <span />
          <span>{t("dbgc.col_step")}</span>
          <span>{t("dbgc.col_status")}</span>
          <span>{t("dbgc.col_time")}</span>
          <span>{t("dbgc.col_elapsed")}</span>
          <span>{t("dbgc.col_detail")}</span>
          <span />
        </div>
        <ol class="flow-list" aria-label={t("dbgc.section_flow")}>
          <For each={STEPS}>
            {(st) => {
              const status = () => (stepOf(st)?.status ?? "pending") as StepStatus;
              const seq = () => stepOf(st)?.seq ?? null;
              return (
                <li class={`flow-step st-${status()}`} classList={{ "has-line": seq() !== null }} onDblClick={() => jump(st)}>
                  <span class="flow-icon" aria-hidden="true">
                    <Show when={STATUS_ICON[status()]} fallback={<span class="flow-dot" />}>
                      {(i) => <Icon name={i()} />}
                    </Show>
                  </span>
                  <span class="flow-name">{t(`dbgc.step.${st}`)}</span>
                  <span class="flow-status">
                    <span class="sr-only">{t("dbgc.col_status")}: </span>
                    {statusText(status())}
                  </span>
                  <span class="flow-clock num">
                    <Show when={stepTime(stepOf(st))}>
                      <span class="sr-only">{t("dbgc.col_time")}: </span>
                    </Show>
                    {stepTime(stepOf(st))}
                  </span>
                  <span class="flow-time num">
                    <Show when={elapsed(stepOf(st)?.elapsed_ms)}>
                      <span class="sr-only">{t("dbgc.col_elapsed")}: </span>
                    </Show>
                    {elapsed(stepOf(st)?.elapsed_ms)}
                  </span>
                  <span class="flow-detail" title={detail(st)}>
                    <Show when={detail(st)}>
                      <span class="sr-only">{t("dbgc.col_detail")}: </span>
                    </Show>
                    {detail(st)}
                  </span>
                  <span class="flow-jump">
                    <Show when={seq() !== null}>
                      <IconButton icon="arrow_right" label={t("ui.diag.show_line")} onClick={() => jump(st)} />
                    </Show>
                  </span>
                </li>
              );
            }}
          </For>
        </ol>
      </div>
      <p class="hint">{t("dbgc.flow_hint")}</p>
    </div>
  );
}

// ---- Log -------------------------------------------------------------------

const LEVEL_FILTERS: { value: LevelFilter; label: string }[] = [
  { value: "all", label: "dbgc.level_all" },
  { value: "info", label: "dbgc.level_info" },
  { value: "warn", label: "dbgc.level_warn" },
  { value: "error", label: "dbgc.level_error" },
];
/** Lines drawn at most (the filter and copy see all kept lines) */
const LOG_SHOWN = 1500;

/** The user has selected text inside `el` (a native copy then copies that) */
function textSelectedIn(el: HTMLElement | undefined): boolean {
  const sel = typeof window !== "undefined" ? window.getSelection() : null;
  return !!el && !!sel && !sel.isCollapsed && sel.rangeCount > 0 && el.contains(sel.getRangeAt(0).commonAncestorContainer);
}

function LogPanel(props: { view: LogView }) {
  const app = useApp();
  const { core } = app;
  const v = props.view;
  onCleanup(core.wantFlow());
  // "All" shows debug lines: ask the core for them while that is chosen
  createEffect(() => {
    const release = core.wantLogs(v.level() === "all" ? "debug" : "info");
    onCleanup(release);
  });

  const attempt = () => (v.attemptOnly() ? core.flow() : null);
  const filtered = createMemo(() => filterLog(core.logs(), { level: v.level(), query: v.query(), attempt: attempt() }));
  const focusIndex = createMemo(() => {
    const k = v.focusKey();
    if (v.follow() || k === null) return null;
    return filtered().findIndex((l) => logKey(l) === k);
  });
  const shown = createMemo(() => {
    const [a, b] = drawnRange(filtered().length, LOG_SHOWN, focusIndex());
    return filtered().slice(a, b);
  });
  const text = (l: LogLine) => (mask() ? maskIdentifiers(l.message) : l.message);
  const lineText = (l: LogLine) => formatLogLine(l, timeText(l.time_ms), mask());

  let box: HTMLDivElement | undefined;
  const scrollDown = () => {
    if (v.follow() && box) box.scrollTop = box.scrollHeight;
  };
  // Follow new lines
  createEffect(() => {
    shown();
    queueMicrotask(scrollDown);
  });

  // A jump from the flow: find the line (a debug line comes with the
  // "all" subscription), make the filters show it, select and scroll to it
  createEffect(() => {
    const j = v.jump();
    if (!j) return;
    const line = core.logs().find((l) => l.seq === j.seq);
    untrack(() => {
      if (!line) {
        if (v.level() !== "all") v.setLevel("all");
        return;
      }
      if (filterLog([line], { level: v.level(), query: v.query(), attempt: attempt() }).length === 0) {
        v.setLevel("all");
        v.setQuery("");
        v.setAttemptOnly(false);
      }
      const k = logKey(line);
      v.setFollow(false);
      v.setSelected(new Set([k]));
      v.setAnchor(k);
      v.setFocusKey(k);
      v.setJump(null);
      requestAnimationFrame(() => {
        box?.querySelector<HTMLElement>(".log-line.jumped")?.scrollIntoView?.({ block: "center" });
        box?.focus();
      });
    });
  });

  const notifyCopied = (ok: boolean, n: number) => app.notify(ok ? t("dbgc.copied", n) : t("dbgc.copy_failed"), ok ? "info" : "error");

  const copyShown = async () => {
    const lines = filtered().map(lineText);
    notifyCopied(await copyText(lines.join("\n")), lines.length);
  };
  const copySelected = async () => {
    const sel = v.selected();
    const lines = core
      .logs()
      .filter((l) => sel.has(logKey(l)))
      .map(lineText);
    if (lines.length === 0) return;
    notifyCopied(await copyText(lines.join("\n")), lines.length);
  };

  const onLineClick = (e: MouseEvent, l: LogLine) => {
    // Dragging over text selects text, not lines
    if (textSelectedIn(box)) return;
    const k = logKey(l);
    if (e.shiftKey && v.anchor() !== null) {
      v.setSelected(new Set(keysBetween(filtered(), v.anchor()!, k)));
    } else if (e.ctrlKey || e.metaKey) {
      const next = new Set(v.selected());
      if (next.has(k)) next.delete(k);
      else next.add(k);
      v.setSelected(next);
      v.setAnchor(k);
    } else {
      v.setSelected(new Set([k]));
      v.setAnchor(k);
    }
    v.setFocusKey(null);
    // Selecting an older line stops following new ones (v1)
    const list = filtered();
    if (list.length && logKey(list[list.length - 1]) !== k) v.setFollow(false);
  };

  const onLogKey = (e: KeyboardEvent) => {
    if (e.key === "Escape" && v.selected().size > 0) {
      v.setSelected(new Set<string>());
      return;
    }
    if (!(e.ctrlKey || e.metaKey)) return;
    const k = e.key.toLowerCase();
    if (k === "a") {
      e.preventDefault();
      v.setSelected(new Set(filtered().map(logKey)));
    } else if (k === "c" && !textSelectedIn(box) && v.selected().size > 0) {
      e.preventDefault();
      void copySelected();
    }
  };

  const logDir = () => core.state.snap!.log_dir;

  return (
    <div class="log-panel">
      <div class="log-tools">
        <Select
          label={t("dbgc.col_level")}
          value={v.level()}
          options={LEVEL_FILTERS.map((f) => ({ value: f.value, label: t(f.label) }))}
          onChange={v.setLevel}
        />
        <div class="search">
          <Icon name="search" />
          <input
            type="search"
            class="input"
            placeholder={t("dbgc.search_hint")}
            aria-label={t("dbgc.search_hint")}
            value={v.query()}
            onInput={(e) => v.setQuery(e.currentTarget.value)}
          />
        </div>
        <Check label={t("dbgc.session_only")} checked={v.attemptOnly()} onChange={v.setAttemptOnly} />
        <Check label={t("dbgc.autoscroll")} checked={v.follow()} onChange={v.setFollow} />
        <span class="muted num">{t("dbgc.count", filtered().length, core.logs().length)}</span>
      </div>
      <div class="log-tools">
        <Button icon="copy" kind="subtle" disabled={v.selected().size === 0} onClick={copySelected}>
          {t("dbgc.copy_selected")}
        </Button>
        <Button icon="copy" kind="subtle" onClick={copyShown}>
          {t("dbgc.copy_log")}
        </Button>
        <Button
          icon="delete"
          kind="subtle"
          onClick={() => {
            core.clearLogs();
            v.setSelected(new Set<string>());
            v.setFocusKey(null);
          }}
        >
          {t("ui.diag.clear")}
        </Button>
        <span class="log-tools-gap" />
        <span class="hint">{t("ui.diag.logs_unmasked")}</span>
        <Button icon="folder_open" kind="subtle" disabled={!logDir()} onClick={() => void core.transport.openFolder(logDir())}>
          {t("ui.diag.open_logs")}
        </Button>
      </div>
      <div class="log" ref={box} role="log" aria-live="off" tabindex="0" aria-label={t("dbgc.section_log")} onKeyDown={onLogKey}>
        <Show when={shown().length > 0} fallback={<p class="muted empty-line">{t("ui.diag.no_log")}</p>}>
          <For each={shown()}>
            {(l) => {
              const k = logKey(l);
              return (
                <div
                  class={`log-line lv-${l.level}`}
                  classList={{ selected: v.selected().has(k), jumped: v.focusKey() === k }}
                  onClick={(e) => onLineClick(e, l)}
                >
                  <span class="log-time num" title={clock(l.time_ms)}>
                    {timeText(l.time_ms)}
                  </span>
                  <span class="log-level">{l.level.toUpperCase()}</span>
                  <span class="log-target">{l.target}</span>
                  <span class="log-msg">{text(l)}</span>
                </div>
              );
            }}
          </For>
        </Show>
      </div>
    </div>
  );
}
