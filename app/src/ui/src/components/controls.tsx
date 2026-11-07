// Form controls on native elements (keyboard and screen reader behaviour
// for free), styled in styles/app.css.

import {
  createEffect,
  createMemo,
  createSignal,
  createUniqueId,
  For,
  onCleanup,
  onMount,
  Show,
  splitProps,
  untrack,
  type JSX,
} from "solid-js";
import { Portal } from "solid-js/web";
import { t } from "../i18n";
import { Icon } from "./Icon";
import type { IconName } from "./icons";

type ButtonProps = JSX.ButtonHTMLAttributes<HTMLButtonElement> & {
  kind?: "default" | "subtle" | "danger" | "strong";
  icon?: IconName;
  busy?: boolean;
};

export function Button(props: ButtonProps) {
  const [own, rest] = splitProps(props, ["kind", "icon", "busy", "children", "class", "disabled"]);
  return (
    <button
      type="button"
      class={`btn btn-${own.kind ?? "default"} ${own.class ?? ""}`}
      disabled={own.disabled || own.busy}
      aria-busy={own.busy ? "true" : undefined}
      {...rest}
    >
      <Show when={own.busy} fallback={own.icon && <Icon name={own.icon} />}>
        <span class="spinner" aria-hidden="true" />
      </Show>
      <Show when={own.children}>
        <span>{own.children}</span>
      </Show>
    </button>
  );
}

/** Icon-only button: the label is for screen readers and the tooltip */
export function IconButton(props: JSX.ButtonHTMLAttributes<HTMLButtonElement> & { icon: IconName; label: string }) {
  const [own, rest] = splitProps(props, ["icon", "label", "class"]);
  return (
    <button type="button" class={`btn btn-icon ${own.class ?? ""}`} aria-label={own.label} title={own.label} {...rest}>
      <Icon name={own.icon} />
    </button>
  );
}

/** A page's tabs (WAI-ARIA tabs pattern: arrow keys, Home, End). Tabs that
 *  do not fit scroll with the arrows beside them, not a scrollbar; the
 *  arrows are for the mouse, the keys already bring a tab into view. */
export function TabBar<T extends string>(props: {
  label: string;
  tabs: { id: T; label: string; icon: IconName }[];
  current: T;
  onSelect: (id: T) => void;
}) {
  let list: HTMLDivElement | undefined;
  const [canLeft, setCanLeft] = createSignal(false);
  const [canRight, setCanRight] = createSignal(false);
  const measure = () => {
    if (!list) return;
    setCanLeft(list.scrollLeft > 1);
    setCanRight(list.scrollLeft + list.clientWidth < list.scrollWidth - 1);
  };
  const scroll = (dir: -1 | 1) => list?.scrollBy({ left: dir * list.clientWidth * 0.7, behavior: "smooth" });
  onMount(() => {
    measure();
    if (typeof ResizeObserver === "undefined" || !list) return;
    const ro = new ResizeObserver(measure);
    ro.observe(list);
    onCleanup(() => ro.disconnect());
  });
  const onKey = (e: KeyboardEvent) => {
    const tabs = props.tabs;
    const i = tabs.findIndex((x) => x.id === props.current);
    let n = -1;
    if (e.key === "ArrowRight") n = (i + 1) % tabs.length;
    else if (e.key === "ArrowLeft") n = (i - 1 + tabs.length) % tabs.length;
    else if (e.key === "Home") n = 0;
    else if (e.key === "End") n = tabs.length - 1;
    if (n < 0) return;
    e.preventDefault();
    props.onSelect(tabs[n].id);
    document.getElementById(`tab-${tabs[n].id}`)?.focus();
  };
  const arrow = (dir: -1 | 1) => (
    <button
      type="button"
      class={`tabs-scroll ${dir < 0 ? "left" : "right"}`}
      tabindex={-1}
      aria-hidden="true"
      onClick={() => scroll(dir)}
    >
      <Icon name="chevron_right" />
    </button>
  );
  return (
    <div class="tabs-bar">
      <Show when={canLeft()}>{arrow(-1)}</Show>
      <div class="tabs" role="tablist" aria-label={props.label} onKeyDown={onKey} onScroll={measure} ref={list}>
        <For each={props.tabs}>
          {(x) => (
            <button
              type="button"
              role="tab"
              id={`tab-${x.id}`}
              class="tab"
              aria-selected={props.current === x.id}
              aria-controls={`panel-${x.id}`}
              tabindex={props.current === x.id ? 0 : -1}
              onClick={() => props.onSelect(x.id)}
            >
              <Icon name={x.icon} />
              {x.label}
            </button>
          )}
        </For>
      </div>
      <Show when={canRight()}>{arrow(1)}</Show>
    </div>
  );
}

