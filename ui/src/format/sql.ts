const KEYWORDS = new Set([
  "select",
  "from",
  "where",
  "and",
  "or",
  "insert",
  "into",
  "values",
  "update",
  "set",
  "delete",
  "join",
  "left",
  "right",
  "inner",
  "outer",
  "on",
  "as",
  "order",
  "by",
  "group",
  "limit",
  "offset",
  "returning",
  "in",
  "is",
  "not",
  "like",
  "between",
  "exists",
  "case",
  "when",
  "then",
  "else",
  "end",
  "distinct",
  "count",
  "with",
]);

export type SqlToken = {
  kind: "keyword" | "text";
  text: string;
};

export function tokenizeSql(sql: string): SqlToken[] {
  const tokens: SqlToken[] = [];
  const pattern = /[A-Za-z_][A-Za-z0-9_]*|\s+|[^A-Za-z_\s]+/g;
  for (const match of sql.matchAll(pattern)) {
    const text = match[0];
    if (text === undefined) {
      continue;
    }
    const kind = KEYWORDS.has(text.toLowerCase()) ? "keyword" : "text";
    tokens.push({ kind, text });
  }
  return tokens;
}
