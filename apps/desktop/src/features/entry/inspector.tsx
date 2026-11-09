import { open as pickFile, save as pickSave } from "@tauri-apps/plugin-dialog";
import { Ellipsis, Eye, EyeOff, File, KeyRound, Pencil, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { ReactElement } from "react";

import { useMessages, errorMessage } from "../../i18n";
import { commands } from "../../ipc/bindings.gen";
import type { FieldRef } from "../../ipc/bindings.gen";
import { call } from "../../ipc/call";
import { formatBytes, groupDigits } from "../../lib/format";
import { parentPath } from "../../lib/paths";
import { Button, cn, IconButton, ListGroup, Menu, ProgressRing, Spinner, toast } from "../../ui";
import { CopyButton } from "./copy-button";
import { useCopyField, useEntryDetail, useImportFile, useTotp } from "./hooks";

export function Inspector({
  name,
  onClose,
  onEdit,
  onRename,
  onDelete,
}: {
  name: string;
  onClose: () => void;
  onEdit: () => void;
  onRename: () => void;
  onDelete: () => void;
}): ReactElement {
  const m = useMessages();
  const detail = useEntryDetail(name);
  const copyField = useCopyField();
  const importFile = useImportFile();
  const d = detail.data;

  const exportBinary = async (): Promise<void> => {
    const path = await pickSave({ defaultPath: name.split("/").pop() ?? name });
    if (typeof path !== "string") {
      return;
    }
    try {
      await call(commands.exportBinary(name, path));
      toast(m.toasts.exported);
    } catch (error) {
      toast(errorMessage(m, error), { tone: "error" });
    }
  };

  const replaceBinary = async (): Promise<void> => {
    const path = await pickFile();
    if (typeof path !== "string") {
      return;
    }
    try {
      await importFile.mutateAsync({ name, path, overwrite: true });
    } catch (error) {
      toast(errorMessage(m, error), { tone: "error" });
    }
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex justify-end px-3 pt-3">
        <IconButton icon={X} label={m.inspector.close} onClick={onClose} />
      </div>
      {d === undefined ? (
        detail.isError ? (
          <p className="text-muted px-5 text-base">{errorMessage(m, detail.error)}</p>
        ) : (
          <div className="flex justify-center pt-16">
            <Spinner className="text-muted" />
          </div>
        )
      ) : (
        <div className="min-h-0 flex-1 overflow-y-auto px-5 pb-5">
          <div className="bg-surface flex size-14 items-center justify-center rounded-2xl">
            {d.kind === "binary" ? (
              <File className="text-muted size-7" strokeWidth={2} aria-hidden="true" />
            ) : (
              <KeyRound className="text-muted size-7" strokeWidth={2} aria-hidden="true" />
            )}
          </div>
          <h3 className="text-ink pt-3 text-2xl font-bold tracking-tight break-all">{d.name}</h3>
          {parentPath(d.name) !== "" && (
            <p className="text-muted text-base">{parentPath(d.name)}</p>
          )}
          {d.kind === "text" ? (
            <>
              <div className="flex items-center gap-2 pt-4">
                <Button
                  block
                  disabled={(d.value?.length ?? 0) === 0}
                  onClick={() => {
                    void copyField(d.name, "value");
                  }}
                >
                  {m.inspector.copySecret}
                </Button>
                <IconButton
                  icon={Pencil}
                  label={m.inspector.edit}
                  variant="surface"
                  onClick={onEdit}
                />
                <Menu
                  align="end"
                  trigger={
                    <IconButton icon={Ellipsis} label={m.inspector.more} variant="surface" />
                  }
                  items={[
                    { label: m.common.rename, onSelect: onRename },
                    { label: m.common.delete, destructive: true, onSelect: onDelete },
                  ]}
                />
              </div>
              <ListGroup>
                <FieldRow
                  name={d.name}
                  field="value"
                  label={m.inspector.secret}
                  length={d.value?.length ?? 0}
                />
                {d.fields.map((f) => (
                  <FieldRow
                    key={f.key}
                    name={d.name}
                    field={{ field: f.key }}
                    label={f.key}
                    length={f.length}
                  />
                ))}
                {d.notesLength > 0 && (
                  <FieldRow
                    name={d.name}
                    field="notes"
                    label={m.inspector.notes}
                    length={d.notesLength}
                  />
                )}
                {d.hasTotp && <TotpRow name={d.name} />}
              </ListGroup>
            </>
          ) : (
            <div className="pt-4">
              <ListGroup>
                <div className="flex min-h-13 items-center gap-3 px-4 py-2.5">
                  <div className="min-w-0 flex-1">
                    <p className="text-ink text-base">{m.inspector.binary}</p>
                    <p className="text-muted text-xs">{formatBytes(d.binaryLength ?? 0)}</p>
                  </div>
                </div>
              </ListGroup>
              <div className="flex gap-2 pt-4">
                <Button variant="secondary" block onClick={() => void exportBinary()}>
                  {m.inspector.export}
                </Button>
                <Button variant="secondary" block onClick={() => void replaceBinary()}>
                  {m.inspector.replace}
                </Button>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function FieldRow({
  name,
  field,
  label,
  length,
}: {
  name: string;
  field: FieldRef;
  label: string;
  length: number;
}): ReactElement {
  const m = useMessages();
  const [value, setValue] = useState<string | undefined>(undefined);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const revealed = value !== undefined;

  useEffect(
    () => () => {
      clearTimeout(timer.current);
    },
    [],
  );

  const hide = (): void => {
    clearTimeout(timer.current);
    setValue(undefined);
  };

  const reveal = async (): Promise<void> => {
    try {
      setValue(await call(commands.revealField(name, field)));
      clearTimeout(timer.current);
      timer.current = setTimeout(hide, 30_000);
    } catch (error) {
      toast(errorMessage(m, error), { tone: "error" });
    }
  };

  const toggle = (): void => {
    if (revealed) {
      hide();
    } else {
      void reveal();
    }
  };

  return (
    <div className="relative flex min-h-13 items-center gap-2 px-4 py-2.5">
      <div className="min-w-0 flex-1">
        <p className="text-muted text-xs">{label}</p>
        <p
          className={cn(
            "font-mono text-base",
            revealed
              ? "text-ink break-all whitespace-pre-wrap select-text"
              : "text-ink-secondary truncate",
          )}
        >
          {revealed ? value : "•".repeat(Math.min(length, 16))}
        </p>
      </div>
      <IconButton
        icon={revealed ? EyeOff : Eye}
        label={revealed ? m.common.hide : m.common.show}
        size="sm"
        onClick={toggle}
      />
      <CopyButton name={name} field={field} />
    </div>
  );
}

function TotpRow({ name }: { name: string }): ReactElement {
  const m = useMessages();
  const t = useTotp(name);
  const code = t.data?.code ?? "——————";
  return (
    <div className="flex min-h-13 items-center gap-2 px-4 py-2.5">
      <div className="min-w-0 flex-1">
        <p className="text-muted text-xs">{m.inspector.oneTimeCode}</p>
        <p className="font-mono text-3xl tabular-nums">{groupDigits(code)}</p>
      </div>
      {t.data !== undefined && <ProgressRing value={t.data.validFor / 30} size={22} />}
      <CopyButton name={name} field="totp" />
    </div>
  );
}
