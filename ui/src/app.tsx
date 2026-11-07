import { Monitor, Moon, RefreshCw, Sun } from "lucide-preact";
import { useEffect, useRef, useState } from "preact/hooks";
import { loadCapture, loadCaptures, mountPrefix } from "./api";
import { formatCount } from "./format/count";
import type { Capture, CaptureSummary } from "./model/capture";
import { appendPage, headline, refreshFirstPage } from "./model/capture";
import { captureIdFromPath, listPathname } from "./path";
import { TOAST_EVENT } from "./pieces";
import { RequestDetail } from "./request-detail";
import { RequestList } from "./request-list";

type Preference = "system" | "dark" | "light";

type ListPhase =
  | { kind: "loading"; slow: boolean }
  | { kind: "empty" }
  | { kind: "error" }
  | {
      kind: "ready";
      items: CaptureSummary[];
      cursor: string | null;
      more: "idle" | "loading" | "failed";
      moreAttempt: number;
    };

type DetailPhase =
  | { kind: "idle" }
  | { kind: "loading"; slow: boolean; previous: Capture | null }
  | { kind: "missing" }
  | { kind: "error"; previous: Capture | null }
  | { kind: "ready"; capture: Capture };

const storageKey = "oxray.theme";

const nextPreference: Record<Preference, Preference> = {
  system: "dark",
  dark: "light",
  light: "system",
};

const labels: Record<Preference, string> = {
  system: "System",
  dark: "Dark",
  light: "Light",
};

function readPreference(): Preference {
  const stored = localStorage.getItem(storageKey);
  if (stored === "dark" || stored === "light" || stored === "system") {
    return stored;
  }
  return "system";
}

