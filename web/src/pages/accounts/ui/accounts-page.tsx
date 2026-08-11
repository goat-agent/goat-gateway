import { useCallback, useState } from "react";
import { Body, Button, Cell, Count, Head, Nothing, Page, Panel, PanelHead, Row, Table } from "@/shared/ui";
import { useHappenings, useResource } from "@/shared/api";
import { StateBadge, type Account } from "@/entities/account";
import { worstWindow, type Overview } from "@/entities/provider";
import { AccountActions } from "@/features/set-account-state";
import { RegisterAccount } from "@/widgets/register-account";
import { ago, share, UNKNOWN } from "@/shared/lib/format";

export function AccountsPage() {
  const [adding, setAdding] = useState(false);
  const accounts = useResource<{ accounts: Account[] }>("/api/accounts");
  const overview = useResource<Overview>("/api/overview");

  const reloadAccounts = accounts.reload;
  const reloadOverview = overview.reload;
  useHappenings(
    useCallback(
      (happening) => {
        if (happening === "account_changed" || happening === "limits_observed") {
          reloadAccounts();
          reloadOverview();
        }
      },
      [reloadAccounts, reloadOverview],
    ),
  );

  const rows = accounts.data?.accounts ?? [];
  const now = overview.data?.now ?? Date.now();
  const limitsOf = (name: string) =>
    overview.data?.providers.flatMap((provider) => provider.limits).find((limit) => limit.account === name);

  return (
    <Page
      title="Accounts"
      note={accounts.error}
      aside={
        <Button tone="primary" onClick={() => setAdding(true)}>
          Add an account
        </Button>
      }
    >
      <RegisterAccount
        open={adding}
        onClose={() => setAdding(false)}
        onAdded={() => {
          setAdding(false);
          reloadAccounts();
        }}
      />

      <Panel>
        <PanelHead title="Registered" note="requests go to whichever one can serve them" />
        {rows.length === 0 ? (
          <Nothing
            says="No account is registered. Add an API key, or sign in to a subscription, and this gateway starts serving."
            offers={
              <Button tone="primary" onClick={() => setAdding(true)}>
                Add an account
              </Button>
            }
          />
        ) : (
          <Table>
            <Head>
              <Row>
                <th>Account</th>
                <th>Provider</th>
                <th>Credential</th>
                <th>State</th>
                <th>Tightest window</th>
                <Count>Observed</Count>
                <th />
              </Row>
            </Head>
            <Body>
              {rows.map((account) => {
                const limits = limitsOf(account.name);
                const worst = limits ? worstWindow(limits.windows) : undefined;
                return (
                  <Row key={account.name} className="hover:bg-raised">
                    <td className="text-ink">{account.name}</td>
                    <td className="text-ink-secondary">{account.provider}</td>
                    <td className="text-ink-secondary">
                      {account.credential_kind === "api_key" ? "API key" : "sign-in"}
                    </td>
                    <td>
                      <StateBadge state={account.state} until={account.cooldown_until} now={now} />
                    </td>
                    <td className="text-ink-secondary">
                      {worst ? (
                        <span className="numeric">
                          {share(worst.used_percent)} of {worst.label}
                          <span className="text-ink-muted"> · {worst.scope ?? "every model"}</span>
                        </span>
                      ) : (
                        UNKNOWN
                      )}
                    </td>
                    <Cell className="text-ink-muted">{ago(limits?.observed_at, now)}</Cell>
                    <td className="text-right">
                      <AccountActions account={account} onChanged={reloadAccounts} />
                    </td>
                  </Row>
                );
              })}
            </Body>
          </Table>
        )}
      </Panel>
    </Page>
  );
}
