const PHRASES: Record<number, string> = {
  400: "Bad Request",
  401: "Unauthorized",
  403: "Forbidden",
  404: "Not Found",
  409: "Conflict",
  415: "Unsupported Media Type",
  422: "Unprocessable Content",
  500: "Internal Server Error",
  502: "Bad Gateway",
  503: "Service Unavailable",
};

export function statusPhrase(status: number): string | null {
  if (status < 400) {
    return null;
  }
  return PHRASES[status] ?? null;
}

export function statusTone(status: number): "neutral" | "warn" | "err" {
  if (status >= 500) {
    return "err";
  }
  if (status >= 400) {
    return "warn";
  }
  return "neutral";
}
