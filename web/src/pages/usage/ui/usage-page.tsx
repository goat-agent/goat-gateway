import { useState } from "react";
import {
  Body,
  Cell,
  Count,
  Foot,
  Head,
  Nothing,
  Page,
  Panel,
  PanelHead,
  Row,
  Select,
  Table,
  rowHover,
} from "@/shared/ui";
import { query, useResource } from "@/shared/api";
import { SPAN_CHOICES, WEEK, spanOf, type Span } from "@/shared/config/spans";
import { seriesColor } from "@/shared/model/series";
import { count, money, tokens } from "@/shared/lib/format";
import { GROUPINGS, METRICS, groupingOf, type MetricName, type UsageReport } from "@/entities/usage";
import { MetricChart } from "@/widgets/metric-chart";
import { UsageBars } from "@/widgets/usage-bars";
import { MetricPicker } from "@/features/pick-metric";
import { FilterChips, useFilter } from "@/features/filter-requests";

const GROUPING_CHOICES = GROUPINGS.map((grouping) => ({ value: grouping, label: `by ${grouping}` }));

export function UsagePage() {
  const { held, set } = useFilter(GROUPINGS);
  const [span, setSpan] = useState<Span>(WEEK);
  const [by, setBy] = useState(groupingOf("model"));
  const [chosen, setChosen] = useState<MetricName>("cost");
  const shown = METRICS[chosen];

  const report = useResource<UsageReport>(
    `/api/usage${query({ since: Date.now() - span.ms, by, bucket_ms: span.bucket, ...held })}`,
  );
  const data = report.data;
  const narrowed = Object.keys(held).length > 0;

  return (
    <Page
      title="Usage"
      note={`${span.label.toLowerCase()}, by ${by}`}
      refused={report.error}
      toolbar={
        <>
          <Select
            label="Group by"
            className="w-36"
            value={by}
            choices={GROUPING_CHOICES}
            onChange={setBy}
          />
          <Select
            label="Window"
            className="w-36"
            value={String(span.ms)}
            choices={SPAN_CHOICES}
            onChange={(value) => setSpan(spanOf(value))}
          />
          {narrowed ? <span className="mx-1 h-4 w-[var(--hairline)] bg-line" /> : null}
          <FilterChips held={held} onDrop={(field) => set(field, "")} />
        </>
      }
    >
      <Panel>
        <PanelHead title="Over time" note={`by ${by}`}>
          <MetricPicker chosen={chosen} onPick={setChosen} />
        </PanelHead>
        {data ? (
          <MetricChart
            buckets={data.series}
            metric={shown}
            bucketMs={data.bucket_ms}
            windowMs={span.ms}
            space={by}
          />
        ) : (
          <div className="h-[216px]" />
        )}
      </Panel>

      <div className="grid gap-4 lg:grid-cols-[minmax(0,20rem)_minmax(0,1fr)]">
        <Panel className="self-start">
          <PanelHead title={shown.label} note={`by ${by}`} />
          {data ? (
            <UsageBars
              slices={data.breakdown}
              metric={shown}
              space={by}
              onPick={(key) => set(by, key)}
            />
          ) : null}
        </Panel>

        <Panel>
          <PanelHead title="Everything" note={`by ${by}`} />
          {data && data.breakdown.length > 0 ? (
            <Table>
              <Head>
                <Row>
                  <th className="capitalize">{by}</th>
                  <Count>Requests</Count>
                  <Count>Errors</Count>
                  <Count>In</Count>
                  <Count>Out</Count>
                  <Count>Cached</Count>
                  <Count>Thinking</Count>
                  <Count>Cost</Count>
                </Row>
              </Head>
              <Body>
                {data.breakdown.map((slice) => (
                  <Row key={slice.key} className={rowHover}>
                    <td>
                      <span className="flex items-center gap-2.5 font-medium text-ink">
                        <span
                          className="size-2.5 shrink-0 rounded-[3px]"
                          style={{ background: seriesColor(by, slice.key) }}
                        />
                        <span className="max-w-64 truncate">{slice.key || "unattributed"}</span>
                      </span>
                    </td>
                    <Cell>{count(slice.totals.requests)}</Cell>
                    <Cell className={slice.totals.errors > 0 ? "text-critical-text" : undefined}>
                      {count(slice.totals.errors)}
                    </Cell>
                    <Cell>{tokens(slice.totals.usage.input_tokens)}</Cell>
                    <Cell>{tokens(slice.totals.usage.output_tokens)}</Cell>
                    <Cell>{tokens(slice.totals.usage.cache_read_tokens)}</Cell>
                    <Cell>{tokens(slice.totals.usage.reasoning_tokens)}</Cell>
                    <Cell>{money(slice.totals.cost_micros)}</Cell>
                  </Row>
                ))}
              </Body>
              <Foot>
                <Row>
                  <td>Total</td>
                  <Cell>{count(data.totals.requests)}</Cell>
                  <Cell>{count(data.totals.errors)}</Cell>
                  <Cell>{tokens(data.totals.usage.input_tokens)}</Cell>
                  <Cell>{tokens(data.totals.usage.output_tokens)}</Cell>
                  <Cell>{tokens(data.totals.usage.cache_read_tokens)}</Cell>
                  <Cell>{tokens(data.totals.usage.reasoning_tokens)}</Cell>
                  <Cell>{money(data.totals.cost_micros)}</Cell>
                </Row>
              </Foot>
            </Table>
          ) : (
            <Nothing
              says={
                narrowed
                  ? "Nothing matches these filters. Drop one and the rest of the window comes back."
                  : "No usage recorded in this window."
              }
            />
          )}
        </Panel>
      </div>
    </Page>
  );
}
