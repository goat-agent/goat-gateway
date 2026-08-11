import { useCallback, useState } from "react";
import { Link } from "react-router-dom";
import { Page, Tile } from "../page";
import { Panel, PanelHead } from "@/shared/ui/panel";
import { Nothing } from "@/shared/ui/nothing";
import { Button } from "@/shared/ui/button";
import { Select } from "@/shared/ui/field";
import { MetricChart } from "@/widgets/metric-chart/metric-chart";
import { QuotaTable } from "@/widgets/quota-table/quota-table";
import { useResource } from "@/shared/api/use-resource";
import { useHappenings } from "@/shared/api/happenings";
import { query } from "@/shared/api/client";
import { METRICS, metric, type MetricName } from "@/entities/metric";
import type { Overview, UsageReport } from "@/entities/types";
import { count, duration, money, percent, tokens, whole } from "@/shared/lib/format";
import { Body, Foot, Head, Numeric, NumericHead, Row, Table } from "@/shared/ui/table";
import { seriesColor } from "@/shared/lib/series-color";

const WINDOWS = [
  { label: "Last hour", ms: 3_600_000, bucket: 300_000 },
  { label: "Last 24 hours", ms: 86_400_000, bucket: 3_600_000 },
  { label: "Last 7 days", ms: 604_800_000, bucket: 86_400_000 },
  { label: "Last 30 days", ms: 2_592_000_000, bucket: 86_400_000 },
];

export function OverviewPage() {
  const [span, setSpan] = useState(WINDOWS[1]!);
  const [chosen, setChosen] = useState<MetricName>("requests");
  const shown = metric(chosen);

  const overview = useResource<Overview>(`/api/overview${query({ window_ms: span.ms })}`, [span.ms]);
  const usage = useResource<UsageReport>(
    `/api/usage${query({ since: Date.now() - span.ms, by: "provider", bucket_ms: span.bucket })}`,
    [span.ms],
  );

  useHappenings(
    useCallback(
      (happening) => {
        if (happening.happened === "request_settled" || happening.happened === "account_changed") {
          overview.reload();
        }
      },
      [overview],
    ),
  );

  const seen = overview.data;
  const report = usage.data;

  if (!seen && overview.error) {
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
            says="No provider account is registered yet, so there is nothing for this gateway to serve requests with."
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
          onChange={(event) =>
            setSpan(WINDOWS.find((entry) => String(entry.ms) === event.target.value) ?? WINDOWS[1]!)
          }
        >
          {WINDOWS.map((entry) => (
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
          tone={(seen?.totals.in_flight ?? 0) > 0 ? "good" : undefined}
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
          tone={(seen?.error_ratio ?? 0) > 0.05 ? "critical" : undefined}
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
          tone={(seen?.cache_hit_ratio ?? 0) > 0.5 ? "good" : undefined}
        />
        <Tile
          label="Needs attention"
          value={count(seen?.attention.length)}
          note={seen?.attention[0]?.account ?? "every account is usable"}
          tone={(seen?.attention.length ?? 0) > 0 ? "warning" : undefined}
        />
      </div>

      <Panel>
        <PanelHead title="Traffic" note={`by provider, ${span.label.toLowerCase()}`}>
          {METRICS.map((entry) => (
            <Button
              key={entry.name}
              size="small"
              tone={entry.name === chosen ? "plain" : "quiet"}
              onClick={() => setChosen(entry.name)}
            >
              {entry.label}
            </Button>
          ))}
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
                <NumericHead>Requests</NumericHead>
                <NumericHead>Errors</NumericHead>
                <NumericHead>In</NumericHead>
                <NumericHead>Out</NumericHead>
                <NumericHead>Cached</NumericHead>
                <NumericHead>Cost</NumericHead>
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
                      {label(seen, slice.key)}
                    </Link>
                  </td>
                  <Numeric>{count(slice.totals.requests)}</Numeric>
                  <Numeric className={slice.totals.errors > 0 ? "text-[var(--critical)]" : ""}>
                    {count(slice.totals.errors)}
                  </Numeric>
                  <Numeric>{tokens(slice.totals.usage.input_tokens)}</Numeric>
                  <Numeric>{tokens(slice.totals.usage.output_tokens)}</Numeric>
                  <Numeric>{tokens(slice.totals.usage.cache_read_tokens)}</Numeric>
                  <Numeric>{money(slice.totals.cost_micros)}</Numeric>
                </Row>
              ))}
            </Body>
            <Foot>
              <Row>
                <td>Total</td>
                <Numeric>{count(report.totals.requests)}</Numeric>
                <Numeric>{count(report.totals.errors)}</Numeric>
                <Numeric>{tokens(report.totals.usage.input_tokens)}</Numeric>
                <Numeric>{tokens(report.totals.usage.output_tokens)}</Numeric>
                <Numeric>{tokens(report.totals.usage.cache_read_tokens)}</Numeric>
                <Numeric title={priceNote(report)}>{money(report.totals.cost_micros)}</Numeric>
              </Row>
            </Foot>
          </Table>
        ) : (
          <Nothing says="No requests have gone through in this window. Point a client at this gateway and it will show up here." />
        )}
      </Panel>

      <Panel>
        <PanelHead
          title="What is left"
          note="reported by the provider on the requests we already made"
        />
        {seen ? <QuotaTable providers={seen.providers} now={seen.now} /> : null}
      </Panel>
    </Page>
  );
}

function label(seen: Overview | undefined, provider: string) {
  return seen?.providers.find((entry) => entry.provider === provider)?.label ?? provider;
}

function priceNote(report: UsageReport) {
  const priced = report.totals.priced_requests;
  const all = report.totals.requests;
  if (priced === all) return "every request in this window has a published price";
  return `${whole((priced / Math.max(all, 1)) * 100)} of requests have a published price; the rest are not counted`;
}
