import type { Page } from "@playwright/test";
import { expect, test } from "@playwright/test";
import { installCaptures, installTheme, prepare } from "./support";

function summary(index: number) {
  return {
    id: `cap-${String(index).padStart(4, "0")}`,
    short_id: `c${index}`,
    started_at: "2026-11-04T21:14:00.000Z",
    duration_us: 1_000,
    method: "GET",
    path: `/v1/orders/ord_${index}`,
    matched_path: "/v1/orders/{id}",
    status: 200,
    counts: { queries: 0, outgoing: 0, logs: 0, errors: 0, redactions: 0 },
    flags: {
      has_error: false,
      n_plus_one: false,
      truncated: false,
      panicked: false,
    },
  };
}

async function scrollToEnd(page: Page): Promise<void> {
  await page.evaluate(() => {
    const row = document.querySelector("[data-index]");
    const scroller = row?.parentElement?.parentElement;
    if (scroller) {
      scroller.scrollTop = scroller.scrollHeight;
    }
  });
}

test("the next page loads near the end of the list", async ({
  page,
}, testInfo) => {
  await installTheme(page, testInfo);
  const items = Array.from({ length: 600 }, (_, index) => summary(index));
  const cursors: Array<string | null> = [];
  await page.route(
    (url) => url.pathname.endsWith("/api/captures"),
    async (route) => {
      const cursor = new URL(route.request().url()).searchParams.get("cursor");
      cursors.push(cursor);
      if (cursor === null) {
        await route.fulfill({
          json: { items: items.slice(0, 500), next_cursor: "cap-0499" },
        });
        return;
      }
      await route.fulfill({
        json: { items: items.slice(500), next_cursor: null },
      });
    },
  );
  await page.goto("./");
  await expect(page.locator("header")).toContainText("500+ requests");
  await expect
    .poll(async () => {
      await scrollToEnd(page);
      return page.locator("[data-index='599']").count();
    })
    .toBe(1);
  await expect(page.locator("header")).toContainText("600 requests");
  expect(cursors).toEqual([null, "cap-0499"]);
});

test("a failing next page backs off without refetching the first", async ({
  page,
}, testInfo) => {
  await installTheme(page, testInfo);
  const items = Array.from({ length: 120 }, (_, index) => summary(index));
  let first = 0;
  let next = 0;
  await page.route(
    (url) => url.pathname.endsWith("/api/captures"),
    async (route) => {
      const cursor = new URL(route.request().url()).searchParams.get("cursor");
      if (cursor === null) {
        first += 1;
        await route.fulfill({ json: { items, next_cursor: "cap-0119" } });
        return;
      }
      next += 1;
      await route.fulfill({ status: 500, body: "nope" });
    },
  );
  await page.goto("./");
  await expect(page.locator("[data-index='0']")).toBeVisible();
  await expect
    .poll(async () => {
      await scrollToEnd(page);
      return next;
    })
    .toBeGreaterThanOrEqual(1);
  await page.waitForTimeout(2_500);
  expect(next).toBeLessThanOrEqual(3);
  expect(first).toBe(1);
  await expect(page.getByText("Can't reach the app. Retrying…")).toBeHidden();
});

test("a failed refresh keeps the list and says the app is unreachable", async ({
  page,
}, testInfo) => {
  await installTheme(page, testInfo);
  await installCaptures(page);
  await page.goto("./");
  await expect(page.getByText("/webhooks/{psp}").first()).toBeVisible();
  await page.route(
    (url) => url.pathname.endsWith("/api/captures"),
    async (route) => {
      await route.abort();
    },
  );
  await page.keyboard.press("r");
  const notice = page.getByText("Can't reach the app. Retrying…");
  await expect(notice).toBeVisible();
  await expect(page.getByText("/webhooks/{psp}").first()).toBeVisible();
  await expect(page.locator("[data-index]")).toHaveCount(9);
  await installCaptures(page);
  await expect(notice).toBeHidden({ timeout: 5_000 });
  await expect(page.locator("[data-index]")).toHaveCount(9);
});

