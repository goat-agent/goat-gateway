import { Body, Cell, Count, Head, Nothing, Row, Table } from "@/shared/ui";
import type { ProviderHealth } from "@/entities/provider";
import { ago, share, within } from "@/shared/lib/format";

export function QuotaTable({ providers, now }: { providers: ProviderHealth[]; now: number }) {
  const rows = providers.flatMap((provider) =>
    provider.limits.flatMap((limit) =>
      limit.windows.map((window) => ({ account: limit.account, seen: limit.observed_at, ...window })),
    ),
  );

  const quiet = providers.filter((provider) => !provider.reports_limits);
  const unseen = providers.filter(
    (provider) => provider.reports_limits && provider.limits.length === 0,
  );

  if (rows.length === 0) {
    return (
      <Nothing
        says={
          unseen.length > 0
            ? "Nothing observed yet. Quota is read off the responses to real requests, so it fills in as soon as traffic flows."
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
            <th>Applies to</th>
            <Count>Used</Count>
            <Count>Resets in</Count>
            <Count>Observed</Count>
          </Row>
        </Head>
        <Body>
          {rows.map((row) => (
            <Row key={`${row.account}-${row.label}-${row.scope ?? "all"}`}>
              <td className="text-ink">{row.account}</td>
              <td className="text-ink-secondary">{row.label}</td>
              <td className="text-ink-secondary">{row.scope ?? "every model"}</td>
              <td className="w-44">
                <span className="flex items-center gap-2">
                  <span className="numeric w-9 shrink-0 text-right">{share(row.used_percent)}</span>
                  <span className="block h-1.5 grow rounded-full bg-raised">
                    <span
                      className="block h-full rounded-full"
                      style={{
                        width: `${Math.min(100, Math.max(2, row.used_percent))}%`,
                        background: pressure(row.used_percent),
                      }}
                    />
                  </span>
                </span>
              </td>
              <Cell className="text-ink-secondary">{within(row.resets_at_ms, now)}</Cell>
              <Cell className="text-ink-muted">{ago(row.seen, now)}</Cell>
            </Row>
          ))}
        </Body>
      </Table>

      {quiet.length > 0 || unseen.length > 0 ? (
        <p className="m-0 border-t border-line-subtle px-3 py-2 text-micro text-ink-muted">
          {[
            quiet.length > 0 ? `${named(quiet)} never reports quota.` : undefined,
            unseen.length > 0 ? `${named(unseen)} reports it, but nothing has come back yet.` : undefined,
          ]
            .filter((line) => line !== undefined)
            .join(" ")}
        </p>
      ) : null}
    </>
  );
}

function named(providers: ProviderHealth[]) {
  return providers.map((provider) => provider.label).join(", ");
}

function pressure(used: number) {
  if (used >= 90) return "var(--critical)";
  if (used >= 70) return "var(--warning)";
  return "var(--series-3)";
}
