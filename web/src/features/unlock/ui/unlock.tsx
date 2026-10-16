import { useState } from "react";
import { Button, Field, Input, Mark } from "@/shared/ui";
import { tell } from "@/shared/api";
import { useSession } from "@/shared/model/session";

export function Unlock() {
  const unlock = useSession((store) => store.unlock);
  const [key, setKey] = useState("");
  const [refused, setRefused] = useState<string>();
  const [trying, setTrying] = useState(false);

  return (
    <main className="flex h-full items-start justify-center bg-base px-6 pt-[18vh]">
      <form
        className="flex w-[min(22rem,100%)] flex-col gap-6"
        onSubmit={(event) => {
          event.preventDefault();
          setTrying(true);
          setRefused(undefined);
          tell("/api/session", { key })
            .then(unlock)
            .catch((error: Error) => setRefused(error.message))
            .finally(() => setTrying(false));
        }}
      >
        <div className="flex flex-col items-center gap-4 text-center">
          <Mark className="size-9 rounded-[10px] text-regular" />
          <div className="flex flex-col gap-1.5">
            <h1 className="display m-0 text-headline font-semibold text-ink-title">
              Open goat gateway
            </h1>
            <p className="m-0 text-small text-ink-secondary">
              The admin key was printed once, the first time this gateway started.
            </p>
          </div>
        </div>

        <div className="flex flex-col gap-3 rounded-xl hairline border-line bg-panel p-5 shadow-medium">
          <Field label="Admin key" refused={refused}>
            <Input
              autoFocus
              type="password"
              value={key}
              placeholder="gwa_…"
              aria-invalid={refused ? true : undefined}
              onChange={(event) => setKey(event.target.value)}
            />
          </Field>

          <Button
            tone="primary"
            size="medium"
            type="submit"
            className="w-full"
            disabled={trying || key.trim() === ""}
          >
            {trying ? "Checking…" : "Open"}
          </Button>
        </div>
      </form>
    </main>
  );
}
