import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";
import assert from "node:assert/strict";

const workflow = readFileSync(
  new URL("../.github/workflows/ci.yml", import.meta.url),
  "utf8",
);
const releaseWorkflow = readFileSync(
  new URL("../.github/workflows/release.yml", import.meta.url),
  "utf8",
);
const cargoManifest = readFileSync(
  new URL("../Cargo.toml", import.meta.url),
  "utf8",
);
const cargoConfigPath = new URL("../.cargo/config.toml", import.meta.url);
const cargoConfig = existsSync(cargoConfigPath)
  ? readFileSync(cargoConfigPath, "utf8")
  : "";
const retryScript = readFileSync(
  new URL("./ci-retry.ps1", import.meta.url),
  "utf8",
);
const e2eScript = readFileSync(
  new URL("./run-e2e-ui.ps1", import.meta.url),
  "utf8",
);
const e2eSpec = readFileSync(
  new URL("../apps/desktop/e2e/app.spec.ts", import.meta.url),
  "utf8",
);
const windowsSource = readFileSync(
  new URL("../apps/desktop/src-tauri/src/windows.rs", import.meta.url),
  "utf8",
);
const desktopBuildScript = readFileSync(
  new URL("../apps/desktop/src-tauri/build.rs", import.meta.url),
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

test("Cargo build concurrency is scoped to build-heavy jobs", () => {
  const globalEnv = workflow.slice(
    workflow.indexOf("env:\n"),
    workflow.indexOf("jobs:\n"),
  );
  assert.doesNotMatch(globalEnv, /CARGO_BUILD_JOBS/);
  for (const [name, nextJob] of [
    ["check", "test"],
    ["test", "e2e-ui"],
    ["e2e-ui", "compile"],
    ["compile", null],
  ]) {
    assert.match(
      jobSection(name, nextJob),
      /^    env:\r?\n(?:      [^\r\n]*\r?\n)*      CARGO_BUILD_JOBS:[ \t]*2\b/m,
      `${name} should own its build concurrency setting`,
    );
  }
});

test("Rust caches share CI artifacts without crossing architecture targets", () => {
  for (const [name, nextJob] of [
    ["check", "test"],
    ["test", "e2e-ui"],
    ["e2e-ui", "compile"],
  ]) {
    const section = jobSection(name, nextJob);
    assert.match(section, /shared-key:\s*ci\b/, `${name} should share the CI cache`);
    assert.doesNotMatch(section, /^\s+key:/m);
  }
  const compile = jobSection("compile");
  assert.match(
    compile,
    /shared-key:\s*compile-\$\{\{ matrix\.target \}\}/,
  );
  assert.doesNotMatch(compile, /^\s+key:/m);
});

test("release builds use the release cache, sccache, and only the shipped bundle", () => {
  assert.match(releaseWorkflow, /shared-key:\s*release-\$\{\{ matrix\.target \}\}/);
  assert.match(releaseWorkflow, /cache-workspace-crates:\s*true/);
  assert.match(releaseWorkflow, /mozilla-actions\/sccache-action@/);
  assert.match(releaseWorkflow, /SCCACHE_GHA_ENABLED:\s*["']?true["']?/);
  assert.match(releaseWorkflow, /RUSTC_WRAPPER:\s*sccache/);
  assert.match(releaseWorkflow, /tauri build[^\r\n]*--bundles\s+nsis/);
  assert.doesNotMatch(releaseWorkflow, /tauri-apps\/tauri-action/);
});

test("Cargo release profile and Windows targets are tuned for fast release links", () => {
  assert.match(cargoManifest, /^\[profile\.release\]/m);
  assert.match(cargoManifest, /^lto\s*=\s*false$/m);
  assert.match(cargoManifest, /^codegen-units\s*=\s*16$/m);
  assert.match(cargoManifest, /^incremental\s*=\s*false$/m);
  assert.match(cargoConfig, /\[build\][\s\S]*?incremental\s*=\s*true/);
  for (const target of [
    "x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc",
    "i686-pc-windows-msvc",
  ]) {
    const section = target.replaceAll("-", "\\s*[-_]\\s*");
    assert.match(
      cargoConfig,
      new RegExp(`\\[target\\.${section}\\][\\s\\S]*?linker\\s*=\\s*[\"']rust-lld(?:\\.exe)?[\"']`),
      `${target} should use rust-lld`,
    );
  }
});

test("build jobs prepare the native messaging host before consuming it", () => {
  const e2e = jobSection("e2e-ui", "compile");
  assert.match(e2e, /cargo build -p lnwdeck-native-messaging-host\b(?![^\n]*--release)/);

  const compile = jobSection("compile");
  assert.match(
    compile,
    /cargo build -p lnwdeck-native-messaging-host --target \$\{\{ matrix\.target \}\}(?![^\n]*--release)/,
  );
});

test("architecture compile checks do not build every test target", () => {
  const compile = jobSection("compile");
  assert.match(compile, /cargo check --workspace --target/);
  assert.doesNotMatch(compile, /cargo check --workspace --all-targets/);
});

test("UI build reuses debug Native Host artifacts and skips ARM64 LLVM setup", () => {
  const e2e = jobSection("e2e-ui", "compile");
  assert.doesNotMatch(e2e, /Install LLVM for ARM64 cross-compilation/);
  assert.match(e2e, /cargo build -p lnwdeck-native-messaging-host\b(?![^\n]*--release)/);
  assert.match(e2e, /target\/debug\/lnwdeck-browser-host\.exe/);
  assert.match(e2e, /tauri build --debug --no-bundle/);
  assert.match(e2e, /LNWD_E2E_BUILD_PROFILE:\s*debug/);

  const compile = jobSection("compile");
  assert.match(
    compile,
    /cargo build -p lnwdeck-native-messaging-host --target \$\{\{ matrix\.target \}\}(?![^\n]*--release)/,
  );
  assert.match(compile, /target\/\$\{\{ matrix\.target \}\}\/debug\/lnwdeck-browser-host\.exe/);
  assert.match(desktopBuildScript, /CARGO_PROFILE|PROFILE/);
  assert.match(desktopBuildScript, /["']debug["']/);
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

test("UI smoke does not retry runtime startup failures", () => {
  const e2e = jobSection("e2e-ui", "compile");
  assert.match(e2e, /pnpm --filter @lnwdeck\/desktop run e2e:run/);
  assert.doesNotMatch(e2e, /ci-retry\.ps1[^\r\n]*e2e:run/);
});

test("WebView2 smoke uses a dynamic endpoint and reports startup state", () => {
  assert.ok(e2eScript.includes("TcpListener"));
  assert.ok(e2eScript.includes('LNWD_E2E_CDP_PORT = "$port"'));
  assert.ok(e2eScript.includes("-WorkingDirectory $repo"));
  assert.ok(e2eScript.includes("$app.HasExited"));
  assert.ok(e2eScript.includes("exit code"));
  assert.ok(e2eSpec.includes("LNWD_E2E_CDP_PORT"));
});

test("WebView2 CDP is configured programmatically for every desktop webview", () => {
  assert.match(windowsSource, /LNWD_E2E_CDP_PORT/);
  assert.match(windowsSource, /additional_browser_args/);
  assert.match(windowsSource, /remote-debugging-port/);
  assert.equal(
    (windowsSource.match(/additional_browser_args/g) ?? []).length,
    4,
    "main, widget, pet, and tray must receive the same browser arguments",
  );
});

test("WebView2 smoke passes only the CDP port and uses a real deadline", () => {
  assert.ok(e2eScript.includes("LNWD_E2E_CDP_PORT"));
  assert.ok(e2eScript.includes("AddSeconds(60)"));
  assert.ok(e2eScript.includes("Start-Sleep -Milliseconds 250"));
  assert.doesNotMatch(e2eScript, /WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS/);
  assert.doesNotMatch(e2eScript, /AdditionalBrowserArguments/);
  assert.doesNotMatch(e2eScript, /New-ItemProperty|Remove-ItemProperty/);
  assert.doesNotMatch(e2eScript, /WEBVIEW2_USER_DATA_FOLDER/);
});
