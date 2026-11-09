import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { UseMutationResult, UseQueryResult } from "@tanstack/react-query";

import { queryKeys } from "../../app/query";
import { errorMessage, useMessages } from "../../i18n";
import { commands } from "../../ipc/bindings.gen";
import type { DoctorReport } from "../../ipc/bindings.gen";
import { call } from "../../ipc/call";
import { toast } from "../../ui";

/** Vault check result; mounted only while the doctor page is shown. */
export function useDoctor(): UseQueryResult<DoctorReport> {
  return useQuery({
    queryKey: queryKeys.doctor,
    queryFn: async () => call(commands.doctor()),
    gcTime: 0,
    staleTime: 0,
  });
}

/** Applies the safe repairs; the remaining report replaces the cache. */
export function useDoctorFix(): UseMutationResult<DoctorReport, Error, void> {
  const m = useMessages();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async () => call(commands.doctorFix()),
    onSuccess: async (report) => {
      qc.setQueryData(queryKeys.doctor, report);
      // A fix can rewrite `.gitignore`, which shows in the sync pill.
      await qc.invalidateQueries({ queryKey: queryKeys.gitStatus });
    },
    onError: (error) => {
      toast(errorMessage(m, error), { tone: "error" });
    },
  });
}

/** Whether an interrupted key rotation is pending (key_count > 1). */
export function useRotationPending(): UseQueryResult<boolean> {
  return useQuery({
    queryKey: queryKeys.rotationPending,
    queryFn: async () => call(commands.rotationPending()),
    gcTime: 0,
    staleTime: 0,
  });
}
