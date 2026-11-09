import { useQueryClient } from "@tanstack/react-query";
import { Fingerprint, Lock } from "lucide-react";
import { useActionState, useEffect, useId, useRef } from "react";
import type { ReactElement } from "react";

import { errorMessage, useMessages } from "../../i18n";
import { commands } from "../../ipc/bindings.gen";
import { call } from "../../ipc/call";
import { Button, PasswordField } from "../../ui";
import { quickKeys, useAppState, useDevices } from "./hooks";

/**
 * Compact locked view for the quick panel — centered column: lock icon, title, vault name,
 * passphrase + Touch ID when a device source exists.
 */
export function Unlock(): ReactElement {
  const m = useMessages();
  const qc = useQueryClient();
  const app = useAppState();
  const devices = useDevices();
  const touchId = devices.data?.touchId;
  const vaultName = app.data?.vault?.path.split("/").at(-1) ?? "";
  const [error, submit, busy] = useActionState(async (_p: unknown, form: FormData) => {
    try {
      const v = form.get("passphrase");
      const pass = typeof v === "string" ? v : "";
      await call(commands.unlock(pass, false));
      await qc.invalidateQueries({ queryKey: quickKeys.appState });
      await qc.invalidateQueries({ queryKey: quickKeys.entries });
      return undefined;
    } catch (error_) {
      return errorMessage(m, error_);
    }
  }, undefined);

  const device = async (): Promise<void> => {
    try {
      const r = await call(commands.unlockDevice());
      if (r.unlocked) {
        await qc.invalidateQueries({ queryKey: quickKeys.appState });
        await qc.invalidateQueries({ queryKey: quickKeys.entries });
      }
    } catch (error_) {
      void errorMessage(m, error_);
    }
  };
  const id = useId();
  const form = useRef<HTMLFormElement>(null);
  useEffect(() => {
    form.current?.querySelector("input")?.focus();
  }, []);
  return (
    <main className="bg-card flex h-screen flex-col items-center justify-center gap-4 p-6">
      <Lock className="text-muted size-8" strokeWidth={1.5} aria-hidden="true" />
      <div className="text-center">
        <h1 className="text-ink text-lg font-medium">{m.quick.lockedTitle}</h1>
        {vaultName !== "" && <p className="text-muted text-sm">{vaultName}</p>}
      </div>
      <form ref={form} action={submit} className="flex w-full flex-col gap-3">
        <PasswordField label={m.unlock.passphrase} name="passphrase" id={id} />
        {error !== undefined && <p className="text-danger text-sm">{error}</p>}
        <Button type="submit" loading={busy} block>
          {m.unlock.submit}
        </Button>
      </form>
      {touchId?.enabled === true && (
        <Button type="button" variant="secondary" block onClick={() => void device()}>
          <Fingerprint className="size-4" aria-hidden="true" /> {m.unlock.touchId}
        </Button>
      )}
    </main>
  );
}
