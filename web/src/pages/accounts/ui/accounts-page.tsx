import { useCallback, useState } from "react";
import { Plus, PlugZap } from "lucide-react";
import {
  Body,
  Button,
  Cell,
  Count,
  ContextMenu,
  Head,
  Meter,
  Nothing,
  Page,
  Pill,
  Row,
  Table,
  rowHover,
} from "@/shared/ui";
import { useHappenings, useResource } from "@/shared/api";
import { useCommand } from "@/shared/model/commands";
import { StateBadge, type Account } from "@/entities/account";
import { worstWindow, type Overview } from "@/entities/provider";
import { useMe, type User } from "@/entities/user";
import { AccountActions, AccountMenuItems } from "@/features/set-account-state";
import { HandOver } from "@/features/hand-over-account";
import { RegisterAccount } from "@/widgets/register-account";
import { ago, UNKNOWN } from "@/shared/lib/format";

const COLUMNS = 6;

export function AccountsPage() {
  const [adding, setAdding] = useState(false);
  const accounts = useResource<{ accounts: Account[] }>("/api/accounts");
  const overview = useResource<Overview>("/api/overview");
  const me = useMe((store) => store.me);
  const admin = me?.admin ?? false;
  const users = useResource<{ users: User[] }>(admin ? "/api/users" : undefined);
  const people = users.data?.users ?? [];

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

  useCommand({
    id: "accounts/add",
    label: "Add an account",
    group: "Accounts",
    icon: Plus,
    perform: () => setAdding(true),
  });

  const rows = accounts.data?.accounts ?? [];
  const now = overview.data?.now ?? Date.now();
  const limitsOf = (name: string) =>
    overview.data?.providers
      .flatMap((provider) => provider.limits)
      .find((limit) => limit.account === name);
  const labelOf = (provider: string) =>
    overview.data?.providers.find((entry) => entry.provider === provider)?.label ?? provider;

  const providers = [...new Set(rows.map((account) => account.provider))];

  return (
    <Page
      title="Accounts"
      note="each request goes to one the caller may use and that can serve it"
      refused={accounts.error}
      bleed
      aside={
        <Button tone="primary" onClick={() => setAdding(true)}>
          <Plus strokeWidth={2} />
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

      {rows.length === 0 ? (
        <Nothing
          icon={PlugZap}
          says="No account is registered. Add an API key, or sign in to a subscription, and this gateway starts serving."
          offers={
            <Button tone="primary" size="medium" onClick={() => setAdding(true)}>
              Add an account
            </Button>
          }
        />
      ) : (
        <Table
          contained={false}
          className="[&_td:first-child]:pl-5 [&_td:last-child]:pr-3 [&_th:first-child]:pl-5 [&_th:last-child]:pr-3 [&_thead_th]:sticky [&_thead_th]:top-0 [&_thead_th]:z-[1] [&_thead_th]:border-t-0 [&_thead_th]:bg-panel"
        >
          <Head>
            <Row>
              <th>Account</th>
              <th className="w-44">Whose</th>
              <th className="w-40">State</th>
              <th className="w-72">Tightest window</th>
              <Count className="w-28">Observed</Count>
              <th className="w-32" />
            </Row>
          </Head>
          {providers.map((provider) => {
            const held = rows.filter((account) => account.provider === provider);
            return (
              <Body key={provider}>
                <tr>
                  <td
                    colSpan={COLUMNS}
                    className="h-[var(--group-height)]! bg-shade text-small select-none"
                  >
                    <span className="font-medium text-ink-title">{labelOf(provider)}</span>
                    <span className="numeric ml-2 text-ink-secondary">{held.length}</span>
                  </td>
                </tr>
                {held.map((account) => {
                  const limits = limitsOf(account.name);
                  const worst = limits ? worstWindow(limits.windows) : undefined;
                  return (
                    <ContextMenu
                      key={account.name}
                      render={<Row className={rowHover} />}
                      menu={<AccountMenuItems account={account} onChanged={reloadAccounts} />}
                    >
                      <td>
                        <span className="flex min-w-0 items-center gap-2.5">
                          <span className="truncate font-medium text-ink">{account.name}</span>
                          <Pill className="h-5 px-2 text-micro text-ink-secondary">
                            {account.credential_kind === "api_key" ? "API key" : "plan"}
                          </Pill>
                        </span>
                      </td>
                      <td>
                        {admin ? (
                          <HandOver account={account} people={people} onChanged={reloadAccounts} />
                        ) : (
                          <span className="truncate text-ink-secondary">
                            {account.owner === null
                              ? "Everyone"
                              : account.owner === me?.person?.id
                                ? "You"
                                : (account.owner_name ?? account.owner)}
                          </span>
                        )}
                      </td>
                      <td>
                        <StateBadge state={account.state} until={account.cooldown_until} now={now} />
                      </td>
                      <td>
                        {worst ? (
                          <span className="flex items-center gap-3">
                            <Meter
                              used={worst.used_percent}
                              label={`${worst.label} used on ${account.name}`}
                              className="w-28 shrink-0"
                            />
                            <span className="truncate text-mini text-ink-secondary">
                              of {worst.label} · {worst.scope ?? "every model"}
                            </span>
                          </span>
                        ) : (
                          <span className="text-ink-muted">{UNKNOWN}</span>
                        )}
                      </td>
                      <Cell className="text-ink-secondary">{ago(limits?.observed_at, now)}</Cell>
                      <td className="text-right">
                        <AccountActions account={account} onChanged={reloadAccounts} />
                      </td>
                    </ContextMenu>
                  );
                })}
              </Body>
            );
          })}
        </Table>
      )}
    </Page>
  );
}
