import { Body, Cell, Count, Head, Meter, Nothing, Row, Table, rowHover } from "@/shared/ui";
import type { AccountLimits, ProviderHealth } from "@/entities/provider";
import { ago, within } from "@/shared/lib/format";

export function QuotaTable({ providers, now }: { providers: ProviderHealth[]; now: number }) {
  const rows = providers.flatMap((provider) =>
    provider.limits.flatMap((limit) =>
      limit.windows.map((window) => ({ account: limit.account, seen: limit.observed_at, ...window })),
    ),
  );

  const unread = providers.flatMap((provider) =>
    provider.limits.filter((limit) => limit.windows.length === 0 && limit.said),
  );
  const quiet = providers.filter((provider) => !provider.reports_limits);
  const unseen = providers.filter(
    (provider) => provider.reports_limits && provider.limits.length === 0,
  );

  if (rows.length === 0 && unread.length > 0) {
    return <Undeclared limits={unread} />;
  }

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
            <th className="w-56">Used</th>
            <Count>Resets in</Count>
            <Count>Observed</Count>
          </Row>
        </Head>
        <Body>
          {rows.map((row) => (
            <Row key={`${row.account}-${row.label}-${row.scope ?? "all"}`} className={rowHover}>
              <td className="font-medium text-ink">{row.account}</td>
              <td className="text-ink-secondary">{row.label}</td>
              <td className="text-ink-secondary">{row.scope ?? "every model"}</td>
              <td className="w-56">
                <Meter used={row.used_percent} label={`${row.label} used on ${row.account}`} />
              </td>
              <Cell className="text-ink-secondary">{within(row.resets_at_ms, now)}</Cell>
              <Cell className="text-ink-secondary">{ago(row.seen, now)}</Cell>
            </Row>
          ))}
        </Body>
      </Table>

      {unread.length > 0 ? <Undeclared limits={unread} /> : null}

      {quiet.length > 0 || unseen.length > 0 ? (
        <p className="m-0 hairline-t border-line-subtle px-4 py-2.5 text-mini text-ink-secondary">
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

function Undeclared({ limits }: { limits: AccountLimits[] }) {
  return (
    <div className="flex flex-col gap-3 hairline-t border-line-subtle px-4 py-3">
      <p className="m-0 text-mini text-ink-secondary">
        These providers answered, but this gateway has not been told where the numbers are in what
        they said. Add pointers under the provider's limits in config.toml.
      </p>
      {limits.map((limit) => (
        <div key={limit.account} className="flex flex-col gap-1.5">
          <span className="text-mini font-medium text-ink">{limit.account}</span>
          <pre className="mono m-0 overflow-x-auto rounded-lg hairline border-line bg-base p-3 text-ink-secondary select-text">
            {limit.said}
          </pre>
        </div>
      ))}
    </div>
  );
}

function named(providers: ProviderHealth[]) {
  return providers.map((provider) => provider.label).join(", ");
}
