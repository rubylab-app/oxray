import { ChevronRight } from "lucide-preact";
import { useState } from "preact/hooks";
import {
  countNodes,
  isMasked,
  isObject,
  parseJsonText,
  previewString,
} from "./json/tree";
import { CopyButton, RedactedChip } from "./pieces";

export function JsonOrText(props: { text: string }) {
  const parsed = parseJsonText(props.text);
  if (!parsed.ok) {
    return <PlainText text={props.text} />;
  }
  const openByDefault = countNodes(parsed.value) <= 300;
  return (
    <div class="relative">
      <CopyButton
        value={JSON.stringify(parsed.value, null, 2)}
        label="Copy JSON"
        class="absolute top-0 right-0 inline-flex h-7 w-7 items-center justify-center rounded-[6px] text-text-2 hover:bg-control-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
      />
      <div class="pr-10 font-mono text-[13px] leading-[20px]">
        <JsonNode
          value={parsed.value}
          depth={0}
          openByDefault={openByDefault}
        />
      </div>
    </div>
  );
}

function PlainText(props: { text: string }) {
  const lines = props.text.split("\n");
  return (
    <div class="font-mono text-[13px] leading-[20px]">
      {lines.map((line, index) => (
        <div
          key={line.length === 0 ? `blank-${index}` : `${line}-${index}`}
          class="flex gap-3"
        >
          <span class="w-8 shrink-0 text-right text-text-3 tabular-nums">
            {index + 1}
          </span>
          <span class="min-w-0 text-text">
            {line.length === 0 ? " " : line}
          </span>
        </div>
      ))}
    </div>
  );
}

function JsonNode(props: {
  value: unknown;
  depth: number;
  openByDefault: boolean;
}) {
  if (Array.isArray(props.value)) {
    return (
      <JsonCollection
        values={props.value.map((item, index) => ({
          key: String(index),
          value: item,
          label: null,
        }))}
        empty="[…] 0 items"
        summary={`[…] ${props.value.length} items`}
        depth={props.depth}
        openByDefault={props.openByDefault}
        paged
      />
    );
  }
  if (isObject(props.value)) {
    const entries = Object.entries(props.value).map(([key, value]) => ({
      key,
      value,
      label: key,
    }));
    return (
      <JsonCollection
        values={entries}
        empty="{…} 0 keys"
        summary={`{…} ${entries.length} keys`}
        depth={props.depth}
        openByDefault={props.openByDefault}
        paged={false}
      />
    );
  }
  return <JsonLeaf value={props.value} />;
}

function JsonCollection(props: {
  values: Array<{ key: string; value: unknown; label: string | null }>;
  empty: string;
  summary: string;
  depth: number;
  openByDefault: boolean;
  paged: boolean;
}) {
  const [open, setOpen] = useState(props.openByDefault || props.depth <= 2);
  const [shown, setShown] = useState(100);
  if (!open) {
    return (
      <button
        type="button"
        class="inline-flex items-center gap-1 text-left text-text-3"
        aria-expanded="false"
        onClick={() => {
          setOpen(true);
        }}
      >
        <ChevronRight size={12} strokeWidth={1.5} aria-hidden="true" />
        {props.values.length === 0 ? props.empty : props.summary}
      </button>
    );
  }
  const visible = props.paged ? props.values.slice(0, shown) : props.values;
  const hidden = props.values.length - visible.length;
  return (
    <div>
      <button
        type="button"
        class="inline-flex items-center gap-1 text-left text-text-3"
        aria-expanded="true"
        onClick={() => {
          setOpen(false);
        }}
      >
        <ChevronRight
          size={12}
          strokeWidth={1.5}
          aria-hidden="true"
          class="rotate-90"
        />
        <span>{props.paged ? "[" : "{"}</span>
      </button>
      <div class="ml-2 border-l border-border pl-4">
        {visible.map((entry) => (
          <div key={entry.key} class="flex flex-wrap gap-x-2">
            {entry.label === null ? null : (
              <span class="text-text-2">
                "{entry.label}"<span class="text-text-3">:</span>
              </span>
            )}
            <JsonNode
              value={entry.value}
              depth={props.depth + 1}
              openByDefault={props.openByDefault}
            />
          </div>
        ))}
        {hidden > 0 ? (
          <button
            type="button"
            class="text-text-2"
            onClick={() => {
              setShown((value) => value + 20);
            }}
          >
            Show 20 more
          </button>
        ) : null}
      </div>
      <span class="text-text-3">{props.paged ? "]" : "}"}</span>
    </div>
  );
}

function JsonLeaf(props: { value: unknown }) {
  if (typeof props.value === "string") {
    return <JsonString value={props.value} />;
  }
  if (typeof props.value === "number" || typeof props.value === "boolean") {
    return <span class="text-code-literal">{String(props.value)}</span>;
  }
  if (props.value === null) {
    return <span class="text-code-literal">null</span>;
  }
  return <span class="text-text-3">…</span>;
}

function JsonString(props: { value: string }) {
  const [expanded, setExpanded] = useState(false);
  const text = previewString(props.value, expanded);
  return (
    <span>
      <span class="text-text-3">"</span>
      <span class="text-text">{text}</span>
      <span class="text-text-3">"</span>
      {!expanded && props.value.length > 200 ? (
        <button
          type="button"
          class="ml-2 text-text-2"
          onClick={() => {
            setExpanded(true);
          }}
        >
          show all
        </button>
      ) : null}
      {isMasked(props.value) ? <RedactedChip /> : null}
    </span>
  );
}
