import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import assert from "node:assert/strict";
import { writeChecksums } from "./generate-release-metadata.mjs";
import { verifyReleaseAssets } from "./verify-release-assets.mjs";

test("release metadata verifies all v10 architectures and checksums", () => {
  const root = mkdtempSync(join(tmpdir(), "lnwdeck-release-"));
  try {
    const version = "10.0.0";
    const signatures = {};
    for (const arch of ["x64", "arm64", "x86"]) {
      const installer = `lnwdeck_${version}_${arch}-setup.exe`;
      const signature = `${installer}.sig`;
      writeFileSync(join(root, installer), `${arch}-installer`);
      signatures[arch] = `signature-${arch}`;
      writeFileSync(join(root, signature), signatures[arch]);
      const portable = `lnwdeck_${version}_${arch}_portable.zip`;
      writeFileSync(join(root, portable), `${arch}-portable`);
      writeFileSync(join(root, `${portable}.sig`), `portable-signature-${arch}`);
    }
    writeFileSync(join(root, "lnwdeck_10.0.0_sbom.cdx.json"), "{}\n");
    writeFileSync(
      join(root, "latest.json"),
      JSON.stringify({
        version,
        platforms: {
          "windows-x86_64": {
            signature: signatures.x64,
            url: `https://github.com/example/lnwdeck/releases/download/v${version}/lnwdeck_${version}_x64-setup.exe`,
          },
          "windows-aarch64": {
            signature: signatures.arm64,
            url: `https://github.com/example/lnwdeck/releases/download/v${version}/lnwdeck_${version}_arm64-setup.exe`,
          },
          "windows-i686": {
            signature: signatures.x86,
            url: `https://github.com/example/lnwdeck/releases/download/v${version}/lnwdeck_${version}_x86-setup.exe`,
          },
        },
      }),
    );

    const result = verifyReleaseAssets(root, `v${version}`);
    assert.equal(result.version, version);
    writeChecksums(root);
    const checksumLines = requireChecksumLines(root);
    for (const line of checksumLines) {
      const [digest, , name] = line.split(" ");
      assert.equal(digest, createHash("sha256").update(requireFile(root, name)).digest("hex"));
    }
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

function requireFile(root, name) {
  return readFileSync(join(root, name));
}

function requireChecksumLines(root) {
  return readFileSync(join(root, "SHA256SUMS"), "utf8")
    .trim()
    .split("\n");
}
