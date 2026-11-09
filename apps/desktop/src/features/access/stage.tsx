import { ChevronLeft } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { Dialog } from "radix-ui";
import type { ReactElement, ReactNode } from "react";

import { useMessages } from "../../i18n";
import { BrandMark, IconButton } from "../../ui";

/** Full `base` stage: a centered 440px column under the drag strip. */
export function AuthStage({ children }: { children: ReactNode }): ReactElement {
  return (
    <div className="bg-window relative h-dvh overflow-hidden">
      <div data-tauri-drag-region className="absolute inset-x-0 top-0 z-30 h-12" />
      <div className="mx-auto flex h-full w-full max-w-110 flex-col px-5 pt-12 pb-6">
        {children}
      </div>
    </div>
  );
}

export function StageTopBar({ onBack }: { onBack?: (() => void) | undefined }): ReactElement {
  const m = useMessages();
  return (
    <div className="flex h-11 items-center justify-between">
      {onBack === undefined ? (
        <span className="size-11" />
      ) : (
        <IconButton
          icon={ChevronLeft}
          label={m.common.back}
          size="lg"
          variant="card"
          onClick={onBack}
        />
      )}
      <BrandMark />
    </div>
  );
}

export function GiantTitle({ lines }: { lines: string[] }): ReactElement {
  return (
    <h1 className="text-giant text-ink pt-10 font-bold tracking-[-0.045em]">
      {lines.map((line) => (
        <span key={line} className="block">
          {line}
        </span>
      ))}
    </h1>
  );
}

export function StageNote({ children }: { children: ReactNode }): ReactElement {
  return <p className="text-muted pt-3 text-base">{children}</p>;
}

/** A card that rises from the bottom edge over a dimmed stage. */
export function RisingCard({
  open,
  onOpenChange,
  children,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  children: ReactNode;
}): ReactElement {
  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="bg-overlay data-[state=open]:animate-fade-in data-[state=closed]:animate-fade-out fixed inset-0 z-40" />
        <Dialog.Content className="data-[state=open]:animate-card-rise data-[state=closed]:animate-card-sink fixed inset-x-0 bottom-2 z-50 px-2 outline-none">
          <div className="rounded-sheet bg-card mx-auto flex w-full max-w-110 flex-col gap-4 p-6">
            {children}
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export function CardHeader({
  icon: Icon,
  title,
  description,
}: {
  icon: LucideIcon;
  title: string;
  description: string;
}): ReactElement {
  return (
    <>
      <Icon className="text-ink size-7" strokeWidth={2} aria-hidden="true" />
      <Dialog.Title className="text-display text-ink font-extrabold tracking-tight">
        {title}
      </Dialog.Title>
      <Dialog.Description className="text-muted -mt-2 text-base">{description}</Dialog.Description>
    </>
  );
}
