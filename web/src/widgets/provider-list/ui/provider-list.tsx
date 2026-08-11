import { Link } from "react-router-dom";
import type { Report } from "@/entities/usage";
import { Body, Foot, Head, Numeric, NumericHead, Row, Table } from "@/shared/ui";
import { seriesColor } from "@/shared/model";
import { show } from "@/shared/lib";

export function ProviderList({
  report,
  labels,
}: {
  report: Report;
  labels: Record<string, string>;
}) {
  return (
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
                {labels[slice.key] ?? slice.key}
              </Link>
            </td>
            <Numeric>{show.count(slice.totals.requests)}</Numeric>
            <Numeric className={slice.totals.errors > 0 ? "text-[var(--critical)]" : ""}>
              {show.count(slice.totals.errors)}
            </Numeric>
            <Numeric>{show.tokens(slice.totals.usage.input_tokens)}</Numeric>
            <Numeric>{show.tokens(slice.totals.usage.output_tokens)}</Numeric>
            <Numeric>{show.tokens(slice.totals.usage.cache_read_tokens)}</Numeric>
            <Numeric>{show.money(slice.totals.cost_micros)}</Numeric>
          </Row>
        ))}
      </Body>
      <Foot>
        <Row>
          <td>Total</td>
          <Numeric>{show.count(report.totals.requests)}</Numeric>
          <Numeric>{show.count(report.totals.errors)}</Numeric>
          <Numeric>{show.tokens(report.totals.usage.input_tokens)}</Numeric>
          <Numeric>{show.tokens(report.totals.usage.output_tokens)}</Numeric>
          <Numeric>{show.tokens(report.totals.usage.cache_read_tokens)}</Numeric>
          <Numeric title={priceNote(report)}>{show.money(report.totals.cost_micros)}</Numeric>
        </Row>
      </Foot>
    </Table>
  );
}

function priceNote(report: Report) {
  const { priced_requests: priced, requests } = report.totals;
  if (priced === requests) return "every request in this window has a published price";
  return `${show.percent(priced / Math.max(requests, 1), 0)} of requests have a published price; the rest are not counted`;
}
