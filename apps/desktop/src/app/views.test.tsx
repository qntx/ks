import { QueryClientProvider } from "@tanstack/react-query";
import { open as dialogOpen, save as dialogSave } from "@tauri-apps/plugin-dialog";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { Tooltip } from "radix-ui";
import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

import { Unlock } from "../features/access";
import { Inspector } from "../features/entry";
import { VaultScreen } from "../features/vault";
import { I18nProvider } from "../i18n";
import { App } from "./app";
import { queryClient } from "./query";
import { boot, resetSession } from "./session";

const ipc = await vi.hoisted(async () => {
  const m = await import("../test/bindings-mock");
  return m.createBindingsMock();
});
vi.mock("../ipc/bindings.gen", () => ({ commands: ipc.commands, events: ipc.events }));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(async () => Promise.resolve(null)),
  save: vi.fn(async () => Promise.resolve(null)),
}));

function renderApp(): ReturnType<typeof render> {
  return render(<App />);
}

function renderInspector(name: string): ReturnType<typeof render> {
  return render(
    <QueryClientProvider client={queryClient}>
      <I18nProvider language="en">
        <Tooltip.Provider>
          <Inspector
            name={name}
            onClose={vi.fn()}
            onEdit={vi.fn()}
            onRename={vi.fn()}
            onDelete={vi.fn()}
          />
        </Tooltip.Provider>
      </I18nProvider>
    </QueryClientProvider>,
  );
}

function renderUnlocked(): ReturnType<typeof render> {
  return render(
    <QueryClientProvider client={queryClient}>
      <I18nProvider language="en">
        <Tooltip.Provider>
          <VaultScreen />
        </Tooltip.Provider>
      </I18nProvider>
    </QueryClientProvider>,
  );
}

async function bootTo(phase: "welcome" | "locked" | "unlocked"): Promise<void> {
  ipc.commands.appState.mockResolvedValue({
    vault: phase === "welcome" ? null : ipc.store.vault,
    unlocked: phase === "unlocked",
    lockReason: null,
    settings: ipc.store.settings,
  });
  await boot();
}

