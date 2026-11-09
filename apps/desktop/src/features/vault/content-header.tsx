import { ChevronLeft, FilePlus2, Import, Plus } from "lucide-react";
import type { ReactElement } from "react";

import { useMessages } from "../../i18n";
import { parentPath } from "../../lib/paths";
import { Button, IconButton, Menu } from "../../ui";

/** Header above the item list: back button in subfolders, title + count, the New menu. */
export function ContentHeader({
  title,
  itemCount,
  searching,
  validFolder,
  onBack,
  onNewSecret,
  onImport,
}: {
  title: string;
  itemCount: number;
  searching: boolean;
  validFolder: string;
  onBack: (path: string) => void;
  onNewSecret: () => void;
  onImport: () => void;
}): ReactElement {
  const m = useMessages();
  return (
    <div className="flex items-center gap-3 px-5 pt-4 pb-3">
      <div className="flex min-w-0 flex-1 items-center gap-2" data-tauri-drag-region>
        {!searching && validFolder !== "" && (
          <IconButton
            icon={ChevronLeft}
            label={m.common.back}
            onClick={() => {
              onBack(parentPath(validFolder));
            }}
          />
        )}
        <div className="min-w-0" data-tauri-drag-region>
          <h2 className="text-ink truncate text-2xl font-bold tracking-tight">{title}</h2>
          <p className="text-muted text-xs">{m.vault.items(itemCount)}</p>
        </div>
      </div>
      <Menu
        align="end"
        trigger={
          <Button icon={Plus} aria-label={m.vault.new}>
            {m.vault.new}
          </Button>
        }
        items={[
          { icon: FilePlus2, label: m.vault.newSecret, shortcut: "⌘N", onSelect: onNewSecret },
          { icon: Import, label: m.vault.importFile, onSelect: onImport },
        ]}
      />
    </div>
  );
}
