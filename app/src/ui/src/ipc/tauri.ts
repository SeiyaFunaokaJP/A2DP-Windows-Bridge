// The transport inside bluecaster.exe: commands and events of the Rust backend
// (src-tauri/src/commands.rs, connection.rs).

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ConnStatus, Transport, UpdatePhase, WindowKind } from "./transport";
import type { ServerMessage } from "./types";

interface UiInfo {
  window: string;
  version: string;
  mica: boolean;
  licenses_dir: string | null;
}

export async function createTauriTransport(): Promise<Transport> {
  const info = await invoke<UiInfo>("ui_info");
  const kind: WindowKind = info.window === "flyout" ? "flyout" : "main";
  return {
    window: kind,
    mica: info.mica,
    version: info.version,
    licensesDir: info.licenses_dir ?? null,
    async start(onMessage, onStatus) {
      // Listen first, then read the status: nothing falls between the two
      await listen<ServerMessage>("ipc://message", (e) => onMessage(e.payload));
      await listen<ConnStatus>("ipc://status", (e) => onStatus(e.payload));
      onStatus(await invoke<ConnStatus>("ipc_status"));
    },
    call: (method, params) => invoke("ipc_request", { method, params: params ?? null }),
    showMain: (route) => invoke("show_main", { route: route ?? null }),
    hideWindow: async () => {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().hide();
    },
    fitHeight: (height) => invoke("fit_flyout", { height }),
    openUrl: (url) => invoke("open_url", { url }),
    openFolder: (path) => invoke("open_folder", { path }),
    revealPath: (path) => invoke("reveal_path", { path }),
    setTheme: (theme) => invoke("set_theme", { theme }),
    onNavigate(cb) {
      void listen<string>("ui://navigate", (e) => cb(e.payload));
    },
    updateStatus: () => invoke<UpdatePhase>("update_status"),
    updateInstall: (manifestUrl) => invoke("update_install", { manifestUrl }),
    onUpdatePhase(cb) {
      void listen<UpdatePhase>("update://phase", (e) => cb(e.payload));
    },
    onMinimized(cb) {
      void (async () => {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        const w = getCurrentWindow();
        await w.onResized(async () => {
          if (await w.isMinimized()) cb();
        });
      })();
    },
  };
}

export function insideTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}
