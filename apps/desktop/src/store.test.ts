// oxlint-disable typescript/require-await -- test mocks resolve synchronously by design
// oxlint-disable vitest/prefer-strict-equal -- toEqual suffices for plain fixtures
// oxlint-disable import/first -- vi.mock factory must precede the mocked import in source order
import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

// Mock the generated bindings before the store imports them.
vi.mock("./bindings.gen", () => {
  const ok = <T>(data: T) => ({ status: "ok" as const, data });
  const err = (code: string, message = code) => ({
    status: "error" as const,
    error: { code, message },
  });
  const listeners = new Map<string, (e: { payload: unknown }) => void>();
  return {
    __emit: (ev: string, payload: unknown) => listeners.get(ev)?.({ payload }),
    commands: {
      appState: vi.fn(async () => ({
        vault: { path: "/v", isGit: false, warnings: [] },
        unlocked: false,
        settings: {
          autoLockMinutes: 15,
          maxSessionHours: 8,
          clipboardSeconds: 45,
          lockOnSleep: true,
          language: "system",
        },
      })),
      unlock: vi.fn(async (p: string) =>
        p === "right" ? ok({ warnings: [] }) : err("wrong_passphrase"),
      ),
      lock: vi.fn(async () => ok(null)),
      heartbeat: vi.fn(async () => undefined),
      listEntries: vi.fn(async () => ok(["a/b", "a/c", "b"])),
      entryDetail: vi.fn(async (name: string) =>
        ok({
          name,
          kind: "text",
          value: { key: "value", length: 6 },
          fields: [{ key: "user", length: 4 }],
          notesLength: 0,
          hasTotp: false,
          binaryLength: null,
        }),
      ),
      revealField: vi.fn(async () => ok("SECRET")),
      copyField: vi.fn(async () => ok({ clearsInSecs: 45 })),
      saveEntry: vi.fn(async () => ok([])),
      renameEntry: vi.fn(async () => ok([])),
      deleteEntry: vi.fn(async () => ok([])),
      getSettings: vi.fn(async () => ({
        autoLockMinutes: 15,
        maxSessionHours: 8,
        clipboardSeconds: 45,
        lockOnSleep: true,
        language: "system",
      })),
      setSettings: vi.fn(async () => ok(null)),
      passphraseStrength: vi.fn(async () => ({
        score: 4,
        ok: true,
        problem: null,
        suggestions: [],
      })),
      generatePassword: vi.fn(async () => ok("GENERATED")),
      openVault: vi.fn(async () => ok({ path: "/v", isGit: false, warnings: [] })),
      createVault: vi.fn(async () =>
        ok({ path: "/v", isGit: true, warnings: ["initial commit skipped"] }),
      ),
      cloneVault: vi.fn(async () => ok({ path: "/v", isGit: true, warnings: [] })),
      totpCode: vi.fn(async () => ok({ code: "123456", validFor: 30 })),
      editEntry: vi.fn(async () => ok("v\nuser: u")),
      importBinary: vi.fn(async () => ok([])),
      exportBinary: vi.fn(async () => ok(null)),
      recentVaults: vi.fn(async () => []),
      vaultStatus: vi.fn(async () => ok({ path: "/v", exists: true, isGit: false, agent: null })),
    },
    events: {
      locked: {
        listen: vi.fn(async (cb: (e: { payload: unknown }) => void) => {
          listeners.set("locked", cb);
          return () => {};
        }),
      },
      entriesChanged: {
        listen: vi.fn(async (cb: () => void) => {
          listeners.set("entries-changed", cb);
          return () => {};
        }),
      },
    },
  };
});

import { commands } from "./bindings.gen";
import { fuzzyFilter } from "./lib/fuzzy";
import * as store from "./store";

const bg = await import("./bindings.gen");
const emit = (ev: string, payload: unknown): void => {
  (bg as unknown as { __emit: (e: string, p: unknown) => void }).__emit(ev, payload);
};
const emitLocked = async (reason: string): Promise<void> => {
  await store.wireEvents();
  emit("locked", { reason });
};

beforeEach(() => {
  vi.clearAllMocks();
  store.reset();
});

describe("DESK-CORE store", () => {
  it("desk_core_unlock_success_moves_to_main_and_lists", async () => {
    await store.init();
    expect(store.getSnap().phase).toBe("unlock");
    await expect(store.doUnlock("right")).resolves.toBe(true);
    expect(store.getSnap().phase).toBe("main");
    expect(store.getSnap().entries).toEqual(["a/b", "a/c", "b"]);
  });

  it("desk_core_unlock_wrong_keeps_unlock_and_sets_error", async () => {
    await store.init();
    await expect(store.doUnlock("nope")).resolves.toBe(false);
    expect(store.getSnap().phase).toBe("unlock");
    expect(store.getSnap().error?.code).toBe("wrong_passphrase");
  });

  it("desk_core_locked_event_clears_everything", async () => {
    await store.init();
    await store.doUnlock("right");
    expect(store.getSnap().entries).toHaveLength(3);
    await emitLocked("idle");
    const s = store.getSnap();
    expect(s.phase).toBe("unlock");
    expect(s.entries).toEqual([]);
    expect(s.selected).toBeUndefined();
    expect(s.detail).toBeUndefined();
    expect(s.lockReason).toBe("idle");
  });

  it("desk_core_entries_changed_refreshes_list", async () => {
    await store.init();
    await store.doUnlock("right");
    vi.mocked(commands.listEntries).mockResolvedValueOnce({
      status: "ok",
      data: ["x"],
    });
    await store.wireEvents();
    emit("entries-changed", null);
    await Promise.resolve();
    await Promise.resolve();
    expect(store.getSnap().entries).toEqual(["x"]);
  });

  it("desk_core_search_orders_by_subsequence_score", () => {
    const names = ["alpha/beta", "a/b", "beta/alpha", "gamma"];
    expect(fuzzyFilter("ab", names)[0]).toBe("a/b");
    expect(fuzzyFilter("alp", names)).toEqual(["alpha/beta", "beta/alpha"]);
    expect(fuzzyFilter("", names)).toEqual(names);
    expect(fuzzyFilter("zzz", names)).toEqual([]);
  });

  it("desk_core_select_loads_detail_without_values", async () => {
    await store.init();
    await store.doUnlock("right");
    await store.select("a/b");
    const d = store.getSnap().detail;
    expect(d?.fields[0]?.length).toBe(4);
    expect(JSON.stringify(d)).not.toContain("SECRET");
  });

  it("desk_core_settings_roundtrip", async () => {
    const r = await commands.setSettings({
      autoLockMinutes: 5,
      clipboardSeconds: 10,
    });
    expect(r.status).toBe("ok");
  });

  it("desk_core_i18n_key_sets_match", async () => {
    const { en } = await import("./i18n/en");
    const { zh } = await import("./i18n/zh");
    expect(Object.keys(zh).toSorted()).toEqual(Object.keys(en).toSorted());
    for (const k of Object.keys(en) as Array<keyof typeof en>) {
      expect(zh[k]).toBeTypeOf(typeof en[k]);
    }
    expect(Object.keys(zh.lockedReason).toSorted()).toEqual(
      Object.keys(en.lockedReason).toSorted(),
    );
    expect(Object.keys(zh.error).toSorted()).toEqual(Object.keys(en.error).toSorted());
  });
});
