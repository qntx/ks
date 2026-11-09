import type { ReactElement } from "react";

import { useMessages } from "../../i18n";
import { ListGroup, ListRow } from "../../ui";

/** Settings "Maintenance" group — the row navigates to the doctor page. */
export function MaintenanceGroup({ onOpenDoctor }: { onOpenDoctor: () => void }): ReactElement {
  const m = useMessages();
  return (
    <ListGroup title={m.settings.maintenance}>
      <ListRow label={m.settings.checkVault} onClick={onOpenDoctor} />
    </ListGroup>
  );
}
