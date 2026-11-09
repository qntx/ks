import { KeyRound, Lock, Plus, Settings as SettingsIcon } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type React from "react";

import type { Messages } from "../i18n";
import { fuzzyFilter } from "../lib/fuzzy";
import { throttle } from "../lib/throttle";
import { doLock, heartbeat, select, useStore } from "../store";
import { Button, Input } from "../ui";
import { Editor } from "./editor";
import { EntryView } from "./entry";
import { Settings } from "./settings";

const BEAT = throttle(() => heartbeat(), 30_000);

export function Main(props: { m: Messages }): React.JSX.Element {
  const { m } = props;
  const { entries, selected } = useStore();
  const [query, setQuery] = useState("");
  const [editor, setEditor] = useState<{ name?: string | undefined } | undefined>();
  const [settings, setSettings] = useState(false);
  const search = useRef<HTMLInputElement>(null);
  const filtered = fuzzyFilter(query, entries);

  useEffect(() => {
    const onKey = (e: KeyboardEvent): void => {
      if (e.metaKey || e.ctrlKey) {
        if (e.key === "f") {
          e.preventDefault();
          search.current?.focus();
        } else if (e.key === "n") {
          e.preventDefault();
          setEditor({});
        } else if (e.key === "l") {
          e.preventDefault();
          void doLock();
        }
        return;
      }
      BEAT();
      const i = filtered.indexOf(selected ?? "");
      const next =
        e.key === "ArrowDown"
          ? filtered[i + 1]
          : e.key === "ArrowUp"
            ? filtered[i - 1]
            : e.key === "Enter" && i === -1
              ? filtered[0]
              : undefined;
      if (typeof next === "string") {
        void select(next);
      }
    };
    globalThis.addEventListener("keydown", onKey);
    globalThis.addEventListener("pointerdown", BEAT);
    return () => {
      globalThis.removeEventListener("keydown", onKey);
      globalThis.removeEventListener("pointerdown", BEAT);
    };
  }, [filtered, selected]);

  return (
    <div className="flex h-screen bg-white text-zinc-900 dark:bg-zinc-950 dark:text-zinc-100">
      <aside className="flex w-64 shrink-0 flex-col border-r border-zinc-200 dark:border-zinc-800">
        <div className="flex items-center gap-2 border-b border-zinc-200 p-2 dark:border-zinc-800">
          <div className="flex-1">
            <Input
              ref={search}
              type="search"
              value={query}
              onChange={setQuery}
              placeholder={m.search}
            />
          </div>
        </div>
        <nav aria-label="entries" className="flex-1 overflow-y-auto p-1">
          {filtered.length === 0 && <p className="p-3 text-sm text-zinc-500">{m.noEntries}</p>}
          <ul>
            {filtered.map((name) => (
              <li key={name}>
                <button
                  type="button"
                  onClick={() => void select(name)}
                  aria-current={name === selected}
                  className={`flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm outline-none focus-visible:ring-2 focus-visible:ring-blue-500 ${
                    name === selected
                      ? "bg-blue-100 dark:bg-blue-950"
                      : "hover:bg-zinc-100 dark:hover:bg-zinc-800"
                  }`}
                >
                  <KeyRound size={14} className="shrink-0 text-zinc-400" aria-hidden />
                  <span className="truncate">{name}</span>
                </button>
              </li>
            ))}
          </ul>
        </nav>
        <div className="flex items-center gap-1 border-t border-zinc-200 p-2 dark:border-zinc-800">
          <Button onClick={() => setEditor({})} title="⌘N">
            <Plus size={14} aria-hidden />
            {m.newEntry}
          </Button>
          <div className="flex-1" />
          <Button kind="ghost" onClick={() => setSettings(true)} title={m.settingsTitle}>
            <SettingsIcon size={14} aria-hidden />
          </Button>
          <Button kind="ghost" onClick={() => void doLock()} title="⌘L">
            <Lock size={14} aria-hidden />
            {m.lock}
          </Button>
        </div>
      </aside>
      <section className="flex-1 overflow-y-auto p-6" aria-label="entry">
        {editor === null && (
          <EntryView
            m={m}
            onEdit={(name) => setEditor({ name })}
            onSettings={() => setSettings(true)}
          />
        )}
        {editor !== undefined && (
          <Editor m={m} name={editor.name} onClose={() => setEditor(undefined)} />
        )}
      </section>
      <Settings open={settings} onOpenChange={setSettings} m={m} />
    </div>
  );
}
