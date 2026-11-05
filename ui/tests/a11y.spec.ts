import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";
import { installTheme } from "./support";

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
