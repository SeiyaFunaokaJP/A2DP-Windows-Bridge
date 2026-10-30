// Adapter: the USB Bluetooth adapters with their driver and firmware,
// WinUSB assign / restore, the chip for the firmware, and AFH.

import { createEffect, createMemo, createSignal, createUniqueId, For, type JSX, onCleanup, onMount, Show } from "solid-js";
import { useApp } from "../app/context";
import { saveSettings } from "../app/settings";
import { Banner, Button, Field, Section, Segmented, Select } from "../components/controls";
import { Icon } from "../components/Icon";
import { ErrorPanel, FirmwareDownload, openFirmwareFolder, WinusbDialog } from "../components/stream";
import { t, tm } from "../i18n";
import type { AdapterInfo, DriverKind, FirmwareFile } from "../ipc/types";
import {
  adapterFirmwareTag,
  buildChipOptions,
  type ChipStatus,
  chipStatus,
  chipTargetIndex,
  isAutoChip,
} from "../lib/chips";
import { LINKS, vendorFirmwareUrl } from "../lib/links";

/** The adapters BlueCaster can use first (WinUSB), the ones without a driver
 *  and then the ones it cannot use at all last */
export function usableFirst(list: readonly AdapterInfo[]): AdapterInfo[] {
  const rank = (a: AdapterInfo) =>
    (adapterSupported(a) ? 0 : 3) + (a.driver === "winusb" ? 0 : a.driver === "none" ? 2 : 1);
  return [...list].sort((a, b) => rank(a) - rank(b));
}

const hex4 = (n: number) => n.toString(16).toUpperCase().padStart(4, "0");

/** "Realtek Bluetooth 5.3 Adapter (RTL8761BU)": the name Device Manager
 *  gives it with the chip (issue #86); before Windows' Bluetooth ever ran
 *  it and with no product name of its own, the vendor and chip ("Realtek
 *  RTL8761BU"), without the VID:PID the line below shows and the
 *  "[unsupported]" / "[experimental]" tags say */
export function adapterName(
  a: Pick<AdapterInfo, "display_name" | "vendor" | "windows_name" | "realtek_chip">,
): string {
  const name = a.windows_name?.trim();
  if (!name) {
    const chip = a.display_name
      .replace(/\s*\(0x[0-9a-f]{4}:0x[0-9a-f]{4}\)/i, "")
      .replace(/\s*\[(unsupported|experimental)\]/i, "")
      .trim();
    if (!a.vendor || chip.toLowerCase().startsWith(a.vendor.toLowerCase())) return chip;
    return `${a.vendor} ${chip}`;
  }
  const chip = a.realtek_chip;
  return chip && !name.toUpperCase().includes(chip.toUpperCase()) ? `${name} (${chip})` : name;
}

/** "8C:88:4B:46:DD:AE | 0BDA:A729 | v5.1": what is known of it */
export function adapterIds(a: Pick<AdapterInfo, "address" | "vid" | "pid" | "bt_version">): string {
  return [a.address, `${hex4(a.vid)}:${hex4(a.pid)}`, a.bt_version && `v${a.bt_version}`].filter(Boolean).join(" | ");
}

/** Why adapterIds lacks the address or the Bluetooth version: neither
 *  Windows' Bluetooth (it runs one radio only) nor BlueCaster has
 *  used the adapter yet (issue #86); undefined when both are known */
export function adapterIdsHint(a: Pick<AdapterInfo, "address" | "bt_version">): string | undefined {
  return a.address && a.bt_version ? undefined : t("ui.adapter.ids_unknown");
}

/** A vendor whose loader is experimental (Intel, Broadcom): the core marks
 *  the chip "[experimental]" */
export function adapterExperimental(a: Pick<AdapterInfo, "display_name">): boolean {
  return /\[experimental\]/i.test(a.display_name);
}

/** BlueCaster can stream through it: not a chip whose firmware loader it lacks
 *  (MediaTek, Qualcomm). Those need their firmware loaded at every start
 *  before they answer as a Bluetooth controller, so they cannot work at all:
 *  neither WinUSB nor a firmware download is offered for them (issue #86) */
export function adapterSupported(a: Pick<AdapterInfo, "vendor_note">): boolean {
  return a.vendor_note?.key !== "firmware.vendor_unsupported";
}

