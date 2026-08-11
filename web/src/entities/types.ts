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

export type UsageReport = {
  totals: Totals;
  cache_hit_ratio: number | null;
  by: Grouping;
  bucket_ms: number;
  breakdown: Slice[];
  series: Bucket[];
};

export type Grouping = "provider" | "account" | "model" | "person" | "client" | "status";

export type Window = { label: string; scope: string | null; used_percent: number; resets_at_ms: number | null };

export type ProviderHealth = {
  provider: string;
  label: string;
  accounts: number;
  usable: number;
  soonest_reset_ms: number | null;
  reports_limits: boolean;
  limits: { account: string; windows: Window[]; observed_at: number }[];
};

export type Overview = {
  now: number;
  window_ms: number;
  totals: Totals;
  cache_hit_ratio: number | null;
  error_ratio: number | null;
  requests_per_hour: number;
  providers: ProviderHealth[];
  attention: { account: string; provider: string; state: AccountState; until: number | null }[];
  pricing_as_of: string;
};

export type AccountState = "active" | "rate_limited" | "sign_in_expired" | "disabled";

export type Account = {
  name: string;
  provider: string;
  credential_kind: string;
  state: AccountState;
  cooldown_until: number | null;
  created_at: number;
};

export type Request = {
  id: string;
  started_at: number;
  person: string | null;
  client: string | null;
  conversation: string | null;
  provider: string;
  account: string | null;
  model: string;
  ingress: string;
  egress: string;
  translated: boolean;
  status: string;
  error_kind: string | null;
  error_message: string | null;
  ttft_ms: number | null;
  duration_ms: number | null;
  usage: Usage;
  cost_micros: number | null;
  input_digest: string | null;
  output_digest: string | null;
  byte_identical: boolean | null;
  evidence: unknown;
  upstream_request_id: string | null;
};

export type User = { id: string; name: string; created_at: number };

export type Key = {
  id: string;
  user_id: string;
  label: string;
  prefix: string;
  created_at: number;
  last_used_at: number | null;
  revoked_at: number | null;
};

export type SignInProvider = { provider: string; label: string; modes: string[] };
