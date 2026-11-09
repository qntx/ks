import { useEffect, useRef, useState } from "react";
import type React from "react";

import type { Messages } from "../i18n";
import { doUnlock, useStore } from "../store";
import { Button, Input } from "../ui";

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
    <main className="flex min-h-screen flex-col items-center justify-center gap-4 bg-white p-8 text-zinc-900 dark:bg-zinc-950 dark:text-zinc-100">
      <h1 className="text-xl font-semibold">{m.unlockTitle}</h1>
      {lockReason !== undefined && (
        <p className="text-sm text-zinc-500">
          {m.lockedReason[lockReason] ?? m.lockedReason["manual"]}
        </p>
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
        {errText !== undefined && <p className="text-sm text-red-600">{errText}</p>}
        <Button type="submit" disabled={busy}>
          {m.unlock}
        </Button>
      </form>
      {warnings.map((w) => (
        <p key={w} className="text-xs text-amber-600">
          {w}
        </p>
      ))}
    </main>
  );
}
