import { expect, test } from "vitest";
import { formatBytes } from "./bytes";

test("formats byte sizes on a 1024 base", () => {
  expect(formatBytes(482)).toBe("482 B");
  expect(formatBytes(1229)).toBe("1.2 KB");
  expect(formatBytes(65536)).toBe("64.0 KB");
  expect(formatBytes(493568)).toBe("482.0 KB");
  expect(formatBytes(1363149)).toBe("1.3 MB");
});
