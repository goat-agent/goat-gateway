import { useState } from "react";
import { KeyRound, MoreHorizontal, Plug, Plus, Trash2 } from "lucide-react";
import {
  Badge,
  Body,
  Button,
  Dialog,
  Head,
  Menu,
  MenuItem,
  Nothing,
  Page,
  Row,
  Table,
  confirmed,
  rowHover,
  toast,
} from "@/shared/ui";
import { drop, useResource } from "@/shared/api";
import { useCommand } from "@/shared/model/commands";
import { useMe, type Key, type User } from "@/entities/user";
import { ago, day } from "@/shared/lib/format";
import { ConnectGuide, IssueKey } from "@/features/issue-key";

const COLUMNS = 5;

export function KeysPage() {
  const me = useMe((store) => store.me);
  const admin = me?.admin ?? false;
  const users = useResource<{ users: User[] }>(admin ? "/api/users" : undefined);
  const keys = useResource<{ keys: Key[] }>("/api/keys");

  const [issuing, setIssuing] = useState(false);
  const [guiding, setGuiding] = useState(false);

  const people = users.data?.users ?? [];
  const live = (keys.data?.keys ?? []).filter((key) => key.revoked_at === null);
  const holders = [...new Set(live.map((key) => key.user_name))];
  const blocked = admin && people.length === 0;

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
        keys.reload();
      })
      .catch((error: Error) =>
        toast(`Could not revoke ${key.label}`, { says: error.message, tone: "critical" }),
      );
  };

  return (
    <Page
      title="Keys"
      note={admin ? "what every program presents to this gateway" : "what your programs present to this gateway"}
      refused={users.error ?? keys.error}
      bleed
      aside={
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

      {live.length === 0 ? (
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
      ) : (
        <Table
          contained={false}
          className="[&_td:first-child]:pl-5 [&_td:last-child]:pr-3 [&_th:first-child]:pl-5 [&_th:last-child]:pr-3 [&_thead_th]:border-t-0"
        >
          <Head>
            <Row>
              <th>Label</th>
              <th className="w-40">Prefix</th>
              <th className="w-36">Last used</th>
              <th className="w-36">Issued</th>
              <th className="w-12" />
            </Row>
          </Head>
          {holders.map((holder) => (
            <Body key={holder}>
              {admin ? (
                <tr>
                  <td colSpan={COLUMNS} className="h-[var(--group-height)]! bg-shade text-small select-none">
                    <span className="font-medium text-ink-title">{holder}</span>
                    <span className="numeric ml-2 text-ink-secondary">
                      {live.filter((key) => key.user_name === holder).length}
                    </span>
                  </td>
                </tr>
              ) : null}
              {live
                .filter((key) => key.user_name === holder)
                .map((key) => (
                  <Row key={key.id} className={`group ${rowHover}`}>
                    <td className="font-medium text-ink">{key.label}</td>
                    <td className="mono text-ink-secondary">{key.prefix}…</td>
                    <td className="text-ink-secondary">
                      {key.last_used_at ? ago(key.last_used_at) : <Badge>never</Badge>}
                    </td>
                    <td className="text-ink-secondary">{day(key.created_at)}</td>
                    <td className="text-right">
                      <Menu
                        trigger={
                          <Button tone="ghost" size="icon" aria-label={`Actions for ${key.label}`}>
                            <MoreHorizontal strokeWidth={1.75} />
                          </Button>
                        }
                      >
                        <MenuItem icon={Trash2} danger onClick={() => void revoke(key)}>
                          Revoke
                        </MenuItem>
                      </Menu>
                    </td>
                  </Row>
                ))}
            </Body>
          ))}
        </Table>
      )}
    </Page>
  );
}
