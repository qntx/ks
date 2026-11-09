import { useEffect } from "react";
import type { ReactElement, ReactNode } from "react";

import { MessagesContext, bundles, resolveLocale } from "./shared";

export function I18nProvider({
  language,
  children,
}: {
  language: string;
  children: ReactNode;
}): ReactElement {
  const locale = resolveLocale(language);
  useEffect(() => {
    document.documentElement.lang = locale === "zh" ? "zh-CN" : "en";
  }, [locale]);
  return <MessagesContext value={bundles[locale]}>{children}</MessagesContext>;
}
