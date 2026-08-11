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
