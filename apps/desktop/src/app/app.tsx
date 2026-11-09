import { QueryClientProvider } from "@tanstack/react-query";
import { Tooltip } from "radix-ui";
import { useEffect } from "react";
import type { ReactElement } from "react";

import { Unlock, Welcome } from "../features/access";
import { VaultScreen } from "../features/vault";
import { I18nProvider } from "../i18n";
import { BrandMark, Toaster } from "../ui";
import { queryClient } from "./query";
import { boot, useSession } from "./session";
import { useTrayLabels } from "./tray";

export function App(): ReactElement {
  const phase = useSession((s) => s.phase);
  const language = useSession((s) => s.settings.language);

  useEffect(() => {
    void boot();
  }, []);
  useTrayLabels();

  return (
    <QueryClientProvider client={queryClient}>
      <I18nProvider language={language}>
        <Tooltip.Provider delayDuration={400}>
          {phase === "loading" && (
            <div className="bg-window flex h-dvh items-center justify-center">
              <div className="animate-fade-in">
                <BrandMark />
              </div>
            </div>
          )}
          {phase === "welcome" && <Welcome />}
          {phase === "locked" && <Unlock />}
          {phase === "unlocked" && <VaultScreen />}
          <Toaster />
        </Tooltip.Provider>
      </I18nProvider>
    </QueryClientProvider>
  );
}
