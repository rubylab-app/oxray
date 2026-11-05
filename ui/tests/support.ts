import { readFileSync } from "node:fs";
import type { Page, TestInfo } from "@playwright/test";
import type { Capture } from "../src/model/capture";
import { parseCaptures, toSummary } from "../src/model/capture";

const captures = parseCaptures(
  JSON.parse(
    readFileSync(new URL("./fixtures/captures.json", import.meta.url), "utf8"),
  ) as unknown,
);

export function fixtureCaptures(): Capture[] {
  return captures;
}

export async function installTheme(
  page: Page,
  testInfo: TestInfo,
): Promise<void> {
  const theme = testInfo.project.name.startsWith("dark") ? "dark" : "light";
  await page.addInitScript((value) => {
    localStorage.setItem("oxray.theme", value);
  }, theme);
}

export async function installCaptures(page: Page): Promise<void> {
  const summaries = captures.map((capture) => toSummary(capture));
  await page.unrouteAll({ behavior: "ignoreErrors" });
  await page.route(
    (url) => /\/api\/captures\/[^/]+$/.test(url.pathname),
    async (route) => {
      const id = new URL(route.request().url()).pathname.split("/").pop();
      const found = captures.find((capture) => capture.id === id);
      if (found === undefined) {
        await route.fulfill({
          status: 404,
          contentType: "text/plain",
          body: "missing",
        });
        return;
      }
      await route.fulfill({ json: found });
    },
  );
  await page.route(
    (url) => url.pathname.endsWith("/api/captures"),
    async (route) => {
      await route.fulfill({
        json: { items: summaries, next_cursor: null },
      });
    },
  );
}

export async function installEmpty(page: Page): Promise<void> {
  await page.unrouteAll({ behavior: "ignoreErrors" });
  await page.route(
    (url) => url.pathname.endsWith("/api/captures"),
    async (route) => {
      await route.fulfill({ json: { items: [], next_cursor: null } });
    },
  );
}

export async function installFailure(page: Page): Promise<void> {
  await page.unrouteAll({ behavior: "ignoreErrors" });
  await page.route(
    (url) => url.pathname.includes("/api/captures"),
    async (route) => {
      await route.fulfill({
        status: 500,
        contentType: "text/plain",
        body: "nope",
      });
    },
  );
}

export async function installHang(page: Page): Promise<void> {
  await page.unrouteAll({ behavior: "ignoreErrors" });
  await page.route(
    (url) => url.pathname.endsWith("/api/captures"),
    () => {
      // The response stays pending so the list can show its skeleton.
    },
  );
}

export async function showCapture(page: Page, id: string): Promise<void> {
  const listed = page.waitForResponse((response) =>
    new URL(response.url()).pathname.endsWith("/api/captures"),
  );
  await page.goto("./");
  await listed;
  await page.evaluate((captureId) => {
    window.history.pushState(null, "", `/r/${captureId}`);
    window.dispatchEvent(new PopStateEvent("popstate"));
  }, id);
}

export async function prepare(page: Page, testInfo: TestInfo): Promise<void> {
  await page.clock.install({ time: new Date("2026-11-04T21:15:00Z") });
  await page.emulateMedia({ reducedMotion: "reduce" });
  await installTheme(page, testInfo);
}

export async function fontsReady(page: Page): Promise<void> {
  await page.evaluate(async () => {
    await document.fonts.ready;
    const loaded = document.fonts.check("600 14px 'Inter Variable'");
    if (!loaded) {
      throw new Error("Inter Variable is not loaded");
    }
  });
}
