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

test("pet quota and speech bubbles stay compact and inside the viewport", async () => {
  await page.waitForLoadState("domcontentloaded");
  await page.addStyleTag({ path: "src/windows/pet/DesktopPet.css" });
  const metrics = await page.evaluate(() => {
    const fixture = document.createElement("div");
    fixture.style.cssText =
      "position:fixed;left:50%;top:50%;transform:translate(-50%,-50%);z-index:9999";
    fixture.innerHTML = `
      <div class="pet-tooltip" data-e2e-tooltip>
        <div class="pet-tooltip-inner">
          <div class="pet-tooltip-bars">
            <div class="pet-tooltip-bar-row">
              <span class="pet-tooltip-bar-label">
                <span class="pet-tooltip-bar-provider">OpenCode (Go)</span>
                <span class="pet-tooltip-bar-window">30-day</span>
              </span>
              <span class="pet-tooltip-bar-track"></span>
              <span class="pet-tooltip-bar-pct">21%</span>
            </div>
          </div>
        </div>
      </div>
      <div class="pet-tooltip" data-e2e-speech>
        <div class="pet-tooltip-inner pet-tooltip-speech">
          <span class="pet-tooltip-empty">Hi!</span>
        </div>
      </div>`;
    document.body.appendChild(fixture);

    const quota = fixture.querySelector<HTMLElement>("[data-e2e-tooltip] .pet-tooltip-inner")!;
    const provider = fixture.querySelector<HTMLElement>(".pet-tooltip-bar-provider")!;
    const track = fixture.querySelector<HTMLElement>(".pet-tooltip-bar-track")!;
    const speech = fixture.querySelector<HTMLElement>(".pet-tooltip-speech")!;
    const quotaRect = quota.getBoundingClientRect();
    const providerText = document.createRange();
    providerText.selectNodeContents(provider);
    const gap = track.getBoundingClientRect().left - providerText.getBoundingClientRect().right;
    const result = {
      gap,
      quotaLeft: quotaRect.left,
      quotaRight: quotaRect.right,
      speechWidth: speech.getBoundingClientRect().width,
      viewportWidth: window.innerWidth,
    };
    fixture.remove();
    return result;
  });

  expect(metrics.gap).toBeGreaterThanOrEqual(0);
  expect(metrics.gap).toBeLessThanOrEqual(20);
  expect(metrics.quotaLeft).toBeGreaterThanOrEqual(0);
  expect(metrics.quotaRight).toBeLessThanOrEqual(metrics.viewportWidth);
  expect(metrics.speechWidth).toBeLessThan(160);
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

test("refresh stays responsive during rapid sidebar navigation", async () => {
  test.setTimeout(120_000);
  await page.waitForLoadState("domcontentloaded");

  const refreshButton = page.locator("header.app-topbar .app-topbar-actions button");
  await expect(refreshButton).toBeVisible();
  await expect(refreshButton).toBeEnabled({ timeout: 15_000 });
  await refreshButton.click({ timeout: 3_000 });

  const routes: Array<[string, string]> = [
    ["Overview", "ภาพรวม"],
    ["Providers", "ผู้ให้บริการ"],
    ["Analytics", "การวิเคราะห์"],
    ["Costs", "ค่าใช้จ่าย"],
    ["Budgets", "งบประมาณ"],
    ["Models", "โมเดล"],
    ["Sessions", "เซสชัน"],
    ["Alerts", "การแจ้งเตือน"],
    ["Pet", "สัตว์เลี้ยง"],
    ["Settings", "ตั้งค่า"],
    ["System", "ระบบ"],
  ];

  for (let round = 0; round < 6; round += 1) {
    for (const [en, th] of routes) {
      const name = new RegExp(`${en}|${th}`);
      await page.getByRole("link", { name, exact: true }).click({ timeout: 3_000 });
      await expect(
        page.getByRole("heading", { name }).first(),
      ).toBeVisible({ timeout: 3_000 });
    }
  }

  // The shell must still accept input after the burst; return to Overview and
  // ensure the shared refresh job eventually leaves its refreshing state.
  await page
    .getByRole("link", { name: /Overview|ภาพรวม/, exact: true })
    .click({ timeout: 3_000 });
  await expect(
    page.getByRole("heading", { name: /Overview|ภาพรวม/ }).first(),
  ).toBeVisible({ timeout: 3_000 });
  await expect(refreshButton).toBeEnabled({ timeout: 70_000 });
});

test("switching the UI language to Thai applies immediately", async () => {
  await page.getByRole("link", { name: /Settings|ตั้งค่า/, exact: true }).click();
  await page.getByLabel(/Language|ภาษา/).selectOption("th");
  await expect(page.getByRole("heading", { name: "ตั้งค่า" }).first()).toBeVisible();
});
