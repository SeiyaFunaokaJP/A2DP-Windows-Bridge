// Auracast (Developer screen): broadcasts the test tone or the system audio
// to every receiver nearby that joins (stream.start_broadcast; the core
// refuses it while developer mode is off). The broadcast is a stream like
// any other: Stop and the status follow core.state.snap.stream, the choices
// outlive the view (one set per core connection).

import { createEffect, createRoot, createSignal, Show } from "solid-js";
import { useApp } from "../app/context";
import { Button, Field, Section, Segmented } from "../components/controls";
import { t, tm } from "../i18n";
import type { Core } from "../ipc/store";

type Source = "tone" | "loopback";
type Quality = "hq" | "sq";
type Channels = "stereo" | "mono";

/** The core's limits (le_audio::broadcast): UTF-8 bytes */
export const NAME_BYTES = [4, 32] as const;
export const CODE_BYTES = [4, 16] as const;

interface Choices {
  name: string;
  code: string;
  quality: Quality;
  channels: Channels;
  source: Source;
}

const bytes = (s: string) => new TextEncoder().encode(s).length;

/** The error of a name or code field, null when it is fine (an empty code: none) */
export function lengthError(text: string, [min, max]: readonly [number, number], optional: boolean): string | null {
  const n = bytes(optional ? text : text.trim());
  if (optional && n === 0) return null;
  return n < min || n > max ? t("ui.dev.auracast_err_length", min, max) : null;
}

function createSession(core: Core) {
  return createRoot(() => {
    const [choices, setChoices] = createSignal<Choices>({
      name: "A2DPWB",
      code: "",
      quality: "hq",
      channels: "stereo",
      source: "tone",
    });
    /** A broadcast asked for here, until the stream it started ends or another takes over */
    const [mine, setMine] = createSignal(false);
    /** Why the last broadcast asked for here ended with an error */
    const [failed, setFailed] = createSignal<string | null>(null);
    /** stream.start_broadcast not answered yet: the stream before it still shows */
    const [asking, setAsking] = createSignal(false);
    createEffect(() => {
      const st = core.state.snap?.stream;
      if (!st || !mine() || asking()) return;
      if (st.state === "error") {
        setFailed(tm(st.message));
        setMine(false);
      } else if (st.state === "idle" || (st.state === "streaming" && st.message.key !== "status.broadcasting")) {
        setMine(false);
      }
    });
    return { choices, setChoices, mine, setMine, failed, setFailed, asking, setAsking };
  });
}

const sessions = new WeakMap<Core, ReturnType<typeof createSession>>();

export function Auracast() {
  const app = useApp();
  const { core } = app;
  let session = sessions.get(core);
  if (!session) {
    session = createSession(core);
    sessions.set(core, session);
  }
  const s = session;
  const choices = s.choices;
  const set = (patch: Partial<Choices>) => s.setChoices({ ...choices(), ...patch });
  const stream = () => core.state.snap!.stream;

  /** The running stream is the broadcast */
  const live = () => {
    const st = stream();
    if (st.state === "streaming") return st.message.key === "status.broadcasting";
    return st.state === "connecting" && s.mine() && st.profile === null;
  };
  /** What to show under the form: the broadcast's state, or why it ended */
  const status = () => {
    const st = stream();
    if (live()) return { text: tm(st.message), bad: false };
    const failed = s.failed();
    return failed === null ? null : { text: failed, bad: true };
  };

  const nameError = () => lengthError(choices().name, NAME_BYTES, false);
  const codeError = () => lengthError(choices().code, CODE_BYTES, true);

  const startStop = async () => {
    if (live()) {
      await app.run(() => core.request("stream.stop"));
      return;
    }
    const c = choices();
    s.setAsking(true);
    s.setFailed(null);
    s.setMine(true);
    const ok = await app.run(() =>
      core.request("stream.start_broadcast", {
        name: c.name.trim(),
        code: c.code === "" ? null : c.code,
        quality: c.quality,
        mono: c.channels === "mono",
        source: c.source,
      }),
    );
    if (!ok) s.setMine(false);
    s.setAsking(false);
  };

  const locked = () => live() || s.asking();

  return (
    <div>
      <p class="hint">{t("ui.dev.auracast_intro")}</p>
      <Section
        title={t("ui.dev.auracast")}
        actions={
          <Button
            kind={live() ? "danger" : "strong"}
            icon={live() ? "stop" : "play"}
            busy={s.asking()}
            disabled={!live() && (nameError() !== null || codeError() !== null)}
            onClick={startStop}
          >
            {live() ? t("receiver.stop") : t("receiver.start")}
          </Button>
        }
      >
        <Field label={t("ui.dev.auracast_name")} error={nameError()}>
          {(id, describedBy) => (
            <input
              id={id}
              class="input"
              type="text"
              value={choices().name}
              disabled={locked()}
              aria-describedby={describedBy}
              aria-invalid={nameError() ? "true" : undefined}
              onInput={(e) => set({ name: e.currentTarget.value })}
            />
          )}
        </Field>
        <Field label={t("ui.dev.auracast_code")} error={codeError()}>
          {(id, describedBy) => (
            <input
              id={id}
              class="input"
              type="text"
              autocomplete="off"
              spellcheck={false}
              value={choices().code}
              placeholder={t("ui.dev.auracast_code_none")}
              disabled={locked()}
              aria-describedby={describedBy}
              aria-invalid={codeError() ? "true" : undefined}
              onInput={(e) => set({ code: e.currentTarget.value })}
            />
          )}
        </Field>
        <Choice<Quality>
          label={t("ui.dev.auracast_quality")}
          value={choices().quality}
          disabled={locked()}
          options={[
            ["hq", t("ui.dev.auracast_hq")],
            ["sq", t("ui.dev.auracast_sq")],
          ]}
          onChange={(quality) => set({ quality })}
        />
        <Choice<Channels>
          label={t("ui.dev.auracast_channels")}
          value={choices().channels}
          disabled={locked()}
          options={[
            ["stereo", t("ui.dev.auracast_stereo")],
            ["mono", t("ui.dev.auracast_mono")],
          ]}
          onChange={(channels) => set({ channels })}
        />
        <Choice<Source>
          label={t("receiver.source")}
          value={choices().source}
          disabled={locked()}
          options={[
            ["tone", t("receiver.source_tone")],
            ["loopback", t("receiver.source_system")],
          ]}
          onChange={(source) => set({ source })}
        />
        <Show when={status()}>
          {(st) => (
            <div role="status" class="stack-tight" style={st().bad ? { color: "var(--bad-text)" } : undefined}>
              <p>{st().text}</p>
              <Show when={live() && stream().info?.le?.summary}>{(sum) => <p class="muted">{sum()}</p>}</Show>
            </div>
          )}
        </Show>
      </Section>
    </div>
  );
}

function Choice<T extends string>(props: {
  label: string;
  value: T;
  disabled: boolean;
  options: [T, string][];
  onChange: (v: T) => void;
}) {
  return (
    <div class="row">
      <span class="row-text">
        <span class="row-label">{props.label}</span>
      </span>
      <Segmented<T>
        label={props.label}
        value={props.value}
        options={props.options.map(([value, label]) => ({ value, label, disabled: props.disabled }))}
        onChange={props.onChange}
      />
    </div>
  );
}
