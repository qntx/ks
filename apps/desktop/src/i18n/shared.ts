import { createContext } from "react";

import { en } from "./en";
import { zh } from "./zh";

export type Messages = typeof en;
export type Locale = "en" | "zh";

export const bundles: Record<Locale, Messages> = { en, zh };

/** Resolve the settings language: "system" follows `navigator.language`. */
export function resolveLocale(language: string): Locale {
  if (language === "en" || language === "zh") {
    return language;
  }
  return navigator.language.toLowerCase().startsWith("zh") ? "zh" : "en";
}

export const MessagesContext = createContext<Messages>(en);
