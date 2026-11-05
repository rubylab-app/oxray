import { expect, test } from "@playwright/test";
import { installTheme } from "./support";

test("ten thousand rows stay virtualized", async ({ page }, testInfo) => {
  await installTheme(page, testInfo);
  await page.route(
    (url) => url.pathname.endsWith("/api/captures"),
    async (route) => {
      const items = Array.from({ length: 10_000 }, (_, index) => ({
        id: `cap-${index}`,
        short_id: `c${index}`,
        started_at: "2026-11-04T21:14:00.000Z",
        duration_us: 1_000,
        method: "GET",
        path: `/v1/orders/ord_${index}`,
        matched_path: "/v1/orders/{id}",
        status: 200,
        counts: {
          queries: 0,
          outgoing: 0,
          logs: 0,
          errors: 0,
          redactions: 0,
        },
        flags: {
          has_error: false,
          n_plus_one: false,
          truncated: false,
          panicked: false,
        },
      }));
      await route.fulfill({ json: { items, next_cursor: null } });
    },
  );
  await page.goto("./");
  await expect(page.locator("[data-index='0']")).toBeVisible();
  const mounted = async () => page.locator("[data-index]").count();
  expect(await mounted()).toBeLessThan(100);
  await page.evaluate(() => {
    const row = document.querySelector("[data-index]");
    const scroller = row?.parentElement?.parentElement;
    if (scroller) {
      scroller.scrollTop = scroller.scrollHeight;
    }
  });
  await expect
    .poll(async () =>
      page.locator("[data-index]").first().getAttribute("data-index"),
    )
    .not.toBe("0");
  expect(await mounted()).toBeLessThan(100);
});
