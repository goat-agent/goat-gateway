import { ArrowUpRight, Globe, KeySquare, MonitorSmartphone, type LucideIcon } from "lucide-react";
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

const SAID: Record<string, { text: string; icon: LucideIcon }> = {
  loopback: { text: "Open the browser here", icon: Globe },
  paste: { text: "Open elsewhere and paste the code back", icon: KeySquare },
  device: { text: "Show a code to type on another device", icon: MonitorSmartphone },
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
      <div className="flex flex-col gap-4">
        <p className="m-0 text-small text-ink-secondary">
          Signing in to <span className="font-medium text-ink">{provider}</span> as{" "}
          <span className="font-medium text-ink">{name}</span>.
        </p>
        <div className="flex flex-col gap-1.5">
          {(offered?.modes ?? []).map((mode) => {
            const Icon = SAID[mode]?.icon ?? Globe;
            return (
              <Button
                key={mode}
                size="medium"
                className="justify-start"
                onClick={() => {
                  setRefused(undefined);
                  send<Started>("/api/signin", { provider, name, mode })
                    .then(setStarted)
                    .catch((error: Error) => setRefused(error.message));
                }}
              >
                <Icon strokeWidth={1.75} className="text-ink-secondary" />
                {said(mode)}
              </Button>
            );
          })}
        </div>
        {refused ? <p className="m-0 text-mini text-critical-text">{refused}</p> : null}
        <div className="flex justify-end">
          <Button size="medium" onClick={onCancel}>
            Back
          </Button>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-4">
      {started.authorize_url ? (
        <div className="flex flex-col gap-2">
          <p className="m-0 text-small text-ink-secondary">
            {started.mode === "loopback"
              ? "Finish in the browser tab. This window updates itself."
              : "Open this, approve, then paste the code it gives back."}
          </p>
          <a
            href={started.authorize_url}
            target="_blank"
            rel="noreferrer"
            className="hover-fade group flex items-start gap-2 rounded-lg hairline border-line bg-base px-3 py-2.5 hover:border-line-solid"
          >
            <span className="mono min-w-0 grow break-all text-link">{started.authorize_url}</span>
            <ArrowUpRight className="mt-0.5 size-3.5 shrink-0 text-ink-muted group-hover:text-ink" />
          </a>
        </div>
      ) : null}

      {started.user_code ? (
        <div className="flex flex-col gap-2">
          <p className="m-0 text-small text-ink-secondary">
            Go to{" "}
            <a
              href={started.verification_url}
              target="_blank"
              rel="noreferrer"
              className="text-link hover:underline"
            >
              {started.verification_url}
            </a>{" "}
            and type
          </p>
          <span className="mono self-start rounded-lg hairline border-line bg-base px-3 py-2 text-large tracking-[0.2em] text-ink-title select-all">
            {started.user_code}
          </span>
        </div>
      ) : null}

      {started.mode === "paste" ? (
        <form
          className="flex flex-col gap-4"
          onSubmit={(event) => {
            event.preventDefault();
            setRefused(undefined);
            tell(`/api/signin/${started.session}`, { code: pasted.trim() })
              .then(onDone)
              .catch((error: Error) => setRefused(error.message));
          }}
        >
          <Field label="Code" refused={refused}>
            <Input
              value={pasted}
              placeholder="paste it here"
              onChange={(event) => setPasted(event.target.value)}
            />
          </Field>
          <div className="flex justify-end gap-2">
            <Button size="medium" onClick={onCancel}>
              Cancel
            </Button>
            <Button tone="primary" size="medium" type="submit" disabled={pasted.trim() === ""}>
              Finish
            </Button>
          </div>
        </form>
      ) : (
        <>
          <p className="m-0 flex items-center gap-2 text-small text-ink-secondary">
            <span className="size-1.5 animate-pulse rounded-full bg-info" />
            Waiting…
          </p>
          {refused ? <p className="m-0 text-mini text-critical-text">{refused}</p> : null}
          <div className="flex justify-end">
            <Button size="medium" onClick={onCancel}>
              Cancel
            </Button>
          </div>
        </>
      )}
    </div>
  );
}

function said(mode: string) {
  return SAID[mode]?.text ?? mode;
}

function failureIn(status: unknown): string | undefined {
  if (typeof status !== "object" || status === null || !("Failed" in status)) return undefined;
  const { Failed } = status;
  if (typeof Failed !== "object" || Failed === null || !("message" in Failed)) return undefined;
  return typeof Failed.message === "string" ? Failed.message : undefined;
}
