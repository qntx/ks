import { useEffect } from "react";

import { queryClient, queryKeys } from "../../app/query";
import { commands, events } from "../../ipc/bindings.gen";
import { throttle } from "../../lib/throttle";

/**
 * Wires vault-level effects: `entries_changed` invalidates the entry caches, and pointer/key
 * activity throttles a 30s heartbeat so the idle auto-lock only fires when the app is unused.
 */
export function useVaultEvents(): void {
  useEffect(() => {
    let dead = false;
    let unlisten: (() => void) | undefined;
    const listen = async (): Promise<void> => {
      const u = await events.entriesChanged.listen(() => {
        void queryClient.invalidateQueries({ queryKey: queryKeys.entries });
        void queryClient.invalidateQueries({ queryKey: queryKeys.entryAll });
        void queryClient.invalidateQueries({ queryKey: queryKeys.gitStatus });
      });
      if (dead) {
        u();
      } else {
        unlisten = u;
      }
    };
    void listen();
    return () => {
      dead = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    const beat = throttle(() => {
      void commands.heartbeat();
    }, 30_000);
    globalThis.addEventListener("pointerdown", beat);
    globalThis.addEventListener("keydown", beat);
    return () => {
      globalThis.removeEventListener("pointerdown", beat);
      globalThis.removeEventListener("keydown", beat);
    };
  }, []);
}
