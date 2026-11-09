import { useActionState, useState } from "react";
import type { ReactElement } from "react";

import { errorMessage, useMessages } from "../../i18n";
import { commands } from "../../ipc/bindings.gen";
import { call } from "../../ipc/call";
import { Button, PasswordField, toast } from "../../ui";
import { NewPassphraseFields } from "./fields";

/** Change the master passphrase — the key is untouched, so devices keep working. */
export function PassphrasePage({ onDone }: { onDone: () => void }): ReactElement {
  const m = useMessages();
  const [current, setCurrent] = useState("");
  const [pass, setPass] = useState("");
  const [repeat, setRepeat] = useState("");
  const [error, submit, busy] = useActionState(async () => {
    const c = current;
    const n = pass;
    setCurrent("");
    setPass("");
    setRepeat("");
    try {
      await call(commands.changePassphrase(c, n));
      toast(m.maintenance.passphraseChanged);
      onDone();
      return undefined;
    } catch (error_) {
      return errorMessage(m, error_);
    }
  }, undefined);
  const mismatch = repeat !== "" && pass !== repeat;
  const canSubmit = current !== "" && pass !== "" && !mismatch && pass !== "" && pass === repeat;
  return (
    <form action={submit} className="flex flex-col gap-4">
      <PasswordField
        label={m.maintenance.currentPassphrase}
        name="current"
        value={current}
        onChange={(e) => {
          setCurrent(e.target.value);
        }}
      />
      <NewPassphraseFields pass={pass} repeat={repeat} onPass={setPass} onRepeat={setRepeat} />
      {error !== undefined && <p className="text-danger text-sm">{error}</p>}
      <p className="text-muted text-sm">{m.maintenance.passphraseNote}</p>
      <Button type="submit" block loading={busy} disabled={!canSubmit}>
        {m.maintenance.changePassphrase}
      </Button>
    </form>
  );
}
