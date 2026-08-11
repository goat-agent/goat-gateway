export const ACCOUNT_STATES = ["active", "rate_limited", "sign_in_expired", "disabled"] as const;

export type AccountState = (typeof ACCOUNT_STATES)[number];

export type Account = {
  name: string;
  provider: string;
  credential_kind: string;
  state: AccountState;
  cooldown_until: number | null;
  created_at: number;
};
