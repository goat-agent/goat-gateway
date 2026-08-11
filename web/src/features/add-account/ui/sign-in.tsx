import { useEffect, useState } from "react";
import { ask, send, useResource } from "@/shared/api";
import { Button, Field, Input } from "@/shared/ui";
import type { SignInProvider } from "@/entities/provider";

type Started = {
  session: string;
  mode: "loopback" | "paste" | "device";
  authorize_url?: string;
  user_code?: string;
  verification_url?: string;
};

type Status = "Waiting" | "Done" | { Failed: { message: string } };

const INVITES: Record<string, string> = {
  loopback: "Open the browser here",
  paste: "Open elsewhere and paste the code back",
  device: "Show a code to type on another device",
};

export function SignIn({
  provider,
  name,
  onDone,
  onCancel,
}: {
  provider: string;
  name: string;
  onDone: () => void;
  onCancel: () => void;
}) {
  const flows = useResource<{ providers: SignInProvider[] }>("/api/signin/providers");
  const offered = flows.data?.providers.find((entry) => entry.provider === provider);

  const [started, setStarted] = useState<Started>();
  const [pasted, setPasted] = useState("");
  const [refused, setRefused] = useState<string>();

  useEffect(() => {
    if (!started || started.mode === "paste") return;
    const poll = setInterval(() => {
      ask<Status>(`/api/signin/${started.session}`)
        .then((status) => {
          if (status === "Done") {
            clearInterval(poll);
            onDone();
          } else if (typeof status === "object") {
            clearInterval(poll);
            setRefused(status.Failed.message);
          }
        })
        .catch(() => clearInterval(poll));
    }, 1000);
    return () => clearInterval(poll);
  }, [started, onDone]);

  if (!started) {
    return (
      <div className="flex flex-col gap-3">
        <p className="m-0 text-small text-ink-secondary">
          Signing in to <span className="text-ink">{provider}</span> as{" "}
          <span className="text-ink">{name}</span>.
        </p>
        {(offered?.modes ?? []).map((mode) => (
          <Button
            key={mode}
            onClick={() => {
              setRefused(undefined);
              send<Started>("/api/signin", { provider, name, mode })
                .then(setStarted)
                .catch((error: Error) => setRefused(error.message));
            }}
          >
            {INVITES[mode] ?? mode}
          </Button>
        ))}
        {refused ? <p className="m-0 text-small text-[var(--critical)]">{refused}</p> : null}
        <Button tone="quiet" onClick={onCancel}>
          Back
        </Button>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3">
      {started.authorize_url ? (
        <>
          <p className="m-0 text-small text-ink-secondary">
            {started.mode === "loopback"
              ? "Finish in the browser tab. This window updates itself."
              : "Open this, approve, then paste back the code it gives you."}
          </p>
          <a
            href={started.authorize_url}
            target="_blank"
            rel="noreferrer"
            className="font-mono text-code break-all text-[var(--series-1)]"
          >
            {started.authorize_url}
          </a>
        </>
      ) : null}

      {started.user_code ? (
        <p className="m-0 text-small text-ink-secondary">
          Go to{" "}
          <a href={started.verification_url} target="_blank" rel="noreferrer">
            {started.verification_url}
          </a>{" "}
          and type <span className="numeric text-ink">{started.user_code}</span>.
        </p>
      ) : null}

      {started.mode === "paste" ? (
        <form
          className="flex flex-col gap-3"
          onSubmit={(event) => {
            event.preventDefault();
            setRefused(undefined);
            send(`/api/signin/${started.session}`, { code: pasted.trim() })
              .then(onDone)
              .catch((error: Error) => setRefused(error.message));
          }}
        >
          <Field label="Code">
            <Input
              autoFocus
              value={pasted}
              placeholder="paste it here"
              onChange={(event) => setPasted(event.target.value)}
            />
          </Field>
          <Button tone="primary" type="submit" disabled={pasted.trim() === ""}>
            Finish
          </Button>
        </form>
      ) : (
        <p className="m-0 text-small text-ink-muted">Waiting…</p>
      )}

      {refused ? <p className="m-0 text-small text-[var(--critical)]">{refused}</p> : null}
      <Button tone="quiet" onClick={onCancel}>
        Cancel
      </Button>
    </div>
  );
}
