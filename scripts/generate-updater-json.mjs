#!/usr/bin/env node
// Generates the Tauri updater latest.json for a release.
//
// Usage:
//   node scripts/generate-updater-json.mjs <tag> <assets-dir> [output-file]
//
// Scans the assets directory recursively for signed NSIS installers
// (`*-setup.exe` paired with `*.sig` files) and writes latest.json with the
// correct updater platform keys and download URLs. Never fabricates a
// signature: an installer without its `.sig` file fails the run.

import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { basename, join } from "node:path";

const ARCH_MAP = [
  ["_x64-setup.exe", "windows-x86_64"],
  ["_arm64-setup.exe", "windows-aarch64"],
  ["_x86-setup.exe", "windows-i686"],
];

export function platformFor(fileName) {
  const hit = ARCH_MAP.find(([suffix]) => fileName.endsWith(suffix));
  return hit ? hit[1] : null;
}

export function parseTag(tag) {
  const value = (tag ?? "").trim();
  if (!value.startsWith("v")) return null;
  return value.slice(1);
}

/**
 * Every file under `root`, recursively, as `{ name, path }`.
 *
 * The release workflow downloads build artifacts with their directory structure
 * intact (`target/<triple>/release/bundle/nsis/...`), so a top-level listing
 * finds nothing. This is what made the previous release fail.
 */
export function collectFiles(root) {
  const found = [];
  const stack = [root];
  while (stack.length > 0) {
    const current = stack.pop();
    for (const entry of readdirSync(current, { withFileTypes: true })) {
      const path = join(current, entry.name);
      if (entry.isDirectory()) {
        stack.push(path);
      } else {
        found.push({ name: entry.name, path });
      }
    }
  }
  return found;
}

export function buildUpdaterJson({ tag, repo, assetsDir }) {
  const version = parseTag(tag);
  if (!version) {
    throw new Error(`invalid tag: ${tag}`);
  }
  const platforms = {};
  const files = collectFiles(assetsDir);
  for (const file of files) {
    if (!file.name.endsWith("-setup.exe")) continue;
    const platform = platformFor(file.name);
    if (!platform) continue;
    const sigName = `${file.name}.sig`;
    const sigFile = files.find((candidate) => candidate.name === sigName);
    if (!sigFile) {
      throw new Error(`installer ${file.name} has no ${sigName} signature`);
    }
    const signature = readFileSync(sigFile.path, "utf8").trim();
    if (!signature) {
      throw new Error(`signature file ${sigName} is empty`);
    }
    platforms[platform] = {
      signature,
      url: `https://github.com/${repo}/releases/download/${tag}/${file.name}`,
    };
  }
  if (Object.keys(platforms).length === 0) {
    throw new Error(`no signed NSIS installers found in ${assetsDir}`);
  }
  return {
    version,
    notes: "",
    pub_date: new Date().toISOString(),
    platforms,
  };
}

function main() {
  const [tag, assetsDir, outputFile] = process.argv.slice(2);
  const repo = process.env.GITHUB_REPOSITORY ?? "engasnm111/lnwdeck";
  if (!tag || !assetsDir) {
    process.stderr.write(
      "usage: node scripts/generate-updater-json.mjs <tag> <assets-dir> [output-file]\n",
    );
    process.exit(1);
  }
  try {
    const json = buildUpdaterJson({ tag, repo, assetsDir });
    const out = outputFile ?? "latest.json";
    writeFileSync(out, `${JSON.stringify(json, null, 2)}\n`);
    process.stdout.write(
      `wrote ${out} for ${json.version} (${Object.keys(json.platforms).length} platforms)\n`,
    );
  } catch (err) {
    let listing = "";
    try {
      listing = readdirSync(assetsDir)
        .sort()
        .join(", ");
    } catch {
      listing = `(cannot read ${assetsDir})`;
    }
    process.stderr.write(`::error::${err.message} | assets: [${listing}]\n`);
    process.exit(1);
  }
}

if (process.argv[1] && basename(process.argv[1]) === "generate-updater-json.mjs") {
  main();
}
