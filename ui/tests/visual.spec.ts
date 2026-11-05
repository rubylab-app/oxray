import { expect, test } from "@playwright/test";
import { installTheme } from "./support";

test("@visual shell", async ({ page }, testInfo) => {
  await page.clock.install({ time: new Date("2026-11-04T21:15:00Z") });
  await page.emulateMedia({ reducedMotion: "reduce" });
  await installTheme(page, testInfo);
  await page.goto("./");
  await page.evaluate(async () => {
    await document.fonts.ready;
    const loaded = document.fonts.check("600 14px 'Inter Variable'");
    if (!loaded) {
      throw new Error("Inter Variable is not loaded");
    }
  });
  await expect(page).toHaveScreenshot("shell.png");
});
