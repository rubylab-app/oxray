import { readFileSync } from "node:fs";
import { expect, test } from "vitest";
import type { CaptureSummary } from "./capture";
import {
  appendPage,
  headline,
  parseCaptures,
  refreshFirstPage,
  toSummary,
} from "./capture";

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
    requests: 8,
    failed: 2,
  });
});

test("the headline counts every loaded request, even quiet ones", () => {
  const now = Date.parse("2026-11-04T21:15:00.000Z");
  const old = [summary("a", 200, "2026-11-04T18:00:00.000Z")];
  expect(headline(old, now)).toEqual({ requests: 1, failed: 0 });
  const failedLongAgo = [summary("b", 500, "2026-11-04T18:00:00.000Z")];
  expect(headline(failedLongAgo, now)).toEqual({ requests: 1, failed: 0 });
});

test("a next page is appended once, without duplicates", () => {
  const loaded = { items: [summary("c"), summary("b")], cursor: "b" };
  const next = appendPage(loaded, {
    items: [summary("b"), summary("a")],
    next_cursor: null,
  });
  expect(next.items.map((item) => item.id)).toEqual(["c", "b", "a"]);
  expect(next.cursor).toBeNull();
});

test("a refresh keeps the pages already scrolled through", () => {
  const loaded = {
    items: [summary("e"), summary("d"), summary("c"), summary("b")],
    cursor: "b",
  };
  const refreshed = refreshFirstPage(loaded, {
    items: [summary("f"), summary("e"), summary("d")],
    next_cursor: "d",
  });
  expect(refreshed.items.map((item) => item.id)).toEqual([
    "f",
    "e",
    "d",
    "c",
    "b",
  ]);
  expect(refreshed.cursor).toBe("b");
});

test("a refresh replaces the list when it no longer overlaps", () => {
  const loaded = { items: [summary("b"), summary("a")], cursor: "a" };
  const all = refreshFirstPage(loaded, {
    items: [summary("z")],
    next_cursor: null,
  });
  expect(all).toEqual({ items: [summary("z")], cursor: null });
  const apart = refreshFirstPage(loaded, {
    items: [summary("y"), summary("x")],
    next_cursor: "x",
  });
  expect(apart.items.map((item) => item.id)).toEqual(["y", "x"]);
  expect(apart.cursor).toBe("x");
});

function summary(
  id: string,
  status: number | null = 200,
  startedAt = "2026-11-04T21:14:00.000Z",
): CaptureSummary {
  return {
    id,
    short_id: id,
    started_at: startedAt,
    duration_us: 1_000,
    method: "GET",
    path: `/v1/${id}`,
    matched_path: null,
    status,
    counts: { queries: 0, outgoing: 0, logs: 0, errors: 0, redactions: 0 },
    flags: {
      has_error: false,
      n_plus_one: false,
      truncated: false,
      panicked: false,
    },
  };
}

test("rejects a capture without an id", () => {
  expect(() => parseCaptures([{}])).toThrow();
});
