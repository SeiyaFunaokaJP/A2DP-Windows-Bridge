// The SBC frame setup a profile may ask for (bitpool; in developer mode also
// blocks, subbands, allocation, channel mode) and the bit rate it gives, as
// the core works it out
// (crates/codec/src/sbc/mod.rs, caps.rs RemoteCaps::sbc_params): a value the
// headphones do not list falls back to the default when connecting, Auto
// takes the default, the bitpool stays within the headphones' range.

import type { Device, Profile } from "../ipc/types";
import { parseAddress } from "./format";

export const SBC_BLOCKS = [4, 8, 12, 16];
export const SBC_SUBBANDS = [4, 8];
export const SBC_ALLOCATIONS = ["loudness", "snr"];
/** SBC only: dual channel is SBC XQ's, which is always that (issue #86) */
export const SBC_CHANNEL_MODES = ["joint_stereo", "stereo", "mono"];

/** What a device's SBC endpoint offers (Device.sbc, CodecDetails' SbcDeviceCaps) */
export type SbcCaps = NonNullable<Device["sbc"]>;

export type SbcFields = Pick<
  Profile,
  | "codec"
  | "quality"
  | "sample_rate"
  | "sbc_bitpool"
  | "sbcxq_bitpool"
  | "sbc_blocks"
  | "sbc_subbands"
  | "sbc_allocation"
  | "sbc_channel_mode"
> &
  Partial<Pick<Profile, "sbc_min_bitpool" | "sbcxq_min_bitpool">>;

/** What the device listed when it last connected (null: not known) */
export function knownSbcCaps(devices: readonly Device[], address: string): SbcCaps | null {
  const a = parseAddress(address);
  if (!a) return null;
  return devices.find((d) => !d.le_audio && d.address.toUpperCase() === a)?.sbc ?? null;
}

/** Whether the device lists a value (anything goes while not known) */
export function sbcOffered(caps: SbcCaps | null, field: "blocks" | "subbands" | "allocations" | "channel_modes", value: number | string): boolean {
  return !caps || (caps[field] as (number | string)[]).includes(value);
}

/** The joint stereo bitpool of a quality (mono and stereo the same) */
function qualityBitpool(quality: string): number {
  return quality === "sq" ? 35 : quality === "mq" ? 19 : 53;
}

/** SBC XQ's bitpool per channel of a quality at a rate */
function xqBitpool(quality: string, rate: number): number {
  const [at44, at48] = quality === "sq" ? [43, 39] : quality === "mq" ? [32, 29] : [47, 43];
  return rate === 44100 ? at44 : at48;
}

/**
 * Bytes of one SBC frame: the A2DP specification's formula (12.9), with the
 * bitpool limited to what the channel mode allows
 */
export function sbcFrameLength(mode: string, blocks: number, subbands: number, bitpool: number): number {
  const shared = mode === "stereo" || mode === "joint_stereo";
  const bp = Math.min(bitpool, shared ? (subbands === 8 ? 255 : 128) : 16 * subbands);
  const ch = mode === "mono" ? 1 : 2;
  const data = mode === "joint_stereo" ? subbands + blocks * bp : mode === "stereo" ? blocks * bp : blocks * ch * bp;
  return 4 + Math.floor((4 * subbands * ch) / 8) + Math.ceil(data / 8);
}

/** kbps of frames of `len` bytes, each blocks x subbands samples (rounded down, as the core) */
export function sbcKbps(len: number, rate: number, blocks: number, subbands: number): number {
  return Math.floor((8 * len * rate) / (blocks * subbands) / 1000);
}

export interface SbcSetup {
  blocks: number;
  subbands: number;
  allocation: string;
  mode: string;
  bitpool: number;
  frameBytes: number;
  kbps: number;
}

/**
 * The setup a profile streams SBC / SBC XQ with at a rate (stereo input):
 * what the device does not list taken as the default
 */
export function sbcSetup(p: SbcFields, rate: number, caps: SbcCaps | null): SbcSetup {
  const xq = p.codec === "sbcxq";
  const blocks = p.sbc_blocks && sbcOffered(caps, "blocks", p.sbc_blocks) ? p.sbc_blocks : 16;
  const subbands = p.sbc_subbands && sbcOffered(caps, "subbands", p.sbc_subbands) ? p.sbc_subbands : 8;
  const allocation = p.sbc_allocation && sbcOffered(caps, "allocations", p.sbc_allocation) ? p.sbc_allocation : "loudness";
  const mode = xq
    ? "dual_channel"
    : SBC_CHANNEL_MODES.includes(p.sbc_channel_mode) && sbcOffered(caps, "channel_modes", p.sbc_channel_mode)
      ? p.sbc_channel_mode
      : "joint_stereo";
  const max = xq ? xqBitpool("hq", rate) : 53;
  const asked = xq ? p.sbcxq_bitpool : p.sbc_bitpool;
  const auto = xq ? xqBitpool(p.quality, rate) : qualityBitpool(p.quality);
  // Never below the profile's minimum (issue #86)
  const min = (xq ? p.sbcxq_min_bitpool : p.sbc_min_bitpool) ?? 0;
  let bitpool = Math.min(Math.max(asked || auto, min), max);
  if (caps && caps.max_bitpool >= 2) bitpool = Math.max(Math.min(bitpool, caps.max_bitpool), Math.min(caps.min_bitpool, caps.max_bitpool), 2);
  const frameBytes = sbcFrameLength(mode, blocks, subbands, bitpool);
  return { blocks, subbands, allocation, mode, bitpool, frameBytes, kbps: sbcKbps(frameBytes, rate, blocks, subbands) };
}

/** The rate the bit rate is shown at: the profile's, or on Auto 48 kHz,
 *  44.1 kHz on a device without it, as the core streams it (one figure is
 *  enough, issue #86) */
export function sbcRate(p: Pick<Profile, "sample_rate">, caps: Pick<SbcCaps, "sample_rates"> | null): number {
  if (p.sample_rate === 44100 || p.sample_rate === 48000) return p.sample_rate;
  return !caps || caps.sample_rates.includes(48000) || !caps.sample_rates.includes(44100) ? 48000 : 44100;
}

/** "≈ 552 kbps": the bit rate SBC / SBC XQ streams with at that rate */
export function sbcKbpsText(p: SbcFields, caps: SbcCaps | null): string {
  return `≈ ${sbcSetup(p, sbcRate(p, caps), caps).kbps} kbps`;
}
