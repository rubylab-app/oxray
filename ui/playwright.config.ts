import { readFileSync } from "node:fs";
import { defineConfig } from "@playwright/test";

function screenshotThresholds(): { threshold: number; maxDiffPixels: number } {
  const text = readFileSync(
    new URL("../harness.toml", import.meta.url),
    "utf8",
  );
  const threshold = /screenshot_threshold\s*=\s*([0-9.]+)/.exec(text);
  const pixels = /screenshot_max_diff_pixels\s*=\s*([0-9]+)/.exec(text);
  const thresholdValue = threshold?.[1];
  const pixelValue = pixels?.[1];
  if (thresholdValue === undefined || pixelValue === undefined) {
    throw new Error("harness.toml is missing screenshot thresholds");
  }
  return {
    threshold: Number(thresholdValue),
    maxDiffPixels: Number(pixelValue),
  };
}

const shots = screenshotThresholds();
const themes = ["light", "dark"] as const;
const widths = [
  { name: "1600", width: 1600, height: 1000 },
  { name: "1280", width: 1280, height: 800 },
  { name: "960", width: 960, height: 800 },
] as const;

const jsonReport = process.env.OXRAY_PLAYWRIGHT_JSON;

export default defineConfig({
  testDir: "./tests",
  retries: 0,
  forbidOnly: true,
  reporter:
    jsonReport === undefined
      ? [["list"]]
      : [["list"], ["json", { outputFile: jsonReport }]],
  snapshotPathTemplate: "{testDir}/screenshots/{projectName}/{arg}{ext}",
  expect: {
    toHaveScreenshot: {
      threshold: shots.threshold,
      maxDiffPixels: shots.maxDiffPixels,
      animations: "disabled",
      caret: "hide",
    },
  },
  use: {
    baseURL: "http://127.0.0.1:4173",
    timezoneId: "UTC",
    locale: "en-US",
    deviceScaleFactor: 1,
  },
  webServer: {
    command: "npx vite preview --host 127.0.0.1 --port 4173 --strictPort",
    url: "http://127.0.0.1:4173",
    reuseExistingServer: false,
    timeout: 120_000,
  },
  projects: themes.flatMap((theme) =>
    widths.map((size) => ({
      name: `${theme}-${size.name}`,
      use: {
        viewport: { width: size.width, height: size.height },
        colorScheme: theme,
      },
    })),
  ),
});
