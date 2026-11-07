import { expect, test } from "vitest";
import { formatShare } from "./share";

test("formats a share of the request as a whole percent", () => {
  expect(formatShare(0.1817)).toBe("18%");
  expect(formatShare(1)).toBe("100%");
  expect(formatShare(0)).toBe("0%");
  expect(formatShare(0.004)).toBe("<1%");
  expect(() => formatShare(Number.NaN)).toThrow();
});
