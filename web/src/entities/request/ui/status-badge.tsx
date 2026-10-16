import { ArrowLeftRight } from "lucide-react";
import { Badge, type Tone } from "@/shared/ui";

const TONES: Record<string, Tone> = {
  ok: "good",
  error: "critical",
  abandoned: "serious",
  in_flight: "info",
};

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
      <Badge dot tone={TONES[status] ?? "neutral"} title={kind ?? undefined}>
        {status === "in_flight" ? "running" : status}
      </Badge>
      {translated ? (
        <Badge title="the format was translated on the way through" aria-label="translated">
          <ArrowLeftRight strokeWidth={1.75} />
        </Badge>
      ) : null}
    </span>
  );
}
