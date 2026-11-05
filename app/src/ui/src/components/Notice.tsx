import { Show } from "solid-js";
import { useApp } from "../app/context";
import { Icon } from "./Icon";

/** The transient notice ("Copied", errors of buttons), announced politely */
export function NoticeLine() {
  const { notice } = useApp();
  return (
    <div class="notice-region" aria-live="polite" aria-atomic="true">
      <Show when={notice()}>
        {(n) => (
          <div class={`notice notice-${n().kind}`} role={n().kind === "error" ? "alert" : undefined}>
            <Icon name={n().kind === "error" ? "error_circle" : n().kind === "hint" ? "info" : "checkmark_circle"} />
            <span>{n().text}</span>
          </div>
        )}
      </Show>
    </div>
  );
}
