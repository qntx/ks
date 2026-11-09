import { useQueryClient } from "@tanstack/react-query";
import { useActionState, useState } from "react";
import type { ReactElement } from "react";

import { queryKeys } from "../../app/query";
import { errorMessage, useMessages } from "../../i18n";
import { commands } from "../../ipc/bindings.gen";
import type { RotateReport } from "../../ipc/bindings.gen";
import { call } from "../../ipc/call";
import { Button, PasswordField, Spinner, toast } from "../../ui";
import { syncNow, useGitStatus } from "../sync";
import { NewPassphraseFields } from "./fields";
import { useRotationPending } from "./hooks";

/** Rotate the vault key: intro → optional sync gate → fields → spinner → done. */
export function RotatePage({ onDone }: { onDone: () => void }): ReactElement {
  const m = useMessages();
  const qc = useQueryClient();
  const pending = useRotationPending();
  const git = useGitStatus();
  const [current, setCurrent] = useState("");
  const [pass, setPass] = useState("");
  const [repeat, setRepeat] = useState("");
  const [done, setDone] = useState<RotateReport | undefined>(undefined);
  const [error, submit, busy] = useActionState(async () => {
    const c = current;
    const n = pending.data === true ? null : pass;
    setCurrent("");
    setPass("");
    setRepeat("");
    try {
      const report = await call(commands.rotateKey(c, n));
      for (const w of report.warnings) {
        toast(w);
      }
      await Promise.all(
        [
          queryKeys.devices,
          queryKeys.gitStatus,
          queryKeys.entries,
          queryKeys.entryAll,
          queryKeys.rotationPending,
        ].map(async (k) => qc.invalidateQueries({ queryKey: k })),
      );
      setDone(report);
      return undefined;
    } catch (error_) {
      return errorMessage(m, error_);
    }
  }, undefined);

  if (pending.isPending || pending.data === undefined) {
    return (
      <div className="flex justify-center py-8">
        <Spinner className="size-5" />
      </div>
    );
  }
  if (done !== undefined) {
    return (
      <div className="flex flex-col gap-3">
        <p className="text-ink text-sm">{m.maintenance.rotateDone(done.entries)}</p>
        <p className="text-muted text-sm">{m.maintenance.rotateNextSteps}</p>
        <Button onClick={onDone}>{m.common.done}</Button>
      </div>
    );
  }
  const pendingRotation = pending.data;
  const dirty = (git.data?.changes ?? 0) > 0;
  const isGit = git.data !== null && git.data !== undefined;
  const errText = error;
  if (pendingRotation) {
    return (
      <form action={submit} className="flex flex-col gap-4">
        <p className="text-muted text-sm">{m.maintenance.finishRotationDesc}</p>
        <PasswordField
          label={m.maintenance.passphrase}
          name="current"
          value={current}
          onChange={(e) => {
            setCurrent(e.target.value);
          }}
        />
        {errText !== undefined && <p className="text-danger text-sm wrap-break-word">{errText}</p>}
        <Button type="submit" block loading={busy} disabled={current === ""}>
          {m.maintenance.finishRotation}
        </Button>
        {busy && <p className="text-muted text-sm">{m.maintenance.reencrypting}</p>}
      </form>
    );
  }
  const mismatch = repeat !== "" && pass !== repeat;
  const canSubmit =
    current !== "" && pass !== "" && pass === repeat && !mismatch && !(isGit && dirty);
  return (
    <div className="flex flex-col gap-4">
      <p className="text-muted text-sm">{m.maintenance.rotateIntro}</p>
      {isGit && dirty && (
        <div className="bg-surface rounded-group flex items-center justify-between gap-3 p-3">
          <p className="text-muted text-sm">{m.maintenance.rotateDirtyNote}</p>
          <Button
            size="sm"
            className="shrink-0"
            onClick={() => {
              void syncNow(m);
            }}
          >
            {m.maintenance.syncNow}
          </Button>
        </div>
      )}
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
        {errText !== undefined && <p className="text-danger text-sm wrap-break-word">{errText}</p>}
        <Button type="submit" block loading={busy} disabled={!canSubmit}>
          {m.maintenance.rotateKey}
        </Button>
        {busy && <p className="text-muted text-sm">{m.maintenance.reencrypting}</p>}
      </form>
    </div>
  );
}
