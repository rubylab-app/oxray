import { Monitor, Moon, Sun } from "lucide-preact";
import { useEffect, useState } from "preact/hooks";

type Preference = "system" | "dark" | "light";

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
  const label = labels[preference];
  const Icon =
    preference === "dark" ? Moon : preference === "light" ? Sun : Monitor;

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

  return (
    <div class="min-h-screen bg-bg text-text">
      <header class="flex h-12 items-center justify-between border-b border-border px-4">
        <p class="text-[14px] leading-5 font-semibold">oxray</p>
        <button
          type="button"
          class="inline-flex h-7 items-center gap-2 rounded-[6px] border border-control-border bg-bg px-2 text-[13px] leading-5 font-medium text-text transition-colors duration-[120ms] ease-out hover:bg-control-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent motion-reduce:transition-none"
          aria-label={`Theme: ${label}`}
          onClick={() => {
            setPreference(nextPreference[preference]);
          }}
        >
          <Icon size={16} strokeWidth={1.5} aria-hidden="true" />
          {label}
        </button>
      </header>
    </div>
  );
}
