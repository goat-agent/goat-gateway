import { Link } from "react-router-dom";
import { drop, send } from "@/shared/api";
import { Button } from "@/shared/ui";
import type { Account } from "@/entities/account";

export function AccountActions({ account, onChanged }: { account: Account; onChanged: () => void }) {
  const setState = (state: string) =>
    send(`/api/accounts/${encodeURIComponent(account.name)}/state`, { state }).then(onChanged);

  return (
    <span className="flex justify-end gap-1">
      <Link to={`/usage?account=${encodeURIComponent(account.name)}`}>
        <Button tone="quiet" size="small">
          Usage
        </Button>
      </Link>

      {account.state === "sign_in_expired" ? (
        <Button size="small" onClick={() => setState("active")}>
          Try again
        </Button>
      ) : account.state === "disabled" ? (
        <Button size="small" onClick={() => setState("active")}>
          Turn on
        </Button>
      ) : (
        <Button tone="quiet" size="small" onClick={() => setState("disabled")}>
          Turn off
        </Button>
      )}

      <Button
        tone="grave"
        size="small"
        onClick={() => {
          if (!confirm(`Remove ${account.name}? Its credential is deleted.`)) return;
          drop(`/api/accounts/${encodeURIComponent(account.name)}`).then(onChanged);
        }}
      >
        Remove
      </Button>
    </span>
  );
}
