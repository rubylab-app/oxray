import { expect, test } from "vitest";
import { captureIdFromPath, listPathname } from "./path";

test("reads a capture id and the list path", () => {
  expect(captureIdFromPath("/r/abc")).toBe("abc");
  expect(captureIdFromPath("/_oxray/r/abc")).toBe("abc");
  expect(captureIdFromPath("/")).toBeNull();
  expect(listPathname("/r/abc")).toBe("/");
  expect(listPathname("/_oxray/r/abc")).toBe("/_oxray/");
});
