// The peer receiver test of v1 (Peer Receiver Test, wx_receiver_dialog.cpp on the develop branch):
// finds tools/linux_sink on another PC, streams to its Bluetooth adapter
// without a saved profile (stream.start_custom), checks each step OK / NG
// and sets what this PC sent beside what the receiver counted. The logic is
// in lib/receiver.ts; this is the form, the table and the log.
//
// The measurement (steps, log, Reset, the choices) belongs to the core
// connection, not to this view: switching tabs keeps it.

import { createEffect, createRoot, createSignal, For, onCleanup, onMount, Show, untrack, type JSX } from "solid-js";
import { errorText, useApp } from "../app/context";
import { Button, Field, Section, Segmented, Select } from "../components/controls";
import { Icon } from "../components/Icon";
import type { IconName } from "../components/icons";
import { t, tm } from "../i18n";
import type { Core } from "../ipc/store";
import type { ReceiverInfo } from "../ipc/types";
import { bitDepths, CODECS, codecSpec, sampleRates } from "../lib/codecs";
import { copyText, khz } from "../lib/format";
import {
  BUFFERS_MS,
  canStart,
  compare,
  configureFailed,
  LOG_KEEP,
  MTUS,
  newModel,
  noBaseline,
  ROW_IDS,
  ROW_KEYS,
  startFailed,
  startMeasurement,
  statusText,
  STEP_IDS,
  STEP_KEYS,
  stopMeasurement,
  takeBaseline,
  update,
  type Baseline,
  type StepState,
  type TestModel,
} from "../lib/receiver";

type Source = "tone" | "loopback";

interface LogEntry {
  time: number;
  text: string;
}

interface Choices {
  codec: string;
  quality: string;
  rate: number;
  depth: number;
  source: Source;
  mtu: number;
  buffer: number;
}

/** What outlives the view: one per core connection */
function createSession() {
  return createRoot(() => {
    const model: TestModel = newModel();
    const [version, setVersion] = createSignal(0);
    const [log, setLog] = createSignal<LogEntry[]>([]);
    const [base, setBase] = createSignal<Baseline>(noBaseline());
    const [choices, setChoices] = createSignal<Choices>({
      codec: "auto",
      quality: "hq",
      rate: 0,
      depth: 0,
      source: "tone",
      mtu: 0,
      buffer: 0,
    });
    const [address, setAddress] = createSignal<string | null>(null);
    return {
      model,
      /** Read to follow changes of `model` (it is changed in place) */
      version,
      touched: () => setVersion((v) => v + 1),
      log,
      addLog(lines: string[]) {
        if (lines.length === 0) return;
        const time = Date.now();
        setLog((l) => [...l, ...lines.map((text) => ({ time, text }))].slice(-LOG_KEEP));
      },
      clearLog: () => setLog([]),
      base,
      setBase,
      choices,
      setChoices,
      /** The address field; null until the view first opens */
      address,
      setAddress,
      autoConnected: false,
    };
  });
}

type Session = ReturnType<typeof createSession>;
const sessions = new WeakMap<Core, Session>();

function sessionFor(core: Core): Session {
  let s = sessions.get(core);
  if (!s) {
    s = createSession();
    sessions.set(core, s);
  }
  return s;
}

const pad2 = (n: number) => String(n).padStart(2, "0");
function timeText(ms: number): string {
  const d = new Date(ms);
  return `${pad2(d.getHours())}:${pad2(d.getMinutes())}:${pad2(d.getSeconds())}`;
}

const STEP_ICON: Record<StepState, IconName | null> = {
  pending: null,
  running: "arrow_sync",
  ok: "checkmark_circle",
  failed: "error_circle",
};
const STEP_CLASS: Record<StepState, string> = { pending: "pending", running: "running", ok: "done", failed: "failed" };
const stepStatus = (s: StepState) =>
  s === "ok" ? "OK" : s === "failed" ? "NG" : t(s === "running" ? "dbgc.status.running" : "dbgc.status.pending");

