import { defineConfig } from "vite-plus";
import type { UserConfig } from "vite-plus";

import { fmt } from "@qntx/oxfmt";
import { merge, react } from "@qntx/oxlint";

const config: UserConfig = defineConfig({
  defaultPackage: {
    dev: "./apps/desktop",
    build: "./apps/desktop",
    preview: "./apps/desktop",
  },
  staged: { "*": "vp check --fix" },
  fmt: {
    ...fmt,
    ignorePatterns: [
      ...fmt.ignorePatterns,
      "target/**",
      "bun.lock",
      // TOML is owned by taplo (.taplo.toml, align_entries).
      "**/*.toml",
      // tauri-specta bindings and generated Tauri schemas are byte-frozen.
      "**/*.gen.ts",
      "apps/desktop/src-tauri/gen/**",
      // Byte-frozen test vectors; edited only by their generators.
      "vectors/**",
      // Markdown is owned by markdownlint, CI/editor files are not
      // oxfmt's business.
      "**/*.md",
      "**/*.mdx",
      ".github/**",
      ".vscode/**",
      ".markdownlint.jsonc",
    ],
  },
  lint: merge(react, {
    // merge() concatenates arrays onto the preset's own ignorePatterns.
    ignorePatterns: ["target/**", "**/*.gen.ts", "apps/desktop/src-tauri/gen/**"],
    jsPlugins: [{ name: "vite-plus", specifier: "vite-plus/oxlint-plugin" }],
    rules: {
      "vite-plus/prefer-vite-plus-imports": "error",
      // Vite idiom: CSS is imported for side effects.
      "import/no-unassigned-import": ["error", { allow: ["**/*.css"] }],
      // oxfmt canonicalizes hex digits to lowercase; this rule demands uppercase, so fmt always wins.
      "unicorn/number-literal-case": "off",
    },
    overrides: [
      {
        files: ["scripts/**"],
        rules: {
          // Maintenance scripts print their results.
          "eslint/no-console": "off",
        },
      },
    ],
  }),
  run: { cache: process.env["CI"] === undefined || process.env["CI"] === "" },
  test: {
    include: ["scripts/**/*.test.ts"],
  },
});

export default config;
