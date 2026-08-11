import { useCallback, useState } from "react";
import { Link } from "react-router-dom";
import { Body, Button, Cell, Count, Foot, Head, Nothing, Page, Panel, PanelHead, Row, Select, Table, Tile } from "@/shared/ui";
import { query, useHappenings, useResource } from "@/shared/api";
import { SPANS, DAY, spanOf, type Span } from "@/shared/config/spans";
import { seriesColor } from "@/shared/model/series";
import { count, duration, money, percent, tokens } from "@/shared/lib/format";
import { METRICS, type MetricName, type UsageReport } from "@/entities/usage";
import type { Overview } from "@/entities/provider";
import { MetricChart } from "@/widgets/metric-chart";
import { QuotaTable } from "@/widgets/quota-table";
import { MetricPicker } from "@/features/pick-metric";

export function OverviewPage() {
  const [span, setSpan] = useState<Span>(DAY);
  const [chosen, setChosen] = useState<MetricName>("requests");
  const shown = METRICS[chosen];

  const overview = useResource<Overview>(`/api/overview${query({ window_ms: span.ms })}`);
  const usage = useResource<UsageReport>(
    `/api/usage${query({ since: Date.now() - span.ms, by: "provider", bucket_ms: span.bucket })}`,
  );

  const reload = overview.reload;
  useHappenings(
    useCallback(
      (happening) => {
        if (happening === "request_settled" || happening === "account_changed") reload();
      },
      [reload],
    ),
  );

  const seen = overview.data;
  const report = usage.data;

  if (overview.error && !seen) {
    return (
      <Page title="Overview">
        <Panel>
          <Nothing says={overview.error} />
        </Panel>
      </Page>
    );
  }

  if (seen && seen.providers.length === 0) {
    return (
      <Page title="Overview">
        <Panel>
          <Nothing
            says="No provider account is registered, so there is nothing for this gateway to serve requests with."
            offers={
              <Link to="/accounts">
                <Button tone="primary">Add an account</Button>
              </Link>
            }
          />
        </Panel>
      </Page>
    );
  }

  return (
    <Page
      title="Overview"
      note={seen ? `prices as of ${seen.pricing_as_of}` : undefined}
      aside={
        <Select
          className="w-40"
          value={String(span.ms)}
          onChange={(event) => setSpan(spanOf(event.target.value))}
        >
          {SPANS.map((entry) => (
            <option key={entry.ms} value={entry.ms}>
              {entry.label}
            </option>
          ))}
        </Select>
      }
    >
      <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-6">
        <Tile
          label="In flight"
          value={count(seen?.totals.in_flight)}
          note="right now"
          tone={seen && seen.totals.in_flight > 0 ? "good" : undefined}
        />
        <Tile
          label="Requests / hour"
          value={seen ? seen.requests_per_hour.toFixed(1) : "—"}
          note={`${count(seen?.totals.requests)} in window`}
        />
        <Tile
          label="Error rate"
          value={percent(seen?.error_ratio)}
          note={`${count(seen?.totals.errors)} failed`}
          tone={seen && (seen.error_ratio ?? 0) > 0.05 ? "critical" : undefined}
        />
        <Tile
          label="Median, successful"
          value={duration(seen?.totals.median_ms)}
          note={`slowest tenth ${duration(seen?.totals.slowest_tenth_ms)}`}
        />
        <Tile
          label="Read from cache"
          value={percent(seen?.cache_hit_ratio, 0)}
          note="of prompt tokens"
          tone={seen && (seen.cache_hit_ratio ?? 0) > 0.5 ? "good" : undefined}
        />
        <Tile
          label="Needs attention"
          value={count(seen?.attention.length)}
          note={seen?.attention[0]?.account ?? "every account is usable"}
          tone={seen && seen.attention.length > 0 ? "warning" : undefined}
        />
      </div>

      <Panel>
        <PanelHead title="Traffic" note={`by provider, ${span.label.toLowerCase()}`}>
          <MetricPicker chosen={chosen} onPick={setChosen} />
        </PanelHead>
        {report ? (
          <MetricChart buckets={report.series} metric={shown} bucketMs={report.bucket_ms} />
        ) : (
          <div className="h-[188px]" />
        )}
      </Panel>

      <Panel>
        <PanelHead title="Providers" note="the table is the legend" />
        {report && report.breakdown.length > 0 ? (
          <Table>
            <Head>
              <Row>
                <th>Provider</th>
                <Count>Requests</Count>
                <Count>Errors</Count>
                <Count>In</Count>
                <Count>Out</Count>
                <Count>Cached</Count>
                <Count>Cost</Count>
              </Row>
            </Head>
            <Body>
              {report.breakdown.map((slice) => (
                <Row key={slice.key}>
                  <td>
                    <Link
                      to={`/usage?provider=${encodeURIComponent(slice.key)}`}
                      className="flex items-center gap-2 text-ink no-underline hover:underline"
                    >
                      <span
                        className="size-2 shrink-0 rounded-[2px]"
                        style={{ background: seriesColor(slice.key) }}
                      />
                      {labelOf(seen, slice.key)}
                    </Link>
                  </td>
                  <Cell>{count(slice.totals.requests)}</Cell>
                  <Cell className={slice.totals.errors > 0 ? "text-[var(--critical)]" : undefined}>
                    {count(slice.totals.errors)}
                  </Cell>
                  <Cell>{tokens(slice.totals.usage.input_tokens)}</Cell>
                  <Cell>{tokens(slice.totals.usage.output_tokens)}</Cell>
                  <Cell>{tokens(slice.totals.usage.cache_read_tokens)}</Cell>
                  <Cell>{money(slice.totals.cost_micros)}</Cell>
                </Row>
              ))}
            </Body>
            <Foot>
              <Row>
                <td>Total</td>
                <Cell>{count(report.totals.requests)}</Cell>
                <Cell>{count(report.totals.errors)}</Cell>
                <Cell>{tokens(report.totals.usage.input_tokens)}</Cell>
                <Cell>{tokens(report.totals.usage.output_tokens)}</Cell>
                <Cell>{tokens(report.totals.usage.cache_read_tokens)}</Cell>
                <Cell title={priceNote(report)}>{money(report.totals.cost_micros)}</Cell>
              </Row>
            </Foot>
          </Table>
        ) : (
          <Nothing says="No requests have gone through in this window. Point a client at this gateway and it shows up here." />
        )}
      </Panel>

      <Panel>
        <PanelHead title="What is left" note="reported by the provider on requests we already made" />
        {seen ? <QuotaTable providers={seen.providers} now={seen.now} /> : null}
      </Panel>
    </Page>
  );
}

function labelOf(seen: Overview | undefined, provider: string) {
  return seen?.providers.find((entry) => entry.provider === provider)?.label ?? provider;
}

function priceNote(report: UsageReport) {
  const { priced_requests: priced, requests } = report.totals;
  if (priced === requests) return "every request in this window has a published price";
  return `${requests - priced} of ${requests} requests have no published price and are not counted`;
}
