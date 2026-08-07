import { test, expect, chromium, type Browser, type Page } from "@playwright/test";

/**
 * Main workflow smoke test for the desktop app.
 *
 * The app is started with WebView2 remote debugging enabled
 * (WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=<port>) by
 * the e2e:run script. Playwright attaches to that CDP endpoint, so the tests
 * drive the real WebView2 frontend talking to the real Tauri backend.
 */

let browser: Browser;
let page: Page;

test.beforeAll(async () => {
  const cdpPort = process.env.LNWD_E2E_CDP_PORT ?? "9222";
  browser = await chromium.connectOverCDP("http://127.0.0.1:" + cdpPort);
  const context = browser.contexts()[0];
  const pages = context.pages();
  // The app runs three windows (main, pet, widget); pick the dashboard one.
  page =
    pages.find(
      (p) => p.url().endsWith("/") || p.url().includes("/index.html"),
    ) ?? pages[0];
  await page.bringToFront();
});

test.afterAll(async () => {
  await browser?.close();
});

test("app boots and renders the dashboard", async () => {
  await page.waitForLoadState("domcontentloaded");
  await expect(
    page.getByRole("heading", { name: /Overview|ภาพรวม/ }).first(),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: /Refresh all providers|รีเฟรชผู้ให้บริการทั้งหมด/ }),
  ).toBeVisible();
});

test("sidebar navigation reaches every page", async () => {
  const labels: Array<[string, string]> = [
    ["Providers", "ผู้ให้บริการ"],
    ["Analytics", "การวิเคราะห์"],
    ["Costs", "ค่าใช้จ่าย"],
    ["Budgets", "งบประมาณ"],
    ["Models", "โมเดล"],
    ["Alerts", "การแจ้งเตือน"],
    ["Pet", "สัตว์เลี้ยง"],
    ["Settings", "ตั้งค่า"],
    ["System", "ระบบ"],
  ];
  for (const [en, th] of labels) {
    const link = page.getByRole("link", {
      name: new RegExp(`${en}|${th}`),
      exact: true,
    });
    await link.click();
    await expect(
      page.getByRole("heading", { name: new RegExp(`${en}|${th}`) }).first(),
    ).toBeVisible({ timeout: 15_000 });
  }
});

test("switching the UI language to Thai applies immediately", async () => {
  await page.getByRole("link", { name: /Settings|ตั้งค่า/, exact: true }).click();
  await page.getByLabel(/Language|ภาษา/).selectOption("th");
  await expect(page.getByRole("heading", { name: "ตั้งค่า" }).first()).toBeVisible();
});
