// The codecs a profile can ask for and what each allows, as v1's profile
// dialog (wx_profile_dialog.cpp on the develop branch, update_codec_dependent): quality
// modes with their bitrates (since issue #86 High / Medium / Low for LDAC
// and LHDC V5, LHDC V5's Medium-High too (issue #203), High / Low Latency for aptX Adaptive, only High, the highest
// bit rate the device takes, for the others; core: Quality::for_codec),
// ABR only for LDAC / LHDC V5 (and Auto, which may
// pick them; v2 adds SBC / SBC XQ's adaptive bitpool), 24-bit and 88.2 / 96 kHz only for LDAC and LHDC V5, 192 kHz and
// "mhq" only for LHDC V5, no quality choice for the aptX family (but aptX
// Adaptive) or under LHDC V5 ABR, 48 kHz only for Opus and Opus 05, 48 / 96
// kHz for aptX Adaptive, whose "ll" quality is its low latency mode.
// aptX HD and aptX Adaptive always carry 24-bit (no choice).

export type CodecId =
  | "auto"
  | "ldac"
  | "aptxhd"
  | "aptxll"
  | "aptx"
  | "aptxad"
  | "lhdcv5"
  | "opus"
  | "aac"
  | "sbcxq"
  | "sbc"
  | "faststream"
  | "opus05";

export interface QualityMode {
  id: string;
  /** Translation key of the name */
  label: string;
  kbps?: string;
}

export interface CodecSpec {
  id: CodecId;
  /** Translation key of the name */
  label: string;
  /** 1..5 for the comparison bars (Auto has none: it depends on the device) */
  bars?: { quality: number; latency: number; stability: number };
  /** Quality modes; empty: the codec has a fixed bitrate */
  qualities: QualityMode[];
  abr: boolean;
  /** 24-bit and 88.2 / 96 kHz */
  hiRes: boolean;
  /** The only capture rates it takes (0 = auto), when not the usual ones */
  rates?: number[];
  /** The bit depth it always carries when there is no choice (16 if not set) */
  bits?: number;
  /** Variable bit rate (where the headphones list it), on the "abr" setting */
  vbr?: boolean;
  /** The SBC bitpool and frame setup (blocks, subbands, allocation) can be chosen */
  bitpool?: boolean;
  /** Not yet confirmed with real headphones (the README's 🧪; core:
   *  CodecKind::experimental). The app offers and uses them as any other
   *  (issue #86); only the CLI's --no-experimental leaves them out */
  experimental?: boolean;
}

/** SBC's largest bitpool (the local SEP's) */
export const BITPOOL_MAX = 53;
/** SBC XQ is dual channel: its largest bitpool per channel (44.1 kHz; 43 at 48 kHz) */
export const BITPOOL_MAX_XQ = 47;
export const BITPOOL_MIN = 2;

const HQ = "quality.high";
const MEDIUM_HIGH = "quality.medium_high";
const MEDIUM = "quality.medium";
const LOW = "quality.low";
const LL = "quality.low_latency";

