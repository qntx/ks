import { Slider as RSlider } from "radix-ui";
import type { ReactElement } from "react";

export function Slider({
  value,
  min,
  max,
  onChange,
  label,
}: {
  value: number;
  min: number;
  max: number;
  onChange: (value: number) => void;
  label?: string | undefined;
}): ReactElement {
  return (
    <RSlider.Root
      className="relative flex h-6 w-full touch-none items-center select-none"
      value={[value]}
      min={min}
      max={max}
      onValueChange={(v) => {
        if (v[0] !== undefined) {
          onChange(v[0]);
        }
      }}
      aria-label={label}
    >
      <RSlider.Track className="bg-line-strong relative h-1.5 flex-1 rounded-full">
        <RSlider.Range className="bg-ink absolute h-full rounded-full" />
      </RSlider.Track>
      <RSlider.Thumb
        className="bg-card ring-line-strong focus-visible:ring-focus shadow-thumb block size-5 rounded-full ring-1 outline-none focus-visible:ring-2"
        aria-label={label}
      />
    </RSlider.Root>
  );
}
