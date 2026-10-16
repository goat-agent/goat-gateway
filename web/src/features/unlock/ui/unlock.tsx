import { useState } from "react";
import { Button, Field, Input, Mark, SegmentedControl } from "@/shared/ui";
import { tell } from "@/shared/api";
import { useSession } from "@/shared/model/session";
import { useMe } from "@/entities/user";

const WAYS = [
  { value: "person", label: "Name and password" },
  { value: "admin", label: "Admin key" },
] as const;

type Way = (typeof WAYS)[number]["value"];

export function Unlock() {
  const unlock = useSession((store) => store.unlock);
  const refresh = useMe((store) => store.refresh);
  const [way, setWay] = useState<Way>("person");
  const [name, setName] = useState("");
  const [password, setPassword] = useState("");
  const [key, setKey] = useState("");
  const [refused, setRefused] = useState<string>();
  const [trying, setTrying] = useState(false);

  const ready = way === "admin" ? key.trim() !== "" : name.trim() !== "" && password !== "";

  return (
    <main className="flex h-full items-start justify-center bg-base px-6 pt-[18vh]">
      <form
        className="flex w-[min(22rem,100%)] flex-col gap-6"
        onSubmit={(event) => {
          event.preventDefault();
          setTrying(true);
          setRefused(undefined);
          const credentials = way === "admin" ? { key } : { name: name.trim(), password };
          tell("/api/session", credentials)
            .then(() => refresh())
            .then(unlock)
            .catch((error: Error) => setRefused(error.message))
            .finally(() => setTrying(false));
        }}
      >
        <div className="flex flex-col items-center gap-4 text-center">
          <Mark className="size-9 rounded-[10px] text-regular" />
          <div className="flex flex-col gap-1.5">
            <h1 className="display m-0 text-headline font-semibold text-ink-title">
              Sign in to goat gateway
            </h1>
            <p className="m-0 text-small text-ink-secondary">
              {way === "admin"
                ? "The admin key was printed once, the first time this gateway started."
                : "Your name and the password you set from your invite link."}
            </p>
          </div>
        </div>

        <div className="flex flex-col gap-3 rounded-xl hairline border-line bg-panel p-5 shadow-medium">
          <SegmentedControl
            label="How to sign in"
            value={way}
            choices={WAYS}
            onChange={(next) => {
              setWay(next);
              setRefused(undefined);
            }}
            className="self-stretch [&>*]:flex-1"
          />

          {way === "admin" ? (
            <Field label="Admin key" refused={refused}>
              <Input
                autoFocus
                type="password"
                autoComplete="off"
                value={key}
                placeholder="gwa_…"
                aria-invalid={refused ? true : undefined}
                onChange={(event) => setKey(event.target.value)}
              />
            </Field>
          ) : (
            <>
              <Field label="Name">
                <Input
                  autoFocus
                  autoComplete="username"
                  value={name}
                  onChange={(event) => setName(event.target.value)}
                />
              </Field>
              <Field label="Password" refused={refused}>
                <Input
                  type="password"
                  autoComplete="current-password"
                  value={password}
                  aria-invalid={refused ? true : undefined}
                  onChange={(event) => setPassword(event.target.value)}
                />
              </Field>
            </>
          )}

          <Button
            tone="primary"
            size="medium"
            type="submit"
            className="w-full"
            disabled={trying || !ready}
          >
            {trying ? "Checking…" : "Sign in"}
          </Button>
        </div>
      </form>
    </main>
  );
}
