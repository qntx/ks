import { cva } from "class-variance-authority";
import type { VariantProps } from "class-variance-authority";
import type { LucideIcon } from "lucide-react";
import type { ComponentProps, ReactElement } from "react";

import { cn } from "./cn";
import { Spinner } from "./spinner";

const buttonVariants = cva(
  "ease-standard relative inline-flex cursor-default items-center justify-center gap-2 rounded-full font-semibold transition-[background-color,transform,opacity] duration-120 active:scale-[0.97] disabled:pointer-events-none disabled:opacity-50",
  {
    variants: {
      variant: {
        primary: "bg-primary text-on-primary",
        secondary: "bg-surface text-ink",
        ghost: "text-ink hover:bg-hover",
        danger: "bg-danger text-on-danger",
        pill: "bg-pill text-on-pill border-pill-edge border",
        "pill-strong": "bg-pill-strong text-on-pill-strong border-pill-strong-edge border",
      },
      size: {
        sm: "h-8 px-3 text-xs",
        md: "h-10 px-4 text-base",
        lg: "h-12 px-5 text-base",
        xl: "h-15.5 px-6 text-xl font-medium",
      },
      block: {
        true: "w-full",
      },
    },
    defaultVariants: { variant: "primary", size: "md" },
  },
);

export type ButtonProps = ComponentProps<"button"> &
  VariantProps<typeof buttonVariants> & {
    /** `start` leads with the icon in a 36pt slot and lets the label fill. */
    align?: "center" | "start" | undefined;
    icon?: LucideIcon | undefined;
    loading?: boolean | undefined;
  };

export function Button({
  variant,
  size,
  block,
  align = "center",
  icon: Icon,
  loading,
  type = "button",
  disabled,
  className,
  children,
  ...props
}: ButtonProps): ReactElement {
  const inactive = disabled === true || loading === true;
  return (
    <button
      type={type === "submit" ? "submit" : type === "reset" ? "reset" : "button"}
      disabled={inactive}
      aria-busy={loading === true}
      className={cn(buttonVariants({ variant, size, block }), className)}
      {...props}
    >
      <span
        className={cn(
          "flex min-w-0 items-center gap-2",
          align === "start" && "w-full",
          loading === true && "invisible",
        )}
      >
        {Icon !== undefined &&
          (align === "start" ? (
            <span className="flex size-9 shrink-0 items-center justify-center">
              <Icon className="size-5" strokeWidth={2} aria-hidden="true" />
            </span>
          ) : (
            <Icon className="size-5 shrink-0" strokeWidth={2} aria-hidden="true" />
          ))}
        <span className={cn("truncate", align === "start" && "flex-1 text-left")}>{children}</span>
      </span>
      {loading === true && (
        <span className="absolute inset-0 flex items-center justify-center">
          <Spinner />
        </span>
      )}
    </button>
  );
}
