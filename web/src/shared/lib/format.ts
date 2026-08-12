export const UNKNOWN = "—";

export function count(value: number | null | undefined) {
  return value === null || value === undefined ? UNKNOWN : value.toLocaleString("en-US");
}

export function tokens(value: number | null | undefined) {
  if (value === null || value === undefined) return UNKNOWN;
  if (value < 1000) return String(value);
  if (value < 1_000_000) return `${brief(value / 1000)}k`;
  if (value < 1_000_000_000) return `${brief(value / 1_000_000)}M`;
  return `${brief(value / 1_000_000_000)}B`;
}

export function money(micros: number | null | undefined) {
  if (micros === null || micros === undefined) return UNKNOWN;
  const dollars = micros / 1_000_000;
  if (dollars === 0) return "$0.00";
  if (dollars < 0.01) return "<$0.01";
  return `$${dollars.toLocaleString("en-US", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  })}`;
}

export function duration(ms: number | null | undefined) {
  if (ms === null || ms === undefined) return UNKNOWN;
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60_000) return `${brief(ms / 1000)}s`;
  return `${Math.floor(ms / 60_000)}m ${Math.round((ms % 60_000) / 1000)}s`;
}

export function percent(ratio: number | null | undefined, digits = 1) {
  return ratio === null || ratio === undefined ? UNKNOWN : `${(ratio * 100).toFixed(digits)}%`;
}

export function share(used: number | null | undefined) {
  return used === null || used === undefined ? UNKNOWN : `${Math.round(used)}%`;
}

export function clock(at: number | null | undefined) {
  if (!at) return UNKNOWN;
  return new Date(at).toLocaleTimeString("en-US", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  });
}

export function day(at: number | null | undefined) {
  if (!at) return UNKNOWN;
  return new Date(at).toLocaleDateString("en-US", { month: "short", day: "numeric" });
}

export function moment(at: number | null | undefined, now = Date.now()) {
  if (!at) return UNKNOWN;
  return sameDay(at, now) ? clock(at) : `${day(at)} ${clock(at)}`;
}

export function ago(at: number | null | undefined, now = Date.now()) {
  if (!at) return UNKNOWN;
  return `${spanOf(Math.max(0, now - at))} ago`;
}

export function within(at: number | null | undefined, now = Date.now()) {
  if (!at) return UNKNOWN;
  return at <= now ? "due" : spanOf(at - now);
}

function spanOf(ms: number) {
  const seconds = Math.round(ms / 1000);
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
  if (seconds < 86_400) return `${Math.floor(seconds / 3600)}h`;
  return `${Math.floor(seconds / 86_400)}d`;
}

function sameDay(left: number, right: number) {
  const one = new Date(left);
  const other = new Date(right);
  return one.toDateString() === other.toDateString();
}

function brief(value: number) {
  return value < 10 ? value.toFixed(1) : String(Math.round(value));
}
