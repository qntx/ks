import type { ReactElement } from "react";

import { useMessages } from "../../i18n";
import type { DeviceState, Devices } from "../../ipc/bindings.gen";
import { ListGroup, ListRow, Switch } from "../../ui";
import { useDevices, useSetDevice } from "./hooks";

function rowDescription(supported: boolean, reason: string | undefined, desc: string): string {
  return supported ? desc : (reason ?? desc);
}

/** The "Device unlock" settings group: one switch per device source. */
export function DeviceUnlockGroup(): ReactElement | undefined {
  const m = useMessages();
  const devices = useDevices();
  const setDevice = useSetDevice();

  if (devices.data === undefined) {
    return undefined;
  }
  const d: Devices = devices.data;
  const row = (
    source: DeviceState,
    kind: "touch_id" | "keychain",
    label: string,
    desc: string,
  ): ReactElement => (
    <ListRow
      label={label}
      description={rowDescription(source.supported, source.reason ?? undefined, desc)}
    >
      <Switch
        checked={source.enabled}
        disabled={!source.supported || setDevice.isPending}
        label={label}
        onCheckedChange={(enabled) => {
          setDevice.mutate({ kind, enabled });
        }}
      />
    </ListRow>
  );

  return (
    <ListGroup title={m.settings.deviceUnlock}>
      {row(d.touchId, "touch_id", m.settings.touchId, m.settings.touchIdDesc)}
      {row(d.keychain, "keychain", m.settings.keychain, m.settings.keychainDesc)}
    </ListGroup>
  );
}
