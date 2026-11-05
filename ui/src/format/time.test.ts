import { expect, test } from "vitest";
import { formatAbsolute, formatRelative } from "./time";

const now = Date.parse("2026-11-04T21:15:00.000Z");

test("formats relative time in fixed buckets", () => {
  expect(formatRelative("2026-11-04T21:14:48.000Z", now)).toBe("12s ago");
  expect(formatRelative("2026-11-04T21:12:00.000Z", now)).toBe("3m ago");
  expect(formatRelative("2026-11-04T18:15:00.000Z", now)).toBe("3h ago");
});

test("formats an absolute local timestamp", () => {
  expect(formatAbsolute("2026-11-04T21:14:03.412Z")).toContain("2026-11-04");
  expect(formatAbsolute("2026-11-04T21:14:03.412Z")).toContain("03.412");
});
