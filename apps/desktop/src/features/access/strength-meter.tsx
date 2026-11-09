import { useEffect, useState } from "react";
import type { ReactElement } from "react";

import { useMessages } from "../../i18n";
import { commands } from "../../ipc/bindings.gen";
import type { Strength_Serialize } from "../../ipc/bindings.gen";
import { cn } from "../../ui";

/** Debounced Rust-side strength check; a query would cache the passphrase. */
export function StrengthMeter({ passphrase }: { passphrase: string }): ReactElement {
  const m = useMessages();
  const [strength, setStrength] = useState<Strength_Serialize | undefined>(undefined);

  useEffect(() => {
    let ignore = false;
    const check = async (): Promise<void> => {
      const next = passphrase === "" ? undefined : await commands.passphraseStrength(passphrase);
      if (!ignore) {
        setStrength(next);
      }
    };
    const t = setTimeout(() => {
      void check();
    }, 250);
    return () => {
      ignore = true;
      clearTimeout(t);
    };
  }, [passphrase]);

  if (passphrase === "" || strength === undefined) {
    return <div className="h-6" />;
  }
  const score = Math.min(Math.max(strength.score, 0), 4);
  return (
    <div className="mt-2 px-4">
      <div className="flex gap-1" aria-hidden="true">
        {[0, 1, 2, 3].map((i) => (
          <div
            key={i}
            className={cn("h-1 flex-1 rounded-full", i < score ? "bg-ink" : "bg-line-strong")}
          />
        ))}
      </div>
      <p className="text-muted mt-1.5 text-xs">{m.strength.labels[score]}</p>
      {strength.problem !== null && (
        <p className="text-danger mt-0.5 text-xs">{strength.problem}</p>
      )}
    </div>
  );
}
