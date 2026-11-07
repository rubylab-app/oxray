import { ArrowUpRight, Database, EyeOff, Scissors } from "lucide-preact";
import type { ComponentType } from "preact";
import { useEffect, useLayoutEffect, useRef } from "preact/hooks";
import { formatDuration } from "./format/duration";
import { formatAbsolute, formatRelative } from "./format/time";
import type { CaptureSummary } from "./model/capture";
import { CopyButton, RouteLabel, StatusMark } from "./pieces";
import { useRowVirtualizer } from "./virtual";

const rowGrid =
  "grid h-8 w-full grid-cols-[56px_64px_minmax(160px,1fr)_88px_80px] content-center items-baseline gap-x-3 px-4 text-left @min-[612px]:grid-cols-[72px_56px_64px_minmax(160px,1fr)_88px_80px]";

const LOAD_AHEAD = 50;
const ROW_HEIGHT = 32;
const SKELETON_ROWS = 3;

export function RequestList(props: {
  phase: "loading" | "slow" | "empty" | "error" | "ready";
  items: CaptureSummary[];
  hasMore: boolean;
  active: number | null;
  nowMs: number;
  focusRow: boolean;
  onChoose: (index: number) => void;
  onNearEnd: () => void;
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
    const command = `curl ${window.location.origin}/`;
    return (
      <div class="flex min-h-0 flex-1 flex-col gap-4 overflow-auto px-4 py-6">
        <p class="flex items-center gap-2 text-[14px] leading-[20px] text-text">
          <span
            class="inline-block size-1.5 shrink-0 rounded-full bg-text-3 motion-safe:animate-pulse motion-reduce:animate-none"
            aria-hidden="true"
          />
          Oxray is running. Send a request to this app and it shows up here.
        </p>
        <div class="flex items-center gap-2 rounded-[6px] border border-border bg-surface py-1 pr-1 pl-4">
          <code class="min-w-0 flex-1 truncate font-mono text-[13px] leading-[20px] text-text">
            {command}
          </code>
          <CopyButton value={command} what="command" quiet={false} class="" />
        </div>
        <p class="text-[13px] leading-[20px] text-text-3">
          Nothing showing up?{" "}
          <code class="font-mono text-text-2">.layer(oxray.layer())</code> must
          be the last layer.
        </p>
      </div>
    );
  }
  return (
    <ReadyList
      items={props.items}
      hasMore={props.hasMore}
      active={props.active}
      nowMs={props.nowMs}
      focusRow={props.focusRow}
      onChoose={props.onChoose}
      onNearEnd={props.onNearEnd}
    />
  );
}

