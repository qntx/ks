import { Switch as RSwitch } from "radix-ui";
import type { ReactElement } from "react";

export function Switch({
  checked,
  onCheckedChange,
  label,
}: {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  label?: string | undefined;
}): ReactElement {
  return (
    <RSwitch.Root
      checked={checked}
      onCheckedChange={onCheckedChange}
      aria-label={label}
      className="bg-line-strong data-[state=checked]:bg-accent ease-standard relative h-7 w-11 cursor-default rounded-full transition-colors duration-120 outline-none"
    >
      <RSwitch.Thumb className="bg-card block size-6 translate-x-0.5 rounded-full shadow-sm transition-transform duration-200 data-[state=checked]:translate-x-4.5" />
    </RSwitch.Root>
  );
}
