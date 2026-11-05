// Connection flow tab: the eleven steps of the current attempt, their time
// of day, elapsed time and detail, and a jump to the line that set each.

import { For, onCleanup, Show } from "solid-js";
import { useApp } from "../../app/context";
import { Button, IconButton } from "../../components/controls";
import { Icon } from "../../components/Icon";
import type { IconName } from "../../components/icons";
import { t } from "../../i18n";
import type { FlowStepView } from "../../ipc/types";
import { STEPS, type Step, type StepStatus, stepDetail } from "../../lib/flow";
import { copyText, maskIdentifiers } from "../../lib/format";
import { mask, timeText } from "./shared";

const STATUS_ICON: Record<StepStatus, IconName | null> = {
  pending: null,
  running: "arrow_sync",
  done: "checkmark_circle",
  warning: "warning",
  failed: "error_circle",
};

export function FlowPanel(props: { onJump: (seq: number) => void }) {
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
