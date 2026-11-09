import { Check, ChevronsUpDown } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { ContextMenu as RContextMenu, DropdownMenu } from "radix-ui";
import type { ReactElement, ReactNode } from "react";

import { cn } from "./cn";

const contentClass =
  "z-50 min-w-44 rounded-2xl bg-card p-1.5 shadow-popover animate-fade-in outline-none";

function itemClass(destructive: boolean | undefined): string {
  return cn(
    "data-highlighted:bg-hover flex h-9 w-full cursor-default items-center gap-2.5 rounded-xl px-3 text-base outline-none select-none",
    destructive === true ? "text-danger" : "text-ink",
  );
}

export type MenuItem = {
  icon?: LucideIcon | undefined;
  label: string;
  shortcut?: string | undefined;
  destructive?: boolean | undefined;
  onSelect: () => void;
};

function Row({ it, Item }: { it: MenuItem; Item: typeof DropdownMenu.Item }): ReactElement {
  return (
    <Item
      className={itemClass(it.destructive)}
      onSelect={() => {
        it.onSelect();
      }}
    >
      {it.icon !== undefined && (
        <it.icon
          className={cn("size-4.5", it.destructive === true ? "text-danger" : "text-muted")}
          strokeWidth={2}
          aria-hidden="true"
        />
      )}
      <span className="flex-1 truncate">{it.label}</span>
      {it.shortcut !== undefined && <span className="text-muted text-xs">{it.shortcut}</span>}
    </Item>
  );
}

export function Menu({
  trigger,
  items,
  align = "start",
}: {
  trigger: ReactNode;
  items: MenuItem[];
  align?: "start" | "center" | "end" | undefined;
}): ReactElement {
  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>{trigger}</DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content className={contentClass} sideOffset={6} align={align}>
          {items.map((it) => (
            <Row key={it.label} it={it} Item={DropdownMenu.Item} />
          ))}
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}

export function ContextMenu({
  items,
  children,
}: {
  items: MenuItem[];
  children: ReactNode;
}): ReactElement {
  return (
    <RContextMenu.Root>
      <RContextMenu.Trigger asChild>{children}</RContextMenu.Trigger>
      <RContextMenu.Portal>
        <RContextMenu.Content className={contentClass}>
          {items.map((it) => (
            <Row key={it.label} it={it} Item={RContextMenu.Item as typeof DropdownMenu.Item} />
          ))}
        </RContextMenu.Content>
      </RContextMenu.Portal>
    </RContextMenu.Root>
  );
}

/** Radio-style dropdown used in settings rows: muted value + chevron. */
export function ChoiceMenu({
  value,
  options,
  onChange,
  label,
}: {
  value: string;
  options: Array<{ value: string; label: string }>;
  onChange: (value: string) => void;
  label?: string | undefined;
}): ReactElement {
  const current = options.find((o) => o.value === value);
  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>
        <button
          type="button"
          aria-label={label}
          className="text-muted hover:bg-hover flex h-9 cursor-default items-center gap-1.5 rounded-full px-3 text-base outline-none"
        >
          {current?.label ?? value}
          <ChevronsUpDown className="size-4" strokeWidth={2} aria-hidden="true" />
        </button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content className={contentClass} sideOffset={6} align="end">
          <DropdownMenu.RadioGroup value={value} onValueChange={onChange}>
            {options.map((o) => (
              <DropdownMenu.RadioItem key={o.value} value={o.value} className={itemClass(false)}>
                <span className="size-4.5" aria-hidden="true" />
                <span className="flex-1 truncate">{o.label}</span>
                <DropdownMenu.ItemIndicator>
                  <Check className="text-accent size-4" strokeWidth={2} aria-hidden="true" />
                </DropdownMenu.ItemIndicator>
              </DropdownMenu.RadioItem>
            ))}
          </DropdownMenu.RadioGroup>
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}
