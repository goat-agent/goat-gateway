import { useState } from "react";
import { Body, Cell, Count, Foot, Head, Nothing, Page, Panel, PanelHead, Row, Select, Table } from "@/shared/ui";
import { query, useResource } from "@/shared/api";
import { SPANS, WEEK, spanOf, type Span } from "@/shared/config/spans";
import { seriesColor } from "@/shared/model/series";
import { count, money, tokens } from "@/shared/lib/format";
import { GROUPINGS, METRICS, groupingOf, type MetricName, type UsageReport } from "@/entities/usage";
import { MetricChart } from "@/widgets/metric-chart";
import { UsageBars } from "@/widgets/usage-bars";
import { MetricPicker } from "@/features/pick-metric";
import { FilterChips, useFilter } from "@/features/filter-requests";

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
      note={report.error}
      aside={
        <>
          <Select className="w-32" value={by} onChange={(event) => setBy(groupingOf(event.target.value))}>
            {GROUPINGS.map((grouping) => (
              <option key={grouping} value={grouping}>
                by {grouping}
              </option>
            ))}
          </Select>
          <Select
            className="w-36"
            value={String(span.ms)}
            onChange={(event) => setSpan(spanOf(event.target.value))}
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
      <FilterChips held={held} onDrop={(field) => set(field, "")} />

      <Panel>
        <PanelHead title="Over time" note={`by ${by}`}>
          <MetricPicker chosen={chosen} onPick={setChosen} />
        </PanelHead>
        {data ? (
          <MetricChart buckets={data.series} metric={shown} bucketMs={data.bucket_ms} space={by} />
        ) : (
          <div className="h-[188px]" />
        )}
      </Panel>

      <div className="grid gap-4 lg:grid-cols-[minmax(0,20rem)_minmax(0,1fr)]">
        <Panel>
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
                  <th>{by}</th>
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
                  <Row key={slice.key}>
                    <td>
                      <span className="flex items-center gap-2">
                        <span
                          className="size-2 shrink-0 rounded-[2px]"
                          style={{ background: seriesColor(by, slice.key) }}
                        />
                        {slice.key || "unattributed"}
                      </span>
                    </td>
                    <Cell>{count(slice.totals.requests)}</Cell>
                    <Cell className={slice.totals.errors > 0 ? "text-[var(--critical)]" : undefined}>
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
