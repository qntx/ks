import { open as pickFile } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState } from "react";
import type React from "react";

import { commands } from "../bindings.gen";
import type { Messages } from "../i18n";
import { saveEntry, toast } from "../store";
import { Button, Dialog, Input } from "../ui";
import { Generator } from "./generator";

export function Editor(props: {
  m: Messages;
  name: string | undefined;
  onClose: () => void;
}): React.JSX.Element {
  const { m } = props;
  const [name, setName] = useState(props.name ?? "");
  const [text, setText] = useState("");
  const [binary, setBinary] = useState(false);
  const [gen, setGen] = useState(false);
  const [loaded, setLoaded] = useState(props.name === undefined);
  const entryName = props.name;
  const area = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (entryName === undefined) {
      return undefined;
    }
    let dead = false;
    void (async () => {
      const d = await commands.entryDetail(entryName);
      if (dead || d.status !== "ok") {
        return;
      }
      if (d.data.kind === "binary") {
        setBinary(true);
        setLoaded(true);
        return;
      }
      const t = await commands.editEntry(entryName);
      if (!dead && t.status === "ok") {
        setText(t.data);
        setLoaded(true);
      }
    })();
    return () => {
      dead = true;
    };
  }, [entryName]);

  const insert = (v: string): void => {
    const el = area.current;
    if (el === null) {
      setText((t) => (t.length === 0 ? v : `${t}\n${v}`));
      return;
    }
    const at = el.selectionStart;
    setText((t) => `${t.slice(0, at)}${v}${t.slice(at)}`);
  };

  const doImport = async (): Promise<void> => {
    const p = await pickFile({ title: m.importTitle });
    if (typeof p !== "string" || name.length === 0) {
      return;
    }
    const r = await commands.importBinary(name, p, true);
    if (r.status === "ok") {
      toast(m.savedOk);
      props.onClose();
    }
  };

  const save = async (): Promise<void> => {
    if (name.length === 0) {
      return;
    }
    if (await saveEntry(name, text, props.name !== undefined)) {
      toast(m.savedOk);
      setText("");
      props.onClose();
    }
  };

  if (!loaded) {
    return <p className="p-6 text-sm text-zinc-500">{m.loading}</p>;
  }

  return (
    <div className="flex h-full flex-col gap-3">
      <h2 className="text-lg font-semibold">
        {props.name === undefined ? m.editorTitleNew : m.editorTitleEdit}
      </h2>
      <Input label={m.editorName} value={name} onChange={setName} />
      {binary ? (
        <div className="space-y-3">
          <p className="text-sm text-zinc-500">{m.importBinary}</p>
          <Button kind="ghost" onClick={() => void doImport()}>
            {m.importBinary}
          </Button>
        </div>
      ) : (
        <>
          <label className="flex min-h-0 flex-1 flex-col">
            <span className="mb-1 text-xs text-zinc-500">{m.editorText}</span>
            <textarea
              ref={area}
              value={text}
              onChange={(e) => setText(e.target.value)}
              spellCheck={false}
              aria-label={m.editorText}
              className="min-h-48 flex-1 resize-none rounded-md border border-zinc-300 bg-white p-3 font-mono text-sm outline-none focus-visible:ring-2 focus-visible:ring-blue-500 dark:border-zinc-700 dark:bg-zinc-900"
            />
          </label>
          <p className="text-xs text-zinc-500">{m.editorHint}</p>
          <div className="flex items-center gap-2">
            <Button kind="ghost" onClick={() => setGen(true)}>
              {m.generate}
            </Button>
            <div className="flex-1" />
            <Button kind="ghost" onClick={props.onClose}>
              {m.cancel}
            </Button>
            <Button onClick={() => void save()}>{m.save}</Button>
          </div>
        </>
      )}
      <Dialog open={gen} onOpenChange={setGen} title={m.generate}>
        <Generator m={m} onInsert={insert} />
      </Dialog>
    </div>
  );
}
