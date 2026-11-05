export function formatRelative(iso: string, nowMs: number): string {
  const then = Date.parse(iso);
  if (!Number.isFinite(then) || !Number.isFinite(nowMs)) {
    throw new Error("time must be a valid timestamp");
  }
  const seconds = Math.max(0, Math.floor((nowMs - then) / 1000));
  if (seconds < 60) {
    return `${seconds}s ago`;
  }
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) {
    return `${minutes}m ago`;
  }
  const hours = Math.floor(minutes / 60);
  if (hours < 24) {
    return `${hours}h ago`;
  }
  return formatDay(then);
}

export function formatAbsolute(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) {
    throw new Error("time must be a valid timestamp");
  }
  const parts = new Intl.DateTimeFormat("en-US", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hourCycle: "h23",
    timeZoneName: "short",
  }).formatToParts(date);
  const year = part(parts, "year");
  const month = part(parts, "month");
  const day = part(parts, "day");
  const hour = part(parts, "hour");
  const minute = part(parts, "minute");
  const second = part(parts, "second");
  const zone = part(parts, "timeZoneName");
  const millis = String(date.getMilliseconds()).padStart(3, "0");
  return `${year}-${month}-${day} ${hour}:${minute}:${second}.${millis} ${zone}`;
}

function formatDay(ms: number): string {
  const parts = new Intl.DateTimeFormat("en-US", {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
  }).formatToParts(new Date(ms));
  return `${part(parts, "month")} ${part(parts, "day")}, ${part(parts, "hour")}:${part(parts, "minute")}`;
}

function part(parts: Intl.DateTimeFormatPart[], type: string): string {
  const found = parts.find((item) => item.type === type);
  if (found === undefined) {
    throw new Error(`missing time part ${type}`);
  }
  return found.value;
}
