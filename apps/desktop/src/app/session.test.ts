import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

import { queryClient } from "./query";
import {
  boot,
  createVault,
  lock,
  openVault,
  resetSession,
  saveSettings,
  unlock,
  useSession,
} from "./session";

const ipc = await vi.hoisted(async () => {
  const m = await import("../test/bindings-mock");
  return m.createBindingsMock();
});
vi.mock("../ipc/bindings.gen", () => ({ commands: ipc.commands, events: ipc.events }));

function usePhaseValue(): "loading" | "welcome" | "locked" | "unlocked" {
  return useSession((s) => s.phase);
}

function usePhase() {
  return renderHook(usePhaseValue).result;
}

describe("session store", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetSession();
    queryClient.clear();
    ipc.store.vault = { path: "/vaults/ks", isGit: true, warnings: [] };
  });

  it("boots to welcome without a vault", async () => {
    ipc.commands.appState.mockResolvedValue({
      vault: null,
      unlocked: false,
      lockReason: null,
      settings: ipc.store.settings,
    });
    const hook = usePhase();
    await act(async () => {
      await boot();
    });
    expect(hook.current).toBe("welcome");
  });

  it("boots to locked with a closed vault, unlocked with a live session", async () => {
    ipc.commands.appState.mockResolvedValue({
      vault: ipc.store.vault,
      unlocked: false,
      lockReason: null,
      settings: ipc.store.settings,
    });
    const hook = usePhase();
    await act(async () => {
      await boot();
    });
    expect(hook.current).toBe("locked");
    ipc.commands.appState.mockResolvedValue({
      vault: ipc.store.vault,
      unlocked: true,
      lockReason: null,
      settings: ipc.store.settings,
    });
    await act(async () => {
      await boot();
    });
    expect(hook.current).toBe("unlocked");
  });

  it("boot carries the lock reason from app_state", async () => {
    ipc.commands.appState.mockResolvedValue({
      vault: ipc.store.vault,
      unlocked: false,
      lockReason: "idle",
      settings: ipc.store.settings,
    });
    const hook = renderHook(() => useSession((s) => s.lockReason));
    await act(async () => {
      await boot();
    });
    expect(hook.result.current).toBe("idle");
  });

  it("unlocks with the right passphrase and rejects a wrong one", async () => {
    ipc.commands.appState.mockResolvedValue({
      vault: ipc.store.vault,
      unlocked: false,
      lockReason: null,
      settings: ipc.store.settings,
    });
    const hook = usePhase();
    await act(async () => {
      await boot();
    });
    await expect(
      act(async () => {
        await unlock("nope");
      }),
    ).rejects.toMatchObject({ code: "wrong_passphrase" });
    expect(hook.current).toBe("locked");
    await act(async () => {
      await unlock("correct horse battery staple");
    });
    expect(hook.current).toBe("unlocked");
  });

  it("a locked event locks the UI and clears the query cache", async () => {
    ipc.commands.appState.mockResolvedValue({
      vault: ipc.store.vault,
      unlocked: true,
      lockReason: null,
      settings: ipc.store.settings,
    });
    const hook = usePhase();
    await act(async () => {
      await boot();
    });
    queryClient.setQueryData(["entries"], ["a"]);
    act(() => {
      ipc.emitLocked("idle");
    });
    expect(hook.current).toBe("locked");
    expect(queryClient.getQueryData(["entries"])).toBeUndefined();
  });

  it("openVault lands on locked, createVault on unlocked", async () => {
    ipc.commands.appState.mockResolvedValue({
      vault: null,
      unlocked: false,
      lockReason: null,
      settings: ipc.store.settings,
    });
    const hook = usePhase();
    await act(async () => {
      await boot();
    });
    await act(async () => {
      await openVault("/other");
    });
    expect(hook.current).toBe("locked");
    await act(async () => {
      await createVault("/new", "pass", true);
    });
    expect(hook.current).toBe("unlocked");
  });

  it("lock locks through the manual reason", async () => {
    ipc.commands.appState.mockResolvedValue({
      vault: ipc.store.vault,
      unlocked: true,
      lockReason: null,
      settings: ipc.store.settings,
    });
    const hook = usePhase();
    await act(async () => {
      await boot();
    });
    await act(async () => {
      await lock();
    });
    expect(hook.current).toBe("locked");
  });

  it("saveSettings sends the full merged object", async () => {
    ipc.commands.appState.mockResolvedValue({
      vault: ipc.store.vault,
      unlocked: true,
      lockReason: null,
      settings: ipc.store.settings,
    });
    await act(async () => {
      await boot();
    });
    await act(async () => {
      await saveSettings({ autoLockMinutes: 5 });
    });
    expect(ipc.commands.setSettings).toHaveBeenCalledWith({
      ...ipc.store.settings,
      autoLockMinutes: 5,
    });
  });
});
