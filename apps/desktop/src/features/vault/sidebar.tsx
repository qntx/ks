import { LayoutGrid, Lock, Search, Settings as SettingsIcon, X } from "lucide-react";
import type { ReactElement, Ref } from "react";

import { lock } from "../../app/session";
import { useMessages } from "../../i18n";
import { leafName } from "../../lib/paths";
import { Button, cn, IconButton } from "../../ui";
import { SyncPill } from "../sync";
import { FolderRows } from "./folder-tree";
import type { FolderNode } from "./tree";

/** A `.ks` vault shows as "ks": folder names keep their dot, the display leaf strips it. */
function vaultTitle(path: string): string {
  const leaf = leafName(path);
  return leaf.startsWith(".") ? leaf.slice(1) : leaf;
}

export function Sidebar({
  vaultPath,
  isGit,
  total,
  query,
  searchRef,
  validFolder,
  searching,
  expanded,
  tree,
  onQuery,
  onOpenFolder,
  onToggle,
  onSettings,
}: {
  vaultPath: string;
  isGit: boolean;
  total: number;
  query: string;
  searchRef: Ref<HTMLInputElement>;
  validFolder: string;
  searching: boolean;
  expanded: ReadonlySet<string>;
  tree: FolderNode;
  onQuery: (q: string) => void;
  onOpenFolder: (path: string) => void;
  onToggle: (path: string) => void;
  onSettings: () => void;
}): ReactElement {
  const m = useMessages();
  return (
    <aside className="flex w-60 shrink-0 flex-col">
      <div data-tauri-drag-region className="h-13 shrink-0" />
      <div className="px-4 pb-3">
        <p className="text-ink truncate text-lg font-bold">{vaultTitle(vaultPath)}</p>
        <p className="text-muted truncate text-xs">
          {vaultPath} · {isGit ? m.vault.git : m.vault.localOnly}
        </p>
      </div>
      <div className="px-3 pb-2">
        <div className="bg-card flex h-10 items-center gap-2 rounded-full px-3.5">
          <Search className="text-muted size-4 shrink-0" strokeWidth={2} aria-hidden="true" />
          <input
            ref={searchRef}
            autoComplete="off"
            spellCheck={false}
            autoCorrect="off"
            autoCapitalize="none"
            value={query}
            onChange={(e) => {
              onQuery(e.target.value);
            }}
            onKeyDown={(e) => {
              if (e.key === "Escape") {
                onQuery("");
              }
            }}
            placeholder={m.vault.search}
            aria-label={m.vault.search}
            className="text-ink placeholder:text-muted min-w-0 flex-1 bg-transparent text-base outline-none"
          />
          {query === "" ? (
            <kbd className="text-muted text-xs">⌘F</kbd>
          ) : (
            <button
              type="button"
              aria-label={m.common.close}
              onClick={() => {
                onQuery("");
              }}
            >
              <X className="text-muted size-4" strokeWidth={2} aria-hidden="true" />
            </button>
          )}
        </div>
      </div>
      <button
        type="button"
        className={cn(
          "hover:bg-hover mx-2 flex h-9 cursor-default items-center gap-2.5 rounded-full px-3 text-base",
          validFolder === "" && !searching && "bg-card shadow-selected",
        )}
        onClick={() => {
          onOpenFolder("");
        }}
      >
        <LayoutGrid className="text-muted size-4.5" strokeWidth={2} aria-hidden="true" />
        <span className="text-ink flex-1 text-left">{m.vault.allItems}</span>
        <span className="text-muted text-xs">{total}</span>
      </button>
      <p className="text-muted px-4 pt-3 pb-1 text-xs">{m.vault.folders}</p>
      <nav className="min-h-0 flex-1 overflow-y-auto px-2 pb-2" aria-label={m.vault.folders}>
        <FolderRows
          node={tree}
          current={validFolder}
          searching={searching}
          expanded={expanded}
          onOpen={onOpenFolder}
          onToggle={onToggle}
          expandLabel={(n, isOpen) =>
            isOpen ? m.vault.collapseFolder(n) : m.vault.expandFolder(n)
          }
        />
      </nav>
      <SyncPill />
      <div className="flex items-center gap-2 px-3 pt-2 pb-3">
        <IconButton
          icon={SettingsIcon}
          label={m.vault.settings}
          variant="card"
          onClick={onSettings}
        />
        <Button
          variant="secondary"
          icon={Lock}
          block
          onClick={() => {
            void lock();
          }}
        >
          {m.vault.lock}
        </Button>
      </div>
    </aside>
  );
}
