import { open as pickDialog } from "@tauri-apps/plugin-dialog";
import { HardDrive } from "lucide-react";
import { useActionState, useState } from "react";
import type { ReactElement } from "react";

import { openVault, unlock, useSession } from "../../app/session";
import { errorMessage, useMessages } from "../../i18n";
import { Button, PasswordField, toast } from "../../ui";
import { AuthStage, GiantTitle, StageTopBar } from "./stage";

export function Unlock(): ReactElement {
  const m = useMessages();
  const vault = useSession((s) => s.vault);
  const lockReason = useSession((s) => s.lockReason);
  const [pass, setPass] = useState("");

  const [error, submit, pending] = useActionState(async () => {
    const p = pass;
    setPass("");
    try {
      await unlock(p);
      return undefined;
    } catch (error_) {
      return errorMessage(m, error_);
    }
  }, undefined);

  const openAnother = async (): Promise<void> => {
    const dir = await pickDialog({ directory: true });
    if (typeof dir !== "string") {
      return;
    }
    try {
      await openVault(dir);
    } catch (error_) {
      toast(errorMessage(m, error_), { tone: "error" });
    }
  };

  const reason = lockReason === undefined ? "" : m.lockReason[lockReason];

  return (
    <AuthStage>
      <StageTopBar />
      <GiantTitle lines={[m.unlock.title]} />
      {vault !== undefined && (
        <div className="bg-card mt-5 flex h-10 items-center gap-2 self-start rounded-full px-4">
          <HardDrive className="text-muted size-4 shrink-0" strokeWidth={2} aria-hidden="true" />
          <span className="text-ink-secondary max-w-72 truncate text-base">{vault.path}</span>
        </div>
      )}
      {reason !== "" && <p className="text-muted mt-2 text-base">{reason}</p>}
      <div className="flex-1" />
      <form action={submit} className="flex flex-col gap-3">
        {error !== undefined && <p className="text-danger px-4 text-xs">{error}</p>}
        <PasswordField
          label={m.unlock.passphrase}
          name="passphrase"
          value={pass}
          // oxlint-disable-next-line jsx-a11y/no-autofocus -- entry point wants immediate focus
          autoFocus
          onChange={(e) => {
            setPass(e.target.value);
          }}
        />
        <Button
          type="submit"
          variant="pill-strong"
          size="xl"
          block
          loading={pending}
          disabled={pass === ""}
        >
          {m.unlock.submit}
        </Button>
      </form>
      <div className="flex justify-center pt-3">
        <Button variant="ghost" size="sm" onClick={() => void openAnother()}>
          {m.unlock.openAnother}
        </Button>
      </div>
    </AuthStage>
  );
}
