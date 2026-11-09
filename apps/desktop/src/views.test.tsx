// oxlint-disable typescript/require-await -- test mocks resolve synchronously by design
// oxlint-disable import/first -- vi.mock factory must precede the mocked import in source order
// oxlint-disable vitest/no-conditional-in-test -- fixture lookups tolerate fallbacks
// oxlint-disable typescript/no-unnecessary-type-assertion -- DOM queries need element-specific casts
import { render, screen, fireEvent, act } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

vi.mock("./bindings.gen", () => {
  const ok = <T,>(data: T) => ({ status: "ok" as const, data });
  const err = (code: string, message = code) => ({
    status: "error" as const,
    error: { code, message },
  });
  const listeners = new Map<string, (e: { payload: unknown }) => void>();
  return {
    __emit: (ev: string, payload: unknown) => listeners.get(ev)?.({ payload }),
    commands: {
      appState: vi.fn(async () => ({
        vault: { path: "/v", isGit: false, warnings: [] },
        unlocked: false,
        settings: {
          autoLockMinutes: 15,
          maxSessionHours: 8,
          clipboardSeconds: 45,
          lockOnSleep: true,
          language: "system",
        },
      })),
      unlock: vi.fn(async (p: string) =>
        p === "right" ? ok({ warnings: [] }) : err("wrong_passphrase"),
      ),
      lock: vi.fn(async () => ok(null)),
      heartbeat: vi.fn(async () => undefined),
      listEntries: vi.fn(async () => ok(["a/b"])),
      entryDetail: vi.fn(async (name: string) =>
        ok({
          name,
          kind: "text",
          value: { key: "value", length: 6 },
          fields: [],
          notesLength: 0,
          hasTotp: false,
          binaryLength: null,
        }),
      ),
      revealField: vi.fn(async () => ok("TOPSECRET")),
      copyField: vi.fn(async () => ok({ clearsInSecs: 45 })),
      saveEntry: vi.fn(async () => ok([])),
      editEntry: vi.fn(async () => ok("existing")),
      getSettings: vi.fn(async () => ({})),
      setSettings: vi.fn(async () => ok(null)),
    },
    events: {
      locked: { listen: vi.fn(async (cb: never) => void listeners.set("locked", cb)) },
      entriesChanged: {
        listen: vi.fn(async (cb: never) => void listeners.set("entries-changed", cb)),
      },
    },
  };
});

import { en } from "./i18n/en";
import * as store from "./store";
import { Editor } from "./views/editor";
import { EntryView } from "./views/entry";
import { Unlock } from "./views/unlock";

beforeEach(() => {
  vi.clearAllMocks();
  store.reset();
});

describe("DESK-CORE views", () => {
  it("desk_core_unlock_clears_the_field_and_maps_error", async () => {
    render(<Unlock m={en} />);
    const input = screen.getByLabelText(en.passphrase) as HTMLInputElement;
    fireEvent.change(input, { target: { value: "nope" } });
    fireEvent.click(screen.getByText(en.unlock));
    // field cleared synchronously, before the command resolves
    expect(input.value).toBe("");
    await screen.findByText(en.error["wrong_passphrase"] ?? "");
  });

  it("desk_core_reveal_autohides_and_stays_out_of_the_store", async () => {
    await act(async () => {
      await store.init();
      await store.doUnlock("right");
    });
    render(<EntryView m={en} onEdit={() => {}} onSettings={() => {}} />);
    await act(async () => {
      await store.select("a/b");
    });
    const reveal = await screen.findByText(en.reveal);
    vi.useFakeTimers();
    await act(async () => {
      fireEvent.click(reveal);
      await Promise.resolve();
    });
    expect(screen.queryByText("TOPSECRET")).not.toBeNull();
    // never reaches the store
    expect(JSON.stringify(store.getSnap())).not.toContain("TOPSECRET");
    act(() => {
      vi.advanceTimersByTime(31_000);
    });
    expect(screen.queryByText("TOPSECRET")).toBeNull();
    vi.useRealTimers();
  });

  it("desk_core_editor_clears_text_on_close", async () => {
    const close = vi.fn();
    const { rerender } = render(<Editor m={en} name={undefined} onClose={close} />);
    const name = screen.getByLabelText(en.editorName);
    const text = screen.getByLabelText(en.editorText);
    fireEvent.change(name, { target: { value: "n" } });
    fireEvent.change(text, { target: { value: "typed-secret" } });
    await act(async () => {
      fireEvent.click(screen.getByText(en.save));
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(close).toHaveBeenCalled();
    // a fresh editor mounts empty — the secret text was local state
    rerender(<Editor m={en} name={undefined} onClose={close} />);
    expect((screen.getByLabelText(en.editorText) as HTMLTextAreaElement).value).toBe("");
  });
});
