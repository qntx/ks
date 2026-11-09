import { describe, expect, it } from "vite-plus/test";

import { leafName, parentPath } from "../../lib/paths";
import { buildTree, folderAt, folderItems, searchItems } from "./tree";

const NAMES = [
  "work/github",
  "work/aws/root",
  "work/aws/staging",
  "personal/bank",
  "solo",
  "a",
  "a/b",
];

describe("tree", () => {
  it("builds folders, entries and totals", () => {
    const t = buildTree(NAMES);
    expect(t.total).toBe(7);
    expect(t.entries).toStrictEqual(["a", "solo"]);
    const work = folderAt(t, "work");
    expect(work?.total).toBe(3);
    expect(work?.folders.map((f) => f.path)).toStrictEqual(["work/aws"]);
    expect(folderAt(t, "work/aws")?.entries).toStrictEqual(["work/aws/root", "work/aws/staging"]);
  });

  it("keeps a name that is both entry and folder prefix", () => {
    const t = buildTree(NAMES);
    expect(t.entries).toContain("a");
    expect(t.folders.map((f) => f.path)).toContain("a");
    expect(folderAt(t, "a")?.entries).toStrictEqual(["a/b"]);
  });

  it("returns undefined for missing folders", () => {
    const t = buildTree(NAMES);
    expect(folderAt(t, "nope")).toBeUndefined();
    expect(folderAt(t, "work/nope")).toBeUndefined();
  });

  it("lists folders before entries, each sorted", () => {
    const t = buildTree(NAMES);
    const items = folderItems(t);
    expect(items.map((i) => i.kind)).toStrictEqual([
      "folder",
      "folder",
      "folder",
      "entry",
      "entry",
    ]);
    expect(items[0]).toMatchObject({ kind: "folder", path: "a", count: 1 });
    expect(items[1]).toMatchObject({ kind: "folder", path: "personal", count: 1 });
    expect(items[2]).toMatchObject({ kind: "folder", path: "work", count: 3 });
    expect(items[3]).toMatchObject({ kind: "entry", name: "a", label: "a", parent: "" });
  });

  it("searches full names and reports the parent", () => {
    const items = searchItems("aws", NAMES);
    expect(items).toHaveLength(2);
    expect(items[0]).toMatchObject({ kind: "entry", name: "work/aws/root", parent: "work/aws" });
  });

  it("path helpers", () => {
    expect(parentPath("a/b/c")).toBe("a/b");
    expect(parentPath("a")).toBe("");
    expect(leafName("a/b/c")).toBe("c");
    expect(leafName("a")).toBe("a");
  });
});
