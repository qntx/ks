import { KeyRound } from "lucide-react";
import type { ReactElement } from "react";

import { cn } from "./cn";

/** The app mark: key glyph on the strong-pill surface (mirrors the app icon). */
export function BrandMark({
  size = 44,
  className,
}: {
  size?: number | undefined;
  className?: string | undefined;
}): ReactElement {
  return (
    <div
      className={cn("bg-pill-strong flex items-center justify-center", className)}
      style={{ width: size, height: size, borderRadius: (size * 14) / 44 }}
    >
      <KeyRound
        className="text-on-pill-strong"
        style={{ width: size * 0.55, height: size * 0.55 }}
        strokeWidth={2}
        aria-hidden="true"
      />
    </div>
  );
}
