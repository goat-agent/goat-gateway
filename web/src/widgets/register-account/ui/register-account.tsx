import { useState } from "react";
import { Button, Dialog } from "@/shared/ui";
import { useResource } from "@/shared/api";
import type { SignInProvider } from "@/entities/provider";
import { KeyForm } from "@/features/add-account";
import { SignIn } from "@/features/oauth-signin";

export function RegisterAccount({
  open,
  onClose,
  onAdded,
}: {
  open: boolean;
  onClose: () => void;
  onAdded: () => void;
}) {
  const models = useResource<Record<string, string[]>>(open ? "/api/models" : undefined);
  const flows = useResource<{ providers: SignInProvider[] }>(
    open ? "/api/signin/providers" : undefined,
  );

  const [name, setName] = useState("");
  const [provider, setProvider] = useState("anthropic");
  const [signingIn, setSigningIn] = useState(false);

  const offered = flows.data?.providers.find((entry) => entry.provider === provider);

  const close = () => {
    setName("");
    setSigningIn(false);
    onClose();
  };

  return (
    <Dialog open={open} onClose={close} title="Add an account">
      {signingIn ? (
        <SignIn
          provider={provider}
          name={name}
          offered={offered}
          onDone={onAdded}
          onCancel={() => setSigningIn(false)}
        />
      ) : (
        <KeyForm
          providers={Object.keys(models.data ?? {})}
          provider={provider}
          onProvider={setProvider}
          name={name}
          onName={setName}
          onAdded={onAdded}
        >
          <span className="flex gap-2">
            {offered ? (
              <Button
                type="button"
                tone="quiet"
                disabled={name.trim() === ""}
                onClick={() => setSigningIn(true)}
              >
                Sign in instead
              </Button>
            ) : null}
            <Button type="button" tone="quiet" onClick={close}>
              Cancel
            </Button>
          </span>
        </KeyForm>
      )}
    </Dialog>
  );
}
