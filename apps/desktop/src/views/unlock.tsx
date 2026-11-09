import { KeyRound } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type React from "react";

import type { Messages } from "../i18n";
import { doUnlock, useStore } from "../store";
import { Button, Input } from "../ui";
import * as c from "../ui/classes";

export function Unlock(props: { m: Messages }): React.JSX.Element {
  const { m } = props;
  const { busy, error, lockReason, warnings } = useStore();
  const [pass, setPass] = useState("");
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => {
    input.current?.focus();
  }, []);

  const submit = async (): Promise<void> => {
    // Clear the field immediately — the passphrase must not linger in
    // DOM state longer than the unlock call itself.
    setPass("");
    input.current?.blur();
    await doUnlock(pass);
  };

  const errText =
    error === undefined
      ? undefined
      : (m.error[error.code] ?? `${m.error["other"] ?? ""}: ${error.message}`);

  return (
    <main className="bg-paper text-ink flex min-h-screen flex-col items-center justify-center p-8">
      <div className={`${c.card} flex w-[20rem] flex-col items-center gap-3 p-8`}>
        <KeyRound size={36} strokeWidth={1.75} className="text-muted" aria-hidden />
        <h1 className={c.displayTitle}>{m.unlockTitle}</h1>
        {lockReason !== undefined && (
          <p className={c.caption}>{m.lockedReason[lockReason] ?? m.lockedReason["manual"]}</p>
        )}
        <form
          className="w-64 space-y-3"
          onSubmit={(e) => {
            e.preventDefault();
            void submit();
          }}
        >
          <Input
            label={m.passphrase}
            type="password"
            autoComplete="off"
            spellCheck={false}
            value={pass}
            onChange={setPass}
            ref={input}
          />
          {errText !== undefined && <p className="text-danger text-sm">{errText}</p>}
          <Button type="submit" disabled={busy}>
            {m.unlock}
          </Button>
        </form>
        {warnings.map((w) => (
          <p key={w} className={c.caption}>
            {w}
          </p>
        ))}
      </div>
    </main>
  );
}
