import { ArrowUpRight, Database, EyeOff, Scissors } from "lucide-preact";
import type { ComponentType } from "preact";
import { useEffect, useRef } from "preact/hooks";
import { formatDuration } from "./format/duration";
import { formatAbsolute, formatRelative } from "./format/time";
import type { CaptureSummary } from "./model/capture";
import { RouteLabel, StatusMark } from "./pieces";
import { useRowVirtualizer } from "./virtual";

const INSTALL = `.with(oxray.tracing_layer())
.merge(oxray.router())
.layer(oxray.layer());`;

const rowGrid =
  "grid h-8 w-full grid-cols-[56px_64px_minmax(160px,1fr)_88px_96px] items-center gap-x-3 px-4 text-left @min-[680px]:grid-cols-[88px_56px_64px_minmax(160px,1fr)_88px_96px]";

export function RequestList(props: {
  phase: "loading" | "slow" | "empty" | "error" | "ready";
  items: CaptureSummary[];
  active: number | null;
  nowMs: number;
  focusRow: boolean;
  onChoose: (index: number) => void;
}) {
  if (props.phase === "loading") {
    return <div class="min-h-0 flex-1" />;
  }
  if (props.phase === "slow") {
    return (
      <div
        aria-busy="true"
        class="flex min-h-0 flex-1 flex-col gap-3 px-4 py-3"
      >
        {Array.from({ length: 8 }, (_, index) => (
          <div key={index} class="h-3 rounded-[6px] bg-surface" />
        ))}
      </div>
    );
  }
  if (props.phase === "error") {
    return (
      <p class="px-4 py-6 text-[14px] leading-[20px] text-text">
        Couldn't load captures. Retrying…
      </p>
    );
  }
  if (props.phase === "empty") {
    return (
      <div class="flex min-h-0 flex-1 flex-col gap-4 overflow-auto px-4 py-6">
        <pre class="overflow-auto rounded-[6px] border border-border bg-surface p-4 font-mono text-[13px] leading-[20px] text-text">
          {INSTALL}
        </pre>
        <p class="flex items-center gap-2 text-[14px] leading-[20px] text-text-2">
          <span
            class="inline-block size-1.5 rounded-full bg-text-3 motion-safe:animate-pulse motion-reduce:animate-none"
            aria-hidden="true"
          />
          Waiting for the first request…
        </p>
      </div>
    );
  }
  return (
    <ReadyList
      items={props.items}
      active={props.active}
      nowMs={props.nowMs}
      focusRow={props.focusRow}
      onChoose={props.onChoose}
    />
  );
}

function ReadyList(props: {
  items: CaptureSummary[];
  active: number | null;
  nowMs: number;
  focusRow: boolean;
  onChoose: (index: number) => void;
}) {
  const parentRef = useRef<HTMLDivElement>(null);
  const virtualizer = useRowVirtualizer(props.items.length, parentRef);
  const rows = virtualizer.getVirtualItems();
  useEffect(() => {
    if (!props.focusRow || props.active === null) {
      return;
    }
    virtualizer.scrollToIndex(props.active, { align: "auto" });
    const node = parentRef.current?.querySelector<HTMLElement>(
      `[data-index="${props.active}"]`,
    );
    node?.focus();
  }, [props.focusRow, props.active, virtualizer]);
  return (
    <div class="@container flex min-h-0 flex-1 flex-col">
      <div
        class={`${rowGrid} h-7 shrink-0 border-b border-border text-[12px] leading-[16px] text-text-3`}
      >
        <span class="hidden @min-[680px]:block">Time</span>
        <span>Status</span>
        <span>Method</span>
        <span>Route</span>
        <span class="text-right">Duration</span>
        <span>Signals</span>
      </div>
      <div ref={parentRef} class="min-h-0 flex-1 overflow-auto">
        <div
          class="relative w-full"
          style={{ height: `${virtualizer.getTotalSize()}px` }}
          role="presentation"
        >
          {rows.map((row) => {
            const item = props.items[row.index];
            if (item === undefined) {
              return null;
            }
            const selected = props.active === row.index;
            return (
              <button
                key={item.id}
                type="button"
                data-index={row.index}
                aria-current={selected ? "true" : undefined}
                tabIndex={
                  selected || (props.active === null && row.index === 0)
                    ? 0
                    : -1
                }
                class={`${rowGrid} absolute top-0 left-0 hover:bg-row-hover focus-visible:outline focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-accent ${selected ? "bg-row-selected" : ""}`}
                style={{ transform: `translateY(${row.start}px)` }}
                onClick={() => {
                  props.onChoose(row.index);
                }}
              >
                <span
                  class="hidden truncate font-mono text-[12px] leading-[16px] text-text-3 @min-[680px]:block"
                  title={formatAbsolute(item.started_at)}
                >
                  {formatRelative(item.started_at, props.nowMs)}
                </span>
                <span>
                  <StatusMark status={item.status} phrase={false} />
                </span>
                <span class="font-mono text-[12px] leading-[16px] font-semibold text-text-2">
                  {item.method}
                </span>
                <RouteLabel
                  matched={item.matched_path}
                  path={item.path}
                  large={false}
                />
                <span class="text-right font-mono text-[13px] leading-[20px] text-text tabular-nums">
                  {formatDuration(item.duration_us)}
                </span>
                <Signals item={item} />
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}

function Signals(props: { item: CaptureSummary }) {
  const signals: Array<{
    key: string;
    label: string;
    count: number | null;
    icon: ComponentType<{ size: number; strokeWidth: number }>;
  }> = [];
  if (props.item.counts.queries > 0) {
    signals.push({
      key: "queries",
      label: plural(props.item.counts.queries, "query", "queries"),
      count: props.item.counts.queries,
      icon: Database,
    });
  }
  if (props.item.counts.outgoing > 0) {
    signals.push({
      key: "outgoing",
      label: plural(
        props.item.counts.outgoing,
        "outgoing call",
        "outgoing calls",
      ),
      count: props.item.counts.outgoing,
      icon: ArrowUpRight,
    });
  }
  if (props.item.counts.redactions > 0) {
    signals.push({
      key: "redacted",
      label: "Redacted",
      count: null,
      icon: EyeOff,
    });
  }
  if (props.item.flags.truncated) {
    signals.push({
      key: "truncated",
      label: "Truncated",
      count: null,
      icon: Scissors,
    });
  }
  const shown = signals.slice(0, 4);
  const extra = signals.length - shown.length;
  return (
    <span class="flex items-center gap-1 overflow-hidden text-text-2">
      {shown.map((signal) => {
        const Icon = signal.icon;
        return (
          <span
            key={signal.key}
            class="inline-flex items-center gap-0.5"
            title={signal.label}
          >
            <Icon size={14} strokeWidth={1.5} aria-hidden="true" />
            {signal.count === null ? null : (
              <span class="font-mono text-[12px] leading-[16px] tabular-nums">
                {signal.count}
              </span>
            )}
            <span class="sr-only">{signal.label}</span>
          </span>
        );
      })}
      {extra > 0 ? (
        <span class="font-mono text-[12px] leading-[16px] text-text-3">
          +{extra}
        </span>
      ) : null}
    </span>
  );
}

function plural(count: number, one: string, many: string): string {
  return count === 1 ? `1 ${one}` : `${count} ${many}`;
}
