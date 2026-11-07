export function formatField(value: unknown): string {
  if (typeof value === "string") {
    return value.length === 0 || /\s/.test(value)
      ? JSON.stringify(value)
      : value;
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  if (value === null || value === undefined) {
    return "empty";
  }
  return JSON.stringify(value);
}