/** A "?" that shows `children` while hovered or focused (short reference
 *  text such as an order; longer explanations go behind Field's info button) */
export function HelpTip(props: { label: string; children: JSX.Element }) {
  const id = createUniqueId();
  return (
    <span class="help-tip">
      <button type="button" class="btn btn-icon help-tip-btn" aria-label={props.label} aria-describedby={id}>
        <Icon name="question_circle" />
      </button>
      <span class="help-tip-body" role="tooltip" id={id}>
        {props.children}
      </span>
    </span>
  );
}

export function Toggle(props: {
  label: string;
  hint?: string;
  /** A HelpTip beside the label */
  help?: () => JSX.Element;
  checked: boolean;
  disabled?: boolean;
  onChange: (v: boolean) => void;
}) {
  const id = createUniqueId();
  return (
    <div class="row toggle-row">
      <label class="row-text" for={id}>
        <span class="row-label" id={`${id}-label`}>
          {props.label}
          {props.help?.()}
        </span>
        <Show when={props.hint}>
          <span class="hint" id={`${id}-hint`}>
            {props.hint}
          </span>
        </Show>
      </label>
      <input
        id={id}
        type="checkbox"
        role="switch"
        class="switch"
        checked={props.checked}
        disabled={props.disabled}
        aria-labelledby={`${id}-label`}
        aria-describedby={props.hint ? `${id}-hint` : undefined}
        onChange={(e) => props.onChange(e.currentTarget.checked)}
      />
    </div>
  );
}

/** A small switch with its label after it, for options inside a line or a
 *  dialog (every on/off option is a switch, issue #86) */
export function Check(props: { label: string; checked: boolean; disabled?: boolean; title?: string; onChange: (v: boolean) => void }) {
  return (
    <label class="check" title={props.title}>
      <input
        type="checkbox"
        role="switch"
        class="switch small"
        checked={props.checked}
        disabled={props.disabled}
        onChange={(e) => props.onChange(e.currentTarget.checked)}
      />
      <span>{props.label}</span>
    </label>
  );
}

export interface Option<T extends string | number> {
  value: T;
  label: string;
  icon?: IconName;
  disabled?: boolean;
  /** A Select's options with the same group (next to each other) go under
   *  one heading */
  group?: string;
}

export function Segmented<T extends string | number>(props: {
  label: string;
  value: T;
  options: Option<T>[];
  onChange: (v: T) => void;
}) {
  const name = createUniqueId();
  return (
    <div class="segmented" role="radiogroup" aria-label={props.label}>
      <For each={props.options}>
        {(o) => (
          <label class="seg" classList={{ on: o.value === props.value, disabled: o.disabled }}>
            <input
              type="radio"
              name={name}
              value={String(o.value)}
              checked={o.value === props.value}
              disabled={o.disabled}
              onChange={() => props.onChange(o.value)}
            />
            <Show when={o.icon}>{(i) => <Icon name={i()} />}</Show>
            <span>{o.label}</span>
          </label>
        )}
      </For>
    </div>
  );
}

