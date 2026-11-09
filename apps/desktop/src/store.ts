import { useSyncExternalStore } from "react";

import type { VaultStatusOut } from "./bindings.gen";

// Single store per design: a tiny state machine reduced by backend
// responses, subscribed via useSyncExternalStore.

type Snapshot =
  | { kind: "loading" }
  | { kind: "status"; status: VaultStatusOut }
  | { kind: "error"; message: string };

let snapshot: Snapshot = { kind: "loading" };
const listeners = new Set<() => void>();

function emit(next: Snapshot): void {
  snapshot = next;
  for (const listener of listeners) {
    listener();
  }
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export function useVaultState(): Snapshot {
  return useSyncExternalStore(subscribe, () => snapshot);
}

export async function refresh(commands: {
  vaultStatus: () => Promise<
    | { status: "ok"; data: VaultStatusOut }
    | { status: "error"; error: { code: string; message: string } }
  >;
}): Promise<void> {
  try {
    const result = await commands.vaultStatus();
    if (result.status === "ok") {
      emit({ kind: "status", status: result.data });
    } else {
      emit({ kind: "error", message: result.error.message });
    }
  } catch (error) {
    emit({ kind: "error", message: error instanceof Error ? error.message : String(error) });
  }
}
