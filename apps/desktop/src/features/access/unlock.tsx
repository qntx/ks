import { open as pickDialog } from "@tauri-apps/plugin-dialog";
import { Fingerprint, HardDrive } from "lucide-react";
import { useActionState, useEffect, useEffectEvent, useRef, useState } from "react";
import type { ReactElement } from "react";

import { openVault, unlock, unlockDevice, useSession } from "../../app/session";
import { errorMessage, useMessages } from "../../i18n";
import { Button, PasswordField, toast } from "../../ui";
import { useDevices } from "../devices";
import { AuthStage, GiantTitle, StageTopBar } from "./stage";

export function Unlock(): ReactElement {
  const m = useMessages();
  const vault = useSession((s) => s.vault);
  const lockReason = useSession((s) => s.lockReason);
  const deviceStale = useSession((s) => s.deviceStale);
  const [pass, setPass] = useState("");
  const devices = useDevices();
  const attempted = useRef(false);
  const [devicePending, setDevicePending] = useState(false);

  const touchId = devices.data?.touchId;

  const attemptDevice = async (): Promise<void> => {
    try {
      await unlockDevice();
    } catch (error_) {
      toast(errorMessage(m, error_), { tone: "error" });
    }
  };
  const attemptDeviceOnMount = useEffectEvent(attemptDevice);

  const tryDevice = async (): Promise<void> => {
    setDevicePending(true);
    try {
      await attemptDevice();
    } finally {
      setDevicePending(false);
    }
  };

  // One automatic device attempt per mount when any source is enabled.
  useEffect(() => {
    if (attempted.current || devices.data === undefined) {
      return;
    }
    if (devices.data.touchId.enabled || devices.data.keychain.enabled) {
      attempted.current = true;
      void attemptDeviceOnMount();
    }
  }, [devices.data]);

  const [error, submit, pending] = useActionState(async () => {
    const p = pass;
    setPass("");
    try {
      await unlock(p);
      return undefined;
    } catch (error_) {
      return errorMessage(m, error_);
    }
  }, undefined);

  const openAnother = async (): Promise<void> => {
    const dir = await pickDialog({ directory: true });
    if (typeof dir !== "string") {
      return;
    }
    try {
      await openVault(dir);
    } catch (error_) {
      toast(errorMessage(m, error_), { tone: "error" });
    }
  };

  const reason = lockReason === undefined ? "" : m.lockReason[lockReason];

  return (
    <AuthStage>
      <StageTopBar />
      <GiantTitle lines={[m.unlock.title]} />
      {vault !== undefined && (
        <div className="bg-card mt-5 flex h-10 items-center gap-2 self-start rounded-full px-4">
          <HardDrive className="text-muted size-4 shrink-0" strokeWidth={2} aria-hidden="true" />
          <span className="text-ink-secondary max-w-72 truncate text-base">{vault.path}</span>
        </div>
      )}
      {reason !== "" && <p className="text-muted mt-2 text-base">{reason}</p>}
      {deviceStale && <p className="text-muted mt-2 text-base">{m.unlock.deviceStale}</p>}
      <div className="flex-1" />
      <form action={submit} className="flex flex-col gap-3">
        {error !== undefined && <p className="text-danger px-4 text-xs">{error}</p>}
        {touchId?.enabled === true && (
          <Button
            type="button"
            variant="secondary"
            icon={Fingerprint}
            loading={devicePending}
            onClick={() => {
              void tryDevice();
            }}
          >
            {m.unlock.touchId}
          </Button>
        )}
        <PasswordField
          label={m.unlock.passphrase}
          name="passphrase"
          value={pass}
          // oxlint-disable-next-line jsx-a11y/no-autofocus -- entry point wants immediate focus
          autoFocus
          onChange={(e) => {
            setPass(e.target.value);
          }}
        />
        <Button
          type="submit"
          variant="pill-strong"
          size="xl"
          block
          loading={pending}
          disabled={pass === ""}
        >
          {m.unlock.submit}
        </Button>
      </form>
      <div className="flex justify-center pt-3">
        <Button variant="ghost" size="sm" onClick={() => void openAnother()}>
          {m.unlock.openAnother}
        </Button>
      </div>
    </AuthStage>
  );
}
