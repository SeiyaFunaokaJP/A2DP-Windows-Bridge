// A saved classic device's pairing (developer mode, issue #86): the link key
// A2DPWB keeps for it, to copy as it is or in the form Linux keeps it, and
// the services the device lists (asked while it is connected).

import { createResource, For, Show } from "solid-js";
import { errorText, useApp } from "../app/context";
import { t } from "../i18n";
import type { Device } from "../ipc/types";
import { copyText } from "../lib/format";
import { bluezLinkKey, serviceName } from "../lib/pairing";
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
      .then((list) => ({ list, error: null as string | null }))
      .catch((e: unknown) => ({ list: [], error: errorText(e) })),
  );
  const copy = async (text: string) =>
    app.notify(t((await copyText(text)) ? "ui.pairing.copied" : "dbgc.copy_failed"));
  return (
    <Dialog
      open={props.device !== null}
      title={t("ui.pairing.title")}
      subtitle={props.device ? `${props.device.name || t("device.name_unknown")} ${props.device.address}` : undefined}
      onCancel={() => props.onClose()}
    >
      <section class="pairing-part" aria-labelledby="pairing-key">
        <h3 id="pairing-key" class="mini-title">
          {t("ui.pairing.link_key")}
        </h3>
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
                  <div class="inline">
                    <Button icon="copy" onClick={() => void copy(k().key.toUpperCase())}>
                      {t("ui.pairing.copy_key")}
                    </Button>
                    <Button icon="copy" onClick={() => void copy(bluezLinkKey(k().key, k().key_type))}>
                      {t("ui.pairing.copy_linux")}
                    </Button>
                  </div>
                </>
              )}
            </Show>
          )}
        </Show>
      </section>
      <section class="pairing-part" aria-labelledby="pairing-services">
        <h3 id="pairing-services" class="mini-title">
          {t("ui.pairing.services")}
        </h3>
        <Show when={services()} fallback={<p class="hint">{t("ui.pairing.services_loading")}</p>}>
          {(s) => (
            <Show when={!s().error} fallback={<p class="hint">{s().error}</p>}>
              <Show when={s().list.length > 0} fallback={<p class="hint">{t("ui.pairing.services_none")}</p>}>
                <ul class="pairing-services">
                  <For each={s().list}>
                    {(r) => (
                      <li>
                        <span>{r.uuids.map((u) => serviceName(u)).find((n) => n) ?? r.name ?? "—"}</span>
                        <For each={r.uuids}>{(u) => <span class="mono muted">{u}</span>}</For>
                      </li>
                    )}
                  </For>
                </ul>
              </Show>
            </Show>
          )}
        </Show>
      </section>
    </Dialog>
  );
}
