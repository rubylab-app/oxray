export type BodyEncoding = "utf8" | "omitted";

export type Header = {
  name: string;
  value: string;
};

export type BodyCapture = {
  content_type: string | null;
  size_bytes: number | null;
  captured_bytes: number;
  encoding: BodyEncoding;
  text: string | null;
  truncated: boolean;
  omitted_reason: string | null;
};

export type Message = {
  headers: Header[];
  body: BodyCapture;
};

export type LogEntry = {
  offset_us: number;
  level: string;
  target: string;
  message: string;
  fields: Record<string, unknown>;
};

export type QueryEntry = {
  offset_us: number;
  duration_us: number;
  sql: string;
  fingerprint: string;
  rows_affected: number | null;
  rows_returned: number | null;
};

export type OutgoingCall = {
  offset_us: number;
  duration_us: number;
  method: string;
  url: string;
  status: number | null;
  request: Message;
  response: Message | null;
  error: string | null;
};

export type Redaction = {
  path: string;
  rule: string;
};

export type Flags = {
  has_error: boolean;
  panicked: boolean;
  truncated_request: boolean;
  truncated_response: boolean;
  streaming: boolean;
  dropped_events: number;
};

export type Capture = {
  schema_version: number;
  id: string;
  short_id: string;
  started_at: string;
  duration_us: number;
  method: string;
  path: string;
  query: string;
  matched_path: string | null;
  status: number | null;
  mode: string;
  request: Message;
  response: Message | null;
  logs: LogEntry[];
  queries: QueryEntry[];
  outgoing: OutgoingCall[];
  errors: unknown[];
  n_plus_one: unknown[];
  redactions: Redaction[];
  flags: Flags;
};

export type SummaryCounts = {
  queries: number;
  outgoing: number;
  logs: number;
  errors: number;
  redactions: number;
};

export type SummaryFlags = {
  has_error: boolean;
  n_plus_one: boolean;
  truncated: boolean;
  panicked: boolean;
};

export type CaptureSummary = {
  id: string;
  short_id: string;
  started_at: string;
  duration_us: number;
  method: string;
  path: string;
  matched_path: string | null;
  status: number | null;
  counts: SummaryCounts;
  flags: SummaryFlags;
};

export type CapturePage = {
  items: CaptureSummary[];
  next_cursor: string | null;
};

export function toSummary(capture: Capture): CaptureSummary {
  return {
    id: capture.id,
    short_id: capture.short_id,
    started_at: capture.started_at,
    duration_us: capture.duration_us,
    method: capture.method,
    path: capture.path,
    matched_path: capture.matched_path,
    status: capture.status,
    counts: {
      queries: capture.queries.length,
      outgoing: capture.outgoing.length,
      logs: capture.logs.length,
      errors: capture.errors.length,
      redactions: capture.redactions.length,
    },
    flags: {
      has_error: capture.flags.has_error,
      n_plus_one: capture.n_plus_one.length > 0,
      truncated:
        capture.flags.truncated_request || capture.flags.truncated_response,
      panicked: capture.flags.panicked,
    },
  };
}

export function headline(
  items: CaptureSummary[],
  nowMs: number,
): { requests: number; failed: number } {
  const cutoff = nowMs - 5 * 60 * 1000;
  let failed = 0;
  for (const item of items) {
    const started = Date.parse(item.started_at);
    if (Number.isFinite(started) && started >= cutoff && isFailure(item)) {
      failed += 1;
    }
  }
  return { requests: items.length, failed };
}

export type LoadedPages = {
  items: CaptureSummary[];
  cursor: string | null;
};

export function appendPage(
  loaded: LoadedPages,
  page: CapturePage,
): LoadedPages {
  const seen = new Set(loaded.items.map((item) => item.id));
  return {
    items: [
      ...loaded.items,
      ...page.items.filter((item) => !seen.has(item.id)),
    ],
    cursor: page.next_cursor,
  };
}

// A refresh only fetches the first page. Pages the user already scrolled
// through stay, so the list does not shrink under them.
export function refreshFirstPage(
  loaded: LoadedPages,
  page: CapturePage,
): LoadedPages {
  const last = page.items.at(-1);
  if (page.next_cursor === null || last === undefined) {
    return { items: page.items, cursor: page.next_cursor };
  }
  const overlap = loaded.items.findIndex((item) => item.id === last.id);
  if (overlap < 0) {
    return { items: page.items, cursor: page.next_cursor };
  }
  const fresh = new Set(page.items.map((item) => item.id));
  const older = loaded.items
    .slice(overlap + 1)
    .filter((item) => !fresh.has(item.id));
  return {
    items: [...page.items, ...older],
    cursor: older.length === 0 ? page.next_cursor : loaded.cursor,
  };
}

