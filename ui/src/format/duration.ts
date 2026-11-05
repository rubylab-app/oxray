export function formatDuration(micros: number): string {
  if (!Number.isFinite(micros) || micros < 0) {
    throw new Error(
      "duration must be a non-negative finite number of microseconds",
    );
  }
  const whole = Math.floor(micros);
  const millisecond = 1_000;
  const tenMilliseconds = 10_000;
  const second = 1_000_000;
  const minute = 60_000_000;
  if (whole < millisecond) {
    return `${whole} µs`;
  }
  if (whole < tenMilliseconds) {
    const tenths = Math.floor((whole + 50) / 100);
    const integer = Math.floor(tenths / 10);
    const fraction = tenths % 10;
    return `${integer}.${fraction} ms`;
  }
  if (whole < second) {
    return `${Math.floor((whole + 500) / 1_000)} ms`;
  }
  if (whole < minute) {
    const centiseconds = Math.floor((whole + 5_000) / 10_000);
    const integer = Math.floor(centiseconds / 100);
    const fraction = String(centiseconds % 100).padStart(2, "0");
    return `${integer}.${fraction} s`;
  }
  const totalSeconds = Math.floor((whole + 500_000) / 1_000_000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  if (seconds === 0) {
    return `${minutes} min`;
  }
  return `${minutes} min ${seconds} s`;
}
