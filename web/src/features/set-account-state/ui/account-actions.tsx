import { ChartColumn, Ellipsis, Power, PowerOff, RotateCcw, Rows3, Trash2, Zap } from "lucide-react";
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

function useAccountActions(account: Account, onChanged: () => void, onRemoved?: () => void) {
  const navigate = useNavigate();
  const [trying, setTrying] = useState(false);
  const at = `/api/accounts/${encodeURIComponent(account.name)}`;
  const named = encodeURIComponent(account.name);

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
        onRemoved?.();
        onChanged();
      })
      .catch((error: Error) =>
        toast(`Could not remove ${account.name}`, { says: error.message, tone: "critical" }),
      );
  };

  const showUsage = () => navigate(`/usage?account=${named}`);
  const showRequests = () => navigate(`/requests?account=${named}`);

  return { trying, test, setState, remove, showUsage, showRequests };
}

export function AccountMenuItems({
  account,
  onChanged,
}: {
  account: Account;
  onChanged: () => void;
}) {
  const { trying, test, setState, remove, showUsage, showRequests } = useAccountActions(
    account,
    onChanged,
  );

  return (
    <>
      <MenuItem icon={Zap} disabled={trying} onClick={test}>
        {trying ? "Trying…" : "Send a test request"}
      </MenuItem>
      <MenuItem icon={Rows3} onClick={showRequests}>
        Show its requests
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

export function AccountControls({
  account,
  onChanged,
  onRemoved,
}: {
  account: Account;
  onChanged: () => void;
  onRemoved: () => void;
}) {
  const { trying, test, setState, remove, showUsage } = useAccountActions(
    account,
    onChanged,
    onRemoved,
  );

  return (
    <>
      <Button disabled={trying} onClick={test}>
        <Zap strokeWidth={1.75} />
        {trying ? "Trying…" : "Test"}
      </Button>
      {account.state === "active" ? (
        <Button onClick={() => void setState("disabled")}>
          <PowerOff strokeWidth={1.75} />
          Turn off
        </Button>
      ) : (
        <Button onClick={() => void setState("active")}>
          {account.state === "sign_in_expired" ? (
            <RotateCcw strokeWidth={1.75} />
          ) : (
            <Power strokeWidth={1.75} />
          )}
          {account.state === "sign_in_expired" ? "Try again" : "Turn on"}
        </Button>
      )}
      <Menu
        trigger={
          <Button size="icon" aria-label={`More for ${account.name}`}>
            <Ellipsis strokeWidth={1.75} />
          </Button>
        }
      >
        <MenuItem icon={ChartColumn} onClick={showUsage}>
          Show its usage
        </MenuItem>
        <MenuSeparator />
        <MenuItem icon={Trash2} danger onClick={() => void remove()}>
          Remove…
        </MenuItem>
      </Menu>
    </>
  );
}
