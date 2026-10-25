import { useState } from "react";
import { Link } from "react-router-dom";
import { Link2, Shield, ShieldOff, Trash2, UserPlus, Users } from "lucide-react";
import {
  Avatar,
  Badge,
  Body,
  Button,
  Detail,
  Dialog,
  Head,
  Inspector,
  Nothing,
  Page,
  Properties,
  Property,
  Row,
  Split,
  Table,
  confirmed,
  rowHover,
  toast,
} from "@/shared/ui";
import { drop, send, tell, useResource } from "@/shared/api";
import { useCommand } from "@/shared/model/commands";
import { useRowKeys } from "@/shared/lib/use-row-keys";
import { useSelection } from "@/shared/lib/use-selection";
import { ago, day } from "@/shared/lib/format";
import { StateDot, stateWords, type Account } from "@/entities/account";
import { useMe, type Key, type User } from "@/entities/user";
import { InviteLink, InvitePerson, inviteLink } from "@/features/invite-person";

export function PeoplePage() {
  const users = useResource<{ users: User[] }>("/api/users");
  const keys = useResource<{ keys: Key[] }>("/api/keys");
  const accounts = useResource<{ accounts: Account[] }>("/api/accounts");
  const me = useMe((store) => store.me);
  const [adding, setAdding] = useState(false);
  const [reissued, setReissued] = useState<{ name: string; link: string }>();
  const { selected, select, close } = useSelection("/people");

  useCommand({
    id: "people/add",
    label: "Add a person",
    group: "People",
    icon: UserPlus,
    perform: () => setAdding(true),
  });

  const people = users.data?.users ?? [];
  const live = (keys.data?.keys ?? []).filter((key) => key.revoked_at === null);
  const owned = accounts.data?.accounts ?? [];
  useRowKeys({ ids: people.map((person) => person.id), selected, select, close });
  const shown = people.find((person) => person.id === selected);

  const reinvite = (person: User) =>
    send<{ invite: string }>(`/api/users/${person.id}/invite`, {})
      .then((answer) => setReissued({ name: person.name, link: inviteLink(answer.invite) }))
      .catch((error: Error) =>
        toast(`Could not make a link for ${person.name}`, { says: error.message, tone: "critical" }),
      );

  const promote = (person: User, role: "admin" | "member") =>
    tell(`/api/users/${person.id}/role`, { role })
      .then(() => {
        toast(role === "admin" ? `${person.name} is an admin` : `${person.name} is a member`);
        users.reload();
      })
      .catch((error: Error) =>
        toast(`Could not change ${person.name}`, { says: error.message, tone: "critical" }),
      );

  const remove = async (person: User) => {
    const yes = await confirmed({
      title: `Remove ${person.name}?`,
      says: "Their keys stop working, and the plans they signed in to are removed with them.",
      action: "Remove person",
    });
    if (!yes) return;
    drop(`/api/users/${person.id}`)
      .then(() => {
        toast(`Removed ${person.name}`);
        close();
        users.reload();
        keys.reload();
        accounts.reload();
      })
      .catch((error: Error) =>
        toast(`Could not remove ${person.name}`, { says: error.message, tone: "critical" }),
      );
  };

  const yourself = shown !== undefined && me?.person?.id === shown.id;
  const theirKeys = live.filter((key) => key.user_id === selected);
  const theirPlans = owned.filter((account) => account.owner === selected);

  return (
    <Page
      title="People"
      count={users.data ? people.length : undefined}
      refused={users.error}
      scroll={false}
      actions={
        <Button tone="primary" onClick={() => setAdding(true)}>
          <UserPlus strokeWidth={1.75} />
          Add a person
        </Button>
      }
    >
      <InvitePerson open={adding} onClose={() => setAdding(false)} onInvited={users.reload} />
      <Dialog
        open={reissued !== undefined}
        onClose={() => setReissued(undefined)}
        title={reissued ? `A new link for ${reissued.name}` : ""}
        description="Their old link, if unused, keeps working until it expires."
      >
        {reissued ? <InviteLink link={reissued.link} /> : null}
      </Dialog>

      <Split
        label="Person"
        list={
          people.length === 0 ? (
            users.data ? (
              <Nothing icon={Users} says="Nobody is registered yet. Add a person and send them their link." />
            ) : null
          ) : (
            <Table sticky className="table-fixed">
              <Head>
                <Row>
                  <th>Name</th>
                  <th className="w-28">Role</th>
                  <th className="hidden w-44 @3xl:table-cell">Sign-in</th>
                  <th className="w-20">Keys</th>
                  <th className="hidden w-28 @4xl:table-cell">Since</th>
                </Row>
              </Head>
              <Body>
                {people.map((person) => (
                  <Row
                    key={person.id}
                    data-row={person.id}
                    selected={person.id === selected}
                    className={rowHover}
                    onClick={() => select(person.id)}
                  >
                    <td className="truncate">
                      <span className="flex min-w-0 items-center gap-2.5">
                        <Avatar name={person.name} />
                        <span className="truncate font-medium text-ink">{person.name}</span>
                        {me?.person?.id === person.id ? (
                          <span className="text-mini text-ink-muted">you</span>
                        ) : null}
                      </span>
                    </td>
                    <td className="text-ink-secondary">{person.role === "admin" ? "Admin" : "Member"}</td>
                    <td className="hidden @3xl:table-cell">
                      {person.can_sign_in ? (
                        <span className="text-ink-secondary">password set</span>
                      ) : (
                        <Badge tone="warning">invited, not yet in</Badge>
                      )}
                    </td>
                    <td className="numeric text-ink-secondary">
                      {live.filter((key) => key.user_id === person.id).length}
                    </td>
                    <td className="hidden text-ink-secondary @4xl:table-cell">{day(person.created_at)}</td>
                  </Row>
                ))}
              </Body>
            </Table>
          )
        }
        inspector={
          selected ? (
            shown ? (
              <Inspector
                kicker="Person"
                title={shown.name}
                subtitle={
                  <>
                    <Badge tone={shown.role === "admin" ? "info" : "neutral"}>
                      {shown.role === "admin" ? "Admin" : "Member"}
                    </Badge>
                    <span>{shown.can_sign_in ? "password set" : "invited, not yet in"}</span>
                    {yourself ? <span>· you</span> : null}
                  </>
                }
                actions={
                  <>
                    <Button onClick={() => void reinvite(shown)}>
                      <Link2 strokeWidth={1.75} />
                      {shown.can_sign_in ? "Password reset link" : "New invite link"}
                    </Button>
                    {shown.role === "admin" ? (
                      <Button disabled={yourself} onClick={() => void promote(shown, "member")}>
                        <ShieldOff strokeWidth={1.75} />
                        Make a member
                      </Button>
                    ) : (
                      <Button onClick={() => void promote(shown, "admin")}>
                        <Shield strokeWidth={1.75} />
                        Make an admin
                      </Button>
                    )}
                    <Button
                      size="icon"
                      disabled={yourself}
                      aria-label={`Remove ${shown.name}`}
                      onClick={() => void remove(shown)}
                    >
                      <Trash2 strokeWidth={1.75} />
                    </Button>
                  </>
                }
                onClose={close}
              >
                <Detail>
                  <Properties>
                    <Property name="Role" value={shown.role === "admin" ? "Sees and manages everything" : "Sees their own and what is shared"} />
                    <Property name="Since" value={day(shown.created_at)} />
                  </Properties>
                </Detail>
                <Detail flush title={`Plans ${theirPlans.length}`}>
                  {theirPlans.length === 0 ? (
                    <p className="m-0 px-5 text-small text-ink-secondary">
                      No plan belongs to them. A plan they sign in to is theirs alone.
                    </p>
                  ) : (
                    <div className="flex flex-col">
                      {theirPlans.map((account) => (
                        <Link
                          key={account.name}
                          to={`/accounts/${encodeURIComponent(account.name)}`}
                          className="hover-fade flex h-[var(--row-compact)] items-center gap-3 px-5 text-small hover:bg-panel-hover"
                        >
                          <StateDot state={account.state} until={account.cooldown_until} now={Date.now()} />
                          <span className="min-w-0 flex-1 truncate text-ink">{account.name}</span>
                          <span className="text-mini text-ink-secondary">
                            {stateWords(account.state, account.cooldown_until, Date.now())}
                          </span>
                        </Link>
                      ))}
                    </div>
                  )}
                </Detail>
                <Detail flush title={`Keys ${theirKeys.length}`}>
                  {theirKeys.length === 0 ? (
                    <p className="m-0 px-5 text-small text-ink-secondary">No live key.</p>
                  ) : (
                    <div className="flex flex-col">
                      {theirKeys.map((key) => (
                        <Link
                          key={key.id}
                          to={`/keys/${encodeURIComponent(key.id)}`}
                          className="hover-fade flex h-[var(--row-compact)] items-center gap-3 px-5 text-small hover:bg-panel-hover"
                        >
                          <span className="min-w-0 flex-1 truncate text-ink">{key.label}</span>
                          <span className="text-mini text-ink-secondary">
                            {key.last_used_at ? `used ${ago(key.last_used_at)}` : "never used"}
                          </span>
                        </Link>
                      ))}
                    </div>
                  )}
                </Detail>
              </Inspector>
            ) : (
              <Inspector kicker="Person" title="Not found" onClose={close}>
                {users.data ? <Nothing says="Nobody with this id is registered." /> : null}
              </Inspector>
            )
          ) : undefined
        }
      />
    </Page>
  );
}
