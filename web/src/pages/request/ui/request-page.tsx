import type { ReactNode } from "react";
import { Link, useParams } from "react-router-dom";
import { ArrowLeft, CircleAlert } from "lucide-react";
import {
  ButtonLink,
  Fact,
  Facts,
  Nothing,
  Page,
  Panel,
  PanelHead,
  Pill,
  Properties,
  Property,
} from "@/shared/ui";
import { useResource } from "@/shared/api";
import { StatusBadge, type Attempt, type Request } from "@/entities/request";
import { duration, moment, money, tokens, UNKNOWN } from "@/shared/lib/format";

const TRAIL = (
  <Link to="/requests" className="hover-fade hover:text-ink">
    Requests
  </Link>
);

export function RequestPage() {
  const { id = "" } = useParams();
  const found = useResource<{ request: Request }>(`/api/requests/${encodeURIComponent(id)}`);
  const request = found.data?.request;

  if (found.error) {
    return (
      <Page title="Request" trail={TRAIL}>
        <Panel>
          <Nothing
            says={found.error}
            offers={
              <ButtonLink to="/requests" size="medium">
                <ArrowLeft strokeWidth={1.75} />
                Back to requests
              </ButtonLink>
            }
          />
        </Panel>
      </Page>
    );
  }

  if (!request) return <Page title="Request" trail={TRAIL}>{null}</Page>;

  return (
    <Page
      title={request.model}
      trail={TRAIL}
      note={<span className="mono">{request.id}</span>}
      bleed
    >
      <div className="flex min-h-full flex-col lg:flex-row">
        <div className="flex min-w-0 flex-1 flex-col gap-5 px-6 py-6 lg:px-8">
          <header className="flex flex-col gap-2">
            <h2 className="display m-0 truncate text-headline font-semibold text-ink-title">
              {request.model}
            </h2>
            <div className="flex flex-wrap items-center gap-2 text-small text-ink-secondary">
              <StatusBadge
                status={request.status}
                kind={request.error_kind}
                translated={request.translated}
              />
              <span>
                {request.provider} · started {moment(request.started_at)}
              </span>
            </div>
          </header>

          {request.error_message ? (
            <Panel className="border-critical/40 bg-critical-tint">
              <PanelHead
                title={
                  <span className="flex items-center gap-2 text-critical-text">
                    <CircleAlert className="size-4" strokeWidth={1.75} />
                    What the provider said
                  </span>
                }
                note={request.error_kind ?? undefined}
              />
              <pre className="mono m-0 overflow-x-auto px-4 pb-4 whitespace-pre-wrap text-critical-text select-text">
                {request.error_message}
              </pre>
            </Panel>
          ) : null}

          {request.attempts && request.attempts.length > 0 ? (
            <Panel>
              <PanelHead
                title="Tried first"
                note="refused before a byte reached the client, so it went on to the next"
              />
              <ul className="m-0 flex list-none flex-col p-0 pb-2">
                {request.attempts.map((attempt, index) => (
                  <TriedFirst key={index} attempt={attempt} />
                ))}
              </ul>
            </Panel>
          ) : null}

          <Panel>
            <PanelHead title="What it used" />
            <Facts>
              <Fact name="Input" value={tokens(request.usage.input_tokens)} />
              <Fact name="Output" value={tokens(request.usage.output_tokens)} />
              <Fact name="Read from cache" value={tokens(request.usage.cache_read_tokens)} />
              <Fact name="Written to cache" value={tokens(request.usage.cache_write_tokens)} />
              <Fact name="Thinking" value={tokens(request.usage.reasoning_tokens)} />
              <Fact name="Cost" value={money(request.cost_micros)} />
            </Facts>
          </Panel>

          <Panel>
            <PanelHead
              title="What we changed"
              note={request.byte_identical ? "nothing — the bytes went through untouched" : undefined}
            />
            <Facts>
              <Fact name="Body in" value={<Mono>{request.input_digest ?? UNKNOWN}</Mono>} />
              <Fact name="Body out" value={<Mono>{request.output_digest ?? UNKNOWN}</Mono>} />
              <Fact
                name="Provider request id"
                value={<Mono>{request.upstream_request_id ?? UNKNOWN}</Mono>}
              />
            </Facts>
            {request.evidence ? (
              <pre className="mono m-0 max-h-[28rem] overflow-auto hairline-t border-line-subtle bg-base p-4 text-ink-secondary select-text">
                {JSON.stringify(request.evidence, null, 2)}
              </pre>
            ) : null}
          </Panel>
        </div>

        <aside className="shrink-0 hairline-t border-line-subtle py-3 lg:w-[22rem] lg:border-t-0 lg:hairline-l">
          <h3 className="m-0 flex h-8 items-center px-4 text-mini font-medium text-ink-secondary">
            What happened
          </h3>
          <Properties>
            <Property name="Started" value={moment(request.started_at)} />
            <Property name="Time to first byte" value={duration(request.ttft_ms)} />
            <Property name="Took" value={duration(request.duration_ms)} />
            <Property name="Provider" value={request.provider} />
            <Property
              name="Served on"
              value={
                request.credential === "carried"
                  ? "the client's own login"
                  : (request.account ?? UNKNOWN)
              }
            />
            <Property name="Person" value={request.person ?? UNKNOWN} />
            <Property name="Client" value={request.client ?? UNKNOWN} />
            <Property
              name="Format in / out"
              value={
                <Pill className="h-5 px-2 text-micro">
                  {request.ingress} → {request.egress}
                </Pill>
              }
            />
            <Property name="Conversation" value={<Mono>{request.conversation ?? UNKNOWN}</Mono>} />
          </Properties>
        </aside>
      </div>
    </Page>
  );
}

function TriedFirst({ attempt }: { attempt: Attempt }) {
  if ("carried" in attempt) {
    return (
      <li className="flex min-w-0 flex-col gap-0.5 px-4 py-2 text-small">
        <span className="font-medium text-ink">The client's own login was set aside</span>
        <span className="text-ink-secondary">{attempt.why}</span>
      </li>
    );
  }
  return (
    <li className="flex min-w-0 flex-col gap-0.5 px-4 py-2 text-small">
      <span className="flex items-center gap-2">
        <span className="font-medium text-ink">{attempt.account}</span>
        <span className="numeric text-ink-secondary">refused with {attempt.status}</span>
      </span>
      {attempt.said ? (
        <span className="mono truncate text-mini text-ink-secondary select-text">{attempt.said}</span>
      ) : null}
    </li>
  );
}

function Mono({ children }: { children: ReactNode }) {
  return <span className="mono select-text">{children}</span>;
}
