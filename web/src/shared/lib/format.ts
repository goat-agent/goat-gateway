const UNKNOWN = "—";

export function count(value: number | null | undefined) {
  if (value === null || value === undefined) return UNKNOWN;
  return value.toLocaleString("en-US");
}

export function tokens(value: number | null | undefined) {
  if (value === null || value === undefined) return UNKNOWN;
  if (value < 1000) return String(value);
  if (value < 1_000_000) return `${round(value / 1000)}k`;
  if (value < 1_000_000_000) return `${round(value / 1_000_000)}M`;
  return `${round(value / 1_000_000_000)}B`;
}

export function money(micros: number | null | undefined) {
  if (micros === null || micros === undefined) return UNKNOWN;
  const dollars = micros / 1_000_000;
  if (dollars === 0) return "$0";
  if (dollars < 0.01) return "<$0.01";
  if (dollars < 100) return `$${dollars.toFixed(2)}`;
  return `$${Math.round(dollars).toLocaleString("en-US")}`;
}

export function duration(ms: number | null | undefined) {
  if (ms === null || ms === undefined) return UNKNOWN;
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60_000) return `${round(ms / 1000)}s`;
  return `${Math.floor(ms / 60_000)}m ${Math.round((ms % 60_000) / 1000)}s`;
}

export function percent(ratio: number | null | undefined, digits = 1) {
  if (ratio === null || ratio === undefined) return UNKNOWN;
  return `${(ratio * 100).toFixed(digits)}%`;
}

export function whole(ratio: number | null | undefined) {
  if (ratio === null || ratio === undefined) return UNKNOWN;
  return `${Math.round(ratio)}%`;
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

export function ago(at: number | null | undefined, now = Date.now()) {
  if (!at) return UNKNOWN;
  const seconds = Math.max(0, Math.round((now - at) / 1000));
  if (seconds < 60) return `${seconds}s ago`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`;
  if (seconds < 86_400) return `${Math.floor(seconds / 3600)}h ago`;
  return `${Math.floor(seconds / 86_400)}d ago`;
}

export function until(at: number | null | undefined, now = Date.now()) {
  if (!at) return UNKNOWN;
  const seconds = Math.max(0, Math.round((at - now) / 1000));
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
  return `${Math.floor(seconds / 3600)}h ${Math.round((seconds % 3600) / 60)}m`;
}

function round(value: number) {
  return value < 10 ? value.toFixed(1) : String(Math.round(value));
}

export { UNKNOWN };
