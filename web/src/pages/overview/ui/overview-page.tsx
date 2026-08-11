import { useCallback, useState } from "react";
import { Link } from "react-router-dom";
import { useHappenings } from "@/shared/api";
import { show } from "@/shared/lib";
import { Button, Nothing, Page, Panel, PanelHead, Select, Tile } from "@/shared/ui";
import { useHealth } from "@/entities/provider";
import { METRICS, metric, useReport, type MetricName } from "@/entities/usage";
import { MetricChart } from "@/widgets/metric-chart";
import { ProviderList } from "@/widgets/provider-list";
import { QuotaTable } from "@/widgets/quota-table";

const SPANS = [
  { label: "Last hour", ms: 3_600_000, bucket: 300_000 },
  { label: "Last 24 hours", ms: 86_400_000, bucket: 3_600_000 },
  { label: "Last 7 days", ms: 604_800_000, bucket: 86_400_000 },
  { label: "Last 30 days", ms: 2_592_000_000, bucket: 86_400_000 },
];

export function OverviewPage() {
  const [span, setSpan] = useState(SPANS[1]!);
  const [chosen, setChosen] = useState<MetricName>("requests");
  const shown = metric(chosen);

  const health = useHealth(span.ms);
  const report = useReport({ since: Date.now() - span.ms, by: "provider", bucketMs: span.bucket });

  useHappenings(
    useCallback(
      (happening) => {
        if (happening.happened !== "limits_observed") health.reload();
      },
      [health],
    ),
  );

  const seen = health.data;
  const usage = report.data;

  if (!seen && health.error) {
    return (
      <Page title="Overview">
        <Panel>
          <Nothing says={health.error} />
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
            setSpan(SPANS.find((entry) => String(entry.ms) === event.target.value) ?? SPANS[1]!)
          }
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
          value={show.count(seen?.totals.in_flight)}
          note="right now"
          tone={(seen?.totals.in_flight ?? 0) > 0 ? "good" : undefined}
        />
        <Tile
          label="Requests / hour"
          value={seen ? seen.requests_per_hour.toFixed(1) : show.UNKNOWN}
          note={`${show.count(seen?.totals.requests)} in window`}
        />
        <Tile
          label="Error rate"
          value={show.percent(seen?.error_ratio)}
          note={`${show.count(seen?.totals.errors)} failed`}
          tone={(seen?.error_ratio ?? 0) > 0.05 ? "critical" : undefined}
        />
        <Tile
          label="Median, successful"
          value={show.duration(seen?.totals.median_ms)}
          note={`slowest tenth ${show.duration(seen?.totals.slowest_tenth_ms)}`}
        />
        <Tile
          label="Read from cache"
          value={show.percent(seen?.cache_hit_ratio, 0)}
          note="of prompt tokens"
          tone={(seen?.cache_hit_ratio ?? 0) > 0.5 ? "good" : undefined}
        />
        <Tile
          label="Needs attention"
          value={show.count(seen?.attention.length)}
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
        {usage ? (
          <MetricChart buckets={usage.series} metric={shown} bucketMs={usage.bucket_ms} />
        ) : (
          <div className="h-[188px]" />
        )}
      </Panel>

      <Panel>
        <PanelHead title="Providers" note="the table is the legend" />
        {usage && usage.breakdown.length > 0 ? (
          <ProviderList report={usage} labels={labels(seen?.providers ?? [])} />
        ) : (
          <Nothing says="No requests have gone through in this window. Point a client at this gateway and it will show up here." />
        )}
      </Panel>

      <Panel>
        <PanelHead title="What is left" note="reported on the requests we already made" />
        {seen ? <QuotaTable providers={seen.providers} now={seen.now} /> : null}
      </Panel>
    </Page>
  );
}

function labels(providers: { provider: string; label: string }[]) {
  return Object.fromEntries(providers.map((entry) => [entry.provider, entry.label]));
}
