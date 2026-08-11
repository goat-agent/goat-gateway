import type { AccountState } from "@/entities/account";
import type { Totals } from "@/entities/usage";

export type Window = {
  label: string;
  scope: string | null;
  used_percent: number;
  resets_at_ms: number | null;
};

export type Observed = { account: string; windows: Window[]; observed_at: number };

export type ProviderHealth = {
  provider: string;
  label: string;
  accounts: number;
  usable: number;
  soonest_reset_ms: number | null;
  reports_limits: boolean;
  limits: Observed[];
};

export type Health = {
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

export type SignInProvider = { provider: string; label: string; modes: string[] };
