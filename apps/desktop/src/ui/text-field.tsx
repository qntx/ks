import { Eye, EyeOff } from "lucide-react";
import { useId, useState } from "react";
import type { ComponentProps, ReactElement, ReactNode } from "react";

import { useMessages } from "../i18n";
import { cn } from "./cn";
import { IconButton } from "./icon-button";

export type TextFieldProps = Omit<ComponentProps<"input">, "placeholder"> & {
  label: string;
  error?: string | undefined;
  hint?: string | undefined;
  /** Slot at the field's trailing edge (e.g. a Browse or eye button). */
  trailing?: ReactNode;
};

/** Floating-label pill input: the border is an overlay span so focus can thicken it. */
export function TextField({
  label,
  error,
  hint,
  trailing,
  id,
  className,
  ...props
}: TextFieldProps): ReactElement {
  const generatedId = useId();
  const inputId = id ?? generatedId;
  return (
    <div className={className}>
      <div className="relative">
        <input
          id={inputId}
          placeholder=" "
          aria-invalid={error !== undefined}
          className={cn(
            "peer bg-card text-ink h-14 w-full rounded-full px-4 pt-4 text-base outline-none",
            "focus:placeholder:text-muted placeholder:text-transparent",
            trailing !== undefined && "pr-14",
          )}
          {...props}
          autoComplete="off"
          spellCheck={false}
          autoCorrect="off"
          autoCapitalize="none"
        />
        <span
          aria-hidden="true"
          className={cn(
            "border-line-strong peer-focus:border-ink pointer-events-none absolute inset-0 rounded-full border peer-focus:border-2",
            error !== undefined && "border-danger peer-focus:border-danger",
          )}
        />
        <label
          htmlFor={inputId}
          className={cn(
            "text-muted ease-standard pointer-events-none absolute top-1/2 left-4 -translate-y-1/2 text-base transition-all duration-200",
            "peer-focus:top-2.5 peer-focus:translate-y-0 peer-focus:text-xs",
            "peer-not-placeholder-shown:top-2.5 peer-not-placeholder-shown:translate-y-0 peer-not-placeholder-shown:text-xs",
          )}
        >
          {label}
        </label>
        {trailing !== undefined && (
          <div className="absolute top-1/2 right-2 -translate-y-1/2">{trailing}</div>
        )}
      </div>
      {(error ?? hint) !== undefined && (
        <p
          className={cn("mt-1.5 px-4 text-xs", error === undefined ? "text-muted" : "text-danger")}
        >
          {error ?? hint}
        </p>
      )}
    </div>
  );
}

export function PasswordField({ trailing, ...props }: TextFieldProps): ReactElement {
  const m = useMessages();
  const [show, setShow] = useState(false);
  const eye = (
    <IconButton
      icon={show ? EyeOff : Eye}
      label={show ? m.common.hide : m.common.show}
      size="sm"
      onClick={() => {
        setShow((v) => !v);
      }}
    />
  );
  return (
    <TextField
      type={show ? "text" : "password"}
      trailing={
        trailing === undefined ? (
          eye
        ) : (
          <div className="flex items-center gap-1">
            {trailing}
            {eye}
          </div>
        )
      }
      {...props}
    />
  );
}

export type TextAreaProps = ComponentProps<"textarea"> & {
  label: string;
  error?: string | undefined;
  hint?: string | undefined;
  mono?: boolean | undefined;
};

export function TextArea({
  label,
  error,
  hint,
  mono,
  id,
  className,
  ...props
}: TextAreaProps): ReactElement {
  const generatedId = useId();
  const inputId = id ?? generatedId;
  return (
    <div className={className}>
      <div className="relative">
        <textarea
          id={inputId}
          placeholder=" "
          aria-invalid={error !== undefined}
          className={cn(
            "peer bg-card text-ink min-h-24 w-full rounded-3xl px-4 pt-6 pb-3 text-base outline-none",
            "focus:placeholder:text-muted placeholder:text-transparent",
            mono === true && "font-mono",
          )}
          {...props}
          autoComplete="off"
          spellCheck={false}
          autoCorrect="off"
          autoCapitalize="none"
        />
        <span
          aria-hidden="true"
          className={cn(
            "border-line-strong peer-focus:border-ink pointer-events-none absolute inset-0 rounded-3xl border peer-focus:border-2",
            error !== undefined && "border-danger peer-focus:border-danger",
          )}
        />
        <label
          htmlFor={inputId}
          className={cn(
            "text-muted ease-standard pointer-events-none absolute top-4 left-4 text-base transition-all duration-200",
            "peer-focus:top-2.5 peer-focus:text-xs",
            "peer-not-placeholder-shown:top-2.5 peer-not-placeholder-shown:text-xs",
          )}
        >
          {label}
        </label>
      </div>
      {(error ?? hint) !== undefined && (
        <p
          className={cn("mt-1.5 px-4 text-xs", error === undefined ? "text-muted" : "text-danger")}
        >
          {error ?? hint}
        </p>
      )}
    </div>
  );
}
