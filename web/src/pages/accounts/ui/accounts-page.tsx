import { useCallback, useState } from "react";
import { Ellipsis, Plus, PlugZap } from "lucide-react";
import {
  Body,
  Button,
  ContextMenu,
  GroupRow,
  Head,
  Menu,
  Nothing,
  Page,
  Row,
  Split,
  Table,
  rowHover,
} from "@/shared/ui";
import { useHappenings, useResource } from "@/shared/api";
import { useCommand } from "@/shared/model/commands";
import { useRowKeys } from "@/shared/lib/use-row-keys";
import { useSelection } from "@/shared/lib/use-selection";
import { StateDot, type Account } from "@/entities/account";
import { QuotaGlance, quotaOf, type Overview } from "@/entities/provider";
import { useMe, type User } from "@/entities/user";
import { AccountMenuItems } from "@/features/set-account-state";
import { RegisterAccount } from "@/widgets/register-account";
import { AccountInspector, whoseOf } from "./account-inspector";

const COLUMNS = 5;

export function AccountsPage() {
  const [adding, setAdding] = useState(false);
  const accounts = useResource<{ accounts: Account[] }>("/api/accounts");
  const overview = useResource<Overview>("/api/overview");
  const me = useMe((store) => store.me);
  const admin = me?.admin ?? false;
  const users = useResource<{ users: User[] }>(admin ? "/api/users" : undefined);
  const { selected, select, close } = useSelection("/accounts");

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
  const health = overview.data?.providers;
  const labelOf = (provider: string) =>
    health?.find((entry) => entry.provider === provider)?.label ?? provider;
  const providers = [...new Set(rows.map((account) => account.provider))];
  const ordered = providers.flatMap((provider) =>
    rows.filter((account) => account.provider === provider),
  );

  useRowKeys({ ids: ordered.map((account) => account.name), selected, select, close });
  const shown = rows.find((account) => account.name === selected);

  return (
    <Page
      title="Accounts"
      count={accounts.data ? rows.length : undefined}
      refused={accounts.error}
      scroll={false}
      actions={
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

      <Split
        label="Account"
        list={
          rows.length === 0 ? (
            accounts.data ? (
              <Nothing
                icon={PlugZap}
                says="No account is registered. Add an API key, or sign in to a subscription, and this gateway starts serving."
                offers={
                  <Button tone="primary" size="medium" onClick={() => setAdding(true)}>
                    Add an account
                  </Button>
                }
              />
            ) : null
          ) : (
            <Table sticky className="table-fixed">
              <Head>
                <Row>
                  <th className="w-[46px]" aria-label="State" />
                  <th>Account</th>
                  <th className="w-36">Whose</th>
                  <th className="w-60">What is left</th>
                  <th className="w-14" aria-label="Actions" />
                </Row>
              </Head>
              {providers.map((provider) => {
                const held = rows.filter((account) => account.provider === provider);
                return (
                  <Body key={provider}>
                    <GroupRow span={COLUMNS} title={labelOf(provider)} count={held.length} />
                    {held.map((account) => {
                      const quota = quotaOf(health, account.provider, account.name);
                      return (
                        <ContextMenu
                          key={account.name}
                          render={
                            <Row
                              data-row={account.name}
                              selected={account.name === selected}
                              className={`group ${rowHover}`}
                              onClick={() => select(account.name)}
                            />
                          }
                          menu={<AccountMenuItems account={account} onChanged={reloadAccounts} />}
                        >
                          <td>
                            <StateDot state={account.state} until={account.cooldown_until} now={now} />
                          </td>
                          <td className="truncate">
                            <span className="font-medium text-ink">{account.name}</span>
                            <span className="ml-2 text-mini text-ink-muted">
                              {account.credential_kind === "api_key" ? "API key" : "plan"}
                            </span>
                          </td>
                          <td className="truncate text-ink-secondary">
                            {whoseOf(account, me?.person?.id)}
                          </td>
                          <td>
                            <QuotaGlance quota={quota} account={account.name} now={now} resets />
                          </td>
                          <td className="text-right" onClick={(event) => event.stopPropagation()}>
                            <Menu
                              trigger={
                                <Button
                                  tone="ghost"
                                  size="icon"
                                  aria-label={`More for ${account.name}`}
                                  className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100 data-[popup-open]:opacity-100"
                                >
                                  <Ellipsis strokeWidth={1.75} />
                                </Button>
                              }
                            >
                              <AccountMenuItems account={account} onChanged={reloadAccounts} />
                            </Menu>
                          </td>
                        </ContextMenu>
                      );
                    })}
                  </Body>
                );
              })}
            </Table>
          )
        }
        inspector={
          selected ? (
            <AccountInspector
              name={selected}
              account={shown}
              loaded={accounts.data !== undefined}
              providerLabel={shown ? labelOf(shown.provider) : ""}
              quota={shown ? quotaOf(health, shown.provider, shown.name) : undefined}
              now={now}
              people={users.data?.users ?? []}
              onChanged={reloadAccounts}
              onClose={close}
            />
          ) : undefined
        }
      />
    </Page>
  );
}
