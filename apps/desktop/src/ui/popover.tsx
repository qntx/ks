import { Popover as RPopover } from "radix-ui";
import type { ReactElement, ReactNode } from "react";

import { cn } from "./cn";

export function Popover({
  open,
  onOpenChange,
  trigger,
  className,
  children,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  trigger: ReactNode;
  className?: string | undefined;
  children: ReactNode;
}): ReactElement {
  return (
    <RPopover.Root open={open} onOpenChange={onOpenChange}>
      <RPopover.Trigger asChild>{trigger}</RPopover.Trigger>
      <RPopover.Portal>
        <RPopover.Content
          sideOffset={8}
          className={cn(
            "bg-card shadow-popover animate-fade-in z-50 w-80 rounded-3xl p-4 outline-none",
            className,
          )}
        >
          {children}
        </RPopover.Content>
      </RPopover.Portal>
    </RPopover.Root>
  );
}
