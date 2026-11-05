import { render } from "solid-js/web";
import { App } from "./app/App";
import { createCore } from "./ipc/store";
import { createTauriTransport, insideTauri } from "./ipc/tauri";
import "./styles/app.css";

/** To a2dpwb-ui.*.log (inside a2dpwb.exe; the browser console otherwise) */
function logToFile(level: "error" | "warn" | "info", message: string) {
  if (!insideTauri()) return;
  void import("@tauri-apps/api/core").then(({ invoke }) => invoke("ui_log", { level, message }).catch(() => {}));
}

const describe = (e: unknown) => (e instanceof Error ? `${e.name}: ${e.message}\n${e.stack ?? ""}` : String(e));

// What breaks the page leaves it blank: write it down where it can be found
window.addEventListener("error", (e) =>
  logToFile("error", `uncaught: ${describe(e.error ?? e.message)} (${e.filename}:${e.lineno}:${e.colno})`),
);
window.addEventListener("unhandledrejection", (e) => logToFile("error", `unhandled rejection: ${describe(e.reason)}`));

const root = document.getElementById("root")!;
try {
  // Inside a2dpwb.exe the Rust backend talks to the core; in a plain browser
  // (pnpm dev) a fake core with sample state stands in for it
  const transport = insideTauri() ? await createTauriTransport() : (await import("./ipc/mock")).mockFromUrl();
  render(() => <App core={createCore(transport)} />, root);
} catch (e) {
  logToFile("error", `the window could not start: ${describe(e)}`);
  // Not a blank window: say what happened
  root.textContent = `A2DP Windows Bridge: the window could not start (${e instanceof Error ? e.message : String(e)})`;
  root.style.padding = "24px";
}
