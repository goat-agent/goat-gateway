import { Link, useParams } from "react-router-dom";
import { Button, Fact, Facts, Nothing, Page, Panel, PanelHead } from "@/shared/ui";
import { useResource } from "@/shared/api";
import { StatusBadge, type Request } from "@/entities/request";
import { duration, moment, money, tokens, UNKNOWN } from "@/shared/lib/format";

export function RequestPage() {
  const { id = "" } = useParams();
  const found = useResource<{ request: Request }>(`/api/requests/${encodeURIComponent(id)}`);
  const request = found.data?.request;

  if (found.error) {
    return (
      <Page title="Request">
        <Panel>
          <Nothing
            says={found.error}
            offers={
              <Link to="/requests">
                <Button>Back to requests</Button>
              </Link>
            }
          />
        </Panel>
      </Page>
    );
  }

  if (!request) return <Page title="Request">{null}</Page>;

  return (
    <Page
      title={request.model}
      note={<span className="numeric">{request.id}</span>}
      aside={
        <Link to="/requests">
          <Button tone="quiet" size="small">
            Back
          </Button>
        </Link>
      }
    >
      <Panel>
        <PanelHead title="What happened">
          <StatusBadge
            status={request.status}
            kind={request.error_kind}
            translated={request.translated}
          />
        </PanelHead>
        <Facts>
          <Fact name="Started" value={moment(request.started_at)} />
          <Fact name="Time to first byte" value={duration(request.ttft_ms)} />
          <Fact name="Took" value={duration(request.duration_ms)} />
          <Fact name="Provider" value={request.provider} />
          <Fact name="Account" value={request.account ?? UNKNOWN} />
          <Fact name="Person" value={request.person ?? UNKNOWN} />
          <Fact name="Client" value={request.client ?? UNKNOWN} />
          <Fact name="Format in / out" value={`${request.ingress} → ${request.egress}`} />
          <Fact name="Conversation" value={request.conversation ?? UNKNOWN} />
        </Facts>
      </Panel>

      {request.error_message ? (
        <Panel>
          <PanelHead title="What the provider said" note={request.error_kind ?? undefined} />
          <pre className="m-0 overflow-x-auto p-3 font-mono text-code whitespace-pre-wrap text-[var(--critical)]">
            {request.error_message}
          </pre>
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
          <Fact name="Body in" value={request.input_digest ?? UNKNOWN} />
          <Fact name="Body out" value={request.output_digest ?? UNKNOWN} />
          <Fact name="Provider request id" value={request.upstream_request_id ?? UNKNOWN} />
        </Facts>
        {request.evidence ? (
          <pre className="m-0 overflow-x-auto border-t border-line-subtle p-3 font-mono text-code text-ink-secondary">
            {JSON.stringify(request.evidence, null, 2)}
          </pre>
        ) : null}
      </Panel>
    </Page>
  );
}
