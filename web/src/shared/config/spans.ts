export type Span = { label: string; ms: number; bucket: number };

export const SPANS = [
  { label: "Last hour", ms: 3_600_000, bucket: 300_000 },
  { label: "Last 24 hours", ms: 86_400_000, bucket: 3_600_000 },
  { label: "Last 7 days", ms: 604_800_000, bucket: 86_400_000 },
  { label: "Last 30 days", ms: 2_592_000_000, bucket: 86_400_000 },
] as const satisfies readonly Span[];

export const DAY = SPANS[1];
export const WEEK = SPANS[2];

export function spanOf(ms: string): Span {
  return SPANS.find((span) => String(span.ms) === ms) ?? DAY;
}
