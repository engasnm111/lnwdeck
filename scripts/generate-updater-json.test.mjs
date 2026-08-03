import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  buildUpdaterJson,
  parseTag,
  platformFor,
} from "./generate-updater-json.mjs";

function fixtureDir() {
  return mkdtempSync(join(tmpdir(), "lnwdeck-updater-"));
}

function signedInstaller(dir, fileName, signature = "test-signature-v1") {
  writeFileSync(join(dir, fileName), "binary");
  writeFileSync(join(dir, `${fileName}.sig`), signature);
}

test("parseTag strips v and rejects malformed tags", () => {
  assert.equal(parseTag("v0.2.0"), "0.2.0");
  assert.equal(parseTag("v0.2.0-rc.1"), "0.2.0-rc.1");
  assert.equal(parseTag("0.2.0"), null);
  assert.equal(parseTag(""), null);
});

test("platformFor maps artifact names to updater platforms", () => {
  assert.equal(platformFor("lnwdeck_0.2.0_x64-setup.exe"), "windows-x86_64");
  assert.equal(platformFor("lnwdeck_0.2.0_arm64-setup.exe"), "windows-aarch64");
  assert.equal(platformFor("lnwdeck_0.2.0_x86-setup.exe"), "windows-i686");
  assert.equal(platformFor("lnwdeck_0.2.0_portable.zip"), null);
  assert.equal(platformFor("setup.exe"), null);
});

test("buildUpdaterJson maps signed installers to platform entries", () => {
  const dir = fixtureDir();
  signedInstaller(dir, "lnwdeck_0.2.0_x64-setup.exe", "sig-x64");
  signedInstaller(dir, "lnwdeck_0.2.0_arm64-setup.exe", "sig-arm64");

  const json = buildUpdaterJson({
    tag: "v0.2.0",
    repo: "engasnm111/lnwdeck",
    assetsDir: dir,
  });

  assert.equal(json.version, "0.2.0");
  assert.deepEqual(Object.keys(json.platforms).sort(), [
    "windows-aarch64",
    "windows-x86_64",
  ]);
  assert.equal(json.platforms["windows-x86_64"].signature, "sig-x64");
  assert.equal(
    json.platforms["windows-x86_64"].url,
    "https://github.com/engasnm111/lnwdeck/releases/download/v0.2.0/lnwdeck_0.2.0_x64-setup.exe",
  );
});

test("buildUpdaterJson rejects installers without signatures", () => {
  const dir = fixtureDir();
  writeFileSync(join(dir, "lnwdeck_0.2.0_x64-setup.exe"), "binary");

  assert.throws(
    () =>
      buildUpdaterJson({ tag: "v0.2.0", repo: "x/y", assetsDir: dir }),
    /has no .*\.sig signature/,
  );
});

test("buildUpdaterJson fails when no signed installers exist", () => {
  const dir = fixtureDir();
  writeFileSync(join(dir, "lnwdeck_0.2.0_portable.zip"), "zip");

  assert.throws(
    () =>
      buildUpdaterJson({ tag: "v0.2.0", repo: "x/y", assetsDir: dir }),
    /no signed NSIS installers found/,
  );
});

test("buildUpdaterJson rejects an invalid tag", () => {
  const dir = fixtureDir();
  assert.throws(
    () => buildUpdaterJson({ tag: "0.2.0", repo: "x/y", assetsDir: dir }),
    /invalid tag/,
  );
});