export function isFailure(item: CaptureSummary): boolean {
  return (
    item.flags.has_error ||
    item.flags.panicked ||
    item.status === null ||
    item.status >= 500
  );
}

export function parseCaptures(value: unknown): Capture[] {
  if (!Array.isArray(value)) {
    throw new Error("captures must be an array");
  }
  return value.map((item, index) => parseCapture(item, `captures[${index}]`));
}

export function parseCapture(value: unknown, label = "capture"): Capture {
  const record = recordAt(value, label);
  const queries = arrayAt(record.queries, `${label}.queries`).map(
    (item, index) => parseQuery(item, `${label}.queries[${index}]`),
  );
  const outgoing = arrayAt(record.outgoing, `${label}.outgoing`).map(
    (item, index) => parseOutgoing(item, `${label}.outgoing[${index}]`),
  );
  const logs = arrayAt(record.logs, `${label}.logs`).map((item, index) =>
    parseLog(item, `${label}.logs[${index}]`),
  );
  return {
    schema_version: numberAt(record.schema_version, `${label}.schema_version`),
    id: stringAt(record.id, `${label}.id`),
    short_id: stringAt(record.short_id, `${label}.short_id`),
    started_at: stringAt(record.started_at, `${label}.started_at`),
    duration_us: numberAt(record.duration_us, `${label}.duration_us`),
    method: stringAt(record.method, `${label}.method`),
    path: stringAt(record.path, `${label}.path`),
    query: stringAt(record.query, `${label}.query`),
    matched_path: stringOrNull(record.matched_path, `${label}.matched_path`),
    status: numberOrNull(record.status, `${label}.status`),
    mode: stringAt(record.mode, `${label}.mode`),
    request: parseMessage(record.request, `${label}.request`),
    response:
      record.response === null
        ? null
        : parseMessage(record.response, `${label}.response`),
    logs,
    queries,
    outgoing,
    errors: arrayAt(record.errors, `${label}.errors`),
    n_plus_one: arrayAt(record.n_plus_one, `${label}.n_plus_one`),
    redactions: arrayAt(record.redactions, `${label}.redactions`).map(
      (item, index) => parseRedaction(item, `${label}.redactions[${index}]`),
    ),
    flags: parseFlags(record.flags, `${label}.flags`),
  };
}

export function parsePage(value: unknown): CapturePage {
  const record = recordAt(value, "page");
  return {
    items: arrayAt(record.items, "page.items").map((item, index) =>
      parseSummary(item, `page.items[${index}]`),
    ),
    next_cursor: stringOrNull(record.next_cursor, "page.next_cursor"),
  };
}

function parseSummary(value: unknown, label: string): CaptureSummary {
  const record = recordAt(value, label);
  const counts = recordAt(record.counts, `${label}.counts`);
  const flags = recordAt(record.flags, `${label}.flags`);
  return {
    id: stringAt(record.id, `${label}.id`),
    short_id: stringAt(record.short_id, `${label}.short_id`),
    started_at: stringAt(record.started_at, `${label}.started_at`),
    duration_us: numberAt(record.duration_us, `${label}.duration_us`),
    method: stringAt(record.method, `${label}.method`),
    path: stringAt(record.path, `${label}.path`),
    matched_path: stringOrNull(record.matched_path, `${label}.matched_path`),
    status: numberOrNull(record.status, `${label}.status`),
    counts: {
      queries: numberAt(counts.queries, `${label}.counts.queries`),
      outgoing: numberAt(counts.outgoing, `${label}.counts.outgoing`),
      logs: numberAt(counts.logs, `${label}.counts.logs`),
      errors: numberAt(counts.errors, `${label}.counts.errors`),
      redactions: numberAt(counts.redactions, `${label}.counts.redactions`),
    },
    flags: {
      has_error: booleanAt(flags.has_error, `${label}.flags.has_error`),
      n_plus_one: booleanAt(flags.n_plus_one, `${label}.flags.n_plus_one`),
      truncated: booleanAt(flags.truncated, `${label}.flags.truncated`),
      panicked: booleanAt(flags.panicked, `${label}.flags.panicked`),
    },
  };
}

function parseMessage(value: unknown, label: string): Message {
  const record = recordAt(value, label);
  return {
    headers: arrayAt(record.headers, `${label}.headers`).map((item, index) =>
      parseHeader(item, `${label}.headers[${index}]`),
    ),
    body: parseBody(record.body, `${label}.body`),
  };
}

