import { UserRound } from "lucide-react";
import { useState } from "react";
import { Button, Input, toast } from "@/shared/ui";
import { send } from "@/shared/api";
import { useMe } from "@/entities/user";

export function ClaimPerson() {
  const me = useMe((store) => store.me);
  const refresh = useMe((store) => store.refresh);
  const [name, setName] = useState("");
  const [refused, setRefused] = useState<string>();

  if (!me || me.person) return null;

  return (
    <form
      className="flex shrink-0 flex-wrap items-center gap-3 hairline-b border-line-subtle bg-info-tint px-5 py-2"
      onSubmit={(event) => {
        event.preventDefault();
        setRefused(undefined);
        send("/api/me/claim", { name: name.trim() })
          .then(() => {
            toast(`You are ${name.trim()}`, {
              says: "Set a password in Settings to sign in with your name instead of the admin key.",
              tone: "good",
            });
            refresh();
          })
          .catch((error: Error) => setRefused(error.message));
      }}
    >
      <UserRound className="size-4 shrink-0 text-info-text" strokeWidth={1.75} />
      <span className="min-w-0 text-small text-info-text">
        {refused ??
          "You signed in with the admin key. Say who you are, so the plans you sign in to and the keys you issue belong to you."}
      </span>
      <span className="ml-auto flex items-center gap-1.5">
        <Input
          className="h-7 w-44"
          value={name}
          placeholder="your name"
          onChange={(event) => setName(event.target.value)}
        />
        <Button size="small" tone="primary" type="submit" disabled={name.trim() === ""}>
          That's me
        </Button>
      </span>
    </form>
  );
}
