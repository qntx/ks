import { en } from "./i18n/en";
import { zh } from "./i18n/zh";

export type Locale = "en" | "zh";

const bundles = { en, zh } as const;

// Placeholder until D5 wires locale detection and the settings switch.
let locale: Locale = "en";

export function setLocale(next: Locale): void {
  locale = next;
}

export function t(): (typeof bundles)[Locale] {
  return bundles[locale];
}
