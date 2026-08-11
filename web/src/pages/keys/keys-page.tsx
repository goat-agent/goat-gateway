import { useState } from "react";
import { Page } from "../page";
import { Panel, PanelHead } from "@/shared/ui/panel";
import { Nothing } from "@/shared/ui/nothing";
import { Button } from "@/shared/ui/button";
import { Badge } from "@/shared/ui/badge";
import { Body, Head, Row, Table } from "@/shared/ui/table";
import { Dialog } from "@/shared/ui/dialog";
import { Field, Input, Select } from "@/shared/ui/field";
import { useResource } from "@/shared/api/use-resource";
import { drop, send } from "@/shared/api/client";
import type { Key, User } from "@/entities/types";
import { ago, day } from "@/shared/lib/format";

export function KeysPage() {
  const users = useResource<{ users: User[] }>("/api/users");
  const keys = useResource<{ keys: Key[] }>("/api/keys");

  const [issuing, setIssuing] = useState(false);
  const [minted, setMinted] = useState<string>();
  const [label, setLabel] = useState("");
  const [owner, setOwner] = useState("");
  const [newUser, setNewUser] = useState("");

  const people = users.data?.users ?? [];
  const rows = (keys.data?.keys ?? []).filter((key) => key.revoked_at === null);

  return (
    <Page
      title="Keys"
      note="what your clients present to this gateway"
      aside={
        <Button
          tone="primary"
          disabled={people.length === 0}
          onClick={() => {
            setOwner(people[0]?.id ?? "");
            setIssuing(true);
          }}
        >
          Issue a key
        </Button>
      }
    >
      <Dialog
        open={issuing}
        onClose={() => {
          setIssuing(false);
          setMinted(undefined);
          setLabel("");
        }}
        title={minted ? "Copy this now" : "Issue a key"}
      >
        {minted ? (
          <div className="flex flex-col gap-3">
            <p className="m-0 text-small text-ink-secondary">
              This is the only time the key is shown. Only its hash is kept.
            </p>
            <code className="block rounded-sm border border-line bg-base p-2 font-mono text-code break-all">
              {minted}
            </code>
            <Button
              tone="primary"
              onClick={() => {
                setIssuing(false);
                setMinted(undefined);
                setLabel("");
              }}
            >
              Done
            </Button>
          </div>
        ) : (
          <form
            className="flex flex-col gap-3"
            onSubmit={(event) => {
              event.preventDefault();
              send<{ key: string }>("/api/keys", { user_id: owner, label: label.trim() }).then(
                (issued) => {
                  setMinted(issued.key);
                  keys.reload();
                },
              );
            }}
          >
            <Field label="For">
              <Select value={owner} onChange={(event) => setOwner(event.target.value)}>
                {people.map((person) => (
                  <option key={person.id} value={person.id}>
                    {person.name}
                  </option>
                ))}
              </Select>
            </Field>
            <Field label="Label" hint="So you can tell your machines apart later.">
              <Input
                autoFocus
                value={label}
                placeholder="laptop"
                onChange={(event) => setLabel(event.target.value)}
              />
            </Field>
            <Button tone="primary" type="submit" disabled={label.trim() === ""}>
              Issue
            </Button>
          </form>
        )}
      </Dialog>

      <Panel>
        <PanelHead title="People" note="requests are attributed to whoever's key was used">
          <form
            className="flex gap-1.5"
            onSubmit={(event) => {
              event.preventDefault();
              send("/api/users", { name: newUser.trim() }).then(() => {
                setNewUser("");
                users.reload();
              });
            }}
          >
            <Input
              className="h-7 w-40 text-micro"
              value={newUser}
              placeholder="add a person"
              onChange={(event) => setNewUser(event.target.value)}
            />
            <Button size="small" type="submit" disabled={newUser.trim() === ""}>
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
                    {rows.filter((key) => key.user_id === person.id).length}
                  </td>
                  <td className="text-ink-secondary">{day(person.created_at)}</td>
                  <td className="text-right">
                    <Button
                      tone="grave"
                      size="small"
                      onClick={() => {
                        if (!confirm(`Remove ${person.name}? Their keys stop working.`)) return;
                        drop(`/api/users/${person.id}`).then(() => {
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
        {rows.length === 0 ? (
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
              {rows.map((key) => (
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
                        drop(`/api/keys/${key.id}`).then(keys.reload);
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
