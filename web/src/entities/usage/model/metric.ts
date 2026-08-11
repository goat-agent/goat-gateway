import { show } from "@/shared/lib";
import type { Totals } from "./types";

export type MetricName = "requests" | "tokens" | "cost" | "errors" | "cache_read";

export type Metric = {
  name: MetricName;
  label: string;
  of: (totals: Totals) => number;
  show: (value: number) => string;
};

export const METRICS: Metric[] = [
  { name: "requests", label: "Requests", of: (t) => t.requests, show: show.count },
  {
    name: "tokens",
    label: "Tokens",
    of: (t) => (t.usage.input_tokens ?? 0) + (t.usage.output_tokens ?? 0),
    show: show.tokens,
  },
  { name: "cost", label: "Cost", of: (t) => t.cost_micros ?? 0, show: show.money },
  { name: "errors", label: "Errors", of: (t) => t.errors, show: show.count },
  {
    name: "cache_read",
    label: "Cache reads",
    of: (t) => t.usage.cache_read_tokens ?? 0,
    show: show.tokens,
  },
];

export function metric(name: MetricName): Metric {
  return METRICS.find((entry) => entry.name === name) ?? METRICS[0]!;
}
