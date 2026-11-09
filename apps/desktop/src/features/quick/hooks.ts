import { useQuery, useQueryClient } from "@tanstack/react-query";
import type { UseQueryResult } from "@tanstack/react-query";
import { useEffect } from "react";

import { commands, events } from "../../ipc/bindings.gen";
import type { AppView, Devices } from "../../ipc/bindings.gen";
import { call } from "../../ipc/call";

export const quickKeys = {
  appState: ["quick", "appState"] as const,
  entries: ["quick", "entries"] as const,
  devices: ["quick", "devices"] as const,
};

/** Vault session state for the quick window — refetched on `locked`. */
export function useAppState(): UseQueryResult<AppView> {
  const qc = useQueryClient();
  useEffect(() => {
    const un = events.locked.listen(() => {
      void qc.invalidateQueries({ queryKey: quickKeys.entries });
      void qc.invalidateQueries({ queryKey: quickKeys.appState });
    });
    return () => {
      void (async () => {
        (await un)();
      })();
    };
  }, [qc]);
  return useQuery({
    queryKey: quickKeys.appState,
    queryFn: async () => commands.appState(),
  });
}

/** Entry names for the search list. */
export function useEntries(unlocked: boolean): UseQueryResult<string[]> {
  return useQuery({
    queryKey: quickKeys.entries,
    queryFn: async () => call(commands.listEntries()),
    enabled: unlocked,
  });
}

/** Device sources for the Touch ID button on the locked view. */
export function useDevices(): UseQueryResult<Devices> {
  return useQuery({
    queryKey: quickKeys.devices,
    queryFn: async () => call(commands.devices()),
  });
}
