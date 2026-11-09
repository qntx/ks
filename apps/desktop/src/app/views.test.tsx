import { QueryClientProvider } from "@tanstack/react-query";
import { open as dialogOpen, save as dialogSave } from "@tauri-apps/plugin-dialog";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { Tooltip } from "radix-ui";
import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

import { Unlock } from "../features/access";
import { Inspector } from "../features/entry";
import { QuickApp } from "../features/quick";
import { SettingsDialog } from "../features/settings";
import { resetSync } from "../features/sync";
import { VaultScreen } from "../features/vault";
import { I18nProvider } from "../i18n";
import { IpcError } from "../ipc/call";
import { Toaster } from "../ui";
import { App } from "./app";
import { queryClient, queryKeys } from "./query";
import { boot, resetSession, saveSettings } from "./session";

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

function renderUnlock(): void {
  render(
    <QueryClientProvider client={queryClient}>
      <I18nProvider language="en">
        <Tooltip.Provider>
          <Unlock />
        </Tooltip.Provider>
      </I18nProvider>
    </QueryClientProvider>,
  );
}

const ok = <T,>(data: T): { status: "ok"; data: T } => ({ status: "ok", data });

const DEVICES_TOUCH = {
  touchId: { supported: true, enabled: true, reason: null },
  keychain: { supported: true, enabled: false, reason: null },
};

