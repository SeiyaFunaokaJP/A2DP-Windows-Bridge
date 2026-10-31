// A new profile's defaults and the checks before profile.save. The values
// are those of v1's profile dialog; the core checks again.
//
// Since the Codecs screen (issue #86) the UI keeps one profile per device
// entry (its address, and whether it is the LE Audio entry): the codec card
// chosen changes that profile, and the settings of the codecs it is not
// using wait in its codec_settings. The core folds older extra profiles
// into it at start (crates/core/src/profiles.rs consolidate).

import type { CodecValues } from "../ipc/bindings/CodecValues";
import type { AudioDevice, Device, ParamsOf, Profile, Snapshot } from "../ipc/types";
import { carryToCodec, fitToCodec, PACKET_DEFAULT, PACKET_MAX, PACKET_MIN, type CodecCaps } from "./codecs";
import { parseAddress } from "./format";

/** What belongs to the audio device rather than the codec: the capture
 *  source. Set on the Audio Devices screen and kept the same in all the
 *  device's profiles (issue #86). The max media packet size is the device's
 *  too, set on the Developer page; a profile's own Windows Volume Sync is
 *  no longer used (Settings.volume_sync, for every device). */
export const DEVICE_FIELDS = [
  "capture_mode",
  "audio_device_id",
  "audio_device_name",
  "auto_switch_device",
  "app_exe",
  "app_name",
] as const;

export type DeviceFields = Pick<Profile, (typeof DEVICE_FIELDS)[number]>;

export function deviceFields(p: Profile): DeviceFields {
  return Object.fromEntries(DEVICE_FIELDS.map((k) => [k, p[k]])) as DeviceFields;
}

/** The device's profiles (address in any case) */
export function profilesOf(profiles: readonly Profile[], address: string): Profile[] {
  const a = address.toUpperCase();
  return profiles.filter((p) => p.device_address.toUpperCase() === a);
}

/** A device's settings are complete: a virtual device or an app chosen when captured from */
export function deviceFieldsComplete(f: DeviceFields): boolean {
  if (f.capture_mode === "virtual") return f.audio_device_id !== "";
  if (f.capture_mode === "app") return f.app_exe !== "";
  return true;
}

/** The device's settings as its profiles have them (the first one's), for a new profile of it */
export function deviceDefaults(profiles: readonly Profile[], address: string): DeviceFields | null {
  const p = profilesOf(profiles, address)[0];
  return p ? deviceFields(p) : null;
}

export function newProfile(): Profile {
  return {
    name: "",
    device_address: "",
    device_name: "",
    codec: "auto",
    quality: "hq",
    abr: false,
    codec_fallback: true,
    sample_rate: 0,
    bit_depth: 0,
    // Every new device captures System Loopback (issue #86: it plays with
    // nothing installed); a virtual device or one app is chosen on Audio
    // Devices
    capture_mode: "loopback",
    audio_device_id: "",
    audio_device_name: "",
    // As v1 and the core: follow the virtual device as the default output
    auto_switch_device: true,
    app_exe: "",
    app_name: "",
    volume_sync: "auto",
    volume_device_id: "",
    volume_device_name: "",
    max_media_payload: PACKET_DEFAULT,
    le_audio: false,
    le_low_latency: true,
    sbc_bitpool: 0,
    sbcxq_bitpool: 0,
    sbc_min_bitpool: 0,
    sbcxq_min_bitpool: 0,
    sbc_blocks: 0,
    sbc_subbands: 0,
    sbc_allocation: "",
    sbc_channel_mode: "",
    codec_settings: {},
    codec_order: [],
  };
}

/** One saved audio device as the Codecs screen, Home and the sidebar list
 *  it: its entry and its profile (None until a codec is chosen or it is
 *  connected) */
export interface AudioEntry {
  key: string;
  address: string;
  name: string;
  le_audio: boolean;
  device: Device;
  profile: Profile | null;
}

