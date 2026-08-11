export type AccountState = "active" | "rate_limited" | "sign_in_expired" | "disabled";

export type Account = {
  name: string;
  provider: string;
  credential_kind: string;
  state: AccountState;
  cooldown_until: number | null;
  created_at: number;
};

export const SAID: Record<
  AccountState,
  { text: string; tone: "good" | "warning" | "serious" | "neutral" }
> = {
  active: { text: "usable", tone: "good" },
  rate_limited: { text: "rate limited", tone: "warning" },
  sign_in_expired: { text: "signed out", tone: "serious" },
  disabled: { text: "turned off", tone: "neutral" },
};
