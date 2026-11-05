import { formatBytes } from "./bytes";

export type BodyNoticeInput = {
  content_type: string | null;
  size_bytes: number | null;
  captured_bytes: number;
  encoding: "utf8" | "omitted";
  truncated: boolean;
  omitted_reason: string | null;
};

export function bodyNotice(body: BodyNoticeInput): string | null {
  if (body.truncated) {
    const kept = formatBytes(body.captured_bytes);
    const total =
      body.size_bytes === null ? kept : formatBytes(body.size_bytes);
    return `Body truncated at ${kept} of ${total}`;
  }
  if (body.encoding !== "omitted" && body.omitted_reason === null) {
    return null;
  }
  const reason = body.omitted_reason ?? "omitted";
  const size = formatBytes(body.size_bytes ?? body.captured_bytes);
  if (reason === "not_read_by_app") {
    return `The app did not read this body (${size})`;
  }
  if (reason === "binary" || reason.startsWith("binary:")) {
    const type = reason.startsWith("binary:")
      ? reason.slice("binary:".length)
      : "binary";
    return `Binary body omitted (${type}, ${size})`;
  }
  return `Body omitted (${reason}, ${size})`;
}