/** A labelled control with an optional hint and error, and the details
 *  behind an info button (the long explanations, issue #86). The label and
 *  hint on the left and the control on the right, as a Toggle (issue #86);
 *  `stacked`: one below the other, for a control that needs the width. */
export function Field(props: {
  label: string;
  hint?: string;
  error?: string | null;
  more?: () => JSX.Element;
  stacked?: boolean;
  children: (id: string, describedBy: string | undefined) => JSX.Element;
}) {
  const id = createUniqueId();
  const [open, setOpen] = createSignal(false);
  // Built once: a reactive call would make a new input (and lose the focus
  // and the typed text) whenever the error comes or goes. References to an
  // absent hint or error are ignored by assistive technology.
  const control = untrack(() => props.children(id, `${id}-hint ${id}-err`));
  const head = (
    <div class="field-head">
      <label class="field-label" for={id}>
        {props.label}
      </label>
      <Show when={props.more}>
        <button
          type="button"
          class="btn btn-icon field-more-btn"
          classList={{ on: open() }}
          aria-label={t("ui.more_info", props.label)}
          title={t("ui.more_info", props.label)}
          aria-expanded={open()}
          aria-controls={`${id}-more`}
          onClick={() => setOpen(!open())}
        >
          <Icon name="info" />
        </button>
      </Show>
    </div>
  );
  const hint = (
    <Show when={props.hint}>
      <span class="hint" id={`${id}-hint`}>
        {props.hint}
      </span>
    </Show>
  );
  return (
    <div class="field" classList={{ invalid: !!props.error, side: !props.stacked }}>
      {props.stacked ? (
        <>
          {head}
          {control}
          {hint}
        </>
      ) : (
        <div class="row">
          <div class="row-text">
            {head}
            {hint}
          </div>
          {control}
        </div>
      )}
      <Show when={props.error}>
        <span class="field-error" id={`${id}-err`} role="alert">
          <Icon name="error_circle" />
          {props.error}
        </span>
      </Show>
      <Show when={props.more && open()}>
        <div class="field-more" id={`${id}-more`}>
          {props.more!()}
        </div>
      </Show>
    </div>
  );
}

export function Select<T extends string | number>(props: {
  id?: string;
  value: T;
  options: Option<T>[];
  disabled?: boolean;
  label?: string;
  describedBy?: string;
  onChange: (v: T) => void;
}) {
  // Runs of options with the same group: the options stay in order, so the
  // selected index still finds the option
  const groups = createMemo(() => {
    const list: { label?: string; options: Option<T>[] }[] = [];
    for (const o of props.options) {
      const last = list[list.length - 1];
      if (last && last.label === o.group) last.options.push(o);
      else list.push({ label: o.group, options: [o] });
    }
    return list;
  });
  const option = (o: Option<T>) => (
    <option value={String(o.value)} selected={o.value === props.value} disabled={o.disabled}>
      {o.label}
    </option>
  );
  return (
    <div class="select">
      <select
        id={props.id}
        aria-label={props.label}
        aria-describedby={props.describedBy}
        disabled={props.disabled}
        onChange={(e) => {
          const o = props.options[e.currentTarget.selectedIndex];
          if (o) props.onChange(o.value);
        }}
      >
        <For each={groups()}>
          {(g) =>
            g.label === undefined ? (
              <For each={g.options}>{option}</For>
            ) : (
              <optgroup label={g.label}>
                <For each={g.options}>{option}</For>
              </optgroup>
            )
          }
        </For>
      </select>
      <Icon name="chevron_down" class="select-chevron" />
    </div>
  );
}

/** A card section with a heading */
export function Section(props: { title: string; actions?: JSX.Element; children: JSX.Element; class?: string; id?: string }) {
  const id = createUniqueId();
  return (
    <section id={props.id} class={`card section ${props.class ?? ""}`} aria-labelledby={id}>
      <header class="section-head">
        <h2 id={id}>{props.title}</h2>
        <Show when={props.actions}>
          <div class="section-actions">{props.actions}</div>
        </Show>
      </header>
      {props.children}
    </section>
  );
}

