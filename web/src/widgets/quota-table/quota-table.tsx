import type { ProviderHealth } from "@/entities/types";
import { Body, Head, Numeric, NumericHead, Row, Table } from "@/shared/ui/table";
import { Nothing } from "@/shared/ui/nothing";
import { ago, until, whole } from "@/shared/lib/format";

export function QuotaTable({ providers, now }: { providers: ProviderHealth[]; now: number }) {
  const rows = providers.flatMap((provider) =>
    provider.limits.flatMap((limit) =>
      limit.windows.map((window) => ({
        provider: provider.label,
        account: limit.account,
        observed_at: limit.observed_at,
        ...window,
      })),
    ),
  );

  const quiet = providers.filter((provider) => !provider.reports_limits);
  const unseen = providers.filter((provider) => provider.reports_limits && provider.limits.length === 0);

  if (rows.length === 0) {
    return (
      <Nothing
        says={
          unseen.length > 0
            ? "Nothing has been observed yet. Quota is read off the responses to real requests, so it fills in as soon as traffic flows."
            : "None of the registered providers report what is left."
        }
      />
    );
  }

  return (
    <>
      <Table>
        <Head>
          <Row>
            <th>Account</th>
            <th>Window</th>
            <th>Model</th>
            <NumericHead>Used</NumericHead>
            <NumericHead>Resets in</NumericHead>
            <NumericHead>Observed</NumericHead>
          </Row>
        </Head>
        <Body>
          {rows.map((row) => (
            <Row key={`${row.account}-${row.label}-${row.scope ?? "all"}`}>
              <td className="text-ink">{row.account}</td>
              <td className="text-ink-secondary">{row.label}</td>
              <td className="text-ink-secondary">{row.scope ?? "every model"}</td>
              <td className="w-40">
                <span className="flex items-center gap-2">
                  <span className="numeric w-9 shrink-0 text-right">{whole(row.used_percent)}</span>
                  <span className="block h-1.5 grow rounded-full bg-raised">
                    <span
                      className="block h-full rounded-full"
                      style={{
                        width: `${Math.min(100, Math.max(2, row.used_percent))}%`,
                        background: bar(row.used_percent),
                      }}
                    />
                  </span>
                </span>
              </td>
              <Numeric className="text-ink-secondary">{until(row.resets_at_ms, now)}</Numeric>
              <Numeric className="text-ink-muted">{ago(row.observed_at, now)}</Numeric>
            </Row>
          ))}
        </Body>
      </Table>

      {quiet.length > 0 || unseen.length > 0 ? (
        <p className="m-0 border-t border-line-subtle px-3 py-2 text-micro text-ink-muted">
          {[
            quiet.length > 0 ? `${quiet.map((p) => p.label).join(", ")} never reports quota.` : null,
            unseen.length > 0
              ? `${unseen.map((p) => p.label).join(", ")} reports it, but nothing has come back yet.`
              : null,
          ]
            .filter(Boolean)
            .join(" ")}
        </p>
      ) : null}
    </>
  );
}

function bar(used: number) {
  if (used >= 90) return "var(--critical)";
  if (used >= 70) return "var(--warning)";
  return "var(--series-3)";
}
