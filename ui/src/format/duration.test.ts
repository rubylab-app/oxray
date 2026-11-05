import { expect, test } from "vitest";
import { formatDuration } from "./duration";

test("formats duration buckets", () => {
  const cases: Array<[number, string]> = [
    [0, "0 µs"],
    [74, "74 µs"],
    [999, "999 µs"],
    [1_000, "1.0 ms"],
    [1_200, "1.2 ms"],
    [9_999, "10.0 ms"],
    [10_000, "10 ms"],
    [812_000, "812 ms"],
    [1_000_000, "1.00 s"],
    [1_240_000, "1.24 s"],
    [59_500_000, "59.50 s"],
    [60_000_000, "1 min"],
    [64_000_000, "1 min 4 s"],
    [125_000_000, "2 min 5 s"],
  ];
  for (const [micros, expected] of cases) {
    expect(formatDuration(micros)).toBe(expected);
  }
});

test("rejects a negative duration", () => {
  expect(() => formatDuration(-1)).toThrow(/non-negative/);
});
