import { count, money, tokens } from "@/shared/lib/format";
import type { Totals } from "./types";

export const METRIC_NAMES = ["requests", "tokens", "cost", "errors", "cache_read"] as const;

export type MetricName = (typeof METRIC_NAMES)[number];

export type Metric = {
  label: string;
  of: (totals: Totals) => number;
  show: (value: number) => string;
};

export const METRICS = {
  requests: { label: "Requests", of: (totals) => totals.requests, show: count },
  tokens: {
    label: "Tokens",
    of: (totals) => (totals.usage.input_tokens ?? 0) + (totals.usage.output_tokens ?? 0),
    show: tokens,
  },
  cost: { label: "Cost", of: (totals) => totals.cost_micros ?? 0, show: money },
  errors: { label: "Errors", of: (totals) => totals.errors, show: count },
  cache_read: {
    label: "Cache reads",
    of: (totals) => totals.usage.cache_read_tokens ?? 0,
    show: tokens,
  },
} as const satisfies Record<MetricName, Metric>;