describe("views", () => {
  beforeEach(() => {
    cleanup();
    vi.clearAllMocks();
    resetSession();
    resetSync();
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
    ipc.store.gitStatus = undefined;
    ipc.store.syncState = { status: "idle" };
    ipc.store.doctorProblems = [];
  });

  it("wrong passphrase shows the localized error and clears the field", async () => {
    renderUnlock();
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

  it("auto-runs unlockDevice when a device is enabled, never otherwise", async () => {
    renderUnlock();
    await waitFor(() => {
      expect(ipc.commands.devices).toHaveBeenCalled();
    });
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(ipc.commands.unlockDevice).not.toHaveBeenCalled();

    cleanup();
    resetSession();
    resetSync();
    queryClient.clear();
    ipc.store.devices = structuredClone(DEVICES_TOUCH);
    renderUnlock();
    await waitFor(() => {
      expect(ipc.commands.unlockDevice).toHaveBeenCalledTimes(1);
    });
  });

  it("shows the Touch ID pill only when Touch ID is enabled", async () => {
    renderUnlock();
    await waitFor(() => {
      expect(ipc.commands.devices).toHaveBeenCalled();
    });
    expect(screen.queryByRole("button", { name: "Unlock with Touch ID" })).toBeNull();

    cleanup();
    resetSession();
    resetSync();
    queryClient.clear();
    ipc.store.devices = structuredClone(DEVICES_TOUCH);
    renderUnlock();
    await expect(
      screen.findByRole("button", { name: "Unlock with Touch ID" }),
    ).resolves.toBeDefined();
  });

  it("stale shows the notice and submits passphrase with retireStale", async () => {
    ipc.store.devices = structuredClone(DEVICES_TOUCH);
    ipc.commands.unlockDevice.mockResolvedValue(ok({ unlocked: false, warnings: [], stale: true }));
    renderUnlock();
    await expect(
      screen.findByText("Device unlock is out of date — enter your passphrase to remove it"),
    ).resolves.toBeDefined();
    const input = screen.getByLabelText("Passphrase");
    fireEvent.change(input, { target: { value: "correct horse battery staple" } });
    fireEvent.click(screen.getByRole("button", { name: "Unlock" }));
    await waitFor(() => {
      expect(ipc.commands.unlock).toHaveBeenCalledWith("correct horse battery staple", true);
    });
  });

  it("settings device group toggles via setDevice and refetches state", async () => {
    await bootTo("unlocked");
    ipc.store.devices = structuredClone(DEVICES_TOUCH);
    render(
      <QueryClientProvider client={queryClient}>
        <I18nProvider language="en">
          <Tooltip.Provider>
            <SettingsDialog open onOpenChange={() => {}} />
          </Tooltip.Provider>
        </I18nProvider>
      </QueryClientProvider>,
    );
    const touchRow = await screen.findByRole("switch", {
      name: "Unlock with Touch ID",
    });
    expect(touchRow.getAttribute("aria-checked")).toBe("true");
    fireEvent.click(touchRow);
    await waitFor(() => {
      expect(ipc.commands.setDevice).toHaveBeenCalledWith("touch_id", false);
    });
    await waitFor(() => {
      expect(
        screen.getByRole("switch", { name: "Unlock with Touch ID" }).getAttribute("aria-checked"),
      ).toBe("false");
    });
  });

  it("an unsupported device row is disabled and shows the reason", async () => {
    await bootTo("unlocked");
    ipc.store.devices = {
      touchId: {
        supported: false,
        enabled: false,
        reason: "age-plugin-se was not found; install it with brew install age-plugin-se",
      },
      keychain: { supported: true, enabled: false, reason: null },
    };
    render(
      <QueryClientProvider client={queryClient}>
        <I18nProvider language="en">
          <Tooltip.Provider>
            <SettingsDialog open onOpenChange={() => {}} />
          </Tooltip.Provider>
        </I18nProvider>
      </QueryClientProvider>,
    );
    const row = await screen.findByRole("switch", {
      name: "Unlock with Touch ID",
    });
    expect(row).toHaveProperty("disabled", true);
    await expect(
      screen.findByText("age-plugin-se was not found; install it with brew install age-plugin-se"),
    ).resolves.toBeDefined();
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

  const GIT_STATUS = {
    branch: "main",
    upstream: "origin/main",
    ahead: 1,
    behind: 0,
    changes: 2,
  };

  function renderSettings(): void {
    render(
      <QueryClientProvider client={queryClient}>
        <I18nProvider language="en">
          <Tooltip.Provider>
            <SettingsDialog open onOpenChange={() => {}} />
          </Tooltip.Provider>
        </I18nProvider>
      </QueryClientProvider>,
    );
  }

  it("sync pill is hidden for a non-git vault", async () => {
    renderUnlocked();
    await bootTo("unlocked");
    await screen.findByRole("listbox");
    expect(screen.queryByText("main")).toBeNull();
  });

  it("sync pill shows branch, ahead/behind, changes; click runs syncNow", async () => {
    ipc.store.gitStatus = GIT_STATUS;
    renderUnlocked();
    await bootTo("unlocked");
    const pill = await screen.findByRole("button", { name: /main/ });
    expect(screen.getByText(/↑1/)).toBeDefined();
    expect(screen.queryByText(/↓0/)).toBeNull();
    expect(screen.getByText(/2 changes/)).toBeDefined();
    fireEvent.click(pill);
    await waitFor(() => {
      expect(ipc.commands.syncNow).toHaveBeenCalledWith(null);
    });
  });

  it("sync pill shows running then failed state labels", async () => {
    ipc.store.gitStatus = GIT_STATUS;
    ipc.store.syncState = { status: "running" };
    renderUnlocked();
    await bootTo("unlocked");
    await expect(screen.findByText("Syncing…")).resolves.toBeDefined();
    ipc.emitSyncState({ status: "failed", message: "network unreachable" });
    await expect(screen.findByText("Sync failed")).resolves.toBeDefined();
  });

  it("a content conflict opens the dialog; Keep this Mac retries with local", async () => {
    ipc.store.gitStatus = GIT_STATUS;
    ipc.commands.syncNow
      .mockResolvedValueOnce(
        ok({ status: "conflict", entries: ["mail/github", "bank"], kind: "content" }),
      )
      .mockResolvedValue(
        ok({ status: "synced", committed: false, pulled: true, pushed: true, warnings: [] }),
      );
    renderUnlocked();
    await bootTo("unlocked");
    fireEvent.click(await screen.findByRole("button", { name: /main/ }));
    await expect(screen.findByText("Sync conflict")).resolves.toBeDefined();
    expect(screen.getByText("mail/github")).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: "Keep this Mac" }));
    await waitFor(() => {
      expect(ipc.commands.syncNow).toHaveBeenCalledWith("local");
    });
  });

  it("a delete conflict shows the manual steps and no keep buttons", async () => {
    ipc.store.gitStatus = GIT_STATUS;
    ipc.commands.syncNow.mockResolvedValue(
      ok({ status: "conflict", entries: ["old"], kind: "delete" }),
    );
    renderUnlocked();
    await bootTo("unlocked");
    fireEvent.click(await screen.findByRole("button", { name: /main/ }));
    await screen.findByText("Sync conflict");
    expect(screen.getByText(/ks git pull --rebase/)).toBeDefined();
    expect(screen.queryByRole("button", { name: "Keep this Mac" })).toBeNull();
  });

  it("settings sync group toggles via saveSettings; hidden for non-git", async () => {
    await bootTo("unlocked");
    renderSettings();
    await screen.findByText("Device unlock");
    expect(screen.queryByText("Sync after unlocking")).toBeNull();
    cleanup();
    ipc.store.gitStatus = GIT_STATUS;
    queryClient.removeQueries({ queryKey: queryKeys.gitStatus });
    renderSettings();
    const row = await screen.findByRole("switch", { name: "Sync after unlocking" });
    fireEvent.click(row);
    await waitFor(() => {
      expect(ipc.commands.setSettings).toHaveBeenCalled();
    });
    const last = ipc.commands.setSettings.mock.calls.at(-1)![0] as Record<string, unknown>;
    expect(last["syncOnUnlock"]).toBe(false);
  });

  const DOCTOR_ALL_FIXABLE = [
    {
      path: "/vaults/ks/.tmpabc",
      kind: "temp_file" as const,
      message: "leftover temp file",
      fixable: true,
    },
    {
      path: "/vaults/ks/.gitignore",
      kind: "git_meta" as const,
      message: ".gitignore differs from the canonical content",
      fixable: true,
    },
  ];
  const DOCTOR_UNFIXABLE = [
    {
      path: "/vaults/ks/.identity.age",
      kind: "unfinished_rotation" as const,
      message: "unfinished key rotation; run `ks rekey` with the new passphrase to finish",
      fixable: false,
    },
  ];

  it("doctor page shows clean state; back and Esc return to main", async () => {
    await bootTo("unlocked");
    renderSettings();
    fireEvent.click(await screen.findByText("Check vault"));
    await expect(screen.findByText("No problems found")).resolves.toBeDefined();
    expect(ipc.commands.doctor).toHaveBeenCalled();
    const back = screen.getByRole("button", { name: "Settings" });
    fireEvent.click(back);
    await screen.findByText("Device unlock");

    fireEvent.click(screen.getByText("Check vault"));
    await screen.findByText("No problems found");
    fireEvent.keyDown(document, { key: "Escape" });
    await screen.findByText("Device unlock");
    expect(screen.queryByRole("button", { name: "Settings" })).toBeNull();
  });

  it("doctor page groups problems and Fix returns the remaining", async () => {
    ipc.store.doctorProblems = [
      ...DOCTOR_ALL_FIXABLE,
      ...DOCTOR_UNFIXABLE,
      {
        path: "/vaults/ks/work/github.age",
        kind: "undecryptable" as const,
        message: "decryption failed (possibly written before a key rotation)",
        fixable: false,
      },
    ];
    await bootTo("unlocked");
    renderSettings();
    fireEvent.click(await screen.findByText("Check vault"));
    await expect(screen.findByText("Leftover temp files")).resolves.toBeDefined();
    expect(screen.getByText("Git metadata")).toBeDefined();
    expect(screen.getByText("Unfinished key rotation")).toBeDefined();
    expect(screen.getByText(".tmpabc")).toBeDefined();
    expect(screen.getByText("work/github.age")).toBeDefined();
    expect(screen.getByText(/decryption failed/)).toBeDefined();
    expect(screen.queryByText("leftover temp file")).toBeNull();
    expect(screen.getByText("Finish it from Rotate key")).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: "Fix" }));
    await waitFor(() => {
      expect(ipc.commands.doctorFix).toHaveBeenCalled();
    });
    await waitFor(() => {
      expect(screen.queryByText("Leftover temp files")).toBeNull();
    });
    expect(screen.getByText("Unfinished key rotation")).toBeDefined();
  });

  it("doctor page hides Fix when nothing is fixable", async () => {
    ipc.store.doctorProblems = [...DOCTOR_UNFIXABLE];
    await bootTo("unlocked");
    renderSettings();
    fireEvent.click(await screen.findByText("Check vault"));
    await screen.findByText("Unfinished key rotation");
    expect(screen.queryByRole("button", { name: "Fix" })).toBeNull();
  });

  it("doctor page shows the error and retries into clean", async () => {
    ipc.commands.doctor.mockRejectedValueOnce(new Error("cannot read vault"));
    await bootTo("unlocked");
    renderSettings();
    fireEvent.click(await screen.findByText("Check vault"));
    await expect(screen.findByText(/cannot read vault/)).resolves.toBeDefined();
    expect(screen.queryByText("No problems found")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    await expect(screen.findByText("No problems found")).resolves.toBeDefined();
  });

  function fillField(label: string, value: string): void {
    fireEvent.change(screen.getByLabelText(label), { target: { value } });
  }

  it("passphrase page: mismatch blocks, success returns to main", async () => {
    await bootTo("unlocked");
    renderSettings();
    fireEvent.click(await screen.findByText("Change passphrase"));
    fillField("Current passphrase", "correct horse battery staple");
    fillField("New passphrase", "abc");
    fillField("Repeat new passphrase", "xyz");
    const btn = screen.getByRole("button", { name: "Change passphrase" });
    expect(btn).toHaveProperty("disabled", true);
    await expect(screen.findByText("Passphrases do not match")).resolves.toBeDefined();
    fillField("Repeat new passphrase", "abc");
    fireEvent.click(btn);
    await waitFor(() => {
      expect(ipc.commands.changePassphrase).toHaveBeenCalledWith(
        "correct horse battery staple",
        "abc",
      );
    });
    await screen.findByText("Device unlock");
  });

  it("rotate page: success shows the done view", async () => {
    ipc.store.gitStatus = {
      branch: "main",
      upstream: "origin/main",
      ahead: 0,
      behind: 0,
      changes: 0,
    };
    await bootTo("unlocked");
    renderSettings();
    fireEvent.click(await screen.findByText("Rotate key"));
    await screen.findByLabelText("Current passphrase");
    fillField("Current passphrase", "correct horse battery staple");
    fillField("New passphrase", "troubadour galloping mentor");
    fillField("Repeat new passphrase", "troubadour galloping mentor");
    fireEvent.click(screen.getByRole("button", { name: "Rotate key" }));
    await waitFor(() => {
      expect(ipc.commands.rotateKey).toHaveBeenCalledWith(
        "correct horse battery staple",
        "troubadour galloping mentor",
      );
    });
    await expect(screen.findByText(/3 entries re-encrypted/)).resolves.toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: "Done" }));
    await screen.findByText("Device unlock");
  });

  it("rotate page: a dirty tree disables the form and offers Sync now", async () => {
    ipc.store.gitStatus = {
      branch: "main",
      upstream: "origin/main",
      ahead: 0,
      behind: 0,
      changes: 2,
    };
    await bootTo("unlocked");
    renderSettings();
    fireEvent.click(await screen.findByText("Rotate key"));
    await expect(screen.findByText(/uncommitted changes/)).resolves.toBeDefined();
    expect(screen.getByRole("button", { name: "Rotate key" })).toHaveProperty("disabled", true);
    fireEvent.click(screen.getByRole("button", { name: "Sync now" }));
    await waitFor(() => {
      expect(ipc.commands.syncNow).toHaveBeenCalled();
    });
  });

  it("rotate page: pending rotation shows the finish variant", async () => {
    ipc.store.rotationPending = true;
    await bootTo("unlocked");
    renderSettings();
    fireEvent.click(await screen.findByText("Rotate key"));
    await expect(screen.findByText(/rotation was interrupted/)).resolves.toBeDefined();
    await screen.findByLabelText("Passphrase");
    fillField("Passphrase", "the new passphrase");
    fireEvent.click(screen.getByRole("button", { name: "Finish rotation" }));
    await waitFor(() => {
      expect(ipc.commands.rotateKey).toHaveBeenCalledWith("the new passphrase", null);
    });
  });

  it("recovery page renders the commands", async () => {
    await bootTo("unlocked");
    renderSettings();
    fireEvent.click(await screen.findByText("Recovery instructions"));
    await expect(screen.findByText(/age -d \/vaults\/ks\/.identity.age/)).resolves.toBeDefined();
    expect(screen.getByText(/\/vaults\/ks\/github\/token.age/)).toBeDefined();
    expect(screen.getByText(/ks git log --oneline/)).toBeDefined();
    expect(screen.getByText(/age -d -i \/tmp\/old-identity.age/)).toBeDefined();
  });

  it("settings: the Open at login switch reflects and calls setAutostart", async () => {
    ipc.store.autostart = true;
    await bootTo("unlocked");
    renderSettings();
    const sw = await screen.findByRole("switch", { name: "Open at login" });
    await waitFor(() => {
      expect(sw.getAttribute("aria-checked")).toBe("true");
    });
    fireEvent.click(sw);
    await waitFor(() => {
      expect(ipc.commands.setAutostart).toHaveBeenCalledWith(false);
    });
  });

  it("sends localized tray labels when the language is zh", async () => {
    await bootTo("unlocked");
    renderApp();
    await saveSettings({ language: "zh" });
    await waitFor(() => {
      expect(ipc.commands.setTrayLabels).toHaveBeenCalledWith({
        open: "打开 ks",
        lock: "立即锁定",
        quit: "退出",
        locked: "已锁定",
        unlocked: "已解锁",
        quick: "快速访问",
      });
    });
  });

  it("quick shortcut menu saves a preset", async () => {
    await bootTo("unlocked");
    renderSettings();
    fireEvent.pointerDown(await screen.findByRole("button", { name: "Quick access shortcut" }));
    fireEvent.click(await screen.findByRole("menuitemradio", { name: "⌥Space" }));
    await waitFor(() => {
      expect(ipc.commands.setSettings).toHaveBeenCalledWith(
        expect.objectContaining({ quickShortcut: "Alt+Space" }),
      );
    });
    expect(screen.getByRole("button", { name: "Quick access shortcut" }).textContent).toContain(
      "⌥Space",
    );
  });

  it("keeps the shortcut value when the chord is taken", async () => {
    ipc.store.settings = { ...ipc.store.settings, quickShortcut: "CommandOrControl+Shift+Space" };
    await bootTo("unlocked");
    ipc.commands.setSettings.mockRejectedValueOnce(
      new IpcError({ code: "shortcut_unavailable", message: "in use" }),
    );
    render(
      <QueryClientProvider client={queryClient}>
        <I18nProvider language="en">
          <Tooltip.Provider>
            <SettingsDialog open onOpenChange={() => {}} />
            <Toaster />
          </Tooltip.Provider>
        </I18nProvider>
      </QueryClientProvider>,
    );
    fireEvent.pointerDown(await screen.findByRole("button", { name: "Quick access shortcut" }));
    fireEvent.click(await screen.findByRole("menuitemradio", { name: "⌥Space" }));
    await screen.findByText("This shortcut is used by another app");
    expect(screen.getByRole("button", { name: "Quick access shortcut" }).textContent).toContain(
      "⌘⇧Space",
    );
  });
});

