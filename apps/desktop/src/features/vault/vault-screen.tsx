import { useQuery } from "@tanstack/react-query";
import { open as pickFile } from "@tauri-apps/plugin-dialog";
import { FolderOpen, Plus, Search } from "lucide-react";
import { useRef, useState } from "react";
import type { ReactElement } from "react";

import { queryKeys } from "../../app/query";
import { lock, useSession } from "../../app/session";
import { useMessages } from "../../i18n";
import { commands } from "../../ipc/bindings.gen";
import { call } from "../../ipc/call";
import { leafName, parentPath } from "../../lib/paths";
import { Button, EmptyState } from "../../ui";
import type { MenuItem } from "../../ui";
import {
  DeleteDialog,
  EntryEditor,
  ImportDialog,
  Inspector,
  RenameDialog,
  useCopyField,
} from "../entry";
import type { EditorTarget } from "../entry";
import { SettingsDialog } from "../settings";
import { ContentHeader } from "./content-header";
import { ItemList } from "./item-list";
import { Sidebar } from "./sidebar";
import { buildTree, folderAt, folderItems, searchItems } from "./tree";
import { useHotkeys } from "./use-hotkeys";
import { useVaultEvents } from "./use-vault-events";

type DialogState =
  | { kind: "none" }
  | { kind: "editor"; target: EditorTarget }
  | { kind: "settings" }
  | { kind: "rename"; name: string }
  | { kind: "delete"; name: string }
  | { kind: "import"; path: string };

const NONE: DialogState = { kind: "none" };

