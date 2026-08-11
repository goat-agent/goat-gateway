import type { Totals } from "./types";
import { money, tokens, count } from "@/shared/lib/format";

export type MetricName = "requests" | "tokens" | "cost" | "errors" | "cache_read";

export type Metric = {
  name: MetricName;
  label: string;
  of: (totals: Totals) => number;
  show: (value: number) => string;
};

export const METRICS: Metric[] = [
  { name: "requests", label: "Requests", of: (t) => t.requests, show: count },
  {
    name: "tokens",
    label: "Tokens",
    of: (t) => (t.usage.input_tokens ?? 0) + (t.usage.output_tokens ?? 0),
    show: tokens,
  },
  { name: "cost", label: "Cost", of: (t) => t.cost_micros ?? 0, show: money },
  { name: "errors", label: "Errors", of: (t) => t.errors, show: count },
  { name: "cache_read", label: "Cache reads", of: (t) => t.usage.cache_read_tokens ?? 0, show: tokens },
];

export function metric(name: MetricName): Metric {
  return METRICS.find((entry) => entry.name === name) ?? METRICS[0]!;
}
