import { CircleAlert, CircleCheck, RefreshCw } from "lucide-react";
import type { ReactElement } from "react";

import { useSession } from "../../app/session";
import { errorMessage, useMessages } from "../../i18n";
import type { DoctorKind, DoctorProblem } from "../../ipc/bindings.gen";
import { Button, Spinner } from "../../ui";
import { useDoctor, useDoctorFix } from "./hooks";

const HINTS: Partial<Record<DoctorKind, "rotation" | "undecryptable">> = {
  unfinished_rotation: "rotation",
  undecryptable: "undecryptable",
};

/** Kinds whose `message` carries real error detail worth showing. */
const DETAIL: ReadonlySet<DoctorKind> = new Set(["undecryptable", "unreadable"]);

/**
 * The "Check vault" page inside the settings sheet: localized group titles and hints, the
 * vault-relative path per problem, and Fix when repairable.
 */
export function DoctorPage(): ReactElement {
  const m = useMessages();
  const vaultPath = useSession((s) => s.vault?.path);
  const doctor = useDoctor();
  const fix = useDoctorFix();
  const rel = (p: string): string =>
    vaultPath !== undefined && p.startsWith(`${vaultPath}/`) ? p.slice(vaultPath.length + 1) : p;
  const groups = new Map<DoctorKind, DoctorProblem[]>();
  for (const p of doctor.data?.problems ?? []) {
    const list = groups.get(p.kind) ?? [];
    list.push(p);
    groups.set(p.kind, list);
  }
  const fixable = doctor.data?.problems.some((p) => p.fixable) ?? false;
  return doctor.isPending ? (
    <div className="flex justify-center py-8">
      <Spinner className="size-5" />
    </div>
  ) : doctor.isError ? (
    <div className="flex flex-col gap-3 py-4">
      <p className="text-danger flex items-center gap-2 text-sm">
        <CircleAlert className="size-4 shrink-0" aria-hidden="true" />
        {errorMessage(m, doctor.error)}
      </p>
      <Button
        variant="secondary"
        icon={RefreshCw}
        loading={doctor.isFetching}
        onClick={() => {
          void doctor.refetch();
        }}
      >
        {m.doctor.retry}
      </Button>
    </div>
  ) : doctor.data === undefined || doctor.data.problems.length === 0 ? (
    <div className="text-muted flex items-center gap-2 py-4 text-sm">
      <CircleCheck className="size-4" aria-hidden="true" />
      {m.doctor.clean}
    </div>
  ) : (
    <div className="flex flex-col gap-4">
      {[...groups.entries()].map(([kind, problems]) => (
        <div key={kind}>
          <p className="text-muted flex items-center gap-1.5 pb-1 text-xs">
            <CircleAlert className="text-danger size-3.5" aria-hidden="true" />
            {m.doctor.groups[kind]}
          </p>
          {HINTS[kind] === "rotation" && (
            <p className="text-muted pb-1 text-xs">{m.doctor.hintRotation}</p>
          )}
          {HINTS[kind] === "undecryptable" && (
            <p className="text-muted pb-1 text-xs">{m.doctor.hintUndecryptable}</p>
          )}
          {problems.map((p) => (
            <div key={p.path} className="py-0.5">
              <p className="text-muted truncate font-mono text-xs">{rel(p.path)}</p>
              {DETAIL.has(p.kind) && (
                <p className="text-muted text-xs wrap-break-word">{p.message}</p>
              )}
            </div>
          ))}
        </div>
      ))}
      {fixable && (
        <Button
          onClick={() => {
            fix.mutate();
          }}
          loading={fix.isPending}
        >
          {m.doctor.fix}
        </Button>
      )}
    </div>
  );
}
