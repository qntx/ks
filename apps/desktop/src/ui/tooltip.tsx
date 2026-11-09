import { Tooltip as RTooltip } from "radix-ui";
import type { ReactElement, ReactNode } from "react";

export function Tooltip({
  content,
  side = "top",
  children,
}: {
  content: string;
  side?: "top" | "right" | "bottom" | "left" | undefined;
  children: ReactNode;
}): ReactElement {
  return (
    <RTooltip.Root>
      <RTooltip.Trigger asChild>{children}</RTooltip.Trigger>
      <RTooltip.Portal>
        <RTooltip.Content
          side={side}
          sideOffset={6}
          className="bg-toast text-on-toast animate-fade-in z-60 rounded-full px-2.5 py-1 text-xs"
        >
          {content}
        </RTooltip.Content>
      </RTooltip.Portal>
    </RTooltip.Root>
  );
}
