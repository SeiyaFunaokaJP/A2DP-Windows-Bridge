// The device's Services dialog (developer mode, issue #86): names of
// well-known service UUIDs, the Bluetooth SIG's and some vendors'.

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

/** Vendors' 128-bit service UUIDs that devices list without a name (issue
 *  #86): Qualcomm GAIA on many Qualcomm-based headsets, Google Fast Pair */
const VENDOR_NAMES: Record<string, string> = {
  "00001100-d102-11e1-9b23-00025b00a5a5": "Qualcomm GAIA Root Service",
  "00001101-d102-11e1-9b23-00025b00a5a5": "Qualcomm GAIA Command/Response",
  "00001102-d102-11e1-9b23-00025b00a5a5": "Qualcomm GAIA Data",
  "00001103-d102-11e1-9b23-00025b00a5a5": "Qualcomm GAIA Notification",
  "0000eb03-d102-11e1-9b23-00025b00a5a5": "Qualcomm GAIA Extension",
  "0000eb04-d102-11e1-9b23-00025b00a5a5": "Qualcomm GAIA Extension",
  "0000eb05-d102-11e1-9b23-00025b00a5a5": "Qualcomm GAIA Extension",
  "0000eb06-d102-11e1-9b23-00025b00a5a5": "Qualcomm GAIA Extension",
  "0000eb07-d102-11e1-9b23-00025b00a5a5": "Qualcomm GAIA Extension",
  "0000eb10-d102-11e1-9b23-00025b00a5a5": "Qualcomm GAIA OTA/Debug Log",
  "df21fe2c-2515-4fdb-8886-f12c4d67927c": "Google Fast Pair",
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

/** A vendor's name of a 128-bit service UUID, null for one not in the list */
export function vendorServiceName(uuid: string): string | null {
  return VENDOR_NAMES[uuid.toLowerCase()] ?? null;
}

/** A service's name: the SIG name, else the device's own name, else a
 *  vendor's known name, else null (an unknown service). One name, for the
 *  Name column (issue #86) */
export function serviceLabel(s: { uuids: string[]; name: string | null }): string | null {
  return (
    s.uuids.map((u) => serviceName(u)).find((n) => n) ||
    s.name ||
    s.uuids.map((u) => vendorServiceName(u)).find((n) => n) ||
    null
  );
}

/** Hex digits with their bytes in the other order ("0a1b2c" -> "2c1b0a") */
export function reverseBytes(hex: string): string {
  return (hex.match(/../g) ?? []).reverse().join("");
}
