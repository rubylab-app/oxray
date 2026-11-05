const countFormat = new Intl.NumberFormat("en-US");

export function formatCount(value: number): string {
  if (!Number.isFinite(value)) {
    throw new Error("count must be finite");
  }
  return countFormat.format(value);
}
