// Small display helpers

/** AVRCP absolute volume (0..127) as a percentage */
export const volumePercent = (v: number) => Math.round((v * 100) / 127);
export const percentToVolume = (p: number) => Math.round((p * 127) / 100);

export const kbps = (bytesPerSecond: number) => (bytesPerSecond * 8) / 1000;

export function khz(hz: number): string {
  const k = hz / 1000;
  return Number.isInteger(k) ? String(k) : k.toFixed(1);
}

/** Core uptime in ms as h:mm:ss.mmm */
export function clock(ms: number): string {
  const h = Math.floor(ms / 3_600_000);
  const m = Math.floor(ms / 60_000) % 60;
  const s = Math.floor(ms / 1000) % 60;
  const f = Math.floor(ms % 1000);
  return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}.${String(f).padStart(3, "0")}`;
}

const H = "[0-9A-Fa-f]";
/**
 * "AC:80:0A:12:34:56" or with '-'; not in a longer run (IPv6, "...:55:66"), a
 * letter before or a colon after ("AC:80:0A:12:34:56: pairing") is fine
 */
const SEPARATED = [":", "\\-"].map(
  (s) => new RegExp(`(?<!${s})(${H}{2}${s}${H}{2}${s}${H}{2}${s})${H}{2}${s}${H}{2}${s}${H}{2}(?!${H}|${s}${H}{2})`, "g"),
);
/** A word of 12 hex digits, not a GUID's last part */
const BARE = /(?<![A-Za-z0-9-])[0-9A-Fa-f]{12}(?![A-Za-z0-9])/g;
const SERIAL_END = `\\\\#\\s"',;:.()[\\]{}\\u0080-\\uffff`;
/** USB\VID_0BDA&PID_8771\<serial>, a device path's #<serial># */
const USB_INSTANCE = new RegExp(`(VID_${H}{4}&PID_${H}{4}[A-Z0-9_&]*[\\\\#]+)([^${SERIAL_END}]*)`, "gi");
/** A driver record's name, USB_VID_0BDA_PID_8771_<serial> */
const USB_RECORD = new RegExp(`(VID_${H}{4}_PID_${H}{4}_)([^${SERIAL_END}]*)`, "gi");
/** A device name as the core logs it: “Taro's AirPods” (one line, up to 256 characters) */
const QUOTED_NAME = /\u201C([^\u201C\u201D\n]{1,256})\u201D/g;
/** \Users\<name>\ (or '/', JSON's doubled '\'): the name, up to what a name cannot contain */
const USER_PATH = /([\\/]Users[\\/]+)([^\\/:*?"<>|\x00-\x1f]+)/gi;
const NOBODY = ["public", "default", "default user", "all users"];
/** A drive path, X:\ (or '/'), not in a word: its parts, each after separators; a part
 *  stops at what a file name cannot contain, or at a space that starts another path */
const DRIVE_PATH = /(?<![A-Za-z0-9])[A-Za-z]:((?:[\\/]+(?:(?!\s[A-Za-z]:[\\/])[^\\/:*?"<>|\x00-\x1f])*)+)/g;
/** The app's own folders: a path is kept from one of these on */
const APP_FOLDERS = ["a2dpwb", "a2dp windows bridge"];
/** A SID with a machine part, S-1-5-21-a-b-c-rid (five or more numbers); not in a longer
 *  word (a '-' on either side is fine: the pipe's a2dpwb-<sid>-<instance>) */
const SID = /(?<![A-Za-z0-9])S-1(?:-\d+){5,}(?![A-Za-z0-9]|-\d)/g;

/** Names and addresses seen so far -> their number: the same one the same
 *  <device-N> / <addr-N> while the window is open */
const deviceNumbers = new Map<string, number>();
const addressNumbers = new Map<string, number>();
/** The adapters' own addresses (12 upper-case hex digits): <adapter> */
const adapterKeys = new Set<string>();

/** The 12 hex digits of an address in any form, upper case */
const addressKey = (addr: string) => addr.replace(/[:-]/g, "").toUpperCase();

/** The adapters' own addresses, shown as <adapter> by maskIdentifiers
 *  (Snapshot.adapter.adapters[].address) */
export function knowAdapters(addresses: readonly (string | null | undefined)[]) {
  for (const a of addresses) {
    if (a && /^[0-9A-Fa-f]{12}$/.test(addressKey(a))) adapterKeys.add(addressKey(a));
  }
}

const addressPseudonym = (addr: string) => {
  const key = addressKey(addr);
  if (adapterKeys.has(key)) return "<adapter>";
  let n = addressNumbers.get(key);
  if (n === undefined) addressNumbers.set(key, (n = addressNumbers.size + 1));
  return `<addr-${n}>`;
};

/**
 * Masks what identifies the user, the PC and their devices, as the core's
 * mask_identifiers: device names in typographic quotes (“...”, how the core
 * logs them) -> "<device-N>", paths -> "<path>" and the part from the app's
 * own folder on ("<path>\A2DPWB\logs\x.log") or their last part alone
 * ("<path>\a2dpwb-core.exe"), the Windows user name in a path without a
 * drive ("\Users\<name>\...") -> x's, Windows account SIDs (the IPC pipe's
 * name) -> "<sid>", Bluetooth addresses "AC:80:0A:12:34:56"
 * (or with '-', or the 12 hex digits alone: Windows' Bluetooth device IDs,
 * USB serials that are the adapter's address) -> "<addr-N>", the same
 * address the same N whatever its form, the adapters' own (knowAdapters)
 * -> "<adapter>"; nothing of an address stays, as its first half names the
 * maker. Other USB serial numbers -> x's. IDs Windows generates
 * ("5&2C7B4E3&0&2") stay.
 */
export function maskIdentifiers(text: string): string {
  const serial = (generatedStays: boolean) => (_: string, head: string, s: string) =>
    head + ((generatedStays && s.includes("&")) || /^[0-9A-Fa-f]{12}$/.test(s) ? s : "x".repeat(s.length));
  let out = text.replace(QUOTED_NAME, (_, name: string) => {
    let n = deviceNumbers.get(name);
    if (n === undefined) deviceNumbers.set(name, (n = deviceNumbers.size + 1));
    return `<device-${n}>`;
  });
  out = out.replace(USER_PATH, (whole, head: string, name: string) =>
    NOBODY.includes(name.toLowerCase()) ? whole : head + "x".repeat(name.length),
  );
  // After the user name: a path that ends in the user's folder keeps that part
  out = out.replace(DRIVE_PATH, (_, tail: string) => {
    const parts = [...tail.matchAll(/([\\/]+)([^\\/]*)/g)];
    let keep = parts.findIndex((p) => APP_FOLDERS.includes(p[2].toLowerCase()));
    if (keep < 0) keep = parts.findLastIndex((p) => p[2] !== "");
    if (keep < 0) keep = parts.length - 1;
    return "<path>" + parts.slice(keep).map((p) => p[0]).join("");
  });
  out = out.replace(SID, "<sid>");
  // Before the addresses: a serial that is an address is left to them
  out = out.replace(USB_INSTANCE, serial(true)).replace(USB_RECORD, serial(false));
  for (const re of SEPARATED) out = out.replace(re, (whole: string) => addressPseudonym(whole));
  return out.replace(BARE, (whole: string) => addressPseudonym(whole));
}

/**
 * Full-width characters an IME types ("ＡＣ：８０") as their ASCII forms,
 * as v1 (ascii_from_fullwidth), plus the dashes a Japanese IME gives for
 * "-" ("ー", "−")
 */
export function asciiFromFullwidth(s: string): string {
  return s.replace(/[\uFF01-\uFF5E\u3000\u30FC\u2010-\u2015\u2212]/g, (c) => {
    const v = c.charCodeAt(0);
    if (v >= 0xff01 && v <= 0xff5e) return String.fromCharCode(v - 0xfee0);
    return v === 0x3000 ? " " : "-";
  });
}

/**
 * A Bluetooth address as typed: 12 hex digits, optionally in pairs
 * separated by colons, hyphens, dots or spaces, also full-width (v1
 * accepted the same). Returns "AA:BB:CC:DD:EE:FF" or null.
 */
export function parseAddress(input: string): string | null {
  const s = asciiFromFullwidth(input).trim();
  if (!/^[0-9A-Fa-f]{2}([:\-. ]?[0-9A-Fa-f]{2}){5}$/.test(s)) return null;
  const hex = s.replace(/[:\-. ]/g, "").toUpperCase();
  return (hex.match(/.{2}/g) ?? []).join(":");
}

export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    // No clipboard permission: the selection way
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    const ok = document.execCommand("copy");
    ta.remove();
    return ok;
  }
}
