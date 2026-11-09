import { useEffect } from "react";

import { bundles, resolveLocale } from "../i18n/shared";
import { commands } from "../ipc/bindings.gen";
import { call } from "../ipc/call";
import { useSession } from "./session";

/**
 * Pushes localized tray-menu labels whenever the resolved locale changes, including first mount.
 * The tray is optional (missing in tests and dev), so a rejected call is swallowed.
 */
export function useTrayLabels(): void {
  const language = useSession((s) => s.settings.language);
  useEffect(() => {
    const labels = bundles[resolveLocale(language)].tray;
    const send = async (): Promise<void> => {
      try {
        await call(commands.setTrayLabels(labels));
      } catch {
        // Optional: the tray may not exist (tests, dev browser).
      }
    };
    void send();
  }, [language]);
}
