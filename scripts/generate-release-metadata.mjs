#!/usr/bin/env node
// Generates deterministic checksums for the files that will be published.
// Release assets are flattened before this script runs, so basenames are
// stable in SHA256SUMS and can be verified on any platform.

import { createHash } from "node:crypto";
import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { basename, join } from "node:path";

export function collectReleaseFiles(root) {
  return readdirSync(root, { withFileTypes: true })
    .filter((entry) => entry.isFile())
    .map((entry) => entry.name)
    .filter((name) => name !== "SHA256SUMS")
    .sort();
}

export function sha256File(file) {
  return createHash("sha256").update(readFileSync(file)).digest("hex");
}

export function writeChecksums(root, outputName = "SHA256SUMS") {
  const files = collectReleaseFiles(root);
  if (files.length === 0) {
    throw new Error(`no release files found in ${root}`);
  }
  const output = files
    .map((name) => `${sha256File(join(root, name))}  ${basename(name)}`)
    .join("\n");
  writeFileSync(join(root, outputName), `${output}\n`);
  return files;
}

function main() {
  const [assetsDir, outputName = "SHA256SUMS"] = process.argv.slice(2);
  if (!assetsDir) {
    process.stderr.write(
      "usage: node scripts/generate-release-metadata.mjs <assets-dir> [output-name]\n",
    );
    process.exit(1);
  }
  try {
    const files = writeChecksums(assetsDir, outputName);
    process.stdout.write(`wrote ${outputName} for ${files.length} release files\n`);
  } catch (error) {
    process.stderr.write(`checksum generation failed: ${error.message}\n`);
    process.exit(1);
  }
}

if (process.argv[1] && basename(process.argv[1]) === "generate-release-metadata.mjs") {
  main();
}
