import { useState, type ReactNode } from "react";
import { Button, Field, Input, Select, type Choice } from "@/shared/ui";
import { send } from "@/shared/api";

export function Owners({
  choices,
  value,
  onChange,
  hint,
}: {
  choices: readonly Choice<string>[];
  value: string;
  onChange: (value: string) => void;
  hint: string;
}) {
  if (choices.length === 0) return null;
  return (
    <Field label="Whose" hint={hint}>
      <Select label="Whose" size="medium" value={value} choices={choices} onChange={onChange} />
    </Field>
  );
}

export function KeyForm({
  provider,
  label,
  owners,
  onAdded,
  children,
}: {
  provider: string;
  label: string;
  owners: readonly Choice<string>[];
  onAdded: (name: string) => void;
  children?: ReactNode;
}) {
  const [name, setName] = useState(label);
  const [secret, setSecret] = useState("");
  const [owner, setOwner] = useState("");
  const [refused, setRefused] = useState<string>();
  const [adding, setAdding] = useState(false);
  const whose = owner || owners[0]?.value || "";

  return (
    <form
      className="flex flex-col gap-4"
      onSubmit={(event) => {
        event.preventDefault();
        setRefused(undefined);
        setAdding(true);
        send("/api/accounts", {
          name: name.trim(),
          provider,
          secret: secret.trim(),
          owner: owners.length > 0 ? whose : undefined,
        })
          .then(() => onAdded(name.trim()))
          .catch((error: Error) => setRefused(error.message))
          .finally(() => setAdding(false));
      }}
    >
      <Field label="Name" hint="What it is called in the console and in the request history.">
        <Input autoFocus value={name} onChange={(event) => setName(event.target.value)} />
      </Field>

      <Field
        label={`${label} API key`}
        hint="Stored encrypted. It never touches the disk in the clear."
        refused={refused}
      >
        <Input
          type="password"
          autoComplete="off"
          value={secret}
          placeholder="sk-…"
          aria-invalid={refused ? true : undefined}
          onChange={(event) => setSecret(event.target.value)}
        />
      </Field>

      <Owners
        choices={owners}
        value={whose}
        onChange={setOwner}
        hint="Everyone means every person's requests may use it. A key can be shared; a plan cannot."
      />

      <div className="flex items-center justify-between gap-2 pt-1">
        {children ?? <span />}
        <Button
          tone="primary"
          size="medium"
          type="submit"
          disabled={adding || name.trim() === "" || secret.trim() === ""}
        >
          {adding ? "Adding…" : "Add account"}
        </Button>
      </div>
    </form>
  );
}

export function SetupTokenForm({
  label,
  owners,
  onAdded,
  children,
}: {
  label: string;
  owners: readonly Choice<string>[];
  onAdded: (name: string) => void;
  children?: ReactNode;
}) {
  const [name, setName] = useState(label);
  const [token, setToken] = useState("");
  const [owner, setOwner] = useState("");
  const [refused, setRefused] = useState<string>();
  const [adding, setAdding] = useState(false);
  const whose = owner || owners[0]?.value || "";

  return (
    <form
      className="flex flex-col gap-4"
      onSubmit={(event) => {
        event.preventDefault();
        setRefused(undefined);
        setAdding(true);
        send("/api/accounts", {
          name: name.trim(),
          provider: "anthropic",
          credential_kind: "setup_token",
          secret: token.trim(),
          owner: owners.length > 0 ? whose : undefined,
        })
          .then(() => onAdded(name.trim()))
          .catch((error: Error) => setRefused(error.message))
          .finally(() => setAdding(false));
      }}
    >
      <ol className="m-0 flex list-decimal flex-col gap-1 pl-5 text-small text-ink-secondary">
        <li>
          On a machine where Claude Code is signed in to the plan, run{" "}
          <code className="mono rounded hairline border-line bg-base px-1 text-ink-title">claude setup-token</code>.
        </li>
        <li>Paste what it prints. It starts with sk-ant-oat and lasts a year.</li>
      </ol>
      <Field label="Name" hint="What it is called in the console and in the request history.">
        <Input autoFocus value={name} onChange={(event) => setName(event.target.value)} />
      </Field>
      <Field label="Setup token" refused={refused}>
        <Input
          type="password"
          autoComplete="off"
          value={token}
          placeholder="sk-ant-oat…"
          aria-invalid={refused ? true : undefined}
          onChange={(event) => setToken(event.target.value)}
        />
      </Field>
      <Owners
        choices={owners}
        value={whose}
        onChange={setOwner}
        hint="A plan belongs to one person, and only their requests use it."
      />
      <div className="flex items-center justify-between gap-2 pt-1">
        {children ?? <span />}
        <Button
          tone="primary"
          size="medium"
          type="submit"
          disabled={adding || name.trim() === "" || token.trim() === ""}
        >
          {adding ? "Adding…" : "Add plan"}
        </Button>
      </div>
    </form>
  );
}
