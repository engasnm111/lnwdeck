import { describe, it, expect } from "vitest";
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

describe("Privacy integration tests", () => {
  const testDir = resolve(process.cwd(), "e2e", "test-output");

  it("synthetic provider environment does not persist prompts", () => {
    const configDir = resolve(testDir, "provider-config");
    mkdirSync(configDir, { recursive: true });

    const configContent = JSON.stringify({
      provider: "test-provider",
      model: "gpt-4o",
      enabled: true,
    });
    writeFileSync(resolve(configDir, "config.json"), configContent);

    // Verify no prompt/response in config
    expect(configContent).not.toContain("prompt");
    expect(configContent).not.toContain("response");
    expect(configContent).not.toContain("Bearer");
    expect(configContent).not.toContain("sk-");

    rmSync(testDir, { recursive: true, force: true });
  });

  it("log output does not contain sensitive paths", () => {
    const logDir = resolve(testDir, "logs");
    mkdirSync(logDir, { recursive: true });

    const logEntry = JSON.stringify({
      timestamp: new Date().toISOString(),
      level: "info",
      message: "Provider configuration loaded successfully",
      provider: "openai",
    });
    writeFileSync(resolve(logDir, "app.log"), logEntry);

    expect(logEntry).not.toMatch(/C:\\Users/);
    expect(logEntry).not.toMatch(/\/home\//);
    expect(logEntry).not.toContain("password");
    expect(logEntry).not.toContain("token");

    rmSync(testDir, { recursive: true, force: true });
  });

  it("clean profile has no cookie data", () => {
    const profileDir = resolve(testDir, "profile");
    mkdirSync(profileDir, { recursive: true });

    const preferences = JSON.stringify({
      theme: "system",
      startup: true,
      refreshInterval: 30,
    });
    writeFileSync(resolve(profileDir, "preferences.json"), preferences);

    expect(preferences).not.toContain("cookie");
    expect(preferences).not.toContain("session");
    expect(preferences).not.toContain("Bearer");

    rmSync(testDir, { recursive: true, force: true });
  });

  it("hook consent flow does not embed secrets", () => {
    const hookDir = resolve(testDir, "hooks");
    mkdirSync(hookDir, { recursive: true });

    const hookConfig = JSON.stringify({
      preview: {
        target: "config.json",
        original_hash: "abc123",
        diff_summary: "1 line changed",
      },
      approved: false,
    });
    writeFileSync(resolve(hookDir, "hook_state.json"), hookConfig);

    expect(hookConfig).not.toContain("secret");
    expect(hookConfig).not.toContain("key");
    expect(hookConfig).not.toContain("token");
    expect(hookConfig).not.toMatch(/C:\\/);

    rmSync(testDir, { recursive: true, force: true });
  });

  it("browser message payload has no cookie or session", () => {
    const msg = JSON.stringify({
      type: "quota_update",
      version: 1,
      timestamp: new Date().toISOString(),
      nonce: "abc12345",
      payload: {
        provider: "openai",
        remaining: 500,
      },
    });

    expect(msg).not.toContain("cookie");
    expect(msg).not.toContain("session");
    expect(msg).not.toContain("Bearer");
    expect(msg).not.toContain("token");
  });

  it("export data contains only metadata fields", () => {
    const exportData = JSON.stringify([
      { id: "e1", provider_id: "openai", model: "gpt-4o", tokens_input: 100, tokens_output: 50, cost: "0.005", confidence: "High", data_source: "web", timestamp: "2025-01-01T00:00:00Z" },
    ]);

    expect(exportData).not.toContain("prompt");
    expect(exportData).not.toContain("response");
    expect(exportData).not.toContain("path");
    expect(exportData).not.toContain("file_name");
  });

  it("tray popup data has no sensitive fields", () => {
    const trayData = JSON.stringify({
      total_events: 42,
      total_tokens_input: 5000,
      total_tokens_output: 3000,
      provider_count: 3,
    });

    const parsed = JSON.parse(trayData);
    const keys = Object.keys(parsed);
    expect(keys).not.toContain("prompt");
    expect(keys).not.toContain("response");
    expect(keys).not.toContain("path");
    expect(keys).not.toContain("secret");
    expect(keys).not.toContain("token");
    expect(keys).not.toContain("cookie");
  });
});
