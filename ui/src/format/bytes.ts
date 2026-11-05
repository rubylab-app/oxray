const UNITS = ["KB", "MB", "GB", "TB"] as const;

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) {
    throw new Error("bytes must be a non-negative finite number");
  }
  const whole = Math.floor(bytes);
  if (whole < 1024) {
    return `${whole} B`;
  }
  let value = whole / 1024;
  let unit = 0;
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const label = UNITS[unit];
  if (label === undefined) {
    throw new Error("bytes unit");
  }
  return `${value.toFixed(1)} ${label}`;
}
