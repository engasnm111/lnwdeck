#!/usr/bin/env node
// Release version consistency check for lnwdeck.
//
// Usage:
//   node scripts/check-release-version.mjs [tag]
//
// Reads the tag (from GITHUB_REF_NAME env or the first CLI argument),
// strips a leading "v", and verifies that the application version declared
// in tauri.conf.json, the desktop crate Cargo.toml, and
// installer/package-config.json all match. Exits 1 with a clear message on
// mismatch. Supports SemVer prerelease tags such as v0.2.0-rc.1.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const REPO_ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");

export function normalizeTag(tag) {
  const value = (tag ?? "").trim();
  if (!value) return null;
  return value.startsWith("v") ? value.slice(1) : value;
}

export function isSemVer(version) {
  return /^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$/.test(
    version,
  );
}

export function readJson(file) {
  return JSON.parse(readFileSync(file, "utf8"));
}

export function readCargoVersion(file) {
  const content = readFileSync(file, "utf8");
  const match = content.match(/^version\s*=\s*"([^"]+)"/m);
  if (!match) {
    throw new Error(`version field not found in ${file}`);
  }
  return match[1];
}

export function loadDeclaredVersions(root = REPO_ROOT) {
  const tauri = readJson(join(root, "apps/desktop/src-tauri/tauri.conf.json"));
  const installer = readJson(join(root, "installer/package-config.json"));
  return {
    tauri: tauri.version,
    cargo: readCargoVersion(join(root, "apps/desktop/src-tauri/Cargo.toml")),
    installer: installer.version,
  };
}

export function checkVersions(tag, declared) {
  const tagVersion = normalizeTag(tag);
  if (!tagVersion) {
    return { ok: false, errors: ["tag is missing or empty"] };
  }
  if (!isSemVer(tagVersion)) {
    return {
      ok: false,
      errors: [`tag "${tag}" is not a valid SemVer version`],
    };
  }
  const errors = [];
  const sources = [
    ["tauri.conf.json", declared.tauri],
    ["apps/desktop/src-tauri/Cargo.toml", declared.cargo],
    ["installer/package-config.json", declared.installer],
  ];
  for (const [label, version] of sources) {
    if (version !== tagVersion) {
      errors.push(
        `${label} declares "${version}" but tag is "${tagVersion}"`,
      );
    }
  }
  return { ok: errors.length === 0, errors, version: tagVersion };
}

function main() {
  const tag = process.argv[2] ?? process.env.GITHUB_REF_NAME;
  const result = checkVersions(tag, loadDeclaredVersions());
  if (!result.ok) {
    for (const error of result.errors) {
      process.stderr.write(`release version check failed: ${error}\n`);
    }
    process.exit(1);
  }
  process.stdout.write(`release version ${result.version} is consistent\n`);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  main();
}
