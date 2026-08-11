import { Link, useParams } from "react-router-dom";
import { Page } from "../page";
import { Panel, PanelHead } from "@/shared/ui/panel";
import { Nothing } from "@/shared/ui/nothing";
import { Button } from "@/shared/ui/button";
import { StatusBadge } from "@/widgets/request-table/status-badge";
import { useResource } from "@/shared/api/use-resource";
import type { Request } from "@/entities/types";
import { clock, duration, money, tokens, UNKNOWN } from "@/shared/lib/format";

export function RequestPage() {
  const { id = "" } = useParams();
  const found = useResource<{ request: Request }>(`/api/requests/${encodeURIComponent(id)}`);
  const row = found.data?.request;

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

  if (!row) return <Page title="Request">{null}</Page>;

  return (
    <Page
      title={row.model}
      note={<span className="numeric">{row.id}</span>}
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
          <StatusBadge status={row.status} kind={row.error_kind} translated={row.translated} />
        </PanelHead>
        <dl className="m-0 grid grid-cols-2 gap-x-6 gap-y-0 p-3 sm:grid-cols-3">
          <Fact name="Started" value={clock(row.started_at)} />
          <Fact name="Time to first byte" value={duration(row.ttft_ms)} />
          <Fact name="Took" value={duration(row.duration_ms)} />
          <Fact name="Provider" value={row.provider} />
          <Fact name="Account" value={row.account ?? UNKNOWN} />
          <Fact name="Person" value={row.person ?? UNKNOWN} />
          <Fact name="Client" value={row.client ?? UNKNOWN} />
          <Fact name="Format in / out" value={`${row.ingress} → ${row.egress}`} />
          <Fact name="Conversation" value={row.conversation ?? UNKNOWN} />
        </dl>
      </Panel>

      {row.error_message ? (
        <Panel>
          <PanelHead title="What the provider said" note={row.error_kind ?? undefined} />
          <pre className="m-0 overflow-x-auto p-3 font-mono text-code whitespace-pre-wrap text-[var(--critical)]">
            {row.error_message}
          </pre>
        </Panel>
      ) : null}

      <Panel>
        <PanelHead title="What it used" />
        <dl className="m-0 grid grid-cols-2 gap-x-6 gap-y-0 p-3 sm:grid-cols-3">
          <Fact name="Input" value={tokens(row.usage.input_tokens)} />
          <Fact name="Output" value={tokens(row.usage.output_tokens)} />
          <Fact name="Read from cache" value={tokens(row.usage.cache_read_tokens)} />
          <Fact name="Written to cache" value={tokens(row.usage.cache_write_tokens)} />
          <Fact name="Thinking" value={tokens(row.usage.reasoning_tokens)} />
          <Fact name="Cost" value={money(row.cost_micros)} />
        </dl>
      </Panel>

      <Panel>
        <PanelHead
          title="What we changed"
          note={row.byte_identical ? "nothing — the bytes went through untouched" : undefined}
        />
        <dl className="m-0 grid grid-cols-2 gap-x-6 gap-y-0 p-3 sm:grid-cols-3">
          <Fact name="Body in" value={row.input_digest ?? UNKNOWN} />
          <Fact name="Body out" value={row.output_digest ?? UNKNOWN} />
          <Fact name="Provider request id" value={row.upstream_request_id ?? UNKNOWN} />
        </dl>
        {row.evidence ? (
          <pre className="m-0 overflow-x-auto border-t border-line-subtle p-3 font-mono text-code text-ink-secondary">
            {JSON.stringify(row.evidence, null, 2)}
          </pre>
        ) : null}
      </Panel>
    </Page>
  );
}

function Fact({ name, value }: { name: string; value: string }) {
  return (
    <div className="flex flex-col gap-0.5 border-b border-line-subtle py-2 last:border-b-0">
      <dt className="text-micro tracking-wide text-ink-muted uppercase">{name}</dt>
      <dd className="numeric m-0 truncate text-ink" title={value}>
        {value}
      </dd>
    </div>
  );
}