export function Banner(props: {
  kind: "info" | "warn" | "error" | "good";
  title?: string;
  children?: JSX.Element;
  actions?: JSX.Element;
  onDismiss?: () => void;
}) {
  const icon = (): IconName =>
    props.kind === "error" ? "error_circle" : props.kind === "warn" ? "warning" : props.kind === "good" ? "checkmark_circle" : "info";
  return (
    <div class={`banner banner-${props.kind}`} role={props.kind === "error" ? "alert" : "status"}>
      <Icon name={icon()} class="banner-icon" />
      <div class="banner-body">
        <Show when={props.title}>
          <strong class="banner-title">{props.title}</strong>
        </Show>
        <Show when={props.children}>
          <div class="banner-text">{props.children}</div>
        </Show>
        <Show when={props.actions}>
          <div class="banner-actions">{props.actions}</div>
        </Show>
      </div>
      <Show when={props.onDismiss}>
        <IconButton icon="dismiss" label={t("modal.close")} onClick={() => props.onDismiss?.()} />
      </Show>
    </div>
  );
}

/** A modal question. Escape and the backdrop cancel; Tab stays inside. */
export function Dialog(props: {
  open: boolean;
  title: string;
  subtitle?: string;
  children: JSX.Element;
  /** Without these two the dialog only closes: what is in it applies at once */
  confirm?: string;
  cancel?: string;
  danger?: boolean;
  busy?: boolean;
  /** Room for a table whose rows should not wrap */
  wide?: boolean;
  onConfirm?: () => void;
  onCancel: () => void;
}) {
  const id = createUniqueId();
  let box: HTMLDivElement | undefined;
  let before: Element | null = null;
  createEffect(() => {
    if (!props.open) return;
    before = document.activeElement;
    queueMicrotask(() => box?.querySelector<HTMLElement>("[data-autofocus]")?.focus());
    onCleanup(() => {
      if (before instanceof HTMLElement) before.focus();
    });
  });
  // While the confirmed action runs the dialog stays: closing it would hide
  // the spinner, not stop the work
  const cancel = () => {
    if (!props.busy) props.onCancel();
  };
  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      e.preventDefault();
      cancel();
    } else if (e.key === "Tab" && box) {
      const f = [...box.querySelectorAll<HTMLElement>("button:not([disabled]),input,select,textarea,[tabindex='0']")];
      if (f.length === 0) return;
      const first = f[0];
      const last = f[f.length - 1];
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    }
  };
  return (
    <Show when={props.open}>
      <Portal>
        <div class="backdrop" onClick={(e) => e.target === e.currentTarget && cancel()}>
          <div
            class={props.wide ? "dialog dialog-wide card" : "dialog card"}
            role="dialog"
            aria-modal="true"
            aria-labelledby={id}
            ref={box}
            onKeyDown={onKey}
          >
            <div class="dialog-head">
              <div class="dialog-heading">
                <h2 id={id}>{props.title}</h2>
                <Show when={props.subtitle}>
                  <p class="hint">{props.subtitle}</p>
                </Show>
              </div>
              <Show when={!props.confirm}>
                <IconButton icon="dismiss" label={t("modal.close")} onClick={cancel} data-autofocus />
              </Show>
            </div>
            <div class="dialog-body">{props.children}</div>
            <Show when={props.confirm}>
              <div class="dialog-actions">
                <Button kind="subtle" disabled={props.busy} onClick={cancel} data-autofocus>
                  {props.cancel}
                </Button>
                <Button kind={props.danger ? "danger" : "strong"} busy={props.busy} onClick={() => props.onConfirm?.()}>
                  {props.confirm}
                </Button>
              </div>
            </Show>
          </div>
        </div>
      </Portal>
    </Show>
  );
}
