import { RefreshCw } from "lucide-react";
import { useEffect, useState } from "react";
import type { ReactElement, ReactNode } from "react";

import { useMessages } from "../../i18n";
import { commands } from "../../ipc/bindings.gen";
import { call } from "../../ipc/call";
import { Button, IconButton, Popover, Segmented, Slider } from "../../ui";

const CHARSETS = ["alphanum", "printable", "hex", "slug"] as const;

export function GeneratorPopover({
  trigger,
  onUse,
}: {
  trigger: ReactNode;
  onUse: (value: string) => void;
}): ReactElement {
  const m = useMessages();
  const [open, setOpen] = useState(false);
  const [charset, setCharset] = useState<string>("alphanum");
  const [length, setLength] = useState(24);
  const [preview, setPreview] = useState("");

  const generate = async (): Promise<void> => {
    setPreview(await call(commands.generatePassword(length, charset)));
  };

  useEffect(() => {
    if (!open) {
      return undefined;
    }
    let ignore = false;
    const load = async (): Promise<void> => {
      const value = await call(commands.generatePassword(length, charset));
      if (!ignore) {
        setPreview(value);
      }
    };
    void load();
    return () => {
      ignore = true;
    };
  }, [open, length, charset]);

  return (
    <Popover open={open} onOpenChange={setOpen} trigger={trigger}>
      <p className="text-ink pb-3 text-base font-semibold">{m.generator.title}</p>
      <Segmented
        label={m.generator.charsetLabel}
        value={charset}
        options={CHARSETS.map((c) => ({ value: c, label: m.generator.charsets[c] }))}
        onChange={setCharset}
      />
      <div className="pt-3">
        <div className="flex items-baseline justify-between pb-1">
          <span className="text-muted text-xs">{m.generator.length}</span>
          <span className="text-ink text-xs tabular-nums">{length}</span>
        </div>
        <Slider value={length} min={8} max={128} onChange={setLength} label={m.generator.length} />
      </div>
      <div className="bg-surface mt-3 flex items-start gap-2 rounded-2xl p-3">
        <p className="min-w-0 flex-1 font-mono text-base break-all select-text">{preview}</p>
        <IconButton
          icon={RefreshCw}
          label={m.generator.regenerate}
          size="sm"
          onClick={() => {
            void generate();
          }}
        />
      </div>
      <Button
        block
        className="mt-3"
        onClick={() => {
          onUse(preview);
          setOpen(false);
        }}
      >
        {m.generator.use}
      </Button>
    </Popover>
  );
}
