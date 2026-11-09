import { execFileSync } from "node:child_process";

import { defineConfig } from "bumpp";

// npm package and Cargo workspace versions are lockstep; bumpp rewrites every
// `x.y.z` occurrence of the current version in non-JSON files, so Cargo.toml
// picks up [workspace.package].version and any `=<version>` internal path deps.
const config: ReturnType<typeof defineConfig> = defineConfig({
  files: ["Cargo.toml"],
  // Cargo.lock records the workspace version; refresh it on every bump.
  execute: () => {
    execFileSync("cargo", ["update", "--workspace"], { stdio: "inherit" });
  },
  // execute rewrites the lockfile; commit it too.
  all: true,
  commit: true,
  // main is protected: release through a `release/v<version>` pull request
  // opened via `gh`. No tag is created; after the release PR merges, tag the
  // merge commit by hand (GITHUB_TOKEN tags do not trigger publish workflows).
  pr: true,
  push: true,
});

export default config;
