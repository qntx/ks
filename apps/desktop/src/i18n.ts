import type { Settings_Serialize } from "./bindings.gen";
import { en } from "./i18n/en";
import { zh } from "./i18n/zh";

export type Locale = "en" | "zh";

const bundles = { en, zh } as const;
export type Messages = typeof en;

let locale: Locale = "en";

export function setLocale(next: Locale): void {
  locale = next;
}

/** Resolve the settings language ("system" picks navigator.language). */
export function resolveLocale(language: Settings_Serialize["language"]): Locale {
  if (language === "en" || language === "zh") {
    return language;
  }
  return navigator.language.toLowerCase().startsWith("zh") ? "zh" : "en";
}

export function t(language?: Settings_Serialize["language"]): Messages {
  return bundles[language === undefined ? locale : resolveLocale(language)];
}
