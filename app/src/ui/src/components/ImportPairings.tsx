// Taking over the pairings Windows' Bluetooth made with the adapter BlueCaster
// now has on WinUSB (issue #86): on Audio Devices and in the setup's device
// step, where devices are added. Shown only while Windows paired a device
// through this adapter that BlueCaster has not saved (other radios' pairings
// are no use here, and Windows may have no radio running at all); the
// core reads the keys after an administrator prompt and saves the audio
// devices with Windows' names.

import { createResource, createSignal, Show } from "solid-js";
import { useApp } from "../app/context";
import { t } from "../i18n";
import { Button } from "./controls";

export function ImportPairings() {
  const app = useApp();
  const { core } = app;
  const snap = () => core.state.snap!;
  const adapter = () => snap().adapter.adapters.find((a) => a.driver === "winusb" && a.address)?.address ?? null;
  // What Windows paired through this adapter (no prompt to read the list),
  // against the saved ones
  const [windows, { refetch }] = createResource(adapter, (address) =>
    core.request("device.windows_paired_through", { adapter: address }).catch(() => []),
  );
  const unsaved = () => {
    const saved = snap().devices.filter((d) => d.saved).map((d) => d.address.toUpperCase());
    return (windows() ?? []).filter((a) => !saved.includes(a.toUpperCase())).length;
  };
  const [busy, setBusy] = createSignal(false);
  const importNow = (address: string) => {
    setBusy(true);
    void app
      .run(async () => {
        const r = await core.request("device.import_windows_pairings", { adapter: address });
        if (!r.adapter_found) return app.notify(t("ui.adapter.import_none"));
        const kept = r.kept.length > 0 ? ` ${t("ui.adapter.import_kept", r.kept.length)}` : "";
        app.notify(`${t("ui.adapter.import_done", r.imported.length)}${kept}`);
        void refetch();
      })
      .finally(() => setBusy(false));
  };
  return (
    <Show when={adapter() && unsaved() > 0 ? adapter() : null}>
      {(address) => (
        <div class="import-pairings">
          <Button icon="arrow_download" busy={busy()} onClick={() => importNow(address())}>
            {t("ui.adapter.import_windows")}
          </Button>
          <p class="hint">{t("ui.devices.import_hint")}</p>
        </div>
      )}
    </Show>
  );
}