export const CODECS: CodecSpec[] = [
  {
    id: "auto",
    label: "codec.auto",
    // Auto-select takes the best options (issue #86)
    qualities: [{ id: "hq", label: HQ }],
    abr: true,
    hiRes: false,
  },
  {
    id: "ldac",
    label: "codec.ldac",
    bars: { quality: 5, latency: 2, stability: 2 },
    qualities: [
      { id: "hq", label: HQ, kbps: "990" },
      { id: "sq", label: MEDIUM, kbps: "660" },
      { id: "mq", label: LOW, kbps: "330" },
    ],
    abr: true,
    hiRes: true,
  },
  {
    id: "lhdcv5",
    label: "codec.lhdc_v5",
    bars: { quality: 5, latency: 2, stability: 2 },
    // Android's four fixed levels (the sink may cap the maximum). Medium-
    // High was left out in issue #86 and is back since issue #203
    qualities: [
      { id: "hq", label: HQ, kbps: "1000" },
      { id: "mhq", label: MEDIUM_HIGH, kbps: "900" },
      { id: "sq", label: MEDIUM, kbps: "500" },
      { id: "mq", label: LOW, kbps: "400" },
    ],
    abr: true,
    hiRes: true,
    rates: [0, 44100, 48000, 88200, 96000, 192000],
  },
  {
    id: "aptxhd",
    label: "codec.aptx_hd",
    bars: { quality: 4, latency: 3, stability: 3 },
    qualities: [],
    abr: false,
    hiRes: false,
    bits: 24,
    experimental: true,
  },
  { id: "aptx", label: "codec.aptx", bars: { quality: 3, latency: 3, stability: 4 }, qualities: [], abr: false, hiRes: false },
  {
    id: "aptxll",
    label: "codec.aptx_ll",
    bars: { quality: 2, latency: 5, stability: 4 },
    qualities: [],
    abr: false,
    hiRes: false,
    experimental: true,
  },
  {
    // aptX Adaptive: High with an R1 sink
    // (an R2 sink: 427 -> 340), 48 kHz; ABR steps down from High; Low
    // Latency has a fixed bit rate. 96 kHz when chosen, to a device that
    // lists it (an R2 one), not in Low Latency (48 kHz otherwise; 646
    // kbps). Always 24-bit.
    id: "aptxad",
    label: "codec.aptx_adaptive",
    bars: { quality: 4, latency: 3, stability: 4 },
    qualities: [
      { id: "hq", label: HQ, kbps: "427" },
      { id: "ll", label: LL, kbps: "265" },
    ],
    abr: true,
    hiRes: false,
    rates: [0, 48000, 96000],
    bits: 24,
  },
  {
    id: "opus",
    label: "codec.opus",
    bars: { quality: 4, latency: 3, stability: 4 },
    // Android's A2DP Opus: 256 kbps stereo (mono: half), 48 kHz, 20 ms frames
    qualities: [{ id: "hq", label: HQ, kbps: "256" }],
    abr: false,
    hiRes: false,
    rates: [0, 48000],
    experimental: true,
  },
  {
    id: "aac",
    label: "codec.aac",
    bars: { quality: 3, latency: 2, stability: 4 },
    qualities: [{ id: "hq", label: HQ, kbps: "256" }],
    abr: false,
    vbr: true,
    hiRes: false,
  },
  {
    // Dual channel, high bitpool (PipeWire's SBC-XQ); only on sinks whose
    // SBC allows it
    id: "sbcxq",
    label: "codec.sbc_xq",
    bars: { quality: 3, latency: 3, stability: 3 },
    // High: the highest bitpool the device takes (or the one set)
    qualities: [{ id: "hq", label: HQ, kbps: "~552" }],
    // Adaptive bitpool, down to Mobile's (core: SBC ABR only when the
    // profile asks for SBC / SBC XQ itself)
    abr: true,
    bitpool: true,
    hiRes: false,
  },
  {
    id: "sbc",
    label: "codec.sbc",
    bars: { quality: 2, latency: 3, stability: 5 },
    qualities: [{ id: "hq", label: HQ, kbps: "~345" }],
    abr: true,
    bitpool: true,
    hiRes: false,
  },
  // CSR FastStream: fixed SBC (bitpool 29, ~200 kbps), music only; never
  // picked by Auto
  {
    id: "faststream",
    label: "codec.faststream",
    bars: { quality: 1, latency: 4, stability: 4 },
    qualities: [],
    abr: false,
    hiRes: false,
    experimental: true,
  },
  {
    // PipeWire's own Opus ("Opus 05"; never picked by Auto):
    // Android Opus' levels, stereo, 48 kHz, 10 ms frames when offered
    id: "opus05",
    label: "codec.opus05",
    bars: { quality: 4, latency: 3, stability: 3 },
    qualities: [{ id: "hq", label: HQ, kbps: "256" }],
    abr: false,
    hiRes: false,
    rates: [0, 48000],
    experimental: true,
  },
];

export function codecSpec(id: string): CodecSpec {
  return CODECS.find((c) => c.id === id) ?? CODECS[0];
}

