import { expect, test } from "vitest";
import { bodyNotice, textLines } from "./body";

test("describes truncated, binary, and unread bodies", () => {
  expect(
    bodyNotice({
      content_type: "application/json",
      size_bytes: 1363149,
      captured_bytes: 65536,
      encoding: "utf8",
      truncated: true,
      omitted_reason: null,
    }),
  ).toBe("Body truncated at 64.0 KB of 1.3 MB");
  expect(
    bodyNotice({
      content_type: "image/png",
      size_bytes: 493568,
      captured_bytes: 0,
      encoding: "omitted",
      truncated: false,
      omitted_reason: "binary:image/png",
    }),
  ).toBe("Binary body omitted (image/png, 482.0 KB)");
  expect(
    bodyNotice({
      content_type: "application/json",
      size_bytes: 1229,
      captured_bytes: 0,
      encoding: "omitted",
      truncated: false,
      omitted_reason: "not_read_by_app",
    }),
  ).toBe("The app did not read this body (1.2 KB)");
});

test("splits text into lines without a phantom last line", () => {
  expect(textLines("a\n    b\n")).toEqual(["a", "    b"]);
  expect(textLines("a\r\nb")).toEqual(["a", "b"]);
  expect(textLines("a\n\n")).toEqual(["a", ""]);
  expect(textLines("one")).toEqual(["one"]);
});