/** The driver the adapter has now, in two names only whatever the adapter
 *  and its manufacturer (issue #86): WinUSB, or the "original" one WinUSB
 *  replaces (Windows' BTHUSB, or a vendor's own such as MediaTek's filter or
 *  a composite device's; the service shows on hover) */
export function driverText(d: DriverKind): { text: string; status: "good" | "warn" | "bad" } {
  switch (d) {
    case "winusb":
      return { text: t("ui.adapter.driver_winusb"), status: "good" };
    case "bluetooth":
    case "other":
      return { text: t("ui.adapter.driver_original"), status: "warn" };
    default:
      return { text: t("ui.adapter.driver_none"), status: "bad" };
  }
}

/** How often the shown list is asked for anew: there is no Refresh button
 *  (issue #86) and the core has no device notification, so an adapter
 *  plugged in or out shows up by itself */
export const ADAPTER_POLL_MS = 5000;

export function Adapter() {
  const app = useApp();
  const { core } = app;
  const snap = () => core.state.snap!;
  const adapter = () => snap().adapter;
  // The order shown; the chip section goes with its adapter in it
  const sorted = () => usableFirst(adapter().adapters);
  // The card the chip choice goes in; none for an adapter BlueCaster cannot
  // use, the choice then stands on its own below the cards
  const chipCard = () => {
    const i = chipTargetIndex(sorted());
    return i >= 0 && adapterSupported(sorted()[i]) ? i : -1;
  };
  const [dialog, setDialog] = createSignal<{ adapter: AdapterInfo; mode: "assign" | "restore" } | null>(null);
  const [listing, setListing] = createSignal(false);
  // The *_fw.bin files in the settings folder: chips the table lacks
  const [files, setFiles] = createSignal<FirmwareFile[]>([]);
  const loadFiles = async () => {
    try {
      setFiles(await core.request("adapter.firmware_files"));
    } catch {
      // The choice keeps the table's chips
    }
  };

  // The list anew, one request at a time
  const relist = async () => {
    if (listing()) return;
    setListing(true);
    await Promise.all([core.request("adapter.list").catch(() => {}), loadFiles()]);
    setListing(false);
  };

  // v1 checked the firmware again when its dialog got the focus back: the
  // files may have been placed through Open Config Folder meanwhile. On
  // opening the screen and every few seconds while it is in view as well,
  // for an adapter plugged in or out
  const onFocus = () => void relist();
  onMount(() => {
    void relist();
    window.addEventListener("focus", onFocus);
    const timer = setInterval(() => {
      if (document.visibilityState === "visible" && dialog() === null) void relist();
    }, ADAPTER_POLL_MS);
    onCleanup(() => clearInterval(timer));
  });
  onCleanup(() => window.removeEventListener("focus", onFocus));
  // #/adapter/afh (the receiver test links there): the AFH section in view;
  // #/adapter/firmware (Home's firmware bar): the firmware part
  createEffect(() => {
    const id = { afh: AFH_SECTION, firmware: FIRMWARE_SECTION }[app.route().arg ?? ""];
    if (id) document.getElementById(id)?.scrollIntoView?.({ block: "start" });
  });

  return (
    <div class="page">
      <header class="page-head">
        <h1>{t("ui.nav.adapter")}</h1>
        <div class="head-actions">
          <Button icon="arrow_reset" onClick={() => app.run(() => core.request("adapter.restart"))}>
            {t("ui.adapter.restart")}
          </Button>
        </div>
      </header>

      <p class={`status-line st-${adapter().status}`} role="status">
        <span class="dot" aria-hidden="true" />
        {t("ui.adapter.stack")}: {t(`ui.adapter.status_${adapter().status}`)}
      </p>
      <Show when={adapter().status === "error" && adapter().error}>{(m) => <ErrorPanel message={m()} />}</Show>

      <Show
        when={adapter().adapters.length > 0}
        fallback={
          <Banner kind="warn" title={t("firmware.no_adapters")}>
            {t("ui.adapter.none_hint")}
          </Banner>
        }
      >
        <For each={sorted()}>
          {(a, i) => {
            const drv = () => driverText(a.driver);
            const fw = () => adapterFirmwareTag(a, snap().settings);
            const driverHead = createUniqueId();
            const firmwareHead = createUniqueId();
            return (
              <section class="card adapter-card" aria-label={adapterName(a)}>
                <div class="adapter-head">
                  <Icon name="usb_stick" />
                  <div>
                    {/* Device Manager's name; the USB device's own on hover */}
                    <h2 title={a.device_name}>
                      {adapterName(a)}
                      <Show when={adapterExperimental(a)}>
                        {" "}
                        <span class="tag st-warn" title={t("ui.adapter.experimental_hint")}>
                          {t("ui.adapter.experimental")}
                        </span>
                      </Show>
                    </h2>
                    <span class="muted mono" title={adapterIdsHint(a)}>
                      {adapterIds(a)}
                    </span>
                  </div>
                </div>
                <div class="adapter-boxes">
                  <div class="adapter-box" role="group" aria-labelledby={driverHead}>
                    <h3 id={driverHead}>
                      {t("ui.adapter.driver")}:{" "}
                      <span class={`tag st-${drv().status}`} title={a.driver_service || undefined}>
                        {drv().text}
                      </span>
                      <Show when={a.restart_needed}>
                        {" "}
                        <span class="tag st-warn" title={t("ui.adapter.restart_needed_hint")}>
                          {t("ui.adapter.restart_needed")}
                        </span>
                      </Show>
                    </h3>
                    <Show
                      when={a.driver === "winusb"}
                      fallback={
                        <>
                          {/* One BlueCaster cannot use keeps its driver (issue #86) */}
                          <Button
                            kind="strong"
                            icon="shield"
                            disabled={!adapterSupported(a)}
                            onClick={() => setDialog({ adapter: a, mode: "assign" })}
                          >
                            {t("ui.adapter.assign_winusb")}
                          </Button>
                          <p class="hint">
                            {t(adapterSupported(a) ? "ui.adapter.assign_short" : "ui.adapter.assign_unsupported")}
                          </p>
                        </>
                      }
                    >
                      <Button icon="arrow_reset" onClick={() => setDialog({ adapter: a, mode: "restore" })}>
                        {t("ui.adapter.restore_driver")}
                      </Button>
                      <p class="hint">{t("ui.adapter.restore_short")}</p>
                    </Show>
                  </div>
                  <div
                    class="adapter-box"
                    id={i() === chipCard() ? FIRMWARE_SECTION : undefined}
                    role="group"
                    aria-labelledby={firmwareHead}
                  >
                    <h3 id={firmwareHead}>
                      {t("ui.adapter.firmware")}: <span class={`tag st-${fw().status}`}>{t(fw().key)}</span>
                      {/* The running stack found it on its ROM code (issue #81) */}
                      <Show when={adapter().rom_firmware && a.realtek_chip && a.driver === "winusb"}>
                        {" "}
                        <span class="tag st-bad" title={t("firmware.rom_status")}>
                          {t("ui.adapter.rom_firmware")}
                        </span>
                      </Show>
                    </h3>
                    {/* The chip setting is one for all adapters: with the one it applies to */}
                    <Show when={i() === chipCard()}>
                      <ChipSection files={files()} onChanged={loadFiles} adapter={a} />
                    </Show>
                    <Show when={!adapterSupported(a)}>
                      <Button icon="arrow_download" disabled>
                        {t("ui.adapter.firmware_download_unavailable")}
                      </Button>
                      <p class="hint">{t("firmware.vendor_unsupported")}</p>
                    </Show>
                  </div>
                </div>
              </section>
            );
          }}
        </For>
      </Show>

      {/* No adapter to put it with: on its own, to choose the chip beforehand */}
      <Show when={chipCard() < 0}>
        <ChipSection files={files()} onChanged={loadFiles} />
      </Show>
      <AfhSection />

      <WinusbDialog adapter={dialog()?.adapter ?? null} mode={dialog()?.mode ?? "assign"} onClose={() => setDialog(null)} />
    </div>
  );
}

