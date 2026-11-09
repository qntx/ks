import { open as pickDialog } from "@tauri-apps/plugin-dialog";
import { FolderLock, GitBranch } from "lucide-react";
import { useActionState, useEffect, useState } from "react";
import type { ReactElement } from "react";

import { cloneVault, createVault } from "../../app/session";
import { errorMessage, useMessages } from "../../i18n";
import { commands } from "../../ipc/bindings.gen";
import { Button, PasswordField, Switch, TextField } from "../../ui";
import { CardHeader } from "./stage";
import { StrengthMeter } from "./strength-meter";

async function pickDir(): Promise<string | undefined> {
  const dir = await pickDialog({ directory: true });
  return typeof dir === "string" ? dir : undefined;
}

function LocationField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
}): ReactElement {
  const m = useMessages();
  const browse = async (): Promise<void> => {
    const dir = await pickDir();
    if (dir !== undefined) {
      onChange(dir);
    }
  };
  return (
    <TextField
      label={label}
      name="location"
      value={value}
      onChange={(e) => {
        onChange(e.target.value);
      }}
      trailing={
        <Button
          variant="secondary"
          size="sm"
          onClick={() => {
            void browse();
          }}
        >
          {m.common.browse}
        </Button>
      }
    />
  );
}

export function CreateVaultCard(): ReactElement {
  const m = useMessages();
  const [location, setLocation] = useState("");
  const [pass, setPass] = useState("");
  const [repeat, setRepeat] = useState("");
  const [git, setGit] = useState(true);

  useEffect(() => {
    let ignore = false;
    const prefill = async (): Promise<void> => {
      const paths = await commands.recentVaults();
      if (!ignore && paths[0] !== undefined) {
        setLocation(paths[0]);
      }
    };
    void prefill();
    return () => {
      ignore = true;
    };
  }, []);

  const [error, submit, pending] = useActionState(async () => {
    const p = pass;
    setPass("");
    setRepeat("");
    try {
      await createVault(location, p, git);
      return undefined;
    } catch (error_) {
      return errorMessage(m, error_);
    }
  }, undefined);

  const mismatch = repeat !== "" && pass !== repeat;
  const canSubmit = location !== "" && pass !== "" && pass === repeat;

  return (
    <>
      <CardHeader icon={FolderLock} title={m.create.title} description={m.create.description} />
      <form action={submit} className="flex flex-col gap-4">
        <LocationField label={m.create.location} value={location} onChange={setLocation} />
        <div>
          <PasswordField
            label={m.create.passphrase}
            name="passphrase"
            value={pass}
            onChange={(e) => {
              setPass(e.target.value);
            }}
          />
          <StrengthMeter passphrase={pass} />
        </div>
        <PasswordField
          label={m.create.repeat}
          name="repeat"
          value={repeat}
          error={mismatch ? m.create.mismatch : undefined}
          onChange={(e) => {
            setRepeat(e.target.value);
          }}
        />
        <label className="flex h-11 cursor-default items-center justify-between px-1">
          <span className="text-ink text-base">{m.create.syncGit}</span>
          <Switch checked={git} onCheckedChange={setGit} label={m.create.syncGit} />
        </label>
        {error !== undefined && <p className="text-danger px-1 text-xs">{error}</p>}
        <Button
          type="submit"
          variant="pill-strong"
          size="xl"
          block
          loading={pending}
          disabled={!canSubmit}
        >
          {m.create.submit}
        </Button>
      </form>
    </>
  );
}

export function CloneVaultCard(): ReactElement {
  const m = useMessages();
  const [url, setUrl] = useState("");
  const [location, setLocation] = useState("");
  const [pass, setPass] = useState("");

  useEffect(() => {
    let ignore = false;
    const prefill = async (): Promise<void> => {
      const paths = await commands.recentVaults();
      if (!ignore && paths[0] !== undefined) {
        setLocation(paths[0]);
      }
    };
    void prefill();
    return () => {
      ignore = true;
    };
  }, []);

  const [error, submit, pending] = useActionState(async () => {
    const p = pass;
    setPass("");
    try {
      await cloneVault(url, location, p);
      return undefined;
    } catch (error_) {
      return errorMessage(m, error_);
    }
  }, undefined);

  const canSubmit = url !== "" && location !== "" && pass !== "";

  return (
    <>
      <CardHeader icon={GitBranch} title={m.clone.title} description={m.clone.description} />
      <form action={submit} className="flex flex-col gap-4">
        <TextField
          label={m.clone.url}
          name="url"
          value={url}
          onChange={(e) => {
            setUrl(e.target.value);
          }}
        />
        <LocationField label={m.clone.location} value={location} onChange={setLocation} />
        <PasswordField
          label={m.clone.passphrase}
          name="passphrase"
          value={pass}
          onChange={(e) => {
            setPass(e.target.value);
          }}
        />
        {error !== undefined && <p className="text-danger px-1 text-xs">{error}</p>}
        <Button
          type="submit"
          variant="pill-strong"
          size="xl"
          block
          loading={pending}
          disabled={!canSubmit}
        >
          {m.clone.submit}
        </Button>
      </form>
    </>
  );
}
