import { readFileSync } from "node:fs";
import { expect, test } from "vitest";
import { headline, parseCaptures, toSummary } from "./capture";

const fixture = new URL("../../tests/fixtures/captures.json", import.meta.url);

test("fixtures match capture v1 and their summaries", () => {
  const captures = parseCaptures(JSON.parse(readFileSync(fixture, "utf8")));
  expect(captures).toHaveLength(8);
  const statuses = captures.map((capture) => capture.status);
  expect(statuses).toContain(200);
  expect(statuses).toContain(422);
  expect(statuses).toContain(500);
  expect(statuses).toContain(null);
  expect(captures.some((capture) => capture.matched_path === null)).toBe(true);
  expect(captures.some((capture) => capture.flags.truncated_request)).toBe(
    true,
  );
  expect(
    captures.some(
      (capture) => capture.request.body.omitted_reason === "binary:image/png",
    ),
  ).toBe(true);
  expect(
    captures.some(
      (capture) => capture.request.body.omitted_reason === "not_read_by_app",
    ),
  ).toBe(true);
  expect(
    captures.some((capture) =>
      capture.request.body.text?.includes("•••• 4242"),
    ),
  ).toBe(true);
  for (const capture of captures) {
    expect(capture.schema_version).toBe(1);
    const summary = toSummary(capture);
    expect(summary.id).toBe(capture.id);
    expect(summary.counts.queries).toBe(capture.queries.length);
    expect(summary.counts.redactions).toBe(capture.redactions.length);
    expect(summary.flags.truncated).toBe(
      capture.flags.truncated_request || capture.flags.truncated_response,
    );
  }
  const now = Date.parse("2026-11-04T21:15:00.000Z");
  expect(headline(captures.map(toSummary), now)).toEqual({
    requests: 6,
    failed: 2,
  });
});

test("rejects a capture without an id", () => {
  expect(() => parseCaptures([{}])).toThrow();
});