/** Translation key of a codec name as the core reports it ("LDAC", "aptX HD") or a profile id */
export function codecLabelKey(name: string): string {
  const n = name.toLowerCase().replace(/[\s_-]/g, "");
  const map: Record<string, string> = {
    auto: "codec.auto",
    ldac: "codec.ldac",
    aptx: "codec.aptx",
    aptxhd: "codec.aptx_hd",
    aptxll: "codec.aptx_ll",
    aptxlowlatency: "codec.aptx_ll",
    aptxad: "codec.aptx_adaptive",
    aptxadaptive: "codec.aptx_adaptive",
    aac: "codec.aac",
    sbc: "codec.sbc",
    sbcxq: "codec.sbc_xq",
    faststream: "codec.faststream",
    lhdcv5: "codec.lhdc_v5",
    lhdc: "codec.lhdc_v5",
    opus: "codec.opus",
    opus05: "codec.opus05",
    // LE Audio (StreamInfo.le)
    lc3: "ui.codec.lc3",
  };
  return map[n] ?? "";
}

/**
 * Whether a device offers a codec, from the codec keys the core learnt when
 * it last connected (Device.codecs: "ldac", "aac", ...; empty: not known
 * until the first connection). Auto takes what the device offers. One it may
 * offer only once chosen (`onRequest`, Device.codecs_on_request) is not known
 * until then (issue #30).
 */
export type CodecSupport = "offered" | "not_offered" | "unknown";

export function codecSupport(codec: string, known: readonly string[], onRequest: readonly string[] = []): CodecSupport {
  if (known.length === 0) return "unknown";
  if (codec === "auto") return "offered";
  const id = codec.toLowerCase();
  if (known.some((k) => k.toLowerCase() === id)) return "offered";
  return onRequest.includes(id) ? "unknown" : "not_offered";
}

/**
 * Auto's choice, highest sound quality first (core: CodecKind::AUTO_PRIORITY,
 * issue #86). FastStream and Opus 05 are used only when chosen.
 */
export const AUTO_ORDER: readonly CodecId[] = [
  "lhdcv5",
  "ldac",
  "aptxad",
  "opus",
  "aptxhd",
  "aptx",
  "aptxll",
  "aac",
  "sbcxq",
  "sbc",
];

/**
 * The codec keys of `keys` that Auto and the fallback take (those of
 * AUTO_ORDER) in the order given, then any missing in their default order
 * (core: AutoOrder::from_keys). Empty: AUTO_ORDER, highest sound quality
 * first.
 */
export function autoOrder(keys: readonly string[] = []): CodecId[] {
  const order: CodecId[] = [];
  for (const k of [...keys.map((k) => k.toLowerCase()), ...AUTO_ORDER]) {
    if (AUTO_ORDER.includes(k as CodecId) && !order.includes(k as CodecId)) order.push(k as CodecId);
  }
  return order;
}

/**
 * The default order of a device's codec cards: AUTO_ORDER, with Opus 05
 * beside Opus and FastStream last (Auto and the fallback never take those
 * two)
 */
export const CARD_ORDER: readonly CodecId[] = [
  "lhdcv5",
  "ldac",
  "aptxad",
  "opus",
  "opus05",
  "aptxhd",
  "aptx",
  "aptxll",
  "aac",
  "sbcxq",
  "sbc",
  "faststream",
];

/**
 * A device's codec cards in its order (the profile's codec_order, issue #86
 * round 8: dragged with Auto-select off): the codecs given (each once,
 * unknown keys left out), then any missing in CARD_ORDER's order (as the
 * core's AutoOrder::from_keys). Empty: CARD_ORDER.
 */
export function cardOrder(keys: readonly string[] = []): CodecId[] {
  const order: CodecId[] = [];
  for (const k of [...keys.map((k) => k.toLowerCase()), ...CARD_ORDER]) {
    if (CARD_ORDER.includes(k as CodecId) && !order.includes(k as CodecId)) order.push(k as CodecId);
  }
  return order;
}

/** The cards' order is the default one */
export function isDefaultOrder(keys: readonly string[]): boolean {
  return cardOrder(keys).every((c, i) => c === CARD_ORDER[i]);
}

/** `list` with the item at `from` moved to `to` */
export function moved<T>(list: readonly T[], from: number, to: number): T[] {
  const next = [...list];
  const [item] = next.splice(from, 1);
  next.splice(to, 0, item);
  return next;
}

