import { useCallback } from "react";
import { Link } from "react-router-dom";
import { Detail, Inspector, Nothing, Properties, Property } from "@/shared/ui";
import { query, useHappenings, useResource } from "@/shared/api";
import { ago, day, within } from "@/shared/lib/format";
import { StateBadge, type Account } from "@/entities/account";
import { QuotaWindow, type Quota } from "@/entities/provider";
import { RequestLine, type Request } from "@/entities/request";
import { useMe, type User } from "@/entities/user";
import { AccountControls } from "@/features/set-account-state";
import { HandOver } from "@/features/hand-over-account";

export function whoseOf(account: Account, me: string | undefined) {
  if (account.owner === null) return "Everyone";
  if (account.owner === me) return "You";
  return account.owner_name ?? account.owner;
}

export function AccountInspector({
  name,
  account,
  loaded,
  providerLabel,
  quota,
  now,
  people,
  onChanged,
  onClose,
}: {
  name: string;
  account: Account | undefined;
  loaded: boolean;
  providerLabel: string;
  quota: Quota | undefined;
  now: number;
  people: readonly User[];
  onChanged: () => void;
  onClose: () => void;
}) {
  const me = useMe((store) => store.me);

  if (!account) {
    return (
      <Inspector kicker="Account" title={name} onClose={onClose}>
        {loaded ? <Nothing says="No account by this name is registered, or it is not yours to see." /> : null}
      </Inspector>
    );
  }

  return (
    <Inspector
      kicker={providerLabel}
      title={account.name}
      subtitle={
        <>
          <StateBadge state={account.state} until={account.cooldown_until} now={now} />
          <span>{account.credential_kind === "api_key" ? "API key" : "Plan sign-in"}</span>
        </>
      }
      actions={<AccountControls account={account} onChanged={onChanged} onRemoved={onClose} />}
      onClose={onClose}
    >
      {account.state !== "active" ? (
        <Detail>
          <p className="m-0 text-small text-ink-secondary">{whyIdle(account, now)}</p>
        </Detail>
      ) : null}

      <Detail>
        <Properties>
          <Property name="Provider" value={providerLabel} />
          <Property
            name="Whose"
            value={
              me?.admin ? (
                <HandOver account={account} people={people} onChanged={onChanged} />
              ) : (
                whoseOf(account, me?.person?.id)
              )
            }
          />
          <Property name="Added" value={day(account.created_at)} />
        </Properties>
      </Detail>

      <Detail
        title="What is left"
        aside={quota && "observed_at" in quota ? `seen ${ago(quota.observed_at, now)}` : undefined}
      >
        <Left quota={quota} account={account.name} now={now} />
      </Detail>

      <Latest account={account.name} now={now} />
    </Inspector>
  );
}

function Left({ quota, account, now }: { quota: Quota | undefined; account: string; now: number }) {
  if (!quota) return null;
  if (quota.seen === "never") {
    return <p className="m-0 text-small text-ink-muted">This provider does not report what is left.</p>;
  }
  if (quota.seen === "not_yet") {
    return (
      <p className="m-0 text-small text-ink-secondary">
        Not observed yet. It is read off the responses to real requests, so it fills in once this
        account serves one.
      </p>
    );
  }
  if (quota.seen === "unread") {
    return (
      <div className="flex flex-col gap-2">
        <p className="m-0 text-small text-ink-secondary">
          The provider answered, but this gateway has not been told where the numbers are in what it
          said. Add pointers under the provider's limits in config.toml.
        </p>
        <pre className="mono m-0 max-h-48 overflow-auto rounded-lg hairline border-line-subtle bg-base p-3 text-ink-secondary select-text">
          {quota.said}
        </pre>
      </div>
    );
  }
  return (
    <div className="flex flex-col gap-4">
      {quota.windows.map((window) => (
        <QuotaWindow key={`${window.label}-${window.scope ?? "all"}`} window={window} account={account} now={now} />
      ))}
    </div>
  );
}

function Latest({ account, now }: { account: string; now: number }) {
  const found = useResource<{ requests: Request[] }>(`/api/requests${query({ account, limit: 8 })}`);
  const reload = found.reload;
  useHappenings(
    useCallback(
      (happening) => {
        if (happening === "request_settled") reload();
      },
      [reload],
    ),
  );
  const requests = found.data?.requests ?? [];

  return (
    <Detail
      flush
      title="Latest requests"
      aside={
        requests.length > 0 ? (
          <Link
            to={`/requests${query({ account })}`}
            className="hover-fade text-ink-secondary hover:text-ink"
          >
            Show all
          </Link>
        ) : undefined
      }
    >
      {requests.length === 0 ? (
        <p className="m-0 px-5 text-small text-ink-secondary">
          {found.data ? "Nothing has gone through this account yet." : ""}
        </p>
      ) : (
        <div className="flex flex-col">
          {requests.map((request) => (
            <RequestLine key={request.id} request={request} now={now} showing="client" />
          ))}
        </div>
      )}
    </Detail>
  );
}

function whyIdle(account: Account, now: number) {
  if (account.state === "rate_limited") {
    return account.cooldown_until
      ? `The provider said it has no room. Nothing is routed to it for ${within(account.cooldown_until, now)}, then it is tried again on its own.`
      : "The provider said it has no room. Nothing is routed to it until it reports room again.";
  }
  if (account.state === "sign_in_expired") {
    return "The provider stopped accepting this sign-in, so nothing is routed to it. Its history stays.";
  }
  return "Turned off. Nothing is routed to it until it is turned back on.";
}
