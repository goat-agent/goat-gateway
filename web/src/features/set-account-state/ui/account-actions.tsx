import { ChartColumn, Ellipsis, Power, PowerOff, RotateCcw, Trash2, Zap } from "lucide-react";
import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { drop, send } from "@/shared/api";
import { duration } from "@/shared/lib/format";
import { Button, Menu, MenuItem, MenuSeparator, confirmed, toast } from "@/shared/ui";
import type { Account } from "@/entities/account";

type Reached = {
  ok: boolean;
  status: number;
  model: string | null;
  said: string | null;
  took_ms: number;
};

type Answer = { good: boolean; says: string; why: string };

function read(reached: Reached): Answer {
  return reached.ok
    ? {
        good: true,
        says: `answered in ${duration(reached.took_ms)}`,
        why: `${reached.model ?? "the first declared model"} replied`,
      }
    : {
        good: false,
        says: `refused with ${reached.status}`,
        why: reached.said ?? "the provider gave no reason",
      };
}

function useAccountActions(account: Account, onChanged: () => void) {
  const navigate = useNavigate();
  const [trying, setTrying] = useState(false);
  const at = `/api/accounts/${encodeURIComponent(account.name)}`;

  const test = () => {
    setTrying(true);
    send<Reached>(`${at}/test`, {})
      .then((reached) => {
        const answer = read(reached);
        toast(`${account.name} ${answer.says}`, {
          says: answer.why,
          tone: answer.good ? "good" : "critical",
          id: `test/${account.name}`,
        });
        onChanged();
      })
      .catch((error: Error) =>
        toast(`Could not try ${account.name}`, {
          says: error.message,
          tone: "critical",
          id: `test/${account.name}`,
        }),
      )
      .finally(() => setTrying(false));
  };

  const setState = (state: string) =>
    send(`${at}/state`, { state })
      .then(onChanged)
      .catch((error: Error) =>
        toast(`Could not change ${account.name}`, { says: error.message, tone: "critical" }),
      );

  const remove = async () => {
    const yes = await confirmed({
      title: `Remove ${account.name}?`,
      says: "Its credential is deleted.",
      action: "Remove account",
    });
    if (!yes) return;
    drop(at)
      .then(() => {
        toast(`Removed ${account.name}`, { says: "Its credential is deleted." });
        onChanged();
      })
      .catch((error: Error) =>
        toast(`Could not remove ${account.name}`, { says: error.message, tone: "critical" }),
      );
  };

  const showUsage = () => navigate(`/usage?account=${encodeURIComponent(account.name)}`);

  return { trying, test, setState, remove, showUsage };
}

export function AccountMenuItems({
  account,
  onChanged,
}: {
  account: Account;
  onChanged: () => void;
}) {
  const { trying, test, setState, remove, showUsage } = useAccountActions(account, onChanged);

  return (
    <>
      <MenuItem icon={Zap} disabled={trying} onClick={test}>
        {trying ? "Trying…" : "Send a test request"}
      </MenuItem>
      <MenuItem icon={ChartColumn} onClick={showUsage}>
        Show its usage
      </MenuItem>
      {account.state === "active" ? (
        <MenuItem icon={PowerOff} onClick={() => void setState("disabled")}>
          Turn off
        </MenuItem>
      ) : (
        <MenuItem
          icon={account.state === "sign_in_expired" ? RotateCcw : Power}
          onClick={() => void setState("active")}
        >
          {account.state === "sign_in_expired" ? "Try again" : "Turn on"}
        </MenuItem>
      )}
      <MenuSeparator />
      <MenuItem icon={Trash2} danger onClick={() => void remove()}>
        Remove…
      </MenuItem>
    </>
  );
}

export function AccountActions({
  account,
  onChanged,
}: {
  account: Account;
  onChanged: () => void;
}) {
  const { trying, test } = useAccountActions(account, onChanged);

  return (
    <span className="flex items-center justify-end gap-1">
      <Button tone="ghost" size="small" disabled={trying} onClick={test}>
        <Zap strokeWidth={1.75} />
        {trying ? "Trying…" : "Test"}
      </Button>
      <Menu
        trigger={
          <Button tone="ghost" size="icon" aria-label={`More for ${account.name}`}>
            <Ellipsis strokeWidth={1.75} />
          </Button>
        }
      >
        <AccountMenuItems account={account} onChanged={onChanged} />
      </Menu>
    </span>
  );
}
