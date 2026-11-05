import type { Page } from "@playwright/test";
import { expect, test } from "@playwright/test";
import { installCaptures, installTheme } from "./support";

function detailError(page: Page) {
  return page
    .getByRole("region", { name: "Request", exact: true })
    .getByText("Couldn't load captures. Retrying…");
}

async function failDetails(page: Page): Promise<void> {
  await page.route(
    (url) => /\/api\/captures\/[^/]+$/.test(url.pathname),
    async (route) => {
      await route.fulfill({
        status: 500,
        contentType: "text/plain",
        body: "nope",
      });
    },
  );
}

async function openCapture(page: Page, id: string): Promise<void> {
  await page.evaluate((captureId) => {
    window.history.pushState(null, "", `/r/${captureId}`);
    window.dispatchEvent(new PopStateEvent("popstate"));
  }, id);
}

test("keyboard opens a request and changes the theme", async ({
  page,
}, testInfo) => {
  await installTheme(page, testInfo);
  await installCaptures(page);
  await page.goto("./");
  await expect(page.getByText("/webhooks/{psp}").first()).toBeVisible();
  const starting = testInfo.project.name.startsWith("dark") ? "Dark" : "Light";
  await page.keyboard.press("t");
  const next = starting === "Dark" ? "Light" : "System";
  await expect(
    page.getByRole("button", { name: `Theme: ${next}` }),
  ).toBeVisible();
  await page.keyboard.press("j");
  if (testInfo.project.name.endsWith("960")) {
    await page.keyboard.press("Enter");
  }
  await expect(
    page.getByRole("heading", { name: /\/webhooks\/\{psp\}/ }),
  ).toBeVisible();
  await page.getByRole("tab", { name: /Queries/ }).click();
  await expect(
    page.getByText("select id from payments where pan = ?"),
  ).toBeVisible();
  if (testInfo.project.name.endsWith("960")) {
    await page.keyboard.press("Escape");
    await expect(
      page.getByRole("heading", { name: /\/webhooks\/\{psp\}/ }),
    ).toBeHidden();
  }
});

test("arrow keys move focus across tabs", async ({ page }, testInfo) => {
  await installTheme(page, testInfo);
  await installCaptures(page);
  await page.goto("./");
  await expect(page.getByText("/webhooks/{psp}").first()).toBeVisible();
  await page.keyboard.press("j");
  if (testInfo.project.name.endsWith("960")) {
    await page.keyboard.press("Enter");
  }
  const request = page.getByRole("tab", { name: "Request", exact: true });
  await request.focus();
  await page.keyboard.press("ArrowRight");
  await expect(
    page.getByRole("tab", { name: "Response", exact: true }),
  ).toBeFocused();
  await page.keyboard.press("ArrowRight");
  await expect(page.getByRole("tab", { name: /Queries/ })).toBeFocused();
});

test("enter activates controls and the first request", async ({
  page,
}, testInfo) => {
  await installTheme(page, testInfo);
  await installCaptures(page);
  await page.goto("./");
  await expect(page.getByText("/webhooks/{psp}").first()).toBeVisible();
  const starting = testInfo.project.name.startsWith("dark") ? "Dark" : "Light";
  const theme = page.getByRole("button", { name: `Theme: ${starting}` });
  await theme.focus();
  await page.keyboard.press("Enter");
  const next = starting === "Dark" ? "Light" : "System";
  await expect(
    page.getByRole("button", { name: `Theme: ${next}` }),
  ).toBeVisible();
  await expect(page.getByRole("heading")).toHaveCount(0);
  await page.locator("header p").first().click();
  await page.keyboard.press("Enter");
  await expect(
    page.getByRole("heading", { name: /\/webhooks\/\{psp\}/ }),
  ).toBeVisible();
  if (testInfo.project.name.endsWith("960")) {
    await page.getByRole("button", { name: "oxray", exact: true }).focus();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("heading")).toHaveCount(0);
  }
});

test("a failed detail load retries", async ({ page }, testInfo) => {
  await installTheme(page, testInfo);
  await installCaptures(page);
  await failDetails(page);
  let listHits = 0;
  let detailHits = 0;
  page.on("request", (request) => {
    const pathname = new URL(request.url()).pathname;
    if (pathname.endsWith("/api/captures")) {
      listHits += 1;
    } else if (/\/api\/captures\/[^/]+$/.test(pathname)) {
      detailHits += 1;
    }
  });
  await page.goto("./");
  await expect(page.getByText("/webhooks/{psp}").first()).toBeVisible();
  await openCapture(page, "0193f2c4-7a1e-7c3d-9f42-6b1d2e8a51c7");
  await expect(detailError(page)).toBeVisible();
  const lists = listHits;
  const before = detailHits;
  await expect
    .poll(() => detailHits, { timeout: 4_000 })
    .toBeGreaterThan(before);
  expect(listHits).toBe(lists);
});

test("a new capture starts its retry backoff over", async ({
  page,
}, testInfo) => {
  await page.clock.install();
  await installTheme(page, testInfo);
  await installCaptures(page);
  await failDetails(page);
  let detailHits = 0;
  page.on("request", (request) => {
    if (/\/api\/captures\/[^/]+$/.test(new URL(request.url()).pathname)) {
      detailHits += 1;
    }
  });
  await page.goto("./");
  await expect(page.getByText("/webhooks/{psp}").first()).toBeVisible();
  await openCapture(page, "0193f2c4-7a1e-7c3d-9f42-6b1d2e8a51c7");
  await expect(detailError(page)).toBeVisible();
  await page.clock.fastForward(1_000);
  await expect.poll(() => detailHits).toBeGreaterThanOrEqual(2);
  await page.clock.fastForward(2_000);
  await expect.poll(() => detailHits).toBeGreaterThanOrEqual(3);
  await page.clock.fastForward(4_000);
  await expect.poll(() => detailHits).toBeGreaterThanOrEqual(4);
  const beforeSwitch = detailHits;
  await openCapture(page, "0193f2c4-7a1e-7c3d-9f42-6b1d2e8a51c8");
  await expect.poll(() => detailHits).toBeGreaterThan(beforeSwitch);
  const afterLoad = detailHits;
  await page.clock.fastForward(1_000);
  await expect.poll(() => detailHits).toBeGreaterThan(afterLoad);
});

test("refresh asks for the capture list again", async ({ page }, testInfo) => {
  await installTheme(page, testInfo);
  await installCaptures(page);
  let hits = 0;
  page.on("request", (request) => {
    const pathname = new URL(request.url()).pathname;
    if (pathname.endsWith("/api/captures")) {
      hits += 1;
    }
  });
  await page.goto("./");
  await expect(page.getByText("/webhooks/{psp}").first()).toBeVisible();
  const before = hits;
  await page.keyboard.press("r");
  await expect.poll(() => hits).toBeGreaterThan(before);
});
