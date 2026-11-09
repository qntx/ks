import { useSyncExternalStore } from "react";

import { commands, events } from "../ipc/bindings.gen";
import type { Reason, Settings_Serialize, VaultInfo } from "../ipc/bindings.gen";
import { call } from "../ipc/call";
import { toast } from "../ui";
import { queryClient } from "./query";

export type Phase = "loading" | "welcome" | "locked" | "unlocked";

export type SessionState = {
  phase: Phase;
  vault: VaultInfo | undefined;
  settings: Settings_Serialize;
  lockReason: Reason | undefined;
};

const DEFAULT_SETTINGS: Settings_Serialize = {
  autoLockMinutes: 15,
  maxSessionHours: 8,
  clipboardSeconds: 45,
  lockOnSleep: true,
  language: "system",
};

let state: SessionState = {
  phase: "loading",
  vault: undefined,
  settings: DEFAULT_SETTINGS,
  lockReason: undefined,
};

const listeners = new Set<() => void>();
let wired = false;

function set(patch: Partial<SessionState>): void {
  state = { ...state, ...patch };
  for (const l of listeners) {
    l();
  }
}

function subscribe(l: () => void): () => void {
  listeners.add(l);
  return () => {
    listeners.delete(l);
  };
}

export function useSession<T>(selector: (s: SessionState) => T): T {
  return useSyncExternalStore(subscribe, () => selector(state));
}

function onLocked(reason: Reason): void {
  queryClient.clear();
  set({ phase: "locked", lockReason: reason });
}

/** Loads `app_state` once and wires the backend `locked` event. */
export async function boot(): Promise<void> {
  if (!wired) {
    wired = true;
    void events.locked.listen((e) => {
      onLocked(e.payload.reason);
    });
  }
  const view = await commands.appState();
  set({
    phase: view.vault === null ? "welcome" : view.unlocked ? "unlocked" : "locked",
    vault: view.vault ?? undefined,
    settings: view.settings,
  });
}

export async function unlock(passphrase: string): Promise<void> {
  const r = await call(commands.unlock(passphrase));
  for (const w of r.warnings) {
    toast(w);
  }
  set({ phase: "unlocked", lockReason: undefined });
}

export async function lock(): Promise<void> {
  await call(commands.lock());
  onLocked("manual");
}

export async function openVault(path: string): Promise<void> {
  const vault = await call(commands.openVault(path));
  queryClient.clear();
  set({ phase: "locked", vault, lockReason: undefined });
}

export async function createVault(path: string, passphrase: string, git: boolean): Promise<void> {
  const vault = await call(commands.createVault(path, passphrase, git));
  for (const w of vault.warnings) {
    toast(w);
  }
  queryClient.clear();
  set({ phase: "unlocked", vault, lockReason: undefined });
}

export async function cloneVault(url: string, path: string, passphrase: string): Promise<void> {
  const vault = await call(commands.cloneVault(url, path, passphrase));
  for (const w of vault.warnings) {
    toast(w);
  }
  queryClient.clear();
  set({ phase: "unlocked", vault, lockReason: undefined });
}

/** Saves a partial settings patch; `set_settings` replaces the whole file. */
export async function saveSettings(patch: Partial<Settings_Serialize>): Promise<void> {
  const next = { ...state.settings, ...patch };
  await call(commands.setSettings(next));
  set({ settings: next });
}

/** Test hook: resets the store and event wiring between tests. */
export function resetSession(): void {
  wired = false;
  state = {
    phase: "loading",
    vault: undefined,
    settings: DEFAULT_SETTINGS,
    lockReason: undefined,
  };
}
