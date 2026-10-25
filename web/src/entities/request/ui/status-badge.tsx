import { ArrowLeftRight } from "lucide-react";
import { Link } from "react-router-dom";
import { Badge, Dot, type DotTone, type Tone } from "@/shared/ui";
import { cn } from "@/shared/lib/cn";
import { dayOrClock, duration, moment } from "@/shared/lib/format";
import type { Request } from "../model/types";

const TONES: Record<string, Tone & DotTone> = {
  ok: "good",
  error: "critical",
  abandoned: "serious",
  in_flight: "info",
};

function said(status: string, kind?: string | null) {
  if (status === "in_flight") return "running";
  if (status === "error" && kind) return `error · ${kind}`;
  return status;
}

export function StatusGlyph({ status, kind }: { status: string; kind?: string | null }) {
  return (
    <Dot
      tone={TONES[status] ?? "neutral"}
      pulse={status === "in_flight"}
      label={said(status, kind)}
    />
  );
}

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
      <Badge dot tone={TONES[status] ?? "neutral"}>
        {said(status, kind)}
      </Badge>
      {translated ? (
        <Badge title="the format was translated on the way through" aria-label="translated">
          <ArrowLeftRight strokeWidth={1.75} />
          translated
        </Badge>
      ) : null}
    </span>
  );
}

export function formatOf(request: Pick<Request, "ingress" | "egress" | "translated">) {
  return request.translated ? `${request.ingress} → ${request.egress}` : request.ingress;
}

export function RequestLine({
  request,
  now,
  showing = "account",
  className,
}: {
  request: Request;
  now: number;
  showing?: "account" | "client";
  className?: string;
}) {
  const aside =
    showing === "client"
      ? request.client
      : request.credential === "carried"
        ? "own login"
        : request.account;

  return (
    <Link
      to={`/requests/${encodeURIComponent(request.id)}`}
      className={cn(
        "hover-fade flex h-[var(--row-compact)] min-w-0 items-center gap-3 px-5 text-small hover:bg-panel-hover",
        className,
      )}
    >
      <StatusGlyph status={request.status} kind={request.error_kind} />
      <span className="mono w-16 shrink-0 text-ink-secondary" title={moment(request.started_at, now)}>
        {dayOrClock(request.started_at, now)}
      </span>
      <span className="min-w-0 flex-1 truncate text-ink">{request.model}</span>
      <span className="max-w-[40%] shrink truncate text-ink-secondary">{aside ?? ""}</span>
      <span
        className={cn(
          "numeric w-12 shrink-0 text-right",
          request.status === "error" ? "text-critical-text" : "text-ink-secondary",
        )}
      >
        {request.status === "error" ? "failed" : duration(request.duration_ms)}
      </span>
    </Link>
  );
}
