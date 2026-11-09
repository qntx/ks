import { fuzzyFilter } from "../../lib/fuzzy";
import { leafName, parentPath } from "../../lib/paths";

export type FolderNode = {
  /** Full folder path; "" is the root. */
  path: string;
  /** Leaf segment; "" for the root. */
  name: string;
  folders: FolderNode[];
  /** Entry names directly inside this folder. */
  entries: string[];
  /** Entries in the whole subtree. */
  total: number;
};

/** Builds the folder tree from flat entry names. */
export function buildTree(names: string[]): FolderNode {
  const root: FolderNode = { path: "", name: "", folders: [], entries: [], total: 0 };
  const byPath = new Map<string, FolderNode>([["", root]]);
  const folderNode = (path: string): FolderNode => {
    const hit = byPath.get(path);
    if (hit !== undefined) {
      return hit;
    }
    const node: FolderNode = { path, name: leafName(path), folders: [], entries: [], total: 0 };
    byPath.set(path, node);
    folderNode(parentPath(path)).folders.push(node);
    return node;
  };
  for (const name of names) {
    folderNode(parentPath(name)).entries.push(name);
  }
  const finish = (node: FolderNode): number => {
    node.folders = node.folders.toSorted((a, b) => a.name.localeCompare(b.name));
    node.entries = node.entries.toSorted((a, b) => a.localeCompare(b));
    node.total = node.entries.length;
    for (const f of node.folders) {
      node.total += finish(f);
    }
    return node.total;
  };
  finish(root);
  return root;
}

export function folderAt(root: FolderNode, path: string): FolderNode | undefined {
  let node = root;
  if (path === "") {
    return root;
  }
  for (const seg of path.split("/")) {
    const next = node.folders.find((f) => f.name === seg);
    if (next === undefined) {
      return undefined;
    }
    node = next;
  }
  return node;
}

export type ListItem =
  | { kind: "folder"; path: string; name: string; count: number }
  | { kind: "entry"; name: string; label: string; parent: string };

/** Folders first, then entries; each sorted by name. */
export function folderItems(folder: FolderNode): ListItem[] {
  return [
    ...folder.folders.map((f) => ({
      kind: "folder" as const,
      path: f.path,
      name: f.name,
      count: f.total,
    })),
    ...folder.entries.map((name) => ({
      kind: "entry" as const,
      name,
      label: leafName(name),
      parent: parentPath(name),
    })),
  ];
}

/** Fuzzy search over full entry names; the parent is shown as secondary text. */
export function searchItems(query: string, names: string[]): ListItem[] {
  return fuzzyFilter(query, names).map((name) => ({
    kind: "entry" as const,
    name,
    label: leafName(name),
    parent: parentPath(name),
  }));
}
