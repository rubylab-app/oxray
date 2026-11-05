import { useState } from "preact/hooks";
import { bodyNotice } from "./format/body";
import { formatCount } from "./format/count";
import { formatDuration } from "./format/duration";
import { tokenizeSql } from "./format/sql";
import { formatAbsolute, formatRelative } from "./format/time";
import { JsonOrText } from "./json-view";
import type { Capture, Message, QueryEntry } from "./model/capture";
import { FieldList, RouteLabel, Section, StatusMark } from "./pieces";

type Tab = "request" | "response" | "queries" | "outgoing" | "logs";

export function RequestDetail(props: {
  phase: "idle" | "loading" | "slow" | "missing" | "error" | "ready";
  capture: Capture | null;
  nowMs: number;
  listHref: string;
  onBack: () => void;
}) {
  if (props.phase === "idle") {
    return (
      <div class="flex flex-1 flex-col items-center justify-center gap-2 px-6">
        <p class="text-[14px] leading-[20px] text-text">Select a request</p>
        <p class="text-[13px] leading-[20px] text-text-3">Press Enter</p>
      </div>
    );
  }
  if (props.phase === "loading") {
    return <div class="flex-1" />;
  }
  if (props.phase === "slow") {
    return (
      <div aria-busy="true" class="flex flex-1 flex-col gap-3 px-6 py-5">
        <div class="h-5 w-2/3 rounded-[6px] bg-surface" />
        <div class="h-3 w-1/2 rounded-[6px] bg-surface" />
      </div>
    );
  }
  if (props.phase === "missing") {
    return (
      <div class="flex flex-1 flex-col items-start gap-3 px-6 py-5">
        <p class="text-[14px] leading-[20px] text-text">
          This capture is no longer in the store
        </p>
        <a
          class="text-[13px] leading-[20px] font-medium text-text-2 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
          href={props.listHref}
          onClick={(event) => {
            event.preventDefault();
            props.onBack();
          }}
        >
          Back to the list
        </a>
      </div>
    );
  }
  if (props.phase === "error" || props.capture === null) {
    return (
      <p class="px-6 py-5 text-[14px] leading-[20px] text-text">
        Couldn't load captures. Retrying…
      </p>
    );
  }
  return <ReadyDetail capture={props.capture} nowMs={props.nowMs} />;
}

function ReadyDetail(props: { capture: Capture; nowMs: number }) {
  const [tab, setTab] = useState<Tab>("request");
  const capture = props.capture;
  const tabs: Array<{ id: Tab; label: string; count: number | null }> = [
    { id: "request", label: "Request", count: null },
    { id: "response", label: "Response", count: null },
    { id: "queries", label: "Queries", count: capture.queries.length },
    { id: "outgoing", label: "Outgoing", count: capture.outgoing.length },
    { id: "logs", label: "Logs", count: capture.logs.length },
  ];
  return (
    <div class="flex min-h-0 flex-1 flex-col">
      <div class="px-6 pt-5 pb-4">
        <div class="flex items-start gap-3">
          <span class="font-mono text-[14px] leading-[20px] font-semibold text-text-2">
            {capture.method}
          </span>
          <h1 class="min-w-0 flex-1">
            <RouteLabel
              matched={capture.matched_path}
              path={capture.path}
              large
            />
          </h1>
        </div>
        <p class="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-[13px] leading-[20px] text-text-2">
          <StatusMark status={capture.status} phrase />
          <span class="font-mono text-text tabular-nums">
            {formatDuration(capture.duration_us)}
          </span>
          <span title={formatAbsolute(capture.started_at)}>
            {formatRelative(capture.started_at, props.nowMs)}
          </span>
          {capture.matched_path !== null &&
          capture.matched_path !== capture.path ? (
            <span class="font-mono text-text">{capture.path}</span>
          ) : null}
          <span class="font-mono text-[12px] leading-[16px] text-text-3">
            {capture.short_id}
          </span>
          <CopyId id={capture.id} />
        </p>
      </div>
      <div
        role="tablist"
        aria-label="Capture sections"
        class="flex h-9 shrink-0 gap-4 border-b border-border px-6"
      >
        {tabs.map((item) => {
          const selected = tab === item.id;
          const quiet =
            !selected &&
            ((item.id === "response" && capture.response === null) ||
              (item.count !== null && item.count === 0));
          return (
            <button
              key={item.id}
              type="button"
              role="tab"
              id={`tab-${item.id}`}
              aria-selected={selected}
              aria-controls={`panel-${item.id}`}
              tabIndex={selected ? 0 : -1}
              class={`inline-flex h-9 items-center border-b-2 text-[13px] leading-[20px] font-medium focus-visible:outline focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-accent ${selected ? "border-accent text-text" : "border-transparent"} ${quiet ? "text-text-3" : ""} ${!selected && !quiet ? "text-text-2" : ""}`}
              onClick={() => {
                setTab(item.id);
              }}
              onKeyDown={(event) => {
                if (event.key !== "ArrowRight" && event.key !== "ArrowLeft") {
                  return;
                }
                event.preventDefault();
                event.stopPropagation();
                const index = tabs.findIndex((entry) => entry.id === item.id);
                const delta = event.key === "ArrowRight" ? 1 : -1;
                const next = tabs[(index + delta + tabs.length) % tabs.length];
                if (next) {
                  setTab(next.id);
                  document.getElementById(`tab-${next.id}`)?.focus();
                }
              }}
            >
              {item.label}
              {item.count === null ? null : (
                <span class="ml-1 font-mono text-[12px] leading-[16px] font-normal text-text-3 tabular-nums">
                  {formatCount(item.count)}
                </span>
              )}
            </button>
          );
        })}
      </div>
      <div
        role="tabpanel"
        id={`panel-${tab}`}
        aria-labelledby={`tab-${tab}`}
        class="min-h-0 flex-1 overflow-auto px-6 py-4"
      >
        {tab === "request" ? (
          <MessageView message={capture.request} query={capture.query} />
        ) : null}
        {tab === "response" ? (
          capture.response === null ? (
            <p class="text-[14px] leading-[20px] text-text">
              No response: the handler panicked before responding
            </p>
          ) : (
            <MessageView message={capture.response} query={null} />
          )
        ) : null}
        {tab === "queries" ? <QueryList queries={capture.queries} /> : null}
        {tab === "outgoing" ? <OutgoingList capture={capture} /> : null}
        {tab === "logs" ? <LogList capture={capture} /> : null}
      </div>
    </div>
  );
}

