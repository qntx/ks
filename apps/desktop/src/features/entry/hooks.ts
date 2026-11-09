import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { UseMutationResult, UseQueryResult } from "@tanstack/react-query";

import { queryKeys } from "../../app/query";
import { errorMessage, useMessages } from "../../i18n";
import { commands } from "../../ipc/bindings.gen";
import type { EntryDetail_Serialize, FieldRef, TotpCode } from "../../ipc/bindings.gen";
import { call } from "../../ipc/call";
import { toast } from "../../ui";

export function useEntryDetail(name: string): UseQueryResult<EntryDetail_Serialize> {
  return useQuery({
    queryKey: queryKeys.entry(name),
    queryFn: async () => call(commands.entryDetail(name)),
  });
}

export function useTotp(name: string): UseQueryResult<TotpCode> {
  return useQuery({
    queryKey: queryKeys.totp(name),
    queryFn: async () => call(commands.totpCode(name)),
    refetchInterval: 1000,
    gcTime: 0,
  });
}

/**
 * Shared mutation wiring: invalidate the entry caches on success and toast each returned warning;
 * callers own error handling.
 */
function useVaultMutation<TVars>(
  mutationFn: (v: TVars) => Promise<string[]>,
): UseMutationResult<string[], Error, TVars> {
  const qc = useQueryClient();
  return useMutation({
    mutationFn,
    onSuccess: async (warnings) => {
      await qc.invalidateQueries({ queryKey: queryKeys.entries });
      await qc.invalidateQueries({ queryKey: queryKeys.entryAll });
      await qc.invalidateQueries({ queryKey: queryKeys.gitStatus });
      for (const w of warnings) {
        toast(w);
      }
    },
  });
}

export function useSaveEntry(): UseMutationResult<
  string[],
  Error,
  { name: string; text: string; overwrite: boolean }
> {
  return useVaultMutation(async (v) => call(commands.saveEntry(v.name, v.text, v.overwrite)));
}

export function useRenameEntry(): UseMutationResult<
  string[],
  Error,
  { from: string; to: string; overwrite: boolean }
> {
  return useVaultMutation(async (v) => call(commands.renameEntry(v.from, v.to, v.overwrite)));
}

export function useDeleteEntry(): UseMutationResult<string[], Error, string> {
  return useVaultMutation(async (name) => call(commands.deleteEntry(name)));
}

export function useImportFile(): UseMutationResult<
  string[],
  Error,
  { name: string; path: string; overwrite: boolean }
> {
  return useVaultMutation(async (v) => call(commands.importBinary(v.name, v.path, v.overwrite)));
}

/** Copies a field server-side; the value never touches the webview. */
export function useCopyField(): (name: string, field: FieldRef) => Promise<boolean> {
  const m = useMessages();
  return async (name: string, field: FieldRef) => {
    try {
      const r = await call(commands.copyField(name, field));
      toast(m.toasts.copied(r.clearsInSecs));
      return true;
    } catch (error) {
      toast(errorMessage(m, error), { tone: "error" });
      return false;
    }
  };
}