export function VaultScreen(): ReactElement {
  const m = useMessages();
  const vault = useSession((s) => s.vault);
  const [folder, setFolder] = useState("");
  const [selected, setSelected] = useState<string | undefined>(undefined);
  const [query, setQuery] = useState("");
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(new Set());
  const [active, setActive] = useState(0);
  const [dialog, setDialog] = useState<DialogState>(NONE);
  const searchRef = useRef<HTMLInputElement>(null);
  const copyField = useCopyField();

  useVaultEvents();

  const entriesQ = useQuery({
    queryKey: queryKeys.entries,
    queryFn: async () => call(commands.listEntries()),
  });
  const names = entriesQ.data ?? [];
  const tree = buildTree(names);
  const searching = query.trim() !== "";
  let validFolder = folder;
  while (validFolder !== "" && folderAt(tree, validFolder) === undefined) {
    validFolder = parentPath(validFolder);
  }
  const current = folderAt(tree, validFolder) ?? tree;
  const items = searching ? searchItems(query.trim(), names) : folderItems(current);
  const selectedEntry = selected !== undefined && names.includes(selected) ? selected : undefined;

  const openFolder = (path: string): void => {
    setFolder(path);
    setQuery("");
    setActive(0);
  };

  const pickImport = async (): Promise<void> => {
    const path = await pickFile();
    if (typeof path === "string") {
      setDialog({ kind: "import", path });
    }
  };

  const newSecret = (): void => {
    setDialog({ kind: "editor", target: { mode: "new", folder: validFolder } });
  };

  const copySelected = (): void => {
    if (selectedEntry !== undefined) {
      void copyField(selectedEntry, "value");
    }
  };

  useHotkeys(dialog.kind !== "none", {
    onSearch: () => searchRef.current?.focus(),
    onNew: newSecret,
    onLock: () => {
      void lock();
    },
    onSettings: () => {
      setDialog({ kind: "settings" });
    },
    onUp: () => {
      if (!searching && validFolder !== "") {
        openFolder(parentPath(validFolder));
      }
    },
    onCopy: copySelected,
    onEscape: () => {
      if (searching) {
        setQuery("");
      } else {
        setSelected(undefined);
      }
    },
  });

  const entryMenu = (name: string): MenuItem[] => [
    {
      label: m.vault.copySecret,
      onSelect: () => {
        void copyField(name, "value");
      },
    },
    {
      label: m.common.edit,
      onSelect: () => {
        setDialog({ kind: "editor", target: { mode: "edit", name } });
      },
    },
    {
      label: m.common.rename,
      onSelect: () => {
        setDialog({ kind: "rename", name });
      },
    },
    {
      label: m.common.delete,
      destructive: true,
      onSelect: () => {
        setDialog({ kind: "delete", name });
      },
    },
  ];

  const title = searching
    ? m.vault.results
    : validFolder === ""
      ? m.vault.allItems
      : leafName(validFolder);

  return (
    <div className="bg-window flex h-dvh">
      <Sidebar
        vaultPath={vault?.path ?? ""}
        isGit={vault?.isGit ?? false}
        total={tree.total}
        query={query}
        searchRef={searchRef}
        validFolder={validFolder}
        searching={searching}
        expanded={expanded}
        tree={tree}
        onQuery={(q) => {
          setQuery(q);
          setActive(0);
        }}
        onOpenFolder={openFolder}
        onToggle={(path) => {
          setExpanded((prev) =>
            prev.has(path)
              ? new Set([...prev].filter((p) => p !== path))
              : new Set([...prev, path]),
          );
        }}
        onSettings={() => {
          setDialog({ kind: "settings" });
        }}
      />

      <main className="bg-card rounded-panel mx-2 my-2 flex min-w-0 flex-1 flex-col">
        <ContentHeader
          title={title}
          itemCount={items.length}
          searching={searching}
          validFolder={validFolder}
          onBack={openFolder}
          onNewSecret={newSecret}
          onImport={() => {
            void pickImport();
          }}
        />
        {items.length === 0 ? (
          <EmptyState
            icon={searching ? Search : FolderOpen}
            title={searching ? m.vault.noResultsTitle : m.vault.emptyTitle}
            description={
              searching
                ? m.vault.noResultsHint
                : validFolder === ""
                  ? m.vault.emptyVaultHint
                  : m.vault.emptyFolderHint
            }
            action={
              searching ? undefined : (
                <Button icon={Plus} onClick={newSecret}>
                  {m.vault.newSecret}
                </Button>
              )
            }
          />
        ) : (
          <ItemList
            items={items}
            active={active}
            selected={selectedEntry}
            searching={searching}
            itemsLabel={m.vault.items}
            menuFor={(name) => entryMenu(name)}
            onActive={setActive}
            onSelect={setSelected}
            onOpenFolder={openFolder}
          />
        )}
      </main>

      {selectedEntry !== undefined && (
        <aside className="bg-card rounded-panel my-2 mr-2 flex w-80 shrink-0 flex-col">
          <Inspector
            key={selectedEntry}
            name={selectedEntry}
            onClose={() => {
              setSelected(undefined);
            }}
            onEdit={() => {
              setDialog({ kind: "editor", target: { mode: "edit", name: selectedEntry } });
            }}
            onRename={() => {
              setDialog({ kind: "rename", name: selectedEntry });
            }}
            onDelete={() => {
              setDialog({ kind: "delete", name: selectedEntry });
            }}
          />
        </aside>
      )}

      {dialog.kind === "editor" && (
        <EntryEditor
          target={dialog.target}
          onClose={() => {
            setDialog(NONE);
          }}
          onSaved={(name) => {
            setDialog(NONE);
            setFolder(parentPath(name));
            setSelected(name);
          }}
        />
      )}
      <SettingsDialog
        open={dialog.kind === "settings"}
        onOpenChange={(open) => {
          if (!open) {
            setDialog(NONE);
          }
        }}
      />
      {dialog.kind === "rename" && (
        <RenameDialog
          name={dialog.name}
          onClose={() => {
            setDialog(NONE);
          }}
          onRenamed={(next) => {
            setDialog(NONE);
            setFolder(parentPath(next));
            setSelected(next);
          }}
        />
      )}
      {dialog.kind === "delete" && (
        <DeleteDialog
          name={dialog.name}
          onClose={() => {
            setDialog(NONE);
          }}
          onDeleted={() => {
            setDialog(NONE);
            if (selected === dialog.name) {
              setSelected(undefined);
            }
          }}
        />
      )}
      {dialog.kind === "import" && (
        <ImportDialog
          path={dialog.path}
          folder={validFolder}
          onClose={() => {
            setDialog(NONE);
          }}
          onImported={(name) => {
            setDialog(NONE);
            setFolder(parentPath(name));
            setSelected(name);
          }}
        />
      )}
    </div>
  );
}
