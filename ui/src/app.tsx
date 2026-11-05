import { Monitor, Moon, RefreshCw, Sun } from "lucide-preact";
import { useEffect, useRef, useState } from "preact/hooks";
import { loadCapture, loadCaptures, mountPrefix } from "./api";
import { formatCount } from "./format/count";
import type { Capture, CaptureSummary } from "./model/capture";
import { headline } from "./model/capture";
import { captureIdFromPath, listPathname } from "./path";
import { RequestDetail } from "./request-detail";
import { RequestList } from "./request-list";

type Preference = "system" | "dark" | "light";

type ListPhase =
  | { kind: "loading"; slow: boolean }
  | { kind: "empty" }
  | { kind: "error" }
  | { kind: "ready"; items: CaptureSummary[] };

type DetailPhase =
  | { kind: "idle" }
  | { kind: "loading"; slow: boolean }
  | { kind: "missing" }
  | { kind: "error" }
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
  const [detail, setDetail] = useState<DetailPhase>({ kind: "idle" });
  const [active, setActive] = useState<number | null>(null);
  const [focusRow, setFocusRow] = useState(false);
  const [nowMs, setNowMs] = useState(() => Date.now());
  const id = captureIdFromPath(pathname);
  const view = useRef({ list, active, wide, id });
  view.current = { list, active, wide, id };

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
        if (page.items.length === 0) {
          setList({ kind: "empty" });
          return;
        }
        setList({ kind: "ready", items: page.items });
      },
      () => {
        if (!activeLoad || controller.signal.aborted) {
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
    if (list.kind !== "empty") {
      return;
    }
    const timer = window.setInterval(() => {
      setReloadToken((value) => value + 1);
    }, 2000);
    return () => {
      window.clearInterval(timer);
    };
  }, [list.kind]);

  useEffect(() => {
    if (list.kind !== "error") {
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
  }, [list.kind, attempt]);

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
        current.kind === "loading" ? { kind: "loading", slow: true } : current,
      );
    }, 300);
    setDetail({ kind: "loading", slow: false });
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
        setDetail({ kind: "error" });
      },
    );
    return () => {
      activeLoad = false;
      controller.abort();
      window.clearTimeout(slowTimer);
    };
  }, [id, detailToken]);

  useEffect(() => {
    if (list.kind !== "ready" || id === null) {
      return;
    }
    const index = list.items.findIndex((item) => item.id === id);
    if (index >= 0) {
      setActive(index);
    }
  }, [id, list]);

  function go(path: string): void {
    if (window.location.pathname !== path) {
      window.history.pushState(null, "", path);
    }
    setPathname(window.location.pathname);
  }

  function detailPath(captureId: string): string {
    return `${mountPrefix()}/r/${encodeURIComponent(captureId)}`;
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
    setActive(index);
    go(detailPath(item.id));
  }

  function move(delta: number): void {
    const state = view.current;
    if (state.list.kind !== "ready" || state.list.items.length === 0) {
      return;
    }
    const current = state.active ?? -1;
    if (current < 0 && delta < 0) {
      return;
    }
    const next =
      current < 0 ? 0 : Math.min(state.list.items.length - 1, current + delta);
    const item = state.list.items[next];
    if (item === undefined) {
      return;
    }
    setActive(next);
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
        setReloadToken((value) => value + 1);
        setDetailToken((value) => value + 1);
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
        choose(state.active ?? 0);
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
  const label = labels[preference];
  const Icon =
    preference === "dark" ? Moon : preference === "light" ? Sun : Monitor;
  const showList = wide || id === null;
  const showDetail = wide || id !== null;

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
              </span>
              {" requests"}
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
          {list.kind === "ready" ? (
            <button
              type="button"
              class="inline-flex h-7 w-7 items-center justify-center rounded-[6px] border border-control-border bg-bg text-text-2 hover:bg-control-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
              aria-label="Refresh"
              onClick={() => {
                setReloadToken((value) => value + 1);
                setDetailToken((value) => value + 1);
              }}
            >
              <RefreshCw size={16} strokeWidth={1.5} aria-hidden="true" />
            </button>
          ) : null}
          <button
            type="button"
            class="inline-flex h-7 items-center gap-2 rounded-[6px] border border-control-border bg-bg px-2 text-[13px] leading-[20px] font-medium text-text hover:bg-control-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
            aria-label={`Theme: ${label}`}
            onClick={() => {
              setPreference(nextPreference[preference]);
            }}
          >
            <Icon size={16} strokeWidth={1.5} aria-hidden="true" />
            {label}
          </button>
        </div>
      </header>
      <div class="flex min-h-0 flex-1">
        {showList ? (
          <section
            aria-label="Requests"
            class={`flex min-h-0 min-w-0 flex-col ${wide ? "w-[44%] min-w-[560px] border-r border-border" : "w-full"}`}
          >
            <RequestList
              phase={
                list.kind === "loading"
                  ? list.slow
                    ? "slow"
                    : "loading"
                  : list.kind
              }
              items={list.kind === "ready" ? list.items : []}
              active={active}
              nowMs={nowMs}
              focusRow={focusRow}
              onChoose={(index) => {
                setFocusRow(false);
                choose(index);
              }}
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
                detail.kind === "loading"
                  ? detail.slow
                    ? "slow"
                    : "loading"
                  : detail.kind
              }
              capture={detail.kind === "ready" ? detail.capture : null}
              nowMs={nowMs}
              listHref={listPathname(pathname)}
              onBack={() => {
                go(listPathname(pathname));
              }}
            />
          </section>
        ) : null}
      </div>
    </div>
  );
}
