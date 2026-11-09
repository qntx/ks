import { QueryClient, QueryClientProvider, useQuery } from "@tanstack/react-query";
import { Tooltip } from "radix-ui";
import type { ReactElement } from "react";

import { I18nProvider } from "../../i18n";
import { commands } from "../../ipc/bindings.gen";
import { useAppState } from "./hooks";
import { Search } from "./search";
import { Unlock } from "./unlock";

const queryClient = new QueryClient();

/**
 * Quick-access root: own QueryClient and no main-session boot — the panel is a thin window over the
 * same vault.
 */
export function QuickApp(): ReactElement {
  return (
    <QueryClientProvider client={queryClient}>
      <QuickInner />
    </QueryClientProvider>
  );
}

function QuickInner(): ReactElement {
  const settings = useQuery({
    queryKey: ["quick", "settings"],
    queryFn: async () => commands.getSettings(),
  });
  return (
    <I18nProvider language={settings.data?.language ?? "system"}>
      <Tooltip.Provider delayDuration={400}>
        <QuickBody />
      </Tooltip.Provider>
    </I18nProvider>
  );
}

function QuickBody(): ReactElement {
  const app = useAppState();
  if (app.data === undefined) {
    return <div className="bg-card h-screen" />;
  }
  return app.data.unlocked ? <Search /> : <Unlock />;
}
