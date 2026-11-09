import type { ReactElement } from "react";

import { useMessages } from "../../i18n";
import { PasswordField } from "../../ui";
import { StrengthMeter } from "../access";

/** New + meter + repeat passphrase block shared by the passphrase and rotate pages. */
export function NewPassphraseFields({
  pass,
  repeat,
  onPass,
  onRepeat,
}: {
  pass: string;
  repeat: string;
  onPass: (v: string) => void;
  onRepeat: (v: string) => void;
}): ReactElement {
  const m = useMessages();
  const mismatch = repeat !== "" && pass !== repeat;
  return (
    <>
      <div>
        <PasswordField
          label={m.maintenance.newPassphrase}
          name="new"
          value={pass}
          onChange={(e) => {
            onPass(e.target.value);
          }}
        />
        <StrengthMeter passphrase={pass} />
      </div>
      <PasswordField
        label={m.maintenance.repeatPassphrase}
        name="repeat"
        value={repeat}
        error={mismatch ? m.maintenance.mismatch : undefined}
        onChange={(e) => {
          onRepeat(e.target.value);
        }}
      />
    </>
  );
}
