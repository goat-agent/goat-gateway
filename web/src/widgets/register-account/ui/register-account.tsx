import { useCallback, useState } from "react";
import { LogIn } from "lucide-react";
import { Button, Dialog, toast } from "@/shared/ui";
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

  const added = useCallback(() => {
    toast(`Added ${name.trim()}`, {
      says: `Registered as a ${provider} account.`,
      tone: "good",
      id: `added/${name.trim()}`,
    });
    setName("");
    setSigningIn(false);
    onAdded();
  }, [name, provider, onAdded]);

  return (
    <Dialog
      open={open}
      onClose={close}
      title={signingIn ? "Sign in to a subscription" : "Add an account"}
      description={
        signingIn
          ? undefined
          : "Requests go to whichever registered account can serve them."
      }
    >
      {signingIn ? (
        <SignIn
          provider={provider}
          name={name}
          offered={offered}
          onDone={added}
          onCancel={() => setSigningIn(false)}
        />
      ) : (
        <KeyForm
          providers={Object.keys(models.data ?? {})}
          provider={provider}
          onProvider={setProvider}
          name={name}
          onName={setName}
          onAdded={added}
        >
          <span className="flex gap-2">
            <Button size="medium" onClick={close}>
              Cancel
            </Button>
            {offered ? (
              <Button
                tone="ghost"
                size="medium"
                disabled={name.trim() === ""}
                onClick={() => setSigningIn(true)}
              >
                <LogIn strokeWidth={1.75} />
                Sign in instead
              </Button>
            ) : null}
          </span>
        </KeyForm>
      )}
    </Dialog>
  );
}
