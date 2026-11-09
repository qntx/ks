import { use } from "react";

import { IpcError } from "../ipc/call";
import { MessagesContext } from "./shared";
import type { Messages } from "./shared";

export { I18nProvider } from "./provider";
export { resolveLocale } from "./shared";
export type { Locale, Messages } from "./shared";

export function useMessages(): Messages {
  return use(MessagesContext);
}

/** Maps an error to UI copy: `IpcError.code` picks from `m.errors`. */
export function errorMessage(m: Messages, e: unknown): string {
  if (e instanceof IpcError) {
    const known = m.errors[e.code];
    if (known !== undefined) {
      return known;
    }
    return e.message === "" ? m.errors.other : `${m.errors.other} — ${e.message}`;
  }
  return e instanceof Error ? e.message : m.errors.other;
}
