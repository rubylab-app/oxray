export type JsonKind =
  | "null"
  | "boolean"
  | "number"
  | "string"
  | "array"
  | "object";

export function parseJsonText(
  text: string,
): { ok: true; value: unknown } | { ok: false } {
  try {
    return { ok: true, value: JSON.parse(text) as unknown };
  } catch {
    return { ok: false };
  }
}

export function countNodes(value: unknown): number {
  if (Array.isArray(value)) {
    let total = 1;
    for (const item of value) {
      total += countNodes(item);
    }
    return total;
  }
  if (isObject(value)) {
    let total = 1;
    for (const item of Object.values(value)) {
      total += countNodes(item);
    }
    return total;
  }
  return 1;
}

export function isMasked(value: string): boolean {
  return value.includes("[redacted]") || value.includes("••••");
}

export function isFullyMasked(value: string): boolean {
  return value === "[redacted]";
}

export function previewString(value: string, expanded: boolean): string {
  if (expanded || value.length <= 200) {
    return value;
  }
  return value.slice(0, 200);
}

export function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function jsonKind(value: unknown): JsonKind {
  if (value === null) {
    return "null";
  }
  if (Array.isArray(value)) {
    return "array";
  }
  if (isObject(value)) {
    return "object";
  }
  if (typeof value === "string") {
    return "string";
  }
  if (typeof value === "number") {
    return "number";
  }
  if (typeof value === "boolean") {
    return "boolean";
  }
  return "string";
}