/** The profile of a device entry: the one streaming, else the active one,
 *  else the first (the core keeps one per entry; more only until it starts
 *  again) */
export function deviceProfile(
  snap: Pick<Snapshot, "profiles" | "active_profile" | "stream">,
  address: string,
  leAudio: boolean,
): Profile | null {
  const mine = profilesOf(snap.profiles, address).filter((p) => p.le_audio === leAudio);
  const streaming = snap.stream.state !== "idle" ? snap.stream.profile : null;
  return (
    mine.find((p) => p.name === streaming) ?? mine.find((p) => p.name === snap.active_profile) ?? mine[0] ?? null
  );
}

/** The saved devices with their profiles, in the saved list's order; LE
 *  Audio entries only with LE Audio (experimental) on. A forgotten device's
 *  profile (Audio Devices leaves it) gets no entry. */
export function audioEntries(snap: Pick<Snapshot, "profiles" | "active_profile" | "stream" | "devices" | "settings">): AudioEntry[] {
  return listedDevices(
    snap.devices.filter((d) => d.saved),
    snap.settings.le_audio_experimental,
  ).map((d) => ({
    key: deviceKey(d),
    address: d.address.toUpperCase(),
    name: d.name,
    le_audio: d.le_audio,
    device: d,
    profile: deviceProfile(snap, d.address, d.le_audio),
  }));
}

/** A list's entry by its key, kept for a moment after it left the list
 *  (lists of devices are keyed: a device keeps its component across updates) */
export function entryByKey(list: () => AudioEntry[], key: string): () => AudioEntry {
  let last: AudioEntry | undefined;
  return () => (last = list().find((e) => e.key === key) ?? last)!;
}

/** The profile a device entry starts from before it has its own: Auto,
 *  and the device settings of its other entry's profile, else the defaults
 *  (System Loopback) */
export function entryBase(entry: Pick<AudioEntry, "address" | "name" | "le_audio">, profiles: readonly Profile[]): Profile {
  return {
    ...newProfile(),
    device_address: entry.address.toUpperCase(),
    device_name: entry.name,
    le_audio: entry.le_audio,
    ...(deviceDefaults(profiles, entry.address) ?? {}),
  };
}

/** The entry a profile belongs to */
export function entryOf(entries: readonly AudioEntry[], p: Profile | null): AudioEntry | null {
  if (!p) return null;
  const key = deviceKey({ address: p.device_address, le_audio: p.le_audio });
  return entries.find((e) => e.key === key) ?? null;
}

/** The settings of the codec a profile uses */
export function codecValues(p: Profile): CodecValues {
  return {
    quality: p.quality,
    sample_rate: p.sample_rate,
    bit_depth: p.bit_depth,
    abr: p.abr,
    sbc_bitpool: p.sbc_bitpool,
    sbcxq_bitpool: p.sbcxq_bitpool,
    sbc_min_bitpool: p.sbc_min_bitpool,
    sbcxq_min_bitpool: p.sbcxq_min_bitpool,
    sbc_blocks: p.sbc_blocks,
    sbc_subbands: p.sbc_subbands,
    sbc_allocation: p.sbc_allocation,
    sbc_channel_mode: p.sbc_channel_mode,
  };
}

/**
 * The profile with `codec` chosen: the settings of the codec it used go to
 * its codec_settings, and `codec` gets `values` (its "..." dialog's), else
 * what it was set to before (issue #86: AAC's VBR came back off), else what
 * carries over from the codec it leaves (carryToCodec)
 */
export function withCodec(p: Profile, codec: string, values?: CodecValues): Profile {
  const kept: Record<string, CodecValues> = { ...(p.codec_settings ?? {}) };
  if (p.codec !== codec) kept[p.codec] = codecValues(p);
  const back = values ?? kept[codec];
  delete kept[codec];
  const base: Profile = { ...p, codec_settings: kept };
  // Values of its own come back as they were; else what carries over from the codec it leaves
  return back ? fitToCodec({ ...base, ...back, codec }) : carryToCodec(base, codec);
}

