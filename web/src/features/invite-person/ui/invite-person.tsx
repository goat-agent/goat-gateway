import { Check, Copy } from "lucide-react";
import { useState } from "react";
import { Button, Dialog, Field, Input, SegmentedControl } from "@/shared/ui";
import { send } from "@/shared/api";
import type { Role, User } from "@/entities/user";

const ROLES = [
  { value: "member", label: "Member" },
  { value: "admin", label: "Admin" },
] as const;

export function inviteLink(token: string) {
  return `${window.location.origin}/invite/${token}`;
}

export function InviteLink({ link }: { link: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <div className="flex flex-col gap-3">
      <code className="mono block rounded-lg hairline border-line bg-base px-3 py-2.5 break-all text-ink-title select-all">
        {link}
      </code>
      <div className="flex items-center justify-between gap-3">
        <span className="text-mini text-ink-secondary">
          Works once, for seven days. It sets their password and signs them in.
        </span>
        <Button
          size="medium"
          onClick={() => {
            void navigator.clipboard.writeText(link).then(
              () => setCopied(true),
              () => setCopied(false),
            );
          }}
        >
          {copied ? <Check strokeWidth={1.75} /> : <Copy strokeWidth={1.75} />}
          {copied ? "Copied" : "Copy link"}
        </Button>
      </div>
    </div>
  );
}

export function InvitePerson({
  open,
  onClose,
  onInvited,
}: {
  open: boolean;
  onClose: () => void;
  onInvited: () => void;
}) {
  const [name, setName] = useState("");
  const [role, setRole] = useState<Role>("member");
  const [link, setLink] = useState<string>();
  const [refused, setRefused] = useState<string>();

  const close = () => {
    setName("");
    setRole("member");
    setLink(undefined);
    setRefused(undefined);
    onClose();
  };

  return (
    <Dialog
      open={open}
      onClose={close}
      title={link ? `Send this to ${name}` : "Add a person"}
      description={
        link
          ? "Anyone holding the link can become this person, so send it the way you would send a password."
          : "Each person signs in with their own password, owns the plans they sign in to, and sees only their own keys and requests."
      }
    >
      {link ? (
        <div className="flex flex-col gap-4">
          <InviteLink link={link} />
          <div className="flex justify-end">
            <Button tone="primary" size="medium" onClick={close}>
              Done
            </Button>
          </div>
        </div>
      ) : (
        <form
          className="flex flex-col gap-4"
          onSubmit={(event) => {
            event.preventDefault();
            setRefused(undefined);
            send<{ user: User; invite: string }>("/api/users", { name: name.trim(), role })
              .then((created) => {
                setLink(inviteLink(created.invite));
                onInvited();
              })
              .catch((error: Error) => setRefused(error.message));
          }}
        >
          <Field label="Name" hint="What they type to sign in." refused={refused}>
            <Input autoFocus value={name} onChange={(event) => setName(event.target.value)} />
          </Field>
          <Field label="Role" hint="Admins see everyone's accounts and requests, and manage people.">
            <SegmentedControl label="Role" value={role} choices={ROLES} onChange={setRole} />
          </Field>
          <div className="flex justify-end gap-2 pt-1">
            <Button size="medium" onClick={close}>
              Cancel
            </Button>
            <Button tone="primary" size="medium" type="submit" disabled={name.trim() === ""}>
              Add and get a link
            </Button>
          </div>
        </form>
      )}
    </Dialog>
  );
}
