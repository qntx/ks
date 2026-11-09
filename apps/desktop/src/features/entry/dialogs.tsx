import { useActionState, useState } from "react";
import type { ReactElement } from "react";

import { errorMessage, useMessages } from "../../i18n";
import { IpcError } from "../../ipc/call";
import { Button, ConfirmDialog, Sheet, TextField, toast } from "../../ui";
import { useDeleteEntry, useImportFile, useRenameEntry } from "./hooks";
import { suggestName } from "./name";

export function RenameDialog({
  name,
  onClose,
  onRenamed,
}: {
  name: string;
  onClose: () => void;
  onRenamed: (next: string) => void;
}): ReactElement {
  const m = useMessages();
  const [next, setNext] = useState(name);
  const rename = useRenameEntry();
  const [error, submit, pending] = useActionState(
    async (_prev: string | undefined): Promise<string | undefined> => {
      try {
        await rename.mutateAsync({ from: name, to: next, overwrite: false });
        onRenamed(next);
        return undefined;
      } catch (error_) {
        return errorMessage(m, error_);
      }
    },
    undefined,
  );

  return (
    <Sheet
      open
      onOpenChange={(open) => {
        if (!open) {
          onClose();
        }
      }}
      title={m.renameDialog.title}
      size="sm"
      action={submit}
      footer={
        <>
          <Button variant="secondary" onClick={onClose}>
            {m.common.cancel}
          </Button>
          <Button type="submit" loading={pending} disabled={next === "" || next === name}>
            {m.renameDialog.save}
          </Button>
        </>
      }
    >
      <TextField
        label={m.renameDialog.name}
        value={next}
        error={error}
        // oxlint-disable-next-line jsx-a11y/no-autofocus -- entry point wants immediate focus
        autoFocus
        onChange={(e) => {
          setNext(e.target.value);
        }}
      />
    </Sheet>
  );
}

export function DeleteDialog({
  name,
  onClose,
  onDeleted,
}: {
  name: string;
  onClose: () => void;
  onDeleted: () => void;
}): ReactElement {
  const m = useMessages();
  const del = useDeleteEntry();
  return (
    <ConfirmDialog
      open
      onOpenChange={(open) => {
        if (!open) {
          onClose();
        }
      }}
      title={m.deleteDialog.title}
      message={m.deleteDialog.message(name)}
      confirmLabel={m.deleteDialog.confirm}
      cancelLabel={m.deleteDialog.cancel}
      destructive
      onConfirm={async () => {
        try {
          await del.mutateAsync(name);
          onDeleted();
        } catch (error) {
          toast(errorMessage(m, error), { tone: "error" });
        }
      }}
    />
  );
}

export function ImportDialog({
  path,
  folder,
  onClose,
  onImported,
}: {
  path: string;
  folder: string;
  onClose: () => void;
  onImported: (name: string) => void;
}): ReactElement {
  const m = useMessages();
  const [name, setName] = useState(() => suggestName(folder, path));
  const importFile = useImportFile();
  const [error, submit, pending] = useActionState(
    async (_prev: string | undefined): Promise<string | undefined> => {
      try {
        await importFile.mutateAsync({ name, path, overwrite: false });
        onImported(name);
        return undefined;
      } catch (error_) {
        if (error_ instanceof IpcError && error_.code === "exists") {
          return m.editor.exists;
        }
        toast(errorMessage(m, error_), { tone: "error" });
        onClose();
        return undefined;
      }
    },
    undefined,
  );

  return (
    <Sheet
      open
      onOpenChange={(open) => {
        if (!open) {
          onClose();
        }
      }}
      title={m.importDialog.title}
      description={m.importDialog.description}
      size="sm"
      action={submit}
      footer={
        <>
          <Button variant="secondary" onClick={onClose}>
            {m.common.cancel}
          </Button>
          <Button type="submit" loading={pending} disabled={name === ""}>
            {m.importDialog.submit}
          </Button>
        </>
      }
    >
      <TextField
        label={m.importDialog.name}
        value={name}
        error={error}
        // oxlint-disable-next-line jsx-a11y/no-autofocus -- entry point wants immediate focus
        autoFocus
        onChange={(e) => {
          setName(e.target.value);
        }}
      />
    </Sheet>
  );
}
