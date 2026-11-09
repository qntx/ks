/// <reference types="node" />
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

// The repo does not depend on @types/bun; declare the used surface.
declare const Bun: {
  TOML: { parse: (text: string) => unknown };
};

type JsonObject = Record<string, unknown>;

function isRecord(value: unknown): value is JsonObject {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function asRecord(value: unknown): JsonObject {
  return isRecord(value) ? value : {};
}

function escapeRegExp(text: string): string {
  return text.replaceAll(/[.*+?^${}()|[\]\\]/g, String.raw`\$&`);
}

export type PackageEntry = { path: string; pkg: unknown };
export type CrateEntry = { path: string; manifest: unknown };

/**
 * Lockstep check: every published workspace package's `version` must equal
 * `[workspace.package].version`, every member crate manifest must inherit it via
 * `version.workspace`, every internal `crates/` path dependency must pin `=<version>`, and the
 * version string may appear nowhere else in Cargo.toml (bumpp rewrites every occurrence). Private
 * packages are ignored; the private workspace root is not passed in and takes no part.
 */
export function checkVersion(
  packages: PackageEntry[],
  cargoToml: unknown,
  cargoText: string,
  crates: CrateEntry[],
): string[] {
  const errors: string[] = [];
  const workspace = asRecord(asRecord(cargoToml)["workspace"]);
  const { version: cargoVersion } = asRecord(workspace["package"]);

  if (typeof cargoVersion !== "string") {
    errors.push("Cargo.toml: missing string [workspace.package].version");
  }

  for (const { path, pkg } of packages) {
    const manifest = asRecord(pkg);
    if (manifest["private"] === true) {
      continue;
    }
    const { version } = manifest;
    if (typeof version !== "string") {
      errors.push(`${path}: missing string "version"`);
      continue;
    }
    if (typeof cargoVersion === "string" && version !== cargoVersion) {
      errors.push(`version mismatch: ${path} has ${version}, Cargo.toml has ${cargoVersion}`);
    }
  }

  for (const { path, manifest } of crates) {
    const crate = asRecord(asRecord(manifest)["package"]);
    const { version } = crate;
    if (isRecord(version) && version["workspace"] === true) {
      continue;
    }
    errors.push(`${path}: package version must be "version.workspace = true"`);
  }

  let internalDeps = 0;
  const dependencies = asRecord(workspace["dependencies"]);
  for (const [name, spec] of Object.entries(dependencies)) {
    const entry = asRecord(spec);
    const { path } = entry;
    if (typeof path !== "string" || !path.startsWith("crates/")) {
      continue;
    }
    internalDeps += 1;
    // ks pins internal crates with a major.minor requirement ("0.8"),
    // not the exact `=<version>` used in bot.
    const expected =
      typeof cargoVersion === "string" ? cargoVersion.split(".").slice(0, 2).join(".") : "";
    if (entry["version"] !== expected) {
      errors.push(
        `Cargo.toml: internal dependency "${name}" must use version "${expected}", got ${JSON.stringify(entry["version"])}`,
      );
    }
  }

  if (typeof cargoVersion === "string") {
    const occurrences =
      cargoText.match(new RegExp(`\\b${escapeRegExp(cargoVersion)}\\b`, "g"))?.length ?? 0;
    // ks internal deps carry a major.minor ("0.8") requirement, not the
    // full version, so the literal may only appear in [workspace.package].
    const expected = 1;
    if (occurrences !== expected) {
      errors.push(
        `Cargo.toml: "${cargoVersion}" appears ${occurrences} times, expected ${expected} ([workspace.package] plus internal path dependencies)`,
      );
    }
  }

  return errors;
}

/** Expands `[workspace].members` to manifest paths; a member ending in `/*` lists its subdirs. */
export function memberManifestPaths(cargoToml: unknown): string[] {
  const workspace = asRecord(asRecord(cargoToml)["workspace"]);
  const { members } = workspace;
  const paths: string[] = [];
  for (const member of Array.isArray(members) ? members : []) {
    if (typeof member !== "string") {
      continue;
    }
    if (member.endsWith("/*")) {
      const dir = member.slice(0, -2);
      for (const entry of readdirSync(dir, { withFileTypes: true })) {
        if (entry.isDirectory()) {
          paths.push(join(dir, entry.name, "Cargo.toml"));
        }
      }
    } else {
      paths.push(join(member, "Cargo.toml"));
    }
  }
  return paths;
}

if (import.meta.main) {
  const packages: PackageEntry[] = existsSync("packages")
    ? readdirSync("packages", { withFileTypes: true })
        .filter((entry) => entry.isDirectory())
        .map((entry) => {
          const path = join("packages", entry.name, "package.json");
          return { path, pkg: JSON.parse(readFileSync(path, "utf8")) as unknown };
        })
    : [];
  const cargoText = readFileSync("Cargo.toml", "utf8");
  const cargoToml = Bun.TOML.parse(cargoText);
  const crates: CrateEntry[] = memberManifestPaths(cargoToml).map((path) => ({
    path,
    manifest: Bun.TOML.parse(readFileSync(path, "utf8")),
  }));
  const errors = checkVersion(packages, cargoToml, cargoText, crates);
  for (const error of errors) {
    console.error(`check-version: ${error}`);
  }
  if (errors.length > 0) {
    process.exitCode = 1;
  }
}
