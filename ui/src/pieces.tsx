import { Check, Copy } from "lucide-preact";
import type { ComponentChildren } from "preact";
import { useLayoutEffect, useRef, useState } from "preact/hooks";
import { statusPhrase, statusTone } from "./format/status";
import { isFullyMasked, isMasked } from "./json/tree";

export const TOAST_EVENT = "oxray:toast";

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

export function RedactedChip(props: { spaced: boolean }) {
  return (
    <span
      class={`${props.spaced ? "ml-2 " : ""}inline-flex h-4 items-center rounded-[6px] bg-warn/10 px-1.5 text-[11px] leading-[16px] font-medium text-warn-strong`}
    >
      redacted
    </span>
  );
}

export function copyText(value: string, what: string, done: () => void) {
  // The Clipboard API only exists on localhost and HTTPS.
  const clipboard = window.isSecureContext ? navigator.clipboard : undefined;
  if (clipboard === undefined) {
    return;
  }
  void clipboard.writeText(value).then(
    () => {
      done();
      window.dispatchEvent(
        new CustomEvent(TOAST_EVENT, { detail: `Copied ${what}` }),
      );
    },
    () => undefined,
  );
}

export function CopyButton(props: {
  value: string;
  what: string;
  quiet: boolean;
  class: string;
}) {
  const [copied, setCopied] = useState(false);
  const reveal =
    props.quiet && !copied
      ? "opacity-0 group-hover:opacity-100 group-focus-within:opacity-100"
      : "";
  const Icon = copied ? Check : Copy;
  return (
    <button
      type="button"
      class={`inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-[6px] text-text-2 hover:bg-control-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent ${reveal} ${props.class}`}
      aria-label={copied ? `Copied ${props.what}` : `Copy ${props.what}`}
      title={`Copy ${props.what}`}
      onClick={() => {
        copyText(props.value, props.what, () => {
          setCopied(true);
          window.setTimeout(() => {
            setCopied(false);
          }, 2000);
        });
      }}
    >
      <Icon size={16} strokeWidth={1.5} aria-hidden="true" />
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
        <FieldRow
          key={`${row.name}-${index}`}
          name={row.name}
          value={row.value}
        />
      ))}
    </div>
  );
}

function FieldRow(props: { name: string; value: string }) {
  const rowRef = useRef<HTMLDivElement>(null);
  const valueRef = useRef<HTMLElement | null>(null);
  const [cut, setCut] = useState(false);
  const [open, setOpen] = useState(false);
  useLayoutEffect(() => {
    const row = rowRef.current;
    if (row === null || open) {
      return;
    }
    const measure = () => {
      const node = valueRef.current;
      if (node !== null) {
        setCut(node.scrollWidth > node.clientWidth);
      }
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(row);
    return () => {
      observer.disconnect();
    };
  }, [open]);
  const content = isFullyMasked(props.value) ? (
    <RedactedChip spaced={false} />
  ) : (
    <>
      {props.value}
      {isMasked(props.value) ? <RedactedChip spaced /> : null}
    </>
  );
  const keep = (node: HTMLElement | null) => {
    valueRef.current = node;
  };
  const valueClass = `min-w-0 flex-1 text-left font-mono text-[13px] leading-[20px] text-text ${open ? "py-1.5 break-words whitespace-pre-wrap" : "truncate"}`;
  return (
    <div
      ref={rowRef}
      class={`group flex gap-3 ${open ? "items-start" : "h-8 items-center"}`}
    >
      <span
        class={`w-[30%] shrink-0 truncate font-mono text-[12px] leading-[16px] text-text-2 ${open ? "pt-2" : ""}`}
        title={props.name}
      >
        {props.name}
      </span>
      {cut || open ? (
        <button
          ref={keep}
          type="button"
          class={`${valueClass} rounded-[6px] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent`}
          aria-expanded={open}
          title={open ? undefined : props.value}
          onClick={() => {
            setOpen(!open);
          }}
        >
          {content}
        </button>
      ) : (
        <span ref={keep} class={valueClass} title={props.value}>
          {content}
        </span>
      )}
      <CopyButton
        value={props.value}
        what={props.name}
        quiet
        class={open ? "mt-0.5" : ""}
      />
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
