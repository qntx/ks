import type { ReactElement } from "react";

import { useMessages } from "../../i18n";
import { ListGroup, ListRow } from "../../ui";

/** Settings "Maintenance" group — the row navigates to the doctor page. */
export type MaintenancePage = "doctor" | "passphrase" | "rotate" | "recovery";

export function MaintenanceGroup({
  onOpenPage,
}: {
  onOpenPage: (p: MaintenancePage) => void;
}): ReactElement {
  const m = useMessages();
  return (
    <ListGroup title={m.settings.maintenance}>
      <ListRow
        label={m.settings.checkVault}
        onClick={() => {
          onOpenPage("doctor");
        }}
      />
      <ListRow
        label={m.maintenance.changePassphrase}
        onClick={() => {
          onOpenPage("passphrase");
        }}
      />
      <ListRow
        label={m.maintenance.rotateKey}
        onClick={() => {
          onOpenPage("rotate");
        }}
      />
      <ListRow
        label={m.maintenance.recovery}
        onClick={() => {
          onOpenPage("recovery");
        }}
      />
    </ListGroup>
  );
}
