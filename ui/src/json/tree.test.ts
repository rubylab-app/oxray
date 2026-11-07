import { expect, test } from "vitest";
import {
  countNodes,
  isFullyMasked,
  isMasked,
  parseJsonText,
  previewString,
} from "./tree";

test("parses json and counts nodes", () => {
  const parsed = parseJsonText('{"id":"ord_2042","ok":true}');
  expect(parsed.ok).toBe(true);
  if (!parsed.ok) {
    return;
  }
  expect(countNodes(parsed.value)).toBe(3);
  expect(parseJsonText("not json").ok).toBe(false);
});

test("masks redacted strings and shortens long ones", () => {
  expect(isMasked("•••• 4242")).toBe(true);
  expect(isMasked("[redacted]")).toBe(true);
  expect(isMasked("paid")).toBe(false);
  expect(isFullyMasked("[redacted]")).toBe(true);
  expect(isFullyMasked("•••• 4242")).toBe(false);
  expect(isFullyMasked("token=[redacted]")).toBe(false);
  const long = "a".repeat(240);
  expect(previewString(long, false)).toHaveLength(200);
  expect(previewString(long, true)).toBe(long);
});
