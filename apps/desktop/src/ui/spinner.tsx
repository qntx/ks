import type { ReactElement } from "react";

import { cn } from "./cn";

export function Spinner({ className }: { className?: string | undefined }): ReactElement {
  return (
    <svg
      className={cn("size-5 animate-spin", className)}
      viewBox="0 0 20 20"
      fill="none"
      aria-hidden="true"
    >
      <circle
        cx="10"
        cy="10"
        r="8"
        stroke="currentColor"
        strokeWidth="2.5"
        className="opacity-25"
      />
      <path
        d="M18 10a8 8 0 0 0-8-8"
        stroke="currentColor"
        strokeWidth="2.5"
        strokeLinecap="round"
        className="opacity-90"
      />
    </svg>
  );
}
