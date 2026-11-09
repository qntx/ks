import { Plus, Sparkles, X } from "lucide-react";
import { useActionState, useEffect, useEffectEvent, useState } from "react";
import type { ReactElement } from "react";

import { errorMessage, useMessages } from "../../i18n";
import { commands } from "../../ipc/bindings.gen";
import { call, IpcError } from "../../ipc/call";
import { Button, cn, IconButton, PasswordField, Sheet, TextArea, TextField, toast } from "../../ui";
import { GeneratorPopover } from "../generator";
import { newFieldId, parseEntryText, serializeDraft, validateDraft } from "./entry-text";
import type { EntryDraft } from "./entry-text";
import { useSaveEntry } from "./hooks";

export type EditorTarget = { mode: "new"; folder: string } | { mode: "edit"; name: string };

const CHIPS = ["user", "url", "totp", "notes"] as const;

type FormErrors = { name?: string; form?: string };

export function EntryEditor({
  target,
  onClose,
  onSaved,
}: {
  target: EditorTarget;
  onClose: () => void;
  onSaved: (name: string) => void;
}): ReactElement {
  const m = useMessages();
  const isEdit = target.mode === "edit";
  const [name, setName] = useState(
    isEdit ? target.name : target.folder === "" ? "" : `${target.folder}/`,
  );
  const [draft, setDraft] = useState<EntryDraft>({ value: "", fields: [], extra: "" });
  const [loaded, setLoaded] = useState(false);
  const save = useSaveEntry();

  const reportLoadError = useEffectEvent((error: unknown): void => {
    toast(errorMessage(m, error), { tone: "error" });
    onClose();
  });

  useEffect(() => {
    if (target.mode !== "edit") {
      return undefined;
    }
    let ignore = false;
    const load = async (): Promise<void> => {
      try {
        const text = await call(commands.editEntry(target.name));
        if (!ignore) {
          setDraft(parseEntryText(text));
          setLoaded(true);
        }
      } catch (error) {
        if (!ignore) {
          reportLoadError(error);
        }
      }
    };
    void load();
    return () => {
      ignore = true;
    };
  }, [target.mode, target]);

  const [errors, submit, pending] = useActionState(
    async (_prev: FormErrors | undefined): Promise<FormErrors | undefined> => {
      if (!isEdit && name.trim() === "") {
        return { name: m.editor.errors.empty };
      }
      const err = validateDraft(draft);
      if (err !== undefined) {
        return { form: m.editor.errors[err] };
      }
      try {
        await save.mutateAsync({
          name: name.trim(),
          text: serializeDraft(draft),
          overwrite: isEdit,
        });
        onSaved(name.trim());
        return undefined;
      } catch (error) {
        if (error instanceof IpcError && error.code === "exists") {
          return { name: m.editor.exists };
        }
        toast(errorMessage(m, error), { tone: "error" });
        return undefined;
      }
    },
    undefined,
  );

  const setField = (id: string, patch: Partial<EntryDraft["fields"][number]>): void => {
    setDraft((d) => ({
      ...d,
      fields: d.fields.map((f) => (f.id === id ? { ...f, ...patch } : f)),
    }));
  };

  const addField = (key = ""): void => {
    setDraft((d) => ({ ...d, fields: [...d.fields, { id: newFieldId(), key, value: "" }] }));
  };

  const removeField = (id: string): void => {
    setDraft((d) => ({ ...d, fields: d.fields.filter((f) => f.id !== id) }));
  };

  const existingKeys = new Set(draft.fields.map((f) => f.key));
  const visibleChips = CHIPS.filter((key) => !existingKeys.has(key));

  return (
    <Sheet
      open
      onOpenChange={(open) => {
        if (!open) {
          onClose();
        }
      }}
      title={isEdit ? m.editor.editTitle : m.editor.newTitle}
      size="md"
      action={submit}
      footer={
        <>
          <Button variant="secondary" onClick={onClose}>
            {m.editor.cancel}
          </Button>
          <Button type="submit" size="lg" loading={pending || (isEdit && !loaded)}>
            {m.editor.save}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-4">
        {isEdit ? (
          <div>
            <p className="text-muted text-xs">{m.editor.name}</p>
            <p className="text-ink font-mono text-base break-all">{target.name}</p>
          </div>
        ) : (
          <TextField
            label={m.editor.name}
            value={name}
            error={errors?.name}
            hint={m.editor.nameHint}
            // oxlint-disable-next-line jsx-a11y/no-autofocus -- new entries start at the name
            autoFocus
            onFocus={(e) => {
              e.target.setSelectionRange(e.target.value.length, e.target.value.length);
            }}
            onChange={(e) => {
              setName(e.target.value);
            }}
          />
        )}
        <PasswordField
          label={m.editor.secret}
          value={draft.value}
          // oxlint-disable-next-line jsx-a11y/no-autofocus -- editing starts at the secret
          autoFocus={isEdit}
          onChange={(e) => {
            setDraft((d) => ({ ...d, value: e.target.value }));
          }}
          trailing={
            <GeneratorPopover
              onUse={(v) => {
                setDraft((d) => ({ ...d, value: v }));
              }}
              trigger={
                <IconButton icon={Sparkles} label={m.editor.generate} size="sm" variant="ghost" />
              }
            />
          }
        />
        {draft.fields.map((f) => (
          <div key={f.id} className="flex items-start gap-2">
            <div className="w-36 shrink-0">
              <TextField
                label={m.editor.fieldKey}
                value={f.key}
                onChange={(e) => {
                  setField(f.id, { key: e.target.value });
                }}
              />
            </div>
            <div className="min-w-0 flex-1">
              <PasswordField
                label={m.editor.fieldValue}
                value={f.value}
                onChange={(e) => {
                  setField(f.id, { value: e.target.value });
                }}
              />
            </div>
            <IconButton
              icon={X}
              label={m.editor.removeField}
              size="sm"
              className="mt-3.5"
              onClick={() => {
                removeField(f.id);
              }}
            />
          </div>
        ))}
        <div className="flex flex-wrap items-center gap-1.5">
          {visibleChips.map((key) => (
            <button
              key={key}
              type="button"
              className={cn(
                "bg-surface hover:bg-hover text-ink-secondary h-8 rounded-full px-3 text-sm",
              )}
              onClick={() => {
                addField(key);
              }}
            >
              {m.editor.chips[key]}
            </button>
          ))}
          <Button
            variant="ghost"
            size="sm"
            icon={Plus}
            onClick={() => {
              addField();
            }}
          >
            {m.editor.addField}
          </Button>
        </div>
        {draft.extra !== "" && (
          <TextArea
            label={m.editor.otherText}
            value={draft.extra}
            onChange={(e) => {
              setDraft((d) => ({ ...d, extra: e.target.value }));
            }}
          />
        )}
        {errors?.form !== undefined && <p className="text-danger text-xs">{errors.form}</p>}
      </div>
    </Sheet>
  );
}
