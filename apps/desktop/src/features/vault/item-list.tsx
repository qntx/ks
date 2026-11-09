/* oxlint-disable jsx-a11y/prefer-tag-over-role, jsx-a11y/click-events-have-key-events --
   WAI-ARIA listbox: options don't carry their own key handlers; keys live on the listbox. */
import { ChevronRight, Folder, KeyRound } from "lucide-react";
import type { KeyboardEvent as ReactKeyboardEvent, ReactElement } from "react";

import { cn, ContextMenu } from "../../ui";
import type { MenuItem } from "../../ui";
import type { ListItem } from "./tree";

/** Stable option id from the item identity (folder path or entry name), safe for aria ids. */
function itemDomId(item: ListItem): string {
  const raw = item.kind === "folder" ? `d:${item.path}` : `e:${item.name}`;
  return `ks-item-${raw.replaceAll(/[^a-zA-Z0-9_-]/g, "-")}`;
}

function scrollIntoView(id: string): void {
  document.querySelector(`#${id}`)?.scrollIntoView({ block: "nearest" });
}

/**
 * WAI-ARIA listbox. The keyboard-active option is the selection: arrows move it (entries become
 * selected so the inspector follows, folders only highlight), Enter opens a folder or keeps the
 * entry selected. Hover does not move the active option; clicking does.
 */
export function ItemList({
  items,
  active,
  selected,
  searching,
  itemsLabel,
  menuFor,
  onActive,
  onSelect,
  onOpenFolder,
}: {
  items: ListItem[];
  active: number;
  selected: string | undefined;
  searching: boolean;
  itemsLabel: (n: number) => string;
  menuFor: (name: string) => MenuItem[];
  onActive: (index: number) => void;
  onSelect: (name: string) => void;
  onOpenFolder: (path: string) => void;
}): ReactElement {
  const activate = (index: number): void => {
    const it = items[index];
    if (it === undefined) {
      return;
    }
    onActive(index);
    scrollIntoView(itemDomId(it));
    if (it.kind === "entry") {
      onSelect(it.name);
    }
  };

  const onKeyDown = (e: ReactKeyboardEvent<HTMLDivElement>): void => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      activate(Math.min(active + 1, items.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      activate(Math.max(active - 1, 0));
    } else if (e.key === "Home") {
      e.preventDefault();
      activate(0);
    } else if (e.key === "End") {
      e.preventDefault();
      activate(items.length - 1);
    } else if (e.key === "Enter") {
      e.preventDefault();
      const it = items[active];
      if (it?.kind === "folder") {
        onOpenFolder(it.path);
      } else if (it?.kind === "entry") {
        onSelect(it.name);
      }
    }
  };

  return (
    <div
      role="listbox"
      tabIndex={0}
      aria-label={itemsLabel(items.length)}
      aria-activedescendant={items[active] === undefined ? undefined : itemDomId(items[active])}
      className="min-h-0 flex-1 overflow-y-auto px-2 pb-3 outline-none"
      onKeyDown={onKeyDown}
    >
      {items.map((item, i) => (
        <ItemRow
          key={itemDomId(item)}
          id={itemDomId(item)}
          item={item}
          selected={item.kind === "entry" && item.name === selected}
          countLabel={item.kind === "folder" ? itemsLabel(item.count) : undefined}
          folderLabel={item.kind === "entry" && searching ? item.parent : undefined}
          menu={item.kind === "entry" ? menuFor(item.name) : undefined}
          onClick={() => {
            onActive(i);
            if (item.kind === "folder") {
              onOpenFolder(item.path);
            } else {
              onSelect(item.name);
            }
          }}
        />
      ))}
    </div>
  );
}

function ItemRow({
  id,
  item,
  selected,
  countLabel,
  folderLabel,
  menu,
  onClick,
}: {
  id: string;
  item: ListItem;
  selected: boolean;
  countLabel?: string | undefined;
  folderLabel?: string | undefined;
  menu?: MenuItem[] | undefined;
  onClick: () => void;
}): ReactElement {
  const row = (
    <div
      id={id}
      role="option"
      tabIndex={-1}
      aria-selected={selected}
      className={cn(
        "hover:bg-hover flex h-14 cursor-default items-center gap-3 rounded-2xl px-3",
        selected && "bg-surface hover:bg-surface",
      )}
      onClick={onClick}
    >
      <div
        className={cn(
          "bg-surface flex size-9 shrink-0 items-center justify-center rounded-xl",
          selected && "bg-card",
        )}
      >
        {item.kind === "folder" ? (
          <Folder className="text-muted size-4.5" strokeWidth={2} aria-hidden="true" />
        ) : (
          <KeyRound className="text-muted size-4.5" strokeWidth={2} aria-hidden="true" />
        )}
      </div>
      <div className="min-w-0 flex-1">
        <p className="text-ink truncate text-base font-medium">
          {item.kind === "folder" ? item.name : item.label}
        </p>
        {(countLabel ?? folderLabel) !== undefined && (
          <p className="text-muted truncate text-xs">{countLabel ?? folderLabel}</p>
        )}
      </div>
      {item.kind === "folder" && (
        <ChevronRight className="text-muted size-4" strokeWidth={2} aria-hidden="true" />
      )}
    </div>
  );
  if (menu !== undefined) {
    return <ContextMenu items={menu}>{row}</ContextMenu>;
  }
  return row;
}
