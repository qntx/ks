/** Human byte size: `512 B`, `1.2 KB`, `3.4 MB`. */
export function formatBytes(n: number): string {
  if (n < 1024) {
    return `${n} B`;
  }
  const units = ["KB", "MB", "GB"];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${Math.round(v * 10) / 10} ${units[i] ?? "TB"}`;
}

/** Groups digits three at a time for TOTP display: `123456` → `123 456`. */
export function groupDigits(s: string): string {
  return s.replaceAll(/(\d{3})(?=\d)/g, "$1 ");
}
