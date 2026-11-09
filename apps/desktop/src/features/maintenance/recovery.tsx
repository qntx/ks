import type { ReactElement } from "react";

import { useSession } from "../../app/session";
import { useMessages } from "../../i18n";

/** Single-quote a shell argument when it contains spaces or metacharacters. */
function shellQuote(s: string): string {
  return /^[\w./~-]+$/.test(s) ? s : `'${s.replaceAll("'", `'\\''`)}'`;
}

function Step({ code }: { code: string[] }): ReactElement {
  return (
    <pre className="bg-surface rounded-group overflow-x-auto p-3 font-mono text-xs">
      {code.join("\n")}
    </pre>
  );
}

/** Static recovery instructions — mirrors docs/recovery.mdx, with the open vault's real path. */
export function RecoveryPage(): ReactElement {
  const m = useMessages();
  const root = useSession((s) => s.vault?.path) ?? "~/.ks";
  const v = shellQuote(root);
  return (
    <div className="flex flex-col gap-4">
      <p className="text-muted text-sm">{m.recovery.intro}</p>
      <Step
        code={[
          "# 1. Open the identity file (asks for the master passphrase)",
          `age -d ${v}/.identity.age > /tmp/vault-identity.txt`,
          "# 2. Decrypt any entry with the identities inside",
          `age -d -i /tmp/vault-identity.txt ${v}/github/token.age | tail -n +2`,
        ]}
      />
      <p className="text-muted text-sm">{m.recovery.headerNote}</p>
      <p className="text-ink text-sm font-medium">{m.recovery.orphanTitle}</p>
      <p className="text-muted text-sm">{m.recovery.orphanIntro}</p>
      <Step
        code={[
          'ks git log --oneline -- .identity.age            # find the "ks: rekey" commit <c>',
          "ks git show '<c>~1:.identity.age' > /tmp/old-identity.age",
          `age -d -i /tmp/old-identity.age <entry>.age | tail -n +2 | ks set <name> -m --force`,
          "rm /tmp/old-identity.age",
        ]}
      />
    </div>
  );
}
