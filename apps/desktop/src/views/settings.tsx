import { useState } from "react";
import type React from "react";

import { commands } from "../bindings.gen";
import type { Settings_Deserialize } from "../bindings.gen";
import type { Messages } from "../i18n";
import { setLocale } from "../i18n";
import { toast, useStore } from "../store";
import { Button, Dialog, Input, Switch } from "../ui";

function num(v: string): number {
  const n = Number(v);
  return Number.isFinite(n) && n >= 0 ? n : 0;
}

export function Settings(props: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  m: Messages;
}): React.ReactNode {
  if (!props.open) {
    return undefined;
  }
  return <SettingsForm {...props} />;
}

function SettingsForm(props: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  m: Messages;
}): React.JSX.Element {
  const { m } = props;
  const current = useStore().settings;
  // draft state initialized once per mount (wrapper remounts on open)
  const [s, setS] = useState<Settings_Deserialize>(() => ({ ...current }));

  const save = async (): Promise<void> => {
    const r = await commands.setSettings(s);
    if (r.status === "ok") {
      toast(m.settingsSaved);
      const lang = s.language ?? current.language;
      setLocale(lang === "system" ? "en" : lang === "zh" ? "zh" : "en");
      props.onOpenChange(false);
    }
  };

  return (
    <Dialog open={props.open} onOpenChange={props.onOpenChange} title={m.settingsTitle}>
      <div className="space-y-4">
        <Input
          label={m.autoLock}
          type="number"
          value={String(s.autoLockMinutes ?? current.autoLockMinutes)}
          onChange={(v) => setS({ ...s, autoLockMinutes: num(v) })}
        />
        <Input
          label={m.maxSession}
          type="number"
          value={String(s.maxSessionHours ?? current.maxSessionHours)}
          onChange={(v) => setS({ ...s, maxSessionHours: num(v) })}
        />
        <Input
          label={m.clipSeconds}
          type="number"
          value={String(s.clipboardSeconds ?? current.clipboardSeconds)}
          onChange={(v) => setS({ ...s, clipboardSeconds: num(v) })}
        />
        <Switch
          checked={s.lockOnSleep ?? current.lockOnSleep}
          onCheckedChange={(v) => setS({ ...s, lockOnSleep: v })}
          label={m.lockOnSleep}
        />
        <label className="block text-sm">
          <span className="mb-1 block text-xs text-zinc-500">{m.language}</span>
          <select
            value={s.language ?? current.language}
            onChange={(e) => setS({ ...s, language: e.target.value })}
            className="w-full rounded-md border border-zinc-300 bg-white px-3 py-1.5 text-sm outline-none focus-visible:ring-2 focus-visible:ring-blue-500 dark:border-zinc-700 dark:bg-zinc-900"
          >
            <option value="system">{m.langSystem}</option>
            <option value="en">English</option>
            <option value="zh">中文</option>
          </select>
        </label>
        <div className="flex justify-end gap-2">
          <Button kind="ghost" onClick={() => props.onOpenChange(false)}>
            {m.cancel}
          </Button>
          <Button onClick={() => void save()}>{m.save}</Button>
        </div>
      </div>
    </Dialog>
  );
}
