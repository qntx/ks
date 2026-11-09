import { useState } from "react";
import type { ReactElement } from "react";

import { useMessages } from "../../i18n";
import { Button, Sheet } from "../../ui";
import { syncNow } from "./hooks";
import { closeConflict, useSyncConflict } from "./store";

/**
 * The sync conflict dialog: `content` conflicts offer keep-local / keep-remote retries; `delete`
 * conflicts only show the CLI steps.
 */
export function ConflictDialog(): ReactElement {
  const m = useMessages();
  const conflict = useSyncConflict();
  const [busy, setBusy] = useState<"local" | "remote" | undefined>(undefined);
  const shown = conflict === undefined ? [] : conflict.entries.slice(0, 6);
  const more = conflict === undefined ? 0 : conflict.entries.length - shown.length;
  const retry = async (keep: "local" | "remote"): Promise<void> => {
    setBusy(keep);
    try {
      await syncNow(m, keep);
    } finally {
      setBusy(undefined);
    }
  };
  return (
    <Sheet
      open={conflict !== undefined}
      onOpenChange={(open) => {
        if (!open) {
          closeConflict();
        }
      }}
      title={m.sync.conflictTitle}
      size="sm"
      footer={
        conflict?.kind === "content" ? (
          <>
            <Button
              variant="secondary"
              loading={busy === "local"}
              onClick={() => void retry("local")}
            >
              {m.sync.keepLocal}
            </Button>
            <Button
              variant="primary"
              loading={busy === "remote"}
              onClick={() => void retry("remote")}
            >
              {m.sync.keepRemote}
            </Button>
          </>
        ) : (
          <Button
            variant="secondary"
            block
            onClick={() => {
              closeConflict();
            }}
          >
            {m.sync.close}
          </Button>
        )
      }
    >
      {conflict?.kind === "delete" ? (
        <>
          <p className="text-muted text-sm">{m.sync.deleteDesc}</p>
          <pre className="bg-card text-ink mt-2 rounded-xl p-3 font-mono text-xs">
            {
              "ks git pull --rebase\nks git rm <file>   # or: ks git add <file>\nks git rebase --continue"
            }
          </pre>
        </>
      ) : (
        <p className="text-muted text-sm">{m.sync.contentDesc}</p>
      )}
      <ul className="mt-2 font-mono text-sm">
        {shown.map((e) => (
          <li key={e} className="text-ink truncate py-0.5">
            {e}
          </li>
        ))}
        {more > 0 && <li className="text-muted py-0.5">{m.sync.more(more)}</li>}
      </ul>
    </Sheet>
  );
}
