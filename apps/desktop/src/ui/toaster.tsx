import { CircleAlert } from "lucide-react";
import { Toast as RToast } from "radix-ui";
import { useSyncExternalStore } from "react";
import type { ReactElement } from "react";

import { cn } from "./cn";
import { dismissToast, getToasts, subscribeToasts } from "./toast-store";

export function Toaster(): ReactElement {
  const list = useSyncExternalStore(subscribeToasts, getToasts);
  return (
    <RToast.Provider duration={3000}>
      {list.map((t) => (
        <RToast.Root
          key={t.id}
          className={cn(
            "animate-toast-in data-[state=closed]:animate-fade-out flex h-11 items-center gap-2 rounded-full px-4 text-base",
            t.tone === "error" ? "bg-danger text-on-danger" : "bg-toast text-on-toast",
          )}
          onOpenChange={(open) => {
            if (!open) {
              dismissToast(t.id);
            }
          }}
        >
          {t.tone === "error" && (
            <CircleAlert className="size-4.5" strokeWidth={2} aria-hidden="true" />
          )}
          <RToast.Title>{t.text}</RToast.Title>
        </RToast.Root>
      ))}
      <RToast.Viewport className="fixed bottom-6 left-1/2 z-60 flex -translate-x-1/2 flex-col items-center gap-2 outline-none" />
    </RToast.Provider>
  );
}
