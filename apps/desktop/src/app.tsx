import { KeyRound, RefreshCw } from "lucide-react";
import type React from "react";
import { useEffect } from "react";

import { commands } from "./bindings.gen";
import { t } from "./i18n";
import { refresh, useVaultState } from "./store";

export function App(): React.JSX.Element {
  const messages = t();
  const state = useVaultState();

  useEffect(() => {
    void refresh(commands);
  }, []);

  return (
    <main className="flex min-h-screen flex-col items-center justify-center gap-6 bg-white p-8 text-zinc-900 dark:bg-zinc-950 dark:text-zinc-100">
      <KeyRound size={40} className="text-zinc-500" aria-hidden />
      <h1 className="text-2xl font-semibold tracking-tight">{messages.appTitle}</h1>
      {state.kind === "loading" && <p className="text-sm text-zinc-500">{messages.loading}</p>}
      {state.kind === "error" && <p className="text-sm text-red-600">{state.message}</p>}
      {state.kind === "status" && (
        <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
          <dt className="text-zinc-500">{messages.vaultPath}</dt>
          <dd className="font-mono break-all">{state.status.path || "—"}</dd>
          <dt className="text-zinc-500">{messages.git}</dt>
          <dd>
            {state.status.exists
              ? state.status.isGit
                ? messages.git
                : messages.notGit
              : messages.noVault}
          </dd>
          <dt className="text-zinc-500">{messages.agent}</dt>
          <dd>
            {state.status.agent === null || state.status.agent === undefined
              ? messages.agentNone
              : messages.agentHost(state.status.agent.host)}
          </dd>
        </dl>
      )}
      <button
        type="button"
        onClick={() => void refresh(commands)}
        className="inline-flex items-center gap-2 rounded-md border border-zinc-300 px-3 py-1.5 text-sm hover:bg-zinc-100 dark:border-zinc-700 dark:hover:bg-zinc-800"
      >
        <RefreshCw size={14} aria-hidden />
        {messages.refresh}
      </button>
    </main>
  );
}