/** v1 FirmwareDialog: the chip choice and the firmware state of that chip */
function ChipSection(props: { files: FirmwareFile[]; onChanged: () => Promise<void>; adapter?: AdapterInfo }) {
  const app = useApp();
  const snap = () => app.core.state.snap!;
  const settings = () => snap().settings;
  const adapters = () => snap().adapter.adapters;
  const detected = () => adapters().find((a) => a.realtek_chip)?.realtek_chip ?? null;
  const choice = () =>
    buildChipOptions(settings(), props.files, {
      auto: t("firmware.chip_auto", detected() ?? t("firmware.chip_auto_none")),
      nonRealtek: t("firmware.chip_non_realtek"),
    });
  const hint = () => (isAutoChip(settings()) && detected() ? t("firmware.auto_selected") : t("firmware.select_chip"));
  const choose = async (key: string) => {
    const o = choice().options.find((x) => x.key === key);
    if (!o || (o.pid === settings().bt_chip_pid && o.stem === settings().bt_chip_fw_stem)) return;
    // The core lists the adapters anew with the firmware state for this chip
    // (and releases an idle stack); the folder may have changed as well
    if (await saveSettings(app, { bt_chip_pid: o.pid, bt_chip_fw_stem: o.stem })) await props.onChanged();
  };
  // A new object on every published state would build the banner anew and
  // close a firmware download's dialog while it runs: only a changed status
  // counts
  const status = createMemo(() => chipStatus(settings(), adapters(), props.files), undefined, {
    equals: (a, b) => JSON.stringify(a) === JSON.stringify(b),
  });
  const body = () => (
    <>
      <Field label={t("settings.bt_chip")} hint={hint()}>
        {(id, described) => (
          <Select
            id={id}
            describedBy={described}
            value={choice().selected}
            options={choice().options.map((o) => ({ value: o.key, label: o.label }))}
            onChange={(key) => void choose(key)}
          />
        )}
      </Field>
      <div aria-live="polite">
        <Show when={status()} keyed>
          {(st) => <FirmwareStatus status={st} embedded={props.adapter !== undefined} />}
        </Show>
      </div>
      <div class="inline wrap">
        {/* Also to fetch the files again when they are there */}
        <Show when={props.adapter?.firmware_download && props.adapter}>
          {(a) => <FirmwareDownload files={a().firmware_files} kind={a().firmware_present ? undefined : "strong"} />}
        </Show>
        <Button icon="folder_open" kind="subtle" onClick={() => void openFirmwareFolder(app)}>
          {t("firmware.open_folder")}
        </Button>
        <Button icon="open" kind="subtle" onClick={() => app.core.transport.openUrl(LINKS.setupGuide)}>
          {t("ui.setup.guide")}
        </Button>
      </div>
    </>
  );
  // In the adapter's firmware box, which has the heading
  return (
    <Show
      when={props.adapter}
      fallback={
        <Section id={FIRMWARE_SECTION} title={t("firmware.title")}>
          {body()}
        </Section>
      }
    >
      {body()}
    </Show>
  );
}

