import { useSyncExternalStore } from "react";

import { queryClient, queryKeys } from "../../app/query";
import { commands, events } from "../../ipc/bindings.gen";
import type { ConflictKind, SyncState } from "../../ipc/bindings.gen";

/** The open conflict entry list the dialog shows. */
export type SyncConflict = { entries: string[]; kind: ConflictKind };

let current: SyncState = { status: "idle" };
let started = false;
const listeners = new Set<() => void>();

let conflict: SyncConflict | undefined = undefined;
const conflictListeners = new Set<() => void>();

function notify(): void {
  for (const l of listeners) {
    l();
  }
}

function notifyConflict(): void {
  for (const l of conflictListeners) {
    l();
  }
}

/** Starts the initial read + `sync-state` subscription once per session. */
async function ensureStarted(): Promise<void> {
  if (started) {
    return;
  }
  started = true;
  try {
    current = await commands.syncState();
  } catch {
    current = { status: "idle" };
  }
  notify();
  await events.syncState.listen((e) => {
    current = e.payload;
    void queryClient.invalidateQueries({ queryKey: queryKeys.gitStatus });
    notify();
  });
}

/** Live sync state for the pill (initial command + `sync-state` events). */
export function useSyncState(): SyncState {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      void ensureStarted();
      return () => {
        listeners.delete(l);
      };
    },
    () => current,
  );
}

/** Opens the conflict dialog. */
export function openConflict(c: SyncConflict): void {
  conflict = c;
  notifyConflict();
}

/** Closes the conflict dialog. */
export function closeConflict(): void {
  conflict = undefined;
  notifyConflict();
}

/** The pending conflict for the dialog; `null` when closed. */
export function useSyncConflict(): SyncConflict | undefined {
  return useSyncExternalStore(
    (l) => {
      conflictListeners.add(l);
      return () => {
        conflictListeners.delete(l);
      };
    },
    () => conflict,
  );
}

/** Test-only reset: clears the cached state, conflict and boot flag. */
export function resetSync(): void {
  current = { status: "idle" };
  started = false;
  conflict = undefined;
}
