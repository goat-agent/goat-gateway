import { cn } from "@/shared/lib/cn";

const FILLS = {
  good: "bg-good",
  warning: "bg-warning",
  serious: "bg-serious",
  critical: "bg-critical",
  info: "bg-info",
  neutral: "bg-line-strong",
  off: "border border-line-strong bg-transparent",
} as const;

export type DotTone = keyof typeof FILLS;

export function Dot({
  tone,
  pulse = false,
  label,
  className,
}: {
  tone: DotTone;
  pulse?: boolean;
  label?: string;
  className?: string;
}) {
  return (
    <span
      role={label ? "img" : undefined}
      aria-label={label}
      title={label}
      className={cn("relative inline-flex size-2 shrink-0", className)}
    >
      {pulse ? (
        <span className={cn("absolute inset-0 animate-ping rounded-full opacity-60", FILLS[tone])} />
      ) : null}
      <span className={cn("relative size-2 rounded-full", FILLS[tone])} />
    </span>
  );
}