/** v1 UpdateFirmwareStatus: the files, what is missing, a vendor's note */
/** `embedded`: in the adapter's firmware box, whose tag says the files are
 *  there and which has the download button itself */
function FirmwareStatus(props: { status: ChipStatus; embedded?: boolean }): JSX.Element {
  const app = useApp();
  const download = (url: string) => (
    <Button icon="open" onClick={() => app.core.transport.openUrl(url)}>
      {t("firmware.open_download_page")}
    </Button>
  );
  // The core fetches the files itself; the page stays for doing it by hand
  const realtekActions = (files: string[], fetch: boolean) => (
    <>
      <Show when={fetch && !props.embedded}>
        <FirmwareDownload files={files} kind="strong" />
      </Show>
      {download(LINKS.firmwareRealtek)}
    </>
  );
  const fileList = (files: string[]) => (
    <ul class="mono files">
      <For each={files}>{(f) => <li>{f}</li>}</For>
    </ul>
  );
  const st = props.status;
  switch (st.kind) {
    case "present":
      if (props.embedded) return null;
      return (
        <Banner kind="good" title={t("firmware.present")}>
          {fileList(st.files)}
        </Banner>
      );
    case "missing":
    case "unchecked": {
      if (st.kind === "unchecked" && st.present === true) {
        if (props.embedded) return null;
        return (
          <Banner kind="good" title={t("firmware.present")}>
            {fileList(st.files)}
          </Banner>
        );
      }
      const missing = st.kind === "missing" || st.present === false;
      return (
        <Banner
          kind={missing ? "warn" : "info"}
          title={missing ? t("firmware.missing_manual") : undefined}
          actions={realtekActions(st.files, st.kind === "missing" && st.download)}
        >
          <p>{t("firmware.required_files")}</p>
          {fileList(st.files)}
          <p class="hint">{t("firmware.manual_instructions")}</p>
        </Banner>
      );
    }
    case "invalid":
      return (
        <Banner kind="warn" title={t("firmware.invalid_short")} actions={realtekActions(st.files, st.download)}>
          <p>{t("firmware.invalid", st.file)}</p>
          <p>{t("firmware.required_files")}</p>
          {fileList(st.files)}
        </Banner>
      );
    case "vendor": {
      const url = vendorFirmwareUrl(st.vendor);
      return (
        <Banner
          kind={st.note.key === "firmware.vendor_unsupported" ? "warn" : "info"}
          actions={url ? download(url) : undefined}
        >
          {tm(st.note)}
        </Banner>
      );
    }
    case "non_realtek":
      return <Banner kind="good">{t("firmware.non_realtek")}</Banner>;
    case "auto_no_adapter":
      return <Banner kind="warn">{t("firmware.auto_no_adapter")}</Banner>;
    case "auto_unrecognized":
      return <Banner kind="warn">{t("firmware.auto_unrecognized")}</Banner>;
    default:
      return <Banner kind="warn">{t("firmware.select_chip")}</Banner>;
  }
}

