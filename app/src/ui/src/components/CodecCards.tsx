// The codec cards of the Codecs screen: one codec with its three
// comparison bars and whether the device supported it when it last
// connected (green / red, issue #86), and behind "..." that codec's
// quality, sample rate, bit depth
// and ABR (SBC and SBC XQ: bitpool, in developer mode frame setup). The
// codecs offered
// and what they can be set to come from lib/codecs.ts, as far as the
// device listed them when it last connected (issue #86: its rates, bit
// depths, largest bit rate and bitpool range).

import { createEffect, on, Show } from "solid-js";
import { createStore } from "solid-js/store";
import { t } from "../i18n";
import type { Profile } from "../ipc/types";
import {
  autoBitDepth,
  autoSampleRate,
  capsOf,
  carryToCodec,
  fixedBitDepth,
  fixedKbpsText,
  codecSpec,
  matchedBitDepth,
  matchedSampleRate,
  BITPOOL_MAX,
  BITPOOL_MAX_XQ,
  BITPOOL_MIN,
  offeredBitDepths,
  offeredQualities,
  offeredSampleRates,
  onlyChoice,
  qualityIgnored,
  unlistedCaps,
  type CodecCaps,
  type CodecSpec,
  type CodecSupport,
} from "../lib/codecs";
import {
  SBC_ALLOCATIONS,
  SBC_BLOCKS,
  SBC_CHANNEL_MODES,
  SBC_SUBBANDS,
  sbcKbpsText,
  sbcOffered,
  sbcRate,
  sbcSetup,
  type SbcCaps,
} from "../lib/sbc";
import { Button, Dialog, Field, IconButton, Select, Toggle } from "./controls";
import { Icon } from "./Icon";

