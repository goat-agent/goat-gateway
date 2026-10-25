import { useCallback, type ReactNode } from "react";
import { CircleAlert } from "lucide-react";
import { Detail, Fact, Facts, Inspector, Nothing, Properties, Property } from "@/shared/ui";
import { useHappenings, useResource } from "@/shared/api";
import { StatusBadge, formatOf, type Attempt, type Request } from "@/entities/request";
import { duration, moment, money, tokens, UNKNOWN } from "@/shared/lib/format";

export function RequestInspector({ id, onClose }: { id: string; onClose: () => void }) {
  const found = useResource<{ request: Request }>(`/api/requests/${encodeURIComponent(id)}`);
  const request = found.data?.request;

  const reload = found.reload;
  useHappenings(
    useCallback(
      (happening) => {
        if (happening === "request_settled") reload();
      },
      [reload],
    ),
  );

  if (found.error) {
    return (
      <Inspector kicker="Request" title="Not found" onClose={onClose}>
        <Nothing says={found.error} />
      </Inspector>
    );
  }

  if (!request || request.id !== id) {
    return <Inspector kicker="Request" title={<span className="text-ink-muted">Loading…</span>} onClose={onClose} />;
  }

  return (
    <Inspector
      kicker={<Mono>{request.id}</Mono>}
      title={request.model}
      subtitle={
        <>
          <StatusBadge status={request.status} kind={request.error_kind} translated={request.translated} />
          <span>{moment(request.started_at)}</span>
        </>
      }
      onClose={onClose}
    >
      {request.error_message ? (
        <Detail
          title={
            <span className="flex items-center gap-1.5 text-critical-text">
              <CircleAlert className="size-3.5" strokeWidth={1.75} />
              What the provider said
            </span>
          }
        >
          <pre className="mono m-0 max-h-60 overflow-auto rounded-lg bg-critical-tint px-3 py-2.5 whitespace-pre-wrap text-critical-text select-text">
            {request.error_message}
          </pre>
        </Detail>
      ) : null}

      {request.attempts && request.attempts.length > 0 ? (
        <Detail title="Tried first" aside="could not serve it, so it moved on before a byte reached the client">
          <ul className="m-0 flex list-none flex-col gap-2.5 p-0">
            {request.attempts.map((attempt, index) => (
              <TriedFirst key={index} attempt={attempt} />
            ))}
          </ul>
        </Detail>
      ) : null}

      <Detail>
        <Properties>
          <Property name="Provider" value={request.provider} />
          <Property
            name="Served on"
            value={request.credential === "carried" ? "the client's own login" : (request.account ?? UNKNOWN)}
          />
          <Property name="Person" value={request.person ?? UNKNOWN} />
          <Property name="Client" value={request.client ?? UNKNOWN} />
          <Property name="Format" value={formatOf(request)} />
          <Property name="First byte" value={duration(request.ttft_ms)} />
          <Property name="Took" value={duration(request.duration_ms)} />
          <Property name="Conversation" value={<Mono truncate>{request.conversation ?? UNKNOWN}</Mono>} />
        </Properties>
      </Detail>

      <Detail title="What it used">
        <Facts>
          <Fact name="Input" value={tokens(request.usage.input_tokens)} />
          <Fact name="Output" value={tokens(request.usage.output_tokens)} />
          <Fact name="Thinking" value={tokens(request.usage.reasoning_tokens)} />
          <Fact name="Cache read" value={tokens(request.usage.cache_read_tokens)} />
          <Fact name="Cache write" value={tokens(request.usage.cache_write_tokens)} />
          <Fact name="Cost" value={money(request.cost_micros)} />
        </Facts>
      </Detail>

      <Detail
        title="What we changed"
        aside={request.byte_identical ? "nothing — the bytes went through untouched" : undefined}
      >
        <Properties>
          <Property name="Body in" value={<Mono truncate>{request.input_digest ?? UNKNOWN}</Mono>} />
          <Property name="Body out" value={<Mono truncate>{request.output_digest ?? UNKNOWN}</Mono>} />
          <Property
            name="Provider's id"
            value={<Mono truncate>{request.upstream_request_id ?? UNKNOWN}</Mono>}
          />
        </Properties>
        {request.evidence ? (
          <pre className="mono m-0 max-h-[28rem] overflow-auto rounded-lg hairline border-line-subtle bg-base p-3 text-ink-secondary select-text">
            {JSON.stringify(request.evidence, null, 2)}
          </pre>
        ) : null}
      </Detail>
    </Inspector>
  );
}

function TriedFirst({ attempt }: { attempt: Attempt }) {
  if ("carried" in attempt) {
    return (
      <li className="flex min-w-0 flex-col gap-0.5 text-small">
        <span className="font-medium text-ink">The client's own login was set aside</span>
        <span className="text-ink-secondary">{attempt.why}</span>
      </li>
    );
  }
  return (
    <li className="flex min-w-0 flex-col gap-0.5 text-small">
      <span className="flex items-center gap-2">
        <span className="font-medium text-ink">{attempt.account ?? "The client's own login"}</span>
        <span className="numeric text-critical-text">answered {attempt.status}</span>
      </span>
      {attempt.said ? (
        <span className="mono truncate text-ink-secondary select-text">{attempt.said}</span>
      ) : null}
    </li>
  );
}

function Mono({ children, truncate = false }: { children: ReactNode; truncate?: boolean }) {
  return <span className={`mono select-text ${truncate ? "min-w-0 truncate" : ""}`}>{children}</span>;
}
