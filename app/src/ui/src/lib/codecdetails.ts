// What Diagnostics' codec details card shows: each codec setting with what
// the device offered, the SBC bitpool bar's scale and how full a packet is.

import type { CodecDetails, StreamInfo } from "../ipc/types";

export type SbcDetails = NonNullable<CodecDetails["sbc"]>;
export type AacDetails = NonNullable<CodecDetails["aac"]>;

/** "used": what the stream runs with; "offered": the device takes it too;
 *  "off": the device does not take it */
export type ChipState = "used" | "offered" | "off";

export interface Chip<T> {
  value: T;
  state: ChipState;
}

/** Every value of a setting with its state, in the order of `all`. Without
 *  what the device offered (it did not say), only the value in use. */
export function chips<T>(all: readonly T[], offered: readonly T[] | null | undefined, used: T): Chip<T>[] {
  if (!offered) return [{ value: used, state: "used" }];
  const list = all.includes(used) ? all : [...all, used];
  return list.map((value) => ({ value, state: value === used ? "used" : offered.includes(value) ? "offered" : "off" }));
}

// The values of the SBC codec information (A2DP 4.3.2)
export const SBC_RATES = [16000, 32000, 44100, 48000];
export const SBC_MODES = ["mono", "dual_channel", "stereo", "joint_stereo"];
export const SBC_BLOCKS = [4, 8, 12, 16];
export const SBC_SUBBANDS = [4, 8];
export const SBC_ALLOCATIONS = ["loudness", "snr"];
export const AAC_CHANNELS = [1, 2];

/** The bitpool bar: its end (64, or the largest value shown rounded up to
 *  16) and where each range and the bitpool in use sit, in % of the bar */
export interface BitpoolBar {
  max: number;
  device: { from: number; width: number } | null;
  config: { from: number; width: number } | null;
  /** SBC ABR: the range the bitpool moves in */
  abr: { from: number; width: number } | null;
  used: number;
}

export function bitpoolBar(s: SbcDetails): BitpoolBar {
  const top = Math.max(64, s.device?.max_bitpool ?? 0, s.config_bitpool?.[1] ?? 0, s.abr_bitpool?.[1] ?? 0, s.bitpool);
  const max = Math.ceil(top / 16) * 16;
  const pct = (v: number) => (Math.min(Math.max(v, 0), max) / max) * 100;
  const span = (lo: number, hi: number) => ({ from: pct(lo), width: Math.max(pct(hi) - pct(lo), 0) });
  return {
    max,
    device: s.device ? span(s.device.min_bitpool, s.device.max_bitpool) : null,
    config: s.config_bitpool ? span(s.config_bitpool[0], s.config_bitpool[1]) : null,
    abr: s.abr_bitpool ? span(s.abr_bitpool[0], s.abr_bitpool[1]) : null,
    used: pct(s.bitpool),
  };
}

/**
 * SBC / SBC XQ under ABR: the bitpool in use now and the range it moves in
 * (floor .. the bitpool the stream started with). The core sends a new
 * StreamInfo while it changes. null without SBC ABR (a fixed bitpool, or
 * another codec).
 */
export function liveBitpool(info: StreamInfo): { now: number; min: number; max: number } | null {
  const s = info.details?.sbc;
  if (!info.abr || !s?.abr_bitpool) return null;
  return { now: s.bitpool, min: s.abr_bitpool[0], max: s.abr_bitpool[1] };
}

/** A codec frame's duration in ms (0 while the rate is not known) */
export function frameMs(samples: number, sampleRate: number): number {
  return sampleRate > 0 ? (samples * 1000) / sampleRate : 0;
}

/** Bytes a full packet carries (media payload header and frames) and what
 *  it may carry; null when the frames do not have a fixed length */
export function packetFill(d: CodecDetails): { used: number; of: number } | null {
  if (d.frames_per_packet == null || d.frame_bytes == null || d.mtu <= 0) return null;
  return { used: d.media_header + d.frames_per_packet * d.frame_bytes, of: d.mtu };
}

/** 44100 -> "44.1 kHz" */
export function khz(rate: number): string {
  return `${rate / 1000} kHz`;
}
