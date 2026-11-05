// A saved classic device's pairing (developer mode, issue #86), from two
// buttons of its row: the services it lists, as Windows' Bluetooth Services
// list shows them (the last connection's while it is not connected), and
// the link key A2DPWB keeps for it, to copy as it is or as BlueZ' whole
// info file for Linux.

import { createResource, For, Show } from "solid-js";
import { errorText, useApp } from "../app/context";
import { t } from "../i18n";
import type { Device } from "../ipc/types";
import { copyText } from "../lib/format";
import { bluezInfo, serviceLabel } from "../lib/pairing";
import { Button, Dialog } from "./controls";

export type PairingPart = "services" | "key";

export function PairingDialog(props: { device: Device | null; part: PairingPart; onClose: () => void }) {
  const app = useApp();
  const address = () => props.device?.address ?? null;
  // An error is the answer here (not connected, no answer): shown in place
  const [pairing] = createResource(
    () => (props.part === "key" ? address() : null),
    (a) =>
      app.core
        .request("device.pairing", { address: a })
        .then((p) => ({ key: p.link_key, error: null as string | null }))
        .catch((e: unknown) => ({ key: null, error: errorText(e) })),
  );
  const [services] = createResource(address, (a) =>
    app.core
      .request("device.services", { address: a })
      .then((l) => ({ list: l.services, last: l.from_last_connection, error: null as string | null }))
      .catch((e: unknown) => ({ list: [], last: false, error: errorText(e) })),
  );
  const copy = async (text: string) =>
    app.notify(t((await copyText(text)) ? "ui.pairing.copied" : "dbgc.copy_failed"));
  const name = () => props.device?.name || t("device.name_unknown");
  const uuids = () => (services()?.list ?? []).flatMap((s) => s.uuids);
  return (
    <Dialog
      open={props.device !== null}
      title={t(props.part === "key" ? "ui.pairing.key_title" : "ui.pairing.services")}
      subtitle={props.device ? `${name()} ${props.device.address}` : undefined}
      onCancel={() => props.onClose()}
    >
      <Show when={props.part === "key"}>
        <Show when={pairing()}>
          {(p) => (
            <Show when={p().key} fallback={<p class="hint">{p().error ?? t("ui.pairing.no_key")}</p>}>
              {(k) => (
                <>
                  <p>
                    <code class="mono pairing-key">{k().key.toUpperCase()}</code>{" "}
                    <span class="tag">{t("ui.pairing.key_type", k().key_type)}</span>
                  </p>
                  <p class="hint">{t("ui.pairing.secret")}</p>
                  <p class="hint">{t("ui.pairing.no_le_keys")}</p>
                  <div class="inline">
                    <Button icon="copy" onClick={() => void copy(k().key.toUpperCase())}>
                      {t("ui.pairing.copy_key")}
                    </Button>
                    <Button icon="copy" onClick={() => void copy(bluezInfo(name(), uuids(), k().key, k().key_type))}>
                      {t("ui.pairing.copy_linux")}
                    </Button>
                  </div>
                  <p class="hint">{t("ui.pairing.linux_file")}</p>
                </>
              )}
            </Show>
          )}
        </Show>
      </Show>
      <Show when={props.part === "services"}>
        <Show when={services()} fallback={<p class="hint">{t("ui.pairing.services_loading")}</p>}>
          {(s) => (
            <Show when={!s().error} fallback={<p class="hint">{s().error}</p>}>
              <Show when={s().last}>
                <p class="hint">{t("ui.pairing.services_last")}</p>
              </Show>
              <Show when={s().list.length > 0} fallback={<p class="hint">{t("ui.pairing.services_none")}</p>}>
                <ul class="pairing-services">
                  <For each={s().list}>
                    {(r) => (
                      <li>
                        <span>{serviceLabel(r) ?? t("ui.pairing.unknown_service")}</span>
                        <For each={r.uuids}>{(u) => <span class="mono muted">{u}</span>}</For>
                      </li>
                    )}
                  </For>
                </ul>
              </Show>
            </Show>
          )}
        </Show>
      </Show>
    </Dialog>
  );
}