/** Not yet confirmed with real headphones (CodecSpec.experimental) */
export function isExperimental(codec: string): boolean {
  return codecSpec(codec).experimental === true;
}

/** Offered by the device and allowed (an experimental codec only with `experimental`) */
function usable(codec: string, known: readonly string[], experimental: boolean): boolean {
  return codecSupport(codec, known) === "offered" && (experimental || !isExperimental(codec));
}

/** The codec Auto picks among those a device offers (`known`, as
 *  Device.codecs): the first in `order` (the device's, AUTO_ORDER unless
 *  given; never FastStream nor Opus 05), leaving out the experimental ones
 *  unless `experimental`; null when not known */
export function autoPick(
  known: readonly string[],
  experimental = true,
  order: readonly string[] = AUTO_ORDER,
): CodecId | null {
  return autoOrder(order).find((c) => usable(c, known, experimental)) ?? null;
}

/**
 * The codec a connection uses for `codec` chosen on a device that offers
 * `known` (core: caps::choose_in with the fallback every profile has): the
 * codec itself when offered, else the first in the device's `order`
 * (AUTO_ORDER unless given) it offers, as Auto picks (issue #86 round 8:
 * not the next one below the codec chosen). Null when the device's codecs
 * are not known yet. Unless `experimental`, the experimental codecs are
 * never used (a chosen one falls back the same way).
 */
export function effectiveCodec(
  codec: string,
  known: readonly string[],
  experimental = true,
  order: readonly string[] = AUTO_ORDER,
): CodecId | null {
  if (known.length === 0) return null;
  if (codec !== "auto" && usable(codec, known, experimental)) return codec as CodecId;
  return autoPick(known, experimental, order);
}

export function sampleRates(codec: string): number[] {
  const spec = codecSpec(codec);
  return spec.rates ?? (spec.hiRes ? [0, 44100, 48000, 88200, 96000] : [0, 44100, 48000]);
}

/** What a device's endpoint of a codec lists (Device.codec_caps) */
export interface CodecCaps {
  codec: string;
  sample_rates: number[];
  bit_depths: number[];
  max_bitrate: number;
}

/** The caps of `codec` among a device's (null: not known) */
export function capsOf(caps: readonly CodecCaps[], codec: string): CodecCaps | null {
  return caps.find((c) => c.codec.toLowerCase() === codec.toLowerCase()) ?? null;
}

/** Whether the device lists a sample rate (anything goes while not known;
 *  LHDC V5's 88.2 kHz runs as 96 kHz). aptX Adaptive's 96 kHz only when
 *  the device lists it: few do (an R2 one with a 96 kHz bit; issue #86, a
 *  Bose QC Ultra has none), and the core streams 48 kHz to the others */
export function rateOffered(codec: string, rate: number, caps: CodecCaps | null): boolean {
  if (rate === 0) return true;
  if (codec === "aptxad" && rate === 96000) return caps?.sample_rates.includes(rate) ?? false;
  if (!caps || caps.sample_rates.length === 0) return true;
  return caps.sample_rates.includes(codec === "lhdcv5" && rate === 88200 ? 96000 : rate);
}

/** The sample rates the dialog offers (issue #86): the codec's, as far as
 *  the device lists them */
export function offeredSampleRates(codec: string, caps: CodecCaps | null): number[] {
  return sampleRates(codec).filter((r) => rateOffered(codec, r, caps));
}

/** The bit depths the dialog offers: the codec's, as far as the device
 *  lists them (LHDC V5 says) */
export function offeredBitDepths(codec: string, caps: CodecCaps | null): number[] {
  const listed = caps?.bit_depths ?? [];
  return bitDepths(codec).filter((b) => b === 0 || listed.length === 0 || listed.includes(b));
}

/**
 * The profile's ABR flag means AAC's VBR under AAC, the adaptive bitpool
 * under SBC and under SBC XQ (each its own: issue #86, SBC's turned XQ's on),
 * and LDAC's / LHDC V5's ABR elsewhere: switching between those leaves it
 * off rather than carrying it over (an LDAC profile with ABR moved to SBC
 * does not start adapting its bitpool unasked).
 */
