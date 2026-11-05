import AxeBuilder from "@axe-core/playwright";
import type { Page } from "@playwright/test";
import { expect, test } from "@playwright/test";
import { installCaptures, installTheme, showCapture } from "./support";

test("axe finds no serious or critical violations", async ({
  page,
}, testInfo) => {
  await installTheme(page, testInfo);
  await page.goto("./");
  await expect(page.getByText("oxray", { exact: true })).toBeVisible();
  const results = await new AxeBuilder({ page }).analyze();
  const blocking = results.violations.filter(
    (violation) =>
      violation.impact === "serious" || violation.impact === "critical",
  );
  const contrast = results.incomplete.filter(
    (item) => item.id === "color-contrast",
  );
  expect(blocking).toEqual([]);
  expect(contrast).toEqual([]);
});

test("axe finds no serious or critical violations on a request", async ({
  page,
}, testInfo) => {
  await installTheme(page, testInfo);
  await installCaptures(page);
  await showCapture(page, "0193f2c4-7a1e-7c3d-9f42-6b1d2e8a51c7");
  await expect(page.getByText("•••• 4242")).toBeVisible();
  await expectClean(page);
});

async function expectClean(page: Page): Promise<void> {
  const results = await new AxeBuilder({ page }).analyze();
  const blocking = results.violations.filter(
    (violation) =>
      violation.impact === "serious" || violation.impact === "critical",
  );
  const contrast = results.incomplete.filter(
    (item) => item.id === "color-contrast",
  );
  expect(blocking).toEqual([]);
  expect(contrast).toEqual([]);
}
