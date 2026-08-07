import { defineConfig } from "@playwright/test";

/**
 * Playwright config for the lnwdeck desktop app. Tests run against a real
 * release build of the application through tauri-driver, which exposes the
 * WebView2 frontend as a WebDriver session on port 4444.
 */
export default defineConfig({
  testDir: "./e2e",
  timeout: 90_000,
  retries: 0,
  workers: 1,
  reporter: [["list"]],
  use: {
    baseURL: "tauri://localhost",
  },
});