describe("quick", () => {
  let storeUnlocked = true;

  beforeEach(() => {
    cleanup();
    ipc.store.settings = { ...ipc.store.settings, language: "en" };
    ipc.commands.appState.mockImplementation(async () => {
      await Promise.resolve();
      return {
        vault: ipc.store.vault,
        unlocked: storeUnlocked,
        lockReason: null,
        settings: ipc.store.settings,
      };
    });
  });

  function renderQuick(): void {
    render(<QuickApp />);
  }

  async function searchInput(): Promise<HTMLElement> {
    return screen.findByRole("combobox", { name: "Search entries…" });
  }

  it("filters results while typing", async () => {
    storeUnlocked = true;
    renderQuick();
    const input = await searchInput();
    const found1 = await screen.findByRole("option", { name: /github/ });
    expect(found1).toBeDefined();
    fireEvent.change(input, { target: { value: "passport" } });
    await waitFor(() => {
      expect(screen.queryByRole("option", { name: /github/ })).toBeNull();
    });
    const found2 = await screen.findByRole("option", { name: /passport/ });
    expect(found2).toBeDefined();
  });

  it("arrow down moves the active option", async () => {
    storeUnlocked = true;
    renderQuick();
    const input = await searchInput();
    await waitFor(() => {
      expect(screen.getAllByRole("option").length).toBeGreaterThan(1);
    });
    const [first, second] = screen.getAllByRole("option");
    fireEvent.keyDown(input, { key: "ArrowDown" });
    await waitFor(() => {
      expect(input.getAttribute("aria-activedescendant")).toBe(second?.id);
      expect(input.getAttribute("aria-activedescendant")).not.toBe(first?.id);
    });
  });

  it("enter copies the secret and hides", async () => {
    storeUnlocked = true;
    renderQuick();
    const input = await searchInput();
    await waitFor(() => {
      expect(screen.getAllByRole("option").length).toBeGreaterThan(0);
    });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => {
      expect(ipc.commands.copyField).toHaveBeenCalledWith("personal/bank", "value");
      expect(ipc.commands.hideQuick).toHaveBeenCalled();
    });
  });

  it("meta+enter without totp shows the message and stays", async () => {
    storeUnlocked = true;
    renderQuick();
    const input = await searchInput();
    await waitFor(() => {
      expect(screen.getAllByRole("option").length).toBeGreaterThan(0);
    });
    // The copy_field(totp) call rejects with `no_totp` for this entry.
    ipc.commands.copyField.mockRejectedValueOnce(
      new IpcError({ code: "no_totp", message: "this entry has no TOTP" }),
    );
    fireEvent.keyDown(input, { key: "Enter", metaKey: true });
    const found3 = await screen.findByText("No one-time code for this entry");
    expect(found3).toBeDefined();
    expect(ipc.commands.hideQuick).not.toHaveBeenCalled();
  });

  it("shows an empty state for no matches", async () => {
    storeUnlocked = true;
    renderQuick();
    const input = await searchInput();
    const found4 = await screen.findByRole("option", { name: /github/ });
    expect(found4).toBeDefined();
    fireEvent.change(input, { target: { value: "zzzznomatch" } });
    const found5 = await screen.findByText("No matching entries");
    expect(found5).toBeDefined();
  });

  it("shows an empty state for an empty vault", async () => {
    storeUnlocked = true;
    const { names } = ipc.store;
    ipc.store.names = [];
    try {
      renderQuick();
      await searchInput();
      const found = await screen.findByText("No entries yet");
      expect(found).toBeDefined();
    } finally {
      ipc.store.names = names;
    }
  });

  it("esc hides the panel", async () => {
    storeUnlocked = true;
    renderQuick();
    const input = await searchInput();
    fireEvent.keyDown(input, { key: "Escape" });
    await waitFor(() => {
      expect(ipc.commands.hideQuick).toHaveBeenCalled();
    });
  });

  it("unlocking shows the search view", async () => {
    storeUnlocked = false;
    ipc.commands.unlock.mockImplementation(async () => {
      await Promise.resolve();
      storeUnlocked = true;
      return { status: "ok" as const, data: { warnings: [] } };
    });
    renderQuick();
    const field = await screen.findByLabelText("Passphrase");
    fireEvent.change(field, { target: { value: "correct horse battery staple" } });
    fireEvent.click(screen.getByRole("button", { name: "Unlock" }));
    const found6 = await screen.findByRole("combobox", { name: "Search entries…" });
    expect(found6).toBeDefined();
  });

  it("quick-shown resets the query", async () => {
    storeUnlocked = true;
    renderQuick();
    const input = await searchInput();
    fireEvent.change(input, { target: { value: "passport" } });
    await waitFor(() => {
      expect((input as HTMLInputElement).value).toBe("passport");
    });
    act(() => {
      ipc.emitQuickShown();
    });
    await waitFor(() => {
      expect((input as HTMLInputElement).value).toBe("");
    });
  });
});
