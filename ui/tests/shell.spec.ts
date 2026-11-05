import { expect, test } from "@playwright/test";
import { installTheme } from "./support";

test("shows the wordmark and theme control", async ({ page }, testInfo) => {
  await installTheme(page, testInfo);
  await page.goto("./");
  await expect(page.getByText("oxray", { exact: true })).toBeVisible();
  const theme = await page.locator("html").getAttribute("data-theme");
  expect(theme === "dark" || theme === "light").toBe(true);
  const starting = testInfo.project.name.startsWith("dark") ? "Dark" : "Light";
  await expect(
    page.getByRole("button", { name: `Theme: ${starting}` }),
  ).toBeVisible();
});

test("theme control cycles the preference", async ({ page }, testInfo) => {
  await installTheme(page, testInfo);
  await page.goto("./");
  const starting = testInfo.project.name.startsWith("dark") ? "Dark" : "Light";
  const button = page.getByRole("button", { name: `Theme: ${starting}` });
  await button.click();
  const next = starting === "Dark" ? "Light" : "System";
  await expect(
    page.getByRole("button", { name: `Theme: ${next}` }),
  ).toBeVisible();
  const theme = await page.locator("html").getAttribute("data-theme");
  expect(theme === "dark" || theme === "light").toBe(true);
});