test("the headline counts every loaded request", async ({ page }, testInfo) => {
  await prepare(page, testInfo);
  await installCaptures(page);
  await page.goto("./");
  await expect(page.locator("header")).toContainText(
    "9 requests · 2 failed in the last 5 min",
  );
});

test("rows describe themselves and the selected one is marked", async ({
  page,
}, testInfo) => {
  await prepare(page, testInfo);
  await installCaptures(page);
  await page.goto("./");
  const first = page.locator("[data-index='0']");
  await expect(first).toHaveAccessibleName(
    /^(2s ago ?, )?500 POST \/webhooks\/\{psp\} ?, [^,]+ ?, 4 queries ?, 1 outgoing call ?, Redacted$/,
  );
  await expect(page.locator("main")).toHaveCount(1);
  await expect(
    page.getByRole("heading", { level: 1, name: "Requests" }),
  ).toHaveCount(1);
  await page.keyboard.press("j");
  const selected = page.locator("[aria-current='true']");
  await expect(selected).toHaveAttribute("data-index", "0");
  const bar = await selected
    .locator("> span[aria-hidden]")
    .evaluate((element) => {
      const style = getComputedStyle(element, "::before");
      return { width: style.width, position: style.position };
    });
  expect(bar).toEqual({ width: "2px", position: "absolute" });
});

test("relative times keep moving", async ({ page }, testInfo) => {
  await prepare(page, testInfo);
  await installCaptures(page);
  await page.goto("./");
  const time = page.locator("[data-index='0'] > span").first();
  await expect(time).toContainText("2s ago");
  await page.clock.fastForward(15_000);
  await expect(time).toContainText("17s ago");
});

test("controls name their shortcuts", async ({ page }, testInfo) => {
  await installTheme(page, testInfo);
  await installCaptures(page);
  await page.goto("./");
  await expect(page.getByText("/webhooks/{psp}").first()).toBeVisible();
  const starting = testInfo.project.name.startsWith("dark") ? "Dark" : "Light";
  await expect(page.getByRole("button", { name: "Refresh" })).toHaveAttribute(
    "title",
    "Refresh (r)",
  );
  await expect(
    page.getByRole("button", { name: `Theme: ${starting}` }),
  ).toHaveAttribute("title", `Theme: ${starting} (t)`);
  if (!testInfo.project.name.endsWith("960")) {
    await expect(page.getByText("to move · Enter to open")).toBeVisible();
  }
});

test("the page brings its own icon", async ({ page }, testInfo) => {
  await installTheme(page, testInfo);
  const asked: string[] = [];
  page.on("request", (request) => {
    asked.push(new URL(request.url()).pathname);
  });
  await page.goto("./");
  const href = await page
    .locator("link[rel='icon']")
    .evaluate((element) =>
      element instanceof HTMLLinkElement ? element.href : "",
    );
  const icon = await page.request.get(href);
  expect(icon.status()).toBe(200);
  expect(icon.headers()["content-type"]).toContain("image/svg+xml");
  expect(asked).not.toContain("/favicon.ico");
});

