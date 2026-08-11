import { Link } from "react-router-dom";
import { Button } from "@/shared/ui";
import { drop, tell } from "@/shared/api";
import type { Account } from "@/entities/account";

export function AccountActions({ account, onChanged }: { account: Account; onChanged: () => void }) {
  const move = (state: string) =>
    void tell(`/api/accounts/${encodeURIComponent(account.name)}/state`, { state }).then(onChanged);

  return (
    <span className="flex justify-end gap-1">
      <Link to={`/usage?account=${encodeURIComponent(account.name)}`}>
        <Button tone="quiet" size="small">
          Usage
        </Button>
      </Link>
      {account.state === "disabled" ? (
        <Button size="small" onClick={() => move("active")}>
          Turn on
        </Button>
      ) : (
        <Button tone="quiet" size="small" onClick={() => move("disabled")}>
          Turn off
        </Button>
      )}
      <Button
        tone="grave"
        size="small"
        onClick={() => {
          if (!confirm(`Remove ${account.name}? Its credential is deleted.`)) return;
          void drop(`/api/accounts/${encodeURIComponent(account.name)}`).then(onChanged);
        }}
      >
        Remove
      </Button>
    </span>
  );
}
