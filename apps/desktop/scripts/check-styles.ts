// Style guard for apps/desktop: no color literals in TS, canonical Tailwind classes only,
// and no --color-* token name may collide with a --text-* name (they share utility prefixes).
import { readdirSync, readFileSync } from "node:fs";
import { join, relative } from "node:path";

import { __unstable__loadDesignSystem } from "@tailwindcss/node";

type DesignSystem = {
  theme: { entries: () => Iterable<[string, unknown]> };
  parseCandidate: (candidate: string) => unknown[];
  canonicalizeCandidates: (candidates: string[], opts: { rem: number }) => string[];
};

const here = import.meta.dirname;
const root = join(here, "../src");
const css = readFileSync(join(root, "styles.css"), "utf8");
const ds = (await __unstable__loadDesignSystem(css, { base: root })) as DesignSystem;

const COLOR_LITERAL = /#[0-9a-fA-F]{3,8}\b|\b(?:rgb|rgba|hsl|hsla)\(/;
const CANDIDATE = /^[!a-z0-9:[\]\-./_%(),=#&>*@'"]+$/i;

const files = readdirSync(root, { recursive: true })
  .filter((f): f is string => typeof f === "string")
  .filter((f) => /\.tsx?$/.test(f) && !f.endsWith(".gen.ts"));

let bad = 0;
const report = (file: string, line: number, msg: string): void => {
  bad += 1;
  process.stdout.write(`${file}:${String(line)}: ${msg}\n`);
};

// (c) theme-name collision: --color-foo and --text-foo both claim the `foo` prefix.
const textNames = new Set(
  [...ds.theme.entries()]
    .filter(([k]) => k.startsWith("--text-"))
    .map(([k]) => k.slice("--text-".length)),
);
for (const [key] of ds.theme.entries()) {
  const name = key.startsWith("--color-") ? key.slice("--color-".length) : "";
  if (name !== "" && textNames.has(name)) {
    report("src/styles.css", 1, `--color-${name} collides with --text-${name}`);
  }
}

// (a)+(b): scan string literals in every source file.
for (const rel of files) {
  const text = readFileSync(join(root, rel), "utf8");
  const relName = relative(join(here, ".."), join(root, rel));
  for (const [i, line] of text.split("\n").entries()) {
    for (const m of line.matchAll(/"([^"]*)"|'([^']*)'|`([^`]*)`/g)) {
      const s = m[1] ?? m[2] ?? m[3] ?? "";
      for (const tok of s.split(/\s+/).filter((t) => t.length > 0)) {
        if (COLOR_LITERAL.test(tok)) {
          report(relName, i + 1, `color literal: ${tok}`);
        }
        if (!CANDIDATE.test(tok) || ds.parseCandidate(tok).length === 0) {
          continue;
        }
        const [canon] = ds.canonicalizeCandidates([tok], { rem: 16 });
        if (canon !== undefined && canon !== tok) {
          report(relName, i + 1, `${tok} -> ${canon}`);
        }
      }
    }
  }
}

if (bad > 0) {
  process.exitCode = 1;
} else {
  process.stdout.write(`check-styles: ${String(files.length)} files clean\n`);
}
