import { useState } from "react";
import { KeyRound, Plug, Plus, Trash2 } from "lucide-react";
import {
  Badge,
  Body,
  Button,
  Detail,
  Dialog,
  GroupRow,
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
import { drop, useResource } from "@/shared/api";
import { useCommand } from "@/shared/model/commands";
import { useRowKeys } from "@/shared/lib/use-row-keys";
import { useSelection } from "@/shared/lib/use-selection";
import { useMe, type Key, type User } from "@/entities/user";
import { ago, day } from "@/shared/lib/format";
import { ConnectGuide, IssueKey } from "@/features/issue-key";

const COLUMNS = 5;

export function KeysPage() {
  const me = useMe((store) => store.me);
  const admin = me?.admin ?? false;
  const users = useResource<{ users: User[] }>(admin ? "/api/users" : undefined);
  const keys = useResource<{ keys: Key[] }>("/api/keys");
  const { selected, select, close } = useSelection("/keys");

  const [issuing, setIssuing] = useState(false);
  const [guiding, setGuiding] = useState(false);

  const people = users.data?.users ?? [];
  const live = (keys.data?.keys ?? []).filter((key) => key.revoked_at === null);
  const holders = [...new Set(live.map((key) => key.user_name))];
  const ordered = holders.flatMap((holder) => live.filter((key) => key.user_name === holder));
  const blocked = admin && people.length === 0;

  useRowKeys({ ids: ordered.map((key) => key.id), selected, select, close });
  const shown = live.find((key) => key.id === selected);

  useCommand(
    { id: "keys/issue", label: "Issue a key", group: "Keys", icon: KeyRound, perform: () => setIssuing(true) },
    !blocked,
  );
  useCommand({
    id: "keys/connect",
    label: "How to connect a client",
    group: "Keys",
    icon: Plug,
    perform: () => setGuiding(true),
  });

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
        close();
        keys.reload();
      })
      .catch((error: Error) =>
        toast(`Could not revoke ${key.label}`, { says: error.message, tone: "critical" }),
      );
  };

  return (
    <Page
      title="Keys"
      count={keys.data ? live.length : undefined}
      refused={users.error ?? keys.error}
      scroll={false}
      actions={
        <>
          <Button onClick={() => setGuiding(true)}>
            <Plug strokeWidth={1.75} />
            How to connect
          </Button>
          <Button tone="primary" disabled={blocked} onClick={() => setIssuing(true)}>
            <Plus strokeWidth={2} />
            Issue a key
          </Button>
        </>
      }
    >
      <IssueKey
        open={issuing}
        people={admin ? people : []}
        onClose={() => setIssuing(false)}
        onIssued={keys.reload}
      />
      <Dialog
        open={guiding}
        onClose={() => setGuiding(false)}
        width="wide"
        title="Connect a client"
        description="Put a key from this page where the snippet says gwk_…"
      >
        <ConnectGuide />
      </Dialog>

      <Split
        label="Key"
        list={
          live.length === 0 ? (
            keys.data ? (
              <Nothing
                icon={KeyRound}
                says={
                  blocked
                    ? "Nobody is registered yet. Add a person on the People page, or say who you are above, then issue a key."
                    : "No key is live. Issue one for each program you point at this gateway."
                }
                offers={
                  blocked ? undefined : (
                    <Button tone="primary" size="medium" onClick={() => setIssuing(true)}>
                      Issue a key
                    </Button>
                  )
                }
              />
            ) : null
          ) : (
            <Table sticky className="table-fixed">
              <Head>
                <Row>
                  <th className="w-[46px]" aria-label="Key" />
                  <th>Label</th>
                  <th className="w-40">Prefix</th>
                  <th className="w-32">Last used</th>
                  <th className="w-28">Issued</th>
                </Row>
              </Head>
              {holders.map((holder) => {
                const theirs = live.filter((key) => key.user_name === holder);
                return (
                  <Body key={holder}>
                    {admin ? <GroupRow span={COLUMNS} title={holder} count={theirs.length} /> : null}
                    {theirs.map((key) => (
                      <Row
                        key={key.id}
                        data-row={key.id}
                        selected={key.id === selected}
                        className={rowHover}
                        onClick={() => select(key.id)}
                      >
                        <td>
                          <KeyRound className="size-3.5 text-ink-muted" strokeWidth={1.75} />
                        </td>
                        <td className="truncate font-medium text-ink">{key.label}</td>
                        <td className="mono truncate text-ink-secondary">
                          {key.prefix}…
                        </td>
                        <td className="truncate text-ink-secondary">
                          {key.last_used_at ? ago(key.last_used_at) : <Badge>never</Badge>}
                        </td>
                        <td className="text-ink-secondary">{day(key.created_at)}</td>
                      </Row>
                    ))}
                  </Body>
                );
              })}
            </Table>
          )
        }
        inspector={
          selected ? (
            shown ? (
              <Inspector
                kicker={`Key of ${shown.user_name}`}
                title={shown.label}
                subtitle={<span className="mono">{shown.prefix}…</span>}
                actions={
                  <Button onClick={() => void revoke(shown)}>
                    <Trash2 strokeWidth={1.75} />
                    Revoke
                  </Button>
                }
                onClose={close}
              >
                <Detail>
                  <Properties>
                    <Property name="Belongs to" value={shown.user_name} />
                    <Property name="Issued" value={day(shown.created_at)} />
                    <Property
                      name="Last used"
                      value={shown.last_used_at ? ago(shown.last_used_at) : "never"}
                    />
                  </Properties>
                </Detail>
              </Inspector>
            ) : (
              <Inspector kicker="Key" title="Not found" onClose={close}>
                {keys.data ? <Nothing says="No live key has this id. It may have been revoked." /> : null}
              </Inspector>
            )
          ) : undefined
        }
      />
    </Page>
  );
}
