/** Mirrors `crates/ks/src/entry.rs` `Text::fields`: `key: value` after line 1. */
export type EntryDraft = {
  value: string;
  fields: Array<{ id: string; key: string; value: string }>;
  /** Free-text lines that are not fields, joined by "\n". */
  extra: string;
};

const hasWs = (s: string): boolean => /[ \t\n\f\r]/.test(s);

/** Stable per-field id used as the React key for draft rows. */
export function newFieldId(): string {
  return crypto.randomUUID();
}

export function parseEntryText(text: string): EntryDraft {
  const lines = text.split("\n");
  const value = lines[0] ?? "";
  const fields: EntryDraft["fields"] = [];
  const extra: string[] = [];
  for (const line of lines.slice(1)) {
    const i = line.indexOf(": ");
    const key = i > 0 ? line.slice(0, i) : "";
    if (i > 0 && !hasWs(key)) {
      fields.push({ id: newFieldId(), key, value: line.slice(i + 2) });
    } else {
      extra.push(line);
    }
  }
  return { value, fields, extra: extra.join("\n") };
}

export function serializeDraft(draft: EntryDraft): string {
  const lines = [draft.value];
  for (const f of draft.fields) {
    if (f.key === "" && f.value === "") {
      continue;
    }
    lines.push(`${f.key}: ${f.value}`);
  }
  if (draft.extra !== "") {
    lines.push(...draft.extra.split("\n"));
  }
  return lines.join("\n");
}

export type DraftError = "empty" | "key" | "keyChars" | "newline";

export function validateDraft(draft: EntryDraft): DraftError | undefined {
  if (draft.value.includes("\n") || draft.value.includes("\r")) {
    return "newline";
  }
  for (const f of draft.fields) {
    if (f.key === "" && f.value === "") {
      continue;
    }
    if (f.value.includes("\n") || f.value.includes("\r")) {
      return "newline";
    }
    if (f.key === "") {
      return "key";
    }
    if (hasWs(f.key) || f.key.includes(":")) {
      return "keyChars";
    }
  }
  return serializeDraft(draft) === "" ? "empty" : undefined;
}
