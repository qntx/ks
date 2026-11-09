import { useState } from "react";
import type { ReactElement } from "react";

import { useSession } from "../../app/session";
import { useMessages } from "../../i18n";
import type { Settings_Serialize } from "../../ipc/bindings.gen";
import { ListGroup, ListRow, Switch } from "../../ui";
import { useGitStatus } from "./hooks";

/**
 * The "Sync" settings group — three toggle rows saved immediately; hidden entirely for non-git
 * vaults.
 */
export function SyncGroup({
  patch,
}: {
  patch: (p: Partial<Settings_Serialize>) => Promise<void>;
}): ReactElement | undefined {
  const m = useMessages();
  const settings = useSession((s) => s.settings);
  const status = useGitStatus();
  const [saving, setSaving] = useState(false);
  if (status.data === null || status.data === undefined) {
    return undefined;
  }
  const toggle = async (
    field: "syncOnUnlock" | "syncOnLock" | "syncAfterWrite",
    v: boolean,
  ): Promise<void> => {
    setSaving(true);
    try {
      await patch({ [field]: v });
    } finally {
      setSaving(false);
    }
  };
  const row = (
    field: "syncOnUnlock" | "syncOnLock" | "syncAfterWrite",
    label: string,
    desc?: string,
  ): ReactElement => (
    <ListRow label={label} description={desc}>
      <Switch
        checked={settings[field]}
        disabled={saving}
        label={label}
        onCheckedChange={(v) => void toggle(field, v)}
      />
    </ListRow>
  );
  return (
    <ListGroup title={m.settings.sync}>
      {row("syncOnUnlock", m.settings.syncOnUnlock)}
      {row("syncOnLock", m.settings.syncOnLock)}
      {row("syncAfterWrite", m.settings.syncAfterWrite, m.settings.syncAfterWriteDesc)}
    </ListGroup>
  );
}
