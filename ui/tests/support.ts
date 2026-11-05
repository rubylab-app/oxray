import type { Page, TestInfo } from "@playwright/test";

export async function installTheme(
  page: Page,
  testInfo: TestInfo,
): Promise<void> {
  const theme = testInfo.project.name.startsWith("dark") ? "dark" : "light";
  await page.addInitScript((value) => {
    localStorage.setItem("oxray.theme", value);
  }, theme);
}
