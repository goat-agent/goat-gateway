import { Badge } from "@/shared/ui/badge";

export function StatusBadge({
  status,
  kind,
  translated,
}: {
  status: string;
  kind?: string | null;
  translated?: boolean;
}) {
  const tone =
    status === "error"
      ? "critical"
      : status === "abandoned"
        ? "serious"
        : status === "in_flight"
          ? "warning"
          : "good";

  return (
    <span className="flex items-center gap-1.5">
      <Badge tone={tone} title={kind ?? undefined}>
        {status === "in_flight" ? "running" : status}
      </Badge>
      {translated ? <Badge title="the format was translated on the way through">↔</Badge> : null}
    </span>
  );
}
