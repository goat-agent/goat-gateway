export type Role = "admin" | "member";

export type User = {
  id: string;
  name: string;
  role: Role;
  can_sign_in: boolean;
  created_at: number;
};

export type Person = { id: string; name: string; role: Role };

export type Me = { person: Person | null; admin: boolean };

export type Key = {
  id: string;
  user_id: string;
  user_name: string;
  label: string;
  prefix: string;
  created_at: number;
  last_used_at: number | null;
  revoked_at: number | null;
};

