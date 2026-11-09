import { Check, Copy } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { ReactElement } from "react";

import { useMessages } from "../../i18n";
import type { FieldRef } from "../../ipc/bindings.gen";
import { IconButton } from "../../ui";
import { useCopyField } from "./hooks";

/** Icon button that copies a field server-side and flips to an accent Check for 1.5s. */
export function CopyButton({ name, field }: { name: string; field: FieldRef }): ReactElement {
  const m = useMessages();
  const copyField = useCopyField();
  const [copied, setCopied] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  useEffect(
    () => () => {
      clearTimeout(timer.current);
    },
    [],
  );

  const copy = async (): Promise<void> => {
    const ok = await copyField(name, field);
    if (ok) {
      setCopied(true);
      clearTimeout(timer.current);
      timer.current = setTimeout(() => {
        setCopied(false);
      }, 1500);
    }
  };

  return (
    <IconButton
      icon={copied ? Check : Copy}
      label={m.common.copy}
      size="sm"
      className={copied ? "text-accent" : undefined}
      onClick={() => {
        void copy();
      }}
    />
  );
}
