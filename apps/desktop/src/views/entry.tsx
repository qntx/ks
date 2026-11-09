import { open as pickSave } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import type React from "react";

import { commands } from "../bindings.gen";
import type { FieldRef, TotpCode } from "../bindings.gen";
import type { Messages } from "../i18n";
import { deleteEntry, renameEntry, toast, useStore } from "../store";
import { Button, Dialog, Input } from "../ui";
import * as c from "../ui/classes";

function Field(props: {
  label: string;
  name: string;
  field: FieldRef;
  length: number;
  m: Messages;
}): React.JSX.Element {
  const [value, setValue] = useState<string | undefined>();

  // Auto-hide 30 s after reveal; switching entries also hides.
  useEffect(() => {
    if (value === undefined) {
      return undefined;
    }
    const id = setTimeout(() => setValue(undefined), 30_000);
    return () => clearTimeout(id);
  }, [value]);
  const reveal = async (): Promise<void> => {
    const r = await commands.revealField(props.name, props.field);
    if (r.status === "ok") {
      setValue(r.data);
    }
  };

  const copy = async (): Promise<void> => {
    const r = await commands.copyField(props.name, props.field);
    if (r.status === "ok") {
      toast(props.m.copiedFor(r.data.clearsInSecs));
    }
  };

  return (
    <div className={c.fieldRow}>
      <span className="w-24 shrink-0 truncate text-xs text-zinc-500">{props.label}</span>
      <span className="flex-1 truncate font-mono text-sm tabular-nums" aria-live="polite">
        {value ?? "•".repeat(Math.min(props.length, 24))}
      </span>
      <Button kind="ghost" onClick={() => void reveal()}>
        {props.m.reveal}
      </Button>
      <Button kind="ghost" onClick={() => void copy()}>
        {props.m.copy}
      </Button>
    </div>
  );
}

function Totp(props: { name: string; m: Messages }): React.ReactNode {
  const [code, setCode] = useState<TotpCode | undefined>();
  useEffect(() => {
    let dead = false;
    const tick = async (): Promise<void> => {
      const r = await commands.totpCode(props.name);
      if (!dead && r.status === "ok") {
        setCode(r.data);
      }
    };
    void tick();
    const t = setInterval(tick, 1000);
    return () => {
      dead = true;
      clearInterval(t);
    };
  }, [props.name]);
  if (code === undefined) {
    return undefined;
  }
  const pct = (code.validFor / 30) * 360;
  return (
    <div className="flex items-center gap-3 rounded-md border border-zinc-200 px-3 py-2 dark:border-zinc-800">
      <span
        className="inline-block h-4 w-4 rounded-full"
        style={{ background: `conic-gradient(#2563eb ${pct}deg, #d4d4d8 ${pct}deg)` }}
        aria-hidden
      />
      <span className="flex-1 font-mono text-lg tracking-widest">{code.code}</span>
      <span className="text-xs text-zinc-500">{code.validFor}s</span>
    </div>
  );
}

export function EntryView(props: {
  m: Messages;
  onEdit: (name: string) => void;
  onSettings: () => void;
}): React.JSX.Element {
  const { m } = props;
  const { detail, selected } = useStore();
  const [confirm, setConfirm] = useState(false);
  const [rename, setRename] = useState<string | undefined>();
  const [newName, setNewName] = useState("");

  if (selected === undefined) {
    return <p className="p-6 text-sm text-zinc-500">{m.noSelection}</p>;
  }
  if (detail === undefined || detail.name !== selected) {
    return <p className="p-6 text-sm text-zinc-500">{m.loading}</p>;
  }
  const d = detail;

  const exportBin = async (): Promise<void> => {
    const p = await pickSave({ title: m.exportTitle, defaultPath: selected });
    if (typeof p !== "string") {
      return;
    }
    await commands.exportBinary(selected, p);
  };

  const doRename = async (): Promise<void> => {
    if (rename === undefined) {
      return;
    }
    if (newName.length > 0 && (await renameEntry(rename, newName))) {
      setRename(undefined);
    }
  };

  return (
    <div className={`${c.card} space-y-3`}>
      <div className="flex items-center gap-2">
        <h2 className={`flex-1 ${c.title}`}>{d.name}</h2>
        <Button kind="ghost" onClick={() => props.onEdit(d.name)}>
          {m.editEntry}
        </Button>
        <Button
          kind="ghost"
          onClick={() => {
            setRename(d.name);
            setNewName(d.name);
          }}
        >
          {m.renameEntry}
        </Button>
        <Button kind="danger" onClick={() => setConfirm(true)}>
          {m.deleteEntry}
        </Button>
      </div>
      {d.kind === "text" && d.value !== null && (
        <Field
          key={`${d.name}/value`}
          label={m.fieldValue}
          name={d.name}
          field="value"
          length={d.value.length}
          m={m}
        />
      )}
      {d.fields.map((f) => (
        <Field
          key={`${d.name}/${f.key}`}
          label={f.key}
          name={d.name}
          field={{ field: f.key }}
          length={f.length}
          m={m}
        />
      ))}
      {d.notesLength > 0 && (
        <Field
          key={`${d.name}/notes`}
          label={m.fieldNotes}
          name={d.name}
          field="notes"
          length={d.notesLength}
          m={m}
        />
      )}
      {d.hasTotp && <Totp name={d.name} m={m} />}
      {d.kind === "binary" && (
        <div className={c.fieldRow}>
          <span className="flex-1 text-sm text-zinc-500">{m.binarySize(d.binaryLength ?? 0)}</span>
          <Button kind="ghost" onClick={() => void exportBin()}>
            {m.exportBinary}
          </Button>
        </div>
      )}
      <Dialog open={confirm} onOpenChange={setConfirm} title={m.deleteEntry}>
        <p className="mb-4 text-sm">{m.deleteConfirm(d.name)}</p>
        <div className="flex justify-end gap-2">
          <Button kind="ghost" onClick={() => setConfirm(false)}>
            {m.cancel}
          </Button>
          <Button
            kind="danger"
            onClick={() => {
              void (async () => {
                if (await deleteEntry(d.name)) {
                  setConfirm(false);
                  toast(m.deletedOk);
                }
              })();
            }}
          >
            {m.deleteYes}
          </Button>
        </div>
      </Dialog>
      <Dialog
        open={rename !== undefined}
        onOpenChange={(o) => {
          if (!o) {
            setRename(undefined);
          }
        }}
        title={m.renamePrompt}
      >
        <div className="space-y-3">
          <Input label={m.editorName} value={newName} onChange={setNewName} />
          <div className="flex justify-end gap-2">
            <Button kind="ghost" onClick={() => setRename(undefined)}>
              {m.cancel}
            </Button>
            <Button onClick={() => void doRename()}>{m.save}</Button>
          </div>
        </div>
      </Dialog>
    </div>
  );
}