/** "6,11": Wi-Fi channels 1..14, at least one */
export function parseAfhChannels(s: string): string | null {
  const parts = s.split(/[\s,]+/).filter(Boolean);
  if (parts.length === 0) return null;
  const nums = parts.map(Number);
  if (nums.some((n) => !Number.isInteger(n) || n < 1 || n > 14)) return null;
  return [...new Set(nums)].sort((a, b) => a - b).join(",");
}

const AFH_SECTION = "adapter-afh";
const FIRMWARE_SECTION = "adapter-firmware";

function AfhSection() {
  const app = useApp();
  const afh = () => app.core.state.snap!.settings.afh;
  const mode = () => (afh() === "auto" || afh() === "off" ? afh() : "manual");
  const [channels, setChannels] = createSignal(mode() === "manual" ? afh() : "");
  const [manual, setManual] = createSignal(mode() === "manual");
  const bad = () => manual() && channels().trim() !== "" && parseAfhChannels(channels()) === null;
  const saveChannels = () => {
    const c = parseAfhChannels(channels());
    if (c && c !== afh()) void saveSettings(app, { afh: c });
  };
  // Auto: the Wi-Fi channels the running stack avoids now (empty while none
  // or no stack runs)
  onCleanup(app.core.wantLink());
  const avoided = () => {
    const l = app.core.link();
    return l.length ? l[l.length - 1].afh_avoided.replace(/\s+/g, "") : "";
  };
  // One box for the channels (issue #86): Auto shows what it chose, Off
  // nothing, both read only; Manual takes the user's
  const shown = () => (manual() ? channels() : mode() === "auto" ? avoided() : "");
  return (
    <Section
      id={AFH_SECTION}
      title={t("receiver.afh")}
      actions={
        <div class="afh-control">
          <Segmented
            label={t("receiver.afh")}
            value={manual() ? "manual" : mode()}
            options={[
              { value: "auto", label: t("receiver.afh_auto") },
              { value: "off", label: t("receiver.afh_off") },
              { value: "manual", label: t("receiver.afh_manual") },
            ]}
            onChange={(v) => {
              setManual(v === "manual");
              if (v === "manual") saveChannels();
              else void saveSettings(app, { afh: v });
            }}
          />
          <input
            class="input mono afh-channels"
            placeholder={manual() ? "6,11" : ""}
            value={shown()}
            readOnly={!manual()}
            aria-label={t("receiver.afh_channels")}
            aria-describedby="afh-channels-err"
            aria-invalid={bad() ? "true" : undefined}
            onInput={(e) => setChannels(e.currentTarget.value)}
            onBlur={() => manual() && saveChannels()}
            onKeyDown={(e) => manual() && e.key === "Enter" && saveChannels()}
          />
        </div>
      }
    >
      <p class="hint">{t("receiver.afh_tooltip")}</p>
      <Show when={bad()}>
        <span class="field-error" id="afh-channels-err" role="alert">
          <Icon name="error_circle" />
          {t("ui.adapter.afh_bad")}
        </span>
      </Show>
    </Section>
  );
}
