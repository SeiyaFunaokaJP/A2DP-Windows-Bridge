// Log tab: the core's log lines with level, text and attempt filters,
// selection and copy, following new lines, and the jump from a flow step.

import { createEffect, createMemo, createSignal, For, onCleanup, Show, untrack } from "solid-js";
import { useApp } from "../../app/context";
import { Button, Check, Select } from "../../components/controls";
import { Icon } from "../../components/Icon";
import { t } from "../../i18n";
import type { LogLine } from "../../ipc/types";
import { clock, copyText, maskIdentifiers } from "../../lib/format";
import { drawnRange, filterLog, formatLogLine, keysBetween, logKey, type LevelFilter } from "../../lib/logview";
import { mask, timeText } from "./shared";

/** The log view's filters and selection (they outlive the log tab) */
export function createLogView() {
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
export type LogView = ReturnType<typeof createLogView>;

/** How long a jump waits for its line (the debug lines come after a resubscription) */
export const JUMP_WAIT_MS = 3000;

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

export function LogPanel(props: { view: LogView }) {
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
