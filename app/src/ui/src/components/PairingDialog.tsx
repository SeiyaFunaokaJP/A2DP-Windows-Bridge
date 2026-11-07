// A saved classic device's services and pairing keys (developer mode, issue
// #86), from the Services tag of its row: the services it lists as a table
// (the last connection's while it is not connected), and below them the
// link key BlueCaster keeps for it and the LE keys derived from it (LE
// Audio on), each "not available" when BlueCaster has none.

import { createResource, createSignal, For, onCleanup, Show } from "solid-js";
import { errorText, useApp } from "../app/context";
import { t } from "../i18n";
import { IpcError, type Device, type DeviceService } from "../ipc/types";
import { copyText } from "../lib/format";
import { reverseBytes, serviceLabel } from "../lib/pairing";
import { Button, Check, Dialog } from "./controls";

/** How often and how long the services are asked again while another
 *  query runs (issue #86: the stack's own SDP queries while connecting, or
 *  the last one of this dialog; the core gives one up after 15 s) */
const BUSY_RETRY_MS = 500;
const BUSY_RETRIES = 30;

export function PairingDialog(props: { device: Device | null; onClose: () => void }) {
  const app = useApp();
  const address = () => props.device?.address ?? null;
  // An error is the answer here (not connected, no answer): shown in place
  const [pairing] = createResource(address, (a) =>
    app.core
      .request("device.pairing", { address: a })
      .then((p) => ({ key: p.link_key, le: p.le_keys, error: null as string | null }))
      .catch((e: unknown) => ({ key: null, le: null, error: errorText(e) })),
  );
  let closed = false;
  onCleanup(() => (closed = true));
  // Busy: still loading, asked again shortly, never shown as an error
  // unless it lasts
  const askServices = async (a: string, retries: number): Promise<{ list: DeviceService[]; last: boolean; error: string | null }> => {
    try {
      const l = await app.core.request("device.services", { address: a });
      return { list: l.services, last: l.from_last_connection, error: null };
    } catch (e: unknown) {
      if (IpcError.from(e).code === "device.busy" && retries > 0 && !closed && address() === a) {
        await new Promise((r) => setTimeout(r, BUSY_RETRY_MS));
        return askServices(a, retries - 1);
      }
      return { list: [], last: false, error: errorText(e) };
    }
  };
  const [services] = createResource(address, (a) => askServices(a, BUSY_RETRIES));
  const copy = async (text: string) =>
    app.notify(t((await copyText(text)) ? "ui.pairing.copied" : "dbgc.copy_failed"));
  const name = () => props.device?.name || t("device.name_unknown");
  // One switch for every key (issue #86), off: the order Bluetooth sends them
  const [reversed, setReversed] = createSignal(false);
  return (
    <Dialog
      open={props.device !== null}
      title={t("ui.pairing.services")}
      subtitle={props.device ? `${name()} ${props.device.address}` : undefined}
      wide
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
            <Show when={!p().error} fallback={<p class="hint">{p().error}</p>}>
              <Show when={p().key || p().le}>
                <Check label={t("ui.pairing.reverse_bytes")} checked={reversed()} onChange={setReversed} />
              </Show>
              <dl class="pairing-keys">
                <KeyRow
                  name={t("ui.pairing.link_key")}
                  value={p().key?.key}
                  tag={p().key ? t("ui.pairing.key_type", p().key!.key_type) : undefined}
                  reversed={reversed()}
                  copy={copy}
                />
                <KeyRow
                  name={t("ui.pairing.ltk")}
                  value={p().le?.long_term_key}
                  tag={p().le ? t("ui.pairing.key_size", p().le!.key_size) : undefined}
                  reversed={reversed()}
                  copy={copy}
                />
                <KeyRow
                  name={t("ui.pairing.irk")}
                  value={p().le?.identity_resolving_key}
                  reversed={reversed()}
                  copy={copy}
                />
                <Show when={p().le}>
                  {(le) => (
                    <>
                      <dt>{t("ui.pairing.identity_address")}</dt>
                      <dd>
                        <span class="mono">{le().identity_address}</span>
                        <Show when={le().identity_random}>
                          {" "}
                          <span class="tag">{t("ui.pairing.random")}</span>
                        </Show>
                      </dd>
                    </>
                  )}
                </Show>
              </dl>
              <Show when={p().key}>
                <p class="hint">{t("ui.pairing.secret")}</p>
                <Show when={!p().le}>
                  <p class="hint">{t("ui.pairing.le_keys_hint")}</p>
                </Show>
              </Show>
            </Show>
          )}
        </Show>
      </section>
    </Dialog>
  );
}

/** One key: its hex digits (upper case), in the other byte order with the
 *  dialog's switch on (issue #86: other systems keep some keys the other way
 *  round), with a copy button, or "not available" */
function KeyRow(props: {
  name: string;
  value: string | null | undefined;
  tag?: string;
  reversed: boolean;
  copy: (text: string) => Promise<unknown>;
}) {
  return (
    <>
      <dt>{props.name}</dt>
      <dd>
        <Show when={props.value} fallback={<span class="muted">{t("ui.pairing.not_available")}</span>}>
          {(v) => {
            const shown = () => (props.reversed ? reverseBytes(v()) : v()).toUpperCase();
            return (
              <>
                <code class="mono pairing-key">{shown()}</code>{" "}
                <Show when={props.tag}>
                  <span class="tag">{props.tag}</span>{" "}
                </Show>
                <Button icon="copy" onClick={() => void props.copy(shown())}>
                  {t("ui.pairing.copy_key")}
                </Button>
              </>
            );
          }}
        </Show>
      </dd>
    </>
  );
}
