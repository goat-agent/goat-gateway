import { useState } from "react";
import {
  Body,
  Cell,
  Count,
  Foot,
  Head,
  Nothing,
  Page,
  Row,
  Section,
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
import { MetricPicker } from "@/features/pick-metric";
import { FilterChips, useFilter } from "@/features/filter-requests";
import { useKeyNames } from "@/entities/user";

const GROUPING_CHOICES = GROUPINGS.map((grouping) => ({ value: grouping, label: `by ${grouping}` }));

export function UsagePage() {
  const { held, set } = useFilter(GROUPINGS);
  const [span, setSpan] = useState<Span>(WEEK);
  const [by, setBy] = useState(groupingOf("model"));
  const [chosen, setChosen] = useState<MetricName>("cost");
  const shown = METRICS[chosen];
  const keyName = useKeyNames();
  const named = (key: string) => (by === "key" ? keyName(key) : key) || "unattributed";

  const report = useResource<UsageReport>(
    `/api/usage${query({ since: Date.now() - span.ms, by, bucket_ms: span.bucket, ...held })}`,
  );
  const data = report.data;
  const narrowed = Object.keys(held).length > 0;
  const slices = data?.breakdown ?? [];
  const widest = Math.max(1, ...slices.map((slice) => shown.of(slice.totals)));

  return (
    <Page
      title="Usage"
      refused={report.error}
      actions={
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
        </>
      }
      toolbar={narrowed ? <FilterChips held={held} onDrop={(field) => set(field, "")} /> : undefined}
    >
      <Section
        title="Over time"
        note={`${shown.label.toLowerCase()} by ${by}, ${span.label.toLowerCase()}`}
        aside={<MetricPicker chosen={chosen} onPick={setChosen} />}
      >
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
      </Section>

      {data && slices.length > 0 ? (
        <Table className="mt-2 mb-6">
          <Head>
            <Row>
              <th className="capitalize">{by}</th>
              <th className="w-[18%]">Share</th>
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
            {slices.map((slice) => (
              <Row
                key={slice.key}
                className={rowHover}
                title={`Only show ${named(slice.key)}`}
                onClick={() => set(by, slice.key)}
              >
                <td>
                  <span className="flex items-center gap-2.5 font-medium text-ink">
                    <span
                      className="size-2.5 shrink-0 rounded-[3px]"
                      style={{ background: seriesColor(by, slice.key) }}
                    />
                    <span className="max-w-72 truncate">{named(slice.key)}</span>
                  </span>
                </td>
                <td>
                  <span className="block h-1.5 w-full rounded-full bg-line-subtle">
                    <span
                      className="block h-full rounded-full"
                      style={{
                        width: `${(shown.of(slice.totals) / widest) * 100}%`,
                        background: seriesColor(by, slice.key),
                      }}
                    />
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
              <td />
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
      ) : data ? (
        <Nothing
          says={
            narrowed
              ? "Nothing matches these filters. Drop one and the rest of the window comes back."
              : "No usage recorded in this window."
          }
        />
      ) : null}
    </Page>
  );
}
