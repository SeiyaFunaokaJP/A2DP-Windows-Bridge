// The 2.4 GHz band as the Radio tab draws it: Bluetooth channel n is at
// 2402 + n MHz (79 channels), a Wi-Fi channel spreads +-11 MHz around its
// center (the same width bt-host's afh module avoids).

export const BT_CHANNELS = 79;
export const BT_FIRST_MHZ = 2402;
export const WIFI_HALF_WIDTH_MHZ = 11;
/** Wi-Fi channels that do not overlap each other */
export const WIFI_CLEAR_CHANNELS = [1, 6, 11];

/** Center frequency of a 2.4 GHz Wi-Fi channel (1-14) */
export function wifiCenterMhz(channel: number): number {
  return channel === 14 ? 2484 : 2407 + 5 * channel;
}

/** The link's channel map (10 bytes, bit n = channel n) as 79 flags: true
 *  = the link hops over it. Null without a reading. */
export function channelsInUse(map: readonly number[] | null | undefined): boolean[] | null {
  if (!map || map.length < 10) return null;
  return Array.from({ length: BT_CHANNELS }, (_, n) => (map[n >> 3] & (1 << (n & 7))) !== 0);
}

/** Bluetooth channels a Wi-Fi channel covers */
export function wifiCovers(channel: number): number[] {
  const c = wifiCenterMhz(channel);
  const out: number[] = [];
  for (let n = 0; n < BT_CHANNELS; n++) if (Math.abs(BT_FIRST_MHZ + n - c) <= WIFI_HALF_WIDTH_MHZ) out.push(n);
  return out;
}

/** "6, 11" -> [6, 11] */
export function parseWifiChannels(s: string): number[] {
  return (s.match(/\d+/g) ?? []).map(Number).filter((n) => n >= 1 && n <= 14);
}

/** The non-overlapping Wi-Fi channel (1, 6 or 11) whose Bluetooth channels
 *  the link leaves out most, when it leaves out at least half of them:
 *  where the interference most likely is. */
export function busiestWifiChannel(inUse: readonly boolean[] | null): number | null {
  if (!inUse) return null;
  let best: number | null = null;
  let bestShare = 0.5;
  for (const ch of WIFI_CLEAR_CHANNELS) {
    const covered = wifiCovers(ch);
    const share = covered.filter((n) => !inUse[n]).length / covered.length;
    if (share >= bestShare) {
      best = ch;
      bestShare = share;
    }
  }
  return best;
}

/** Height (0-1) of an access point drawn at `rssi` dBm: -90 dBm and
 *  quieter at the floor, -30 dBm at the top */
export function apLevel(rssi: number): number {
  return Math.min(1, Math.max(0, (rssi + 90) / 60));
}