function CopyId(props: { id: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <button
      type="button"
      class="inline-flex h-7 items-center rounded-[6px] px-2 text-[12px] leading-[16px] font-medium text-text-2 hover:bg-control-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
      onClick={() => {
        void navigator.clipboard.writeText(props.id).then(
          () => {
            setCopied(true);
            window.setTimeout(() => {
              setCopied(false);
            }, 2000);
          },
          () => {
            setCopied(false);
          },
        );
      }}
    >
      {copied ? "Copied" : "Copy id"}
    </button>
  );
}

function MessageView(props: { message: Message; query: string | null }) {
  const notice = bodyNotice(props.message.body);
  const text = props.message.body.text;
  return (
    <div class="flex flex-col gap-5">
      <Section title="Body">
        {notice === null ? null : (
          <p class="text-[13px] leading-[20px] text-text-2">{notice}</p>
        )}
        {text === null || text.length === 0 ? null : <JsonOrText text={text} />}
      </Section>
      {props.query === null ? null : (
        <Section title="Query params">
          <FieldList
            rows={queryRows(props.query)}
            empty="No query parameters"
          />
        </Section>
      )}
      <Section title="Headers">
        <FieldList
          rows={props.message.headers.map((header) => ({
            name: header.name,
            value: header.value,
          }))}
          empty="No headers"
        />
      </Section>
    </div>
  );
}

function queryRows(query: string): Array<{ name: string; value: string }> {
  if (query.length === 0) {
    return [];
  }
  return Array.from(new URLSearchParams(query), ([name, value]) => ({
    name,
    value,
  }));
}

function QueryList(props: { queries: QueryEntry[] }) {
  if (props.queries.length === 0) {
    return <p class="text-[13px] leading-[20px] text-text-3">No queries</p>;
  }
  return (
    <div class="flex flex-col gap-2">
      {props.queries.map((query, index) => {
        const rows = query.rows_returned ?? query.rows_affected;
        const rowText =
          rows === null
            ? null
            : rows === 1
              ? "1 row"
              : `${formatCount(rows)} rows`;
        return (
          <div
            key={`${query.offset_us}-${index}`}
            class="flex items-start gap-3 rounded-[6px] border border-border bg-surface p-3"
          >
            <p class="min-w-0 flex-1 font-mono text-[13px] leading-[20px] text-text">
              {tokenizeSql(query.sql).map((token, tokenIndex) =>
                token.kind === "keyword" ? (
                  <span key={tokenIndex} class="text-code-literal">
                    {token.text}
                  </span>
                ) : (
                  <span key={tokenIndex}>{token.text}</span>
                ),
              )}
            </p>
            <p class="shrink-0 font-mono text-[12px] leading-[16px] text-text-3 tabular-nums">
              {formatDuration(query.duration_us)}
              {rowText === null ? "" : ` · ${rowText}`}
            </p>
          </div>
        );
      })}
    </div>
  );
}

