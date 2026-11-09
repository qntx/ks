import { useEffect } from "react";
import type React from "react";

import { resolveLocale, setLocale, t } from "./i18n";
import { dismissToast, init, useStore, wireEvents } from "./store";
import { Toasts } from "./ui";
import { Main } from "./views/main";
import { Unlock } from "./views/unlock";
import { Welcome } from "./views/welcome";

export function App(): React.JSX.Element {
  const snap = useStore();
  useEffect(() => {
    void init();
    void wireEvents();
  }, []);
  useEffect(() => {
    setLocale(resolveLocale(snap.settings.language));
  }, [snap.settings.language]);

  const m = t();
  return (
    <>
      {snap.phase === "loading" && (
        <main className="flex min-h-screen items-center justify-center bg-white text-zinc-500 dark:bg-zinc-950">
          {m.loading}
        </main>
      )}
      {snap.phase === "welcome" && <Welcome m={m} />}
      {snap.phase === "unlock" && <Unlock m={m} />}
      {snap.phase === "main" && <Main m={m} />}
      <Toasts items={snap.toasts} onDismiss={dismissToast} />
    </>
  );
}
