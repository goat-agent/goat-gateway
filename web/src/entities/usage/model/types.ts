export type Usage = {
  input_tokens: number | null;
  output_tokens: number | null;
  cache_read_tokens: number | null;
  cache_write_tokens: number | null;
  reasoning_tokens: number | null;
};

export type Totals = {
  requests: number;
  errors: number;
  in_flight: number;
  usage: Usage;
  cost_micros: number | null;
  priced_requests: number;
  median_ms: number | null;
  slowest_tenth_ms: number | null;
};

export type Slice = { key: string; totals: Totals };

export type Bucket = { at: number; slices: Slice[] };

export type Grouping = "provider" | "account" | "model" | "person" | "client" | "status";

export type Report = {
  totals: Totals;
  cache_hit_ratio: number | null;
  by: Grouping;
  bucket_ms: number;
  breakdown: Slice[];
  series: Bucket[];
};
