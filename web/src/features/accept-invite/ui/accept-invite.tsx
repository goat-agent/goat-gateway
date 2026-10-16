import { useState } from "react";
import { Button, Field, Input, Mark } from "@/shared/ui";
import { tell, useResource } from "@/shared/api";
import { useSession } from "@/shared/model/session";
import { useMe } from "@/entities/user";

const SHORTEST = 8;

export function AcceptInvite({ token, onAccepted }: { token: string; onAccepted: () => void }) {
  const invited = useResource<{ name: string }>(`/api/invites/${encodeURIComponent(token)}`);
  const unlock = useSession((store) => store.unlock);
  const refresh = useMe((store) => store.refresh);
  const [password, setPassword] = useState("");
  const [again, setAgain] = useState("");
  const [refused, setRefused] = useState<string>();
  const [trying, setTrying] = useState(false);

  const short = password.length > 0 && password.length < SHORTEST;
  const mismatched = again.length > 0 && again !== password;

  return (
    <main className="flex h-full items-start justify-center bg-base px-6 pt-[18vh]">
      <form
        className="flex w-[min(22rem,100%)] flex-col gap-6"
        onSubmit={(event) => {
          event.preventDefault();
          setTrying(true);
          setRefused(undefined);
          tell(`/api/invites/${encodeURIComponent(token)}`, { password })
            .then(() => refresh())
            .then(() => {
              unlock();
              onAccepted();
            })
            .catch((error: Error) => setRefused(error.message))
            .finally(() => setTrying(false));
        }}
      >
        <div className="flex flex-col items-center gap-4 text-center">
          <Mark className="size-9 rounded-[10px] text-regular" />
          <div className="flex flex-col gap-1.5">
            <h1 className="display m-0 text-headline font-semibold text-ink-title">
              {invited.data ? `Welcome, ${invited.data.name}` : "Accept your invite"}
            </h1>
            <p className="m-0 text-small text-ink-secondary">
              {invited.error ??
                "Choose a password. You sign in with your name and this password from now on."}
            </p>
          </div>
        </div>

        {invited.data ? (
          <div className="flex flex-col gap-3 rounded-xl hairline border-line bg-panel p-5 shadow-medium">
            <Field
              label="Password"
              hint={`At least ${SHORTEST} characters.`}
              refused={short ? `At least ${SHORTEST} characters.` : undefined}
            >
              <Input
                autoFocus
                type="password"
                autoComplete="new-password"
                value={password}
                onChange={(event) => setPassword(event.target.value)}
              />
            </Field>
            <Field
              label="Again"
              refused={mismatched ? "The two passwords are not the same." : refused}
            >
              <Input
                type="password"
                autoComplete="new-password"
                value={again}
                onChange={(event) => setAgain(event.target.value)}
              />
            </Field>
            <Button
              tone="primary"
              size="medium"
              type="submit"
              className="w-full"
              disabled={trying || password.length < SHORTEST || again !== password}
            >
              {trying ? "Setting…" : "Set password and sign in"}
            </Button>
          </div>
        ) : null}
      </form>
    </main>
  );
}
