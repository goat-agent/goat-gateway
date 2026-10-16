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
  said: string | null;
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

export const WIRES = ["messages", "responses", "chat"] as const;
export type Wire = (typeof WIRES)[number];

export type DeclaredModel = {
  name: string;
  priced: boolean;
  thinking: string | null;
  max_tokens: number | null;
  cache_min_tokens: number | null;
};

export type ProviderEntry = {
  id: string;
  label: string;
  origin: "built_in" | "config" | "overridden";
  key: "bearer" | "x_api_key";
  endpoints: { wire: Wire; url: string }[];
  limits: "none" | "headers" | "endpoint";
  models: DeclaredModel[];
  sign_in: { label: string; modes: string[] } | null;
  carries_own_login: boolean;
  api_key: boolean;
  accounts: number;
};

export type Providers = { providers: ProviderEntry[]; writable: boolean };

export type Finding = {
  wire: Wire;
  url: string;
  status: number | null;
  serves: boolean;
  said: string | null;
};

export type Discovered = {
  suggested_id: string;
  root: string;
  key: "bearer" | "x_api_key" | null;
  models: string[];
  findings: Finding[];
  reports_limits: boolean;
  declaration: string;
};

export const WIRE_LABELS: Record<Wire, string> = {
  messages: "Messages",
  responses: "Responses",
  chat: "Chat Completions",
};

export function worstWindow(windows: LimitWindow[]): LimitWindow | undefined {
  return windows.reduce<LimitWindow | undefined>(
    (held, window) => (held && held.used_percent >= window.used_percent ? held : window),
    undefined,
  );
}
