import { useState } from "react";
import { KeyRound, Plus, Trash2, UserPlus, Users } from "lucide-react";
import {
  Badge,
  Body,
  Button,
  Group,
  Head,
  Input,
  Nothing,
  Page,
  Row,
  Table,
  confirmed,
  rowHover,
  toast,
} from "@/shared/ui";
import { drop, tell, useResource } from "@/shared/api";
import { useCommand } from "@/shared/model/commands";
import type { Key, User } from "@/entities/user";
import { ago, day, UNKNOWN } from "@/shared/lib/format";
import { IssueKey } from "@/features/issue-key";

const LIST =
  "[&_td:first-child]:pl-5 [&_td:last-child]:pr-3 [&_th:first-child]:pl-5 [&_th:last-child]:pr-3 [&_thead_th]:border-t-0";

const REVEALED =
  "opacity-0 group-hover:opacity-100 focus-visible:opacity-100 group-data-[popup-open]:opacity-100";

export function KeysPage() {
  const users = useResource<{ users: User[] }>("/api/users");
  const keys = useResource<{ keys: Key[] }>("/api/keys");

  const [issuing, setIssuing] = useState(false);
  const [adding, setAdding] = useState("");

  const people = users.data?.users ?? [];
  const live = (keys.data?.keys ?? []).filter((key) => key.revoked_at === null);

  useCommand(
    { id: "keys/issue", label: "Issue a key", group: "Keys", icon: KeyRound, perform: () => setIssuing(true) },
    people.length > 0,
  );

  const removePerson = async (person: User) => {
    const yes = await confirmed({
      title: `Remove ${person.name}?`,
      says: "Their keys stop working.",
      action: "Remove person",
    });
    if (!yes) return;
    drop(`/api/users/${person.id}`)
      .then(() => {
        toast(`Removed ${person.name}`, { says: "Their keys no longer open this gateway." });
        users.reload();
        keys.reload();
      })
      .catch((error: Error) =>
        toast(`Could not remove ${person.name}`, { says: error.message, tone: "critical" }),
      );
  };

  const revoke = async (key: Key) => {
    const yes = await confirmed({
      title: `Revoke ${key.label}?`,
      says: "Anything using it stops working.",
      action: "Revoke key",
    });
    if (!yes) return;
    drop(`/api/keys/${key.id}`)
      .then(() => {
        toast(`Revoked ${key.label}`, { says: "It no longer opens this gateway." });
        keys.reload();
      })
      .catch((error: Error) =>
        toast(`Could not revoke ${key.label}`, { says: error.message, tone: "critical" }),
      );
  };

  return (
    <Page
      title="Keys"
      note="what your clients present to this gateway"
      refused={users.error ?? keys.error}
      bleed
      aside={
        <Button tone="primary" disabled={people.length === 0} onClick={() => setIssuing(true)}>
          <Plus strokeWidth={2} />
          Issue a key
        </Button>
      }
    >
      <IssueKey
        open={issuing}
        people={people}
        onClose={() => setIssuing(false)}
        onIssued={keys.reload}
      />

      <Group title="People" count={people.length}>
        <form
          className="flex items-center gap-1.5"
          onSubmit={(event) => {
            event.preventDefault();
            const name = adding.trim();
            tell("/api/users", { name })
              .then(() => {
                setAdding("");
                users.reload();
                toast(`Added ${name}`, { says: "Issue them a key next.", tone: "good" });
              })
              .catch((error: Error) =>
                toast(`Could not add ${name}`, { says: error.message, tone: "critical" }),
              );
          }}
        >
          <Input
            lead={<UserPlus strokeWidth={1.75} />}
            className="h-7 w-48"
            value={adding}
            placeholder="Add a person"
            aria-label="Name of the person to add"
            onChange={(event) => setAdding(event.target.value)}
          />
          <Button type="submit" disabled={adding.trim() === ""}>
            Add
          </Button>
        </form>
      </Group>

      {people.length === 0 ? (
        <Nothing icon={Users} says="Nobody is registered yet. Add a person, then issue them a key." />
      ) : (
        <Table contained={false} className={LIST}>
          <Head>
            <Row>
              <th>Name</th>
              <th className="w-32">Keys</th>
              <th className="w-40">Since</th>
              <th className="w-28" />
            </Row>
          </Head>
          <Body>
            {people.map((person) => (
              <Row key={person.id} className={`group ${rowHover}`}>
                <td className="font-medium text-ink">{person.name}</td>
                <td className="numeric text-ink-secondary">
                  {live.filter((key) => key.user_id === person.id).length}
                </td>
                <td className="text-ink-secondary">{day(person.created_at)}</td>
                <td className="text-right">
                  <Button
                    tone="ghost"
                    size="small"
                    className={REVEALED}
                    onClick={() => void removePerson(person)}
                  >
                    <Trash2 strokeWidth={1.75} />
                    Remove
                  </Button>
                </td>
              </Row>
            ))}
          </Body>
        </Table>
      )}

      <Group title="Live keys" count={live.length} />

      {live.length === 0 ? (
        <Nothing
          icon={KeyRound}
          says="No key is live. Issue one and point a client at this gateway with it."
        />
      ) : (
        <Table contained={false} className={LIST}>
          <Head>
            <Row>
              <th>Label</th>
              <th className="w-48">Person</th>
              <th className="w-40">Prefix</th>
              <th className="w-40">Last used</th>
              <th className="w-28" />
            </Row>
          </Head>
          <Body>
            {live.map((key) => (
              <Row key={key.id} className={`group ${rowHover}`}>
                <td className="font-medium text-ink">{key.label}</td>
                <td className="text-ink-secondary">
                  {people.find((person) => person.id === key.user_id)?.name ?? UNKNOWN}
                </td>
                <td className="mono text-ink-secondary">{key.prefix}…</td>
                <td className="text-ink-secondary">
                  {key.last_used_at ? ago(key.last_used_at) : <Badge>never</Badge>}
                </td>
                <td className="text-right">
                  <Button
                    tone="ghost"
                    size="small"
                    className={REVEALED}
                    onClick={() => void revoke(key)}
                  >
                    <Trash2 strokeWidth={1.75} />
                    Revoke
                  </Button>
                </td>
              </Row>
            ))}
          </Body>
        </Table>
      )}
    </Page>
  );
}
