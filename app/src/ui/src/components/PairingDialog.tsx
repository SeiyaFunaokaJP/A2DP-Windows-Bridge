// A saved classic device's services and pairing key (developer mode, issue
// #86), from the Services tag of its row: the services it lists as a table
// (the last connection's while it is not connected), and below them the
// link key BlueCaster keeps for it.

import { createResource, For, Show } from "solid-js";
import { errorText, useApp } from "../app/context";
import { t } from "../i18n";
import type { Device } from "../ipc/types";
import { copyText } from "../lib/format";
import { serviceLabel } from "../lib/pairing";
import { Button, Dialog } from "./controls";

export function PairingDialog(props: { device: Device | null; onClose: () => void }) {
  const app = useApp();
  const address = () => props.device?.address ?? null;
  // An error is the answer here (not connected, no answer): shown in place
  const [pairing] = createResource(address, (a) =>
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
  return (
    <Dialog
      open={props.device !== null}
      title={t("ui.pairing.services")}
      subtitle={props.device ? `${name()} ${props.device.address}` : undefined}
      onCancel={() => props.onClose()}
    >
      <section class="pairing-part">
        <Show when={services()} fallback={<p class="hint">{t("ui.pairing.services_loading")}</p>}>
          {(s) => (
            <Show when={!s().error} fallback={<p class="hint">{s().error}</p>}>
              <Show when={s().last}>
                <p class="hint">{t("ui.pairing.services_last")}</p>
              </Show>
              <Show when={s().list.length > 0} fallback={<p class="hint">{t("ui.pairing.services_none")}</p>}>
                <table class="table pairing-services">
                  <thead>
                    <tr>
                      <th scope="col">{t("ui.pairing.col_name")}</th>
                      <th scope="col">{t("ui.pairing.col_guid")}</th>
                    </tr>
                  </thead>
                  <tbody>
                    <For each={s().list}>
                      {(r) => (
                        <tr>
                          <td>{serviceLabel(r) ?? t("ui.pairing.unknown_service")}</td>
                          <td>
                            <For each={r.uuids}>{(u) => <span class="mono muted">{u}</span>}</For>
                          </td>
                        </tr>
                      )}
                    </For>
                  </tbody>
                </table>
              </Show>
            </Show>
          )}
        </Show>
      </section>
      <section class="pairing-part">
        <h3 class="field-label">{t("ui.pairing.key_title")}</h3>
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
                  </div>
                </>
              )}
            </Show>
          )}
        </Show>
      </section>
    </Dialog>
  );
}
