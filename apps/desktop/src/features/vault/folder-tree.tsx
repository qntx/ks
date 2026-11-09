import { ChevronRight, Folder } from "lucide-react";
import type { ReactElement } from "react";

import { cn } from "../../ui";
import type { FolderNode } from "./tree";

/**
 * Recursive folder rows. Each row is a plain container with two sibling buttons: the chevron toggle
 * (`aria-expanded`) and the folder name (`aria-current` on the active folder).
 */
export function FolderRows({
  node,
  depth = 0,
  current,
  searching,
  expanded,
  onOpen,
  onToggle,
  expandLabel,
}: {
  node: FolderNode;
  depth?: number;
  current: string;
  searching: boolean;
  expanded: ReadonlySet<string>;
  onOpen: (path: string) => void;
  onToggle: (path: string) => void;
  expandLabel: (name: string, isOpen: boolean) => string;
}): ReactElement {
  return (
    <>
      {node.folders.map((f) => {
        const isOpen = expanded.has(f.path);
        const isCurrent = current === f.path && !searching;
        return (
          <div key={f.path}>
            <div
              className={cn(
                "hover:bg-hover flex h-9 items-center gap-0.5 rounded-full pr-3",
                isCurrent && "bg-card shadow-selected",
              )}
              style={{ paddingLeft: 8 + depth * 14 }}
            >
              <button
                type="button"
                aria-expanded={isOpen}
                aria-label={expandLabel(f.name, isOpen)}
                className="flex size-5 shrink-0 items-center justify-center"
                onClick={() => {
                  onToggle(f.path);
                }}
              >
                <ChevronRight
                  className={cn(
                    "text-muted size-3.5 transition-transform duration-120",
                    isOpen && "rotate-90",
                  )}
                  strokeWidth={2}
                  aria-hidden="true"
                />
              </button>
              <button
                type="button"
                aria-current={isCurrent ? "page" : undefined}
                className="flex h-full min-w-0 flex-1 items-center gap-1.5 text-left"
                onClick={() => {
                  onOpen(f.path);
                }}
              >
                <Folder
                  className="text-muted size-4.5 shrink-0"
                  strokeWidth={2}
                  aria-hidden="true"
                />
                <span className="text-ink min-w-0 flex-1 truncate text-base">{f.name}</span>
                <span className="text-muted text-xs">{f.total}</span>
              </button>
            </div>
            {isOpen && (
              <FolderRows
                node={f}
                depth={depth + 1}
                current={current}
                searching={searching}
                expanded={expanded}
                onOpen={onOpen}
                onToggle={onToggle}
                expandLabel={expandLabel}
              />
            )}
          </div>
        );
      })}
    </>
  );
}
