// Dark / light / system. The colours are CSS variables (styles/app.css)
// keyed on <html data-theme>; "system" follows prefers-color-scheme live.
// The backend gets the choice too, for the title bar and Mica.

import type { Transport } from "./ipc/transport";

/** Applies `theme`; returns the cleanup (stops following the system) */
export function applyTheme(setting: string, transport: Transport): () => void {
  // Anything but "light" and "system" is dark, as v1 read it (v1 files may
  // say e.g. "white"); the window frame gets the same value
  const theme = setting === "light" || setting === "system" ? setting : "dark";
  const root = document.documentElement;
  const media = typeof matchMedia === "function" ? matchMedia("(prefers-color-scheme: dark)") : null;
  const set = () => {
    const dark = theme === "dark" || (theme === "system" && (media?.matches ?? true));
    root.dataset.theme = dark ? "dark" : "light";
  };
  set();
  void transport.setTheme(theme).catch(() => {});
  if (theme === "system" && media) {
    media.addEventListener("change", set);
    return () => media.removeEventListener("change", set);
  }
  return () => {};
}

/** Windows' accent colour on the controls (styles/app.css --accent-fill),
 *  read again whenever the window gets the focus */
export function followAccent(transport: Transport): void {
  if (!transport.accentColors) return;
  const root = document.documentElement;
  const read = () =>
    void transport
      .accentColors?.()
      .then((a) => {
        if (!a) return;
        root.style.setProperty("--sys-accent-light", a.light);
        root.style.setProperty("--sys-accent-dark", a.dark);
      })
      .catch(() => {});
  read();
  window.addEventListener("focus", read);
}
