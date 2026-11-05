import { expect, test } from "vitest";
import { tokenizeSql } from "./sql";

test("marks sql keywords and keeps the original text", () => {
  const tokens = tokenizeSql("SELECT id FROM payments WHERE pan = ?");
  expect(
    tokens
      .filter((token) => token.kind === "keyword")
      .map((token) => token.text),
  ).toEqual(["SELECT", "FROM", "WHERE"]);
  expect(tokens.map((token) => token.text).join("")).toBe(
    "SELECT id FROM payments WHERE pan = ?",
  );
});
