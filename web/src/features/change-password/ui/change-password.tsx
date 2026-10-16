import { useState } from "react";
import { KeyRound } from "lucide-react";
import { Button, Dialog, Field, Input, toast } from "@/shared/ui";
import { tell } from "@/shared/api";

const SHORTEST = 8;

export function ChangePassword() {
  const [open, setOpen] = useState(false);
  const [password, setPassword] = useState("");
  const [again, setAgain] = useState("");
  const [refused, setRefused] = useState<string>();
  const [saving, setSaving] = useState(false);

  const close = () => {
    setOpen(false);
    setPassword("");
    setAgain("");
    setRefused(undefined);
  };

  const short = password.length > 0 && password.length < SHORTEST;
  const differs = again.length > 0 && again !== password;

  return (
    <>
      <Button onClick={() => setOpen(true)}>
        <KeyRound strokeWidth={1.75} />
        Change password
      </Button>
      <Dialog
        open={open}
        onClose={close}
        width="narrow"
        title="Change your password"
        description="Sessions already open in other browsers stay signed in."
      >
        <form
          className="flex flex-col gap-4"
          onSubmit={(event) => {
            event.preventDefault();
            setRefused(undefined);
            setSaving(true);
            tell("/api/me/password", { password })
              .then(() => {
                toast("Your password changed", { tone: "good" });
                close();
              })
              .catch((error: Error) => setRefused(error.message))
              .finally(() => setSaving(false));
          }}
        >
          <Field
            label="New password"
            hint={`At least ${SHORTEST} characters.`}
            refused={short ? `Use at least ${SHORTEST} characters.` : undefined}
          >
            <Input
              type="password"
              autoComplete="new-password"
              value={password}
              onChange={(event) => setPassword(event.target.value)}
            />
          </Field>
          <Field
            label="The same again"
            refused={differs ? "The two do not match." : refused}
          >
            <Input
              type="password"
              autoComplete="new-password"
              value={again}
              onChange={(event) => setAgain(event.target.value)}
            />
          </Field>
          <div className="flex justify-end pt-1">
            <Button
              type="submit"
              tone="primary"
              size="medium"
              disabled={password.length < SHORTEST || again !== password || saving}
            >
              {saving ? "Saving…" : "Change password"}
            </Button>
          </div>
        </form>
      </Dialog>
    </>
  );
}
