#!/usr/bin/env node
// Generates the Tauri updater latest.json for a release.
//
// Usage:
//   node scripts/generate-updater-json.mjs <tag> <assets-dir> [output-file]
//
// Scans the assets directory for signed NSIS installers (`*-setup.exe`
// paired with `*.sig` files) and writes latest.json with the correct
// updater platform keys and download URLs. Never fabricates signatures:
// an installer without its `.sig` file is skipped.

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

export function buildUpdaterJson({ tag, repo, assetsDir }) {
  const version = parseTag(tag);
  if (!version) {
    throw new Error(`invalid tag: ${tag}`);
  }
  const platforms = {};
  const files = readdirSync(assetsDir);
  for (const file of files) {
    if (!file.endsWith("-setup.exe")) continue;
    const platform = platformFor(file);
    if (!platform) continue;
    const sigFile = `${file}.sig`;
    if (!files.includes(sigFile)) {
      throw new Error(`installer ${file} has no ${sigFile} signature`);
    }
    const signature = readFileSync(join(assetsDir, sigFile), "utf8").trim();
    if (!signature) {
      throw new Error(`signature file ${sigFile} is empty`);
    }
    platforms[platform] = {
      signature,
      url: `https://github.com/${repo}/releases/download/${tag}/${file}`,
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
  const json = buildUpdaterJson({ tag, repo, assetsDir });
  const out = outputFile ?? "latest.json";
  writeFileSync(out, `${JSON.stringify(json, null, 2)}\n`);
  process.stdout.write(
    `wrote ${out} for ${json.version} (${Object.keys(json.platforms).length} platforms)\n`,
  );
}

if (process.argv[1] && basename(process.argv[1]) === "generate-updater-json.mjs") {
  main();
}
