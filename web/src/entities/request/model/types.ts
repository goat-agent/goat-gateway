import type { Usage } from "@/entities/usage/@x/request";

export type Request = {
  id: string;
  started_at: number;
  person: string | null;
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
  user_id: string | null;
  key_id: string | null;
  credential: "account" | "carried" | null;
  attempts: Attempt[] | null;
};

export type Attempt =
  | { account: string | null; credential: string; status: number; said: string | null }
  | { carried: string; why: string };

export const STATUSES = ["ok", "error", "in_flight", "abandoned"] as const;
