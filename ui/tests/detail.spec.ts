import { expect, test } from "@playwright/test";
import {
  detailHeading,
  fixtureCaptures,
  installCaptures,
  installTheme,
  showCapture,
} from "./support";

const webhook = "0193f2c4-7a1e-7c3d-9f42-6b1d2e8a51c7";
const order = "0193f2c4-7a1e-7c3d-9f42-6b1d2e8a51c8";
const refund = "0193f2c4-7a1e-7c3d-9f42-6b1d2e8a51c9";

test("moving to another request keeps one on screen", async ({
  page,
}, testInfo) => {
  await installTheme(page, testInfo);
  await installCaptures(page);
  await page.route(
    (url) => /\/api\/captures\/[^/]+$/.test(url.pathname),
    async (route) => {
      await new Promise((done) => setTimeout(done, 200));
      await route.fallback();
    },
  );
  await showCapture(page, webhook);
  await expect(detailHeading(page)).toContainText("/webhooks/");
  await page.evaluate(() => {
    const watch = window as unknown as { blankFrames: number };
    watch.blankFrames = 0;
    const tick = () => {
      if (document.querySelector("[data-detail] h1") === null) {
        watch.blankFrames += 1;
      }
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  });
  expect(fixtureCaptures()[1]?.id).toBe(order);
  await page.evaluate((id) => {
    window.history.pushState(null, "", `/r/${id}`);
    window.dispatchEvent(new PopStateEvent("popstate"));
  }, order);
  await expect(detailHeading(page)).toContainText("/v1/orders/");
  const blank = await page.evaluate(
    () => (window as unknown as { blankFrames: number }).blankFrames,
  );
  expect(blank).toBe(0);
});

test("a failed outgoing call shows its error", async ({ page }, testInfo) => {
  await installTheme(page, testInfo);
  await installCaptures(page);
  await showCapture(page, refund);
  await expect(detailHeading(page)).toBeVisible();
  await page.getByRole("tab", { name: /Outgoing/ }).click();
  const call = page.getByRole("button", { name: /4011\/refunds/ });
  await expect(call).toContainText("error");
  await expect(call).not.toContainText("panicked");
  await call.click();
  await expect(
    page.getByText("error sending request: connection refused (os error 61)"),
  ).toBeVisible();
  const line = page.getByText("id = rf_2042");
  expect(await line.evaluate((element) => element.textContent)).toBe(
    "    id = rf_2042",
  );
  expect(
    await line.evaluate((element) => getComputedStyle(element).whiteSpace),
  ).toBe("pre-wrap");
});

test("copy buttons are reachable by keyboard and confirm the copy", async ({
  page,
  context,
}, testInfo) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await installTheme(page, testInfo);
  await installCaptures(page);
  await showCapture(page, refund);
  await expect(detailHeading(page)).toBeVisible();
  await page.getByRole("tab", { name: "Request", exact: true }).focus();
  const copy = page.getByRole("button", { name: "Copy content-type" });
  for (let step = 0; step < 12; step += 1) {
    if (await copy.evaluate((element) => element === document.activeElement)) {
      break;
    }
    await page.keyboard.press("Tab");
  }
  await expect(copy).toBeFocused();
  await expect(copy).toHaveCSS("opacity", "1");
  await page.keyboard.press("Enter");
  await expect(
    page.getByRole("status").filter({ hasText: "Copied content-type" }),
  ).toBeVisible();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
    "application/json",
  );
});

test("a long header value opens in place", async ({ page }, testInfo) => {
  await installTheme(page, testInfo);
  await installCaptures(page);
  await showCapture(page, refund);
  const value = page.getByRole("button", { name: /refunds-dashboard/ });
  await expect(value).toHaveAttribute("aria-expanded", "false");
  await value.click();
  await expect(value).toHaveAttribute("aria-expanded", "true");
});

test("empty bodies and masked values say so once", async ({
  page,
}, testInfo) => {
  await installTheme(page, testInfo);
  await installCaptures(page);
  await showCapture(page, order);
  await expect(detailHeading(page)).toBeVisible();
  await expect(page.getByRole("tabpanel").getByText("No body")).toBeVisible();
  await showCapture(page, webhook);
  await expect(detailHeading(page)).toBeVisible();
  await expect(page.getByText("[redacted]")).toHaveCount(0);
  await expect(
    page.getByText("redacted", { exact: true }).first(),
  ).toBeVisible();
});

test("the tab title names the open request", async ({ page }, testInfo) => {
  await installTheme(page, testInfo);
  await installCaptures(page);
  await showCapture(page, webhook);
  await expect(detailHeading(page)).toBeVisible();
  await expect(page).toHaveTitle("500 POST /webhooks/{psp} · Oxray");
  await page.keyboard.press("Escape");
  await expect(page).toHaveTitle("Oxray");
});

test("a single panel has a way back to the list", async ({
  page,
}, testInfo) => {
  await installTheme(page, testInfo);
  await installCaptures(page);
  await showCapture(page, webhook);
  await expect(detailHeading(page)).toBeVisible();
  const back = page.getByRole("link", { name: "Back to requests" });
  if (!testInfo.project.name.endsWith("960")) {
    await expect(back).toHaveCount(0);
    return;
  }
  await expect(back).toHaveAttribute("title", "Back to requests (Esc)");
  await back.click();
  await expect(detailHeading(page)).toHaveCount(0);
  await expect(page.getByText("/webhooks/{psp}").first()).toBeVisible();
});
