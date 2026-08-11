import type { Totals } from "@/entities/usage/@x/provider";
import type { AccountState } from "@/entities/account/@x/provider";

export type LimitWindow = {
  label: string;
  scope: string | null;
  used_percent: number;
  resets_at_ms: number | null;
};

export type AccountLimits = {
  account: string;
  windows: LimitWindow[];
  observed_at: number;
};

export type ProviderHealth = {
  provider: string;
  label: string;
  accounts: number;
  usable: number;
  soonest_reset_ms: number | null;
  reports_limits: boolean;
  limits: AccountLimits[];
};

export type Attention = {
  account: string;
  provider: string;
  state: AccountState;
  until: number | null;
};

export type Overview = {
  now: number;
  window_ms: number;
  totals: Totals;
  cache_hit_ratio: number | null;
  error_ratio: number | null;
  requests_per_hour: number;
  providers: ProviderHealth[];
  attention: Attention[];
  pricing_as_of: string;
};

export type SignInProvider = { provider: string; label: string; modes: string[] };

export function worstWindow(windows: LimitWindow[]): LimitWindow | undefined {
  return windows.reduce<LimitWindow | undefined>(
    (held, window) => (held && held.used_percent >= window.used_percent ? held : window),
    undefined,
  );
}
