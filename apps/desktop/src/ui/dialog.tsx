import { X } from "lucide-react";
import { Dialog } from "radix-ui";
import { useState } from "react";
import type { ReactElement, ReactNode } from "react";

import { useMessages } from "../i18n";
import { Button } from "./button";
import { cn } from "./cn";
import { IconButton } from "./icon-button";

/**
 * Centered dialog surface; `footer` sits under the scrollable body. With `action` the body and
 * footer are wrapped in a form so Enter submits and the primary button is `type="submit"`.
 */
export function Sheet({
  open,
  onOpenChange,
  title,
  description,
  size = "md",
  action,
  footer,
  children,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description?: string | undefined;
  size?: "sm" | "md" | undefined;
  action?: ((formData: FormData) => void | Promise<void>) | undefined;
  footer?: ReactNode;
  children: ReactNode;
}): ReactElement {
  const m = useMessages();
  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="bg-overlay data-[state=open]:animate-fade-in data-[state=closed]:animate-fade-out fixed inset-0 z-40" />
        <Dialog.Content
          className={cn(
            "rounded-dialog bg-card fixed top-1/2 left-1/2 z-50 flex max-h-[86vh] w-[calc(100vw-2rem)] -translate-1/2 flex-col p-7 outline-none",
            "data-[state=open]:animate-dialog-in data-[state=closed]:animate-dialog-out",
            size === "sm" ? "max-w-105" : "max-w-140",
          )}
        >
          <Dialog.Title className="text-ink text-3xl font-bold tracking-tight">
            {title}
          </Dialog.Title>
          {description !== undefined && (
            <Dialog.Description className="text-muted mt-1 text-base">
              {description}
            </Dialog.Description>
          )}
          {action === undefined ? (
            <>
              <div className="mt-5 min-h-0 flex-1 overflow-y-auto">{children}</div>
              {footer !== undefined && <div className="mt-6 flex justify-end gap-2">{footer}</div>}
            </>
          ) : (
            <form action={action} className="mt-5 flex min-h-0 flex-1 flex-col">
              <div className="min-h-0 flex-1 overflow-y-auto">{children}</div>
              {footer !== undefined && <div className="mt-6 flex justify-end gap-2">{footer}</div>}
            </form>
          )}
          {/* The close button comes last in DOM order so it is not the initial focus target. */}
          <Dialog.Close asChild>
            <IconButton icon={X} label={m.common.close} className="absolute top-5 right-5" />
          </Dialog.Close>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export function ConfirmDialog({
  open,
  onOpenChange,
  title,
  message,
  confirmLabel,
  cancelLabel,
  destructive,
  onConfirm,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  message: string;
  confirmLabel: string;
  cancelLabel: string;
  destructive?: boolean | undefined;
  onConfirm: () => Promise<void>;
}): ReactElement {
  const [busy, setBusy] = useState(false);
  const confirm = async (): Promise<void> => {
    setBusy(true);
    try {
      await onConfirm();
    } finally {
      setBusy(false);
    }
  };
  return (
    <Sheet
      open={open}
      onOpenChange={onOpenChange}
      title={title}
      size="sm"
      footer={
        <>
          <Button
            variant="secondary"
            onClick={() => {
              onOpenChange(false);
            }}
          >
            {cancelLabel}
          </Button>
          <Button
            variant={destructive === true ? "danger" : "primary"}
            loading={busy}
            onClick={() => {
              void confirm();
            }}
          >
            {confirmLabel}
          </Button>
        </>
      }
    >
      <p className="text-ink-secondary text-base">{message}</p>
    </Sheet>
  );
}