test("the empty list says Oxray is running and how to test it", async ({
  page,
}, testInfo) => {
  await installTheme(page, testInfo);
  await page.goto("./");
  await expect(page.getByText("Oxray is running.")).toBeVisible();
  const origin = new URL(page.url()).origin;
  await expect(
    page.getByText(`curl ${origin}/`, { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText("must be the last layer", { exact: false }),
  ).toBeVisible();
  await expect(page.getByText("Select a request")).toHaveCount(0);
});

test("coming back to the tab refreshes and keeps the selection", async ({
  page,
}, testInfo) => {
  await installTheme(page, testInfo);
  const rows = Array.from({ length: 3 }, (_, index) => summary(index));
  let served = rows;
  await page.route(
    (url) => url.pathname.endsWith("/api/captures"),
    async (route) => {
      await route.fulfill({ json: { items: served, next_cursor: null } });
    },
  );
  await page.goto("./");
  await expect(page.locator("[data-index='2']")).toBeVisible();
  await page.keyboard.press("j");
  await page.keyboard.press("j");
  const selected = page.locator("[aria-current='true']");
  await expect(selected).toContainText("/v1/orders/{id}");
  await expect(selected).toHaveAttribute("data-index", "1");
  served = [summary(7), summary(8), ...rows];
  await page.evaluate(() => {
    document.dispatchEvent(new Event("visibilitychange"));
  });
  await expect(page.locator("[data-index]")).toHaveCount(5);
  await expect(selected).toHaveAttribute("data-index", "3");
});

test("new requests on top do not move the rows being read", async ({
  page,
}, testInfo) => {
  await installTheme(page, testInfo);
  const rows = Array.from({ length: 200 }, (_, index) => summary(index + 10));
  let served = rows;
  await page.route(
    (url) => url.pathname.endsWith("/api/captures"),
    async (route) => {
      await route.fulfill({ json: { items: served, next_cursor: null } });
    },
  );
  await page.goto("./");
  await expect(page.locator("[data-index='0']")).toBeVisible();
  const scrollTop = (value: number | null) =>
    page.evaluate((next) => {
      const scroller =
        document.querySelector("[data-index]")?.parentElement?.parentElement;
      if (scroller && next !== null) {
        scroller.scrollTop = next;
      }
      return scroller?.scrollTop ?? -1;
    }, value);
  await scrollTop(1_600);
  await expect(page.locator("[data-index='50']")).toBeVisible();
  served = [summary(1), summary(2), summary(3), ...rows];
  await page.evaluate(() => {
    document.dispatchEvent(new Event("visibilitychange"));
  });
  await expect.poll(() => scrollTop(null)).toBe(1_600 + 3 * 32);
});

test("a refresh after moving with j keeps the reader's scroll", async ({
  page,
}, testInfo) => {
  await installTheme(page, testInfo);
  const rows = Array.from({ length: 200 }, (_, index) => summary(index + 10));
  let served = rows;
  await page.route(
    (url) => url.pathname.endsWith("/api/captures"),
    async (route) => {
      await route.fulfill({ json: { items: served, next_cursor: null } });
    },
  );
  await page.goto("./");
  await expect(page.locator("[data-index='0']")).toBeVisible();
  await page.keyboard.press("j");
  await page.keyboard.press("j");
  const selected = page.locator("[aria-current='true']");
  await expect(selected).toHaveAttribute("data-index", "1");
  await expect(selected).toBeFocused();
  const scrollTop = (value: number | null) =>
    page.evaluate((next) => {
      const scroller =
        document.querySelector("[data-index]")?.parentElement?.parentElement;
      if (scroller && next !== null) {
        scroller.scrollTop = next;
      }
      return scroller?.scrollTop ?? -1;
    }, value);
  await scrollTop(1_600);
  await expect(page.locator("[data-index='50']")).toBeVisible();
  served = [summary(1), summary(2), ...rows];
  await page.evaluate(() => {
    document.dispatchEvent(new Event("visibilitychange"));
  });
  await expect.poll(() => scrollTop(null)).toBe(1_600 + 2 * 32);
  await page.waitForTimeout(300);
  expect(await scrollTop(null)).toBe(1_600 + 2 * 32);
});

test("the Time column fits a 1440 px split view", async ({
  page,
}, testInfo) => {
  await installTheme(page, testInfo);
  await installCaptures(page);
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("./");
  await expect(page.getByText("/webhooks/{psp}").first()).toBeVisible();
  await expect(page.getByText("Time", { exact: true })).toBeVisible();
});
