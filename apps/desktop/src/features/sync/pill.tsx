import { GitBranch } from "lucide-react";
import { useEffect, useState } from "react";
import type { ReactElement } from "react";

import { useMessages } from "../../i18n";
import { Spinner, Tooltip, cn } from "../../ui";
import { useGitStatus, syncNow } from "./hooks";
import { openConflict, useSyncState } from "./store";

/**
 * Sidebar sync pill: branch + sparse counters on the left, one state label on the right; full
 * detail lives in the tooltip. Hidden for non-git vaults.
 */
export function SyncPill(): ReactElement | undefined {
  const m = useMessages();
  const status = useGitStatus();
  const sync = useSyncState();
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const t = setInterval(() => {
      setNow(Date.now());
    }, 30_000);
    return () => {
      clearInterval(t);
    };
  }, []);
  const g = status.data;
  if (g === null || g === undefined) {
    return undefined;
  }
  const state = ((): ReactElement => {
    if (sync.status === "running") {
      return (
        <>
          <Spinner className="size-3.5" />
          <span className="shrink-0 whitespace-nowrap">{m.sync.syncing}</span>
        </>
      );
    }
    if (sync.status === "conflict" || sync.status === "failed") {
      return (
        <>
          <span className="bg-danger size-2 rounded-full" />
          <span
            className="text-danger shrink-0 whitespace-nowrap"
            title={sync.status === "failed" ? sync.message : undefined}
          >
            {sync.status === "conflict" ? m.sync.conflict : m.sync.failed}
          </span>
        </>
      );
    }
    if (g.changes > 0) {
      return (
        <span className="text-muted shrink-0 whitespace-nowrap">{m.sync.changes(g.changes)}</span>
      );
    }
    if (sync.status === "synced") {
      const secs = Math.max(0, Math.floor(now / 1000) - sync.at_secs);
      const label =
        secs < 60
          ? m.sync.syncedJustNow
          : secs < 3600
            ? m.sync.syncedMinutesAgo(Math.floor(secs / 60))
            : m.sync.syncedHoursAgo(Math.floor(secs / 3600));
      return <span className="text-muted shrink-0 whitespace-nowrap">{label}</span>;
    }
    return <span className="text-muted shrink-0 whitespace-nowrap">{m.sync.upToDate}</span>;
  })();
  const detail = [
    g.upstream ?? m.sync.detached,
    `↑${g.ahead} ↓${g.behind}`,
    m.sync.changes(g.changes),
    sync.status === "synced"
      ? `${m.sync.lastSync} ${new Date(sync.at_secs * 1000).toTimeString().slice(0, 5)}`
      : undefined,
  ]
    .filter((p) => p !== undefined)
    .join(" · ");
  return (
    <Tooltip content={detail} side="top">
      <button
        type="button"
        className={cn(
          "hover:bg-hover bg-card mx-3 mb-2 flex h-10 cursor-default items-center gap-2 rounded-full px-4 text-xs",
        )}
        onClick={() => {
          if (sync.status === "conflict") {
            openConflict({ entries: sync.entries, kind: sync.kind });
          } else {
            void syncNow(m);
          }
        }}
      >
        <GitBranch className="text-muted size-4 shrink-0" strokeWidth={2} aria-hidden="true" />
        <span className="text-ink min-w-0 truncate">{g.branch ?? m.sync.detached}</span>
        {g.ahead > 0 && <span className="text-muted shrink-0 tabular-nums">↑{g.ahead}</span>}
        {g.behind > 0 && <span className="text-muted shrink-0 tabular-nums">↓{g.behind}</span>}
        <span className="ml-auto flex shrink-0 items-center gap-1.5">{state}</span>
      </button>
    </Tooltip>
  );
}
