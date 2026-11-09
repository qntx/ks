export function parentPath(name: string): string {
  const i = name.lastIndexOf("/");
  return i === -1 ? "" : name.slice(0, i);
}

export function leafName(path: string): string {
  const i = path.lastIndexOf("/");
  return i === -1 ? path : path.slice(i + 1);
}
