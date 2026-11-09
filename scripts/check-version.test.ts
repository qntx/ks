import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { describe, expect, test } from "vite-plus/test";

import { checkVersion, memberManifestPaths } from "./check-version.ts";

const CARGO_TOML = `[workspace]
members = ["apps/desktop/src-tauri", "crates/*"]

[workspace.package]
version = "1.8.0"

[workspace.dependencies]
ks-core = { version = "1.8", path = "crates/ks-core" }
serde = "1"
`;

const PARSED = {
  workspace: {
    members: ["apps/desktop/src-tauri", "crates/*"],
    package: { version: "1.8.0" },
    dependencies: {
      "ks-core": { version: "1.8", path: "crates/ks-core" },
      serde: "1",
    },
  },
};

const OVO = {
  path: "packages/ovo/package.json",
  pkg: { name: "@qntx/ovo", version: "1.8.0" },
};

const PRIVATE_PKG = {
  path: "packages/sandbox/package.json",
  pkg: { name: "@qntx/sandbox", version: "0.0.1", private: true },
};

const CRATES = [
  {
    path: "apps/desktop/src-tauri/Cargo.toml",
    manifest: { package: { name: "ovo-desktop", version: { workspace: true } } },
  },
  {
    path: "crates/ks-core/Cargo.toml",
    manifest: { package: { name: "ks-core", version: { workspace: true } } },
  },
];

const PACKAGES = [OVO, PRIVATE_PKG];

describe("check-version", () => {
  test("accepts synced versions", () => {
    expect(checkVersion(PACKAGES, PARSED, CARGO_TOML, CRATES)).toStrictEqual([]);
  });

  test("rejects a package.json mismatch", () => {
    const errors = checkVersion(
      [{ ...OVO, pkg: { ...OVO.pkg, version: "1.8.1" } }, PRIVATE_PKG],
      PARSED,
      CARGO_TOML,
      CRATES,
    );
    expect(errors).toStrictEqual([
      "version mismatch: packages/ovo/package.json has 1.8.1, Cargo.toml has 1.8.0",
    ]);
  });

  test("ignores private packages", () => {
    const pkg = { name: "@qntx/sandbox", version: "9.9.9", private: true };
    const errors = checkVersion([OVO, { ...PRIVATE_PKG, pkg }], PARSED, CARGO_TOML, CRATES);
    expect(errors).toStrictEqual([]);
  });

  test("rejects a crate manifest that hard-codes a version", () => {
    const crates = [
      {
        path: "apps/desktop/src-tauri/Cargo.toml",
        manifest: { package: { name: "ovo-desktop", version: { workspace: true } } },
      },
      {
        path: "crates/ks-core/Cargo.toml",
        manifest: { package: { name: "ks-core", version: "1.8.0" } },
      },
    ];
    const errors = checkVersion(PACKAGES, PARSED, CARGO_TOML, crates);
    expect(errors).toStrictEqual([
      'crates/ks-core/Cargo.toml: package version must be "version.workspace = true"',
    ]);
  });

  test("rejects an internal dep not pinned to the workspace version", () => {
    const parsed = structuredClone(PARSED);
    parsed.workspace.dependencies["ks-core"].version = "=1.7.0";
    const errors = checkVersion(PACKAGES, parsed, CARGO_TOML, CRATES);
    expect(errors).toHaveLength(1);
    expect(errors[0]).toContain("ks-core");
  });

  test("rejects a third-party dep equal to the current version", () => {
    const toml = CARGO_TOML.replace('serde = "1"', 'serde = "1.8.0"');
    const parsed = structuredClone(PARSED);
    parsed.workspace.dependencies.serde = "1.8.0";
    const errors = checkVersion(PACKAGES, parsed, toml, CRATES);
    expect(errors).toStrictEqual([
      'Cargo.toml: "1.8.0" appears 2 times, expected 1 ([workspace.package] plus internal path dependencies)',
    ]);
  });
});

describe("memberManifestPaths", () => {
  test("expands glob members to subdir manifests and keeps plain members", () => {
    const root = mkdtempSync(join(tmpdir(), "check-version-"));
    try {
      mkdirSync(join(root, "crates/ks-core"), { recursive: true });
      mkdirSync(join(root, "crates/ovo-rpc"), { recursive: true });
      mkdirSync(join(root, "apps/desktop/src-tauri"), { recursive: true });
      writeFileSync(join(root, "crates/ks-core/Cargo.toml"), "");
      writeFileSync(join(root, "crates/ovo-rpc/Cargo.toml"), "");
      writeFileSync(join(root, "apps/desktop/src-tauri/Cargo.toml"), "");
      const toml = {
        workspace: {
          members: [join(root, "apps/desktop/src-tauri"), join(root, "crates/*")],
        },
      };
      expect(memberManifestPaths(toml)).toStrictEqual([
        join(root, "apps/desktop/src-tauri/Cargo.toml"),
        join(root, "crates/ks-core/Cargo.toml"),
        join(root, "crates/ovo-rpc/Cargo.toml"),
      ]);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
});