describe("views", () => {
  beforeEach(() => {
    cleanup();
    vi.clearAllMocks();
    resetSession();
    queryClient.clear();
    ipc.store.texts = new Map([
      [
        "work/github",
        "ghp_secret\nuser: octocat\ntotp: GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ\nnotes: main",
      ],
      ["work/aws/root", "AKIAEXAMPLE\nuser: root"],
      ["personal/bank", "rosebud\nuser: 4471"],
      ["solo", "v"],
    ]);
    ipc.store.names = [...ipc.store.texts.keys(), "personal/passport.pdf"];
  });

  it("wrong passphrase shows the localized error and clears the field", async () => {
    render(
      <I18nProvider language="en">
        <Tooltip.Provider>
          <Unlock />
        </Tooltip.Provider>
      </I18nProvider>,
    );
    const input = screen.getByLabelText("Passphrase");
    fireEvent.change(input, { target: { value: "wrong" } });
    fireEvent.click(screen.getByRole("button", { name: "Unlock" }));
    await expect(screen.findByText("Wrong passphrase")).resolves.toBeDefined();
    expect(input).toHaveProperty("value", "");
  });

  it("a locked event returns the app to Unlock", async () => {
    await bootTo("unlocked");
    renderApp();
    const allItems = await screen.findAllByText("All items");
    expect(allItems.length).toBeGreaterThan(0);
    ipc.emitLocked("idle");
    await waitFor(() => {
      expect(screen.getByText("Locked after inactivity")).toBeDefined();
    });
  });

  it("navigates folders and searches", async () => {
    renderUnlocked();
    await bootTo("unlocked");
    const listbox = await screen.findByRole("listbox");
    // root: folders first, then entries
    expect(within(listbox).getByText("work")).toBeDefined();
    expect(within(listbox).getByText("personal")).toBeDefined();
    expect(within(listbox).getByText("solo")).toBeDefined();
    fireEvent.click(within(listbox).getByText("work"));
    await expect(within(listbox).findByText("aws")).resolves.toBeDefined();
    expect(within(listbox).getByText("github")).toBeDefined();
    fireEvent.change(screen.getByLabelText("Search"), { target: { value: "bank" } });
    await expect(screen.findByText("bank")).resolves.toBeDefined();
    expect(screen.queryByText("github")).toBeNull();
    expect(within(listbox).getByText("personal")).toBeDefined();
  });

  it("reveal calls revealField; copy calls copyField only", async () => {
    const clipSpy = vi.fn();
    Object.defineProperty(navigator, "clipboard", {
      value: { writeText: clipSpy },
      configurable: true,
    });
    renderUnlocked();
    await bootTo("unlocked");
    const listbox = await screen.findByRole("listbox");
    fireEvent.click(within(listbox).getByText("work"));
    fireEvent.click(await within(listbox).findByText("github"));
    const showButtons = await screen.findAllByRole("button", { name: "Show" });
    fireEvent.click(showButtons[0]!);
    await waitFor(() => {
      expect(ipc.commands.revealField).toHaveBeenCalledWith("work/github", "value");
    });
    fireEvent.click(screen.getByRole("button", { name: "Copy secret" }));
    await waitFor(() => {
      expect(ipc.commands.copyField).toHaveBeenCalledWith("work/github", "value");
    });
    expect(clipSpy).not.toHaveBeenCalled();
  });

  it("welcome → create card calls createVault and lands on the vault screen", async () => {
    await bootTo("welcome");
    renderApp();
    fireEvent.click(await screen.findByRole("button", { name: "New vault" }));
    const pass = screen.getByLabelText("Passphrase");
    fireEvent.change(pass, { target: { value: "correct horse battery staple" } });
    fireEvent.change(screen.getByLabelText("Repeat passphrase"), {
      target: { value: "correct horse battery staple" },
    });
    const submitBtn = screen.getByRole("button", { name: "Create vault" });
    await waitFor(() => {
      expect(submitBtn).toHaveProperty("disabled", false);
    });
    fireEvent.click(submitBtn);
    await waitFor(() => {
      expect(ipc.commands.createVault).toHaveBeenCalled();
    });
    const allItems = await screen.findAllByText("All items");
    expect(allItems.length).toBeGreaterThan(0);
  });

  it("welcome 'Choose a folder' calls openVault with the picked path", async () => {
    await bootTo("welcome");
    renderApp();
    fireEvent.click(await screen.findByRole("button", { name: "I have one" }));
    vi.mocked(dialogOpen).mockResolvedValue("/vaults/alt");
    fireEvent.click(await screen.findByRole("button", { name: "Choose a folder" }));
    await waitFor(() => {
      expect(ipc.commands.openVault).toHaveBeenCalledWith("/vaults/alt");
    });
  });

  it("unlock 'Open another vault' calls openVault with the picked path", async () => {
    await bootTo("locked");
    renderApp();
    vi.mocked(dialogOpen).mockResolvedValue("/vaults/other");
    fireEvent.click(await screen.findByRole("button", { name: "Open another vault" }));
    await waitFor(() => {
      expect(ipc.commands.openVault).toHaveBeenCalledWith("/vaults/other");
    });
  });

  it("inspector Export calls exportBinary with the picked path", async () => {
    await bootTo("unlocked");
    renderInspector("personal/passport.pdf");
    vi.mocked(dialogSave).mockResolvedValue("/tmp/out.pdf");
    fireEvent.click(await screen.findByRole("button", { name: /Export/ }));
    await waitFor(() => {
      expect(ipc.commands.exportBinary).toHaveBeenCalledWith(
        "personal/passport.pdf",
        "/tmp/out.pdf",
      );
    });
  });

  it("inspector Replace calls importBinary with overwrite", async () => {
    await bootTo("unlocked");
    renderInspector("personal/passport.pdf");
    vi.mocked(dialogOpen).mockResolvedValue("/tmp/new.pdf");
    fireEvent.click(await screen.findByRole("button", { name: /Replace/ }));
    await waitFor(() => {
      expect(ipc.commands.importBinary).toHaveBeenCalledWith(
        "personal/passport.pdf",
        "/tmp/new.pdf",
        true,
      );
    });
  });

  it("editor saves the structured draft as serialized text", async () => {
    renderUnlocked();
    await bootTo("unlocked");
    const listbox = await screen.findByRole("listbox");
    fireEvent.click(within(listbox).getByText("work"));
    fireEvent.click(await within(listbox).findByText("github"));
    fireEvent.click(await screen.findByRole("button", { name: "Edit" }));
    const save = await screen.findByRole("button", { name: "Save" });
    fireEvent.click(save);
    await waitFor(() => {
      expect(ipc.commands.saveEntry).toHaveBeenCalledWith(
        "work/github",
        "ghp_secret\nuser: octocat\ntotp: GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ\nnotes: main",
        true,
      );
    });
  });
});