/**
 * A device profile's name: "<device> | <codec>" (v2's own, as the setup
 * makes it; "<device> LE Audio" for LE Audio) while it has that form, the
 * user's own otherwise. A name another profile has gets " (2)", " (3)"...
 */
export function followedName(p: Profile, codecLabel: string, taken: readonly string[]): string {
  const base = p.device_name || p.device_address;
  const own = p.name !== "" && p.name !== base && !p.name.startsWith(`${base} | `) && p.name !== `${base} LE Audio`;
  if (own) return p.name;
  const name = defaultProfileName(p.device_name, p.device_address, p.le_audio, codecLabel);
  if (!taken.includes(name)) return name;
  for (let n = 2; ; n++) if (!taken.includes(`${name} (${n})`)) return `${name} (${n})`;
}

/** A problem with a field: translation key and its arguments */
export interface Problem {
  key: string;
  args?: (string | number)[];
}

export type ProfileProblems = Partial<Record<"name" | "device" | "packet" | "audio_device" | "app", Problem>>;

/**
 * `taken`: the names of the other profiles (a rename must not overwrite
 * one of them)
 */
export function validateProfile(p: Profile, taken: readonly string[]): ProfileProblems {
  const out: ProfileProblems = {};
  const name = p.name.trim();
  if (!name) out.name = { key: "ui.profile.err_name_empty" };
  else if (taken.includes(name)) out.name = { key: "ui.profile.err_name_taken" };
  if (!p.device_address.trim()) out.device = { key: "ui.profile.err_device" };
  else if (!parseAddress(p.device_address)) out.device = { key: "error.invalid_address_format" };
  if (!Number.isInteger(p.max_media_payload) || p.max_media_payload < PACKET_MIN || p.max_media_payload > PACKET_MAX) {
    out.packet = { key: "ui.profile.err_packet", args: [PACKET_MIN, PACKET_MAX] };
  }
  if (p.capture_mode === "virtual" && !p.audio_device_id) out.audio_device = { key: "error.no_virtual_device_selected" };
  if (p.capture_mode === "app" && !p.app_exe) out.app = { key: "error.no_app_selected" };
  // LE Audio streams loopback and virtual device capture only (the core
  // refuses the stream otherwise)
  if (p.le_audio && p.capture_mode === "app") out.app = { key: "error.le_audio_capture" };
  return out;
}

/** The profile as it is saved: trimmed name, normalised address */
export function normalizeProfile(p: Profile): Profile {
  return { ...p, name: p.name.trim(), device_address: parseAddress(p.device_address) ?? p.device_address.trim() };
}

/** The codec keys a device offered when it last connected (empty: not known) */
export function knownCodecs(devices: readonly Device[], address: string): string[] {
  const a = parseAddress(address);
  if (!a) return [];
  return devices.find((d) => !d.le_audio && d.address.toUpperCase() === a)?.codecs ?? [];
}

/** The codec keys a device may offer only once chosen (issue #30) */
export function codecsOnRequest(devices: readonly Device[], address: string): string[] {
  const a = parseAddress(address);
  if (!a) return [];
  return devices.find((d) => !d.le_audio && d.address.toUpperCase() === a)?.codecs_on_request ?? [];
}

/** What each codec a device offered lists (rates, bit depths, largest bit
 *  rate: Device.codec_caps; empty while not known, issue #86) */
export function knownCodecCaps(devices: readonly Device[], address: string): CodecCaps[] {
  const a = parseAddress(address);
  if (!a) return [];
  return devices.find((d) => !d.le_audio && d.address.toUpperCase() === a)?.codec_caps ?? [];
}

/**
 * A device entry's identity: a dual-mode device has a classic entry and an
 * LE Audio one with the same address
 */
export function deviceKey(d: Pick<Device, "address" | "le_audio">): string {
  return `${d.address.toUpperCase()}${d.le_audio ? "/le" : ""}`;
}

