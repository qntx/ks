import type { ReactElement } from "react";

export function ProgressRing({
  value,
  size = 20,
}: {
  /** 0..1 fraction remaining. */
  value: number;
  size?: number | undefined;
}): ReactElement {
  const r = (size - 3) / 2;
  const c = 2 * Math.PI * r;
  const clamped = Math.min(Math.max(value, 0), 1);
  return (
    <svg
      width={size}
      height={size}
      viewBox={`0 0 ${size} ${size}`}
      className="-rotate-90"
      aria-hidden="true"
    >
      <circle
        cx={size / 2}
        cy={size / 2}
        r={r}
        fill="none"
        strokeWidth="1.5"
        className="stroke-line-strong"
      />
      <circle
        cx={size / 2}
        cy={size / 2}
        r={r}
        fill="none"
        strokeWidth="1.5"
        strokeLinecap="round"
        className="stroke-ink"
        strokeDasharray={c}
        strokeDashoffset={c * (1 - clamped)}
      />
    </svg>
  );
}
