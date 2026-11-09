import { open as pickDir } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import type { ReactElement } from "react";

import { lock, openVault, saveSettings, useSession } from "../../app/session";
import { errorMessage, useMessages } from "../../i18n";
import { ChoiceMenu, ListGroup, ListRow, Sheet, Switch, toast } from "../../ui";
import { DeviceUnlockGroup } from "../devices";
import {
  DoctorPage,
  MaintenanceGroup,
  PassphrasePage,
  RecoveryPage,
  RotatePage,
} from "../maintenance";
import type { MaintenancePage } from "../maintenance";
import { SyncGroup } from "../sync";

function options(
  presets: number[],
  current: number,
  label: (n: number) => string,
): Array<{ value: string; label: string }> {
  return [...new Set([...presets, current])]
    .toSorted((a, b) => a - b)
    .map((n) => ({ value: String(n), label: label(n) }));
}

/** In-sheet pages: the maintenance group navigates to sub-pages. */
type SettingsPage = "main" | MaintenancePage;

export function SettingsDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}): ReactElement {
  const m = useMessages();
  const settings = useSession((s) => s.settings);
  const vault = useSession((s) => s.vault);
  const [page, setPage] = useState<SettingsPage>("main");

  const patch = async (p: Parameters<typeof saveSettings>[0]): Promise<void> => {
    try {
      await saveSettings(p);
    } catch (error) {
      toast(errorMessage(m, error), { tone: "error" });
    }
  };

  const openAnother = async (): Promise<void> => {
    const dir = await pickDir({ directory: true });
    if (typeof dir !== "string") {
      return;
    }
    try {
      await lock();
      await openVault(dir);
    } catch (error) {
      toast(errorMessage(m, error), { tone: "error" });
    }
  };

  return (
    <Sheet
      open={open}
      onOpenChange={(o) => {
        setPage("main");
        onOpenChange(o);
      }}
      title={
        page === "doctor"
          ? m.settings.checkVault
          : page === "passphrase"
            ? m.maintenance.changePassphrase
            : page === "rotate"
              ? m.maintenance.rotateKey
              : page === "recovery"
                ? m.maintenance.recovery
                : m.settings.title
      }
      back={
        page === "main"
          ? undefined
          : {
              label: m.settings.title,
              onClick: () => {
                setPage("main");
              },
            }
      }
      onEscapeKeyDown={(e) => {
        if (page !== "main") {
          e.preventDefault();
          setPage("main");
        }
      }}
    >
      {page === "doctor" ? (
        <DoctorPage />
      ) : page === "passphrase" ? (
        <PassphrasePage
          onDone={() => {
            setPage("main");
          }}
        />
      ) : page === "rotate" ? (
        <RotatePage
          onDone={() => {
            setPage("main");
          }}
        />
      ) : page === "recovery" ? (
        <RecoveryPage />
      ) : (
        <>
          <ListGroup title={m.settings.security}>
            <ListRow label={m.settings.autoLock}>
              <ChoiceMenu
                label={m.settings.autoLock}
                value={String(settings.autoLockMinutes)}
                options={options([0, 1, 5, 15, 30, 60], settings.autoLockMinutes, (n) =>
                  n === 0 ? m.settings.never : m.settings.minutes(n),
                )}
                onChange={(v) => {
                  void patch({ autoLockMinutes: Number(v) });
                }}
              />
            </ListRow>
            <ListRow label={m.settings.sessionLimit}>
              <ChoiceMenu
                label={m.settings.sessionLimit}
                value={String(settings.maxSessionHours)}
                options={options([0, 1, 4, 8, 12, 24], settings.maxSessionHours, (n) =>
                  n === 0 ? m.settings.never : m.settings.hours(n),
                )}
                onChange={(v) => {
                  void patch({ maxSessionHours: Number(v) });
                }}
              />
            </ListRow>
            <ListRow label={m.settings.clearClipboard}>
              <ChoiceMenu
                label={m.settings.clearClipboard}
                value={String(settings.clipboardSeconds)}
                options={options([10, 20, 30, 45, 60, 90], settings.clipboardSeconds, (n) =>
                  m.settings.seconds(n),
                )}
                onChange={(v) => {
                  void patch({ clipboardSeconds: Number(v) });
                }}
              />
            </ListRow>
            <ListRow label={m.settings.lockOnSleep}>
              <Switch
                checked={settings.lockOnSleep}
                label={m.settings.lockOnSleep}
                onCheckedChange={(v) => {
                  void patch({ lockOnSleep: v });
                }}
              />
            </ListRow>
          </ListGroup>
          <DeviceUnlockGroup />
          <SyncGroup patch={patch} />
          <MaintenanceGroup
            onOpenPage={(p) => {
              setPage(p);
            }}
          />
          <ListGroup title={m.settings.general}>
            <ListRow label={m.settings.language}>
              <ChoiceMenu
                label={m.settings.language}
                value={settings.language}
                options={(
                  [
                    ["system", m.settings.languages.system],
                    ["en", m.settings.languages.en],
                    ["zh", m.settings.languages.zh],
                  ] as const
                ).map(([value, label]) => ({ value, label }))}
                onChange={(v) => {
                  void patch({ language: v });
                }}
              />
            </ListRow>
          </ListGroup>
          <ListGroup title={m.settings.vault}>
            <ListRow label={m.settings.vault} description={vault?.path} />
            <ListRow label={m.settings.openAnother} onClick={() => void openAnother()} />
          </ListGroup>
        </>
      )}
    </Sheet>
  );
}
