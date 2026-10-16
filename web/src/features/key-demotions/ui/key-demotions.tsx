import { ArrowDownToLine, Ban, RotateCcw } from "lucide-react";
import { useState } from "react";
import { Body, Button, Dialog, Head, Nothing, Row, Select, Table, toast } from "@/shared/ui";
import { drop, tell, useResource } from "@/shared/api";
import type { Account } from "@/entities/account";
import type { Demotion, Key } from "@/entities/user";
import { within } from "@/shared/lib/format";

export function KeyDemotions({
  open,
  keyRow,
  onClose,
}: {
  open: boolean;
  keyRow: Key | undefined;
  onClose: () => void;
}) {
  const path = keyRow && open ? `/api/keys/${keyRow.id}/demotions` : undefined;
  const demotions = useResource<{ demotions: Demotion[] }>(path);
  const accounts = useResource<{ accounts: Account[] }>(open ? "/api/accounts" : undefined);
  const [chosen, setChosen] = useState("");

  const held = demotions.data?.demotions ?? [];
  const choices = (accounts.data?.accounts ?? [])
    .filter((account) => !held.some((demotion) => demotion.account === account.name))
    .map((account) => ({ value: account.name, label: account.name }));
  const picked = chosen || choices[0]?.value || "";

  const useAgain = (account: string) =>
    drop(`${path}/${encodeURIComponent(account)}`)
      .then(() => {
        toast(`${keyRow?.label} uses ${account} again`);
        demotions.reload();
      })
      .catch((error: Error) => toast("Could not change it", { says: error.message, tone: "critical" }));

  const keepOff = () =>
    tell(path ?? "", { account: picked })
      .then(() => {
        toast(`${keyRow?.label} stays off ${picked}`);
        setChosen("");
        demotions.reload();
      })
      .catch((error: Error) => toast("Could not change it", { says: error.message, tone: "critical" }));

  return (
    <Dialog
      open={open}
      onClose={onClose}
      width="wide"
      title={keyRow ? `Where ${keyRow.label} goes last` : ""}
      description="When a provider refuses this key's requests on an account, the key tries that account last for a while, and the account stays available to everything else. You can also keep a key off an account for good."
    >
      <div className="flex flex-col gap-4">
        {held.length === 0 ? (
          <Nothing
            icon={ArrowDownToLine}
            says="Nothing here. Every account this key may use is tried in the usual order."
          />
        ) : (
          <Table>
            <Head>
              <Row>
                <th>Account</th>
                <th>Why</th>
                <th className="w-28">Until</th>
                <th className="w-28" />
              </Row>
            </Head>
            <Body>
              {held.map((demotion) => (
                <Row key={demotion.account}>
                  <td className="font-medium text-ink">{demotion.account}</td>
                  <td className="max-w-72 truncate text-ink-secondary" title={demotion.cause}>
                    {demotion.cause}
                  </td>
                  <td className="text-ink-secondary">
                    {demotion.until === null ? "kept off" : within(demotion.until, Date.now())}
                  </td>
                  <td className="text-right">
                    <Button tone="ghost" size="small" onClick={() => void useAgain(demotion.account)}>
                      <RotateCcw strokeWidth={1.75} />
                      Use again
                    </Button>
                  </td>
                </Row>
              ))}
            </Body>
          </Table>
        )}
        {choices.length > 0 ? (
          <div className="flex items-center justify-end gap-2">
            <span className="text-small text-ink-secondary">Keep this key off</span>
            <Select
              label="Account to keep this key off"
              size="medium"
              value={picked}
              choices={choices}
              onChange={setChosen}
            />
            <Button size="medium" onClick={() => void keepOff()}>
              <Ban strokeWidth={1.75} />
              Keep off
            </Button>
          </div>
        ) : null}
      </div>
    </Dialog>
  );
}
