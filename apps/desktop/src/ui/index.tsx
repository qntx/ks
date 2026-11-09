/** Thin styled wrappers over Radix primitives (Tailwind v4). */
// oxlint-disable react/button-has-type -- every control receives a literal type prop
import { clsx } from "clsx";
import {
  Dialog as RxDialog,
  Switch as RxSwitch,
  ToggleGroup as RxToggleGroup,
  Toast as RxToast,
  Tooltip as RxTooltip,
} from "radix-ui";
import React from "react";

import * as c from "./classes";

export function Button(props: {
  kind?: "primary" | "secondary" | "ghost" | "danger";
  children: React.ReactNode;
  onClick?: () => void;
  disabled?: boolean;
  type?: "button" | "submit";
  title?: string;
}): React.JSX.Element {
  return (
    <button
      type={props.type ?? "button"}
      onClick={props.onClick}
      disabled={props.disabled}
      title={props.title}
      className={clsx(
        "focus-visible:ring-ink/40 outline-none focus-visible:ring-2",
        c.button[props.kind ?? "primary"],
      )}
    >
      {props.children}
    </button>
  );
}

export const Input = React.forwardRef<
  HTMLInputElement,
  {
    label?: string;
    type?: string;
    value: string;
    onChange: (v: string) => void;
    placeholder?: string;
    autoComplete?: string;
    spellCheck?: boolean;
    onKeyDown?: React.KeyboardEventHandler<HTMLInputElement>;
  }
>(function Input(props, ref) {
  const el = (
    <input
      ref={ref}
      type={props.type ?? "text"}
      value={props.value}
      onChange={(e) => props.onChange(e.target.value)}
      onKeyDown={props.onKeyDown}
      placeholder={props.placeholder}
      autoComplete={props.autoComplete}
      spellCheck={props.spellCheck}
      aria-label={props.label}
      className={c.input}
    />
  );
  if (props.label === undefined) {
    return el;
  }
  return (
    <label className="block text-left">
      <span className={`${c.caption} mb-1 block`}>{props.label}</span>
      {el}
    </label>
  );
});

export function Dialog(props: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  children: React.ReactNode;
}): React.JSX.Element {
  return (
    <RxDialog.Root open={props.open} onOpenChange={props.onOpenChange}>
      <RxDialog.Portal>
        <RxDialog.Overlay className={c.dialogOverlay} />
        <RxDialog.Content className={c.dialogContent}>
          <RxDialog.Title className={`${c.title} mb-3`}>{props.title}</RxDialog.Title>
          {props.children}
        </RxDialog.Content>
      </RxDialog.Portal>
    </RxDialog.Root>
  );
}

export function Switch(props: {
  checked: boolean;
  onCheckedChange: (v: boolean) => void;
  label: string;
}): React.JSX.Element {
  return (
    <label className="flex items-center justify-between gap-3 text-sm">
      <span>{props.label}</span>
      <RxSwitch.Root
        checked={props.checked}
        onCheckedChange={props.onCheckedChange}
        aria-label={props.label}
        className={c.switchTrack}
      >
        <RxSwitch.Thumb className={c.switchThumb} />
      </RxSwitch.Root>
    </label>
  );
}

export function Tooltip(props: { label: string; children: React.ReactNode }): React.JSX.Element {
  return (
    <RxTooltip.Root>
      <RxTooltip.Trigger asChild>
        <span>{props.children}</span>
      </RxTooltip.Trigger>
      <RxTooltip.Portal>
        <RxTooltip.Content sideOffset={4} className={c.tooltip}>
          {props.label}
        </RxTooltip.Content>
      </RxTooltip.Portal>
    </RxTooltip.Root>
  );
}

export function Segmented(props: {
  items: Array<{ id: string; label: string }>;
  value: string;
  onValueChange: (v: string) => void;
}): React.JSX.Element {
  return (
    <RxToggleGroup.Root
      type="single"
      value={props.value}
      onValueChange={props.onValueChange}
      className={c.segmented.root}
    >
      {props.items.map((i) => (
        <RxToggleGroup.Item key={i.id} value={i.id} className={c.segmented.item}>
          {i.label}
        </RxToggleGroup.Item>
      ))}
    </RxToggleGroup.Root>
  );
}

export function Toasts(props: {
  items: Array<{ id: number; text: string; ok?: boolean }>;
  onDismiss: (id: number) => void;
}): React.JSX.Element {
  return (
    <RxToast.Provider duration={4000}>
      {props.items.map((t) => (
        <RxToast.Root
          key={t.id}
          onOpenChange={(open) => {
            if (!open) {
              props.onDismiss(t.id);
            }
          }}
          className={c.toast}
        >
          <RxToast.Title className="flex items-center gap-2">
            {t.ok === true && (
              <span className="text-ok" aria-hidden>
                ✓
              </span>
            )}
            {t.text}
          </RxToast.Title>
        </RxToast.Root>
      ))}
      <RxToast.Viewport className="fixed right-4 bottom-4 z-50 flex w-80 flex-col gap-2 outline-none" />
    </RxToast.Provider>
  );
}
