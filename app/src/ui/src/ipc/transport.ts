// What the store needs from the host: the Tauri backend (tauri.ts) or the
// fake core used in a plain browser and in tests (mock.ts).

import type { ServerMessage } from "./types";

export type ConnStatus =
  | { state: "connecting" }
  | { state: "connected"; server: string }
  | { state: "core_missing"; detail: string }
  | { state: "rejected"; reason: string };

export type WindowKind = "main" | "flyout";

/** The in-app update (src-tauri/src/update.rs) */
export type UpdatePhase =
  | { state: "unavailable"; reason: "portable" | "unsigned" }
  | { state: "idle" }
  | { state: "downloading"; version: string; downloaded: number; total: number | null }
  | { state: "waiting"; version: string }
  | { state: "installing"; version: string }
  | { state: "failed"; code: string; detail: string };

export interface Transport {
  readonly window: WindowKind;
  /** The window has the Mica backdrop: keep the page background translucent */
  readonly mica: boolean;
  /** The UI's own version */
  readonly version: string;
  /** The licence texts shipped with the app (null in a development build) */
  readonly licensesDir: string | null;
  /** Delivers every server message and status change from now on */
  start(onMessage: (m: ServerMessage) => void, onStatus: (s: ConnStatus) => void): Promise<void>;
  /** One request; rejects with an ErrorInfo-shaped value */
  call(method: string, params?: unknown): Promise<unknown>;

  showMain(route?: string): Promise<void>;
  hideWindow(): Promise<void>;
  /** The flyout's page is this tall (CSS pixels): its window fits it */
  fitHeight?(height: number): Promise<void>;
  /** The main window takes the setup's size, or the size it had before */
  setupWindow?(active: boolean): Promise<void>;
  openUrl(url: string): Promise<void>;
  openFolder(path: string): Promise<void>;
  revealPath(path: string): Promise<void>;
  setTheme(theme: string): Promise<void>;
  /** Windows' accent colours ("#rrggbb"), null if not known */
  accentColors?(): Promise<{ light: string; dark: string } | null>;
  /** The backend asks the main window to show a screen */
  onNavigate(cb: (route: string) => void): void;
  /** The window was minimized */
  onMinimized(cb: () => void): void;

  updateStatus(): Promise<UpdatePhase>;
  /** Downloads the release of `manifestUrl` (UpdateInfo.manifest_url) and
   *  installs it once nothing streams; the app restarts */
  updateInstall(manifestUrl: string): Promise<void>;
  onUpdatePhase(cb: (p: UpdatePhase) => void): void;
}