/** One codec choice with its three comparison bars, and "..." for its settings */
export function CodecCard(props: {
  spec: CodecSpec;
  /** The radio group: one per device */
  group: string;
  selected: boolean;
  disabled?: boolean;
  /** Auto chooses (issue #86): the card only shows whether Auto picked it */
  readOnly?: boolean;
  support: CodecSupport | null;
  onPick: () => void;
  /** The card or its "..." (`settings`) clicked while read-only: offers to
   *  turn Auto-select off (issue #86 round 8) */
  onReadOnly?: (settings: boolean) => void;
  onSettings: () => void;
  /** The card can be dragged (and moved with Alt+arrow keys) into the
   *  device's order (issue #86 round 8) */
  orderable?: boolean;
  /** Being dragged */
  dragging?: boolean;
  /** Alt+arrow key: one place earlier (-1) or later (1) */
  onMove?: (by: -1 | 1) => void;
  /** The id of what tells how to move it (screen readers) */
  describedBy?: string;
}) {
  const bar = (label: string, v: number) => (
    <div class="cbar">
      <span class="cbar-label">{label}</span>
      <span class="cbar-track" role="meter" aria-valuemin={0} aria-valuemax={5} aria-valuenow={v} aria-label={label}>
        <span class="cbar-fill" style={{ width: `${v * 20}%` }} />
      </span>
    </div>
  );
  const name = () => t(props.spec.label);
  return (
    // The button is beside the label, not in it: the radio's name stays the codec's
    <div class="codec-cell" classList={{ dragging: props.dragging }} data-codec={props.spec.id}>
      <label
        class="codec-card"
        classList={{ on: props.selected, ro: props.readOnly, orderable: props.orderable }}
        onClick={() => {
          if (props.readOnly) props.onReadOnly?.(false);
        }}
      >
        <input
          type="radio"
          name={props.group}
          value={props.spec.id}
          checked={props.selected}
          disabled={props.disabled || props.readOnly}
          aria-describedby={props.orderable ? props.describedBy : undefined}
          onChange={() => props.onPick()}
          onKeyDown={(e) => {
            if (!props.orderable || !e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return;
            const by = { ArrowLeft: -1, ArrowUp: -1, ArrowRight: 1, ArrowDown: 1 }[e.key] as -1 | 1 | undefined;
            if (!by) return;
            // Not the radio group's own arrow keys (they choose the next card)
            e.preventDefault();
            props.onMove?.(by);
          }}
        />
        <span class="codec-card-name">{name()}</span>
        {/* The wrapper keeps the tag its own size in the card's column */}
        <Show when={props.support}>
          {(s) => (
            <span class="codec-card-tags">
              {/* Unknown until the device's first connection */}
              <Show
                when={s() !== "unknown"}
                fallback={
                  <span class="tag">
                    <Icon name="question_circle" />
                    {t("ui.profile.codec_support_unknown")}
                  </span>
                }
              >
                {/* Only the mark in colour (issue #86) */}
                <span class="tag">
                  <Icon
                    name={s() === "offered" ? "checkmark" : "dismiss"}
                    class={s() === "offered" ? "st-good" : "st-bad"}
                  />
                  {s() === "offered" ? t("ui.profile.codec_supported") : t("ui.profile.codec_not_supported")}
                </span>
              </Show>
            </span>
          )}
        </Show>
        <Show when={props.spec.bars}>
          {(b) => (
            <>
              {bar(t("ui.codec.bar_quality"), b().quality)}
              {bar(t("ui.codec.bar_latency"), b().latency)}
              {bar(t("ui.codec.bar_stability"), b().stability)}
            </>
          )}
        </Show>
      </label>
      <IconButton
        icon="more_horizontal"
        class="codec-more"
        label={t("ui.profile.codec_settings", name())}
        aria-haspopup="dialog"
        disabled={props.disabled && !props.readOnly}
        onClick={() => (props.readOnly ? props.onReadOnly?.(true) : props.onSettings())}
      />
    </div>
  );
}

export type CodecChoice = Pick<
  Profile,
  | "codec"
  | "quality"
  | "sample_rate"
  | "bit_depth"
  | "abr"
  | "sbc_bitpool"
  | "sbcxq_bitpool"
  | "sbc_min_bitpool"
  | "sbcxq_min_bitpool"
  | "sbc_blocks"
  | "sbc_subbands"
  | "sbc_allocation"
  | "sbc_channel_mode"
  | "unlisted_options"
>;

export function codecChoice(p: CodecChoice): CodecChoice {
  return {
    codec: p.codec,
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
    unlisted_options: p.unlisted_options,
  };
}

/** The SBC / SBC XQ bitpool range the dialog offers: the codec's, within
 *  the device's when known (issue #86: a device of max bitpool 39 is not
 *  offered 53; the core keeps the encoder within it the same way) */
export function bitpoolRange(codec: string, caps: SbcCaps | null): { min: number; max: number } {
  const local = codec === "sbcxq" ? BITPOOL_MAX_XQ : BITPOOL_MAX;
  if (!caps || caps.max_bitpool < BITPOOL_MIN) return { min: BITPOOL_MIN, max: local };
  const max = Math.max(Math.min(local, caps.max_bitpool), BITPOOL_MIN);
  return { min: Math.min(Math.max(caps.min_bitpool, BITPOOL_MIN), max), max };
}

/**
 * The SBC / SBC XQ bitpool as one slider with two handles (issue #86): the
 * lowest bitpool (the SET_CONFIGURATION's minimum and the floor of ABR) and
 * the highest (the bitpool the stream starts with), each 0 for Auto. Each
 * handle is a native range input (arrow keys, Page Up / Down, Home / End),
 * the lower one never past the higher. Auto shows where it is: the lowest
 * at the range's start, the highest where the quality puts it.
 */
export function BitpoolSlider(props: {
  /** The highest handle's (the field's label points at it) */
  id?: string;
  describedBy?: string;
  label: string;
  min: number;
  max: number;
  /** 0 = Auto */
  low: number;
  high: number;
  /** Where Auto puts the highest */
  autoHigh: number;
  onLow: (bitpool: number) => void;
  onHigh: (bitpool: number) => void;
}) {
  const clamp = (x: number) => Math.min(Math.max(x, props.min), props.max);
  const hi = () => clamp(props.high || Math.max(props.autoHigh, props.low));
  const lo = () => Math.min(clamp(props.low || props.min), hi());
  const at = (x: number) => (props.max > props.min ? (x - props.min) / (props.max - props.min) : 0);
  const lowText = () => (props.low ? String(lo()) : t("ui.profile.sbc_bitpool_auto", lo()));
  const highText = () => (props.high ? String(hi()) : t("ui.profile.sbc_bitpool_auto", hi()));
  const highShown = () => (props.high ? String(hi()) : t("ui.profile.sbc_bitpool_auto_high", hi()));
  return (
    <div class="bp-slider" role="group" aria-label={props.label}>
      <output class="bp-value num" aria-hidden="true">
        {lowText()}
      </output>
      <div
        class="bp-track"
        // Where they meet, the higher handle on top (it can still go up),
        // unless it is at the end: then the lower one (it can still go down)
        classList={{ "low-on-top": hi() >= props.max }}
        style={{ "--lo": String(at(lo())), "--hi": String(at(hi())) }}
      >
        <span class="bp-fill" />
        <input
          type="range"
          class="bp-low"
          min={props.min}
          max={props.max}
          step="1"
          value={lo()}
          aria-label={t("ui.profile.sbc_bitpool_min")}
          aria-valuetext={lowText()}
          aria-describedby={props.describedBy}
          onInput={(e) => {
            const x = Math.min(clamp(e.currentTarget.valueAsNumber), hi());
            e.currentTarget.value = String(x);
            props.onLow(x);
          }}
        />
        <input
          id={props.id}
          type="range"
          class="bp-high"
          min={props.min}
          max={props.max}
          step="1"
          value={hi()}
          aria-label={t("ui.profile.sbc_bitpool_max")}
          aria-valuetext={highText()}
          aria-describedby={props.describedBy}
          onInput={(e) => {
            const x = Math.max(clamp(e.currentTarget.valueAsNumber), lo());
            e.currentTarget.value = String(x);
            props.onHigh(x);
          }}
        />
      </div>
      <output class="bp-value num" aria-hidden="true">
        {highShown()}
      </output>
    </div>
  );
}

/**
 * One codec's quality, sample rate, bit depth and ABR (what "Advanced" had
 * for the chosen codec); SBC and SBC XQ also their bitpool and, in developer
 * mode, frame setup, offering only what the device listed when it last
 * connected (`sbcCaps`, and for every codec its rates, bit depths and largest
 * bit rate: `caps`, issue #86), with the bit rate that gives; with "Show
 * only supported options" turned off at its top, everything the codec has
 * (the profile's unlisted_options, issue #86), what the device did not list
 * marked. A codec with one quality shows
 * it rather than a choice. It starts from the profile's values fitted
 * to the codec, or from what that codec was last set to (`kept`: the
 * profile's codec_settings); each change goes to `onChange` at once
 * (issue #86: no OK or Cancel).
 */
export function CodecSettings(props: {
  spec: CodecSpec | null;
  profile: Profile;
  kept: (codec: string) => Omit<CodecChoice, "codec"> | undefined;
  sbcCaps: SbcCaps | null;
  /** Device.codec_caps (empty: not known) */
  caps?: readonly CodecCaps[];
  /** Offers the SBC frame setup, which only counts then */
  developer?: boolean;
  /** Developer mode: Auto follows the PC's output format, as far as the
   *  device lists it (no value beside Auto while the format is not known) */
  matchSource?: boolean;
  /** The format Windows plays the profile's source in (null: not known) */
  sourceFormat?: { rate: number; bits: number } | null;
  /** Who listed what is offered, once it did: "Show only supported
   *  options" names it */
  deviceName?: string;
  /** Forgets what the device listed (LHDC V5: OPPO earbuds are then asked
   *  for Hi-Res again, issue #237) */
  onForgetReport?: () => void;
  onChange: (values: CodecChoice) => void;
  onClose: () => void;
}) {
  const [v, setV] = createStore<CodecChoice>({
    codec: "auto",
    quality: "hq",
    sample_rate: 0,
    bit_depth: 0,
    abr: false,
    sbc_bitpool: 0,
    sbcxq_bitpool: 0,
    sbc_min_bitpool: 0,
    sbcxq_min_bitpool: 0,
    sbc_blocks: 0,
    sbc_subbands: 0,
    sbc_allocation: "",
    sbc_channel_mode: "",
    unlisted_options: false,
  });
  /** `from` with what the device does not list back to Auto, a bitpool
   *  above the device's largest down to it, a quality capped like a higher
   *  one to High (issue #86); all of it kept with unlisted options */
  const fitted = (from: CodecChoice): CodecChoice => {
    const s = codecSpec(from.codec);
    const out = { ...from };
    if (out.unlisted_options) return out;
    const caps = capsOf(props.caps ?? [], s.id);
    if (!offeredSampleRates(s.id, caps).includes(out.sample_rate)) out.sample_rate = 0;
    if (!offeredBitDepths(s.id, caps).includes(out.bit_depth)) out.bit_depth = 0;
    if (s.qualities.length > 0 && !offeredQualities(s.id, out.sample_rate, caps?.max_bitrate).some((q) => q.id === out.quality))
      out.quality = "hq";
    const range = bitpoolRange(s.id, props.sbcCaps);
    const key = s.id === "sbcxq" ? "sbcxq_bitpool" : "sbc_bitpool";
    const minKey = s.id === "sbcxq" ? "sbcxq_min_bitpool" : "sbc_min_bitpool";
    if (s.bitpool && out[key] > range.max) out[key] = range.max;
    // The minimum likewise, and never above the maximum
    if (s.bitpool && out[minKey] > range.max) out[minKey] = range.max;
    if (s.bitpool && out[minKey] < range.min && out[minKey] !== 0) out[minKey] = range.min;
    if (s.bitpool && out[key] && out[minKey] > out[key]) out[minKey] = out[key];
    return out;
  };
  createEffect(
    on(
      () => props.spec,
      (s) => {
        if (!s) return;
        const kept = props.kept(s.id);
        const p = s.id === props.profile.codec || !kept ? props.profile : { ...kept, codec: s.id };
        // From another codec's values: without its ABR or SBC frame setup
        // (SBC and SBC XQ apart, issue #86)
        const from = carryToCodec(codecChoice(p), s.id);
        // SBC / SBC XQ: no quality to choose, Auto is the highest bitpool
        setV(fitted(s.bitpool ? { ...from, quality: "hq" } : from));
      },
    ),
  );
  const spec = () => codecSpec(v.codec);
  const name = () => t(spec().label);
  /** What the device listed for this codec (null: not known) */
  const listedCaps = () => capsOf(props.caps ?? [], v.codec);
  /** What the dialog offers: what the device listed, or with "Show only
   *  supported options" off everything the codec has */
  const caps = () => (v.unlisted_options ? unlistedCaps(v.codec, listedCaps()) : listedCaps());
  const sbcCaps = () => (v.unlisted_options ? null : props.sbcCaps);
  const maxBitrate = () => caps()?.max_bitrate ?? 0;
  // SBC and SBC XQ keep their own bitpool
  const bpKey = () => (v.codec === "sbcxq" ? "sbcxq_bitpool" : "sbc_bitpool");
  const bp = () => v[bpKey()];
  const bpRange = () => bitpoolRange(v.codec, sbcCaps());
  const bpMin = () => bpRange().min;
  const bpMax = () => bpRange().max;
  // ... and their own minimum (issue #86: the slider's two handles)
  const bpMinKey = () => (v.codec === "sbcxq" ? "sbcxq_min_bitpool" : "sbc_min_bitpool");
  const bpLow = () => v[bpMinKey()];
  // A value from the profile outside the device's range (the slider itself
  // stays within it)
  const outside = (x: number) => x !== 0 && !(Number.isInteger(x) && x >= bpMin() && x <= bpMax());
  const bitpoolError = () =>
    outside(bp()) || outside(bpLow()) ? t("error.sbc_bitpool_range", bpMin(), bpMax()) : undefined;
  /** Where Auto puts the highest bitpool: the quality's (High) within the
   *  device's range, as the core does */
  const autoMax = () => {
    const r = sbcRate(v, sbcCaps());
    const at = sbcSetup({ ...v, [bpKey()]: 0, [bpMinKey()]: 0 }, r, sbcCaps()).bitpool;
    return Math.min(Math.max(at, bpMin()), bpMax());
  };
  // The qualities with their bit rates on this device; one is shown, not chosen
  const qualities = () => offeredQualities(v.codec, v.sample_rate, maxBitrate(), v.quality);
  const qualityText = (q: { label: string; kbps?: string }) => (q.kbps ? `${t(q.label)} (${q.kbps} kbps)` : t(q.label));
  const rates = () => {
    const offered = offeredSampleRates(v.codec, caps());
    return offered.includes(v.sample_rate) ? offered : [...offered, v.sample_rate];
  };
  const depths = () => {
    const offered = offeredBitDepths(v.codec, caps());
    return offered.includes(v.bit_depth) ? offered : [...offered, v.bit_depth];
  };
  // Auto with the value it streams at (issue #86); matching the PC's output,
  // the value only once its format is known
  const autoRateLabel = () => {
    if (!props.matchSource) return t("codec.auto_value", `${autoSampleRate(v.codec, caps())} Hz`);
    const f = props.sourceFormat;
    return f ? t("codec.auto_value", `${matchedSampleRate(v.codec, caps(), f.rate)} Hz`) : t("codec.auto");
  };
  const autoDepthLabel = () => {
    const f = props.sourceFormat;
    const bits = !props.matchSource
      ? autoBitDepth(v.codec, caps())
      : f
        ? matchedBitDepth(v.codec, caps(), f.bits)
        : 0;
    return bits ? t("codec.auto_value", t(`bitdepth.${bits}`)) : t("codec.auto");
  };
  // One rate or bit depth the device takes: a fixed value, not Auto and that
  // value to choose between
  const onlyRate = () => onlyChoice(offeredSampleRates(v.codec, caps()), v.sample_rate, 0);
  const onlyDepth = () =>
    depths().length > 1 ? onlyChoice(offeredBitDepths(v.codec, caps()), v.bit_depth, 0) : fixedBitDepth(v.codec);
  // The frame setup: Auto, then what the device listed (the value chosen
  // stays, marked, when it did not list it: it falls back when connecting);
  // one value listed is shown fixed, as the rate and bit depth are
  const sbcField = <K extends "sbc_blocks" | "sbc_subbands" | "sbc_allocation" | "sbc_channel_mode">(
    title: string,
    key: K,
    field: "blocks" | "subbands" | "allocations" | "channel_modes",
    values: CodecChoice[K][],
    label: (v: CodecChoice[K]) => string,
    // What Auto takes (core: SbcParams' defaults), shown beside it (issue #86)
    auto: CodecChoice[K],
  ) => {
    const none = (typeof v[key] === "number" ? 0 : "") as CodecChoice[K];
    const only = () => onlyChoice(values.filter((x) => sbcOffered(sbcCaps(), field, x)), v[key], none);
    const options = () => [
      { value: none, label: t("codec.auto_value", label(auto)) },
      ...values
        .filter((x) => x === v[key] || sbcOffered(sbcCaps(), field, x))
        .map((x) => ({
          value: x,
          label: sbcOffered(props.sbcCaps, field, x) ? label(x) : t("ui.profile.sbc_not_offered", label(x)),
        })),
    ];
    return (
      <Field label={title} stacked>
        {(id) => (
          <Show when={only() === undefined} fallback={<input id={id} class="input readonly" readOnly value={label(only()!)} />}>
            <Select id={id} value={v[key]} options={options()} onChange={(x) => set(key, x)} />
          </Show>
        )}
      </Field>
    );
  };
  // What streams: outside developer mode the frame setup kept is not used
  const streamed = (): CodecChoice =>
    props.developer ? v : { ...v, sbc_blocks: 0, sbc_subbands: 0, sbc_allocation: "", sbc_channel_mode: "" };
  // The bit rate it gives, as the core works it out (one figure, issue #86)
  const sbcBitrate = () => (bitpoolError() ? "" : sbcKbpsText(streamed(), sbcCaps()));
  const sbcMode = () => sbcSetup(streamed(), sbcRate(v, sbcCaps()), sbcCaps()).mode;
  const changed = () => {
    if (!bitpoolError()) props.onChange({ ...v });
  };
  const set = <K extends keyof CodecChoice>(key: K, value: CodecChoice[K]) => {
    setV(key as "codec", value as string);
    changed();
  };
  const listed = () => !!props.deviceName && !!(spec().bitpool ? props.sbcCaps : listedCaps());
  // "Show only supported options" turned on again: back within what the
  // device lists
  const setUnlisted = (on: boolean) => {
    setV(fitted({ ...v, unlisted_options: on }));
    changed();
  };
  return (
    <Dialog
      open={props.spec !== null}
      title={t("ui.profile.codec_settings", name())}
      onCancel={() => props.onClose()}
    >
      <div class="codec-settings">
        {/* What the device listed, or everything the codec has (issue #86:
            the user's choice, the device may refuse it) */}
        <Show when={listed()}>
          <Toggle
            label={t("ui.profile.codec_options_listed", props.deviceName!)}
            checked={!v.unlisted_options}
            onChange={(on) => setUnlisted(!on)}
          />
        </Show>
        {/* SBC and SBC XQ: the bitpool below decides the bit rate, shown
            here as worked out (issue #86) */}
        <Show
          when={!spec().bitpool}
          fallback={
            <Field label={t("connection.quality")} hint={t("tooltip.quality")}>
              {(id, described) => (
                <output id={id} class="input readonly num" aria-describedby={described} aria-live="polite">
                  {sbcBitrate() || "–"}
                </output>
              )}
            </Field>
          }
        >
          {/* One quality (or a fixed bit rate): shown, not a choice (issue
              #86: High is the highest bit rate the device takes) */}
          <Show
            when={qualities().length > 1}
            fallback={
              <Field label={t("connection.quality")} hint={t("tooltip.quality")}>
                {(id, described) => (
                  <output id={id} class="input readonly" aria-describedby={described}>
                    {qualities().length ? qualityText(qualities()[0]) : fixedKbpsText(v.codec, sbcRate(v, caps()))}
                  </output>
                )}
              </Field>
            }
          >
            <Field label={t("connection.quality")} hint={t("tooltip.quality")}>
              {(id, described) => (
                <Select
                  id={id}
                  describedBy={described}
                  value={v.quality}
                  disabled={qualityIgnored(v.codec, v.abr)}
                  options={qualities().map((q) => ({ value: q.id, label: qualityText(q) }))}
                  onChange={(x) => set("quality", x)}
                />
              )}
            </Field>
          </Show>
        </Show>
        {/* One rate the device takes: shown as a value, without Auto */}
        <Show
          when={onlyRate() === undefined}
          fallback={
            <Field label={t("connection.sample_rate")}>
              {(id) => <input id={id} class="input readonly" readOnly value={`${onlyRate()} Hz`} />}
            </Field>
          }
        >
          <Field label={t("connection.sample_rate")} hint={t("tooltip.sample_rate")}>
            {(id, described) => (
              <Select
                id={id}
                describedBy={described}
                value={v.sample_rate}
                options={rates().map((r) => {
                  // Auto with the rate it streams at (issue #86)
                  const label = r === 0 ? autoRateLabel() : `${r} Hz`;
                  const listed = offeredSampleRates(v.codec, listedCaps()).includes(r);
                  return { value: r, label: listed ? label : t("ui.profile.sbc_not_offered", label) };
                })}
                onChange={(x) => set("sample_rate", x)}
              />
            )}
          </Field>
        </Show>
        {/* One bit depth (SBC, AAC, aptX, Opus: 16-bit; aptX Adaptive:
            24-bit; or the one the device lists): shown as a value, not as a
            list to open (issue #86) */}
        <Show
          when={onlyDepth() === undefined}
          fallback={
            <Field label={t("connection.bit_depth")}>
              {(id) => <input id={id} class="input readonly" readOnly value={t(`bitdepth.${onlyDepth()}`)} />}
            </Field>
          }
        >
          <Field label={t("connection.bit_depth")} hint={t("tooltip.bit_depth")}>
            {(id, described) => (
              <Select
                id={id}
                describedBy={described}
                value={v.bit_depth}
                options={depths().map((b) => ({
                  value: b,
                  label:
                    b === 0
                      ? autoDepthLabel()
                      : offeredBitDepths(v.codec, listedCaps()).includes(b)
                        ? t(`bitdepth.${b}`)
                        : t("ui.profile.sbc_not_offered", t(`bitdepth.${b}`)),
                }))}
                onChange={(x) => set("bit_depth", x)}
              />
            )}
          </Field>
        </Show>
        <Show when={spec().abr}>
          <Toggle label={t("connection.abr")} hint={t("tooltip.abr")} checked={v.abr} onChange={(x) => set("abr", x)} />
        </Show>
        <Show when={spec().vbr}>
          <Toggle label={t("connection.vbr")} hint={t("tooltip.vbr")} checked={v.abr} onChange={(x) => set("abr", x)} />
        </Show>
        <Show when={spec().bitpool}>
          <Field label={t("connection.sbc_bitpool")} hint={t("tooltip.sbc_bitpool")} error={bitpoolError()} stacked>
            {(id, described) => (
              <div class="inline bp-field">
                <BitpoolSlider
                  id={id}
                  describedBy={described}
                  label={t("connection.sbc_bitpool")}
                  min={bpMin()}
                  max={bpMax()}
                  low={bpLow()}
                  high={bp()}
                  autoHigh={autoMax()}
                  onLow={(x) => set(bpMinKey(), x)}
                  onHigh={(x) => set(bpKey(), x)}
                />
                <Button
                  kind="subtle"
                  disabled={bp() === 0 && bpLow() === 0}
                  onClick={() => {
                    setV(bpKey(), 0);
                    set(bpMinKey(), 0);
                  }}
                >
                  {t("codec.auto")}
                </Button>
              </div>
            )}
          </Field>
          <Show when={props.developer}>
            <fieldset class="sbc-frame">
              <legend class="field-label">{t("ui.profile.sbc_frame")}</legend>
              {sbcField(t("ui.diag.cd_blocks"), "sbc_blocks", "blocks", SBC_BLOCKS, String, 16)}
              {sbcField(t("ui.diag.cd_subbands"), "sbc_subbands", "subbands", SBC_SUBBANDS, String, 8)}
              {sbcField(t("ui.diag.cd_allocation"), "sbc_allocation", "allocations", SBC_ALLOCATIONS, (a) => t(`ui.diag.cd_alloc.${a}`), "loudness")}
              {/* SBC XQ is dual channel by definition */}
              <Show when={v.codec === "sbc"}>
                {sbcField(t("ui.diag.cd_channel_mode"), "sbc_channel_mode", "channel_modes", SBC_CHANNEL_MODES, (m) => t(`ui.diag.cd_mode.${m}`), "joint_stereo")}
              </Show>
            </fieldset>
          </Show>
          {/* Fixed for each codec, unless chosen above (issue #86) */}
          <Show when={!props.developer || v.codec === "sbcxq"}>
            <Field label={t("ui.diag.cd_channel_mode")}>
              {(id) => <input id={id} class="input readonly" readOnly value={t(`ui.diag.cd_mode.${sbcMode()}`)} />}
            </Field>
          </Show>
        </Show>
        {/* OPPO's Hi-Res request: niche, so developer mode (issue #86) */}
        <Show when={v.codec === "lhdcv5" && listed() && props.developer && props.onForgetReport}>
          {(forget) => (
            <div>
              <Button icon="arrow_reset" onClick={() => forget()()}>
                {t("ui.profile.ask_hi_res_again")}
              </Button>
            </div>
          )}
        </Show>
      </div>
    </Dialog>
  );
}
