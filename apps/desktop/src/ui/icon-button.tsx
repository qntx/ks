import type { LucideIcon } from "lucide-react";
import type { ComponentProps, ReactElement } from "react";

import { cn } from "./cn";
import { Tooltip } from "./tooltip";

const SIZES = {
  sm: "size-8",
  md: "size-10",
  lg: "size-11",
} as const;

const VARIANTS = {
  ghost: "hover:bg-hover",
  surface: "bg-surface",
  card: "bg-card",
} as const;

export type IconButtonProps = Omit<ComponentProps<"button">, "children" | "aria-label"> & {
  icon: LucideIcon;
  /** Required: used as the aria-label and the tooltip. */
  label: string;
  variant?: keyof typeof VARIANTS | undefined;
  size?: keyof typeof SIZES | undefined;
};

export function IconButton({
  icon: Icon,
  label,
  variant = "ghost",
  size = "md",
  className,
  ...props
}: IconButtonProps): ReactElement {
  return (
    <Tooltip content={label}>
      <button
        type="button"
        aria-label={label}
        className={cn(
          "ease-standard inline-flex cursor-default items-center justify-center rounded-full transition-[background-color,transform,opacity] duration-120 active:scale-[0.97] disabled:pointer-events-none disabled:opacity-50",
          SIZES[size],
          VARIANTS[variant],
          className,
        )}
        {...props}
      >
        <Icon className="size-4.5" strokeWidth={2} aria-hidden="true" />
      </button>
    </Tooltip>
  );
}
