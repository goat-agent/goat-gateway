import { useState } from "react";
import { useSearchParams } from "react-router-dom";
import { X } from "lucide-react";
import { show } from "@/shared/lib";
import { seriesColor } from "@/shared/model";
import {
  Badge,
  Body,
  Button,
  Foot,
  Head,
  Nothing,
  Numeric,
  NumericHead,
  Page,
  Panel,
  PanelHead,
  Row,
  Select,
  Table,
} from "@/shared/ui";
import { METRICS, metric, useReport, type Grouping, type MetricName } from "@/entities/usage";
import { MetricChart } from "@/widgets/metric-chart";
import { UsageBars } from "@/widgets/usage-bars";

const SPANS = [
  { label: "Last 24 hours", ms: 86_400_000, bucket: 3_600_000 },
  { label: "Last 7 days", ms: 604_800_000, bucket: 86_400_000 },
  { label: "Last 30 days", ms: 2_592_000_000, bucket: 86_400_000 },
];

const GROUPINGS: Grouping[] = ["provider", "model", "account", "person", "client"];

export function UsagePage() {
  const [params, setParams] = useSearchParams();
  const [span, setSpan] = useState(SPANS[1]!);
  const [by, setBy] = useState<Grouping>("model");
  const [chosen, setChosen] = useState<MetricName>("cost");
  const shown = metric(chosen);

  const narrowed: Record<string, string> = {};
  for (const field of GROUPINGS) {
    const value = params.get(field);
    if (value) narrowed[field] = value;
  }

  const report = useReport({ since: Date.now() - span.ms, by, bucketMs: span.bucket, narrowed });
  const data = report.data;

  const narrow = (field: string, value: string | null) => {
    const next = new URLSearchParams(params);
    if (value) next.set(field, value);
    else next.delete(field);
    setParams(next);
  };

  return (
    <Page
      title="Usage"
      note={report.error}
      aside={
        <>
          <Select className="w-32" value={by} onChange={(event) => setBy(event.target.value as Grouping)}>
            {GROUPINGS.map((field) => (
              <option key={field} value={field}>
                by {field}
              </option>
            ))}
          </Select>
          <Select
            className="w-36"
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
        </>
      }
    >
      {Object.keys(narrowed).length > 0 ? (
        <div className="flex flex-wrap items-center gap-1.5">
          {Object.entries(narrowed).map(([field, value]) => (
            <Badge key={field} className="gap-1.5 pr-1">
              {field}: {value}
              <button
                type="button"
                aria-label={`Stop filtering by ${field}`}
                className="grid size-3.5 place-items-center rounded-[2px] hover:bg-line"
                onClick={() => narrow(field, null)}
              >
                <X className="size-2.5" />
              </button>
            </Badge>
          ))}
        </div>
      ) : null}

      <Panel>
        <PanelHead title="Over time" note={`by ${by}`}>
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
        {data ? (
          <MetricChart buckets={data.series} metric={shown} bucketMs={data.bucket_ms} />
        ) : (
          <div className="h-[188px]" />
        )}
      </Panel>

      <div className="grid gap-4 xl:grid-cols-[minmax(0,18rem)_minmax(0,1fr)]">
        <Panel className="self-start">
          <PanelHead title={shown.label} note={`by ${by}`} />
          {data ? (
            <UsageBars slices={data.breakdown} metric={shown} onPick={(key) => narrow(by, key)} />
          ) : null}
        </Panel>

        <Panel>
          <PanelHead title="Everything" note={`by ${by}`} />
          {data && data.breakdown.length > 0 ? (
            <Table>
              <Head>
                <Row>
                  <th>{by}</th>
                  <NumericHead>Requests</NumericHead>
                  <NumericHead>Errors</NumericHead>
                  <NumericHead>In</NumericHead>
                  <NumericHead>Out</NumericHead>
                  <NumericHead>Cached</NumericHead>
                  <NumericHead>Thinking</NumericHead>
                  <NumericHead>Cost</NumericHead>
                </Row>
              </Head>
              <Body>
                {data.breakdown.map((slice) => (
                  <Row key={slice.key}>
                    <td>
                      <span className="flex items-center gap-2">
                        <span
                          className="size-2 shrink-0 rounded-[2px]"
                          style={{ background: seriesColor(slice.key) }}
                        />
                        {slice.key || "unattributed"}
                      </span>
                    </td>
                    <Numeric>{show.count(slice.totals.requests)}</Numeric>
                    <Numeric className={slice.totals.errors > 0 ? "text-[var(--critical)]" : ""}>
                      {show.count(slice.totals.errors)}
                    </Numeric>
                    <Numeric>{show.tokens(slice.totals.usage.input_tokens)}</Numeric>
                    <Numeric>{show.tokens(slice.totals.usage.output_tokens)}</Numeric>
                    <Numeric>{show.tokens(slice.totals.usage.cache_read_tokens)}</Numeric>
                    <Numeric>{show.tokens(slice.totals.usage.reasoning_tokens)}</Numeric>
                    <Numeric>{show.money(slice.totals.cost_micros)}</Numeric>
                  </Row>
                ))}
              </Body>
              <Foot>
                <Row>
                  <td>Total</td>
                  <Numeric>{show.count(data.totals.requests)}</Numeric>
                  <Numeric>{show.count(data.totals.errors)}</Numeric>
                  <Numeric>{show.tokens(data.totals.usage.input_tokens)}</Numeric>
                  <Numeric>{show.tokens(data.totals.usage.output_tokens)}</Numeric>
                  <Numeric>{show.tokens(data.totals.usage.cache_read_tokens)}</Numeric>
                  <Numeric>{show.tokens(data.totals.usage.reasoning_tokens)}</Numeric>
                  <Numeric>{show.money(data.totals.cost_micros)}</Numeric>
                </Row>
              </Foot>
            </Table>
          ) : (
            <Nothing
              says={
                Object.keys(narrowed).length > 0
                  ? "Nothing matches these filters. Clear one and the rest of the window comes back."
                  : "No usage recorded in this window."
              }
            />
          )}
        </Panel>
      </div>
    </Page>
  );
}
