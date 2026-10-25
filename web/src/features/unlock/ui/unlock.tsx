import { useEffect, useState } from "react";
import { Button, Field, Input, Mark, SegmentedControl } from "@/shared/ui";
import { ask, tell } from "@/shared/api";
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
  const [nobodyYet, setNobodyYet] = useState(false);
  const [way, setWay] = useState<Way>("person");
  const [name, setName] = useState("");
  const [password, setPassword] = useState("");
  const [key, setKey] = useState("");
  const [refused, setRefused] = useState<string>();
  const [trying, setTrying] = useState(false);

  useEffect(() => {
    void ask<{ nobody_yet: boolean }>("/api/health").then((health) => {
      setNobodyYet(health.nobody_yet);
      if (health.nobody_yet) setWay("admin");
    });
  }, []);

  const recovering = way === "admin";
  const ready =
    name.trim() !== "" && password !== "" && (!recovering || key.trim() !== "");

  return (
    <main className="flex h-full items-start justify-center bg-base px-6 pt-[18vh]">
      <form
        className="flex w-[min(22rem,100%)] flex-col gap-6"
        onSubmit={(event) => {
          event.preventDefault();
          setTrying(true);
          setRefused(undefined);
          const credentials = recovering
            ? { key: key.trim(), name: name.trim(), password }
            : { name: name.trim(), password };
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
              {nobodyYet ? "Set up goat gateway" : "Sign in to goat gateway"}
            </h1>
            <p className="m-0 text-small text-ink-secondary">
              {nobodyYet
                ? "Use the admin key printed when the gateway first started, and choose the name and password you will sign in with."
                : recovering
                  ? "The admin key signs you in as the person you name, with a new password. A name nobody has yet makes a new admin."
                  : "Your name and the password you set from your invite link."}
            </p>
          </div>
        </div>

        <div className="flex flex-col gap-3 rounded-xl hairline border-line bg-panel p-5 shadow-medium">
          {nobodyYet ? null : (
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
          )}

          {recovering ? (
            <Field label="Admin key">
              <Input
                autoFocus
                type="password"
                autoComplete="off"
                value={key}
                placeholder="gwa_…"
                onChange={(event) => setKey(event.target.value)}
              />
            </Field>
          ) : null}
          <Field label="Name">
            <Input
              autoFocus={!recovering}
              autoComplete="username"
              value={name}
              onChange={(event) => setName(event.target.value)}
            />
          </Field>
          <Field label={recovering ? "New password" : "Password"} refused={refused}>
            <Input
              type="password"
              autoComplete={recovering ? "new-password" : "current-password"}
              value={password}
              aria-invalid={refused ? true : undefined}
              onChange={(event) => setPassword(event.target.value)}
            />
          </Field>

          <Button
            tone="primary"
            size="medium"
            type="submit"
            className="w-full"
            disabled={trying || !ready}
          >
            {trying ? "Checking…" : nobodyYet ? "Set up and sign in" : "Sign in"}
          </Button>
        </div>
      </form>
    </main>
  );
}
