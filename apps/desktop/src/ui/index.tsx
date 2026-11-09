/** Thin styled wrappers over Radix primitives (Tailwind v4). */
// oxlint-disable react/button-has-type -- every control receives a literal type prop
import { clsx } from "clsx";
import {
  Dialog as RxDialog,
  Switch as RxSwitch,
  Tabs as RxTabs,
  Toast as RxToast,
  Tooltip as RxTooltip,
} from "radix-ui";
import React from "react";

export function Button(props: {
  kind?: "primary" | "ghost" | "danger";
  children: React.ReactNode;
  onClick?: () => void;
  disabled?: boolean;
  type?: "button" | "submit";
  title?: string;
}): React.JSX.Element {
  const k = props.kind ?? "primary";
  return (
    <button
      type={props.type ?? "button"}
      onClick={props.onClick}
      disabled={props.disabled}
      title={props.title}
      className={clsx(
        "inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-sm font-medium outline-none focus-visible:ring-2 focus-visible:ring-blue-500 disabled:opacity-40",
        k === "primary" && "bg-blue-600 text-white hover:bg-blue-700",
        k === "ghost" &&
          "border border-zinc-300 text-zinc-800 hover:bg-zinc-100 dark:border-zinc-700 dark:text-zinc-200 dark:hover:bg-zinc-800",
        k === "danger" && "bg-red-600 text-white hover:bg-red-700",
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
      className="w-full rounded-md border border-zinc-300 bg-white px-3 py-1.5 text-sm outline-none focus-visible:ring-2 focus-visible:ring-blue-500 dark:border-zinc-700 dark:bg-zinc-900"
    />
  );
  if (props.label === undefined) {
    return el;
  }
  return (
    <label className="block text-left">
      <span className="mb-1 block text-xs text-zinc-500">{props.label}</span>
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
        <RxDialog.Overlay className="fixed inset-0 z-40 bg-black/40" />
        <RxDialog.Content className="fixed top-1/2 left-1/2 z-50 w-[26rem] max-w-[92vw] -translate-x-1/2 -translate-y-1/2 rounded-lg border border-zinc-200 bg-white p-5 shadow-xl outline-none dark:border-zinc-700 dark:bg-zinc-900">
          <RxDialog.Title className="mb-3 text-sm font-semibold">{props.title}</RxDialog.Title>
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
        className="relative h-5 w-9 rounded-full bg-zinc-300 outline-none focus-visible:ring-2 focus-visible:ring-blue-500 data-[state=checked]:bg-blue-600 dark:bg-zinc-700"
      >
        <RxSwitch.Thumb className="block h-4 w-4 translate-x-0.5 rounded-full bg-white transition data-[state=checked]:translate-x-4.5" />
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
        <RxTooltip.Content
          sideOffset={4}
          className="z-50 rounded-md bg-zinc-800 px-2 py-1 text-xs text-white dark:bg-zinc-200 dark:text-zinc-900"
        >
          {props.label}
        </RxTooltip.Content>
      </RxTooltip.Portal>
    </RxTooltip.Root>
  );
}

export function Tabs(props: {
  items: Array<{ id: string; label: string }>;
  value: string;
  onValueChange: (v: string) => void;
  children: React.ReactNode;
}): React.JSX.Element {
  return (
    <RxTabs.Root value={props.value} onValueChange={props.onValueChange}>
      <RxTabs.List className="mb-4 flex gap-1 rounded-md bg-zinc-100 p-1 dark:bg-zinc-800">
        {props.items.map((i) => (
          <RxTabs.Trigger
            key={i.id}
            value={i.id}
            className="flex-1 rounded px-2 py-1 text-sm outline-none focus-visible:ring-2 focus-visible:ring-blue-500 data-[state=active]:bg-white data-[state=active]:shadow dark:data-[state=active]:bg-zinc-900"
          >
            {i.label}
          </RxTabs.Trigger>
        ))}
      </RxTabs.List>
      {props.children}
    </RxTabs.Root>
  );
}

export function Toasts(props: {
  items: Array<{ id: number; text: string }>;
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
          className="rounded-md border border-zinc-200 bg-white px-3 py-2 text-sm shadow-lg dark:border-zinc-700 dark:bg-zinc-800"
        >
          <RxToast.Title>{t.text}</RxToast.Title>
        </RxToast.Root>
      ))}
      <RxToast.Viewport className="fixed right-4 bottom-4 z-50 flex w-80 flex-col gap-2 outline-none" />
    </RxToast.Provider>
  );
}
