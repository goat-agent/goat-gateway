import { useState } from "react";
import { ArrowLeft, Check, Minus, Radar, X } from "lucide-react";
import { Button, Dialog, Field, Input } from "@/shared/ui";
import { send } from "@/shared/api";
import { WIRE_LABELS, type Discovered, type Finding } from "@/entities/provider";
import { DeclarationEditor } from "./declaration-editor";

const BY_HAND = `label = "My provider"
key = "bearer"
limits = "none"
endpoints = [
  { wire = "chat", url = "https://example.com/v1/chat/completions" },
]

[[models]]
name = "my-model"
`;

type Step = { kind: "probe" } | { kind: "declare"; found: Discovered | undefined };

export function DeclareProvider({
  open,
  onClose,
  onSaved,
}: {
  open: boolean;
  onClose: () => void;
  onSaved: (id: string) => void;
}) {
  const [step, setStep] = useState<Step>({ kind: "probe" });
  const [base, setBase] = useState("");
  const [secret, setSecret] = useState("");
  const [model, setModel] = useState("");
  const [probing, setProbing] = useState(false);
  const [refused, setRefused] = useState<string>();
  const [id, setId] = useState("");
  const [text, setText] = useState("");

  const close = () => {
    setStep({ kind: "probe" });
    setRefused(undefined);
    setSecret("");
    onClose();
  };

  const declare = (found: Discovered | undefined) => {
    setId(found?.suggested_id ?? "");
    setText(found?.declaration ?? BY_HAND);
    setStep({ kind: "declare", found });
  };

  return (
    <Dialog
      open={open}
      onClose={close}
      width="wide"
      title="Declare a provider"
      description={
        step.kind === "probe"
          ? "Point the gateway at an endpoint and it works out which formats it speaks, how it wants its key, and which models it lists."
          : "Check what was found, adjust the declaration, then save. The gateway reloads it without a restart."
      }
    >
      {step.kind === "probe" ? (
        <form
          className="flex flex-col gap-4"
          onSubmit={(event) => {
            event.preventDefault();
            setRefused(undefined);
            setProbing(true);
            send<Discovered>("/api/providers/discover", {
              base_url: base.trim(),
              secret: secret.trim(),
              model: model.trim() || undefined,
            })
              .then(declare)
              .catch((error: Error) => setRefused(error.message))
              .finally(() => setProbing(false));
          }}
        >
          <Field label="Base URL" hint="Where the API lives, with or without /v1.">
            <Input
              className="mono"
              placeholder="https://api.example.com"
              value={base}
              onChange={(event) => setBase(event.target.value)}
            />
          </Field>
          <Field label="API key" hint="Used for these probes only. Add it as an account afterwards to keep it.">
            <Input
              type="password"
              autoComplete="off"
              value={secret}
              onChange={(event) => setSecret(event.target.value)}
            />
          </Field>
          <Field
            label="A model it serves"
            hint="Optional. Needed when the endpoint lists no models, so the probes have something to ask for."
            refused={refused}
          >
            <Input className="mono" value={model} onChange={(event) => setModel(event.target.value)} />
          </Field>
          <div className="flex items-center justify-between gap-2 pt-1">
            <Button size="medium" tone="ghost" onClick={() => declare(undefined)}>
              Write it by hand
            </Button>
            <Button
              type="submit"
              tone="primary"
              size="medium"
              disabled={base.trim() === "" || secret.trim() === "" || probing}
            >
              <Radar strokeWidth={1.75} />
              {probing ? "Probing…" : "Find out"}
            </Button>
          </div>
        </form>
      ) : (
        <div className="flex flex-col gap-4">
          {step.found ? <Findings found={step.found} /> : null}
          <DeclarationEditor
            id={id}
            onId={setId}
            fixed={false}
            text={text}
            onText={setText}
            onSaved={(saved) => {
              close();
              onSaved(saved);
            }}
          >
            <Button size="medium" tone="ghost" onClick={() => setStep({ kind: "probe" })}>
              <ArrowLeft strokeWidth={1.75} />
              Back
            </Button>
          </DeclarationEditor>
        </div>
      )}
    </Dialog>
  );
}

function Findings({ found }: { found: Discovered }) {
  return (
    <div className="flex flex-col gap-2 rounded-lg hairline border-line bg-base px-3 py-2.5">
      <ul className="m-0 flex list-none flex-col gap-1.5 p-0">
        {found.findings.map((finding) => (
          <li key={finding.wire} className="flex min-w-0 items-center gap-2 text-small">
            <Verdict finding={finding} />
            <span className="w-32 shrink-0 font-medium text-ink">{WIRE_LABELS[finding.wire]}</span>
            <span className="mono min-w-0 grow truncate text-mini text-ink-secondary">{finding.url}</span>
            <span className="numeric shrink-0 text-mini text-ink-secondary">
              {finding.status === null ? "no answer" : finding.status}
            </span>
          </li>
        ))}
      </ul>
      <p className="m-0 text-mini text-ink-secondary">
        {found.models.length === 0
          ? "It listed no models."
          : `It lists ${found.models.length} model${found.models.length === 1 ? "" : "s"}.`}{" "}
        {found.key === null
          ? "Neither key style was accepted."
          : found.key === "x_api_key"
            ? "It wants the key in x-api-key."
            : "It wants the key as a bearer token."}{" "}
        {found.reports_limits ? "It reports rate limits in its headers." : "It reported no rate limits."}
      </p>
    </div>
  );
}

function Verdict({ finding }: { finding: Finding }) {
  if (finding.serves) return <Check className="size-3.5 shrink-0 text-good-text" strokeWidth={2} />;
  if (finding.status === null) return <Minus className="size-3.5 shrink-0 text-ink-muted" strokeWidth={2} />;
  return <X className="size-3.5 shrink-0 text-ink-muted" strokeWidth={2} />;
}
