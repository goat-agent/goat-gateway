import type { Usage } from "@/entities/usage";

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
