#!/usr/bin/env node
// Verifies the complete v10 release set before GitHub Release publication.
// This is intentionally independent of the updater generator so a missing
// architecture, portable archive, signature, or manifest entry cannot pass.

import { createHash } from "node:crypto";
import { readFileSync, readdirSync } from "node:fs";
import { basename, join } from "node:path";

const ARCHITECTURES = [
  ["x64", "windows-x86_64"],
  ["arm64", "windows-aarch64"],
  ["x86", "windows-i686"],
];

function versionFromTag(tag) {
  const value = (tag ?? "").trim();
  if (!/^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(value)) return null;
  return value.slice(1);
}

function sha256(file) {
  return createHash("sha256").update(readFileSync(file)).digest("hex");
}

function assertFile(root, name) {
  const file = join(root, name);
  try {
    const bytes = readFileSync(file);
    if (bytes.length === 0) throw new Error(`${name} is empty`);
  } catch (error) {
    throw new Error(`missing or unreadable ${name}: ${error.message}`);
  }
  return file;
}

export function verifyReleaseAssets(root, tag) {
  const version = versionFromTag(tag);
  if (!version) throw new Error(`invalid release tag: ${tag}`);

  for (const [arch] of ARCHITECTURES) {
    const installer = `lnwdeck_${version}_${arch}-setup.exe`;
    const signature = `${installer}.sig`;
    const portable = `lnwdeck_${version}_${arch}_portable.zip`;
    const portableSignature = `${portable}.sig`;
    const signaturePath = assertFile(root, signature);
    if (!readFileSync(signaturePath, "utf8").trim()) {
      throw new Error(`${signature} is empty`);
    }
    const portableSignaturePath = assertFile(root, portableSignature);
    if (!readFileSync(portableSignaturePath, "utf8").trim()) {
      throw new Error(`${portableSignature} is empty`);
    }
    assertFile(root, installer);
    assertFile(root, portable);
  }

  const latestPath = assertFile(root, "latest.json");
  const latest = JSON.parse(readFileSync(latestPath, "utf8"));
  if (latest.version !== version) {
    throw new Error(`latest.json declares ${latest.version}, expected ${version}`);
  }
  const platforms = latest.platforms ?? {};
  for (const [arch, platform] of ARCHITECTURES) {
    const installer = `lnwdeck_${version}_${arch}-setup.exe`;
    const signature = readFileSync(join(root, `${installer}.sig`), "utf8").trim();
    if (platforms[platform]?.signature !== signature) {
      throw new Error(`latest.json signature does not match ${installer}.sig`);
    }
    if (platforms[platform]?.url?.endsWith(`/${installer}`) !== true) {
      throw new Error(`latest.json URL does not point to ${installer}`);
    }
  }

  return {
    version,
    files: readdirSync(root).sort(),
    sha256: Object.fromEntries(
      readdirSync(root)
        .filter((name) => /\.(exe|msi|zip|sig|json|txt)$/.test(name))
        .sort()
        .map((name) => [name, sha256(join(root, name))]),
    ),
  };
}

function main() {
  const [assetsDir, tag] = process.argv.slice(2);
  if (!assetsDir || !tag) {
    process.stderr.write(
      "usage: node scripts/verify-release-assets.mjs <assets-dir> <tag>\n",
    );
    process.exit(1);
  }
  try {
    const result = verifyReleaseAssets(assetsDir, tag);
    process.stdout.write(
      `verified ${result.version}: ${result.files.length} release files\n`,
    );
  } catch (error) {
    process.stderr.write(`release asset verification failed: ${error.message}\n`);
    process.exit(1);
  }
}

if (process.argv[1] && basename(process.argv[1]) === "verify-release-assets.mjs") {
  main();
}
