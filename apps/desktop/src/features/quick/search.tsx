import { useQueryClient } from "@tanstack/react-query";
/* oxlint-disable jsx-a11y/prefer-tag-over-role, jsx-a11y/click-events-have-key-events --
   WAI-ARIA listbox: option keys live on the input, matching item-list. */
import { KeyRound } from "lucide-react";
import { useEffect, useState } from "react";
import type { KeyboardEvent as ReactKeyboardEvent, ReactElement } from "react";

import { errorMessage, useMessages } from "../../i18n";
import { commands, events } from "../../ipc/bindings.gen";
import { call, IpcError } from "../../ipc/call";
import { fuzzyFilter } from "../../lib/fuzzy";
import { cn } from "../../ui";
import { quickKeys, useEntries } from "./hooks";

const MAX_ROWS = 8;

function optionId(name: string): string {
  return `ks-q-${name.replaceAll(/[^a-zA-Z0-9_-]/g, "-")}`;
}

function scrollTo(name: string): void {
  document.querySelector(`#${optionId(name)}`)?.scrollIntoView({ block: "nearest" });
}

function focusInput(): void {
  document.querySelector<HTMLInputElement>("#ks-quick-input")?.focus();
}

function parent(name: string): string {
  const i = name.lastIndexOf("/");
  return i === -1 ? "" : name.slice(0, i);
}

/**
 * The unlocked view: a large search input over the entry list. Enter copies the secret, ⌘Enter the
 * code, Esc hides.
 */
export function Search(): ReactElement {
  const m = useMessages();
  const qc = useQueryClient();
  const entries = useEntries(true);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const [noTotp, setNoTotp] = useState(false);
  const [inline, setInline] = useState<string | undefined>(undefined);

  const names = entries.data ?? [];
  const results =
    query === "" ? names.toSorted((a, b) => a.localeCompare(b)) : fuzzyFilter(query, names);
  const shown = results.slice(0, MAX_ROWS);
  const activeName = shown[Math.min(active, Math.max(shown.length - 1, 0))];

  // `quick-shown`: reset the search and the selection, refresh data and
  // refocus — the panel is reused between invocations.
  useEffect(() => {
    focusInput();
  }, []);

  useEffect(() => {
    const un = events.quickShown.listen(() => {
      setQuery("");
      setActive(0);
      setNoTotp(false);
      void qc.invalidateQueries({ queryKey: quickKeys.entries });
      void qc.invalidateQueries({ queryKey: quickKeys.appState });
      focusInput();
    });
    return () => {
      void (async () => {
        (await un)();
      })();
    };
  }, [qc]);

  const copy = async (name: string, field: "value" | "totp"): Promise<string> => {
    try {
      await call(commands.copyField(name, field));
      return "copied";
    } catch (error) {
      if (error instanceof IpcError && error.code === "no_totp") {
        return "no_totp";
      }
      return errorMessage(m, error);
    }
  };

  const copyAndHide = async (name: string, field: "value" | "totp"): Promise<void> => {
    const r = await copy(name, field);
    if (r === "copied") {
      await commands.hideQuick();
    } else if (r === "no_totp") {
      setNoTotp(true);
    } else {
      setInline(r);
    }
  };

  const onKeyDown = (e: ReactKeyboardEvent<HTMLInputElement>): void => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const next = Math.max(
        0,
        Math.min(shown.length - 1, active + (e.key === "ArrowDown" ? 1 : -1)),
      );
      setActive(next);
      const n = shown[next];
      if (n !== undefined) {
        scrollTo(n);
      }
      return;
    }
    if (e.key === "Escape") {
      e.preventDefault();
      void commands.hideQuick();
      return;
    }
    if (e.key === "Enter" && activeName !== undefined) {
      e.preventDefault();
      if (e.metaKey) {
        void copyAndHide(activeName, "totp");
      } else {
        void copyAndHide(activeName, "value");
      }
    }
  };

  return (
    <main className="bg-card flex h-screen flex-col p-4">
      <input
        id="ks-quick-input"
        type="text"
        aria-label={m.quick.searchPlaceholder}
        aria-activedescendant={activeName === undefined ? undefined : optionId(activeName)}
        aria-controls="ks-quick-list"
        aria-expanded="true"
        role="combobox"
        placeholder={m.quick.searchPlaceholder}
        className="bg-card text-ink placeholder:text-muted text-xl outline-none"
        value={query}
        onChange={(e) => {
          setQuery(e.target.value);
          setActive(0);
          setNoTotp(false);
          setInline(undefined);
        }}
        onKeyDown={onKeyDown}
      />
      {noTotp && <p className="text-muted mt-2 text-sm">{m.quick.noTotp}</p>}
      {inline !== undefined && <p className="text-danger mt-2 text-sm">{inline}</p>}
      <hr className="border-line mt-3" />
      <div
        id="ks-quick-list"
        role="listbox"
        aria-label={m.quick.listLabel}
        className="mt-3 flex-1 overflow-y-auto rounded-2xl"
      >
        {shown.length === 0 && (
          <p className="text-muted px-3 py-4 text-sm">
            {names.length === 0 ? m.quick.empty : m.quick.noMatch}
          </p>
        )}
        {shown.map((name, i) => (
          <div
            key={name}
            id={optionId(name)}
            role="option"
            aria-selected={i === active}
            tabIndex={-1}
            className={cn(
              "flex cursor-default items-center gap-2.5 rounded-xl px-3 py-2",
              i === active ? "bg-hover" : "",
            )}
            onClick={() => {
              void copyAndHide(name, "value");
            }}
          >
            <KeyRound className="text-muted size-4 shrink-0" aria-hidden="true" />
            <span className="min-w-0 flex-1 truncate">
              {name.split("/").at(-1)}
              {parent(name) !== "" && (
                <span className="text-muted ml-2 text-sm">{parent(name)}</span>
              )}
            </span>
          </div>
        ))}
      </div>
      <p className="text-muted mt-2 text-center text-xs">{m.quick.hints}</p>
    </main>
  );
}
