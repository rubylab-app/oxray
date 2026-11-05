import type { Page, TestInfo } from "@playwright/test";
import { expect, test } from "@playwright/test";
import {
  fontsReady,
  installCaptures,
  installFailure,
  installHang,
  prepare,
  showCapture,
} from "./support";

async function shot(
  page: Page,
  testInfo: TestInfo,
  name: string,
): Promise<void> {
  await fontsReady(page);
  await expect(page).toHaveScreenshot(`${name}.png`);
  void testInfo;
}

test("@visual empty", async ({ page }, testInfo) => {
  await prepare(page, testInfo);
  await page.goto("./");
  await expect(page.getByText("Waiting for the first request…")).toBeVisible();
  await shot(page, testInfo, "empty");
});

test("@visual list", async ({ page }, testInfo) => {
  await prepare(page, testInfo);
  await installCaptures(page);
  await page.goto("./");
  await expect(page.getByText("failed in the last 5 min")).toBeVisible();
  if (!testInfo.project.name.endsWith("960")) {
    await expect(page.getByText("Select a request")).toBeVisible();
  }
  await shot(page, testInfo, "list");
});

test("@visual loading", async ({ page }, testInfo) => {
  await prepare(page, testInfo);
  await installHang(page);
  await page.goto("./");
  await expect(page.getByText("oxray", { exact: true })).toBeVisible();
  await page.clock.fastForward(400);
  await expect(page.locator("[aria-busy='true']")).toBeVisible();
  await shot(page, testInfo, "loading");
});

test("@visual error", async ({ page }, testInfo) => {
  await prepare(page, testInfo);
  await installFailure(page);
  await page.goto("./");
  await expect(
    page.getByText("Couldn't load captures. Retrying…"),
  ).toBeVisible();
  await shot(page, testInfo, "error");
});

test("@visual redacted", async ({ page }, testInfo) => {
  await openDetail(page, testInfo, "0193f2c4-7a1e-7c3d-9f42-6b1d2e8a51c7");
  await expect(page.getByText("•••• 4242")).toBeVisible();
  await expect(
    page.getByText("redacted", { exact: true }).first(),
  ).toBeVisible();
  await shot(page, testInfo, "redacted");
});

test("@visual truncated", async ({ page }, testInfo) => {
  await openDetail(page, testInfo, "0193f2c4-7a1e-7c3d-9f42-6b1d2e8a51cc");
  await expect(
    page.getByText("Body truncated at 64.0 KB of 1.3 MB"),
  ).toBeVisible();
  await shot(page, testInfo, "truncated");
});

test("@visual omitted", async ({ page }, testInfo) => {
  await openDetail(page, testInfo, "0193f2c4-7a1e-7c3d-9f42-6b1d2e8a51cd");
  await expect(
    page.getByText("Binary body omitted (image/png, 482.0 KB)"),
  ).toBeVisible();
  await shot(page, testInfo, "omitted");
});

test("@visual unread", async ({ page }, testInfo) => {
  await openDetail(page, testInfo, "0193f2c4-7a1e-7c3d-9f42-6b1d2e8a51ce");
  await expect(
    page.getByText("The app did not read this body (1.2 KB)"),
  ).toBeVisible();
  await shot(page, testInfo, "unread");
});

test("@visual no route", async ({ page }, testInfo) => {
  await openDetail(page, testInfo, "0193f2c4-7a1e-7c3d-9f42-6b1d2e8a51ca");
  await expect(
    page
      .getByRole("region", { name: "Request", exact: true })
      .getByText("no route", { exact: true }),
  ).toBeVisible();
  await shot(page, testInfo, "no-route");
});

test("@visual panic", async ({ page }, testInfo) => {
  await openDetail(page, testInfo, "0193f2c4-7a1e-7c3d-9f42-6b1d2e8a51cb");
  await page.getByRole("tab", { name: "Response" }).click();
  await expect(
    page.getByText("No response: the handler panicked before responding"),
  ).toBeVisible();
  await shot(page, testInfo, "panic");
});

test("@visual missing", async ({ page }, testInfo) => {
  await prepare(page, testInfo);
  await installCaptures(page);
  await showCapture(page, "0193f2c4-7a1e-7c3d-9f42-6b1d2e8a51ff");
  await expect(
    page.getByText("This capture is no longer in the store"),
  ).toBeVisible();
  await shot(page, testInfo, "missing");
});

async function openDetail(
  page: Page,
  testInfo: TestInfo,
  id: string,
): Promise<void> {
  await prepare(page, testInfo);
  await installCaptures(page);
  await showCapture(page, id);
  await expect(page.getByRole("heading").first()).toBeVisible();
}