function OutgoingList(props: { capture: Capture }) {
  const [open, setOpen] = useState<number | null>(null);
  if (props.capture.outgoing.length === 0) {
    return (
      <p class="text-[13px] leading-[20px] text-text-3">No outgoing calls</p>
    );
  }
  return (
    <div class="flex flex-col">
      {props.capture.outgoing.map((call, index) => {
        const expanded = open === index;
        return (
          <div key={`${call.url}-${index}`} class="border-b border-border py-2">
            <button
              type="button"
              class="flex w-full items-center gap-3 text-left focus-visible:outline focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-accent"
              aria-expanded={expanded}
              onClick={() => {
                setOpen(expanded ? null : index);
              }}
            >
              <span class="font-mono text-[12px] leading-[16px] font-semibold text-text-2">
                {call.method}
              </span>
              <span class="min-w-0 flex-1 truncate font-mono text-[13px] leading-[20px] text-text">
                {call.url}
              </span>
              <StatusMark status={call.status} phrase={false} />
              <span class="font-mono text-[12px] leading-[16px] text-text-3 tabular-nums">
                {formatDuration(call.duration_us)}
              </span>
            </button>
            {expanded ? (
              <div class="flex flex-col gap-4 pt-3">
                <Section title="Request headers">
                  <FieldList
                    rows={call.request.headers.map((header) => ({
                      name: header.name,
                      value: header.value,
                    }))}
                    empty="No headers"
                  />
                </Section>
                {call.request.body.text === null ? null : (
                  <Section title="Request body">
                    <JsonOrText text={call.request.body.text} />
                  </Section>
                )}
                {call.response === null ? null : (
                  <Section title="Response headers">
                    <FieldList
                      rows={call.response.headers.map((header) => ({
                        name: header.name,
                        value: header.value,
                      }))}
                      empty="No headers"
                    />
                  </Section>
                )}
                {call.response?.body.text === null ||
                call.response?.body.text === undefined ? null : (
                  <Section title="Response body">
                    <JsonOrText text={call.response.body.text} />
                  </Section>
                )}
              </div>
            ) : null}
          </div>
        );
      })}
    </div>
  );
}

function LogList(props: { capture: Capture }) {
  const [open, setOpen] = useState<number | null>(null);
  if (props.capture.logs.length === 0) {
    return <p class="text-[13px] leading-[20px] text-text-3">No logs</p>;
  }
  return (
    <div class="flex flex-col gap-3">
      {props.capture.logs.map((entry, index) => {
        const fields = Object.entries(entry.fields);
        const expanded = open === index;
        return (
          <div key={`${entry.offset_us}-${index}`}>
            <p class="flex flex-wrap items-baseline gap-x-2 text-[13px] leading-[20px]">
              <span class={levelClass(entry.level)}>{entry.level}</span>
              <span class="font-mono text-[12px] leading-[16px] text-text-3">
                {entry.target}
              </span>
              <span class="text-text">{entry.message}</span>
            </p>
            {fields.length === 0 ? null : (
              <button
                type="button"
                class="mt-1 text-[13px] leading-[20px] font-medium text-text-2 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
                aria-expanded={expanded}
                onClick={() => {
                  setOpen(expanded ? null : index);
                }}
              >
                {expanded ? "Hide fields" : "Show fields"}
              </button>
            )}
            {expanded ? (
              <div class="pt-2">
                <FieldList
                  rows={fields.map(([name, value]) => ({
                    name,
                    value: fieldText(value),
                  }))}
                  empty="No fields"
                />
              </div>
            ) : null}
          </div>
        );
      })}
    </div>
  );
}

function fieldText(value: unknown): string {
  if (typeof value === "string") {
    return value;
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  if (value === null) {
    return "empty";
  }
  return JSON.stringify(value) ?? "empty";
}

function levelClass(level: string): string {
  const name = level.toUpperCase();
  const base = "font-mono text-[12px] leading-[16px] font-semibold";
  if (name === "ERROR") {
    return `${base} text-err`;
  }
  if (name === "WARN" || name === "WARNING") {
    return `${base} text-warn`;
  }
  if (name === "INFO") {
    return `${base} text-text-2`;
  }
  return `${base} text-text-3`;
}
