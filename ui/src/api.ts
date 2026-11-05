import type { Capture, CapturePage } from "./model/capture";
import { parseCapture, parsePage } from "./model/capture";

export function mountPrefix(): string {
  const content = document
    .querySelector('meta[name="oxray-mount"]')
    ?.getAttribute("content")
    ?.trim();
  if (
    content === undefined ||
    content.length === 0 ||
    content === "." ||
    content === "./"
  ) {
    return "";
  }
  return content.replace(/\/$/, "");
}

export function apiPath(path: string): string {
  const suffix = path.startsWith("/") ? path : `/${path}`;
  return `${mountPrefix()}${suffix}`;
}

export async function loadCaptures(
  cursor: string | null,
  signal: AbortSignal,
): Promise<CapturePage> {
  const url = new URL(apiPath("/api/captures"), window.location.origin);
  url.searchParams.set("limit", "500");
  if (cursor !== null) {
    url.searchParams.set("cursor", cursor);
  }
  const response = await fetch(url, {
    signal,
    headers: { accept: "application/json" },
  });
  if (response.status === 404) {
    return { items: [], next_cursor: null };
  }
  if (!response.ok) {
    throw new Error("captures");
  }
  const text = await response.text();
  // vite preview answers unknown paths with index.html. That is the empty
  // inspector, not a broken API.
  let parsed: unknown;
  try {
    parsed = JSON.parse(text) as unknown;
  } catch {
    return { items: [], next_cursor: null };
  }
  return parsePage(parsed);
}

export async function loadCapture(
  id: string,
  signal: AbortSignal,
): Promise<Capture | null> {
  const url = new URL(
    apiPath(`/api/captures/${encodeURIComponent(id)}`),
    window.location.origin,
  );
  const response = await fetch(url, {
    signal,
    headers: { accept: "application/json" },
  });
  if (response.status === 404) {
    return null;
  }
  if (!response.ok) {
    throw new Error("capture");
  }
  const text = await response.text();
  return parseCapture(JSON.parse(text) as unknown);
}
