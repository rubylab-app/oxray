import { Copy } from "lucide-preact";
import type { ComponentChildren } from "preact";
import { useState } from "preact/hooks";
import { statusPhrase, statusTone } from "./format/status";
import { isMasked } from "./json/tree";

export function StatusMark(props: { status: number | null; phrase: boolean }) {
  if (props.status === null) {
    return (
      <span
        role="img"
        class="font-mono text-[13px] leading-[20px] text-err"
        title="No response (panicked)"
        aria-label="No response (panicked)"
      >
        —
      </span>
    );
  }
  const tone = statusTone(props.status);
  if (tone === "neutral") {
    return (
      <span class="font-mono text-[13px] leading-[20px] text-text-2 tabular-nums">
        {props.status}
      </span>
    );
  }
  const color = tone === "err" ? "text-err-strong" : "text-warn-strong";
  const tint = tone === "err" ? "bg-err/10" : "bg-warn/10";
  const phrase = props.phrase ? statusPhrase(props.status) : null;
  return (
    <span class="inline-flex items-center gap-2">
      <span
        class={`inline-flex h-4 items-center rounded-[6px] px-1.5 font-mono text-[11px] leading-[16px] font-semibold tabular-nums ${tint} ${color}`}
      >
        {props.status}
      </span>
      {phrase === null ? null : (
        <span class="text-[13px] leading-[20px] text-text-2">{phrase}</span>
      )}
    </span>
  );
}

export function RouteLabel(props: {
  matched: string | null;
  path: string;
  large: boolean;
}) {
  const route = props.matched ?? props.path;
  const parts = route.split(/(\{[^}]+\})/);
  const missing = props.matched === null;
  const textClass = props.large
    ? "line-clamp-2 font-mono text-[20px] leading-[28px] font-medium whitespace-normal"
    : "truncate font-mono text-[13px] leading-[20px]";
  return (
    <span class="inline-flex min-w-0 items-center gap-2" title={props.path}>
      <span class={textClass}>
        {parts.map((part, index) => (
          <span
            key={`${part}-${index}`}
            class={
              part.startsWith("{") || missing ? "text-text-2" : "text-text"
            }
          >
            {part}
          </span>
        ))}
      </span>
      {missing ? (
        <span class="shrink-0 font-sans text-[12px] leading-[16px] font-normal text-text-3">
          no route
        </span>
      ) : null}
    </span>
  );
}

export function RedactedChip() {
  return (
    <span class="ml-2 inline-flex h-4 items-center rounded-[6px] bg-warn/10 px-1.5 text-[11px] leading-[16px] font-medium text-warn-strong">
      redacted
    </span>
  );
}

export function CopyButton(props: {
  value: string;
  label: string;
  class: string;
}) {
  const [copied, setCopied] = useState(false);
  return (
    <button
      type="button"
      class={props.class}
      aria-label={copied ? "Copied" : props.label}
      onClick={() => {
        void navigator.clipboard.writeText(props.value).then(
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
      <Copy size={16} strokeWidth={1.5} aria-hidden="true" />
    </button>
  );
}

export function FieldList(props: {
  rows: Array<{ name: string; value: string }>;
  empty: string;
}) {
  if (props.rows.length === 0) {
    return <p class="text-[13px] leading-[20px] text-text-3">{props.empty}</p>;
  }
  return (
    <div class="flex flex-col">
      {props.rows.map((row, index) => (
        <div
          key={`${row.name}-${index}`}
          class="group flex h-8 items-center gap-3"
        >
          <span
            class="w-[30%] truncate font-mono text-[12px] leading-[16px] text-text-2"
            title={row.name}
          >
            {row.name}
          </span>
          <span
            class="min-w-0 flex-1 truncate font-mono text-[13px] leading-[20px] text-text"
            title={row.value}
          >
            {row.value}
            {isMasked(row.value) ? <RedactedChip /> : null}
          </span>
          <CopyButton
            value={row.value}
            label={`Copy ${row.name}`}
            class="invisible inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-[6px] text-text-2 group-hover:visible group-focus-within:visible hover:bg-control-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
          />
        </div>
      ))}
    </div>
  );
}

export function Section(props: { title: string; children: ComponentChildren }) {
  return (
    <section class="flex flex-col gap-2">
      <h2 class="text-[16px] leading-[24px] font-medium text-text">
        {props.title}
      </h2>
      {props.children}
    </section>
  );
}