function resolve(preference: Preference): "dark" | "light" {
  if (preference === "dark" || preference === "light") {
    return preference;
  }
  return window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

function apply(preference: Preference): void {
  document.documentElement.setAttribute("data-theme", resolve(preference));
  localStorage.setItem(storageKey, preference);
}

function previousOf(detail: DetailPhase): Capture | null {
  if (detail.kind === "ready") {
    return detail.capture;
  }
  if (detail.kind === "loading" || detail.kind === "error") {
    return detail.previous;
  }
  return null;
}

// While the next capture loads, the one on screen stays, so moving with j/k
// does not blank the panel. A capture that is only being refreshed stays even
// when its reload is slow or fails.
function detailView(
  detail: DetailPhase,
  id: string | null,
): {
  phase: "idle" | "loading" | "slow" | "missing" | "error" | "ready";
  capture: Capture | null;
} {
  if (detail.kind === "idle" || detail.kind === "missing") {
    return { phase: detail.kind, capture: null };
  }
  if (detail.kind === "ready") {
    return { phase: "ready", capture: detail.capture };
  }
  const previous = detail.previous;
  const same = previous !== null && previous.id === id;
  if (detail.kind === "error") {
    return same
      ? { phase: "ready", capture: previous }
      : { phase: "error", capture: null };
  }
  if (previous !== null && (same || !detail.slow)) {
    return { phase: "ready", capture: previous };
  }
  return { phase: detail.slow ? "slow" : "loading", capture: null };
}

export function App() {
  const [preference, setPreference] = useState<Preference>(readPreference);
  const [pathname, setPathname] = useState(() => window.location.pathname);
  const [wide, setWide] = useState(
    () => window.matchMedia("(min-width: 1280px)").matches,
  );
  const [reloadToken, setReloadToken] = useState(0);
  const [detailToken, setDetailToken] = useState(0);
  const [attempt, setAttempt] = useState(0);
  const [detailAttempt, setDetailAttempt] = useState(0);
  const [list, setList] = useState<ListPhase>({
    kind: "loading",
    slow: false,
  });
  const [offline, setOffline] = useState(false);
  const [detail, setDetail] = useState<DetailPhase>({ kind: "idle" });
  const [activeId, setActiveId] = useState<string | null>(null);
  const [focusRow, setFocusRow] = useState(false);
  const [nowMs, setNowMs] = useState(() => Date.now());
  const [toast, setToast] = useState<string | null>(null);
  const id = captureIdFromPath(pathname);
  const view = useRef({ list, activeId, wide, id, offline });
  view.current = { list, activeId, wide, id, offline };

  useEffect(() => {
    apply(preference);
    if (preference !== "system") {
      return;
    }
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => {
      document.documentElement.setAttribute("data-theme", resolve("system"));
    };
    media.addEventListener("change", onChange);
    return () => {
      media.removeEventListener("change", onChange);
    };
  }, [preference]);

  useEffect(() => {
    const media = window.matchMedia("(min-width: 1280px)");
    const onChange = () => {
      setWide(media.matches);
    };
    media.addEventListener("change", onChange);
    return () => {
      media.removeEventListener("change", onChange);
    };
  }, []);

  useEffect(() => {
    const onPop = () => {
      setPathname(window.location.pathname);
    };
    window.addEventListener("popstate", onPop);
    return () => {
      window.removeEventListener("popstate", onPop);
    };
  }, []);

  useEffect(() => {
    const timer = window.setInterval(() => {
      setNowMs(Date.now());
    }, 15_000);
    return () => {
      window.clearInterval(timer);
    };
  }, []);

  useEffect(() => {
    const onVisible = () => {
      if (
        document.visibilityState === "visible" &&
        view.current.list.kind === "ready"
      ) {
        setReloadToken((value) => value + 1);
      }
    };
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, []);

  useEffect(() => {
    let timer = 0;
    const onToast = (event: Event) => {
      if (!(event instanceof CustomEvent) || typeof event.detail !== "string") {
        return;
      }
      setToast(event.detail);
      window.clearTimeout(timer);
      timer = window.setTimeout(() => {
        setToast(null);
      }, 2000);
    };
    window.addEventListener(TOAST_EVENT, onToast);
    return () => {
      window.removeEventListener(TOAST_EVENT, onToast);
      window.clearTimeout(timer);
    };
  }, []);

  useEffect(() => {
    const controller = new AbortController();
    let activeLoad = true;
    const slowTimer = window.setTimeout(() => {
      if (!activeLoad) {
        return;
      }
      setList((current) =>
        current.kind === "loading" ? { kind: "loading", slow: true } : current,
      );
    }, 300);
    setList((current) =>
      current.kind === "ready" ||
      current.kind === "error" ||
      current.kind === "empty"
        ? current
        : { kind: "loading", slow: false },
    );
    void loadCaptures(null, controller.signal).then(
      (page) => {
        if (!activeLoad) {
          return;
        }
        setNowMs(Date.now());
        setAttempt(0);
        if (view.current.offline) {
          setOffline(false);
          setDetailToken((value) => value + 1);
        }
        setList((current) => {
          if (page.items.length === 0) {
            return { kind: "empty" };
          }
          const loaded =
            current.kind === "ready"
              ? refreshFirstPage(current, page)
              : { items: page.items, cursor: page.next_cursor };
          const same =
            current.kind === "ready" && loaded.cursor === current.cursor;
          return {
            kind: "ready",
            ...loaded,
            more: same ? current.more : "idle",
            moreAttempt: same ? current.moreAttempt : 0,
          };
        });
      },
      () => {
        if (!activeLoad || controller.signal.aborted) {
          return;
        }
        const shown = view.current.list.kind;
        if (shown === "ready" || shown === "empty") {
          setOffline(true);
          return;
        }
        setList({ kind: "error" });
      },
    );
    return () => {
      activeLoad = false;
      controller.abort();
      window.clearTimeout(slowTimer);
    };
  }, [reloadToken]);

  useEffect(() => {
    if (list.kind !== "empty" || offline) {
      return;
    }
    const timer = window.setInterval(() => {
      setReloadToken((value) => value + 1);
    }, 2000);
    return () => {
      window.clearInterval(timer);
    };
  }, [list.kind, offline]);

  const failing = list.kind === "error" || offline;
  useEffect(() => {
    if (!failing) {
      return;
    }
    const delay = Math.min(8000, 1000 * 2 ** attempt);
    const timer = window.setTimeout(() => {
      setAttempt((value) => value + 1);
      setReloadToken((value) => value + 1);
    }, delay);
    return () => {
      window.clearTimeout(timer);
    };
  }, [failing, attempt]);

  // A failing next page retries on its own backoff. It does not mean the app
  // is unreachable, so it never touches the first page or the offline notice.
  const moreFailed = list.kind === "ready" && list.more === "failed";
  const moreAttempt = list.kind === "ready" ? list.moreAttempt : 0;
  useEffect(() => {
    if (!moreFailed) {
      return;
    }
    const delay = Math.min(8000, 1000 * 2 ** Math.max(0, moreAttempt - 1));
    const timer = window.setTimeout(() => {
      setList((current) =>
        current.kind === "ready" && current.more === "failed"
          ? { ...current, more: "idle" }
          : current,
      );
    }, delay);
    return () => {
      window.clearTimeout(timer);
    };
  }, [moreFailed, moreAttempt]);

  useEffect(() => {
    setDetailAttempt(0);
  }, [id]);

  useEffect(() => {
    if (detail.kind !== "error" || id === null) {
      return;
    }
    const delay = Math.min(8000, 1000 * 2 ** detailAttempt);
    const timer = window.setTimeout(() => {
      setDetailAttempt((value) => value + 1);
      setDetailToken((value) => value + 1);
    }, delay);
    return () => {
      window.clearTimeout(timer);
    };
  }, [detail.kind, detailAttempt, id]);

  useEffect(() => {
    if (id === null) {
      setDetail({ kind: "idle" });
      return;
    }
    const controller = new AbortController();
    let activeLoad = true;
    const slowTimer = window.setTimeout(() => {
      if (!activeLoad) {
        return;
      }
      setDetail((current) =>
        current.kind === "loading" ? { ...current, slow: true } : current,
      );
    }, 300);
    setDetail((current) => ({
      kind: "loading",
      slow: false,
      previous: previousOf(current),
    }));
    void loadCapture(id, controller.signal).then(
      (capture) => {
        if (!activeLoad) {
          return;
        }
        setDetailAttempt(0);
        setDetail(
          capture === null ? { kind: "missing" } : { kind: "ready", capture },
        );
      },
      () => {
        if (!activeLoad || controller.signal.aborted) {
          return;
        }
        setDetail((current) => ({
          kind: "error",
          previous: previousOf(current),
        }));
      },
    );
    return () => {
      activeLoad = false;
      controller.abort();
      window.clearTimeout(slowTimer);
    };
  }, [id, detailToken]);

  useEffect(() => {
    if (id !== null) {
      setActiveId(id);
    }
  }, [id]);

  const shown = detailView(detail, id);
  const opened = id === null ? null : shown.capture;

  useEffect(() => {
    if (opened === null) {
      document.title = "Oxray";
      return;
    }
    const status = opened.status === null ? "No response" : opened.status;
    document.title = `${status} ${opened.method} ${opened.matched_path ?? opened.path} · Oxray`;
  }, [opened]);

  function go(path: string): void {
    if (window.location.pathname !== path) {
      window.history.pushState(null, "", path);
    }
    setPathname(window.location.pathname);
  }

  function detailPath(captureId: string): string {
    return `${mountPrefix()}/r/${encodeURIComponent(captureId)}`;
  }

  function refresh(): void {
    setReloadToken((value) => value + 1);
    setDetailToken((value) => value + 1);
  }

  function loadMore(): void {
    const state = view.current.list;
    if (
      state.kind !== "ready" ||
      state.cursor === null ||
      state.more !== "idle"
    ) {
      return;
    }
    const cursor = state.cursor;
    setList({ ...state, more: "loading" });
    void loadCaptures(cursor, new AbortController().signal).then(
      (page) => {
        setList((current) =>
          current.kind === "ready" && current.cursor === cursor
            ? {
                kind: "ready",
                ...appendPage(current, page),
                more: "idle",
                moreAttempt: 0,
              }
            : current,
        );
      },
      () => {
        setList((current) =>
          current.kind === "ready" && current.cursor === cursor
            ? {
                ...current,
                more: "failed",
                moreAttempt: current.moreAttempt + 1,
              }
            : current,
        );
      },
    );
  }

  function choose(index: number): void {
    const state = view.current;
    if (state.list.kind !== "ready") {
      return;
    }
    const item = state.list.items[index];
    if (item === undefined) {
      return;
    }
    setActiveId(item.id);
    go(detailPath(item.id));
  }

  function move(delta: number): void {
    const state = view.current;
    if (state.list.kind !== "ready" || state.list.items.length === 0) {
      return;
    }
    const current =
      state.activeId === null
        ? -1
        : state.list.items.findIndex((item) => item.id === state.activeId);
    if (current < 0 && delta < 0) {
      return;
    }
    const next =
      current < 0 ? 0 : Math.min(state.list.items.length - 1, current + delta);
    const item = state.list.items[next];
    if (item === undefined) {
      return;
    }
    setActiveId(item.id);
    setFocusRow(true);
    if (state.wide) {
      go(detailPath(item.id));
    }
  }

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.metaKey || event.ctrlKey || event.altKey) {
        return;
      }
      const target = event.target;
      if (
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement
      ) {
        return;
      }
      if (event.key === "t") {
        event.preventDefault();
        setPreference((current) => nextPreference[current]);
        return;
      }
      const state = view.current;
      if (event.key === "r" && state.list.kind === "ready") {
        event.preventDefault();
        refresh();
        return;
      }
      if (event.key === "Escape" && state.id !== null) {
        event.preventDefault();
        go(listPathname(window.location.pathname));
        return;
      }
      const inDetail =
        target instanceof Element && target.closest("[data-detail]") !== null;
      if (
        inDetail &&
        (event.key === "ArrowLeft" || event.key === "ArrowRight")
      ) {
        return;
      }
      if (event.key === "j" || event.key === "ArrowDown") {
        event.preventDefault();
        move(1);
        return;
      }
      if (event.key === "k" || event.key === "ArrowUp") {
        event.preventDefault();
        move(-1);
        return;
      }
      if (event.key === "Enter" && !inDetail) {
        if (
          target instanceof HTMLButtonElement ||
          target instanceof HTMLAnchorElement
        ) {
          return;
        }
        event.preventDefault();
        const index =
          state.list.kind === "ready" && state.activeId !== null
            ? state.list.items.findIndex((item) => item.id === state.activeId)
            : -1;
        choose(Math.max(0, index));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  const counts =
    list.kind === "ready" || list.kind === "empty"
      ? headline(list.kind === "ready" ? list.items : [], nowMs)
      : null;
  const more = list.kind === "ready" && list.cursor !== null;
  const activeIndex =
    list.kind === "ready" && activeId !== null
      ? list.items.findIndex((item) => item.id === activeId)
      : -1;
  const label = labels[preference];
  const Icon =
    preference === "dark" ? Moon : preference === "light" ? Sun : Monitor;
  const showList = wide || id === null;
  const showDetail = wide || id !== null;
  const control =
    "inline-flex h-7 w-7 items-center justify-center rounded-[6px] border border-control-border bg-bg text-text-2 hover:bg-control-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent";

  return (
    <div class="flex h-dvh min-h-0 flex-col overflow-hidden bg-bg text-[16px] leading-[24px] font-normal text-text">
      <header class="flex h-12 shrink-0 items-center justify-between gap-4 border-b border-border px-4">
        <div class="flex min-w-0 items-center gap-4">
          {showDetail && !wide ? (
            <button
              type="button"
              class="text-[14px] leading-[20px] font-semibold text-text focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
              title="Back to requests"
              onClick={() => {
                go(listPathname(pathname));
              }}
            >
              oxray
            </button>
          ) : (
            <p class="text-[14px] leading-[20px] font-semibold text-text">
              oxray
            </p>
          )}
          {counts === null ? null : (
            <p class="truncate text-[13px] leading-[20px] text-text-2">
              <span class="font-mono text-[13px] leading-[20px] text-text tabular-nums">
                {formatCount(counts.requests)}
                {more ? "+" : ""}
              </span>
              {counts.requests === 1 && !more ? " request" : " requests"}
              {counts.failed > 0 ? (
                <>
                  {" · "}
                  <span class="font-mono text-[13px] leading-[20px] text-err tabular-nums">
                    {formatCount(counts.failed)}
                  </span>
                  {" failed in the last 5 min"}
                </>
              ) : null}
            </p>
          )}
        </div>
        <div class="flex shrink-0 items-center gap-2">
          <p
            role="status"
            class="truncate text-[13px] leading-[20px] text-text-2"
          >
            {offline ? "Can't reach the app. Retrying…" : ""}
          </p>
          {list.kind === "ready" ? (
            <button
              type="button"
              class={control}
              aria-label="Refresh"
              title="Refresh (r)"
              onClick={refresh}
            >
              <RefreshCw size={16} strokeWidth={1.5} aria-hidden="true" />
            </button>
          ) : null}
          <button
            type="button"
            class={control}
            aria-label={`Theme: ${label}`}
            title={`Theme: ${label} (t)`}
            onClick={() => {
              setPreference(nextPreference[preference]);
            }}
          >
            <Icon size={16} strokeWidth={1.5} aria-hidden="true" />
          </button>
        </div>
      </header>
      <main class="flex min-h-0 flex-1">
        {showList ? (
          <section
            aria-label="Requests"
            class={`flex min-h-0 min-w-0 flex-col ${wide ? "w-[44%] min-w-[560px] border-r border-border" : "w-full"}`}
          >
            <h1 class="sr-only">Requests</h1>
            <RequestList
              phase={
                list.kind === "loading"
                  ? list.slow
                    ? "slow"
                    : "loading"
                  : list.kind
              }
              items={list.kind === "ready" ? list.items : []}
              hasMore={more}
              active={activeIndex >= 0 ? activeIndex : null}
              nowMs={nowMs}
              focusRow={focusRow}
              onChoose={(index) => {
                setFocusRow(false);
                choose(index);
              }}
              onNearEnd={loadMore}
            />
          </section>
        ) : null}
        {showDetail ? (
          <section
            aria-label="Request"
            data-detail
            class="flex min-h-0 min-w-0 flex-1 flex-col"
          >
            <RequestDetail
              phase={
                list.kind === "empty" && id === null ? "loading" : shown.phase
              }
              capture={shown.capture}
              nowMs={nowMs}
              single={!wide}
              listHref={listPathname(pathname)}
              onBack={() => {
                go(listPathname(pathname));
              }}
            />
          </section>
        ) : null}
      </main>
      <div
        role="status"
        data-floating
        class={
          toast === null
            ? "sr-only"
            : "pointer-events-none fixed bottom-6 left-1/2 -translate-x-1/2 rounded-[8px] bg-raised px-3 py-2 text-[13px] leading-[20px] text-text shadow-raised"
        }
      >
        {toast ?? ""}
      </div>
    </div>
  );
}
