import { useState } from "react";
import { send, useResource } from "@/shared/api";
import { Button, Dialog, Field, Input, Select } from "@/shared/ui";
import type { SignInProvider } from "@/entities/provider";
import { SignIn } from "./sign-in";

export function AddAccount({
  open,
  onClose,
  onAdded,
}: {
  open: boolean;
  onClose: () => void;
  onAdded: () => void;
}) {
  const models = useResource<Record<string, string[]>>(open ? "/api/models" : null);
  const flows = useResource<{ providers: SignInProvider[] }>(open ? "/api/signin/providers" : null);

  const [name, setName] = useState("");
  const [provider, setProvider] = useState("anthropic");
  const [secret, setSecret] = useState("");
  const [refused, setRefused] = useState<string>();
  const [signingIn, setSigningIn] = useState(false);

  const canSignIn = flows.data?.providers.some((entry) => entry.provider === provider) ?? false;

  const close = () => {
    setName("");
    setSecret("");
    setRefused(undefined);
    setSigningIn(false);
    onClose();
  };

  return (
    <Dialog open={open} onClose={close} title="Add an account">
      {signingIn ? (
        <SignIn
          provider={provider}
          name={name.trim()}
          onDone={onAdded}
          onCancel={() => setSigningIn(false)}
        />
      ) : (
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
              onChange={(event) => setName(event.target.value)}
            />
          </Field>

          <Field label="Provider">
            <Select value={provider} onChange={(event) => setProvider(event.target.value)}>
              {Object.keys(models.data ?? {}).map((entry) => (
                <option key={entry} value={entry}>
                  {entry}
                </option>
              ))}
            </Select>
          </Field>

          <Field label="API key" hint="Stored encrypted. It is never written to disk in the clear.">
            <Input
              type="password"
              value={secret}
              placeholder="sk-…"
              onChange={(event) => setSecret(event.target.value)}
            />
          </Field>

          {refused ? <p className="m-0 text-small text-[var(--critical)]">{refused}</p> : null}

          <div className="flex items-center justify-between gap-2 pt-1">
            {canSignIn ? (
              <Button
                type="button"
                tone="quiet"
                disabled={name.trim() === ""}
                onClick={() => setSigningIn(true)}
              >
                Sign in instead
              </Button>
            ) : (
              <span />
            )}
            <span className="flex gap-2">
              <Button type="button" tone="quiet" onClick={close}>
                Cancel
              </Button>
              <Button
                tone="primary"
                type="submit"
                disabled={name.trim() === "" || secret.trim() === ""}
              >
                Add
              </Button>
            </span>
          </div>
        </form>
      )}
    </Dialog>
  );
}
