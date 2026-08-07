import { readFileSync } from "node:fs";
import { test } from "node:test";
import assert from "node:assert/strict";

const workflow = readFileSync(
  new URL("../.github/workflows/ci.yml", import.meta.url),
  "utf8",
);
const retryScript = readFileSync(
  new URL("./ci-retry.ps1", import.meta.url),
  "utf8",
);
const workflowFiles = [
  ".github/workflows/ci.yml",
  ".github/workflows/security.yml",
  ".github/workflows/release.yml",
].map((file) => [
  file,
  readFileSync(new URL(`../${file}`, import.meta.url), "utf8"),
]);

function jobSectionFrom(source, name, nextJob = null) {
  const normalizedSource = source.replace(/\r\n?/g, "\n");
  const start = normalizedSource.indexOf(`  ${name}:\n`);
  assert.notEqual(start, -1, `CI job ${name} should exist`);
  const nextStart =
    nextJob === null
      ? -1
      : normalizedSource.indexOf(`  ${nextJob}:\n`, start + 1);
  return normalizedSource.slice(
    start,
    nextStart === -1 ? normalizedSource.length : nextStart,
  );
}

function jobSection(name, nextJob = null) {
  return jobSectionFrom(workflow, name, nextJob);
}

test("CI workflow contract handles Windows line endings", () => {
  const windowsLineEndingWorkflow = workflow.replace(/\n/g, "\r\n");
  assert.match(
    jobSectionFrom(windowsLineEndingWorkflow, "e2e-ui", "compile"),
    /runs-on: windows-latest/,
  );
});

test("build-heavy CI jobs use the runner proven by the release workflow", () => {
  assert.match(jobSection("e2e-ui", "compile"), /runs-on: windows-latest/);
  assert.match(jobSection("compile"), /runs-on: windows-latest/);
});

test("build jobs prepare the native messaging host before consuming it", () => {
  const e2e = jobSection("e2e-ui", "compile");
  assert.match(e2e, /cargo build -p lnwdeck-native-messaging-host --release/);

  const compile = jobSection("compile");
  assert.match(
    compile,
    /cargo build -p lnwdeck-native-messaging-host --target \$\{\{ matrix\.target \}\} --release/,
  );
});

test("architecture compile checks do not build every test target", () => {
  const compile = jobSection("compile");
  assert.match(compile, /cargo check --workspace --target/);
  assert.doesNotMatch(compile, /cargo check --workspace --all-targets/);
});

test("all project workflows use the supported Node 24 line", () => {
  for (const [file, content] of workflowFiles) {
    assert.match(content, /node-version:\s*24\b/, file);
    assert.doesNotMatch(content, /node-version:\s*22\b/, file);
  }
});

test("CI retry is limited to silent build-progress failures", () => {
  assert.match(
    retryScript,
    /\$retryableFailure\s*=\s*\(\$code\s*-eq\s*1\)\s*-or\s*\(\$code\s*-eq\s*101\)/,
  );
  assert.match(retryScript, /\$hasDiagnostic/);
  assert.match(retryScript, /\$hasTestOutput/);
  assert.match(
    retryScript,
    /\$canRetry\s*=\s*\$retryableFailure\s*-and\s*-not\s*\$hasDiagnostic\s*-and\s*-not\s*\$hasTestOutput/,
  );
  assert.match(retryScript, /\$attemptsUsed/);
});
