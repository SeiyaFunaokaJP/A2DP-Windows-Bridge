// Chip tools (Developer screen): a chip script typed in or opened from a
// file, checked as it is typed and run on a WinUSB adapter in place of the
// Bluetooth stack (docs/chip-support.md). Lines that may change the chip
// run only when allowed; the core refuses lines that could change it for
// good.

import { createEffect, createSignal, For, on, onCleanup, Show } from "solid-js";
import { useApp } from "../app/context";
import { Banner, Button, Check, Field, Section, Select } from "../components/controls";
import { t } from "../i18n";
import type { ScriptCheck, ScriptResult } from "../ipc/types";
import { LINKS } from "../lib/links";
import { adapterName } from "./Adapter";

/** Delay before a typed script is checked */
const CHECK_MS = 300;

const EXAMPLE = "# Read Local Version, then BD_ADDR\nhci 1001\nhci 1009\n";

export function ChipTools() {
  const app = useApp();
  const { core } = app;
  const adapters = () => core.state.snap!.adapter.adapters.filter((a) => a.driver === "winusb");
  const [adapter, setAdapter] = createSignal("");
  const current = () => adapters().find((a) => a.instance_id === adapter())?.instance_id ?? adapters()[0]?.instance_id ?? "";
  const [script, setScript] = createSignal(EXAMPLE);
  const [check, setCheck] = createSignal<ScriptCheck | null>(null);
  const [allow, setAllow] = createSignal(false);
  const [running, setRunning] = createSignal(false);
  const [result, setResult] = createSignal<ScriptResult | null>(null);

  // Checked by the core as it is typed, for the adapter's vendor
  createEffect(
    on([script, current], ([text, id]) => {
      if (!id) return;
      const timer = setTimeout(() => {
        core
          .request("adapter.check_script", { instance_id: id, script: text })
          .then(setCheck)
          .catch(() => setCheck(null));
      }, CHECK_MS);
      onCleanup(() => clearTimeout(timer));
    }),
  );
  // A new script asks again
  createEffect(on(script, () => setAllow(false), { defer: true }));

  const runnable = () => {
    const c = check();
    return !!c && c.errors.length === 0 && c.denied.length === 0 && (c.writes.length === 0 || allow());
  };

  const run = () => {
    setRunning(true);
    setResult(null);
    void app
      .run(async () => {
        setResult(
          await core.request("adapter.run_script", { instance_id: current(), script: script(), allow_writes: allow() }),
        );
      })
      .finally(() => setRunning(false));
  };

  let file!: HTMLInputElement;
  const open = async () => {
    const f = file.files?.[0];
    if (f) setScript(await f.text());
    file.value = "";
  };

  return (
    <Show when={adapters().length > 0} fallback={<Banner kind="info" title={t("ui.dev.chip_no_adapter")} />}>
      <Section
        title={t("ui.dev.chip_script")}
        actions={
          <Button icon="open" kind="subtle" onClick={() => void core.transport.openUrl(LINKS.chipScripts)}>
            {t("ui.dev.chip_format")}
          </Button>
        }
      >
        <Field label={t("ui.nav.adapter")}>
          {(id) => (
            <Select
              id={id}
              value={current()}
              options={adapters().map((a) => ({ value: a.instance_id, label: adapterName(a) }))}
              onChange={setAdapter}
            />
          )}
        </Field>
        <textarea
          class="input mono chip-script"
          aria-label={t("ui.dev.chip_script")}
          spellcheck={false}
          value={script()}
          onInput={(e) => setScript(e.currentTarget.value)}
        />
        <div class="row">
          <input ref={file} type="file" accept=".txt,.chip,text/plain" hidden onChange={() => void open()} />
          <Button icon="folder_open" onClick={() => file.click()}>
            {t("ui.dev.chip_open")}
          </Button>
          <Button kind="strong" icon="play" disabled={!runnable() || running()} onClick={run}>
            {t("ui.dev.chip_run")}
          </Button>
        </div>
        <Show when={check()}>
          {(c) => (
            <>
              <Show when={c().errors.length > 0}>
                <Banner kind="error" title={t("ui.dev.chip_errors")}>
                  <pre class="chip-lines">{c().errors.join("\n")}</pre>
                </Banner>
              </Show>
              <Show when={c().denied.length > 0}>
                <Banner kind="error" title={t("ui.dev.chip_denied")}>
                  <pre class="chip-lines">{c().denied.join("\n")}</pre>
                </Banner>
              </Show>
              <Show when={c().errors.length === 0 && c().denied.length === 0 && c().writes.length > 0}>
                <Banner kind="warn" title={t("ui.dev.chip_writes")}>
                  <pre class="chip-lines">{c().writes.join("\n")}</pre>
                  <Check label={t("ui.dev.chip_allow_writes")} checked={allow()} onChange={setAllow} />
                </Banner>
              </Show>
            </>
          )}
        </Show>
      </Section>
      <Show when={result()}>
        {(r) => (
          <Section
            title={r().summary}
            actions={
              <Button icon="folder_open" kind="subtle" onClick={() => void core.transport.revealPath(r().path)}>
                {t("ui.dev.chip_show_report")}
              </Button>
            }
          >
            <pre class="chip-lines" aria-label={t("ui.dev.chip_result")}>
              <For each={r().lines}>{(l) => `${l}\n`}</For>
            </pre>
            <Show when={r().other_events.length > 0}>
              <pre class="chip-lines">
                <For each={r().other_events}>{(e) => `event ${e}\n`}</For>
              </pre>
            </Show>
          </Section>
        )}
      </Show>
    </Show>
  );
}
