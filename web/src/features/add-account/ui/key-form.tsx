import { useState, type ReactNode } from "react";
import { Button, Field, Input, Select } from "@/shared/ui";
import { send } from "@/shared/api";

export function KeyForm({
  providers,
  provider,
  onProvider,
  name,
  onName,
  onAdded,
  children,
}: {
  providers: string[];
  provider: string;
  onProvider: (provider: string) => void;
  name: string;
  onName: (name: string) => void;
  onAdded: () => void;
  children?: ReactNode;
}) {
  const [secret, setSecret] = useState("");
  const [refused, setRefused] = useState<string>();
  const [adding, setAdding] = useState(false);

  const offered = providers.includes(provider) ? providers : [provider, ...providers];

  return (
    <form
      className="flex flex-col gap-4"
      onSubmit={(event) => {
        event.preventDefault();
        setRefused(undefined);
        setAdding(true);
        send("/api/accounts", { name: name.trim(), provider, secret: secret.trim() })
          .then(onAdded)
          .catch((error: Error) => setRefused(error.message))
          .finally(() => setAdding(false));
      }}
    >
      <Field label="Name" hint="Whatever helps you tell it apart. Nobody owns an account here.">
        <Input
          value={name}
          placeholder="claude subscription 1"
          onChange={(event) => onName(event.target.value)}
        />
      </Field>

      <Field label="Provider">
        <Select
          label="Provider"
          size="medium"
          value={provider}
          choices={offered.map((entry) => ({ value: entry, label: entry }))}
          onChange={onProvider}
        />
      </Field>

      <Field
        label="API key"
        hint="Stored encrypted. It never touches the disk in the clear."
        refused={refused}
      >
        <Input
          type="password"
          value={secret}
          placeholder="sk-…"
          aria-invalid={refused ? true : undefined}
          onChange={(event) => setSecret(event.target.value)}
        />
      </Field>

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
