import { useSyncExternalStore } from "react";

import type {
  AppError,
  AppView_Serialize,
  EntryDetail_Serialize,
  Locked,
  Reason,
  VaultInfo,
} from "./bindings.gen";
import { commands, events } from "./bindings.gen";

/** What the shell shows. Secrets never live here — reveal/copy values stay inside component state. */
export type Snapshot = {
  phase: "loading" | "welcome" | "unlock" | "main";
  vault: VaultInfo | undefined;
  settings: AppView_Serialize["settings"];
  entries: string[];
  selected: string | undefined;
  detail: EntryDetail_Serialize | undefined;
  lockReason: Reason | undefined;
  toasts: Array<{ id: number; text: string }>;
  warnings: string[];
  error: AppError | undefined;
  busy: boolean;
};

const DEFAULT_SETTINGS: AppView_Serialize["settings"] = {
  autoLockMinutes: 15,
  maxSessionHours: 8,
  clipboardSeconds: 45,
  lockOnSleep: true,
  language: "system",
};

const EMPTY: Snapshot = {
  phase: "loading",
  vault: undefined,
  settings: DEFAULT_SETTINGS,
  entries: [],
  selected: undefined,
  detail: undefined,
  lockReason: undefined,
  toasts: [],
  warnings: [],
  error: undefined,
  busy: false,
};

let snap: Snapshot = { ...EMPTY };
const listeners = new Set<() => void>();
let toastId = 0;

function set(next: Partial<Snapshot>): void {
  snap = { ...snap, ...next };
  for (const l of listeners) {
    l();
  }
}

export function subscribe(l: () => void): () => void {
  listeners.add(l);
  return () => {
    listeners.delete(l);
  };
}

export function useStore(): Snapshot {
  return useSyncExternalStore(subscribe, () => snap);
}

export function getSnap(): Snapshot {
  return snap;
}

export function toast(text: string): void {
  set({ toasts: [...snap.toasts, { id: ++toastId, text }] });
}

export function dismissToast(id: number): void {
  set({ toasts: snap.toasts.filter((t) => t.id !== id) });
}

type R<T> = { status: "ok"; data: T } | { status: "error"; error: AppError };

async function cmd<T>(call: Promise<R<T>>): Promise<T | undefined> {
  const r = await call;
  if (r.status === "ok") {
    return r.data;
  }
  set({ error: r.error });
  return undefined;
}

function applyView(vault: VaultInfo): void {
  set({
    vault,
    phase: "unlock",
  });
}

function unlockedPhase(unlocked: boolean, hasVault: boolean): Snapshot["phase"] {
  if (!hasVault) {
    return "welcome";
  }
  return unlocked ? "main" : "unlock";
}

/** Boot: read app state and subscribe to backend events. */
export async function init(): Promise<void> {
  const v = await commands.appState();
  set({
    vault: v.vault ?? undefined,
    settings: v.settings,
    phase: unlockedPhase(v.unlocked, v.vault !== null),
  });
  if (v.unlocked) {
    await refreshEntries();
  }
}

export async function refreshEntries(): Promise<void> {
  const names = await cmd(commands.listEntries());
  if (names === undefined) {
    return;
  }
  const selected =
    snap.selected !== undefined && names.includes(snap.selected) ? snap.selected : names[0];
  set({ entries: names, selected });
  if (selected === undefined) {
    set({ detail: undefined });
  } else {
    await select(selected, false);
  }
}

export async function select(name: string, setSel = true): Promise<void> {
  if (setSel) {
    set({ selected: name, detail: undefined });
  }
  const d = await cmd(commands.entryDetail(name));
  if (d !== undefined && snap.selected === name) {
    set({ detail: d });
  }
}

export async function doUnlock(pass: string): Promise<boolean> {
  set({ busy: true, error: undefined });
  const r = await cmd(commands.unlock(pass));
  set({ busy: false });
  if (r === undefined) {
    return false;
  }
  set({ phase: "main", lockReason: undefined, warnings: r.warnings });
  await refreshEntries();
  return true;
}

export async function doLock(): Promise<void> {
  await commands.lock();
  applyLock("manual");
}

function applyLock(reason: Reason): void {
  // Clear every non-secret value we cached; component-local secrets
  // unmount with the main view.
  set({
    phase: "unlock",
    entries: [],
    selected: undefined,
    detail: undefined,
    lockReason: reason,
  });
}

function finishOnboarding(v: VaultInfo | undefined): boolean {
  set({ busy: false });
  if (v === undefined) {
    return false;
  }
  applyView(v);
  set({ warnings: v.warnings });
  return true;
}

export async function openVaultAt(path: string): Promise<boolean> {
  set({ busy: true, error: undefined });
  return finishOnboarding(await cmd(commands.openVault(path)));
}

export async function createVaultAt(path: string, pass: string, git: boolean): Promise<boolean> {
  set({ busy: true, error: undefined });
  return finishOnboarding(await cmd(commands.createVault(path, pass, git)));
}

export async function cloneVaultAt(url: string, path: string, pass: string): Promise<boolean> {
  set({ busy: true, error: undefined });
  return finishOnboarding(await cmd(commands.cloneVault(url, path, pass)));
}

export async function saveEntry(name: string, text: string, overwrite: boolean): Promise<boolean> {
  const w = await cmd(commands.saveEntry(name, text, overwrite));
  if (w === undefined) {
    return false;
  }
  set({ warnings: w });
  await refreshEntries();
  set({ selected: name });
  await select(name);
  return true;
}

export async function renameEntry(from: string, to: string): Promise<boolean> {
  const w = await cmd(commands.renameEntry(from, to, false));
  if (w === undefined) {
    return false;
  }
  set({ warnings: w });
  await refreshEntries();
  set({ selected: to });
  await select(to);
  return true;
}

export async function deleteEntry(name: string): Promise<boolean> {
  const w = await cmd(commands.deleteEntry(name));
  if (w === undefined) {
    return false;
  }
  set({ warnings: w });
  set({ selected: undefined, detail: undefined });
  await refreshEntries();
  return true;
}

export function heartbeat(): void {
  void commands.heartbeat();
}

let wired = false;
/** Event listeners — idempotent. */
export async function wireEvents(): Promise<void> {
  if (wired) {
    return;
  }
  wired = true;
  await events.locked.listen((e: { payload: Locked }) => {
    applyLock(e.payload.reason);
  });
  await events.entriesChanged.listen(() => {
    if (snap.phase === "main") {
      void refreshEntries();
    }
  });
}

/** Test hook: reset all state (not the event wiring). */
export function reset(): void {
  snap = { ...EMPTY };
}
