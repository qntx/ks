import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { UseMutationResult, UseQueryResult } from "@tanstack/react-query";

import { queryKeys } from "../../app/query";
import { errorMessage, useMessages } from "../../i18n";
import { commands } from "../../ipc/bindings.gen";
import type { DeviceKind, Devices } from "../../ipc/bindings.gen";
import { call } from "../../ipc/call";
import { toast } from "../../ui";

/** Device-source state for the unlock screen and the settings group. */
export function useDevices(): UseQueryResult<Devices> {
  return useQuery({
    queryKey: queryKeys.devices,
    queryFn: async () => call(commands.devices()),
  });
}

/**
 * Enables or disables a device source: invalidates the device cache on success, toasts warnings,
 * and maps errors through errorMessage.
 */
export function useSetDevice(): UseMutationResult<
  string[],
  Error,
  { kind: DeviceKind; enabled: boolean }
> {
  const m = useMessages();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ kind, enabled }) => call(commands.setDevice(kind, enabled)),
    onSuccess: async (warnings) => {
      await qc.invalidateQueries({ queryKey: queryKeys.devices });
      for (const w of warnings) {
        toast(w);
      }
    },
    onError: (error) => {
      toast(errorMessage(m, error), { tone: "error" });
    },
  });
}
