import { vi } from "vite-plus/test";

import type { AppError, AppView_Serialize, Settings_Serialize } from "../ipc/bindings.gen";

const ok = async <T>(data: T): Promise<{ status: "ok"; data: T }> => {
  await Promise.resolve();
  return { status: "ok", data };
};
const err = async (
  code: string,
  message: string,
): Promise<{ status: "error"; error: AppError }> => {
  await Promise.resolve();
  return { status: "error", error: { code, message } };
};

export const DEFAULT_SETTINGS: Settings_Serialize = {
  autoLockMinutes: 15,
  maxSessionHours: 8,
  clipboardSeconds: 45,
  lockOnSleep: true,
  language: "system",
};

function createBindingsMockImpl() {
  const store = {
    names: ["work/github", "work/aws/root", "personal/bank", "personal/passport.pdf", "solo"],
    texts: new Map<string, string>([
      [
        "work/github",
        "ghp_secret\nuser: octocat\ntotp: GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ\nnotes: main",
      ],
      ["work/aws/root", "AKIAEXAMPLE\nuser: root"],
      ["personal/bank", "rosebud\nuser: 4471"],
      ["solo", "v"],
    ]),
    settings: { ...DEFAULT_SETTINGS },
    vault: { path: "/vaults/ks", isGit: true, warnings: [] as string[] },
  };

  type LockedCb = (e: { payload: { reason: string } }) => void;
  type ChangedCb = () => void;
  const lockedCbs = new Set<LockedCb>();
  const changedCbs = new Set<ChangedCb>();

  const emitLocked = (reason: string): void => {
    for (const cb of lockedCbs) {
      cb({ payload: { reason } });
    }
  };
  const emitChanged = (): void => {
    for (const cb of changedCbs) {
      cb();
    }
  };

  const commands = {
    appState: vi.fn(async (): Promise<AppView_Serialize> => {
      await Promise.resolve();
      return { vault: null, unlocked: false, lockReason: null, settings: store.settings };
    }),
    recentVaults: vi.fn(() => ["/vaults/ks"]),
    openVault: vi.fn(async (path: string) => ok({ ...store.vault, path })),
    createVault: vi.fn(async () => ok(store.vault)),
    cloneVault: vi.fn(async () => ok(store.vault)),
    passphraseStrength: vi.fn(() => ({
      score: 3,
      ok: true,
      problem: null,
      suggestions: [],
    })),
    unlock: vi.fn(async (pass: string) =>
      pass === "correct horse battery staple"
        ? ok({ warnings: [] })
        : err("wrong_passphrase", "Wrong passphrase"),
    ),
    lock: vi.fn(async () => {
      emitLocked("manual");
      return ok(null);
    }),
    heartbeat: vi.fn(() => undefined),
    listEntries: vi.fn(async () => ok(store.names)),
    entryDetail: vi.fn(async (name: string) => {
      const text = store.texts.get(name);
      if (text === undefined) {
        if (store.names.includes(name)) {
          return ok({
            name,
            kind: "binary",
            value: null,
            fields: [],
            notesLength: 0,
            hasTotp: false,
            binaryLength: 2048,
          });
        }
        return err("not_found", name);
      }
      const lines = text.split("\n");
      const fields = lines
        .slice(1)
        .map((l) => {
          const i = l.indexOf(": ");
          return i > 0 ? { key: l.slice(0, i), length: l.slice(i + 2).length } : null;
        })
        .filter((f): f is { key: string; length: number } => f !== null);
      return ok({
        name,
        kind: "text",
        value: { key: "value", length: (lines[0] ?? "").length },
        fields: fields.filter((f) => f.key !== "notes" && f.key !== "totp"),
        notesLength: fields.find((f) => f.key === "notes")?.length ?? 0,
        hasTotp: fields.some((f) => f.key === "totp"),
        binaryLength: null,
      });
    }),
    revealField: vi.fn(async (name: string, field: unknown) => {
      const text = store.texts.get(name);
      if (text === undefined) {
        return err("not_found", name);
      }
      if (field === "value") {
        return ok(text.split("\n")[0] ?? "");
      }
      if (field === "totp") {
        return ok("123456");
      }
      const key =
        field === "notes"
          ? "notes"
          : typeof field === "object" && field !== null && "field" in field
            ? String(field.field)
            : "";
      const line = text.split("\n").find((l) => l.startsWith(`${key}: `));
      return line === undefined ? err("not_found", key) : ok(line.slice(key.length + 2));
    }),
    copyField: vi.fn(async () => ok({ clearsInSecs: 45 })),
    totpCode: vi.fn(async () => ok({ code: "123456", validFor: 17 })),
    editEntry: vi.fn(async (name: string) => {
      const text = store.texts.get(name);
      return text === undefined ? err("not_found", name) : ok(text);
    }),
    saveEntry: vi.fn(async (name: string, text: string, overwrite: boolean) => {
      if (store.texts.has(name) && !overwrite) {
        return err("exists", name);
      }
      store.texts.set(name, text);
      emitChanged();
      return ok([]);
    }),
    renameEntry: vi.fn(async (from: string, to: string) => {
      store.texts.set(to, store.texts.get(from) ?? "");
      store.texts.delete(from);
      emitChanged();
      return ok([]);
    }),
    deleteEntry: vi.fn(async (name: string) => {
      store.texts.delete(name);
      emitChanged();
      return ok([]);
    }),
    importBinary: vi.fn(async () => ok([])),
    exportBinary: vi.fn(async () => ok(null)),
    generatePassword: vi.fn(async () => ok("gen3r4t3d")),
    getSettings: vi.fn(() => store.settings),
    setSettings: vi.fn(async (s: Settings_Serialize) => {
      store.settings = s;
      return ok(null);
    }),
    vaultStatus: vi.fn(async () =>
      ok({ path: "/vaults/ks", exists: true, isGit: true, agent: null }),
    ),
  };

  const events = {
    locked: {
      listen: vi.fn((cb: LockedCb) => {
        lockedCbs.add(cb);
        return () => {
          lockedCbs.delete(cb);
        };
      }),
    },
    entriesChanged: {
      listen: vi.fn((cb: ChangedCb) => {
        changedCbs.add(cb);
        return () => {
          changedCbs.delete(cb);
        };
      }),
    },
  };

  return { commands, events, store, emitLocked, emitChanged };
}

/** Exported factory (explicit boundary type via the impl). */
export function createBindingsMock(): ReturnType<typeof createBindingsMockImpl> {
  return createBindingsMockImpl();
}

export type BindingsMock = ReturnType<typeof createBindingsMockImpl>;