/** The entries to offer: LE Audio ones only with LE Audio (experimental) on, as v1 */
export function listedDevices(devices: readonly Device[], leAudio: boolean): Device[] {
  return leAudio ? [...devices] : devices.filter((d) => !d.le_audio);
}

/** A profile's name until the user types one: the device, "<device> LE Audio" for LE Audio (v1) */
export function defaultProfileName(deviceName: string, address: string, leAudio: boolean, codec = ""): string {
  const base = deviceName || address;
  if (leAudio) return base ? `${base} LE Audio` : base;
  return base && codec ? `${base} | ${codec}` : base;
}

/**
 * The render device a capture mode records: the default output device
 * (loopback), the chosen virtual device, or none (application capture:
 * Windows converts to the rate asked for)
 */
export function captureDevice(
  p: Pick<Profile, "capture_mode" | "audio_device_id">,
  devices: readonly AudioDevice[],
): AudioDevice | null {
  if (p.capture_mode === "loopback") return devices.find((d) => d.is_default) ?? null;
  if (p.capture_mode === "virtual") return devices.find((d) => d.id === p.audio_device_id) ?? null;
  return null;
}

/**
 * Which of the profile's sample rate and bit depth differ from the capture
 * device's format (v1 update_format_info; Auto never differs)
 */
export function formatMismatch(
  p: Pick<Profile, "sample_rate" | "bit_depth">,
  d: Pick<AudioDevice, "mix_rate" | "mix_bits">,
): { rate: boolean; bits: boolean } {
  return {
    rate: p.sample_rate !== 0 && d.mix_rate !== null && p.sample_rate !== d.mix_rate,
    bits: p.bit_depth !== 0 && d.mix_bits !== null && p.bit_depth !== d.mix_bits,
  };
}

/** A device name the core saves: not empty, on one line, without "|" */
export function validDeviceName(name: string): boolean {
  const n = name.trim();
  return n.length > 0 && !/[\r\n|]/.test(n);
}

/** Whether this Windows can capture one application (build 19041 or later) */
export type AppCaptureSupport = "yes" | "no" | "unknown";

/**
 * From User-Agent Client Hints: on Windows the major of platformVersion is
 * the UniversalApiContract version, 10 from Windows 10 version 2004 (build
 * 19041) and 13+ on Windows 11; 1-8 are the earlier Windows 10 versions.
 * Unknown elsewhere or without the hints (the core still refuses the
 * stream with error.app_capture_unsupported).
 */
export function appCaptureSupportFrom(platform: string | undefined, platformVersion: string | undefined): AppCaptureSupport {
  if (platform !== "Windows" || !platformVersion) return "unknown";
  const major = Number.parseInt(platformVersion, 10);
  if (!Number.isFinite(major)) return "unknown";
  return major >= 10 ? "yes" : "no";
}

interface UaData {
  getHighEntropyValues(hints: string[]): Promise<{ platform?: string; platformVersion?: string }>;
}

export async function appCaptureSupport(): Promise<AppCaptureSupport> {
  const ua = (globalThis.navigator as { userAgentData?: UaData } | undefined)?.userAgentData;
  if (!ua?.getHighEntropyValues) return "unknown";
  try {
    const v = await ua.getHighEntropyValues(["platformVersion"]);
    return appCaptureSupportFrom(v.platform, v.platformVersion);
  } catch {
    return "unknown";
  }
}

/** stream.start_custom for the core's test tone (left 1 kHz, right
 *  1.5 kHz, as v1's) to a profile's headphones, with its sound settings */
export function toneParams(p: Profile): ParamsOf<"stream.start_custom"> {
  return {
    device: p.device_address,
    device_name: p.device_name || null,
    codec: p.codec,
    quality: p.quality,
    sample_rate: p.sample_rate || null,
    bit_depth: p.bit_depth || null,
    source: "tone",
    max_media_payload: p.max_media_payload || null,
    abr: p.abr,
    // As a profile: a codec the device does not offer falls back (issue #86)
    codec_fallback: true,
  };
}
