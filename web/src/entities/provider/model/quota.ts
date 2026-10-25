import type { LimitWindow, ProviderHealth } from "./types";

export type Quota =
  | { seen: "windows"; windows: LimitWindow[]; observed_at: number }
  | { seen: "unread"; said: string; observed_at: number }
  | { seen: "not_yet" }
  | { seen: "never" };

export function quotaOf(
  providers: readonly ProviderHealth[] | undefined,
  provider: string,
  account: string,
): Quota | undefined {
  if (!providers) return undefined;
  const health = providers.find((entry) => entry.provider === provider);
  if (!health || !health.reports_limits) return { seen: "never" };
  const limits = health.limits.find((entry) => entry.account === account);
  if (!limits) return { seen: "not_yet" };
  if (limits.windows.length > 0) {
    return { seen: "windows", windows: limits.windows, observed_at: limits.observed_at };
  }
  if (limits.said) return { seen: "unread", said: limits.said, observed_at: limits.observed_at };
  return { seen: "not_yet" };
}
