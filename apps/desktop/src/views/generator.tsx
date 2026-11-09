import { useState } from "react";
import type React from "react";

import { commands } from "../bindings.gen";
import type { Messages } from "../i18n";
import { Button, Segmented } from "../ui";

const CHARSETS = ["alphanum", "hex", "printable", "slug"] as const;

export function Generator(props: {
  m: Messages;
  onInsert: (text: string) => void;
}): React.JSX.Element {
  const { m } = props;
  const [length, setLength] = useState(24);
  const [charset, setCharset] = useState<string>("alphanum");
  const [out, setOut] = useState<string | undefined>();

  const gen = async (): Promise<void> => {
    const r = await commands.generatePassword(length, charset);
    if (r.status === "ok") {
      setOut(r.data);
    }
  };

  return (
    <div className="space-y-3">
      <Segmented
        value={charset}
        onValueChange={setCharset}
        items={CHARSETS.map((c) => ({ id: c, label: c }))}
      />
      <div className="space-y-3">
        <label className="block text-xs text-zinc-500">
          {m.genLength}: {length}
          <input
            type="range"
            min={8}
            max={128}
            value={length}
            aria-label={m.genLength}
            onChange={(e) => setLength(Number(e.target.value))}
            className="mt-1 w-full"
          />
        </label>
        <Button onClick={() => void gen()}>{m.generate}</Button>
        {out !== undefined && (
          <div className="flex items-center gap-2 rounded-md border border-zinc-200 px-3 py-2 dark:border-zinc-800">
            <span className="flex-1 truncate font-mono text-sm" aria-live="polite">
              {out}
            </span>
            <Button
              kind="ghost"
              onClick={() => {
                props.onInsert(out);
                setOut(undefined);
              }}
            >
              {m.genInsert}
            </Button>
          </div>
        )}
      </div>
    </div>
  );
}