const BAD: JSX.CSSProperties = { color: "var(--bad-text)" };
const RECEIVER_PORT = 51201;

export function ReceiverTest() {
  const app = useApp();
  const { core } = app;
  const session = sessionFor(core);
  const m = session.model;
  onCleanup(core.wantReceiver());
  onCleanup(core.wantLink());

  const snap = () => core.state.snap!;
  const receiver = () => core.receiver();
  const connection = () => receiver()?.connection ?? "off";
  const stream = () => snap().stream;
  const lastLink = () => {
    const l = core.link();
    return l.length > 0 ? l[l.length - 1] : null;
  };

  if (session.address() === null) {
    const saved = snap().settings.remote_sink;
    session.setAddress(saved === "auto" ? "" : saved);
  }
  const address = () => session.address() ?? "";

  // ---- Refresh: on every change of the receiver or the stream, and once a
  // second for the timeouts (v1's timer) ----
  const [tick, setTick] = createSignal(0);
  const timer = setInterval(() => setTick((n) => n + 1), 1000);
  onCleanup(() => clearInterval(timer));
  createEffect(() => {
    tick();
    const r = receiver();
    const s = stream();
    // Tracked here: the store changes these in place
    void s.state;
    void s.info?.codec;
    const serviceText = tm(s.message);
    untrack(() => {
      session.addLog(update(m, { receiver: r, stream: s, serviceText, now: Date.now() }));
      session.touched();
    });
  });

  // ---- Receiver connection ----
  const connect = async (target?: string) => {
    if (target !== undefined) session.setAddress(target);
    const to = address().trim() || "auto";
    await app.run(() => core.request("receiver.connect", { target: to }));
  };
  const disconnect = () => app.run(() => core.request("receiver.disconnect"));
  const toggleConnection = () => (connection() === "off" ? connect() : disconnect());

  // v1 looked for the receiver as soon as the window opened
  onMount(() => {
    if (session.autoConnected) return;
    session.autoConnected = true;
    core
      .request("receiver.status")
      .then((r) => {
        if (r.connection === "off") void connect();
      })
      .catch(() => {});
  });

  const [searching, setSearching] = createSignal(false);
  const [found, setFound] = createSignal<ReceiverInfo[] | null>(null);
  const discover = async () => {
    setSearching(true);
    try {
      setFound(await core.request("receiver.discover"));
    } catch (e) {
      app.notify(errorText(e), "error");
    } finally {
      setSearching(false);
    }
  };
  const useFound = async (r: ReceiverInfo) => {
    if (connection() !== "off") await disconnect();
    await connect(r.port === RECEIVER_PORT ? r.ip : `${r.ip}:${r.port}`);
  };

  // ---- Choices ----
  const choices = session.choices;
  const set = (patch: Partial<Choices>) => session.setChoices({ ...choices(), ...patch });
  const setCodec = (codec: string) => {
    const spec = codecSpec(codec);
    const c = choices();
    set({
      codec,
      quality: spec.qualities.some((q) => q.id === c.quality) ? c.quality : "hq",
      rate: sampleRates(codec).includes(c.rate) ? c.rate : 0,
      depth: bitDepths(codec).includes(c.depth) ? c.depth : 0,
    });
  };
  /** Codecs the receiver offers (lower case, no spaces); empty: not known */
  const offered = () => {
    const list = receiver()?.stats?.codecs ?? receiver()?.receiver?.codecs ?? [];
    return list.map((c) => c.toLowerCase().replace(/[\s_-]/g, ""));
  };
  const codecOptions = () =>
    CODECS.map((c) => {
      const no = c.id !== "auto" && offered().length > 0 && !offered().includes(c.id);
      return { value: c.id as string, label: t(c.label) + (no ? t("ui.receiver.not_offered") : ""), disabled: no };
    });
  const qualityOptions = () => {
    const q = codecSpec(choices().codec).qualities;
    return q.length > 0 ? q.map((x) => ({ value: x.id, label: t(x.label) })) : [{ value: choices().quality, label: "-" }];
  };
  const rateLabel = (r: number) => (r ? `${khz(r)} kHz` : t("codec.auto"));
  const depthLabel = (d: number) => (d ? t(`bitdepth.${d}`) : t("codec.auto"));
  const mtuLabel = (v: number) => (v ? String(v) : t("receiver.as_started"));
  const bufferLabel = (v: number) => (v ? `${v} ms` : t("receiver.as_started"));
  const sourceLabel = (s: Source) => t(s === "tone" ? "receiver.source_tone" : "receiver.source_system");

  // ---- Start / Stop ----
  const measuring = () => {
    session.version();
    return m.measuring;
  };
  const [starting, setStarting] = createSignal(false);
  const startStop = async () => {
    if (m.measuring) {
      session.addLog([t("receiver.log_stopped")]);
      stopMeasurement(m);
      session.touched();
      await app.run(() => core.request("stream.stop"));
      return;
    }
    const r = receiver();
    const info = r?.receiver;
    if (!info?.bt_address) return;
    const c = choices();
    const lines = startMeasurement(m, { receiver: r, mtu: c.mtu, bufferMs: c.buffer, now: Date.now() });
    const qualityLabel = qualityOptions().find((q) => q.value === c.quality)?.label ?? "-";
    session.addLog([
      t(
        "receiver.log_start",
        t(codecSpec(c.codec).label),
        qualityLabel,
        rateLabel(c.rate),
        depthLabel(c.depth),
        sourceLabel(c.source),
        info.bt_address,
        mtuLabel(c.mtu),
        bufferLabel(c.buffer),
      ),
      ...lines,
    ]);
    session.touched();
    setStarting(true);
    try {
      // Receiver side first: it applies the settings to the connection that follows
      try {
        await core.request("receiver.configure", { mtu: c.mtu || null, buffer_ms: c.buffer || null });
      } catch (e) {
        session.addLog(configureFailed(m, errorText(e), Date.now()));
      }
      try {
        await core.request("stream.start_custom", {
          device: info.bt_address,
          device_name: info.bt_name || null,
          codec: c.codec,
          quality: c.quality,
          sample_rate: c.rate || null,
          bit_depth: c.depth || null,
          source: c.source,
        });
      } catch (e) {
        const text = errorText(e);
        session.addLog(startFailed(m, text, Date.now()));
        app.notify(text, "error");
      }
    } finally {
      session.touched();
      setStarting(false);
    }
  };

  // ---- Status, steps, table, log ----
  const status = () => {
    session.version();
    return statusText(m, receiver(), stream());
  };
  const steps = () => {
    session.version();
    return STEP_IDS.map((id) => ({ id, ...m.steps[id] }));
  };
  const rows = () => {
    session.version();
    return compare({ receiver: receiver(), stream: stream(), link: lastLink(), base: session.base(), lastInfo: m.lastInfo });
  };
  const reset = () => {
    session.setBase(takeBaseline(receiver()));
    session.addLog([t("ui.receiver.log_reset")]);
  };
  const copyLog = async () => {
    const lines = session.log().map((l) => `${timeText(l.time)} ${l.text}`);
    const ok = await copyText(lines.join("\n"));
    app.notify(ok ? t("dbgc.copied", lines.length) : t("dbgc.copy_failed"), ok ? "info" : "error");
  };
  let logBox: HTMLDivElement | undefined;
  createEffect(() => {
    session.log();
    queueMicrotask(() => {
      if (logBox) logBox.scrollTop = logBox.scrollHeight;
    });
  });

  const locked = () => measuring() || starting();
  const off = () => connection() === "off";

  return (
    <div class="receiver">
      <p class="hint">{t("receiver.intro")}</p>

      <Section
        title={t("receiver.address")}
        actions={
          <Button icon="search" kind="subtle" busy={searching()} onClick={discover}>
            {t("ui.receiver.search")}
          </Button>
        }
      >
        <div class="inline">
          <input
            class="input mono"
            style={{ flex: "1" }}
            aria-label={t("receiver.address")}
            title={t("receiver.address_tooltip")}
            placeholder={t("receiver.address_hint")}
            value={address()}
            disabled={!off()}
            onInput={(e) => session.setAddress(e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void connect();
            }}
          />
          <Button kind={off() ? "strong" : "default"} icon={off() ? undefined : "plug_disconnected"} onClick={toggleConnection}>
            {off() ? t("receiver.connect") : t("receiver.disconnect")}
          </Button>
        </div>
        <Show when={found()}>
          {(list) => (
            <Show when={list().length > 0} fallback={<p class="muted">{t("ui.receiver.none_found")}</p>}>
              <ul class="stack-tight" style={{ margin: "0", padding: "0", "list-style": "none" }} aria-label={t("ui.receiver.found_title")}>
                <For each={list()}>
                  {(r) => (
                    <li>
                      <Button kind="subtle" icon="desktop_speaker" onClick={() => useFound(r)}>
                        {t("receiver.found", r.host, r.ip, r.bt_address, r.mtu)}
                      </Button>
                    </li>
                  )}
                </For>
              </ul>
            </Show>
          )}
        </Show>
        <div role="status" class="stack-tight" style={status().bad ? BAD : undefined}>
          <For each={status().lines}>{(l) => <p>{l}</p>}</For>
        </div>
      </Section>

      <Section title={t("receiver.settings_sent")}>
        <Field label={t("receiver.codec")}>
          {(id) => <Select id={id} value={choices().codec} disabled={locked()} options={codecOptions()} onChange={setCodec} />}
        </Field>
        <Field label={t("receiver.quality")}>
          {(id) => (
            <Select
              id={id}
              value={choices().quality}
              disabled={locked() || codecSpec(choices().codec).qualities.length === 0}
              options={qualityOptions()}
              onChange={(quality) => set({ quality })}
            />
          )}
        </Field>
        <Field label={t("receiver.sample_rate")}>
          {(id) => (
            <Select
              id={id}
              value={choices().rate}
              disabled={locked()}
              options={sampleRates(choices().codec).map((r) => ({ value: r, label: rateLabel(r) }))}
              onChange={(rate) => set({ rate })}
            />
          )}
        </Field>
        <Field label={t("receiver.bit_depth")}>
          {(id) => (
            <Select
              id={id}
              value={choices().depth}
              disabled={locked() || bitDepths(choices().codec).length < 2}
              options={bitDepths(choices().codec).map((d) => ({ value: d, label: depthLabel(d) }))}
              onChange={(depth) => set({ depth })}
            />
          )}
        </Field>
        <div class="row">
          <span class="row-text">
            <span class="row-label">{t("receiver.source")}</span>
            <span class="hint">{t("receiver.source_tooltip")}</span>
          </span>
          <Segmented<Source>
            label={t("receiver.source")}
            value={choices().source}
            options={(["tone", "loopback"] as Source[]).map((s) => ({ value: s, label: sourceLabel(s), disabled: locked() }))}
            onChange={(source) => set({ source })}
          />
        </div>
        <p class="hint">
          {t("ui.receiver.afh_hint")}{" "}
          <button type="button" class="link" onClick={() => app.navigate({ name: "adapter", arg: "afh" })}>
            {t("ui.receiver.afh_open")}
          </button>
        </p>
      </Section>

      <Section title={t("receiver.settings_received")}>
        <Field label={t("receiver.mtu")} hint={t("receiver.mtu_tooltip")}>
          {(id, described) => (
            <Select
              id={id}
              describedBy={described}
              value={choices().mtu}
              disabled={locked()}
              options={MTUS.map((v) => ({ value: v, label: mtuLabel(v) }))}
              onChange={(mtu) => set({ mtu })}
            />
          )}
        </Field>
        <Field label={t("receiver.buffer")}>
          {(id) => (
            <Select
              id={id}
              value={choices().buffer}
              disabled={locked()}
              options={BUFFERS_MS.map((v) => ({ value: v, label: bufferLabel(v) }))}
              onChange={(buffer) => set({ buffer })}
            />
          )}
        </Field>
      </Section>

      <Section
        title={t("ui.receiver.checks")}
        actions={
          <Button
            kind={measuring() ? "danger" : "strong"}
            icon={measuring() ? "stop" : "play"}
            busy={starting()}
            disabled={!measuring() && !canStart(receiver())}
            onClick={startStop}
          >
            {measuring() ? t("receiver.stop") : t("receiver.start")}
          </Button>
        }
      >
        <ol class="flow" aria-label={t("ui.receiver.checks")}>
          <For each={steps()}>
            {(s) => (
              <li class={`flow-step st-${STEP_CLASS[s.state]}`}>
                <span class="flow-icon" aria-hidden="true">
                  <Show when={STEP_ICON[s.state]} fallback={<span class="flow-dot" />}>
                    {(i) => <Icon name={i()} />}
                  </Show>
                </span>
                <span class="flow-name">{t(STEP_KEYS[s.id])}</span>
                <span class="flow-status">{stepStatus(s.state)}</span>
                <span class="flow-time num">{s.at === null ? "" : `+${(s.at / 1000).toFixed(1)} s`}</span>
                <span class="flow-detail" title={s.detail}>
                  {s.detail}
                </span>
              </li>
            )}
          </For>
        </ol>
      </Section>

      <Section
        title={t("ui.receiver.comparison")}
        actions={
          <Button icon="arrow_reset" kind="subtle" onClick={reset}>
            {t("quality.reset")}
          </Button>
        }
      >
        <div style={{ "overflow-x": "auto" }}>
          <table class="table num">
            <thead>
              <tr>
                <td />
                <th scope="col" title={t("receiver.help")}>
                  {t("receiver.col_sent")}
                </th>
                <th scope="col" title={t("receiver.help")}>
                  {t("receiver.col_received")}
                </th>
                <th scope="col">{t("ui.receiver.col_diff")}</th>
              </tr>
            </thead>
            <tbody>
              <For each={ROW_IDS}>
                {(id) => {
                  const row = () => rows()[id];
                  return (
                    <tr>
                      <th scope="row">{t(ROW_KEYS[id])}</th>
                      <td style={row().sent.bad ? BAD : undefined}>{row().sent.text}</td>
                      <td style={row().received.bad ? BAD : undefined}>{row().received.text}</td>
                      <td style={row().diff.bad ? BAD : undefined}>{row().diff.text}</td>
                    </tr>
                  );
                }}
              </For>
            </tbody>
          </table>
        </div>
        <p class="hint">{t("receiver.help")}</p>
      </Section>

      <Section
        title={t("receiver.log")}
        actions={
          <>
            <Button icon="copy" kind="subtle" disabled={session.log().length === 0} onClick={copyLog}>
              {t("receiver.copy_log")}
            </Button>
            <Button icon="delete" kind="subtle" disabled={session.log().length === 0} onClick={() => session.clearLog()}>
              {t("ui.diag.clear")}
            </Button>
          </>
        }
      >
        <div class="log" ref={logBox} role="log" tabindex="0" aria-label={t("receiver.log")} style={{ height: "12rem" }}>
          <Show when={session.log().length > 0} fallback={<p class="muted empty-line">{t("ui.diag.no_log")}</p>}>
            <For each={session.log()}>
              {(l) => (
                <div class="log-line" style={{ "grid-template-columns": "4.5rem 1fr" }}>
                  <span class="log-time num">{timeText(l.time)}</span>
                  <span class="log-msg">{l.text}</span>
                </div>
              )}
            </For>
          </Show>
        </div>
      </Section>
    </div>
  );
}