export function abrCarries(from: string, to: string): boolean {
  const family = (c: string) => (c === "aac" || c === "sbc" || c === "sbcxq" ? c : "abr");
  return family(from) === family(to);
}

/**
 * A codec's starting values from another codec's (`p.codec`): what does
 * not carry over (ABR across families, SBC's frame setup to SBC XQ and
 * back) is left at the default, then the values are fitted to the codec
 */
export function carryToCodec<T extends CodecFields>(p: T, to: string): T {
  const same = p.codec === to;
  return fitToCodec({
    ...p,
    codec: to,
    abr: abrCarries(p.codec, to) ? p.abr : false,
    ...(same || p.sbc_blocks === undefined ? {} : { sbc_blocks: 0 }),
    ...(same || p.sbc_subbands === undefined ? {} : { sbc_subbands: 0 }),
    ...(same || p.sbc_allocation === undefined ? {} : { sbc_allocation: "" }),
  });
}

/** LHDC V5 ABR runs at 400 kbps or below whatever the quality (as on
 *  Android): the quality choice is greyed out then */
export function qualityIgnored(codec: string, abr: boolean): boolean {
  return codec === "lhdcv5" && abr;
}

/** LDAC's rates at 44.1 / 88.2 kHz, LHDC V5's Standard at 44.1 kHz (88.2
 *  kHz runs as 96 kHz there): v1's profile list */
const RATE_44K: Partial<Record<CodecId, Record<string, string>>> = {
  ldac: { hq: "909", sq: "606", mq: "303" },
  lhdcv5: { sq: "480" },
};
/** aptX Adaptive at 96 kHz (an R2 device's 96 kHz levels; Low Latency runs
 *  at 48 kHz) */
const RATE_96K: Partial<Record<CodecId, Record<string, string>>> = {
  aptxad: { hq: "646" },
};

/** A quality's bit rate at the profile's sample rate (0 = auto), at most
 *  the device's largest (`maxBitrate` in bps, 0 = not known: AAC's, LHDC
 *  V5's; the core caps it the same way, issue #86) */
export function qualityKbps(codec: string, quality: string, sampleRate: number, maxBitrate = 0): string | undefined {
  const spec = codecSpec(codec);
  const q = spec.qualities.find((x) => x.id === quality) ?? spec.qualities[0];
  if (!q) return undefined;
  const is441 = codec === "lhdcv5" ? sampleRate === 44100 : sampleRate === 44100 || sampleRate === 88200;
  const table = is441 ? RATE_44K : sampleRate === 96000 ? RATE_96K : {};
  const kbps = table[spec.id]?.[q.id] ?? q.kbps;
  const cap = Math.floor(maxBitrate / 1000);
  return kbps && cap > 0 && /^\d+$/.test(kbps) && Number(kbps) > cap ? String(cap) : kbps;
}

/**
 * The qualities the dialog offers for a codec on a device whose largest
 * bit rate is `maxBitrate` (bps, 0 = not known), each with its bit rate
 * there: one the device caps to the same bit rate as a higher one is left
 * out (LHDC V5 at most 400 kbps: High only), unless it is `current`
 */
export function offeredQualities(
  codec: string,
  sampleRate: number,
  maxBitrate = 0,
  current = "",
): { id: string; label: string; kbps?: string }[] {
  const out: { id: string; label: string; kbps?: string }[] = [];
  for (const q of codecSpec(codec).qualities) {
    const kbps = qualityKbps(codec, q.id, sampleRate, maxBitrate);
    if (q.id !== current && kbps && out.some((o) => o.kbps === kbps)) continue;
    out.push({ id: q.id, label: q.label, kbps });
  }
  return out;
}

/**
 * The quality of a profile as the profile list shows it (v1's
 * rebuild_profile_list): "High (990 kbps)", "ABR (≤400 kbps)" under LHDC V5
 * ABR; null when the codec has no quality choice (aptX family, FastStream,
 * LE Audio). `label` translates the quality's key.
 */
