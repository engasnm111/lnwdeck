import { readFileSync } from "node:fs";
import { test } from "node:test";
import assert from "node:assert/strict";

const workflow = readFileSync(
  new URL("../.github/workflows/ci.yml", import.meta.url),
  "utf8",
);

function jobSection(name, nextJob = null) {
  const start = workflow.indexOf(`  ${name}:\n`);
  assert.notEqual(start, -1, `CI job ${name} should exist`);
  const nextStart =
    nextJob === null ? -1 : workflow.indexOf(`  ${nextJob}:\n`, start + 1);
  return workflow.slice(start, nextStart === -1 ? workflow.length : nextStart);
}

test("build-heavy CI jobs use the runner proven by the release workflow", () => {
  assert.match(jobSection("e2e-ui", "compile"), /runs-on: windows-latest/);
  assert.match(jobSection("compile"), /runs-on: windows-latest/);
});

test("architecture compile checks do not build every test target", () => {
  const compile = jobSection("compile");
  assert.match(compile, /cargo check --workspace --target/);
  assert.doesNotMatch(compile, /cargo check --workspace --all-targets/);
});