function ReadyList(props: {
  items: CaptureSummary[];
  hasMore: boolean;
  active: number | null;
  nowMs: number;
  focusRow: boolean;
  onChoose: (index: number) => void;
  onNearEnd: () => void;
}) {
  const parentRef = useRef<HTMLDivElement>(null);
  const virtualizer = useRowVirtualizer(
    props.items.length + (props.hasMore ? SKELETON_ROWS : 0),
    parentRef,
  );
  const rows = virtualizer.getVirtualItems();
  const lastRow = rows.at(-1)?.index ?? -1;
  const firstId = props.items[0]?.id;
  const shownFirst = useRef(firstId);
  // New requests land on top. When the user has scrolled, keep the rows they
  // are reading where they were.
  useLayoutEffect(() => {
    const before = shownFirst.current;
    shownFirst.current = firstId;
    const scroller = parentRef.current;
    if (before === undefined || before === firstId || scroller === null) {
      return;
    }
    if (scroller.scrollTop === 0) {
      return;
    }
    const shift = props.items.findIndex((item) => item.id === before);
    if (shift > 0) {
      scroller.scrollTop += shift * ROW_HEIGHT;
    }
  }, [firstId, props.items]);
  useEffect(() => {
    if (props.hasMore && lastRow >= props.items.length - LOAD_AHEAD) {
      props.onNearEnd();
    }
  }, [props.hasMore, props.items.length, props.onNearEnd, lastRow]);
  const activeIndex = useRef(props.active);
  activeIndex.current = props.active;
  const activeKey =
    props.active === null ? null : (props.items[props.active]?.id ?? null);
  // Rows landing on top move the selection's index, not the selection, so
  // only a new selected capture scrolls the list or takes focus.
  useEffect(() => {
    const index = activeIndex.current;
    if (!props.focusRow || activeKey === null || index === null) {
      return;
    }
    virtualizer.scrollToIndex(index, { align: "auto" });
    const node = parentRef.current?.querySelector<HTMLElement>(
      `[data-index="${index}"]`,
    );
    node?.focus({ preventScroll: true });
  }, [props.focusRow, activeKey, virtualizer]);
  return (
    <div class="@container flex min-h-0 flex-1 flex-col">
      <div
        class={`${rowGrid} h-7 shrink-0 border-b border-border text-[12px] leading-[16px] text-text-3`}
      >
        <span class="hidden @min-[612px]:block">Time</span>
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
              return (
                <div
                  key={`skeleton-${row.index}`}
                  aria-hidden="true"
                  class="absolute top-0 left-0 flex h-8 w-full items-center px-4"
                  style={{ transform: `translateY(${row.start}px)` }}
                >
                  <div class="h-3 w-full rounded-[6px] bg-surface" />
                </div>
              );
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
                {selected ? (
                  <span
                    aria-hidden="true"
                    class="absolute inset-y-0 left-0 before:absolute before:inset-y-0 before:left-0 before:w-0.5 before:bg-accent"
                  />
                ) : null}
                <span
                  class="hidden truncate font-mono text-[12px] leading-[16px] text-text-3 @min-[612px]:block"
                  title={formatAbsolute(item.started_at)}
                >
                  {formatRelative(item.started_at, props.nowMs)}
                  <span class="sr-only">, </span>
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
                  <span class="sr-only">, </span>
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

type Signal = {
  key: string;
  label: string;
  count: number | null;
  icon: ComponentType<{ size: number; strokeWidth: number }>;
};

function signalsOf(item: CaptureSummary): Signal[] {
  const signals: Signal[] = [];
  if (item.counts.queries > 0) {
    signals.push({
      key: "queries",
      label: plural(item.counts.queries, "query", "queries"),
      count: item.counts.queries,
      icon: Database,
    });
  }
  if (item.counts.outgoing > 0) {
    signals.push({
      key: "outgoing",
      label: plural(item.counts.outgoing, "outgoing call", "outgoing calls"),
      count: item.counts.outgoing,
      icon: ArrowUpRight,
    });
  }
  if (item.counts.redactions > 0) {
    signals.push({
      key: "redacted",
      label: "Redacted",
      count: null,
      icon: EyeOff,
    });
  }
  if (item.flags.truncated) {
    signals.push({
      key: "truncated",
      label: "Truncated",
      count: null,
      icon: Scissors,
    });
  }
  return signals;
}

function Signals(props: { item: CaptureSummary }) {
  const signals = signalsOf(props.item);
  const shown = signals.slice(0, 4);
  const extra = signals.length - shown.length;
  return (
    <span class="flex items-center gap-1 self-center overflow-hidden text-text-2">
      {shown.map((signal) => {
        const Icon = signal.icon;
        const spoken = `, ${signal.label}`;
        return (
          <span
            key={signal.key}
            class="inline-flex items-center gap-0.5"
            title={signal.label}
          >
            <Icon size={14} strokeWidth={1.5} aria-hidden="true" />
            {signal.count === null ? null : (
              <span
                aria-hidden="true"
                class="font-mono text-[12px] leading-[16px] tabular-nums"
              >
                {signal.count}
              </span>
            )}
            <span class="sr-only">{spoken}</span>
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
