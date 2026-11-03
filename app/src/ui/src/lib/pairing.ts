// The device pairing dialog (developer mode): names of well-known service
// UUIDs and the link key in the form Linux's BlueZ keeps it, so a dual-boot
// PC can use the same pairing there (issue #86).

const BASE = "-0000-1000-8000-00805f9b34fb";

/** Bluetooth SIG names of the 16-bit service class UUIDs devices list */
const NAMES: Record<number, string> = {
  0x1101: "Serial Port",
  0x1105: "OBEX Object Push",
  0x1106: "OBEX File Transfer",
  0x1108: "Headset",
  0x110a: "Audio Source",
  0x110b: "Audio Sink",
  0x110c: "A/V Remote Control Target",
  0x110d: "Advanced Audio Distribution",
  0x110e: "A/V Remote Control",
  0x110f: "A/V Remote Control Controller",
  0x1112: "Headset Audio Gateway",
  0x1115: "PANU",
  0x1116: "NAP",
  0x111e: "Handsfree",
  0x111f: "Handsfree Audio Gateway",
  0x1124: "Human Interface Device",
  0x112f: "Phonebook Access Server",
  0x1131: "Headset HS",
  0x1132: "Message Access Server",
  0x1200: "PnP Information",
  0x1203: "Generic Audio",
  0x1800: "Generic Access",
  0x1801: "Generic Attribute",
  0x180a: "Device Information",
  0x180f: "Battery",
};

/** The 16-bit form of a UUID of the Bluetooth base, else null */
export function shortUuid(uuid: string): number | null {
  const u = uuid.toLowerCase();
  if (!u.endsWith(BASE) || !u.startsWith("0000")) return null;
  const n = parseInt(u.slice(4, 8), 16);
  return Number.isNaN(n) ? null : n;
}

/** The SIG name of a service UUID, null for one not in the list (a vendor's) */
export function serviceName(uuid: string): string | null {
  const n = shortUuid(uuid);
  return n === null ? null : (NAMES[n] ?? null);
}

/** The [LinkKey] group of BlueZ' per-device info file
 *  (/var/lib/bluetooth/<adapter>/<device>/info): the key in the same byte
 *  order, upper case */
export function bluezLinkKey(key: string, keyType: number): string {
  return `[LinkKey]\nKey=${key.toUpperCase()}\nType=${keyType}\nPINLength=0\n`;
}
