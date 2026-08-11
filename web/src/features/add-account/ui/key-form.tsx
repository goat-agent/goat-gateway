import { useState } from "react";
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
  children?: React.ReactNode;
}) {
  const [secret, setSecret] = useState("");
  const [refused, setRefused] = useState<string>();

  return (
    <form
      className="flex flex-col gap-3"
      onSubmit={(event) => {
        event.preventDefault();
        setRefused(undefined);
        send("/api/accounts", { name: name.trim(), provider, secret: secret.trim() })
          .then(onAdded)
          .catch((error: Error) => setRefused(error.message));
      }}
    >
      <Field label="Name" hint="Whatever helps you tell it apart. Nobody owns an account here.">
        <Input
          autoFocus
          value={name}
          placeholder="claude subscription 1"
          onChange={(event) => onName(event.target.value)}
        />
      </Field>

      <Field label="Provider">
        <Select value={provider} onChange={(event) => onProvider(event.target.value)}>
          {providers.map((entry) => (
            <option key={entry} value={entry}>
              {entry}
            </option>
          ))}
        </Select>
      </Field>

      <Field label="API key" hint="Stored encrypted. It never touches the disk in the clear.">
        <Input
          type="password"
          value={secret}
          placeholder="sk-…"
          onChange={(event) => setSecret(event.target.value)}
        />
      </Field>

      {refused ? <p className="m-0 text-small text-[var(--critical)]">{refused}</p> : null}

      <div className="flex items-center justify-between gap-2 pt-1">
        {children ?? <span />}
        <Button
          tone="primary"
          type="submit"
          disabled={name.trim() === "" || secret.trim() === ""}
        >
          Add
        </Button>
      </div>
    </form>
  );
}
