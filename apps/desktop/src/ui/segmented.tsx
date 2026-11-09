import { ToggleGroup } from "radix-ui";
import type { ReactElement } from "react";

export function Segmented({
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
  return (
    <ToggleGroup.Root
      type="single"
      value={value}
      onValueChange={(v) => {
        if (v !== "") {
          onChange(v);
        }
      }}
      aria-label={label}
      className="bg-surface flex rounded-full p-0.5"
    >
      {options.map((o) => (
        <ToggleGroup.Item
          key={o.value}
          value={o.value}
          className="text-muted data-[state=on]:bg-card data-[state=on]:text-ink ease-standard h-8 flex-1 cursor-default rounded-full px-3 text-base whitespace-nowrap transition-colors duration-120 outline-none data-[state=on]:shadow-sm"
        >
          {o.label}
        </ToggleGroup.Item>
      ))}
    </ToggleGroup.Root>
  );
}
