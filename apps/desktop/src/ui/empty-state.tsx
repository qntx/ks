import type { LucideIcon } from "lucide-react";
import type { ReactElement, ReactNode } from "react";

export function EmptyState({
  icon: Icon,
  title,
  description,
  action,
}: {
  icon: LucideIcon;
  title: string;
  description?: string | undefined;
  action?: ReactNode;
}): ReactElement {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-3 p-8 text-center">
      <div className="bg-surface flex size-14 items-center justify-center rounded-2xl">
        <Icon className="text-muted size-6" strokeWidth={2} aria-hidden="true" />
      </div>
      <p className="text-ink text-lg font-semibold">{title}</p>
      {description !== undefined && <p className="text-muted max-w-72 text-base">{description}</p>}
      {action}
    </div>
  );
}
