import { useEffect, useState } from "react";
import { Button, Field, Input } from "@/shared/ui";
import { ask, send, tell } from "@/shared/api";
import type { SignInProvider } from "@/entities/provider";

type Started = {
  session: string;
  mode: "loopback" | "paste" | "device";
  authorize_url?: string;
  user_code?: string;
  verification_url?: string;
};

const SAID: Record<string, string> = {
  loopback: "Open the browser here",
  paste: "Open elsewhere and paste the code back",
  device: "Show a code to type on another device",
};

export function SignIn({
  provider,
  name,
  offered,
  onDone,
  onCancel,
}: {
  provider: string;
  name: string;
  offered: SignInProvider | undefined;
  onDone: () => void;
  onCancel: () => void;
}) {
  const [started, setStarted] = useState<Started>();
  const [pasted, setPasted] = useState("");
  const [refused, setRefused] = useState<string>();

  useEffect(() => {
    if (!started || started.mode === "paste") return;
    const session = started.session;
    const poll = setInterval(() => {
      ask<unknown>(`/api/signin/${session}`)
        .then((status) => {
          if (status === "Done") onDone();
          const failure = failureIn(status);
          if (failure) setRefused(failure);
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
            {said(mode)}
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
              : "Open this, approve, then paste the code it gives back."}
          </p>
          <a
            href={started.authorize_url}
            target="_blank"
            rel="noreferrer"
            className="break-all font-mono text-code text-[var(--series-1)]"
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
            tell(`/api/signin/${started.session}`, { code: pasted.trim() })
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

function said(mode: string) {
  return SAID[mode] ?? mode;
}

function failureIn(status: unknown): string | undefined {
  if (typeof status !== "object" || status === null || !("Failed" in status)) return undefined;
  const { Failed } = status;
  if (typeof Failed !== "object" || Failed === null || !("message" in Failed)) return undefined;
  return typeof Failed.message === "string" ? Failed.message : undefined;
}
