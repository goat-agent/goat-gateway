import { useCallback, useState } from "react";
import { useHappenings, useResource } from "@/shared/api";
import { show } from "@/shared/lib";
import {
  Badge,
  Body,
  Button,
  Head,
  Nothing,
  Page,
  Panel,
  PanelHead,
  Row,
  Table,
} from "@/shared/ui";
import { SAID, type Account } from "@/entities/account";
import { useHealth, type Observed } from "@/entities/provider";
import { AddAccount } from "@/features/add-account";
import { AccountActions } from "@/features/set-account-state";

export function AccountsPage() {
  const [adding, setAdding] = useState(false);
  const accounts = useResource<{ accounts: Account[] }>("/api/accounts");
  const health = useHealth();

  const refresh = useCallback(() => {
    accounts.reload();
    health.reload();
  }, [accounts, health]);

  useHappenings(
    useCallback(
      (happening) => {
        if (happening.happened !== "request_opened") refresh();
      },
      [refresh],
    ),
  );

  const rows = accounts.data?.accounts ?? [];
  const now = health.data?.now ?? Date.now();
  const observed = health.data?.providers.flatMap((provider) => provider.limits) ?? [];

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
      <AddAccount
        open={adding}
        onClose={() => setAdding(false)}
        onAdded={() => {
          setAdding(false);
          refresh();
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
                <th>Fullest window</th>
                <th>Observed</th>
                <th />
              </Row>
            </Head>
            <Body>
              {rows.map((account) => {
                const seen = observed.find((entry) => entry.account === account.name);
                return (
                  <Row key={account.name} className="hover:bg-raised">
                    <td className="text-ink">{account.name}</td>
                    <td className="text-ink-secondary">{account.provider}</td>
                    <td className="text-ink-secondary">
                      {account.credential_kind === "api_key" ? "API key" : "sign-in"}
                    </td>
                    <td>
                      <Badge tone={SAID[account.state].tone}>
                        {SAID[account.state].text}
                        {account.state === "rate_limited" && account.cooldown_until
                          ? ` · ${show.until(account.cooldown_until, now)}`
                          : ""}
                      </Badge>
                    </td>
                    <td className="numeric text-ink-secondary">{fullest(seen)}</td>
                    <td className="numeric text-ink-muted">{show.ago(seen?.observed_at, now)}</td>
                    <td className="text-right">
                      <AccountActions account={account} onChanged={refresh} />
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

function fullest(seen: Observed | undefined) {
  const worst = seen?.windows.reduce(
    (held, window) => (held && held.used_percent >= window.used_percent ? held : window),
    seen.windows[0],
  );
  if (!worst) return show.UNKNOWN;
  return `${show.whole(worst.used_percent)} of ${worst.label}${worst.scope ? ` · ${worst.scope}` : ""}`;
}
