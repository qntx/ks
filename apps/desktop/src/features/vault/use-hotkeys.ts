import { useEffect, useEffectEvent } from "react";

const IS_MAC = navigator.userAgent.includes("Mac");

function inEditable(e: KeyboardEvent): boolean {
  return (
    e.target instanceof HTMLElement &&
    e.target.closest("input, textarea, [contenteditable='true']") !== null
  );
}

export type HotkeyHandlers = {
  onSearch: () => void;
  onNew: () => void;
  onLock: () => void;
  onSettings: () => void;
  onUp: () => void;
  onCopy: () => void;
  onEscape: () => void;
};

/**
 * Global vault shortcuts; `disabled` while any dialog is open. The window listener subscribes once
 * per `disabled` change — the handler itself always sees the latest callbacks.
 */
export function useHotkeys(disabled: boolean, h: HotkeyHandlers): void {
  const onKey = useEffectEvent((e: KeyboardEvent): void => {
    if (disabled) {
      return;
    }
    const mod = IS_MAC ? e.metaKey : e.ctrlKey;
    if (mod && !e.altKey) {
      const k = e.key.toLowerCase();
      if (k === "f") {
        e.preventDefault();
        h.onSearch();
        return;
      }
      if (k === "n") {
        e.preventDefault();
        h.onNew();
        return;
      }
      if (k === "l") {
        e.preventDefault();
        h.onLock();
        return;
      }
      if (k === ",") {
        e.preventDefault();
        h.onSettings();
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        h.onUp();
        return;
      }
      if (k === "c" && !inEditable(e) && globalThis.getSelection()?.isCollapsed !== false) {
        e.preventDefault();
        h.onCopy();
        return;
      }
    }
    if (e.key === "Escape" && !inEditable(e)) {
      h.onEscape();
    }
  });

  useEffect(() => {
    const handler = (e: KeyboardEvent): void => {
      onKey(e);
    };
    globalThis.addEventListener("keydown", handler);
    return () => {
      globalThis.removeEventListener("keydown", handler);
    };
  }, []);
}
