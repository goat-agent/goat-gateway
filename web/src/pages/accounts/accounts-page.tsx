import { useCallback, useState } from "react";
import { Page } from "../page";
import { Panel, PanelHead } from "@/shared/ui/panel";
import { Nothing } from "@/shared/ui/nothing";
import { Button } from "@/shared/ui/button";
import { Badge } from "@/shared/ui/badge";
import { Body, Head, Row, Table } from "@/shared/ui/table";
import { AddAccount } from "@/features/add-account/add-account";
import { useResource } from "@/shared/api/use-resource";
import { useHappenings } from "@/shared/api/happenings";
import { drop, send } from "@/shared/api/client";
import type { Account, AccountState, Overview } from "@/entities/types";
import { ago, until, whole } from "@/shared/lib/format";
import { Link } from "react-router-dom";

const SAID: Record<AccountState, { text: string; tone: "good" | "warning" | "serious" | "neutral" }> = {
  active: { text: "usable", tone: "good" },
  rate_limited: { text: "rate limited", tone: "warning" },
  sign_in_expired: { text: "signed out", tone: "serious" },
  disabled: { text: "turned off", tone: "neutral" },
};

export function AccountsPage() {
  const [adding, setAdding] = useState(false);
  const accounts = useResource<{ accounts: Account[] }>("/api/accounts");
  const overview = useResource<Overview>("/api/overview");

  useHappenings(
    useCallback(
      (happening) => {
        if (happening.happened === "account_changed" || happening.happened === "limits_observed") {
          accounts.reload();
          overview.reload();
        }
      },
      [accounts, overview],
    ),
  );

  const rows = accounts.data?.accounts ?? [];
  const now = overview.data?.now ?? Date.now();

  const quota = (name: string) =>
    overview.data?.providers
      .flatMap((provider) => provider.limits)
      .find((limit) => limit.account === name);

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
          accounts.reload();
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
                <th>Worst window</th>
                <th>Observed</th>
                <th />
              </Row>
            </Head>
            <Body>
              {rows.map((account) => {
                const seen = quota(account.name);
                const worst = seen?.windows.reduce(
                  (held, window) => (held && held.used_percent >= window.used_percent ? held : window),
                  seen.windows[0],
                );
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
                          ? ` · ${until(account.cooldown_until, now)}`
                          : ""}
                      </Badge>
                    </td>
                    <td className="numeric text-ink-secondary">
                      {worst ? `${whole(worst.used_percent)} of ${worst.label}` : "—"}
                    </td>
                    <td className="numeric text-ink-muted">{ago(seen?.observed_at, now)}</td>
                    <td className="text-right">
                      <span className="flex justify-end gap-1">
                        <Link to={`/usage?account=${encodeURIComponent(account.name)}`}>
                          <Button tone="quiet" size="small">
                            Usage
                          </Button>
                        </Link>
                        {account.state === "disabled" ? (
                          <Button
                            size="small"
                            onClick={() =>
                              send(`/api/accounts/${encodeURIComponent(account.name)}/state`, {
                                state: "active",
                              }).then(accounts.reload)
                            }
                          >
                            Turn on
                          </Button>
                        ) : (
                          <Button
                            tone="quiet"
                            size="small"
                            onClick={() =>
                              send(`/api/accounts/${encodeURIComponent(account.name)}/state`, {
                                state: "disabled",
                              }).then(accounts.reload)
                            }
                          >
                            Turn off
                          </Button>
                        )}
                        <Button
                          tone="grave"
                          size="small"
                          onClick={() => {
                            if (!confirm(`Remove ${account.name}? Its credential is deleted.`)) return;
                            drop(`/api/accounts/${encodeURIComponent(account.name)}`).then(accounts.reload);
                          }}
                        >
                          Remove
                        </Button>
                      </span>
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