function parseHeader(value: unknown, label: string): Header {
  const record = recordAt(value, label);
  return {
    name: stringAt(record.name, `${label}.name`),
    value: stringAt(record.value, `${label}.value`),
  };
}

function parseBody(value: unknown, label: string): BodyCapture {
  const record = recordAt(value, label);
  const encoding = stringAt(record.encoding, `${label}.encoding`);
  if (encoding !== "utf8" && encoding !== "omitted") {
    throw new Error(`${label}.encoding`);
  }
  return {
    content_type: stringOrNull(record.content_type, `${label}.content_type`),
    size_bytes: numberOrNull(record.size_bytes, `${label}.size_bytes`),
    captured_bytes: numberAt(record.captured_bytes, `${label}.captured_bytes`),
    encoding,
    text: stringOrNull(record.text, `${label}.text`),
    truncated: booleanAt(record.truncated, `${label}.truncated`),
    omitted_reason: stringOrNull(
      record.omitted_reason,
      `${label}.omitted_reason`,
    ),
  };
}

function parseLog(value: unknown, label: string): LogEntry {
  const record = recordAt(value, label);
  const fields = recordAt(record.fields, `${label}.fields`);
  return {
    offset_us: numberAt(record.offset_us, `${label}.offset_us`),
    level: stringAt(record.level, `${label}.level`),
    target: stringAt(record.target, `${label}.target`),
    message: stringAt(record.message, `${label}.message`),
    fields,
  };
}

function parseQuery(value: unknown, label: string): QueryEntry {
  const record = recordAt(value, label);
  return {
    offset_us: numberAt(record.offset_us, `${label}.offset_us`),
    duration_us: numberAt(record.duration_us, `${label}.duration_us`),
    sql: stringAt(record.sql, `${label}.sql`),
    fingerprint: stringAt(record.fingerprint, `${label}.fingerprint`),
    rows_affected: numberOrNull(record.rows_affected, `${label}.rows_affected`),
    rows_returned: numberOrNull(record.rows_returned, `${label}.rows_returned`),
  };
}

function parseOutgoing(value: unknown, label: string): OutgoingCall {
  const record = recordAt(value, label);
  return {
    offset_us: numberAt(record.offset_us, `${label}.offset_us`),
    duration_us: numberAt(record.duration_us, `${label}.duration_us`),
    method: stringAt(record.method, `${label}.method`),
    url: stringAt(record.url, `${label}.url`),
    status: numberOrNull(record.status, `${label}.status`),
    request: parseMessage(record.request, `${label}.request`),
    response:
      record.response === null
        ? null
        : parseMessage(record.response, `${label}.response`),
    error: stringOrNull(record.error, `${label}.error`),
  };
}

function parseRedaction(value: unknown, label: string): Redaction {
  const record = recordAt(value, label);
  return {
    path: stringAt(record.path, `${label}.path`),
    rule: stringAt(record.rule, `${label}.rule`),
  };
}

function parseFlags(value: unknown, label: string): Flags {
  const record = recordAt(value, label);
  return {
    has_error: booleanAt(record.has_error, `${label}.has_error`),
    panicked: booleanAt(record.panicked, `${label}.panicked`),
    truncated_request: booleanAt(
      record.truncated_request,
      `${label}.truncated_request`,
    ),
    truncated_response: booleanAt(
      record.truncated_response,
      `${label}.truncated_response`,
    ),
    streaming: booleanAt(record.streaming, `${label}.streaming`),
    dropped_events: numberAt(record.dropped_events, `${label}.dropped_events`),
  };
}

function recordAt(value: unknown, label: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(label);
  }
  return value as Record<string, unknown>;
}

function arrayAt(value: unknown, label: string): unknown[] {
  if (!Array.isArray(value)) {
    throw new Error(label);
  }
  return value;
}

function stringAt(value: unknown, label: string): string {
  if (typeof value !== "string") {
    throw new Error(label);
  }
  return value;
}

function stringOrNull(value: unknown, label: string): string | null {
  if (value === null) {
    return null;
  }
  return stringAt(value, label);
}

function numberAt(value: unknown, label: string): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new Error(label);
  }
  return value;
}

function numberOrNull(value: unknown, label: string): number | null {
  if (value === null) {
    return null;
  }
  return numberAt(value, label);
}

function booleanAt(value: unknown, label: string): boolean {
  if (typeof value !== "boolean") {
    throw new Error(label);
  }
  return value;
}
