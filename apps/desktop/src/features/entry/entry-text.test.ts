import { describe, expect, it } from "vite-plus/test";

import { parseEntryText, serializeDraft, validateDraft } from "./entry-text";

describe("entry-text", () => {
  it("parses value, fields and free text", () => {
    const d = parseEntryText("secret\nuser: octo\nfree line\nnotes: hi\nweird key: v\n: nokey");
    expect(d.value).toBe("secret");
    expect(d.fields.map((f) => [f.key, f.value])).toStrictEqual([
      ["user", "octo"],
      ["notes", "hi"],
    ]);
    expect(d.extra).toBe("free line\nweird key: v\n: nokey");
  });

  it("parses keys containing colons", () => {
    const d = parseEntryText("v\na:b: c");
    expect(d.fields[0]).toMatchObject({ key: "a:b", value: "c" });
  });

  it("round-trips and serializes fields before free text", () => {
    const text = "secret\nuser: octo\nnotes: hi\nfree line\nweird key: v";
    expect(serializeDraft(parseEntryText(text))).toBe(text);
    const interleaved = "secret\nuser: octo\nfree line\nnotes: hi";
    expect(serializeDraft(parseEntryText(interleaved))).toBe(
      "secret\nuser: octo\nnotes: hi\nfree line",
    );
  });

  it("drops rows whose key and value are both empty and normalizes trailing newline", () => {
    const d = parseEntryText("secret\n");
    expect(d.extra).toBe("");
    expect(serializeDraft(d)).toBe("secret");
    const withBlank = parseEntryText("secret\nuser: a\n\n");
    expect(serializeDraft(withBlank)).toBe("secret\nuser: a\n\n");
  });

  it("serializes fields as `key: value` before extra", () => {
    const d = parseEntryText("v\nx: 1");
    d.fields.push({ id: "z", key: "y", value: "2" });
    expect(serializeDraft(d)).toBe("v\nx: 1\ny: 2");
  });

  it("validates", () => {
    expect(validateDraft({ value: "", fields: [], extra: "" })).toBe("empty");
    expect(validateDraft({ value: "v", fields: [], extra: "" })).toBeUndefined();
    expect(
      validateDraft({ value: "v", fields: [{ id: "a", key: "", value: "x" }], extra: "" }),
    ).toBe("key");
    expect(
      validateDraft({ value: "v", fields: [{ id: "a", key: "b c", value: "x" }], extra: "" }),
    ).toBe("keyChars");
  });
});
