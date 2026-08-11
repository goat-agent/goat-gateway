import { Badge } from "@/shared/ui";

export function StatusBadge({
  status,
  kind,
  translated,
}: {
  status: string;
  kind?: string | null;
  translated?: boolean;
}) {
  return (
    <span className="flex items-center gap-1.5">
      <Badge tone={tone(status)} title={kind ?? undefined}>
        {status === "in_flight" ? "running" : status}
      </Badge>
      {translated ? <Badge title="the format was translated on the way through">↔</Badge> : null}
    </span>
  );
}

function tone(status: string) {
  if (status === "error") return "critical" as const;
  if (status === "abandoned") return "serious" as const;
  if (status === "in_flight") return "warning" as const;
  return "good" as const;
}
