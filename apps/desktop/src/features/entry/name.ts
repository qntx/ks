/** Entry-name helpers; rules mirror `crates/ks/src/name.rs`. */
export function joinName(folder: string, leaf: string): string {
  return folder === "" ? leaf : `${folder}/${leaf}`;
}

/** Suggests a vault name for an imported file inside `folder`. */
export function suggestName(folder: string, fileName: string): string {
  const leaf = fileName.split(/[\\/]/).pop() ?? fileName;
  const cleaned = leaf
    .toLowerCase()
    .replaceAll(/[^a-z0-9._-]+/g, "-")
    .replace(/^[.-]+/, "")
    .replace(/[.-]+$/, "")
    .replace(/\.age$/, "");
  return joinName(folder, cleaned === "" ? "file" : cleaned);
}
