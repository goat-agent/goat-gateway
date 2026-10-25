import { useCallback, useState } from "react";
import { Link } from "react-router-dom";
import { PlugZap } from "lucide-react";
import {
  Body,
  ButtonLink,
  Cell,
  Count,
  Foot,
  Head,
  Nothing,
  Page,
  Row,
  Section,
  Select,
  Stat,
  Stats,
  Table,
  rowHover,
} from "@/shared/ui";
import { query, useHappenings, useResource } from "@/shared/api";
import { DAY, SPAN_CHOICES, spanOf, type Span } from "@/shared/config/spans";
import { seriesColor } from "@/shared/model/series";
import { count, duration, money, percent, tokens } from "@/shared/lib/format";
import { METRICS, type MetricName, type UsageReport } from "@/entities/usage";
import { QuotaGlance, quotaOf, type Overview } from "@/entities/provider";
import { StateDot, stateWords, type Account } from "@/entities/account";
import { RequestLine, type Request } from "@/entities/request";
import { MetricChart } from "@/widgets/metric-chart";
import { MetricPicker } from "@/features/pick-metric";

const ATTENTION_FIRST: Record<Account["state"], number> = {
  rate_limited: 0,
  sign_in_expired: 1,
  active: 2,
  disabled: 3,
};

export function HomePage() {
  const [span, setSpan] = useState<Span>(DAY);
  const [chosen, setChosen] = useState<MetricName>("requests");

  const overview = useResource<Overview>(`/api/overview${query({ window_ms: span.ms })}`);
  const usage = useResource<UsageReport>(
    `/api/usage${query({ since: Date.now() - span.ms, by: "provider", bucket_ms: span.bucket })}`,
  );
  const accounts = useResource<{ accounts: Account[] }>("/api/accounts");
  const latest = useResource<{ requests: Request[] }>(`/api/requests${query({ limit: 12 })}`);

  const reloadOverview = overview.reload;
  const reloadUsage = usage.reload;
  const reloadAccounts = accounts.reload;
  const reloadLatest = latest.reload;
  useHappenings(
    useCallback(
      (happening) => {
        if (happening === "request_opened") reloadLatest();
        if (happening === "request_settled") {
          reloadOverview();
          reloadUsage();
          reloadLatest();
        }
        if (happening === "account_changed" || happening === "limits_observed") {
          reloadOverview();
          reloadAccounts();
        }
      },
      [reloadOverview, reloadUsage, reloadAccounts, reloadLatest],
    ),
  );

  const seen = overview.data;
  const report = usage.data;
  const now = seen?.now ?? Date.now();

  const windowPicker = (
    <Select
      label="Window"
      className="w-36"
      value={String(span.ms)}
      choices={SPAN_CHOICES}
      onChange={(value) => setSpan(spanOf(value))}
    />
  );

  if (overview.error && !seen) {
    return (
      <Page title="Home">
        <Nothing says={overview.error} />
      </Page>
    );
  }

  if (seen && seen.providers.length === 0) {
    return (
      <Page title="Home">
        <Nothing
          icon={PlugZap}
          says="No provider account is registered, so there is nothing for this gateway to serve requests with."
          offers={
            <ButtonLink to="/accounts" tone="primary" size="medium">
              Add an account
            </ButtonLink>
          }
        />
      </Page>
    );
  }

  const registered = [...(accounts.data?.accounts ?? [])].sort(
    (one, other) => ATTENTION_FIRST[one.state] - ATTENTION_FIRST[other.state],
  );
  const usable = registered.filter((account) => account.state === "active").length;
  const labelOf = (provider: string) =>
    seen?.providers.find((entry) => entry.provider === provider)?.label ?? provider;
  const stuck = (seen?.attention ?? []).filter((held) => held.state !== "disabled");

  return (
    <Page title="Home" refused={overview.error ?? usage.error} actions={windowPicker}>
      {stuck.length > 0 ? (
        <div className="flex flex-col hairline-b border-line-subtle bg-warning-tint/50 py-2">
          <span className="flex h-7 items-center px-5 text-mini font-medium text-warning-text">
            {stuck.length === 1
              ? "One account cannot serve right now"
              : `${stuck.length} accounts cannot serve right now`}
          </span>
          {stuck.map((held) => (
            <Link
              key={held.account}
              to={`/accounts/${encodeURIComponent(held.account)}`}
              className="hover-fade flex h-8 items-center gap-3 px-5 text-small hover:bg-warning-tint"
            >
              <StateDot state={held.state} until={held.until} now={now} />
              <span className="font-medium text-ink">{held.account}</span>
              <span className="text-ink-secondary">{stateWords(held.state, held.until, now)}</span>
              <span className="ml-auto text-mini text-ink-secondary">{labelOf(held.provider)}</span>
            </Link>
          ))}
        </div>
      ) : null}

      <Stats>
        <Stat
          label="In flight"
          value={count(seen?.totals.in_flight)}
          note="right now"
          tone={seen && seen.totals.in_flight > 0 ? "good" : undefined}
        />
        <Stat
          label="Requests per hour"
          value={seen ? seen.requests_per_hour.toFixed(1) : "—"}
          note={`${count(seen?.totals.requests)} in window`}
        />
        <Stat
          label="Error rate"
          value={percent(seen?.error_ratio)}
          note={`${count(seen?.totals.errors)} failed`}
          tone={seen && (seen.error_ratio ?? 0) > 0.05 ? "critical" : undefined}
        />
        <Stat
          label="Median, successful"
          value={duration(seen?.totals.median_ms)}
          note={`slowest tenth ${duration(seen?.totals.slowest_tenth_ms)}`}
        />
        <Stat
          label="Read from cache"
          value={percent(seen?.cache_hit_ratio, 0)}
          note="of prompt tokens"
          tone={seen && (seen.cache_hit_ratio ?? 0) > 0.5 ? "good" : undefined}
        />
      </Stats>

      <div className="grid xl:grid-cols-[minmax(0,1fr)_minmax(0,24rem)] 2xl:grid-cols-[minmax(0,1fr)_minmax(0,28rem)]">
        <div className="flex min-w-0 flex-col pb-4">
          <Section
            title="Traffic"
            note={`by provider, ${span.label.toLowerCase()}`}
            aside={<MetricPicker chosen={chosen} onPick={setChosen} />}
          >
            {report ? (
              <MetricChart
                buckets={report.series}
                metric={METRICS[chosen]}
                bucketMs={report.bucket_ms}
                windowMs={span.ms}
                space="provider"
              />
            ) : (
              <div className="h-[216px]" />
            )}
          </Section>

          {report && report.breakdown.length > 0 ? (
            <Table className="mt-2">
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
                  <Row key={slice.key} className={rowHover}>
                    <td>
                      <Link
                        to={`/usage${query({ provider: slice.key })}`}
                        className="flex items-center gap-2.5 font-medium text-ink hover:text-ink-title"
                      >
                        <span
                          className="size-2.5 shrink-0 rounded-[3px]"
                          style={{ background: seriesColor("provider", slice.key) }}
                        />
                        {labelOf(slice.key)}
                      </Link>
                    </td>
                    <Cell>{count(slice.totals.requests)}</Cell>
                    <Cell className={slice.totals.errors > 0 ? "text-critical-text" : undefined}>
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
          ) : report ? (
            <p className="m-0 px-5 py-6 text-small text-ink-secondary">
              No requests have gone through in this window. Point a client at this gateway and it
              shows up here.
            </p>
          ) : null}
        </div>

        <div className="flex min-w-0 flex-col hairline-t border-line-subtle xl:border-t-0 xl:hairline-l">
          <Section
            title="Accounts"
            note={accounts.data ? `${usable} of ${registered.length} usable` : undefined}
            aside={<More to="/accounts" />}
          >
            <div className="flex flex-col pb-3">
              {registered.map((account) => (
                <Link
                  key={account.name}
                  to={`/accounts/${encodeURIComponent(account.name)}`}
                  title={labelOf(account.provider)}
                  className="hover-fade grid h-[var(--row-compact)] grid-cols-[auto_minmax(0,1fr)_minmax(0,11rem)] items-center gap-3 px-5 text-small hover:bg-panel-hover"
                >
                  <StateDot state={account.state} until={account.cooldown_until} now={now} />
                  <span className="truncate text-ink">{account.name}</span>
                  <QuotaGlance
                    quota={quotaOf(seen?.providers, account.provider, account.name)}
                    account={account.name}
                    now={now}
                    resets
                  />
                </Link>
              ))}
            </div>
          </Section>

          <Section
            title="Latest requests"
            aside={<More to="/requests" />}
            className="hairline-t border-line-subtle"
          >
            <div className="flex flex-col pb-3">
              {(latest.data?.requests ?? []).map((request) => (
                <RequestLine key={request.id} request={request} now={now} />
              ))}
              {latest.data && latest.data.requests.length === 0 ? (
                <p className="m-0 px-5 py-2 text-small text-ink-secondary">
                  No request has reached this gateway yet.
                </p>
              ) : null}
            </div>
          </Section>
        </div>
      </div>
    </Page>
  );
}

function More({ to }: { to: string }) {
  return (
    <Link to={to} className="hover-fade text-mini text-ink-secondary hover:text-ink">
      Show all
    </Link>
  );
}

function priceNote(report: UsageReport) {
  const { priced_requests: priced, requests } = report.totals;
  if (priced === requests) return "every request in this window has a published price";
  return `${requests - priced} of ${requests} requests have no published price and are not counted`;
}
