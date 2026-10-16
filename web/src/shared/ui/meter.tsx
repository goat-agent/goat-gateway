import { Meter as Base } from "@base-ui/react/meter";
import { cn } from "@/shared/lib/cn";

const FILLS = {
  good: "bg-good",
  warning: "bg-warning",
  critical: "bg-critical",
} as const;

function pressureOf(used: number): keyof typeof FILLS {
  if (used >= 90) return "critical";
  if (used >= 70) return "warning";
  return "good";
}

export function Meter({
  used,
  label,
  showValue = true,
  className,
}: {
  used: number;
  label: string;
  showValue?: boolean;
  className?: string;
}) {
  const held = Math.min(100, Math.max(0, used));

  return (
    <Base.Root
      value={held}
      min={0}
      max={100}
      aria-label={label}
      className={cn("flex min-w-0 items-center gap-2", className)}
    >
      <Base.Track className="relative block h-1 min-w-10 grow overflow-hidden rounded-full bg-line">
        <Base.Indicator className={cn("block h-full rounded-full", FILLS[pressureOf(held)])} />
      </Base.Track>
      {showValue ? (
        <span className="numeric w-9 shrink-0 text-right text-mini text-ink">
          {Math.round(used)}%
        </span>
      ) : null}
    </Base.Root>
  );
}
