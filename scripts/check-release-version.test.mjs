import { test } from "node:test";
import assert from "node:assert/strict";
import {
  checkVersions,
  isSemVer,
  loadDeclaredVersions,
  normalizeTag,
} from "./check-release-version.mjs";

test("normalizeTag strips a leading v", () => {
  assert.equal(normalizeTag("v0.2.0"), "0.2.0");
  assert.equal(normalizeTag("0.2.0"), "0.2.0");
  assert.equal(normalizeTag("  v0.2.0-rc.1  "), "0.2.0-rc.1");
  assert.equal(normalizeTag(""), null);
  assert.equal(normalizeTag(undefined), null);
});

test("isSemVer accepts stable and prerelease versions", () => {
  assert.equal(isSemVer("0.2.0"), true);
  assert.equal(isSemVer("0.2.0-rc.1"), true);
  assert.equal(isSemVer("0.2.0-beta.2+build.5"), true);
  assert.equal(isSemVer("0.2"), false);
  assert.equal(isSemVer("v0.2.0"), false, "prefix must be stripped first");
  assert.equal(isSemVer("version"), false);
});

test("checkVersions passes when every source matches the tag", () => {
  const declared = {
    tauri: "0.2.0",
    cargo: "0.2.0",
    installer: "0.2.0",
  };
  const result = checkVersions("v0.2.0", declared);
  assert.equal(result.ok, true);
  assert.equal(result.version, "0.2.0");
  assert.deepEqual(result.errors, []);
});

test("checkVersions supports prerelease tags", () => {
  const declared = {
    tauri: "0.2.0-rc.1",
    cargo: "0.2.0-rc.1",
    installer: "0.2.0-rc.1",
  };
  const result = checkVersions("v0.2.0-rc.1", declared);
  assert.equal(result.ok, true);
});

test("checkVersions fails with per-file messages on mismatch", () => {
  const declared = {
    tauri: "0.1.0",
    cargo: "0.2.0",
    installer: "0.2.0",
  };
  const result = checkVersions("v0.2.0", declared);
  assert.equal(result.ok, false);
  assert.equal(result.errors.length, 1);
  assert.match(result.errors[0], /tauri\.conf\.json declares "0\.1\.0"/);
});

test("checkVersions rejects empty or invalid tags", () => {
  const declared = {
    tauri: "0.2.0",
    cargo: "0.2.0",
    installer: "0.2.0",
  };
  assert.equal(checkVersions("", declared).ok, false);
  assert.equal(checkVersions("v2.0", declared).ok, false);
});

test("loadDeclaredVersions reads the real repository files", () => {
  const declared = loadDeclaredVersions();
  assert.match(declared.tauri, /^[0-9]+\.[0-9]+\.[0-9]+/);
  assert.match(declared.cargo, /^[0-9]+\.[0-9]+\.[0-9]+/);
  assert.match(declared.installer, /^[0-9]+\.[0-9]+\.[0-9]+/);
  assert.equal(declared.tauri, declared.cargo);
  assert.equal(declared.cargo, declared.installer);
});
