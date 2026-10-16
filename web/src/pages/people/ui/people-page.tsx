import { useState } from "react";
import { Link2, MoreHorizontal, Shield, ShieldOff, Trash2, UserPlus, Users } from "lucide-react";
import {
  Badge,
  Body,
  Button,
  Dialog,
  Head,
  Menu,
  MenuItem,
  MenuSeparator,
  Nothing,
  Page,
  Row,
  Table,
  confirmed,
  rowHover,
  toast,
} from "@/shared/ui";
import { drop, send, tell, useResource } from "@/shared/api";
import { useCommand } from "@/shared/model/commands";
import { useMe, type Key, type User } from "@/entities/user";
import { day } from "@/shared/lib/format";
import { InviteLink, InvitePerson, inviteLink } from "@/features/invite-person";

const LIST =
  "[&_td:first-child]:pl-5 [&_td:last-child]:pr-3 [&_th:first-child]:pl-5 [&_th:last-child]:pr-3 [&_thead_th]:border-t-0";

export function PeoplePage() {
  const users = useResource<{ users: User[] }>("/api/users");
  const keys = useResource<{ keys: Key[] }>("/api/keys");
  const me = useMe((store) => store.me);
  const [adding, setAdding] = useState(false);
  const [reissued, setReissued] = useState<{ name: string; link: string }>();

  useCommand({
    id: "people/add",
    label: "Add a person",
    group: "People",
    icon: UserPlus,
    perform: () => setAdding(true),
  });

  const people = users.data?.users ?? [];
  const live = (keys.data?.keys ?? []).filter((key) => key.revoked_at === null);

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
        users.reload();
        keys.reload();
      })
      .catch((error: Error) =>
        toast(`Could not remove ${person.name}`, { says: error.message, tone: "critical" }),
      );
  };

  return (
    <Page
      title="People"
      note="who can sign in, and whose plans are whose"
      refused={users.error}
      bleed
      aside={
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

      {people.length === 0 ? (
        <Nothing
          icon={Users}
          says="Nobody is registered yet. Add a person and send them their link."
        />
      ) : (
        <Table contained={false} className={LIST}>
          <Head>
            <Row>
              <th>Name</th>
              <th className="w-32">Role</th>
              <th className="w-44">Sign-in</th>
              <th className="w-24">Keys</th>
              <th className="w-36">Since</th>
              <th className="w-12" />
            </Row>
          </Head>
          <Body>
            {people.map((person) => {
              const yourself = me?.person?.id === person.id;
              return (
                <Row key={person.id} className={`group ${rowHover}`}>
                  <td className="font-medium text-ink">
                    {person.name}
                    {yourself ? <span className="ml-2 text-ink-secondary">you</span> : null}
                  </td>
                  <td>
                    <Badge tone={person.role === "admin" ? "info" : "neutral"}>
                      {person.role === "admin" ? "Admin" : "Member"}
                    </Badge>
                  </td>
                  <td>
                    {person.can_sign_in ? (
                      <span className="text-ink-secondary">password set</span>
                    ) : (
                      <Badge tone="warning">invited, not yet in</Badge>
                    )}
                  </td>
                  <td className="numeric text-ink-secondary">
                    {live.filter((key) => key.user_id === person.id).length}
                  </td>
                  <td className="text-ink-secondary">{day(person.created_at)}</td>
                  <td className="text-right">
                    <Menu
                      trigger={
                        <Button tone="ghost" size="icon" aria-label={`Actions for ${person.name}`}>
                          <MoreHorizontal strokeWidth={1.75} />
                        </Button>
                      }
                    >
                      <MenuItem icon={Link2} onClick={() => void reinvite(person)}>
                        {person.can_sign_in ? "Link to reset their password" : "A new invite link"}
                      </MenuItem>
                      {person.role === "admin" ? (
                        <MenuItem
                          icon={ShieldOff}
                          disabled={yourself}
                          onClick={() => void promote(person, "member")}
                        >
                          Make a member
                        </MenuItem>
                      ) : (
                        <MenuItem icon={Shield} onClick={() => void promote(person, "admin")}>
                          Make an admin
                        </MenuItem>
                      )}
                      <MenuSeparator />
                      <MenuItem
                        icon={Trash2}
                        danger
                        disabled={yourself}
                        onClick={() => void remove(person)}
                      >
                        Remove
                      </MenuItem>
                    </Menu>
                  </td>
                </Row>
              );
            })}
          </Body>
        </Table>
      )}
    </Page>
  );
}
