import { useQuery } from "@tanstack/react-query";
import type { UseQueryResult } from "@tanstack/react-query";

import { queryClient, queryKeys } from "../../app/query";
import { errorMessage } from "../../i18n";
import type { Messages } from "../../i18n";
import { commands } from "../../ipc/bindings.gen";
import type { Keep } from "../../ipc/bindings.gen";
import { call } from "../../ipc/call";
import { toast } from "../../ui";
import { closeConflict, openConflict } from "./store";

const fetchGitStatus = async () => call(commands.gitStatus());

/** Git status of the open vault; `null` when it is not a repo. */
export function useGitStatus(): UseQueryResult<Awaited<ReturnType<typeof fetchGitStatus>>> {
  return useQuery({
    queryKey: queryKeys.gitStatus,
    queryFn: fetchGitStatus,
  });
}

/** Runs a sync; a conflict outcome opens the conflict dialog. */
export async function syncNow(m: Messages, keep?: Keep): Promise<void> {
  try {
    const out = await call(commands.syncNow(keep ?? null));
    void queryClient.invalidateQueries({ queryKey: queryKeys.gitStatus });
    if (out.status === "conflict") {
      openConflict({ entries: out.entries, kind: out.kind });
    } else {
      closeConflict();
    }
  } catch (error) {
    toast(errorMessage(m, error), { tone: "error" });
  }
}
