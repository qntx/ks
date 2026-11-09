import { open as openDir } from "@tauri-apps/plugin-dialog";
import { KeyRound } from "lucide-react";
import { useEffect, useState } from "react";
import type React from "react";

import { commands } from "../bindings.gen";
import type { Strength } from "../bindings.gen";
import type { Messages } from "../i18n";
import { cloneVaultAt, createVaultAt, openVaultAt, useStore } from "../store";
import { Button, Dialog, Input, Segmented, Switch } from "../ui";
import * as c from "../ui/classes";

function StrengthBar(props: { pass: string; m: Messages }): React.ReactNode {
  const [strength, setStrength] = useState<Strength | undefined>();
  useEffect(() => {
    if (props.pass.length === 0) {
      return undefined;
    }
    const id = setTimeout(() => {
      void (async () => setStrength(await commands.passphraseStrength(props.pass)))();
    }, 300);
    return () => clearTimeout(id);
  }, [props.pass]);
  if (strength === undefined || !("score" in strength)) {
    return undefined;
  }
  return (
    <div aria-live="polite" className="mt-1">
      <div className="flex items-center gap-2">
        <div className="h-1 flex-1 rounded bg-zinc-200 dark:bg-zinc-700">
          <div
            className="h-1 rounded bg-blue-600"
            style={{ width: `${(strength.score / 4) * 100}%` }}
          />
        </div>
        <span className="text-xs text-zinc-500">
          {props.m.strength[Math.min(strength.score, 4)]}
        </span>
      </div>
      {strength.problem !== null && (
        <p className="mt-1 text-xs text-amber-600">{strength.problem}</p>
      )}
    </div>
  );
}

export function Welcome(props: { m: Messages }): React.JSX.Element {
  const { m } = props;
  const { busy } = useStore();
  const [open, setOpen] = useState(false);
  const [tab, setTab] = useState("create");
  const [path, setPath] = useState("~/.ks");
  const [pass, setPass] = useState("");
  const [pass2, setPass2] = useState("");
  const [git, setGit] = useState(true);
  const [url, setUrl] = useState("");
  const [err, setErr] = useState<string | undefined>();

  const submit = async (): Promise<void> => {
    setErr(undefined);
    if (tab !== "open" && pass !== pass2) {
      setErr(m.passMismatch);
      return;
    }
    const ok =
      tab === "create"
        ? await createVaultAt(path, pass, git)
        : tab === "open"
          ? await openVaultAt(path)
          : await cloneVaultAt(url, path, pass);
    if (ok) {
      setOpen(false);
      setPass("");
      setPass2("");
    }
  };

  const pickDir = async (): Promise<void> => {
    const d = await openDir({ directory: true, title: m.openDirTitle });
    if (typeof d === "string") {
      setPath(d);
    }
  };

  return (
    <main className="bg-paper text-ink flex min-h-screen flex-col items-center justify-center p-8">
      <div className={`${c.card} flex w-[22rem] flex-col items-center gap-3 p-8`}>
        <KeyRound size={36} strokeWidth={1.75} className="text-muted" aria-hidden />
        <h1 className={c.displayTitle}>{m.welcome}</h1>
        <p className={c.caption}>{m.welcomeSub}</p>
        <Button onClick={() => setOpen(true)}>{m.createVault}</Button>
      </div>
      <Dialog open={open} onOpenChange={setOpen} title={m.createVault}>
        <Segmented
          value={tab}
          onValueChange={setTab}
          items={[
            { id: "create", label: m.create },
            { id: "open", label: m.open },
            { id: "clone", label: m.clone },
          ]}
        />
        <div className="space-y-3">
          {tab === "clone" && <Input label={m.cloneUrl} value={url} onChange={setUrl} />}
          <div className="flex items-end gap-2">
            <div className="flex-1">
              <Input label={m.vaultPath} value={path} onChange={setPath} />
            </div>
            <Button kind="ghost" onClick={() => void pickDir()}>
              {m.browse}
            </Button>
          </div>
          {tab !== "open" && (
            <>
              <div>
                <Input
                  label={m.passphrase}
                  type="password"
                  autoComplete="off"
                  spellCheck={false}
                  value={pass}
                  onChange={setPass}
                />
                <StrengthBar pass={pass} m={m} />
              </div>
              <Input
                label={m.passphraseAgain}
                type="password"
                autoComplete="off"
                spellCheck={false}
                value={pass2}
                onChange={setPass2}
              />
            </>
          )}
          {tab === "create" && (
            <Switch checked={git} onCheckedChange={setGit} label={m.gitToggle} />
          )}
          {err !== undefined && <p className="text-sm text-red-600">{err}</p>}
          <div className="flex justify-end gap-2">
            <Button kind="ghost" onClick={() => setOpen(false)}>
              {m.cancel}
            </Button>
            <Button disabled={busy} onClick={() => void submit()}>
              {tab === "create" ? m.create : tab === "open" ? m.open : m.clone}
            </Button>
          </div>
        </div>
      </Dialog>
    </main>
  );
}