export function qualityTag(
  p: { codec: string; quality: string; abr: boolean; sample_rate: number; le_audio?: boolean },
  label: (key: string) => string,
): string | null {
  if (p.le_audio) return null;
  const spec = codecSpec(p.codec);
  if (spec.qualities.length === 0) return null;
  if (qualityIgnored(p.codec, p.abr)) return "ABR (≤400 kbps)";
  const q = spec.qualities.find((x) => x.id === p.quality) ?? spec.qualities[0];
  const kbps = qualityKbps(p.codec, q.id, p.sample_rate);
  return kbps ? `${label(q.label)} (${kbps} kbps)` : label(q.label);
}

/** Why a stream's bit rate is below the profile's, appended to the codec
 *  line: ABR (LHDC V5 stays at 400 kbps or below), or the device's maximum */
export function bitrateNoteKey(info: { abr: boolean; bitrate_capped: boolean }): string | null {
  if (info.abr) return "quality.codec_abr";
  if (info.bitrate_capped) return "quality.codec_capped";
  return null;
}

/** 0 = auto. Auto may pick LDAC / LHDC V5, so it offers 16 / 24 too (the
 *  core carries 24-bit only when the codec it picks has it) */
export function bitDepths(codec: string): number[] {
  const spec = codecSpec(codec);
  return spec.hiRes || spec.id === "auto" ? [0, 16, 24] : [0];
}

/** The bit depth a codec without a choice carries (aptX HD and aptX
 *  Adaptive 24, the others 16) */
export function fixedBitDepth(codec: string): number {
  return codecSpec(codec).bits ?? 16;
}

interface CodecFields {
  codec: string;
  quality: string;
  abr: boolean;
  sample_rate: number;
  bit_depth: number;
  sbc_bitpool?: number;
  sbcxq_bitpool?: number;
  sbc_min_bitpool?: number;
  sbcxq_min_bitpool?: number;
  sbc_blocks?: number;
  sbc_subbands?: number;
  sbc_allocation?: string;
  sbc_channel_mode?: string;
}

/** Values the codec cannot use go back to the defaults (v1 did the same when the codec changed) */
export function fitToCodec<T extends CodecFields>(p: T): T {
  const spec = codecSpec(p.codec);
  const quality = spec.qualities.length === 0 || spec.qualities.some((m) => m.id === p.quality) ? p.quality : "hq";
  // The frame setup is SBC's and SBC XQ's; the channel mode SBC's only (XQ
  // is dual channel, and plain SBC has no dual channel: issue #86)
  const sbc = p.codec === "sbc" || p.codec === "sbcxq";
  return {
    ...p,
    quality,
    abr: spec.abr || spec.vbr ? p.abr : false,
    sample_rate: sampleRates(p.codec).includes(p.sample_rate) ? p.sample_rate : 0,
    bit_depth: bitDepths(p.codec).includes(p.bit_depth) ? p.bit_depth : 0,
    ...(p.sbc_bitpool === undefined ? {} : { sbc_bitpool: p.codec === "sbc" ? p.sbc_bitpool : 0 }),
    ...(p.sbcxq_bitpool === undefined ? {} : { sbcxq_bitpool: p.codec === "sbcxq" ? p.sbcxq_bitpool : 0 }),
    ...(p.sbc_min_bitpool === undefined ? {} : { sbc_min_bitpool: p.codec === "sbc" ? p.sbc_min_bitpool : 0 }),
    ...(p.sbcxq_min_bitpool === undefined ? {} : { sbcxq_min_bitpool: p.codec === "sbcxq" ? p.sbcxq_min_bitpool : 0 }),
    ...(p.sbc_blocks === undefined ? {} : { sbc_blocks: sbc ? p.sbc_blocks : 0 }),
    ...(p.sbc_subbands === undefined ? {} : { sbc_subbands: sbc ? p.sbc_subbands : 0 }),
    ...(p.sbc_allocation === undefined ? {} : { sbc_allocation: sbc ? p.sbc_allocation : "" }),
    ...(p.sbc_channel_mode === undefined ? {} : { sbc_channel_mode: p.codec === "sbc" && p.sbc_channel_mode !== "dual_channel" ? p.sbc_channel_mode : "" }),
  };
}

// Max media packet size (crates/bt-host/src/media.rs, MEDIA_PAYLOAD_LIMIT_MIN / MAX / DEFAULT)
export const PACKET_MIN = 679;
export const PACKET_MAX = 1679;
export const PACKET_DEFAULT = 1023;
