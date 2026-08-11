import { useState } from "react";
import { send } from "@/shared/api";
import { Button, Field, Input } from "@/shared/ui";
import { useLock } from "../model/lock";

export function Unlock() {
  const open = useLock((held) => held.open);
  const [key, setKey] = useState("");
  const [refused, setRefused] = useState<string>();
  const [trying, setTrying] = useState(false);

  return (
    <main className="flex h-full items-center justify-center bg-base p-6">
      <form
        className="flex w-[22rem] flex-col gap-4 rounded-md border border-line-subtle bg-panel p-5"
        onSubmit={(event) => {
          event.preventDefault();
          setTrying(true);
          setRefused(undefined);
          send("/api/session", { key })
            .then(open)
            .catch((error: Error) => setRefused(error.message))
            .finally(() => setTrying(false));
        }}
      >
        <div className="flex flex-col gap-1">
          <h1 className="m-0 text-heading font-semibold tracking-tight">goat gateway</h1>
          <p className="m-0 text-small text-ink-secondary">
            The admin key was printed once, the first time this gateway started.
          </p>
        </div>

        <Field label="Admin key">
          <Input
            autoFocus
            type="password"
            value={key}
            placeholder="gwa_…"
            onChange={(event) => setKey(event.target.value)}
          />
        </Field>

        {refused ? <p className="m-0 text-small text-[var(--critical)]">{refused}</p> : null}

        <Button tone="primary" type="submit" disabled={trying || key.trim() === ""}>
          {trying ? "Checking…" : "Open"}
        </Button>
      </form>
    </main>
  );
}
