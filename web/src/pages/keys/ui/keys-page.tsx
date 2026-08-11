import { useState } from "react";
import { Badge, Body, Button, Head, Input, Nothing, Page, Panel, PanelHead, Row, Table } from "@/shared/ui";
import { drop, tell, useResource } from "@/shared/api";
import type { Key, User } from "@/entities/user";
import { ago, day } from "@/shared/lib/format";
import { IssueKey } from "@/features/issue-key";

export function KeysPage() {
  const users = useResource<{ users: User[] }>("/api/users");
  const keys = useResource<{ keys: Key[] }>("/api/keys");

  const [issuing, setIssuing] = useState(false);
  const [adding, setAdding] = useState("");

  const people = users.data?.users ?? [];
  const live = (keys.data?.keys ?? []).filter((key) => key.revoked_at === null);

  return (
    <Page
      title="Keys"
      note="what your clients present to this gateway"
      aside={
        <Button tone="primary" disabled={people.length === 0} onClick={() => setIssuing(true)}>
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

      <Panel>
        <PanelHead title="People" note="requests are attributed to whoever's key was used">
          <form
            className="flex gap-1.5"
            onSubmit={(event) => {
              event.preventDefault();
              void tell("/api/users", { name: adding.trim() }).then(() => {
                setAdding("");
                users.reload();
              });
            }}
          >
            <Input
              className="h-7 w-40 text-micro"
              value={adding}
              placeholder="add a person"
              onChange={(event) => setAdding(event.target.value)}
            />
            <Button size="small" type="submit" disabled={adding.trim() === ""}>
              Add
            </Button>
          </form>
        </PanelHead>
        {people.length === 0 ? (
          <Nothing says="Nobody is registered yet. Add a person, then issue them a key." />
        ) : (
          <Table>
            <Head>
              <Row>
                <th>Name</th>
                <th>Keys</th>
                <th>Since</th>
                <th />
              </Row>
            </Head>
            <Body>
              {people.map((person) => (
                <Row key={person.id}>
                  <td className="text-ink">{person.name}</td>
                  <td className="text-ink-secondary">
                    {live.filter((key) => key.user_id === person.id).length}
                  </td>
                  <td className="text-ink-secondary">{day(person.created_at)}</td>
                  <td className="text-right">
                    <Button
                      tone="grave"
                      size="small"
                      onClick={() => {
                        if (!confirm(`Remove ${person.name}? Their keys stop working.`)) return;
                        void drop(`/api/users/${person.id}`).then(() => {
                          users.reload();
                          keys.reload();
                        });
                      }}
                    >
                      Remove
                    </Button>
                  </td>
                </Row>
              ))}
            </Body>
          </Table>
        )}
      </Panel>

      <Panel>
        <PanelHead title="Live keys" />
        {live.length === 0 ? (
          <Nothing says="No key is live. Issue one and point a client at this gateway with it." />
        ) : (
          <Table>
            <Head>
              <Row>
                <th>Label</th>
                <th>Person</th>
                <th>Prefix</th>
                <th>Last used</th>
                <th />
              </Row>
            </Head>
            <Body>
              {live.map((key) => (
                <Row key={key.id}>
                  <td className="text-ink">{key.label}</td>
                  <td className="text-ink-secondary">
                    {people.find((person) => person.id === key.user_id)?.name ?? "—"}
                  </td>
                  <td className="numeric text-ink-secondary">{key.prefix}…</td>
                  <td className="text-ink-secondary">
                    {key.last_used_at ? ago(key.last_used_at) : <Badge>never</Badge>}
                  </td>
                  <td className="text-right">
                    <Button
                      tone="grave"
                      size="small"
                      onClick={() => {
                        if (!confirm(`Revoke ${key.label}? Anything using it stops working.`)) return;
                        void drop(`/api/keys/${key.id}`).then(keys.reload);
                      }}
                    >
                      Revoke
                    </Button>
                  </td>
                </Row>
              ))}
            </Body>
          </Table>
        )}
      </Panel>
    </Page>
  );
}
