import { describe, expect, it, beforeAll } from "vitest";
import { execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

/**
 * End-to-end pipeline test.
 *
 * A real collector runs against real fixture files through the compiled Rust
 * harness, the results are written to a real SQLite database, and the exported
 * rows are inspected. The previous version of this suite wrote strings itself
 * and asserted that those strings contained no secrets, which proved nothing.
 */

const REPO_ROOT = resolve(__dirname, "..");
let workspace: string;

function runHarness(args: string[]): string {
  return execFileSync(
    "cargo",
    ["run", "--quiet", "--package", "lnwdeck-e2e-harness", "--", ...args],
    { cwd: REPO_ROOT, encoding: "utf-8", timeout: 600_000 },
  );
}

describe("collector pipeline", () => {
  beforeAll(() => {
    workspace = mkdtempSync(join(tmpdir(), "lnwdeck-e2e-"));
  });

  it("ingests real fixture sessions and stores only metadata", () => {
    const sessions = join(workspace, "claude", "projects", "demo");
    mkdirSync(sessions, { recursive: true });

    const now = new Date();
    const line = (minutesAgo: number, input: number, output: number) =>
      JSON.stringify({
        type: "assistant",
        timestamp: new Date(now.getTime() - minutesAgo * 60_000).toISOString(),
        cwd: "C:\\Users\\person\\secret-project",
        message: {
          id: "msg_1",
          role: "assistant",
          model: "claude-e2e",
          content: "this response text must never be stored",
          usage: { input_tokens: input, output_tokens: output },
        },
      });

    writeFileSync(
      join(sessions, "session.jsonl"),
      [line(5, 300, 100), line(30, 50, 25)].join("\n"),
      "utf-8",
    );

    const dbPath = join(workspace, "e2e.db");
    const output = runHarness([
      "--db",
      dbPath,
      "--claude-projects",
      join(workspace, "claude", "projects"),
      "--export",
      join(workspace, "export.json"),
    ]);

    const summary = JSON.parse(output) as {
      events_inserted: number;
      providers: string[];
      quota_windows: number;
      privacy_rejections: number;
    };

    expect(summary.events_inserted).toBeGreaterThan(0);
    expect(summary.providers).toContain("anthropic_claude");
    expect(summary.privacy_rejections).toBe(0);
    // Local session usage is not a provider-reported quota. Without a Claude
    // OAuth credential fixture, the pipeline must not invent quota windows.
    expect(summary.quota_windows).toBe(0);

    const exported = readFileSync(join(workspace, "export.json"), "utf-8");
    expect(exported).toContain("anthropic_claude");
    expect(exported).toContain("claude-e2e");
    // Nothing from the source content may survive into stored data.
    expect(exported).not.toContain("secret-project");
    expect(exported).not.toContain("must never be stored");
    expect(exported).not.toMatch(/C:\\Users/);
    expect(exported).not.toMatch(/Bearer /);
    expect(exported).not.toMatch(/sk-[A-Za-z0-9]/);
  });

  it("re-running the same scan does not double count usage", () => {
    const dbPath = join(workspace, "e2e.db");
    const output = runHarness([
      "--db",
      dbPath,
      "--claude-projects",
      join(workspace, "claude", "projects"),
    ]);
    const summary = JSON.parse(output) as {
      events_inserted: number;
      duplicates_skipped: number;
    };
    expect(summary.events_inserted).toBe(0);
    expect(summary.duplicates_skipped).toBeGreaterThan(0);
  });

  it("a provider without a source is recorded as unavailable, not as success", () => {
    const dbPath = join(workspace, "missing.db");
    const output = runHarness([
      "--db",
      dbPath,
      "--claude-projects",
      join(workspace, "does-not-exist"),
    ]);
    const summary = JSON.parse(output) as {
      events_inserted: number;
      error_codes: string[];
    };
    expect(summary.events_inserted).toBe(0);
    expect(summary.error_codes).toContain("SOURCE_UNAVAILABLE");
  });
});
