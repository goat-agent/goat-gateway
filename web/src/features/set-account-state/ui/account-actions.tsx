import { useState } from "react";
import { Link } from "react-router-dom";
import { drop, send } from "@/shared/api";
import { duration } from "@/shared/lib/format";
import { Badge, Button } from "@/shared/ui";
import type { Account } from "@/entities/account";

type Reached = {
  ok: boolean;
  status: number;
  model: string | null;
  said: string | null;
  took_ms: number;
};

export function AccountActions({ account, onChanged }: { account: Account; onChanged: () => void }) {
  const [trying, setTrying] = useState(false);
  const [reached, setReached] = useState<Reached>();

  const at = `/api/accounts/${encodeURIComponent(account.name)}`;
  const setState = (state: string) => send(`${at}/state`, { state }).then(onChanged);

  return (
    <span className="flex items-center justify-end gap-1">
      {reached ? (
        <Badge
          tone={reached.ok ? "good" : "critical"}
          title={reached.said ?? `answered ${reached.status}`}
        >
          {reached.ok
            ? `answered in ${duration(reached.took_ms)}`
            : `refused with ${reached.status}`}
        </Badge>
      ) : null}

      <Button
        tone="quiet"
        size="small"
        disabled={trying}
        onClick={() => {
          setTrying(true);
          setReached(undefined);
          send<Reached>(at + "/test", {})
            .then((answer) => {
              setReached(answer);
              onChanged();
            })
            .finally(() => setTrying(false));
        }}
      >
        {trying ? "Trying…" : "Test"}
      </Button>

      <Link to={`/usage?account=${encodeURIComponent(account.name)}`}>
        <Button tone="quiet" size="small">
          Usage
        </Button>
      </Link>

      {account.state === "active" ? (
        <Button tone="quiet" size="small" onClick={() => setState("disabled")}>
          Turn off
        </Button>
      ) : (
        <Button size="small" onClick={() => setState("active")}>
          {account.state === "sign_in_expired" ? "Try again" : "Turn on"}
        </Button>
      )}

      <Button
        tone="grave"
        size="small"
        onClick={() => {
          if (!confirm(`Remove ${account.name}? Its credential is deleted.`)) return;
          drop(at).then(onChanged);
        }}
      >
        Remove
      </Button>
    </span>
  );
}
