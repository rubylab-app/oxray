import { expect, test } from "vitest";
import { formatField } from "./fields";

test("formats log fields so key=value stays unambiguous", () => {
  expect(formatField("502")).toBe("502");
  expect(formatField("insufficient funds")).toBe('"insufficient funds"');
  expect(formatField("")).toBe('""');
  expect(formatField(42)).toBe("42");
  expect(formatField(true)).toBe("true");
  expect(formatField(null)).toBe("empty");
  expect(formatField({ a: 1 })).toBe('{"a":1}');
});
