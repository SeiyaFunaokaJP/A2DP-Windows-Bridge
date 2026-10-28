// The tray flyout (360×440): everyday control without the main window.
// As Home's card (issue #86): the ring, and beside it the device chooser
// in the title's place, battery and state, the codec (chosen right there)
// and link quality;
// then the volume, the big Connect / Disconnect and "Open app". No close
// button: the flyout hides on Escape and when it loses the focus. Its
// window is as tall as the page (issue #86: no empty space).

import { createSignal, onCleanup, onMount, Show } from "solid-js";
import { errorText, useApp } from "../app/context";
import { CaptureSign } from "../app/indicators";
import { Button, IconButton } from "../components/controls";
import { Icon } from "../components/Icon";
import { NoticeLine } from "../components/Notice";
import { DeviceSwitcher } from "../components/DeviceSwitcher";
import {
  BatteryLevel,
  CodecBadge,
  ConnectButton,
  copyError,
  isAppFault,
  reportError,
  currentProfile,
  QualityIndicator,
  Ring,
  showsBattery,
  statusText,
  useFixes,
  VolumeControl,
} from "../components/stream";
import { t, tm } from "../i18n";
import { IpcError } from "../ipc/types";
import { audioEntries, entryOf } from "../lib/profile";
import { CodecPicker } from "./Home";

export function Flyout() {
  const app = useApp();
  const { core } = app;
  const snap = () => core.state.snap!;
  const stream = () => snap().stream;
  const profile = () => currentProfile(snap());
  const entry = () => entryOf(audioEntries(snap()), profile());
  const [reqError, setReqError] = createSignal<IpcError | null>(null);
  const { fixFor, dialog } = useFixes();
  onCleanup(core.wantLink());

  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Escape") void core.transport.hideWindow();
  };
  onMount(() => document.addEventListener("keydown", onKey));
  onCleanup(() => document.removeEventListener("keydown", onKey));
  let page: HTMLDivElement | undefined;
  onMount(() => {
    if (!page || !core.transport.fitHeight || typeof ResizeObserver === "undefined") return;
    const fit = () => void core.transport.fitHeight!(Math.ceil(page!.getBoundingClientRect().height)).catch(() => {});
    const observer = new ResizeObserver(fit);
    observer.observe(page);
    onCleanup(() => observer.disconnect());
  });

  const error = () => {
    const e = reqError();
    if (e) return { code: e.code as string | null, message: e.msg };
    if (stream().state === "error") return { code: null, message: stream().message };
    return null;
  };
  // In the flyout a fix that needs a screen opens the main window there
  const openMain = (route?: string) => core.transport.showMain(route).catch(() => {});

  return (
    <div class="flyout" ref={page}>
      <header class="fly-head">
        <Ring state={stream().state} volume={stream().volume} size="small">
          <Icon name="headphones" />
        </Ring>
        <div class="fly-title">
          {/* The device chooser is the title: the device once (issue #86) */}
          <Show
            when={profile()}
            fallback={<div class="device-name ellipsis">{t("profile.none_selected")}</div>}
          >
            <DeviceSwitcher />
          </Show>
          {/* As on Home and in the sidebar: battery | state, the codec (issue #86) */}
          <div class="fly-state">
            <Show when={stream().state !== "idle" && showsBattery(stream())}>
              <BatteryLevel class="fly-battery" percent={stream().battery} />
              <span class="muted" aria-hidden="true">
                |
              </span>
            </Show>
            <div class={`status-line st-${stream().state}`} role="status" aria-live="polite">
              <span class="dot" aria-hidden="true" />
              <span class="ellipsis">{statusText(stream())}</span>
            </div>
          </div>
          <CodecBadge
            aside={<QualityIndicator />}
            picker={<Show when={entry()}>{(e) => <CodecPicker entry={e()} />}</Show>}
          />
        </div>
      </header>

      <Show when={profile()} fallback={<p class="muted">{t("profile.empty")}</p>}>
        {/* As on Home: the volume while a device is in use */}
        <Show when={stream().state !== "idle"}>
          <VolumeControl />
        </Show>
        <ConnectButton compact onError={(e) => setReqError(IpcError.from(e))} />
      </Show>
      {/* Kept simple: the capture source is chosen in the main window (issue #86) */}
      <Show when={snap().capture}>
        <div class="fly-meta">
          <CaptureSign onOpen={() => void openMain("diagnostics")} />
        </div>
      </Show>

      <Show when={error()}>
        {(e) => {
          const fix = () => fixFor(e().code, e().message.key);
          return (
            <div class="fly-error" role="alert">
              <Icon name="error_circle" />
              <span class="fly-error-text">{tm(e().message)}</span>
              <IconButton
                icon="copy"
                label={t("ui.error.copy_hint")}
                onClick={() => void copyError(app, e().message, e().code)}
              />
              <Show when={isAppFault(e().message.key, e().code)}>
                <IconButton
                  icon="open"
                  label={t("ui.error.report_hint")}
                  onClick={() => void reportError(app, e().message, e().code)}
                />
              </Show>
              <Show when={fix()}>
                {(f) => (
                  <Button
                    kind="subtle"
                    icon={f().icon}
                    onClick={() => {
                      // Fixes that are a screen of the main window go there
                      const k = e().message.key;
                      if (k.startsWith("error.codec") || k.startsWith("error.app") || k.startsWith("error.no_")) {
                        void openMain("profiles");
                      } else if (e().code === "adapter.driver_missing" || k === "error.btstack_init") {
                        void openMain("adapter");
                      } else {
                        void app.run(async () => f().run()).then((ok) => !ok && app.notify(errorText(e()), "error"));
                      }
                    }}
                  >
                    {f().label}
                  </Button>
                )}
              </Show>
            </div>
          );
        }}
      </Show>

      <footer class="fly-foot">
        <Show when={snap().update.available}>
          <Button kind="subtle" icon="arrow_sync" onClick={() => void openMain("settings")}>
            {t("update.available_title")}
          </Button>
          <span class="spacer" />
        </Show>
        <Button icon="open" onClick={() => void openMain()}>
          {t("ui.flyout.open_app")}
        </Button>
      </footer>
      {dialog()}
      <NoticeLine />
    </div>
  );
}
