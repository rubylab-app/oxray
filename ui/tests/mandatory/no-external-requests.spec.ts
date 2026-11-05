import { expect, test } from "@playwright/test";
import { installTheme } from "../support";

test("records only same-origin requests under the mount", async ({
  page,
}, testInfo) => {
  const urls: string[] = [];
  page.on("request", (request) => {
    urls.push(request.url());
  });
  await installTheme(page, testInfo);
  await page.goto("./");
  await expect(page.getByText("oxray", { exact: true })).toBeVisible();
  expect(urls.length).toBeGreaterThan(0);
  const pageUrl = new URL(page.url());
  for (const url of urls) {
    const parsed = new URL(url);
    expect(parsed.origin).toBe(pageUrl.origin);
    expect(parsed.pathname.startsWith("/")).toBe(true);
  }
});
