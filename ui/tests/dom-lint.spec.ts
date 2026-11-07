import { expect, test } from "@playwright/test";
import { domLintProblems } from "./dom-lint";
import {
  detailHeading,
  installCaptures,
  installFailure,
  installHang,
  installTheme,
  showCapture,
} from "./support";

test("dom lint covers the canonical screens", async ({ page }, testInfo) => {
  await installTheme(page, testInfo);
  const problems: string[] = [];

  await page.goto("./");
  await expect(page.getByText("Waiting for the first request…")).toBeVisible();
  problems.push(...(await domLintProblems(page)));

  await installCaptures(page);
  await page.goto("./");
  await expect(page.getByText("/webhooks/{psp}").first()).toBeVisible();
  problems.push(...(await domLintProblems(page)));

  for (const id of [
    "0193f2c4-7a1e-7c3d-9f42-6b1d2e8a51c7",
    "0193f2c4-7a1e-7c3d-9f42-6b1d2e8a51cc",
    "0193f2c4-7a1e-7c3d-9f42-6b1d2e8a51cd",
    "0193f2c4-7a1e-7c3d-9f42-6b1d2e8a51ce",
    "0193f2c4-7a1e-7c3d-9f42-6b1d2e8a51ca",
    "0193f2c4-7a1e-7c3d-9f42-6b1d2e8a51cb",
  ]) {
    await showCapture(page, id);
    await expect(detailHeading(page)).toBeVisible();
    if (id.endsWith("51cb")) {
      await page.getByRole("tab", { name: "Response" }).click();
    }
    problems.push(...(await domLintProblems(page)));
  }

  await showCapture(page, "0193f2c4-7a1e-7c3d-9f42-6b1d2e8a51ff");
  await expect(
    page.getByText("This capture is no longer in the store"),
  ).toBeVisible();
  problems.push(...(await domLintProblems(page)));

  await installFailure(page);
  await page.goto("./");
  await expect(
    page.getByText("Couldn't load captures. Retrying…"),
  ).toBeVisible();
  problems.push(...(await domLintProblems(page)));

  await installHang(page);
  await page.goto("./");
  await expect(page.getByText("oxray", { exact: true })).toBeVisible();
  await page.waitForTimeout(350);
  await expect(page.locator("[aria-busy='true']")).toBeVisible();
  problems.push(...(await domLintProblems(page)));

  expect(problems).toEqual([]);
});
