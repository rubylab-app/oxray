import { expect, type Page, test } from "@playwright/test";
import { installTheme } from "../support";

const rawPan = "4242 4242 4242 4242";
const digits = "4242424242424242";

test("keyboard reaches the webhook and its queries", async ({
  page,
}, testInfo) => {
  await installTheme(page, testInfo);
  await page.goto("/debug/oxray/");
  await expect(page.getByText("/webhooks/{psp}").first()).toBeVisible();
  await page.keyboard.press("Enter");
  await expect(page.getByText("•••• 4242")).toBeVisible();
  await expectClean(page);
  await page.getByRole("tab", { name: "Request", exact: true }).focus();
  await page.keyboard.press("ArrowRight");
  await expect(page.getByText("processor failed")).toBeVisible();
  await expectClean(page);
  await page.keyboard.press("ArrowRight");
  await expect(page.getByRole("tab", { name: /Queries/ })).toBeFocused();
  await expect(
    page.getByText("select note from payments where id = ?", { exact: true }),
  ).toHaveCount(14);
  await expectClean(page);
  await page.keyboard.press("ArrowRight");
  await expectClean(page);
  await page.keyboard.press("ArrowRight");
  await expectClean(page);
});

async function expectClean(page: Page): Promise<void> {
  const text = (await page.locator("body").textContent()) ?? "";
  expect(text).not.toContain(rawPan);
  expect(text).not.toContain(digits);
}

test("a direct link opens the newest capture", async ({ page }, testInfo) => {
  await installTheme(page, testInfo);
  const response = await page.request.get("/debug/oxray/api/captures");
  const body = (await response.json()) as {
    items: Array<{ id: string }>;
  };
  const id = body.items[0]?.id;
  expect(id).toBeTruthy();
  await page.goto(`/debug/oxray/r/${id}`);
  await expect(
    page.getByRole("heading", { name: /\/webhooks\/\{psp\}/ }),
  ).toBeVisible();
  const text = (await page.locator("body").textContent()) ?? "";
  expect(text).not.toContain(digits);
});

test("the inspector's own icon is served and never captured", async ({
  page,
}, testInfo) => {
  await installTheme(page, testInfo);
  await page.goto("/debug/oxray/");
  await expect(page.getByText("/webhooks/{psp}").first()).toBeVisible();
  const icon = await page.request.get("/debug/oxray/assets/favicon.svg");
  expect(icon.status()).toBe(200);
  expect(icon.headers()["content-type"]).toBe("image/svg+xml");
  const response = await page.request.get("/debug/oxray/api/captures");
  const body = (await response.json()) as { items: Array<{ path: string }> };
  expect(body.items.some((item) => item.path === "/favicon.ico")).toBe(false);
});
